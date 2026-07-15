# SUI tutorials

These tutorials teach the public Rust facade through complete examples that
are compiled from `crates/sui/examples`.

1. [Build your first SUI application](./quickstart.md) covers dependency
   aliasing, windows, retained widgets, `Stack` and `Flex`, Mesh themes,
   callbacks, and headless construction.
2. [Build a stateful form](./stateful-form.md) covers UI-thread application
   state, editable text, password and local date/time fields, dynamic readers,
   explicit invalidation, and production validation concerns.

Check every tutorial example from the workspace root:

```bash
cargo check -p sinomo-ui --examples
```

For lookup-oriented material, continue to the [API guide](../api/README.md).
For more runnable surfaces and language bindings, see the
[examples catalog](../examples.md).
