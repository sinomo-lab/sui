# Text Rendering Benchmarks

This benchmark plan verifies the current text antialiasing implementation from two angles:

- performance: cache churn, atlas uploads, text submission cost, and interactive frame cost
- quality: perceptual weight, edge coverage, DPR stability, LCD fallback, and inspectable captures

The benchmark suite should use the real SUI text path. Do not validate text rendering with offscreen preview images that bypass `DrawText`, `DrawShapedText`, `PushTextRenderPolicy`, or the WGPU text atlas path.

## Performance Benchmarks

### 1. Renderer Policy Cache Microbenchmark

Purpose: prove that coverage policy changes and text color animation do not duplicate glyph atlas entries, while render-mode/subpixel-order changes still create the distinct entries they need.

Run:

```bash
cargo test -p sui-render-wgpu text_render_policy_cache_benchmark -- --ignored --nocapture
```

Scenarios to inspect:

- grayscale perceptual warm path
- linear/perceptual/boost/gamma coverage churn over the same glyphs
- dark/light text color churn over the same glyphs
- LCD RGB text on an axis-aligned transform
- grayscale rotated text as the fallback cache prime
- LCD RGB requested through the same non-LCD-safe transform, which should fall back to the primed grayscale cache entries

Primary metrics:

- wall time per prepared frame
- glyph cache entries, hits, and misses
- text atlas miss count and upload bytes
- generated text instance count

Expected signals:

- coverage churn should keep glyph cache entries stable after the first policy frame
- color churn should keep glyph cache entries stable after the first color frame
- LCD RGB should have separate atlas entries from grayscale
- unsafe LCD fallback should reuse the matching grayscale rotated cache entries instead of creating LCD entries

### 2. Retained Text Scroll Benchmark

Purpose: measure text-heavy retained scrolling through the normal desktop harness, including packet rebuilds, vertex uploads, glyph instances, atlas misses, and frame time.

Run on a machine with a desktop display:

```bash
cargo test -p sui-demo --test desktop_e2e desktop_retained_text_scroll_upload_benchmark -- --ignored --exact --nocapture
```

Primary metrics:

- average, max, and p95 frame time
- average text glyph instances
- average text vertex bytes
- average atlas misses and atlas upload bytes
- retained packet rebuild counts
- retained packet build time

Expected signals:

- average frame time should remain within the printed 60 fps budget
- atlas misses should converge after warmup
- text bytes per glyph should remain stable across policy changes

### 3. Text Editing Interaction Benchmark

Purpose: verify editor-style workloads where text is typed, selected, scrolled, and rendered with style overlays.

Run:

```bash
cargo test -p sui-demo --test desktop_e2e desktop_text_editing_benchmark_reports_frame_samples -- --exact --nocapture
```

Primary metrics:

- frame time distribution during type/select/scroll stages
- text glyph instances
- text payload bytes
- uploaded geometry bytes

Expected signals:

- text payloads and geometry uploads should stay nonzero while interacting
- no stage should stall without publishing new frames

### 4. Web Benchmark Presets

Purpose: check wasm/browser presentation with the same real text surfaces.

Launch:

```bash
trunk serve --config crates/sui-demo/web/Trunk.toml
```

Open:

```text
http://127.0.0.1:8080/?benchmark=retained-text&warmup=60&frames=180
http://127.0.0.1:8080/?benchmark=text-editing&warmup=60&frames=180
http://127.0.0.1:8080/?benchmark=text-comparison&warmup=30&frames=120
```

Primary metrics:

- browser-reported frame timing
- canvas mode and color-management mode
- whether text-comparison visibly changes between linear, perceptual, LCD, and stem-darkened policy cards

## Quality Benchmarks

### 1. Renderer Quality Matrix

Purpose: verify perceptual text behavior across DPR and light/dark surfaces with metrics that are closer to perceived weight than raw changed-pixel percentages.

Run:

```bash
cargo test -p sui-render-wgpu text_coverage_quality_matrix_capture -- --nocapture
```

To also write PNGs:

```bash
SUI_TEXT_COVERAGE_WRITE_PNGS=1 \
cargo test -p sui-render-wgpu text_coverage_quality_matrix_capture -- --nocapture
```

Outputs:

- console metrics for 1x, 1.5x, and 2x
- light and dark surfaces
- linear, perceptual, and LCD RGB policies
- optional PNGs in `target/text-coverage-matrix/`

Primary metrics:

- core luma
- foreground-weight delta from the background
- edge/core coverage ratio
- inked pixel count

Expected signals:

- perceptual coverage should not be lighter than linear for the same surface
- all DPR variants should produce finite edge/core metrics and nontrivial inked pixels
- optional captures should show distinct policy behavior, especially in small UI labels

### 2. Snapshot Capture Matrix

Purpose: generate human-reviewable native captures for policy combinations, including HiDPI and subpixel order.

Example commands:

```bash
SUI_TEXT_COMPARE_DPI_SCALE=1.0 \
SUI_TEXT_COMPARE_COVERAGE=perceptual \
cargo run -p sui-demo --bin sui-text-render-snapshot

SUI_TEXT_COMPARE_DPI_SCALE=1.5 \
SUI_TEXT_COMPARE_COVERAGE=linear \
cargo run -p sui-demo --bin sui-text-render-snapshot

SUI_TEXT_COMPARE_DPI_SCALE=2.0 \
SUI_TEXT_COMPARE_COVERAGE=perceptual \
SUI_TEXT_COMPARE_SUBPIXEL_ORDER=rgb \
cargo run -p sui-demo --bin sui-text-render-snapshot
```

Use this matrix:

| DPR | Coverage | Subpixel order | Purpose |
| --- | --- | --- | --- |
| 1.0 | linear | none | literal coverage control |
| 1.0 | perceptual | none | default grayscale policy |
| 1.0 | perceptual | rgb | explicit LCD policy |
| 1.5 | linear | none | fractional HiDPI control |
| 1.5 | perceptual | none | fractional HiDPI default |
| 1.5 | perceptual | rgb | fractional HiDPI LCD check |
| 2.0 | linear | none | integer HiDPI control |
| 2.0 | perceptual | none | integer HiDPI default |
| 2.0 | perceptual | rgb | integer HiDPI LCD check |

Expected signals:

- text remains crisp at 1.5x and 2x
- perceptual and linear are visibly distinct in small labels
- LCD is only used when explicitly requested through `SUI_TEXT_COMPARE_SUBPIXEL_ORDER`

### 3. Browser Reference Captures

Purpose: compare SUI captures against browser text rendering without treating raw pixel-diff percentage as the only score.

Recommended procedure:

1. Render a static browser page with the same font, text strings, foreground/background colors, and DPR.
2. Capture Chrome screenshots at DPR 1.0, 1.5, and 2.0.
3. Compare SUI and browser crops using:
   - core stem luma
   - edge profile width
   - foreground-weight delta
   - SSIM or delta-E on cropped glyph regions
4. Review the images manually, because a small numeric pixel diff can still look much worse to human eyes.

Acceptance guidance:

- no obvious blur on 1.5x or 2x captures
- no color fringing unless LCD RGB/BGR was explicitly enabled and display policy allows it
- perceptual policy should better match perceived browser weight than linear in small dark-on-light labels

## Reporting Template

Record each run with:

```text
date:
commit:
machine:
gpu/backend:
os/display:
font:
dpr:
policy:
subpixel order:

performance:
- avg frame:
- p95 frame:
- max frame:
- glyph cache entries/hits/misses:
- atlas misses/upload bytes:
- text bytes/glyph:

quality:
- core luma:
- foreground weight:
- edge/core ratio:
- notes from visual review:
```

For regression tracking, compare against a baseline commit on the same machine. Use relative deltas for timing and upload metrics; avoid hard absolute gates unless the runner is fixed.
