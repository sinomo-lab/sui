use std::collections::{BTreeMap, BTreeSet};
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
    let spec = parse_binding_spec(&spec)?;
    let items = &spec.items;

    let manifest_path = root.join("bindings/widgets.toml");
    let manifest = render_manifest(&spec);
    update_generated_file(&manifest_path, &manifest, check)?;

    let python_generated_path = root.join("crates/sui-python/src/generated_widgets.rs");
    let python_template_path = root.join("bindings/templates/python_widgets.rs.in");
    let python_template = fs::read_to_string(&python_template_path)
        .map_err(|error| format!("failed to read {}: {error}", python_template_path.display()))?;
    validate_rust_template("python", items, &python_template)?;
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
    validate_rust_template("js", items, &js_template)?;
    let js_generated = render_generated_rust_template(
        "JavaScript widget bindings",
        "bindings/templates/js_widgets.rs.in",
        &js_template,
    );
    update_generated_file(&js_generated_path, &js_generated, check)?;

    let ts_path = root.join("crates/sui-js/index.d.ts");
    let current_ts = fs::read_to_string(&ts_path)
        .map_err(|error| format!("failed to read {}: {error}", ts_path.display()))?;
    let generated_ts = render_generated_ts(items);
    let next_ts = update_ts_index(&current_ts, &generated_ts, items)?;
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
    let manifest = parse_manifest(&manifest)?;

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

    let rows = manifest
        .items
        .iter()
        .map(|item| CoverageRow::for_item(item, &sources))
        .collect::<Vec<_>>();

    print_report(&rows);

    let public_widgets = inventory_public_widgets(&root)?;
    let widget_issues =
        validate_widget_classifications(&public_widgets, &manifest.rust_widgets, &manifest.items);
    print_widget_classification_report(&public_widgets, &manifest.rust_widgets, &widget_issues);

    let missing = rows
        .iter()
        .filter(|row| row.has_required_gap())
        .collect::<Vec<_>>();

    if missing.is_empty() && widget_issues.is_empty() {
        Ok(())
    } else {
        let mut failures = Vec::new();
        if !missing.is_empty() {
            failures.push(format!(
                "{} binding item(s) are missing required coverage",
                missing.len()
            ));
        }
        if !widget_issues.is_empty() {
            failures.push(format!(
                "{} Rust widget classification issue(s)",
                widget_issues.len()
            ));
        }
        Err(failures.join("; "))
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

#[derive(Clone, Debug, Default)]
struct BindingSpec {
    items: Vec<Item>,
    rust_widgets: Vec<RustWidgetClassification>,
}

#[derive(Clone, Debug, Default)]
struct BindingManifest {
    items: Vec<Item>,
    rust_widgets: Vec<RustWidgetClassification>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RustWidgetClass {
    Bound,
    ManualWrapper,
    Equivalent,
    RustOnly,
}

impl RustWidgetClass {
    fn parse(value: &str, line_number: usize) -> Result<Self, String> {
        match value {
            "bound" => Ok(Self::Bound),
            "manual-wrapper" => Ok(Self::ManualWrapper),
            "equivalent" => Ok(Self::Equivalent),
            "rust-only" => Ok(Self::RustOnly),
            _ => Err(format!(
                "line {line_number}: unsupported Rust widget classification `{value}`"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::ManualWrapper => "manual-wrapper",
            Self::Equivalent => "equivalent",
            Self::RustOnly => "rust-only",
        }
    }
}

impl fmt::Display for RustWidgetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustWidgetClassification {
    name: String,
    classification: RustWidgetClass,
    bindings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublicWidget {
    name: String,
    source: String,
    line: usize,
}

#[derive(Clone, Debug)]
enum TsDecl {
    Class(Vec<String>),
    Function(String),
}

fn parse_manifest(input: &str) -> Result<BindingManifest, String> {
    let mut manifest = BindingManifest::default();
    let mut current: Option<ManifestTable> = None;

    for (line_index, raw_line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if matches!(line, "[[item]]" | "[[rust_widget]]") {
            if let Some(table) = current.take() {
                finish_manifest_table(&mut manifest, table, line_number)?;
            }
            current = Some(if line == "[[item]]" {
                ManifestTable::Item(BTreeMap::new())
            } else {
                ManifestTable::RustWidget(BTreeMap::new())
            });
            continue;
        }

        let Some(current) = current.as_mut() else {
            return Err(format!(
                "line {line_number}: key/value pair found before a manifest table"
            ));
        };
        let table = current.values_mut();

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected `key = value`"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("line {line_number}: empty key"));
        }
        if table
            .insert(key.to_string(), parse_value(value.trim(), line_number)?)
            .is_some()
        {
            return Err(format!("line {line_number}: duplicate key `{key}`"));
        }
    }

    if let Some(table) = current.take() {
        finish_manifest_table(&mut manifest, table, input.lines().count().max(1))?;
    }

    if manifest.items.is_empty() {
        return Err("manifest does not contain any [[item]] entries".to_string());
    }
    if manifest.rust_widgets.is_empty() {
        return Err("manifest does not contain any [[rust_widget]] entries".to_string());
    }

    validate_classification_metadata(&manifest.rust_widgets, &manifest.items)?;

    Ok(manifest)
}

#[derive(Clone, Debug)]
enum ManifestTable {
    Item(BTreeMap<String, Value>),
    RustWidget(BTreeMap<String, Value>),
}

impl ManifestTable {
    fn values_mut(&mut self) -> &mut BTreeMap<String, Value> {
        match self {
            Self::Item(values) | Self::RustWidget(values) => values,
        }
    }
}

fn finish_manifest_table(
    manifest: &mut BindingManifest,
    table: ManifestTable,
    line_number: usize,
) -> Result<(), String> {
    match table {
        ManifestTable::Item(values) => {
            manifest.items.push(item_from_table(values, line_number)?);
        }
        ManifestTable::RustWidget(values) => {
            manifest
                .rust_widgets
                .push(rust_widget_from_table(values, line_number)?);
        }
    }
    Ok(())
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

fn rust_widget_from_table(
    mut table: BTreeMap<String, Value>,
    line_number: usize,
) -> Result<RustWidgetClassification, String> {
    let name = take_required_string(&mut table, "name", line_number)?;
    let classification = RustWidgetClass::parse(
        &take_required_string(&mut table, "classification", line_number)?,
        line_number,
    )?;
    let bindings = take_optional_strings(&mut table, "bindings", line_number)?;

    if !table.is_empty() {
        return Err(format!(
            "line {line_number}: Rust widget `{name}` has unsupported keys: {}",
            table.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    validate_classification_shape(&name, classification, &bindings, line_number)?;
    Ok(RustWidgetClassification {
        name,
        classification,
        bindings,
    })
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

fn parse_binding_spec(input: &str) -> Result<BindingSpec, String> {
    let mut items = Vec::new();
    let mut rust_widgets = Vec::new();
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
            if line.split_whitespace().next() == Some("rust-widget") {
                rust_widgets.push(parse_rust_widget_directive(line, line_number)?);
                continue;
            }

            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(format!(
                    "line {line_number}: expected `<kind> <name>` or a `rust-widget` directive"
                ));
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
    if rust_widgets.is_empty() {
        return Err("binding spec does not contain any `rust-widget` classifications".to_string());
    }

    validate_classification_metadata(&rust_widgets, &items)?;
    Ok(BindingSpec {
        items,
        rust_widgets,
    })
}

fn parse_rust_widget_directive(
    line: &str,
    line_number: usize,
) -> Result<RustWidgetClassification, String> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    let Some(name) = parts.get(1) else {
        return Err(format!(
            "line {line_number}: `rust-widget` is missing the Rust type name"
        ));
    };
    let Some(classification) = parts.get(2) else {
        return Err(format!(
            "line {line_number}: Rust widget `{name}` is missing its classification"
        ));
    };
    let classification = RustWidgetClass::parse(classification, line_number)?;
    let bindings = parts[3..]
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    validate_classification_shape(name, classification, &bindings, line_number)?;

    Ok(RustWidgetClassification {
        name: (*name).to_string(),
        classification,
        bindings,
    })
}

fn validate_classification_shape(
    name: &str,
    classification: RustWidgetClass,
    bindings: &[String],
    line_number: usize,
) -> Result<(), String> {
    match classification {
        RustWidgetClass::Bound if bindings.len() != 1 => Err(format!(
            "line {line_number}: bound Rust widget `{name}` must name exactly one binding item"
        )),
        RustWidgetClass::ManualWrapper | RustWidgetClass::Equivalent if bindings.is_empty() => {
            Err(format!(
                "line {line_number}: {classification} Rust widget `{name}` must name at least one binding item"
            ))
        }
        RustWidgetClass::RustOnly if !bindings.is_empty() => Err(format!(
            "line {line_number}: rust-only widget `{name}` cannot name binding items"
        )),
        _ => Ok(()),
    }
}

fn validate_classification_metadata(
    rust_widgets: &[RustWidgetClassification],
    items: &[Item],
) -> Result<(), String> {
    let mut item_by_name = BTreeMap::new();
    for item in items {
        if item_by_name.insert(item.name.as_str(), item).is_some() {
            return Err(format!("duplicate binding item `{}`", item.name));
        }
    }

    let mut seen_widgets = BTreeSet::new();
    for widget in rust_widgets {
        if !seen_widgets.insert(widget.name.as_str()) {
            return Err(format!(
                "duplicate Rust widget classification for `{}`",
                widget.name
            ));
        }

        for binding in &widget.bindings {
            let Some(item) = item_by_name.get(binding.as_str()) else {
                return Err(format!(
                    "Rust widget `{}` references unknown binding item `{binding}`",
                    widget.name
                ));
            };
            if item.python_kind.is_none() || item.js_kind.is_none() {
                return Err(format!(
                    "Rust widget `{}` references `{binding}`, which is not exposed to both Python and JavaScript",
                    widget.name
                ));
            }
        }
    }

    Ok(())
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

fn render_manifest(spec: &BindingSpec) -> String {
    let mut output = String::new();
    output.push_str("# Generated by `cargo xtask bindings generate` from bindings/widgets.sui.\n");
    output.push_str("# Do not edit by hand.\n\n");

    for (index, item) in spec.items.iter().enumerate() {
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
        if index + 1 < spec.items.len() {
            output.push('\n');
        }
    }

    let mut rust_widgets = spec.rust_widgets.iter().collect::<Vec<_>>();
    rust_widgets.sort_by(|left, right| left.name.cmp(&right.name));
    output.push_str("\n\n");
    for (index, widget) in rust_widgets.iter().enumerate() {
        output.push_str("[[rust_widget]]\n");
        push_toml_string(&mut output, "name", &widget.name);
        push_toml_string(
            &mut output,
            "classification",
            widget.classification.as_str(),
        );
        if !widget.bindings.is_empty() {
            push_toml_strings(&mut output, "bindings", &widget.bindings);
        }
        if index + 1 < rust_widgets.len() {
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

fn push_toml_strings(output: &mut String, key: &str, values: &[String]) {
    output.push_str(key);
    output.push_str(" = [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push('"');
        output.push_str(value);
        output.push('"');
    }
    output.push_str("]\n");
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
    output.push('\n');
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

fn inventory_public_widgets(root: &Path) -> Result<Vec<PublicWidget>, String> {
    let widgets_root = root.join("crates/sui-widgets/src");
    let mut rust_files = Vec::new();
    collect_rust_files(&widgets_root, &mut rust_files)?;
    rust_files.sort();

    let mut public_structs: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    let mut widget_impls = BTreeSet::new();

    for path in rust_files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let relative = relative_display(root, &path);
        let (structs, impls) = public_widgets_in_source(&source);
        for (name, line) in structs {
            public_structs
                .entry(name)
                .or_default()
                .push((relative.clone(), line));
        }
        widget_impls.extend(impls);
    }

    let mut inventory = Vec::new();
    for name in widget_impls {
        let Some(locations) = public_structs.get(&name) else {
            continue;
        };
        if locations.len() != 1 {
            let rendered = locations
                .iter()
                .map(|(source, line)| format!("{source}:{line}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "public Widget type `{name}` is declared more than once: {rendered}"
            ));
        }
        let (source, line) = &locations[0];
        inventory.push(PublicWidget {
            name,
            source: source.clone(),
            line: *line,
        });
    }

    Ok(inventory)
}

fn collect_rust_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to read entry in {}: {error}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_rust_files(&path, output)?;
        } else if file_type.is_file() && path.extension().is_some_and(|value| value == "rs") {
            output.push(path);
        }
    }
    Ok(())
}

fn public_widgets_in_source(source: &str) -> (BTreeMap<String, usize>, BTreeSet<String>) {
    let mut public_structs = BTreeMap::new();
    let mut widget_impls = BTreeSet::new();
    let mut impl_header: Option<String> = None;

    for (line_index, raw_line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();

        if let Some(rest) = line.strip_prefix("pub struct ")
            && let Some(name) = rust_identifier(rest)
        {
            public_structs.insert(name.to_string(), line_number);
        }

        if let Some(header) = impl_header.as_mut() {
            header.push(' ');
            header.push_str(line);
            if line.contains('{') {
                if let Some(name) = widget_impl_name(header) {
                    widget_impls.insert(name.to_string());
                }
                impl_header = None;
            }
        } else if line.starts_with("impl") {
            if line.contains('{') {
                if let Some(name) = widget_impl_name(line) {
                    widget_impls.insert(name.to_string());
                }
            } else {
                impl_header = Some(line.to_string());
            }
        }
    }

    (public_structs, widget_impls)
}

fn widget_impl_name(header: &str) -> Option<&str> {
    let (_, rest) = header.split_once(" Widget for ")?;
    rust_identifier(rest.trim_start())
}

fn rust_identifier(input: &str) -> Option<&str> {
    let end = input
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_alphanumeric() || *character == '_')
        .last()
        .map(|(index, character)| index + character.len_utf8())?;
    let identifier = &input[..end];
    identifier
        .chars()
        .next()
        .filter(|character| character.is_ascii_alphabetic() || *character == '_')?;
    Some(identifier)
}

fn validate_widget_classifications(
    public_widgets: &[PublicWidget],
    rust_widgets: &[RustWidgetClassification],
    items: &[Item],
) -> Vec<String> {
    let public_names = public_widgets
        .iter()
        .map(|widget| widget.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut classifications = BTreeMap::new();
    let mut issues = Vec::new();

    for classification in rust_widgets {
        if classifications
            .insert(classification.name.as_str(), classification)
            .is_some()
        {
            issues.push(format!(
                "duplicate classification for public Widget `{}`",
                classification.name
            ));
        }
    }

    for widget in public_widgets {
        if !classifications.contains_key(widget.name.as_str()) {
            issues.push(format!(
                "unclassified public Widget `{}` at {}:{}",
                widget.name, widget.source, widget.line
            ));
        }
    }

    for classification in rust_widgets {
        if !public_names.contains(classification.name.as_str()) {
            issues.push(format!(
                "classification for `{}` is stale: no public Widget implementation was found",
                classification.name
            ));
        }
    }

    if let Err(error) = validate_classification_metadata(rust_widgets, items) {
        issues.push(error);
    }

    issues
}

fn print_widget_classification_report(
    public_widgets: &[PublicWidget],
    rust_widgets: &[RustWidgetClassification],
    issues: &[String],
) {
    let classifications = rust_widgets
        .iter()
        .map(|widget| (widget.name.as_str(), widget))
        .collect::<BTreeMap<_, _>>();

    println!();
    println!("Rust public Widget classification");
    println!(
        "{:<24} {:<15} {:<28} Source",
        "Rust type", "Classification", "Binding item(s)"
    );
    println!("{}", "-".repeat(96));
    for widget in public_widgets {
        let classification = classifications.get(widget.name.as_str()).copied();
        let class = classification
            .map(|entry| entry.classification.as_str())
            .unwrap_or("UNCLASSIFIED");
        let bindings = classification
            .map(|entry| entry.bindings.join(", "))
            .unwrap_or_default();
        println!(
            "{:<24} {:<15} {:<28} {}:{}",
            widget.name, class, bindings, widget.source, widget.line
        );
    }

    let classified = public_widgets
        .iter()
        .filter(|widget| classifications.contains_key(widget.name.as_str()))
        .count();
    let mut counts = BTreeMap::new();
    for widget in public_widgets {
        if let Some(classification) = classifications.get(widget.name.as_str()) {
            *counts
                .entry(classification.classification)
                .or_insert(0usize) += 1;
        }
    }

    println!();
    println!(
        "Summary: {classified}/{} public Widget types classified (bound {}, manual-wrapper {}, equivalent {}, rust-only {})",
        public_widgets.len(),
        counts.get(&RustWidgetClass::Bound).copied().unwrap_or(0),
        counts
            .get(&RustWidgetClass::ManualWrapper)
            .copied()
            .unwrap_or(0),
        counts
            .get(&RustWidgetClass::Equivalent)
            .copied()
            .unwrap_or(0),
        counts.get(&RustWidgetClass::RustOnly).copied().unwrap_or(0),
    );

    if !issues.is_empty() {
        println!();
        println!("Widget classification issues:");
        for issue in issues {
            println!("- {issue}");
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const CLASSIFIED_SPEC: &str = r#"
widget Alpha
  python function py_alpha
  js function js_alpha
end

rust-widget Direct bound Alpha
rust-widget Wrapped manual-wrapper Alpha
rust-widget FlexLike equivalent Alpha
rust-widget Native rust-only
"#;

    #[test]
    fn classification_spec_round_trips_through_manifest() {
        let spec = parse_binding_spec(CLASSIFIED_SPEC).expect("spec should parse");
        assert_eq!(spec.items.len(), 1);
        assert_eq!(spec.rust_widgets.len(), 4);

        let rendered = render_manifest(&spec);
        let manifest = parse_manifest(&rendered).expect("rendered manifest should parse");
        assert_eq!(manifest.items.len(), 1);
        let mut expected = spec.rust_widgets;
        expected.sort_by(|left, right| left.name.cmp(&right.name));
        assert_eq!(manifest.rust_widgets, expected);
    }

    #[test]
    fn manual_wrapper_requires_a_binding_target() {
        let error = parse_binding_spec(
            r#"
widget Alpha
  python function py_alpha
  js function js_alpha
end
rust-widget Wrapped manual-wrapper
"#,
        )
        .expect_err("targetless manual wrapper should fail");
        assert!(error.contains("must name at least one binding item"));
    }

    #[test]
    fn classification_target_must_be_cross_language_binding() {
        let error = parse_binding_spec(
            r#"
widget Alpha
  python function py_alpha
end
rust-widget Direct bound Alpha
"#,
        )
        .expect_err("one-language target should fail");
        assert!(error.contains("not exposed to both Python and JavaScript"));
    }

    #[test]
    fn source_inventory_intersects_public_structs_and_widget_impls() {
        let (structs, impls) = public_widgets_in_source(
            r#"
pub struct Plain;
impl Widget for Plain {}

pub struct Generic<T>(T);
impl<T> Widget for Generic<T> {}

struct Private;
impl Widget for Private {}

pub struct NotAWidget;
"#,
        );

        let public_names = structs.keys().cloned().collect::<BTreeSet<_>>();
        let public_widgets = impls
            .intersection(&public_names)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            public_widgets,
            BTreeSet::from(["Generic".to_string(), "Plain".to_string()])
        );
    }

    #[test]
    fn classification_gate_reports_missing_and_stale_types() {
        let public_widgets = vec![
            PublicWidget {
                name: "Alpha".to_string(),
                source: "alpha.rs".to_string(),
                line: 10,
            },
            PublicWidget {
                name: "Beta".to_string(),
                source: "beta.rs".to_string(),
                line: 20,
            },
        ];
        let rust_widgets = vec![
            RustWidgetClassification {
                name: "Alpha".to_string(),
                classification: RustWidgetClass::Bound,
                bindings: vec!["Alpha".to_string()],
            },
            RustWidgetClassification {
                name: "Ghost".to_string(),
                classification: RustWidgetClass::RustOnly,
                bindings: Vec::new(),
            },
        ];
        let items = vec![Item {
            name: "Alpha".to_string(),
            python_kind: Some("function".to_string()),
            js_kind: Some("function".to_string()),
            ..Item::default()
        }];

        let issues = validate_widget_classifications(&public_widgets, &rust_widgets, &items);
        assert!(issues.iter().any(|issue| issue.contains("`Beta`")));
        assert!(issues.iter().any(|issue| issue.contains("`Ghost`")));
    }
}
