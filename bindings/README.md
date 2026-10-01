# Binding specification and verification

`widgets.sui` is the canonical binding catalog. `cargo xtask bindings generate`
updates the manifest, Python/JavaScript registration glue, TypeScript
declarations, Python stubs, and idiomatic factories. Use
`cargo xtask bindings generate --check` to verify checked-in generated files.

Signatures use a language-neutral API model:

```text
widget Example
  core_kind Example
  core_constructor example
  python function py_example
  js function js_example
  api function Example(row: int, min? as min_value: number, onPick?: (record: id) => void): Widget;
end
```

- `number` maps to TypeScript `number` and Python `float`.
- `int` maps to TypeScript `number` and Python `int`.
- `id` represents an integer identifier serialized as a TypeScript `string`
  and exposed as a Python `int`.
- `string`, `boolean`, `void`, `null`, named types, arrays, unions, literal
  strings, generics, and callbacks are represented structurally.
- `?` marks an optional parameter. `as` supplies an explicit Python parameter
  alias when an existing Python name differs from the normal snake-case form.
  In the example, JavaScript retains `min` and Python exposes `min_value`.
- `api class` begins a class block containing constructors, methods, static
  methods, and properties; `endapi` closes it.
- `python_stub manual` explicitly selects the hand-maintained Python class
  declaration in `templates/python_api.pyi.in`. Custom runtime adapters remain
  in the Rust templates where their behavior needs dedicated implementation.

Choose types from the API contract. Renaming a parameter never changes its
numeric type. Both TypeScript and generated Python signatures are rendered
from the parsed model rather than deriving Python types from TypeScript names.

`cargo xtask bindings check` follows Rust `mod` declarations, `#[path]`, and
included generated files. It inspects actual public declarations, export
attributes, and Python registrations; comments and string contents do not
satisfy those checks. TypeScript export indexing excludes comments and literals,
and generation checks verify complete generated signatures. Widget inventory
uses parsed public type declarations and `Widget` implementations.

Compatibility evidence comes from the named rendering tests in the Python
and JavaScript bindings, including the JS extended-constructor test. Run the
binding crate tests when changing adapters; static coverage alone is not a
runtime compatibility test.
