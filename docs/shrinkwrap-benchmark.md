# Shrinkwrap conversation

Open **Shrinkwrap** in the desktop demo gallery:

```powershell
cargo run -p sinomo-ui-demo
```

The web demo also accepts `?benchmark=dev&demo=shrinkwrap`.

This single conversation is inspired by
[Pretext's Shrinkwrap Showdown](https://somnai-dreams.github.io/pretext-demos/shrinkwrap-showdown.html).
It uses SUI widgets and text layout, with original message copy, rather than
embedding the web page or importing the Pretext implementation.

The container smoothly cycles between 200 and 450 logical pixels over 12 seconds.
The slider pauses automatic motion; Play resumes at the selected width, Pause
holds it, and Reset restores the initial state. The page scrolls vertically when
the conversation is taller than the viewport. A smaller window clamps the phone
to available width.

Each message first wraps within 80% of the conversation's inner width. A binary
search finds the smallest integer text width preserving its measured height.
With the demo's uniform line height, this preserves the line count. Size-only
probes reuse SUI's preparation caches; only the final result becomes a persistent
text layout. Tests compare the result's line count against full layouts, check
the next narrower width, and check visible ink against bubble padding. The
corpus includes Latin, CJK, Arabic/Latin bidi, combining marks, emoji, and an
unbroken word.

## Correctness and captures

```powershell
cargo test -p sinomo-ui-demo --lib shrinkwrap -- --test-threads=1
cargo test -p sinomo-ui-demo --lib shrinkwrap_visual_capture -- --ignored --nocapture --test-threads=1
```

The capture test writes `target/shrinkwrap-demo/width-200.png`, `width-325.png`,
and `width-450.png`. Tests also exercise actual scheduled animation wakes and
the accessible pause, play, reset, and width controls, and verify gallery routing.

## Frame benchmark

Run serially on an otherwise idle machine:

```powershell
$env:SUI_PROFILE_WIDGET_TIMINGS = '1'
$env:SUI_PROFILE_TEXT_TIMINGS = '1'
cargo test -p sinomo-ui-demo --features sui-runtime/layout-diagnostics --lib shrinkwrap_frame_profile -- --ignored --nocapture --test-threads=1
```

The benchmark mounts the same demo content without the gallery shell, at
960 x 1100 logical pixels. It compares paused and animated phases, each with 30
warmup frames and 720 measured frames. Animation advances at a fixed 1/60 second
step, covering a complete cycle. A 17 ms sleep outside the measured work paces
GPU submissions. Assertions verify scheduled wakes, actual layout work, and
coverage of both width extremes.

Console output reports median/p95/max CPU frame time, runtime and renderer time,
layout/paint/semantics phases, text requests and size-only timing, preparation
cache misses, render-packet rebuilding, and uploaded vertex bytes. Text timing
is part of runtime/layout work; do not add those overlapping totals. The width
readout changes its text and can introduce new paragraph-cache keys even though
the messages are constant. Initial fonts/layouts are warmed at the starting
width, not by prerunning every width in the cycle.

`target/shrinkwrap-demo/profile.csv` contains one row per measured frame with
width, conversation height, phase timings, text counters, and renderer counters.
CSV formatting and disk output are outside the timed frame work. Correlate peaks
with width and height to distinguish wrapping thresholds from steady redraws.

These are headless CPU and submission measurements, not GPU execution time or
native presentation FPS. Keep fonts, theme, viewport, backend, and validation
settings fixed when comparing changes. For a separate development-overhead
comparison, run with `WGPU_DEBUG=0` and `WGPU_VALIDATION=0`; this does not turn the
test binary into a release build. The output records those settings and adapter.

An initial run on Windows/RTX 3090/DirectX 12, with those two flags disabled,
measured 0.38 ms median paused and 1.38 ms median / 1.67 ms p95 animated. Animated
frames averaged 0.45 ms in layout/arrange, including 0.34 ms of text work, with
about 79 size-only requests per frame and no post-warmup glyph-metric misses.
These figures are a local baseline, not a performance requirement.

## On-screen desktop measurements

Run the same content through the normal desktop host, with a visible window and
VSync:

```powershell
$env:SUI_PROFILE_WIDGET_TIMINGS = '1'
$env:SUI_PROFILE_TEXT_TIMINGS = '1'
cargo run -p sinomo-ui-demo --features sui-runtime/layout-diagnostics --example shrinkwrap_live
```

The window closes automatically after about 18 seconds. A desktop extension
observes every submitted frame after two seconds of warmup, for 13 seconds.
It does not inject ticks or redraw requests. Output includes actual host cadence,
work time excluding surface acquisition/presentation wait, total frame time,
native output configuration, and per-stage timings. The trace is written to
`target/shrinkwrap-demo/desktop-profile.csv`.

This example uses the production `DesktopPlatform`; the live testing harness has
different scheduling and performs display-capability discovery per frame, so its
numbers should not be substituted for desktop-host measurements. Stage detail is
enabled after native-window registration so VSync wait is correctly identified.
Some layout work executes inside the redraw callback before `Runtime::render`;
the separate measure/arrange phase can therefore be empty even while text reflows.

An on-screen run on the same RTX 3090/DX12 machine used a 1280 x 720 logical
viewport, 1920 x 1080 physical pixels (150% scaling), and native HDR output:

| Backend settings | Host cadence | Work median | Work p95 |
|---|---:|---:|---:|
| Normal development defaults | 120 FPS | 1.98 ms | 2.42 ms |
| Native debug/validation disabled | 120 FPS | 1.60 ms | 1.90 ms |

Total frame time was about 8.10 ms median, with 6.37–6.76 ms average surface wait.
That wait is pacing, not CPU layout work. Cadence is measured at host presentation
return, not at physical scanout. The native viewport, DPI, clipping, and HDR path
differ from the headless benchmark, so the two are not an exact A/B comparison.
