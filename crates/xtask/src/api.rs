//! Language-neutral binding signatures. Numeric semantics are explicit in the
//! specification; names never determine a parameter's type.
use crate::{to_lower_camel_case, to_snake_case};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApiType {
    String,
    Number,
    Integer,
    Id,
    Boolean,
    Void,
    Null,
    Named(String),
    Literal(String),
    Array(Box<Self>),
    Union(Vec<Self>),
    Generic(String, Vec<Self>),
    Callback(Vec<ApiParameter>, Box<Self>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApiParameter {
    pub name: String,
    pub python_name: Option<String>,
    pub ty: ApiType,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApiFunction {
    pub legacy_name: String,
    pub factory_name: String,
    pub required: Vec<ApiParameter>,
    pub optional: Vec<ApiParameter>,
    pub return_type: ApiType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApiMember {
    Constructor(Vec<ApiParameter>),
    Property {
        name: String,
        ty: ApiType,
        readonly: bool,
    },
    Method {
        name: String,
        parameters: Vec<ApiParameter>,
        result: ApiType,
        is_static: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApiDecl {
    Class(Vec<ApiMember>),
    Function(ApiFunction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Literal(String),
    Punct(char),
    Arrow,
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(source: &str) -> Result<Self, String> {
        let mut chars = source.char_indices().peekable();
        let mut tokens = Vec::new();
        while let Some((start, c)) = chars.next() {
            if c.is_whitespace() {
                continue;
            }
            if c == '"' || c == '\'' {
                let mut escaped = false;
                let mut end = None;
                for (index, next) in chars.by_ref() {
                    if !escaped && next == c {
                        end = Some(index + next.len_utf8());
                        break;
                    }
                    escaped = next == '\\' && !escaped;
                }
                let end = end.ok_or_else(|| "unterminated API literal".to_string())?;
                tokens.push(Token::Literal(source[start..end].to_string()));
            } else if c.is_ascii_alphabetic() || c == '_' {
                let mut end = start + c.len_utf8();
                while chars.peek().is_some_and(|(_, next)| {
                    next.is_ascii_alphanumeric() || *next == '_' || *next == '.'
                }) {
                    let (index, next) = chars.next().unwrap();
                    end = index + next.len_utf8();
                }
                tokens.push(Token::Word(source[start..end].to_string()));
            } else if c == '=' && chars.peek().is_some_and(|(_, next)| *next == '>') {
                chars.next();
                tokens.push(Token::Arrow);
            } else if "()[]<>:;,?|".contains(c) {
                tokens.push(Token::Punct(c));
            } else {
                return Err(format!("unsupported API signature character `{c}`"));
            }
        }
        Ok(Self { tokens, index: 0 })
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }
    fn take(&mut self, token: Token) -> bool {
        if self.peek() == Some(&token) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.take(token.clone()) {
            Ok(())
        } else {
            Err(format!("expected {token:?}, found {:?}", self.peek()))
        }
    }
    fn word(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(Token::Word(value)) => {
                self.index += 1;
                Ok(value)
            }
            other => Err(format!("expected API identifier, found {other:?}")),
        }
    }
    fn finish(&mut self) -> Result<(), String> {
        self.take(Token::Punct(';'));
        if self.peek().is_none() {
            Ok(())
        } else {
            Err(format!("unexpected API signature suffix {:?}", self.peek()))
        }
    }
    fn parameters(&mut self) -> Result<Vec<ApiParameter>, String> {
        self.expect(Token::Punct('('))?;
        let mut parameters = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        let mut optional_seen = false;
        if self.take(Token::Punct(')')) {
            return Ok(parameters);
        }
        loop {
            let name = self.word()?;
            if !names.insert(name.clone()) {
                return Err(format!("duplicate API parameter `{name}`"));
            }
            let optional = self.take(Token::Punct('?'));
            let python_name = if self.take(Token::Word("as".into())) {
                Some(self.word()?)
            } else {
                None
            };
            self.expect(Token::Punct(':'))?;
            let ty = self.ty()?;
            if optional_seen && !optional {
                return Err("required parameters must precede optional parameters".into());
            }
            optional_seen |= optional;
            parameters.push(ApiParameter {
                name,
                python_name,
                ty,
                optional,
            });
            if self.take(Token::Punct(')')) {
                break;
            }
            self.expect(Token::Punct(','))?;
        }
        Ok(parameters)
    }
    fn ty(&mut self) -> Result<ApiType, String> {
        let mut values = vec![self.atom()?];
        while self.take(Token::Punct('|')) {
            values.push(self.atom()?);
        }
        Ok(if values.len() == 1 {
            values.pop().unwrap()
        } else {
            ApiType::Union(values)
        })
    }
    fn atom(&mut self) -> Result<ApiType, String> {
        let mut value = match self.peek().cloned() {
            Some(Token::Literal(value)) => {
                self.index += 1;
                ApiType::Literal(value)
            }
            Some(Token::Punct('(')) => {
                let saved = self.index;
                if let Ok(parameters) = self.parameters() {
                    if self.take(Token::Arrow) {
                        ApiType::Callback(parameters, Box::new(self.ty()?))
                    } else {
                        self.index = saved;
                        self.expect(Token::Punct('('))?;
                        let ty = self.ty()?;
                        self.expect(Token::Punct(')'))?;
                        ty
                    }
                } else {
                    self.index = saved;
                    self.expect(Token::Punct('('))?;
                    let ty = self.ty()?;
                    self.expect(Token::Punct(')'))?;
                    ty
                }
            }
            Some(Token::Word(_)) => {
                let name = self.word()?;
                if self.take(Token::Punct('<')) {
                    let mut types = vec![self.ty()?];
                    while self.take(Token::Punct(',')) {
                        types.push(self.ty()?);
                    }
                    self.expect(Token::Punct('>'))?;
                    ApiType::Generic(name, types)
                } else {
                    match name.as_str() {
                        "string" => ApiType::String,
                        "number" => ApiType::Number,
                        "int" => ApiType::Integer,
                        "id" => ApiType::Id,
                        "boolean" => ApiType::Boolean,
                        "void" => ApiType::Void,
                        "null" => ApiType::Null,
                        _ => ApiType::Named(name),
                    }
                }
            }
            other => return Err(format!("expected API type, found {other:?}")),
        };
        while self.take(Token::Punct('[')) {
            self.expect(Token::Punct(']'))?;
            value = ApiType::Array(Box::new(value));
        }
        Ok(value)
    }
}

impl ApiType {
    #[cfg(test)]
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut parser = Parser::new(source)?;
        let ty = parser.ty()?;
        parser.finish()?;
        Ok(ty)
    }
    pub fn typescript(&self) -> String {
        match self {
            Self::String | Self::Id => "string".into(),
            Self::Number | Self::Integer => "number".into(),
            Self::Boolean => "boolean".into(),
            Self::Void => "void".into(),
            Self::Null => "null".into(),
            Self::Named(name) | Self::Literal(name) => name.clone(),
            Self::Array(ty) => {
                if matches!(ty.as_ref(), Self::Union(_) | Self::Callback(..)) {
                    format!("({})[]", ty.typescript())
                } else {
                    format!("{}[]", ty.typescript())
                }
            }
            Self::Union(types) => types
                .iter()
                .map(Self::typescript)
                .collect::<Vec<_>>()
                .join(" | "),
            Self::Generic(name, args) => format!(
                "{name}<{}>",
                args.iter()
                    .map(Self::typescript)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Callback(parameters, result) => {
                format!("({}) => {}", ts_parameters(parameters), result.typescript())
            }
        }
    }
    pub fn python(&self) -> String {
        match self {
            Self::String => "str".into(),
            Self::Number => "float".into(),
            Self::Integer | Self::Id => "int".into(),
            Self::Boolean => "bool".into(),
            Self::Void | Self::Null => "None".into(),
            Self::Named(name) => name.clone(),
            Self::Literal(value) => format!("Literal[{value}]"),
            Self::Array(ty) => format!("Sequence[{}]", ty.python()),
            Self::Union(types) if types.iter().all(|ty| matches!(ty, Self::Literal(_))) => format!(
                "Literal[{}]",
                types
                    .iter()
                    .map(|ty| if let Self::Literal(value) = ty {
                        value.clone()
                    } else {
                        unreachable!()
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Union(types) => types
                .iter()
                .map(Self::python)
                .collect::<Vec<_>>()
                .join(" | "),
            Self::Generic(name, args) => format!(
                "{}[{}]",
                match name.as_str() {
                    "Record" => "Mapping",
                    "Array" => "Sequence",
                    _ => name,
                },
                args.iter().map(Self::python).collect::<Vec<_>>().join(", ")
            ),
            Self::Callback(parameters, result) => format!(
                "Callable[[{}], {}]",
                parameters
                    .iter()
                    .map(|p| p.ty.python())
                    .collect::<Vec<_>>()
                    .join(", "),
                result.python()
            ),
        }
    }
}

fn ts_parameters(parameters: &[ApiParameter]) -> String {
    parameters
        .iter()
        .map(|p| {
            format!(
                "{}{}: {}",
                p.name,
                if p.optional { "?" } else { "" },
                p.ty.typescript()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn python_parameters(parameters: &[ApiParameter], has_self: bool) -> String {
    let mut values = if has_self {
        vec!["self".to_string()]
    } else {
        Vec::new()
    };
    for parameter in parameters {
        let mut ty = parameter.ty.python();
        if parameter.optional && !ty.split('|').any(|part| part.trim() == "None") {
            ty.push_str(" | None");
        }
        values.push(format!(
            "{}: {ty}{}",
            parameter
                .python_name
                .clone()
                .unwrap_or_else(|| to_snake_case(&parameter.name)),
            if parameter.optional { " = ..." } else { "" }
        ));
    }
    values.join(", ")
}

impl ApiFunction {
    pub fn parse(name: &str, source: &str) -> Result<Self, String> {
        let mut parser = Parser::new(source)?;
        let declared = parser.word()?;
        if declared != name {
            return Err(format!("API function `{name}` declares `{declared}`"));
        }
        let parameters = parser.parameters()?;
        parser.expect(Token::Punct(':'))?;
        let return_type = parser.ty()?;
        parser.finish()?;
        let (optional, required) = parameters.into_iter().partition(|p| p.optional);
        Ok(Self {
            legacy_name: name.into(),
            factory_name: to_lower_camel_case(name),
            required,
            optional,
            return_type,
        })
    }
    pub fn typescript(&self) -> String {
        let parameters = self
            .required
            .iter()
            .chain(&self.optional)
            .cloned()
            .collect::<Vec<_>>();
        format!(
            "{}({}): {};",
            self.legacy_name,
            ts_parameters(&parameters),
            self.return_type.typescript()
        )
    }
    pub fn python(&self, name: &str) -> String {
        let parameters = self
            .required
            .iter()
            .chain(&self.optional)
            .cloned()
            .collect::<Vec<_>>();
        format!(
            "def {name}({}) -> {}: ...\n",
            python_parameters(&parameters, false),
            self.return_type.python()
        )
    }
}

impl ApiMember {
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut parser = Parser::new(source)?;
        let is_static = parser.take(Token::Word("static".into()));
        let readonly = parser.take(Token::Word("readonly".into()));
        let name = parser.word()?;
        let result = if parser.peek() == Some(&Token::Punct('(')) {
            let parameters = parser.parameters()?;
            if name == "constructor" {
                Self::Constructor(parameters)
            } else {
                parser.expect(Token::Punct(':'))?;
                Self::Method {
                    name,
                    parameters,
                    result: parser.ty()?,
                    is_static,
                }
            }
        } else {
            parser.expect(Token::Punct(':'))?;
            Self::Property {
                name,
                ty: parser.ty()?,
                readonly,
            }
        };
        parser.finish()?;
        Ok(result)
    }
    pub fn typescript(&self) -> String {
        match self {
            Self::Constructor(p) => format!("constructor({});", ts_parameters(p)),
            Self::Property { name, ty, readonly } => format!(
                "{}{name}: {};",
                if *readonly { "readonly " } else { "" },
                ty.typescript()
            ),
            Self::Method {
                name,
                parameters,
                result,
                is_static,
            } => format!(
                "{}{name}({}): {};",
                if *is_static { "static " } else { "" },
                ts_parameters(parameters),
                result.typescript()
            ),
        }
    }
    pub fn python(&self) -> String {
        match self {
            Self::Constructor(p) => format!(
                "    def __init__({}) -> None: ...\n",
                python_parameters(p, true)
            ),
            Self::Property {
                name,
                ty,
                readonly: true,
            } => format!(
                "    @property\n    def {}(self) -> {}: ...\n",
                to_snake_case(name),
                ty.python()
            ),
            Self::Property {
                name,
                ty,
                readonly: false,
            } => format!("    {}: {}\n", to_snake_case(name), ty.python()),
            Self::Method {
                name,
                parameters,
                result,
                is_static,
            } => format!(
                "{}    def {}({}) -> {}: ...\n",
                if *is_static {
                    "    @staticmethod\n"
                } else {
                    ""
                },
                to_snake_case(name),
                python_parameters(parameters, !is_static),
                result.python()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_types_survive_parameter_renames() {
        for name in ["index", "rowIndex", "arbitraryName"] {
            let function =
                ApiFunction::parse("Choose", &format!("Choose({name}: int): boolean;")).unwrap();
            assert!(function.python("choose").contains(": int"));
            assert!(function.typescript().contains(": number"));
            let floating =
                ApiFunction::parse("Choose", &format!("Choose({name}: number): boolean;")).unwrap();
            assert!(floating.python("choose").contains(": float"));
        }
    }
    #[test]
    fn nested_callbacks_arrays_and_literal_unions_are_structural() {
        let ty =
            ApiType::parse("(select: (row: int) => string[], mode: \"a|b\" | \"c\") => boolean")
                .unwrap();
        assert_eq!(
            ty.python(),
            "Callable[[Callable[[int], Sequence[str]], Literal[\"a|b\", \"c\"]], bool]"
        );
        assert_eq!(
            ApiType::parse(&ty.typescript()).unwrap().typescript(),
            ty.typescript()
        );
    }
    #[test]
    fn invalid_signatures_are_rejected() {
        assert!(ApiFunction::parse("A", "A(x: int, x: int): void;").is_err());
        assert!(ApiFunction::parse("A", "A(x?: int, y: int): void;").is_err());
        assert!(ApiType::parse("string trailing").is_err());
    }
    #[test]
    fn serialized_identifiers_have_explicit_language_representations() {
        assert_eq!(ApiType::Id.typescript(), "string");
        assert_eq!(ApiType::Id.python(), "int");
    }
    #[test]
    fn explicit_python_parameter_aliases_preserve_host_api_names() {
        let function =
            ApiFunction::parse("Slider", "Slider(min? as min_value: number): Widget;").unwrap();
        assert_eq!(function.typescript(), "Slider(min?: number): Widget;");
        assert_eq!(
            function.python("slider"),
            "def slider(min_value: float | None = ...) -> Widget: ...\n"
        );
    }
}
