# Active implementation plans

This directory contains only work that is not complete. Plans describe intent
and sequencing; they are not guarantees that an API or platform capability is
available. Current behavior belongs in tutorials, API guides, or architecture
documents and completed plans are removed.

## Current plans

- [Cross-language bindings](./cross-language-bindings-plan.md) tracks package
  publication, desktop smoke coverage, editor/virtual-table parity, browser
  JavaScript/WASM support, custom shader registration, and zero-copy binding
  composition. Native Python and Node/Electron alpha APIs, generated coverage,
  and CPU external-surface composition already exist.
- [HDR and wide-gamut output](./hdr-wide-gamut-display-proposal.md) tracks the
  remaining macOS EDR, Linux native-HDR, browser validation, and cross-platform
  output work. Color conversion, renderer output policies, SDR fallback,
  Windows Advanced Color, and capture diagnostics already exist. Demo browser
  configuration attempts and Windows setup-error propagation are partial
  foundations; monitor migration, failure recovery, and hardware acceptance
  remain open.

For shipped behavior, return to the [documentation index](../README.md).
