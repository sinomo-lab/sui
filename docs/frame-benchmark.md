# Demo frame benchmark

`frame_bench` drives the desktop demo through its pages the way a person
would and reports how long each input takes to reach the screen. Use it to
find lag spikes and to compare frame cost between revisions.

```powershell
cargo run -p sinomo-ui-demo --release --example frame_bench
```

The benchmark opens a visible window without VSync. It opens every demo from
the picker, scrolls the page's largest scroll view down and back, moves the
pointer across the content, switches back to each open tab, and cycles the
theme. Each input is timed from dispatch until the frames it caused have been
presented, so a slow frame anywhere in that chain shows up in its sample.

## Output

The first table groups inputs by scenario:

| Scenario | Inputs |
| --- | --- |
| `open` | Opening a page for the first time, including building and laying it out. |
| `picker` | Returning to the demo picker. |
| `scroll` | One wheel step on the page's main scroll view. |
| `hover` | One pointer move across the content. |
| `switch` | Selecting a page's tab again after every page is open. |
| `theme` | Cycling the shell's theme. |

`mean`, `p50` through `max`, `>8ms`, and `>16ms` describe input time in
milliseconds. `work_p95` is the 95th percentile of the last presented frame's
measured work, and `frames` is the mean number of frames an input produced.
A second table gives scroll and hover times by page, followed by the slowest
inputs.

## Options

- `--pages a,b` keeps pages whose titles start with the given prefixes, for
  example `--pages widget,rich`.
- `--repeat N` repeats the tab-switch pass, so warm costs dominate.
- `--detailed` turns on detailed scene statistics, prints each slow input's
  phase breakdown, and averages phases per scenario. Detailed statistics add
  overhead, so compare only runs made with the same flags.
- `--trace` records every presented frame and lists the frames inside each
  input slower than 12 ms. With `--detailed`, heavy frames also show their
  phases and hottest widgets.

`SUI_PROFILE_TEXT_TIMINGS=1` adds text request counts and timings to detailed
output, and `SUI_PROFILE_WIDGET_TIMINGS=1` adds inclusive widget timings.

## Reading the results

The harness presents a frame after each host event, so one scroll input can
present several frames back to back. A composited window then blocks on the
next swapchain image; detailed output reports that wait as `Surface wait`.
It reflects rendering more frames than the display shows, not work the
application did, and the desktop host coalesces such events into one frame.

Results move with machine load, GPU clocks, and installed fonts. Run on a quiet
machine and compare revisions by building both, then alternating their runs.
Treat the maximum as a lead to investigate, not a measurement: confirm a spike
with `--trace` before drawing conclusions from it.
