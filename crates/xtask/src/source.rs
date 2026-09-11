//! Rust declaration and module-tree inspection used by binding verification.
use quote::ToTokens;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use syn::{
    Item, Meta, Visibility,
    visit::{self, Visit},
};

#[derive(Default, Debug)]
pub(crate) struct RustSource {
    pub structs: BTreeSet<String>,
    pub functions: BTreeSet<String>,
    pub methods: BTreeSet<(String, String)>,
    pub variants: BTreeSet<(String, String)>,
    pub python_functions: BTreeSet<(String, String)>,
    pub python_classes: BTreeSet<(String, String)>,
    pub js_functions: BTreeSet<(String, String)>,
    pub js_classes: BTreeSet<(String, String)>,
    pub registered_functions: BTreeSet<String>,
    pub registered_classes: BTreeSet<String>,
    pub fixtures: BTreeMap<String, String>,
}

impl RustSource {
    pub fn parse(source: &str) -> Result<Self, String> {
        let parsed = syn::parse_file(source).map_err(|e| e.to_string())?;
        let mut result = Self::default();
        result.read_items(
            &parsed.items,
            Path::new("."),
            Path::new("."),
            &mut BTreeSet::new(),
            false,
        )?;
        Ok(result)
    }
    pub fn read(root: &Path) -> Result<Self, String> {
        let mut result = Self::default();
        result.read_file(root, &mut BTreeSet::new())?;
        Ok(result)
    }

    fn read_file(&mut self, path: &Path, visited: &mut BTreeSet<PathBuf>) -> Result<(), String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if !visited.insert(path.clone()) {
            return Ok(());
        }
        let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let file = syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().unwrap();
        let module_dir = if matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("lib.rs" | "main.rs" | "mod.rs")
        ) {
            base.to_path_buf()
        } else {
            base.join(path.file_stem().unwrap())
        };
        self.read_items(&file.items, base, &module_dir, visited, false)
    }

    fn read_items(
        &mut self,
        items: &[Item],
        file_dir: &Path,
        module_dir: &Path,
        visited: &mut BTreeSet<PathBuf>,
        tests: bool,
    ) -> Result<(), String> {
        for item in items {
            let tests = tests || test_only(item_attributes(item));
            match item {
                Item::Struct(value) if !tests && matches!(value.vis, Visibility::Public(_)) => {
                    let name = value.ident.to_string();
                    self.structs.insert(name.clone());
                    if let Some(export) = attribute_name(&value.attrs, "pyclass", "name") {
                        self.python_classes.insert((export, name.clone()));
                    }
                    if let Some(export) = attribute_name(&value.attrs, "napi", "js_name") {
                        self.js_classes.insert((export, name));
                    }
                }
                Item::Enum(value) if !tests => {
                    for variant in &value.variants {
                        self.variants
                            .insert((value.ident.to_string(), variant.ident.to_string()));
                    }
                }
                Item::Fn(value) => {
                    let name = value.sig.ident.to_string();
                    if tests || value.attrs.iter().any(|attr| attr.path().is_ident("test")) {
                        self.fixtures
                            .insert(name.clone(), value.block.to_token_stream().to_string());
                    }
                    if !tests && matches!(value.vis, Visibility::Public(_)) {
                        self.functions.insert(name.clone());
                        if let Some(export) = attribute_name(&value.attrs, "pyfunction", "name") {
                            self.python_functions.insert((export, name.clone()));
                        }
                        if let Some(export) = attribute_name(&value.attrs, "napi", "js_name") {
                            self.js_functions.insert((export, name));
                        }
                    }
                }
                Item::Impl(value) if !tests => {
                    if let syn::Type::Path(owner) = value.self_ty.as_ref() {
                        let owner = owner.path.segments.last().unwrap().ident.to_string();
                        for item in &value.items {
                            if let syn::ImplItem::Fn(method) = item
                                && matches!(method.vis, Visibility::Public(_))
                            {
                                self.methods
                                    .insert((owner.clone(), method.sig.ident.to_string()));
                            }
                        }
                    }
                }
                Item::Mod(value) => {
                    let is_test = tests;
                    if let Some((_, children)) = &value.content {
                        self.read_items(
                            children,
                            file_dir,
                            &module_dir.join(value.ident.to_string()),
                            visited,
                            is_test,
                        )?;
                    } else {
                        let explicit = value.attrs.iter().find_map(|attr| {
                            if !attr.path().is_ident("path") {
                                return None;
                            }
                            if let Meta::NameValue(meta) = &attr.meta
                                && let syn::Expr::Lit(expr) = &meta.value
                                && let syn::Lit::Str(path) = &expr.lit
                            {
                                return Some(file_dir.join(path.value()));
                            }
                            None
                        });
                        let file = explicit.unwrap_or_else(|| {
                            let candidate = module_dir.join(format!("{}.rs", value.ident));
                            if candidate.exists() {
                                candidate
                            } else {
                                module_dir.join(value.ident.to_string()).join("mod.rs")
                            }
                        });
                        if is_test {
                            if file.exists() {
                                let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
                                let parsed = syn::parse_file(&text).map_err(|e| e.to_string())?;
                                self.read_items(
                                    &parsed.items,
                                    file.parent().unwrap(),
                                    file.parent().unwrap(),
                                    visited,
                                    true,
                                )?;
                            }
                        } else {
                            self.read_file(&file, visited)?;
                        }
                    }
                }
                Item::Macro(value) if value.mac.path.is_ident("include") && !tests => {
                    if let Ok(file) = syn::parse2::<syn::LitStr>(value.mac.tokens.clone()) {
                        self.read_file(&file_dir.join(file.value()), visited)?;
                    }
                }
                _ => {}
            }
            if !tests && !matches!(item, Item::Mod(_)) {
                RegistrationVisitor(self).visit_item(item);
            }
        }
        Ok(())
    }
}

fn item_attributes(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Struct(v) => &v.attrs,
        Item::Enum(v) => &v.attrs,
        Item::Fn(v) => &v.attrs,
        Item::Impl(v) => &v.attrs,
        Item::Mod(v) => &v.attrs,
        Item::Macro(v) => &v.attrs,
        _ => &[],
    }
}

fn test_only(attrs: &[syn::Attribute]) -> bool {
    fn requires_test(meta: &Meta) -> bool {
        match meta {
            Meta::Path(path) => path.is_ident("test"),
            Meta::List(list) => {
                let Ok(parts) = list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) else {
                    return false;
                };
                if list.path.is_ident("all") {
                    parts.iter().any(requires_test)
                } else if list.path.is_ident("any") {
                    !parts.is_empty() && parts.iter().all(requires_test)
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg"))
        .any(|a| a.parse_args::<Meta>().is_ok_and(|m| requires_test(&m)))
}

pub(crate) fn public_widgets(
    source: &str,
) -> Result<(BTreeMap<String, usize>, BTreeSet<String>), String> {
    fn walk(
        items: &[Item],
        types: &mut BTreeMap<String, usize>,
        implementations: &mut BTreeSet<String>,
    ) {
        for item in items {
            if test_only(item_attributes(item)) {
                continue;
            }
            match item {
                Item::Struct(v) if matches!(v.vis, Visibility::Public(_)) => {
                    types.insert(v.ident.to_string(), v.struct_token.span.start().line);
                }
                Item::Enum(v) if matches!(v.vis, Visibility::Public(_)) => {
                    types.insert(v.ident.to_string(), v.enum_token.span.start().line);
                }
                Item::Impl(v) => {
                    if v.trait_.as_ref().is_some_and(|(_, path, _)| {
                        path.segments
                            .last()
                            .is_some_and(|part| part.ident == "Widget")
                    }) && let syn::Type::Path(path) = v.self_ty.as_ref()
                    {
                        implementations
                            .insert(path.path.segments.last().unwrap().ident.to_string());
                    }
                }
                Item::Mod(v) => {
                    if let Some((_, items)) = &v.content {
                        walk(items, types, implementations)
                    }
                }
                _ => {}
            }
        }
    }
    let parsed = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut types = BTreeMap::new();
    let mut implementations = BTreeSet::new();
    walk(&parsed.items, &mut types, &mut implementations);
    Ok((types, implementations))
}

/// Index top-level TypeScript declarations, excluding comments and literals.
/// Their full generated signatures are checked against the typed API model by
/// `bindings generate --check`.
pub(crate) fn typescript_exports(source: &str) -> BTreeSet<(String, String)> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = ' ';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            continue;
        }
        if matches!(c, '\'' | '"' | '`') {
            let mut escaped = false;
            for next in chars.by_ref() {
                if !escaped && next == c {
                    break;
                }
                escaped = next == '\\' && !escaped;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let mut word = String::from(c);
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                word.push(chars.next().unwrap());
            }
            tokens.push(word);
        } else if !c.is_whitespace() {
            tokens.push(c.to_string());
        }
    }
    let mut depth = 0_usize;
    let mut exports = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if depth == 0 && token == "export" {
            let mut offset = index + 1;
            if tokens.get(offset).is_some_and(|t| t == "declare") {
                offset += 1;
            }
            if let (Some(kind), Some(name)) = (tokens.get(offset), tokens.get(offset + 1))
                && matches!(kind.as_str(), "class" | "function")
            {
                exports.insert((kind.clone(), name.clone()));
            }
        }
        if token == "{" {
            depth += 1;
        } else if token == "}" {
            depth = depth.saturating_sub(1);
        }
    }
    exports
}

fn attribute_name(attrs: &[syn::Attribute], attribute: &str, key: &str) -> Option<String> {
    for attr in attrs.iter().filter(|attr| attr.path().is_ident(attribute)) {
        let entries = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            .ok()?;
        for entry in entries {
            if let Meta::NameValue(value) = entry
                && value.path.is_ident(key)
                && let syn::Expr::Lit(expr) = value.value
                && let syn::Lit::Str(value) = expr.lit
            {
                return Some(value.value());
            }
        }
    }
    None
}

struct RegistrationVisitor<'a>(&'a mut RustSource);
impl<'ast> Visit<'ast> for RegistrationVisitor<'_> {
    fn visit_expr_method_call(&mut self, value: &'ast syn::ExprMethodCall) {
        if value.method == "add_class"
            && let Some(args) = &value.turbofish
        {
            for arg in &args.args {
                if let syn::GenericArgument::Type(syn::Type::Path(path)) = arg {
                    self.0
                        .registered_classes
                        .insert(path.path.segments.last().unwrap().ident.to_string());
                }
            }
        }
        visit::visit_expr_method_call(self, value);
    }
    fn visit_macro(&mut self, value: &'ast syn::Macro) {
        if value.path.is_ident("wrap_pyfunction")
            && let Some(proc_macro2::TokenTree::Ident(name)) =
                value.tokens.clone().into_iter().next()
        {
            self.0.registered_functions.insert(name.to_string());
        }
        visit::visit_macro(self, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_strings_and_test_only_declarations_do_not_count_as_exports() {
        let source = RustSource::parse(
            r#"
            // #[napi(js_name = "Fake")] pub struct JsFake;
            const TEXT: &str = "pub struct Fake;";
            #[cfg(test)] #[napi(js_name = "TestOnly")] pub struct JsTestOnly;
            #[napi(js_name = "Live")] pub struct JsLive;
            #[pyfunction(name = "Run")] pub fn py_run() {}
        "#,
        )
        .unwrap();
        assert!(!source.structs.contains("Fake"));
        assert!(!source.structs.contains("JsFake"));
        assert!(!source.structs.contains("JsTestOnly"));
        assert!(
            source
                .js_classes
                .contains(&("Live".into(), "JsLive".into()))
        );
        assert!(
            source
                .python_functions
                .contains(&("Run".into(), "py_run".into()))
        );
        assert!(!source.registered_functions.contains("py_run"));
    }
    #[test]
    fn exported_declarations_follow_external_modules_and_include_files() {
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let dir = temp.join(format!("sui-source-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("lib.rs"),
            "#[path = \"renamed.rs\"] mod api; include!(\"generated.rs\");",
        )
        .unwrap();
        fs::write(
            dir.join("renamed.rs"),
            "#[napi(js_name=\"Live\")] pub struct JsLive;",
        )
        .unwrap();
        fs::write(dir.join("generated.rs"),"#[pyfunction(name=\"Run\")] pub fn py_run() {} fn register() { wrap_pyfunction!(py_run, m); }").unwrap();
        let source = RustSource::read(&dir.join("lib.rs")).unwrap();
        assert!(
            source
                .js_classes
                .contains(&("Live".into(), "JsLive".into()))
        );
        assert!(source.registered_functions.contains("py_run"));
        let resolved = dir.canonicalize().unwrap();
        assert!(resolved.starts_with(&temp));
        fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    fn typescript_comments_and_literals_are_not_declarations() {
        let exports = typescript_exports(
            "// export class Fake {}\nconst text = 'export function Fake()'; export declare class Real {} export function Run(): void;",
        );
        assert_eq!(
            exports,
            BTreeSet::from([
                ("class".into(), "Real".into()),
                ("function".into(), "Run".into())
            ])
        );
    }
}
