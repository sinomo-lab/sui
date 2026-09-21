# Prepared text layout performance

## Benchmark design

The benchmark measures the public native CPU text pipeline, including request
construction, cache access, shaping,
line breaking and SUI geometry construction where the requested result requires
it. It excludes initial system-font discovery, GPU work and widget-tree traversal.

Run from the workspace root, without another build or benchmark running:

```sh
cargo bench -p sinomo-ui-text --bench text_layout --locked
```

The benchmark uses bundled Noto Sans, font size 16, line height 20, and release
optimization level 2. Keep the dependency lockfile, corpus, and width sequence
fixed across comparison runs. Characters missing from Noto Sans use installed
fallback fonts; compare multilingual timings on the same machine/font environment.
Seven samples retain their individual timings and report their median and maximum.
Each resize sample visits eight new widths; no width is repeated between samples.
An unchanged-width control verifies that existing full-layout reuse stays cheap.

| Workload | What it isolates |
| --- | --- |
| same_width | 200 texts, already cached final layouts |
| fresh_text | New text on every request, with an initialized font context |
| resize | Complete layouts of 200 stable messages at new widths |
| size_only_resize | Only natural width/height requested at new widths |
| wrapped_label_passes | The natural-size, constrained-size and final-layout requests made by a wrapped label |
| multilingual_resize | Longer Latin, Arabic, Hebrew/bidi, CJK, combining-mark and emoji paragraphs |

The size-only workload calls the public measurement API without materializing
glyph geometry. The label workload follows the widget's measurement and final
layout request sequence; its registry is pruned between requests to avoid
measuring unlimited handle retention.

## Correctness and memory gates

- Size-only width/height must match materialized layout for mixed styles, fonts,
  directions, wrapping modes, explicit breaks and empty text.
- Reflowed glyphs, clusters, runs, caret/selection geometry and layout versions
  must match fresh layouts. Color-only changes must retain geometry reuse.
- Font-registry changes must discard prepared state and intrinsic metrics.
- Both cache entry count and retained-byte limits must hold under churn;
  oversized inputs must not flush useful small entries.
- Existing persistent handles and renderer handoff must remain valid after eviction.
- Run the complete affected text, layout and widget suites and relevant renderer
  contract tests. Use timing results as observations, not machine-dependent CI gates.

## Validation

```sh
SUI_PROFILE_TEXT_TIMINGS=1 cargo test -p sinomo-ui-text -p sinomo-ui-layout -p sinomo-ui-widgets --lib --locked
cargo test -p sinomo-ui-render-wgpu --lib tests::text:: --locked
cargo test -p sinomo-ui-runtime --lib --locked
cargo fmt --all -- --check
git diff --check
cargo clippy -p sinomo-ui-text -p sinomo-ui-layout -p sinomo-ui-widgets --lib --tests --locked -- -D warnings
```

## Comparing revisions

Run each revision in a separate checkout with the same compiler, primary and
fallback fonts, workloads, iteration counts, and width sequence. If public APIs
differ, adapt the harness to request equivalent results and document that
adaptation with the local run record.

Keep timings, revision identifiers, machine details, and investigation notes in
an ignored local directory under `target/`. The documentation describes the
benchmark procedure; run-specific results and development history stay local.
