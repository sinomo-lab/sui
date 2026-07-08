use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, subcommand] if command == "bindings" && subcommand == "coverage" => {
            bindings_coverage()
        }
        [command, subcommand] if command == "bindings" && subcommand == "check" => {
            bindings_coverage()
        }
        [command, subcommand] if command == "bindings" && subcommand == "generate" => {
            bindings_generate(false)
        }
        [command, subcommand, flag]
            if command == "bindings" && subcommand == "generate" && flag == "--check" =>
        {
            bindings_generate(true)
        }
        _ => {
            eprintln!("usage: cargo xtask bindings coverage");
            eprintln!("       cargo xtask bindings check");
            eprintln!("       cargo xtask bindings generate [--check]");
            Err("unknown xtask command".to_string())
        }
    }
}

fn bindings_generate(check: bool) -> Result<(), String> {
    let root = workspace_root()?;
    let spec_path = root.join("bindings/widgets.sui");
    let spec = fs::read_to_string(&spec_path)
        .map_err(|error| format!("failed to read {}: {error}", spec_path.display()))?;
    let items = parse_binding_spec(&spec)?;

    let manifest_path = root.join("bindings/widgets.toml");
    let manifest = render_manifest(&items);
    update_generated_file(&manifest_path, &manifest, check)?;

    let python_generated_path = root.join("crates/sui-python/src/generated_widgets.rs");
    let python_template_path = root.join("bindings/templates/python_widgets.rs.in");
    let python_template = fs::read_to_string(&python_template_path)
        .map_err(|error| format!("failed to read {}: {error}", python_template_path.display()))?;
    validate_rust_template("python", &items, &python_template)?;
    let python_generated = render_generated_rust_template(
        "Python widget bindings",
        "bindings/templates/python_widgets.rs.in",
        &python_template,
    );
    update_generated_file(&python_generated_path, &python_generated, check)?;

    let js_generated_path = root.join("crates/sui-js/src/generated_widgets.rs");
    let js_template_path = root.join("bindings/templates/js_widgets.rs.in");
    let js_template = fs::read_to_string(&js_template_path)
        .map_err(|error| format!("failed to read {}: {error}", js_template_path.display()))?;
    validate_rust_template("js", &items, &js_template)?;
    let js_generated = render_generated_rust_template(
        "JavaScript widget bindings",
        "bindings/templates/js_widgets.rs.in",
        &js_template,
    );
    update_generated_file(&js_generated_path, &js_generated, check)?;

    let ts_path = root.join("crates/sui-js/index.d.ts");
    let current_ts = fs::read_to_string(&ts_path)
        .map_err(|error| format!("failed to read {}: {error}", ts_path.display()))?;
    let generated_ts = render_generated_ts(&items);
    let next_ts = update_ts_index(&current_ts, &generated_ts, &items)?;
    update_generated_file(&ts_path, &next_ts, check)?;

    if check {
        println!("binding generated files are up to date");
    } else {
        println!("generated {}", relative_display(&root, &manifest_path));
        println!(
            "generated {}",
            relative_display(&root, &python_generated_path)
        );
        println!("generated {}", relative_display(&root, &js_generated_path));
        println!("generated {}", relative_display(&root, &ts_path));
    }

    Ok(())
}

fn bindings_coverage() -> Result<(), String> {
    let root = workspace_root()?;
    let manifest_path = root.join("bindings/widgets.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let items = parse_manifest(&manifest)?;

    let sources = Sources {
        core: read_source(&root, "crates/sui-bindings-core/src/lib.rs")?,
        python: read_sources(
            &root,
            &[
                "crates/sui-python/src/lib.rs",
                "crates/sui-python/src/generated_widgets.rs",
            ],
        )?,
        js: read_sources(
            &root,
            &[
                "crates/sui-js/src/lib.rs",
                "crates/sui-js/src/generated_widgets.rs",
            ],
        )?,
        ts: read_source(&root, "crates/sui-js/index.d.ts")?,
        docs: read_sources(
            &root,
            &[
                "docs/plans/cross-language-bindings-plan.md",
                "crates/sui-python/README.md",
                "crates/sui-js/README.md",
            ],
        )?,
    };

    let rows = items
        .iter()
        .map(|item| CoverageRow::for_item(item, &sources))
        .collect::<Vec<_>>();

    print_report(&rows);

    let missing = rows
        .iter()
        .filter(|row| row.has_required_gap())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} binding item(s) are missing required coverage",
            missing.len()
        ))
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    let mut current =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    loop {
        if current.join("Cargo.toml").is_file() && current.join("crates").is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err("failed to find workspace root".to_string());
        }
    }
}

fn read_source(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn read_sources(root: &Path, relatives: &[&str]) -> Result<String, String> {
    let mut combined = String::new();
    for relative in relatives {
        combined.push_str(&read_source(root, relative)?);
        combined.push('\n');
    }
    Ok(combined)
}

#[derive(Clone, Debug, Default)]
struct Item {
    name: String,
    kind: String,
    core_kind: Option<String>,
    core_constructor: Option<String>,
    core_descriptor: Option<String>,
    python_kind: Option<String>,
    python_rust: Option<String>,
    js_kind: Option<String>,
    js_rust: Option<String>,
    ts_kind: Option<String>,
    ts_decl: Option<TsDecl>,
    docs: Vec<String>,
    compat: bool,
}

#[derive(Clone, Debug)]
enum TsDecl {
    Class(Vec<String>),
    Function(String),
}

fn parse_manifest(input: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let mut current: Option<BTreeMap<String, Value>> = None;

    for (line_index, raw_line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[[item]]" {
            if let Some(table) = current.take() {
                items.push(item_from_table(table, line_number)?);
            }
            current = Some(BTreeMap::new());
            continue;
        }

        let Some(table) = current.as_mut() else {
            return Err(format!(
                "line {line_number}: key/value pair found before [[item]]"
            ));
        };

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected `key = value`"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("line {line_number}: empty key"));
        }
        table.insert(key.to_string(), parse_value(value.trim(), line_number)?);
    }

    if let Some(table) = current.take() {
        items.push(item_from_table(table, input.lines().count())?);
    }

    if items.is_empty() {
        return Err("manifest does not contain any [[item]] entries".to_string());
    }

    Ok(items)
}

#[derive(Clone, Debug)]
enum Value {
    String(String),
    Bool(bool),
    Strings(Vec<String>),
}

fn parse_value(value: &str, line_number: usize) -> Result<Value, String> {
    if value == "true" {
        return Ok(Value::Bool(true));
    }
    if value == "false" {
        return Ok(Value::Bool(false));
    }
    if let Some(string) = parse_quoted(value) {
        return Ok(Value::String(string));
    }
    if value.starts_with('[') && value.ends_with(']') {
        let inner = value[1..value.len() - 1].trim();
        if inner.is_empty() {
            return Ok(Value::Strings(Vec::new()));
        }

        let mut strings = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            let Some(string) = parse_quoted(part) else {
                return Err(format!(
                    "line {line_number}: expected quoted string in string array"
                ));
            };
            strings.push(string);
        }
        return Ok(Value::Strings(strings));
    }

    Err(format!(
        "line {line_number}: unsupported value syntax `{value}`"
    ))
}

fn parse_quoted(value: &str) -> Option<String> {
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return None;
    }
    let inner = &value[1..value.len() - 1];
    if inner.contains('\\') {
        return None;
    }
    Some(inner.to_string())
}

fn item_from_table(mut table: BTreeMap<String, Value>, line_number: usize) -> Result<Item, String> {
    let name = take_required_string(&mut table, "name", line_number)?;
    let kind = take_required_string(&mut table, "kind", line_number)?;

    let item = Item {
        name,
        kind,
        core_kind: take_optional_string(&mut table, "core_kind", line_number)?,
        core_constructor: take_optional_string(&mut table, "core_constructor", line_number)?,
        core_descriptor: take_optional_string(&mut table, "core_descriptor", line_number)?,
        python_kind: take_optional_string(&mut table, "python_kind", line_number)?,
        python_rust: take_optional_string(&mut table, "python_rust", line_number)?,
        js_kind: take_optional_string(&mut table, "js_kind", line_number)?,
        js_rust: take_optional_string(&mut table, "js_rust", line_number)?,
        ts_kind: take_optional_string(&mut table, "ts_kind", line_number)?,
        ts_decl: None,
        docs: take_optional_strings(&mut table, "docs", line_number)?,
        compat: take_optional_bool(&mut table, "compat", line_number)?.unwrap_or(false),
    };

    if !table.is_empty() {
        return Err(format!(
            "line {line_number}: item `{}` has unsupported keys: {}",
            item.name,
            table.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    Ok(item)
}

fn take_required_string(
    table: &mut BTreeMap<String, Value>,
    key: &str,
    line_number: usize,
) -> Result<String, String> {
    take_optional_string(table, key, line_number)?
        .ok_or_else(|| format!("line {line_number}: item is missing required key `{key}`"))
}

fn take_optional_string(
    table: &mut BTreeMap<String, Value>,
    key: &str,
    line_number: usize,
) -> Result<Option<String>, String> {
    match table.remove(key) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!("line {line_number}: `{key}` must be a string")),
        None => Ok(None),
    }
}

fn take_optional_bool(
    table: &mut BTreeMap<String, Value>,
    key: &str,
    line_number: usize,
) -> Result<Option<bool>, String> {
    match table.remove(key) {
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(_) => Err(format!("line {line_number}: `{key}` must be a bool")),
        None => Ok(None),
    }
}

fn take_optional_strings(
    table: &mut BTreeMap<String, Value>,
    key: &str,
    line_number: usize,
) -> Result<Vec<String>, String> {
    match table.remove(key) {
        Some(Value::Strings(value)) => Ok(value),
        Some(_) => Err(format!(
            "line {line_number}: `{key}` must be a string array"
        )),
        None => Ok(Vec::new()),
    }
}

fn parse_binding_spec(input: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let mut current: Option<Item> = None;
    let mut ts_class_lines: Option<Vec<String>> = None;

    for (line_index, raw_line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(lines) = ts_class_lines.as_mut() {
            if line == "endts" {
                let lines = ts_class_lines.take().expect("ts class lines exist");
                let item = current.as_mut().ok_or_else(|| {
                    format!("line {line_number}: ts class block found outside a binding item")
                })?;
                item.ts_kind = Some("class".to_string());
                item.ts_decl = Some(TsDecl::Class(lines));
            } else {
                lines.push(line.to_string());
            }
            continue;
        }

        if current.is_none() {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(format!("line {line_number}: expected `<kind> <name>`"));
            }
            let kind = parts[0];
            if !matches!(
                kind,
                "descriptor" | "widget" | "alias" | "layout" | "interop-widget"
            ) {
                return Err(format!(
                    "line {line_number}: unsupported binding kind `{kind}`"
                ));
            }
            current = Some(Item {
                name: parts[1].to_string(),
                kind: kind.to_string(),
                core_kind: None,
                core_constructor: None,
                core_descriptor: None,
                python_kind: None,
                python_rust: None,
                js_kind: None,
                js_rust: None,
                ts_kind: None,
                ts_decl: None,
                docs: Vec::new(),
                compat: false,
            });
            continue;
        }

        if line == "end" {
            let item = current.take().expect("current item exists");
            validate_spec_item(&item, line_number)?;
            items.push(item);
            continue;
        }

        let item = current.as_mut().expect("current item exists");
        if line == "compat" {
            item.compat = true;
        } else if let Some(value) = line.strip_prefix("core_kind ") {
            item.core_kind = Some(parse_word(value, "core_kind", line_number)?);
        } else if let Some(value) = line.strip_prefix("core_constructor ") {
            item.core_constructor = Some(parse_word(value, "core_constructor", line_number)?);
        } else if let Some(value) = line.strip_prefix("core_descriptor ") {
            item.core_descriptor = Some(parse_word(value, "core_descriptor", line_number)?);
        } else if let Some(value) = line.strip_prefix("python ") {
            parse_language_line(
                value,
                "python",
                line_number,
                &mut item.python_kind,
                &mut item.python_rust,
            )?;
        } else if let Some(value) = line.strip_prefix("js ") {
            parse_language_line(
                value,
                "js",
                line_number,
                &mut item.js_kind,
                &mut item.js_rust,
            )?;
        } else if line == "ts class" {
            ts_class_lines = Some(Vec::new());
        } else if let Some(value) = line.strip_prefix("ts function ") {
            if !value.starts_with(&format!("{}(", item.name)) {
                return Err(format!(
                    "line {line_number}: TypeScript function signature for `{}` must start with `{}(`",
                    item.name, item.name
                ));
            }
            if !value.ends_with(';') {
                return Err(format!(
                    "line {line_number}: TypeScript function signature must end with `;`"
                ));
            }
            item.ts_kind = Some("function".to_string());
            item.ts_decl = Some(TsDecl::Function(value.to_string()));
        } else if let Some(value) = line.strip_prefix("docs ") {
            item.docs = value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
        } else {
            return Err(format!(
                "line {line_number}: unsupported binding spec directive `{line}`"
            ));
        }
    }

    if ts_class_lines.is_some() {
        return Err("unterminated TypeScript class block".to_string());
    }
    if let Some(item) = current {
        return Err(format!("binding item `{}` is missing `end`", item.name));
    }
    if items.is_empty() {
        return Err("binding spec does not contain any items".to_string());
    }

    Ok(items)
}

fn parse_word(value: &str, key: &str, line_number: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.split_whitespace().count() != 1 {
        return Err(format!("line {line_number}: `{key}` expects one word"));
    }
    Ok(value.to_string())
}

fn parse_language_line(
    value: &str,
    language: &str,
    line_number: usize,
    kind_slot: &mut Option<String>,
    rust_slot: &mut Option<String>,
) -> Result<(), String> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["class"] => {
            *kind_slot = Some("class".to_string());
            *rust_slot = None;
        }
        ["function", rust_name] => {
            *kind_slot = Some("function".to_string());
            *rust_slot = Some((*rust_name).to_string());
        }
        _ => {
            return Err(format!(
                "line {line_number}: `{language}` expects `class` or `function <rust_name>`"
            ));
        }
    }
    Ok(())
}

fn validate_spec_item(item: &Item, line_number: usize) -> Result<(), String> {
    if item.name.is_empty() {
        return Err(format!(
            "line {line_number}: binding item has an empty name"
        ));
    }
    if item.ts_kind.is_some() != item.ts_decl.is_some() {
        return Err(format!(
            "line {line_number}: item `{}` has incomplete TypeScript metadata",
            item.name
        ));
    }
    Ok(())
}

fn render_manifest(items: &[Item]) -> String {
    let mut output = String::new();
    output.push_str("# Generated by `cargo xtask bindings generate` from bindings/widgets.sui.\n");
    output.push_str("# Do not edit by hand.\n\n");

    for (index, item) in items.iter().enumerate() {
        output.push_str("[[item]]\n");
        push_toml_string(&mut output, "name", &item.name);
        push_toml_string(&mut output, "kind", &item.kind);
        if let Some(value) = &item.core_kind {
            push_toml_string(&mut output, "core_kind", value);
        }
        if let Some(value) = &item.core_constructor {
            push_toml_string(&mut output, "core_constructor", value);
        }
        if let Some(value) = &item.core_descriptor {
            push_toml_string(&mut output, "core_descriptor", value);
        }
        if let Some(value) = &item.python_kind {
            push_toml_string(&mut output, "python_kind", value);
        }
        if let Some(value) = &item.python_rust {
            push_toml_string(&mut output, "python_rust", value);
        }
        if let Some(value) = &item.js_kind {
            push_toml_string(&mut output, "js_kind", value);
        }
        if let Some(value) = &item.js_rust {
            push_toml_string(&mut output, "js_rust", value);
        }
        if let Some(value) = &item.ts_kind {
            push_toml_string(&mut output, "ts_kind", value);
        }
        if !item.docs.is_empty() {
            output.push_str("docs = [");
            for (index, doc) in item.docs.iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                output.push('"');
                output.push_str(doc);
                output.push('"');
            }
            output.push_str("]\n");
        }
        if item.compat {
            output.push_str("compat = true\n");
        }
        if index + 1 < items.len() {
            output.push('\n');
        }
    }

    output
}

fn push_toml_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = \"");
    output.push_str(value);
    output.push_str("\"\n");
}

const GENERATED_TS_START: &str = "// BEGIN GENERATED SUI WIDGET BINDINGS";
const GENERATED_TS_END: &str = "// END GENERATED SUI WIDGET BINDINGS";

fn render_generated_ts(items: &[Item]) -> String {
    let mut output = String::new();
    output.push_str(GENERATED_TS_START);
    output.push('\n');
    output.push_str("// Generated by `cargo xtask bindings generate` from bindings/widgets.sui.\n");
    output.push_str("// Do not edit this section by hand.\n\n");

    for item in items {
        match &item.ts_decl {
            Some(TsDecl::Class(lines)) => {
                output.push_str("export class ");
                output.push_str(&item.name);
                output.push_str(" {\n");
                for line in lines {
                    output.push_str("  ");
                    output.push_str(line);
                    output.push('\n');
                }
                output.push_str("}\n\n");
            }
            Some(TsDecl::Function(signature)) => {
                output.push_str("export function ");
                output.push_str(signature);
                output.push_str("\n\n");
            }
            None => {}
        }
    }

    output.push_str(GENERATED_TS_END);
    output.push('\n');
    output
}

fn validate_rust_template(language: &str, items: &[Item], template: &str) -> Result<(), String> {
    for item in items {
        match language {
            "python" => match item.python_kind.as_deref() {
                Some("class") => {
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("#[pyclass(name = \"{}\"", item.name),
                    )?;
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("pub struct Py{}", item.name),
                    )?;
                }
                Some("function") => {
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("#[pyfunction(name = \"{}\")]", item.name),
                    )?;
                    if let Some(function) = &item.python_rust {
                        require_template_contains(
                            language,
                            &item.name,
                            template,
                            &format!("pub fn {function}"),
                        )?;
                    }
                }
                Some(other) => {
                    return Err(format!(
                        "unsupported python binding kind `{other}` for `{}`",
                        item.name
                    ));
                }
                None => {}
            },
            "js" => match item.js_kind.as_deref() {
                Some("class") => {
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("#[napi(js_name = \"{}\")]", item.name),
                    )?;
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("pub struct Js{}", item.name),
                    )?;
                }
                Some("function") => {
                    require_template_contains(
                        language,
                        &item.name,
                        template,
                        &format!("#[napi(js_name = \"{}\")]", item.name),
                    )?;
                    if let Some(function) = &item.js_rust {
                        require_template_contains(
                            language,
                            &item.name,
                            template,
                            &format!("pub fn {function}"),
                        )?;
                    }
                }
                Some(other) => {
                    return Err(format!(
                        "unsupported js binding kind `{other}` for `{}`",
                        item.name
                    ));
                }
                None => {}
            },
            _ => {
                return Err(format!(
                    "unsupported generated Rust template language `{language}`"
                ));
            }
        }
    }

    Ok(())
}

fn require_template_contains(
    language: &str,
    item: &str,
    template: &str,
    needle: &str,
) -> Result<(), String> {
    if template.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "{language} widget template is missing `{needle}` for `{item}`"
        ))
    }
}

fn render_generated_rust_template(title: &str, template_path: &str, template: &str) -> String {
    let mut output = String::new();
    output
        .push_str("// Generated by `cargo xtask bindings generate` from bindings/widgets.sui and ");
    output.push_str(template_path);
    output.push_str(".\n");
    output.push_str("// Do not edit by hand.\n\n");
    output.push_str("// ");
    output.push_str(title);
    output.push_str(".\n\n");
    output.push_str(template.trim_end_matches(['\r', '\n']));
    output.push('\n');
    output
}

fn update_ts_index(current: &str, generated: &str, items: &[Item]) -> Result<String, String> {
    if current.contains(GENERATED_TS_START) || current.contains(GENERATED_TS_END) {
        return replace_generated_ts_section(current, generated);
    }

    let generated_names = items
        .iter()
        .filter(|item| item.ts_decl.is_some())
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();
    let without_generated = remove_ts_declarations(current, &generated_names)?;

    let mut output = without_generated.trim_end_matches(['\r', '\n']).to_string();
    output.push_str("\n\n");
    output.push_str(generated);
    Ok(output)
}

fn replace_generated_ts_section(current: &str, generated: &str) -> Result<String, String> {
    let start = current
        .find(GENERATED_TS_START)
        .ok_or_else(|| format!("found `{GENERATED_TS_END}` without `{GENERATED_TS_START}`"))?;
    let end_marker = current
        .find(GENERATED_TS_END)
        .ok_or_else(|| format!("found `{GENERATED_TS_START}` without `{GENERATED_TS_END}`"))?;
    let end = end_marker + GENERATED_TS_END.len();

    let mut output = String::new();
    output.push_str(current[..start].trim_end_matches(['\r', '\n']));
    output.push_str("\n\n");
    output.push_str(generated.trim_end_matches(['\r', '\n']));
    output.push_str("\n");
    output.push_str(current[end..].trim_start_matches(['\r', '\n']));
    Ok(output)
}

fn remove_ts_declarations(current: &str, names: &[&str]) -> Result<String, String> {
    let lines = current.lines().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some(name) = declaration_name(line, "export class ", names) {
            index = skip_ts_class(&lines, index, name)?;
            continue;
        }
        if let Some(name) = declaration_name(line, "export function ", names) {
            index = skip_ts_function(&lines, index, name)?;
            continue;
        }

        output.push(line);
        index += 1;
    }

    Ok(output.join("\n") + "\n")
}

fn declaration_name<'a>(line: &str, prefix: &str, names: &'a [&str]) -> Option<&'a str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(prefix)?;
    names
        .iter()
        .copied()
        .find(|name| rest.starts_with(&format!("{name} ")) || rest.starts_with(&format!("{name}(")))
}

fn skip_ts_class(lines: &[&str], start: usize, name: &str) -> Result<usize, String> {
    let mut index = start;
    while index < lines.len() {
        if lines[index].trim() == "}" {
            return Ok(index + 1);
        }
        index += 1;
    }
    Err(format!(
        "failed to find end of TypeScript class declaration `{name}`"
    ))
}

fn skip_ts_function(lines: &[&str], start: usize, name: &str) -> Result<usize, String> {
    let mut index = start;
    while index < lines.len() {
        if lines[index].trim_end().ends_with(';') {
            return Ok(index + 1);
        }
        index += 1;
    }
    Err(format!(
        "failed to find end of TypeScript function declaration `{name}`"
    ))
}

fn update_generated_file(path: &Path, content: &str, check: bool) -> Result<(), String> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    if normalize_newlines(&existing) == normalize_newlines(content) {
        return Ok(());
    }

    if check {
        return Err(format!(
            "{} is stale; run `cargo xtask bindings generate`",
            path.display()
        ));
    }

    fs::write(path, content).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

struct Sources {
    core: String,
    python: String,
    js: String,
    ts: String,
    docs: String,
}

struct CoverageRow<'a> {
    item: &'a Item,
    core: Check,
    python: Check,
    js: Check,
    ts: Check,
    docs: Check,
    compat: Check,
}

impl<'a> CoverageRow<'a> {
    fn for_item(item: &'a Item, sources: &Sources) -> Self {
        Self {
            item,
            core: check_core(item, &sources.core),
            python: check_python(item, &sources.python),
            js: check_js(item, &sources.js),
            ts: check_ts(item, &sources.ts),
            docs: check_docs(item, &sources.docs),
            compat: check_compat(item, sources),
        }
    }

    fn has_required_gap(&self) -> bool {
        [
            self.core,
            self.python,
            self.js,
            self.ts,
            self.docs,
            self.compat,
        ]
        .into_iter()
        .any(|check| check == Check::Missing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Check {
    Covered,
    Missing,
    NotApplicable,
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Check::Covered => f.write_str("yes"),
            Check::Missing => f.write_str("no"),
            Check::NotApplicable => f.write_str("-"),
        }
    }
}

fn check_core(item: &Item, core: &str) -> Check {
    let mut requirements = Vec::new();

    if let Some(descriptor) = &item.core_descriptor {
        requirements.push(format!("pub struct {descriptor}"));
    }
    if let Some(kind) = &item.core_kind {
        requirements.push(format!("BindingWidgetKind::{kind}"));
    }
    if let Some(constructor) = &item.core_constructor {
        requirements.push(format!("pub fn {constructor}("));
    }

    check_requirements(&requirements, core)
}

fn check_python(item: &Item, python: &str) -> Check {
    let Some(kind) = item.python_kind.as_deref() else {
        return Check::NotApplicable;
    };

    let mut requirements = Vec::new();
    match kind {
        "function" => {
            requirements.push(format!("#[pyfunction(name = \"{}\")]", item.name));
            if let Some(function) = &item.python_rust {
                requirements.push(format!("wrap_pyfunction!({function}, m)"));
            }
        }
        "class" => {
            requirements.push(format!("#[pyclass(name = \"{}\"", item.name));
            requirements.push(format!("m.add_class::<Py{}>()", item.name));
        }
        _ => return Check::Missing,
    }

    check_requirements(&requirements, python)
}

fn check_js(item: &Item, js: &str) -> Check {
    let Some(kind) = item.js_kind.as_deref() else {
        return Check::NotApplicable;
    };

    let mut requirements = vec![format!("#[napi(js_name = \"{}\")]", item.name)];
    match kind {
        "function" => {
            if let Some(function) = &item.js_rust {
                requirements.push(format!("pub fn {function}"));
            }
        }
        "class" => {
            requirements.push(format!("struct Js{}", item.name));
        }
        _ => return Check::Missing,
    }

    check_requirements(&requirements, js)
}

fn check_ts(item: &Item, ts: &str) -> Check {
    match item.ts_kind.as_deref() {
        Some("function") => check_requirements(&[format!("export function {}(", item.name)], ts),
        Some("class") => check_requirements(&[format!("export class {}", item.name)], ts),
        Some(_) => Check::Missing,
        None => Check::NotApplicable,
    }
}

fn check_docs(item: &Item, docs: &str) -> Check {
    check_requirements(&item.docs, docs)
}

fn check_compat(item: &Item, sources: &Sources) -> Check {
    if !item.compat {
        return Check::NotApplicable;
    }

    let js_compat = section_after(
        &sources.js,
        "fn high_level_app_renders_cross_language_compatibility_signature()",
    )
    .unwrap_or(&sources.js);
    let python_compat = section_after(
        &sources.python,
        "fn python_renders_cross_language_compatibility_signature()",
    )
    .unwrap_or(&sources.python);

    let mut requirements = Vec::new();
    if let Some(constructor) = &item.core_constructor {
        requirements.push((format!("BindingWidget::{constructor}("), js_compat));
    }
    if item.python_kind.as_deref() == Some("function") {
        requirements.push((format!("sui.{}(", item.name), python_compat));
    }

    if requirements.is_empty() {
        return Check::NotApplicable;
    }

    if requirements
        .iter()
        .all(|(needle, source)| source.contains(needle))
    {
        Check::Covered
    } else {
        Check::Missing
    }
}

fn section_after<'a>(source: &'a str, marker: &str) -> Option<&'a str> {
    source.find(marker).map(|index| &source[index..])
}

fn check_requirements<S: AsRef<str>>(requirements: &[S], source: &str) -> Check {
    if requirements.is_empty() {
        return Check::NotApplicable;
    }

    if requirements
        .iter()
        .all(|requirement| source.contains(requirement.as_ref()))
    {
        Check::Covered
    } else {
        Check::Missing
    }
}

fn print_report(rows: &[CoverageRow<'_>]) {
    println!("Binding coverage");
    println!(
        "{:<24} {:<15} {:<5} {:<7} {:<3} {:<3} {:<4} {:<6}",
        "Name", "Kind", "Core", "Python", "JS", "TS", "Docs", "Compat"
    );
    println!("{}", "-".repeat(75));

    for row in rows {
        println!(
            "{:<24} {:<15} {:<5} {:<7} {:<3} {:<3} {:<4} {:<6}",
            row.item.name,
            row.item.kind,
            row.core,
            row.python,
            row.js,
            row.ts,
            row.docs,
            row.compat
        );
    }

    let total = rows.len();
    let complete = rows.iter().filter(|row| !row.has_required_gap()).count();

    println!();
    println!("Summary: {complete}/{total} complete");

    let missing = rows
        .iter()
        .filter(|row| row.has_required_gap())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        println!();
        println!("Coverage gaps:");
        for row in missing {
            let mut columns = Vec::new();
            if row.core == Check::Missing {
                columns.push("core");
            }
            if row.python == Check::Missing {
                columns.push("python");
            }
            if row.js == Check::Missing {
                columns.push("js");
            }
            if row.ts == Check::Missing {
                columns.push("ts");
            }
            if row.docs == Check::Missing {
                columns.push("docs");
            }
            if row.compat == Check::Missing {
                columns.push("compat");
            }
            println!("- {}: {}", row.item.name, columns.join(", "));
        }
    }
}
