# SUI Documentation

This is the documentation map for SUI users and contributors. Start with a
tutorial if you are building an application, use the API guides while you
work, and read the architecture documents only when you need implementation
detail. Roadmaps are deliberately separated from shipped behavior.

SUI is still pre-release: the Rust desktop and testing surfaces are usable,
but package publication and some platform integrations are unfinished. The
[root README](../README.md#platform-status) records the supported surface and
release boundary.

## Start here

1. [Build your first SUI application](./tutorials/quickstart.md) — install SUI,
   create a window, compose widgets, apply a theme, and handle an action.
2. [Build a stateful form](./tutorials/stateful-form.md) — edit text, password,
   and date/time fields while preserving widget-owned selection and focus.
3. [Browse the examples](./examples.md) — run the Rust, test, TUI, Python, and
   JavaScript samples from the correct directory.
4. [Open the API guide](./api/README.md) — find the public concept or widget
   you need.

See the [tutorial index](./tutorials/README.md) for the maintained learning
path and its checked example sources.

The fastest way to explore the complete widget set is the desktop widget book:

```bash
cargo run -p sinomo-ui-demo
```

## API guide

The hand-written API guide complements generated Rust documentation:

- [API guide index](./api/README.md)
- [Getting started and application lifecycle](./api/getting-started.md)
- [Widgets and layout](./api/widgets-and-layout.md)
- [Input and editing](./api/input-and-editing.md)
- [State, events, and async work](./api/state-events-and-async.md)
- [Themes and resources](./api/themes-and-resources.md)
- [Custom widgets](./api/custom-widgets.md)
- [Testing and accessibility](./api/testing-and-accessibility.md)
- [Platforms and feature flags](./api/platforms-and-features.md)

The [user API overview](./user-api.md) is a compact landing page for existing
links and points into these focused guides.

Generate the exact Rust API for the checked-out revision with:

```bash
cargo doc -p sinomo-ui --no-deps --open
```

## Practical guides

- [Testing](./testing.md) — headless automation, semantic locators,
  screenshots, and visual artifact generation.
- [Accessibility-generated TUI](./tui.md) — dump, navigate, validate, and
  embed the semantic terminal interface.
- [Layout and overflow](./layout-and-overflow.md) — constraints, overflow
  policies, and scroll sizing.
- [Stack hosts](./stack-hosts.md) — current stacking, popup resolution, hit
  testing, and ordering contract.
- [Text system](./text-system.md) — current text rendering model, editor
  direction, and benchmark workflow.
- [Text rendering benchmarks](./text-rendering-benchmarks.md) — performance
  and visual-quality capture procedures.
- [HDR debugging](./hdr-debugging.md) — linear EXR capture, SDR previews,
  headroom and clip maps, and output diagnostics.
- [HDR theme tokens](./hdr-theme-token-schema-proposal.md) — implemented HDR
  token API, usage, and explicitly marked future direction.

Language-specific setup and examples:

- [Python binding guide](../crates/sui-python/README.md)
- [Node/Electron binding guide](../crates/sui-js/README.md)

Both binding packages are alpha and must currently be built from source.
Browser JavaScript/WASM bindings are not implemented.

## Architecture and contributors

- [Architecture overview](./architecture.md) — runtime and frame pipeline.
- [Crate architecture](./crate-architecture.md) — ownership, dependencies, and
  where a change belongs.
- [Renderer architecture](./renderer-architecture.md) — scene input, retained
  compositor, output pipeline, diagnostics, and performance constraints.
- [Design](./design.md) — long-term product principles and non-goals.
- [Contributing](../CONTRIBUTING.md) — environment setup, checks, documentation
  style, and change submission.
- [Security policy](../SECURITY.md) — supported revisions and private
  vulnerability reporting.

The implementation follows one main path: the platform normalizes input, the
runtime routes events through the retained tree, widgets request invalidation,
the runtime produces a renderer-neutral `SceneFrame`, and the renderer updates
retained compositor state before presentation. Widgets do not call `wgpu` or
host window APIs directly.

## Active roadmap

Only unfinished plans belong in [`plans/`](./plans/):

- [Cross-language bindings](./plans/cross-language-bindings-plan.md) — package
  publication, browser bindings, richer shader support, and zero-copy graphics
  interop.
- [HDR and wide-gamut output](./plans/hdr-wide-gamut-display-proposal.md) —
  remaining macOS EDR, Linux native-HDR, and platform validation work.

Vision documents are not API guarantees:

- [HDR-native interface manifesto](./hdr-native-interface-manifesto.md)

Completed implementation plans are removed after their stable behavior has
been documented in tutorials, API guides, or architecture pages.

## Common repository commands

```bash
# Compile every target in the workspace
cargo check --workspace --all-targets

# Run tests and lint the complete surface
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Check the public Rust examples
cargo check -p sinomo-ui --examples

# Generate the widget-book artifact bundle
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

The artifact generator writes to
`target/ui-artifacts/sui-demo/widget-book`. Ordinary tests intentionally do
not run that slower generator.
