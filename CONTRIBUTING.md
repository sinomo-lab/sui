# Contributing to SUI

Thank you for helping improve SUI. This repository is a Rust workspace, and
most changes are easiest to review when they stay within one subsystem and
include focused tests.

## Before You Start

- Read the [documentation index](docs/README.md) for the user and developer
  guides.
- Check the existing issue tracker before starting a large feature.
- Open an issue or discussion before making a breaking public-API change or a
  change that crosses several crate boundaries.
- Do not treat documents describing future direction as shipped contracts.
  The public Rust facade in `crates/sui`, its rustdoc, and the current guides
  describe the supported application API.

## Development Setup

Install Rust 1.90 or newer, clone the repository, and run:

```bash
cargo check --workspace --all-targets
cargo test --workspace
```

The default `sinomo-ui` features build the desktop and `wgpu` paths. A GPU is
not required for most unit and semantics tests; desktop presentation and a few
ignored benchmark or artifact tests require an appropriate display or graphics
environment.

Launch the development host and widget book with:

```bash
cargo run -p sui-demo
```

For the browser build, install the `wasm32-unknown-unknown` target and Trunk:

```bash
rustup target add wasm32-unknown-unknown
trunk serve --config crates/sui-demo/web/Trunk.toml
```

## Repository Boundaries

The most common ownership boundaries are:

- `crates/sui`: the stable, application-facing Rust facade.
- `crates/sui-widgets`: built-in widgets and themes.
- `crates/sui-runtime`: retained widget graph, event routing, invalidation,
  layout, paint, and semantics scheduling.
- `crates/sui-scene`: renderer-neutral scene data.
- `crates/sui-render-wgpu`: GPU preparation and presentation.
- `crates/sui-platform`: desktop, web, mobile, accessibility, and headless
  platform integration.
- `crates/sui-testing`: deterministic, semantics-first automation.
- `crates/sui-demo`: widget book, integration examples, benchmarks, and visual
  validation.

Keep renderer-specific details out of widgets and runtime contracts. Widgets
should communicate through events, runtime contexts, semantics, invalidation,
and renderer-neutral scene commands.

## Making A Change

1. Add or update a focused regression test.
2. Implement the smallest coherent change in the owning crate.
3. Re-export application-facing API through `crates/sui` when appropriate.
4. Add or update a widget-book story for visible built-in widget behavior.
5. Update user documentation and runnable examples for public API changes.
6. Run the focused crate tests before the full checks below.

For widget tests, prefer roles, accessible names, values, and user actions over
private widget internals. See the [testing guide](docs/testing.md).

## Required Checks

Before submitting a change, run:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc -p sinomo-ui --no-deps
git diff --check
```

Some platform-specific code is conditionally compiled. If your change affects
one of those paths, also check an installed target for that platform. For
example:

```bash
cargo check -p sui-platform --target x86_64-pc-windows-gnu
cargo check -p sui-demo --target wasm32-unknown-unknown --no-default-features --features web
```

Generate the widget-book artifacts only when the change needs visual review:

```bash
cargo run -p sui-demo --bin sui-demo-artifacts
```

Artifacts are written below `target/ui-artifacts/sui-demo/widget-book` and
should not be committed unless a maintainer explicitly requests them.

## Documentation Style

- Write for an application author first; introduce internals only when they
  affect correct API use.
- Mark experimental and platform-specific behavior explicitly.
- Keep snippets complete enough to compile, and prefer checked examples in
  `crates/sui/examples` for longer tutorials.
- Use relative links within the repository and verify them before submitting.
- Remove completed implementation plans once stable behavior is documented in
  the user, API, architecture, or testing guides.

## Commit And Pull Request Style

Use a short imperative summary such as `Add password input semantics` or
`Document headless testing`. Keep unrelated cleanup in separate commits.

A pull request should explain:

- the user-visible outcome;
- the architectural boundary affected;
- the validation performed;
- screenshots or artifacts when visual behavior changed;
- any known platform limitations or follow-up work.

By contributing, you agree that your contribution is licensed under the
project's [MIT License](LICENSE).
