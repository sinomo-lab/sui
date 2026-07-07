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
        _ => {
            eprintln!("usage: cargo xtask bindings coverage");
            eprintln!("       cargo xtask bindings check");
            Err("unknown xtask command".to_string())
        }
    }
}

fn bindings_coverage() -> Result<(), String> {
    let root = workspace_root()?;
    let manifest_path = root.join("bindings/widgets.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let items = parse_manifest(&manifest)?;

    let sources = Sources {
        core: read_source(&root, "crates/sui-bindings-core/src/lib.rs")?,
        python: read_source(&root, "crates/sui-python/src/lib.rs")?,
        js: read_source(&root, "crates/sui-js/src/lib.rs")?,
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
        .filter(|row| row.item.status == "stable" && row.has_required_gap())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} stable binding item(s) are missing required coverage",
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
    status: String,
    core_kind: Option<String>,
    core_constructor: Option<String>,
    core_descriptor: Option<String>,
    python_kind: Option<String>,
    python_rust: Option<String>,
    js_kind: Option<String>,
    js_rust: Option<String>,
    ts_kind: Option<String>,
    docs: Vec<String>,
    compat: bool,
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
    let status = take_required_string(&mut table, "status", line_number)?;

    let item = Item {
        name,
        kind,
        status,
        core_kind: take_optional_string(&mut table, "core_kind", line_number)?,
        core_constructor: take_optional_string(&mut table, "core_constructor", line_number)?,
        core_descriptor: take_optional_string(&mut table, "core_descriptor", line_number)?,
        python_kind: take_optional_string(&mut table, "python_kind", line_number)?,
        python_rust: take_optional_string(&mut table, "python_rust", line_number)?,
        js_kind: take_optional_string(&mut table, "js_kind", line_number)?,
        js_rust: take_optional_string(&mut table, "js_rust", line_number)?,
        ts_kind: take_optional_string(&mut table, "ts_kind", line_number)?,
        docs: take_optional_strings(&mut table, "docs", line_number)?,
        compat: take_optional_bool(&mut table, "compat", line_number)?.unwrap_or(false),
    };

    if !matches!(item.status.as_str(), "stable" | "planned") {
        return Err(format!(
            "line {line_number}: item `{}` has unsupported status `{}`",
            item.name, item.status
        ));
    }

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
    println!("Binding coverage (stable missing coverage fails)");
    println!(
        "{:<24} {:<8} {:<15} {:<5} {:<7} {:<3} {:<3} {:<4} {:<6}",
        "Name", "Status", "Kind", "Core", "Python", "JS", "TS", "Docs", "Compat"
    );
    println!("{}", "-".repeat(84));

    for row in rows {
        println!(
            "{:<24} {:<8} {:<15} {:<5} {:<7} {:<3} {:<3} {:<4} {:<6}",
            row.item.name,
            row.item.status,
            row.item.kind,
            row.core,
            row.python,
            row.js,
            row.ts,
            row.docs,
            row.compat
        );
    }

    let stable_total = rows
        .iter()
        .filter(|row| row.item.status == "stable")
        .count();
    let stable_complete = rows
        .iter()
        .filter(|row| row.item.status == "stable" && !row.has_required_gap())
        .count();
    let planned_gaps = rows
        .iter()
        .filter(|row| row.item.status == "planned" && row.has_required_gap())
        .count();

    println!();
    println!(
        "Summary: stable {stable_complete}/{stable_total} complete; planned gaps: {planned_gaps}"
    );

    let missing = rows
        .iter()
        .filter(|row| row.item.status == "stable" && row.has_required_gap())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        println!();
        println!("Stable coverage gaps:");
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
