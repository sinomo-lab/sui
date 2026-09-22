# Editorial engine

Run `cargo run -p sinomo-ui-demo` and open **Editorial engine**. The web gallery
also accepts `?benchmark=dev&demo=editorial`.

Inspired by [Pretext's Editorial Engine](https://somnai-dreams.github.io/pretext-demos/the-editorial-engine.html),
this demo uses original article text and SUI's existing text layout and rendering.
Resize the window to switch between one, two, and three columns. Drag circles
to change the available line widths; click a circle to pause its motion.
**Play all**, **Pause all**, and **Reset** control the scene. With the article
focused, Space toggles motion and R resets it. Circle toggles are also exposed
as accessibility actions.

## Deliberately small adapter

All obstacle handling lives in `crates/sui-demo/src/editorial_demo.rs`; no core
text-layout API or rendering architecture changes are needed. For each row,
the demo subtracts circle intersections and the rectangular drop-cap/pull-quote
regions from the column width. It consumes remaining slots left to right, then
continues down the column and into the next column. Slots narrower than 54
logical pixels are skipped. A fully blocked row consumes no text.

For each slot, SUI lays out the unconsumed paragraph suffix in a rectangular
box. The demo paints the first line and advances by its UTF-8 byte range.
This can materialize additional paragraph lines that are never painted; it
does not provide Pretext's prepared-text cursor or general exclusion layout.
Short paragraphs bound the work. Persistent handles reuse retained text draws.
The fixed-height page displays only the article prefix that fits; remaining
copy is not paginated. The copy includes combining marks and CJK, but this
adapter is not intended to establish arbitrary rich-text or bidi support.

The footer's **Reflow** timer covers the demo's measure-time composition and
text requests, excluding toolbar measurement, painting, and GPU rendering.
It reports the most recent measure, so a paused scene retains that value.

## Validation and profiling

```powershell
cargo test -p sinomo-ui-demo --lib editorial -- --test-threads=1
cargo test -p sinomo-ui-demo --lib editorial_visual_capture -- --ignored --nocapture --test-threads=1
$env:SUI_PROFILE_TEXT_TIMINGS = '1'
cargo test -p sinomo-ui-demo --lib editorial_frame_profile -- --ignored --nocapture --test-threads=1
```

Tests check overlapping exclusions, byte-for-byte article continuity across
slots and columns at several widths, scheduled animation, pause behavior,
dragging through redraws, accessible circle toggles, and viewport resizing.
Captures are written to `target/editorial-demo/width-{360,800,1280}.png`.

The headless WGPU benchmark mounts the article without the gallery at 1280 x
900. Each paused/animated phase has 30 warmup frames and 300 measured frames,
with a fixed 1/60-second animation step and a 17 ms sleep outside measurement.
It requests redraws even while paused, permitting comparison with retained
scene reuse. CPU runtime includes ticking, event dispatch, layout, and scene
painting; renderer time measures `renderer.render`, not GPU completion or
native presentation. Text timing is a subset of runtime time.
