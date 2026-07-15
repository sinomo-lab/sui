# Security policy

SUI is pre-release software. Security fixes are developed for the current
`main` branch and the most recent published version, when releases exist. Older
pre-release revisions may not receive backports.

## Report a vulnerability privately

Do not open a public issue for a suspected vulnerability. Use GitHub's
[private vulnerability reporting](https://github.com/sinomo-lab/sui/security/advisories/new)
to send the maintainers:

- the affected commit, version, platform, feature flags, and language binding;
- a minimal reproduction or proof of concept;
- the impact and the trust boundary that is crossed;
- any known mitigations;
- whether the report or exploit details have been shared elsewhere.

If private vulnerability reporting is unavailable, open a public issue that
asks maintainers to establish a private contact channel but does not include
the vulnerability details.

## Relevant surfaces

Reports are especially useful for problems involving:

- unsafe Rust, native windowing, graphics backends, or platform handles;
- parsing or rasterizing untrusted fonts, SVG, PNG, AVIF, text, or animation
  documents;
- shader validation or external texture/render-target interop;
- Python or Node/Electron native package loading;
- cross-thread callbacks, handle lifetimes, or use-after-free behavior;
- denial of service caused by malformed input with realistic resource limits;
- unintended disclosure through password controls, semantics, snapshots,
  diagnostics, logs, or generated artifacts.

Ordinary crashes, rendering errors, and API bugs without a security boundary
can be reported through the public issue tracker.

## Disclosure process

Maintainers will validate the report, identify affected versions and
platforms, coordinate a fix and regression test, and publish an advisory when
users need to take action. Please allow time for a fix and release before
publishing exploit details. No response or remediation deadline is promised
while the project is maintained on a best-effort basis.
