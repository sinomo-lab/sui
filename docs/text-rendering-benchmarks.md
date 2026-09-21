# Text Rendering Benchmarks

Use these benchmarks to measure text rendering from two angles:

- performance: cache churn, atlas uploads, text submission cost, and interactive frame cost
- quality: perceptual weight, edge coverage, DPR stability, LCD fallback, and inspectable captures

The benchmark suite should use the real SUI text path. Do not validate text rendering with offscreen preview images that bypass `DrawText`, `DrawShapedText`, `PushTextRenderPolicy`, or the WGPU text atlas path.

## Performance Benchmarks

### 1. Renderer Policy Cache Microbenchmark

Purpose: prove that coverage policy changes and text color animation do not duplicate glyph atlas entries, while render-mode/subpixel-order changes still create the distinct entries they need.

Run:

```bash
cargo test -p sinomo-ui-render-wgpu text_render_policy_cache_benchmark -- --ignored --nocapture
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
cargo test -p sinomo-ui-demo --test desktop_e2e desktop_retained_text_scroll_upload_benchmark -- --ignored --exact --nocapture
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
cargo test -p sinomo-ui-demo --test desktop_e2e desktop_text_editing_benchmark_reports_frame_samples -- --exact --nocapture
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

### Chrome reference comparison

`npm run text:compare` captures the native headless SUI path and installed Google
Chrome using the same embedded font bytes, weight, sizes, positions, line heights,
and unrounded colors. The snapshot binary writes `samples.json`; the browser
reads that manifest instead of maintaining a second copy of the corpus. The
corpus includes muted/body text and blue, green, red, purple, and orange accents.

Install the Node dependencies with `npm ci` and install the selected browser.
Run the comparison from the repository root:

```powershell
$env:SUI_TEXT_COMPARE_SURFACE = 'dark' # light or dark
$env:SUI_TEXT_COMPARE_DPI_SCALE = '1.5' # also compare 1, 1.25, and 2
$env:SUI_TEXT_COMPARE_COVERAGE = 'perceptual'
$env:SUI_TEXT_COMPARE_MODE = 'grayscale' # or lcd; LCD defaults to RGB order
$env:SUI_TEXT_COMPARE_OUTPUT = 'target/text-rendering-compare/dark-1.5x'
npm run text:compare
```

`SUI_TEXT_COMPARE_FONT` can select another local TTF for
both renderers (for example `C:\Windows\Fonts\segoeui.ttf`).
`SUI_TEXT_COMPARE_BROWSER` selects a Playwright channel; the default is `chrome`.
Use `SUI_TEXT_COMPARE_MODE=lcd` with an explicit RGB/BGR subpixel order to compare
LCD rendering; setting the order alone does not select LCD mode.
`SUI_TEXT_COMPARE_COVERAGE=legacy-perceptual` selects a foreground-only coverage
boost as a control, without adapting to the actual backdrop.

The tool writes original captures to `sui.png`, `browser.png`, and `diff.png`.
`summary.json` records Chrome's version, the font SHA-256, requested
mode/order/hinting, and observed chromatic edges in a neutral RGB probe. Check
`suiLcdChromaticEdges` when requesting LCD: unavailable or ineligible LCD
correctly falls back to gray.

Judge glyph shape and coverage with `alignedRowInkStats` or
`textQuality.aligned.meanInkChannelError`. Each isolated sample row is registered
with Chrome using one integer translation within ±2 **physical** pixels in X/Y.
The search minimizes summed absolute RGB error, with ties preferring the
smallest shift. It never resamples pixels, scales glyphs, adjusts colors, or
moves individual glyphs independently. Alignment is applied to comparison
images only; SUI uses fractional baseline layout and nearest-pixel raster
placement when rendering.

Keep placement visible alongside the quality score:

- `rowInkStats` contains unaligned measurements within each sample's padded
  text region. `textQuality.raw` aggregates those ink errors. The top-level
  image-diff fields describe the full original captures, including the backdrop.
- `sourceRect` records each crop in physical pixels. Its bounds come from the
  sample's X/Y position, width, and line height, expanded by six CSS pixels on
  each side and clipped to the capture. Unrelated canvas borders are excluded.
- Each aligned row reports `alignment.suiShiftX` / `suiShiftY`: the translation
  applied to SUI toward Chrome, in physical pixels; negative Y moves SUI up.
- `atSearchBoundary` flags a best shift at the search limit. Inspect that row's
  placement before assuming alignment is complete.
- `absoluteErrorReduction` uses summed errors before/after alignment. It does
  not divide MAEs, whose ink-union denominators can differ after translation.

`aligned-sui.png`, `aligned-browser.png`, and `aligned-diff.png` contain the
sample crops stacked in manifest order at their original physical resolution.
`alignedImageRect` locates each row in those sheets. Crops include the
six-CSS-pixel sample margin and two additional physical pixels of padding on
each side, retaining ink that a translation moves past a crop edge. Rows with
different widths are padded to the widest crop when assembling the sheets;
that sheet padding does not contribute to their scores.
`alignedImageStats` describes these sheets; their dimensions differ from the
original captures, so their full-image diff percentages are not interchangeable.

Per-row `inkMassRatio` compares the total encoded-luminance difference from the
background (ideal ratio 1). `meanInkChannelError` measures absolute RGB error over
the union of ink pixels, in **0–255 channel units**, not percent mismatch.
`textQuality` aggregates errors and ink-pixel counts across all rows; it is
weighted by each row's ink union, rather than an equal average of row scores.
Matching weight alone does not establish matching sharpness.

Run the comparison-metric regressions without a browser or GPU:

```bash
npm run text:compare:test
```

Browser antialiasing and font metrics depend on the platform and output path.
Check the observed render modes in `summary.json` when comparing with SUI's
grayscale default. Aligned scores separate line placement from glyph shape and
coverage; they do not imply identical hinting or rasterization. For coverage and
hinting behavior, see [WGPU rendering policies](text-system.md#wgpu-rendering-policies).

The renderer regression `transformed_text_rasterizes_at_display_resolution`
separately compares scene-scaled text to directly sized text, including retained
layers, zoom-out, and fractional DPI. This avoids confusing scene zoom with DPI.

LCD regressions cover physical RGB/BGR sample direction, grayscale fallback on
devices without dual-source blending, opacity/effect/transform/output eligibility,
retained capability and opacity transitions, font hint-range cache boundaries,
and GPU channel blending against a separately rendered linear-mask reference.

### 1. Renderer Quality Matrix

Purpose: verify perceptual text behavior across DPR and light/dark surfaces with metrics that are closer to perceived weight than raw changed-pixel percentages.

Run:

```bash
cargo test -p sinomo-ui-render-wgpu text_coverage_quality_matrix_capture -- --nocapture
```

To also write PNGs:

```bash
SUI_TEXT_COVERAGE_WRITE_PNGS=1 \
cargo test -p sinomo-ui-render-wgpu text_coverage_quality_matrix_capture -- --nocapture
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

- perceptual coverage should preserve opaque cores and transparent padding,
  while adapting edge weight to the foreground and backdrop
- all DPR variants should produce finite edge/core metrics and nontrivial inked pixels
- optional captures should show distinct policy behavior, especially in small UI labels

### 2. Snapshot Capture Matrix

Purpose: generate human-reviewable native captures for policy combinations, including HiDPI and subpixel order.

Example commands:

```bash
SUI_TEXT_COMPARE_DPI_SCALE=1.0 \
SUI_TEXT_COMPARE_COVERAGE=perceptual \
cargo run -p sinomo-ui-demo --bin sui-text-render-snapshot

SUI_TEXT_COMPARE_DPI_SCALE=1.5 \
SUI_TEXT_COMPARE_COVERAGE=linear \
cargo run -p sinomo-ui-demo --bin sui-text-render-snapshot

SUI_TEXT_COMPARE_DPI_SCALE=2.0 \
SUI_TEXT_COMPARE_COVERAGE=perceptual \
SUI_TEXT_COMPARE_MODE=lcd \
SUI_TEXT_COMPARE_SUBPIXEL_ORDER=rgb \
cargo run -p sinomo-ui-demo --bin sui-text-render-snapshot
```

Use this matrix:

Set `SUI_TEXT_COMPARE_MODE=lcd` for the RGB rows and `grayscale` for the others.

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
- LCD requires `SUI_TEXT_COMPARE_MODE=lcd`, a subpixel order, and eligible output;
  RGB/BGR order alone does not enable it

### Reviewing Captures

Inspect captures at their original resolution as well as magnified. Use aligned
crops to compare stem weight, edge sharpness, and color fringing; use the original
captures and reported offsets to assess placement. Compare light and dark
surfaces at multiple device scales, and repeat on the target platform.

A low numeric difference alone does not establish text quality. Grayscale text
should have no LCD color fringes, while eligible RGB/BGR rendering should retain
the requested channel order. Check that transformed and HiDPI text stays sharp.

## Reporting Template

Keep run-specific results, captures, and investigation notes in an ignored local
directory such as `target/text-rendering-compare/`; do not add development logs
or result histories to the reference documentation. Record each run with:

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
