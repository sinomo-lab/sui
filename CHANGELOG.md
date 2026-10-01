# Changelog

All notable changes to SUI are documented in this file. SUI follows Semantic
Versioning, with the usual expectation that the API may change during the
`0.x` series.

## [Unreleased]

### Breaking: refreshed default theme

- Redesigned the built-in themes around pure surfaces and vibrant decoration.
  SUI light uses achromatic white and gray surfaces; SUI dark keeps a faint,
  constant blue tint on its surfaces; every SUI preset shares a new azure
  primary (`#1762F4`) and violet secondary (`#7D4DE7`) instead of the former
  petrol teal and scheme-specific cyan.
- Replaced the daisyUI-style `ThemeColors` fields with a source model:
  `neutrals: NeutralRamp` (window, subtle, panel, overlay, control, button,
  field, border, and text tiers), `primary`/`on_primary`,
  `secondary`/`on_secondary`, `info`, `success`, `warning`, and `danger`
  (formerly `error`) with `on_*` content colors, and `decorative`. The
  `base_*`, `*_content`, `accent`, and `neutral` fields were removed.
- Every palette role is now derived from the source colors in OKLCH. Editing
  `colors.primary` (or any status color) re-derives hover, pressed, soft,
  border, legible text, focus, glow, and Display P3/HDR variants; built-in
  role tables keyed by theme name are gone.
- Added a nine-hue categorical palette (`DecorativeHue`, `DecorativeColors`,
  `DecorativePalette`) with derived solid, soft, text, and border roles, plus
  `DefaultTheme::tone_roles` and `ToneRoles` for complete per-tone role sets.
- Added `palette.button*`, `palette.border_control`, `palette.text_disabled`,
  and hover/pressed/border roles for every status tone. Keyboard focus rings
  now follow the primary color; selection fills stay neutral.
- Restyled built-in controls: ordinary buttons use a raised neutral face with
  an outline and full-strength labels, filled buttons no longer draw a darker
  ring, fields are white wells with a strong outline, unchecked checkboxes,
  radios, and switches use a 3:1 control outline, sliders lose their frame,
  tab bars drop their gray strip, and segmented controls use a raised thumb.
- Added `Oklch`, `Color::oklch`, `Color::to_oklch`, and WCAG
  `Color::contrast_ratio` to `sui-core`.
- `ActionCard::decorative` resolves card accents from the active theme's
  decorative palette; the demo launcher and chrome no longer hardcode colors.
- Built-in presets are derived once and cached.
- `Dialog::primary_action` and `SideSheet::primary_action` now build filled
  primary buttons. Dialog and side-sheet action buttons follow the surface's
  theme regardless of builder order and across live theme switches; added
  `Dialog::theme_when` and `ResponsiveSidebar::theme_when`.

### Widget API conventions

- Documented the conventions every built-in widget follows, in
  `docs/api/conventions.md`: builder and getter names, static, polled, and
  observed values, callbacks, and a shared state vocabulary. A phased plan in
  `docs/plans/widget-api-conventions-plan.md` moves the existing widgets onto
  them.
- Breaking: every `_with_ctx` callback receives the `EventCtx` first.
  `ListView::on_change_with_ctx`, `VirtualList::on_change_with_ctx`,
  `BrowserTabBar::on_change_with_ctx` and `on_close_with_ctx`, and
  `SegmentedControl::on_change_with_ctx` passed it last.
- Two-state inputs share one vocabulary. `Switch` takes `checked`,
  `checked_when`, `set_checked`, and `is_checked`; `RadioButton` takes
  `checked`, `set_checked`, and `is_checked`; and `Checkbox`, `Switch`, and
  `RadioButton` report `on_change(bool)` with an `on_change_with_ctx` twin.
  `Switch::on`, `on_when`, `set_on`, and `is_on`, `RadioButton::selected`,
  `set_selected`, `is_selected`, and `on_select`, and `on_toggle` are
  deprecated. A radio button's `on_change` fires only when it becomes
  checked; the deprecated `on_select` still fires on every activation.
- `Label::theme` supplies defaults: a color, font size, line height, or text
  style set on the label wins over it, whichever came first. Before, a theme
  set after them discarded them. `Label` gains `theme_when`, and
  `style`/`style_when` are deprecated in favor of `text_style`/
  `text_style_when`, which keep the label's other settings.
- The JavaScript and Python radio button actions run when the radio becomes
  checked, not again when an already checked radio is clicked.
- Inputs can be disabled: `Checkbox`, `Switch`, `RadioButton`, `Slider`,
  `NumberInput`, `Select`, `TextInput`, `TextArea`, `PasswordInput`, and
  `DateTimeInput` take `enabled`, `enabled_when`, and `enabled_from`. A
  disabled input ignores input, can't take focus, reports itself disabled,
  and paints with the new `DefaultTheme::for_disabled_control()`. The widget
  book shows each input disabled.
- Two-state inputs take `checked_from`, and `RadioButton` takes
  `checked_when`. `Button::is_enabled` and `IconButton::is_enabled` are
  public.
- Selection takes an optional index: every collection's `selected` accepts
  `impl Into<Option<usize>>`, so `selected(2)` keeps working and
  `selected(None)` clears it. `TreeView` gains `selected_path`. Breaking:
  `TabBar`, `Tabs`, and `SegmentedControl` return `Option<usize>` from
  `selected_index()`, `None` only when they have no items.
- Collections and choice groups can be disabled too: `RadioGroup`,
  `SegmentedControl`, `TabBar`, `Tabs`, `ListView`, `Table`, `TreeView`,
  `LayerList`, `ColorPicker`, `ColorPalette`, `ToolPalette`, `PresetStrip`,
  and `Menu` take `enabled`, `enabled_when`, and `enabled_from`. Disabled,
  the widget and each of its items report themselves disabled with no
  actions.
- Items take `enabled(bool)`; `disabled()` is deprecated. Breaking:
  `read_only()` takes a `bool` on `TextInput`, `TextArea`, `PasswordInput`,
  `DateTimeInput`, `ColorSwatch`, and `TextSurface`, which also gain
  `is_read_only()`.
- Every callback has a `_with_ctx` twin. New twins: `RadioGroup`,
  `NumberInput`, `TreeView`, `Table`, `Breadcrumb`, `TabBar`, `Tabs`,
  `PresetStrip`, `SplitView`, `TextSurface`, and `ColorPicker`
  `on_change`; `Popover::on_open_change`; the `on_dismiss` of `Dialog`,
  `SideSheet`, `BottomSheet`, and `CommandPalette`; `TextInput` and
  `TextArea::on_focus_change`; `TextArea::on_submit`; `DropTarget` hover
  callbacks; `RichDocumentView` link, image, and attachment callbacks; and
  every `VirtualTable` callback.
- `LayerList::on_select` and `on_select_with_ctx` are deprecated in favor of
  `on_change` and `on_change_with_ctx`. `VirtualTable` reports selection
  changes through `on_change(usize)` when the user clicks or arrows to a
  different row.
- `TextInput` gains `on_submit`, fired by `Enter`. `PasswordInput` and
  `DateTimeInput` gain `on_submit`, `on_focus_change`, `text_style`, and
  `appearance`. `TextArea` submits when only `on_submit_with_ctx` is set;
  before, `Enter` inserted a newline unless `on_submit` was set too.
- Overlays share one vocabulary: `Dialog`, `CommandPalette`, `SideSheet`,
  `BottomSheet`, `Popover`, and `Select` take `open`, `open_when`, and
  `open_from` and report `is_open()`; dialogs and sheets also take
  `set_open`. `Select` gains `on_open_change` and its `_with_ctx` twin.
  `shown`, `set_shown`, `is_shown`, and `Select::expanded` are deprecated.
- `Icon`, `Tooltip`, `Popover`, `CommandPalette`, and `NotificationHost`
  take `theme_when`. `FloatingStack::theme` is deprecated: it never had an
  effect.
- `Dialog::max_width` and the dialog and sheet `primary_action` and
  `secondary_action` buttons resolve theme metrics when they lay out, so a
  theme set after them still applies.
- Bound values use `_when` and `_from` builders: `StatusBadge` and
  `PlacementBadge` take `label_when` and `label_from`, and
  `StatusBarSegment` takes `text_when`. `Label::dynamic`,
  `StatusBadge::dynamic`, `PlacementBadge::dynamic`,
  `StatusBarSegment::dynamic`, `StatusBar::dynamic_segment`, and
  `RichText::dynamic` are deprecated, as are `RebuildOnChange::new` and
  `new_observable`, in favor of `RebuildOnChange::key_when` and `key_from`.
- Getters no longer share builder names: `Button::current_label`,
  `TextInput::accessible_name`, `ColorPicker::current_color`,
  `SimpleColorPicker::current_color`, and `FloatingWorkspace` and
  `DockWorkspace::shared_state` replace `label`, `name`, `color`, and
  `state`, which are deprecated.
- Layout words: `spacing` is deprecated in favor of `gap` on `Stack`,
  `KeyedStack`, `VirtualList`, `VirtualScrollView`, `ReorderableList`,
  `FieldGroup`, `Toolbar`, `CommandGroup`, and `ToolPalette`, and
  `Toolbar::line_spacing` in favor of `cross_gap`. `radius` is deprecated in
  favor of `corner_radius` on `Surface`, `ShadowBox`, `FormSection`, and the
  `ActionTilePaint`, `CalloutPaint`, `CodePanelPaint`, and
  `SectionPanelPaint` helpers.
- Color overrides are `colors(...)`: `CanvasAppearance`,
  `CanvasRulerAppearance`, `PixelCanvasAppearance`, and
  `ColorPickerAppearance` are now `CanvasColors`, `CanvasRulerColors`,
  `PixelCanvasColors`, and `ColorPickerColors`, and the `appearance`
  builders that took them are deprecated.
- Children: `SizedBox` and `ListItem` take `child(widget)`, `Canvas::content`
  becomes `child`, and `TreeItem` adds nodes with `item` and `items` and
  custom content with `child`. `with_child` on those, `with_content`,
  `ListItem::subtitle`, `activate_with_content`, and `TreeItem::children`
  are deprecated. Breaking: `SizedBox::child()` no longer reads the child;
  every container reads it with `child_pod()`, and the other containers'
  `child()` getters are deprecated.
- Breaking: `Link::url(url)` is a builder that sets the URL; build a link
  labeled with its URL with `Link::from_url`.
- `RadioButton` takes `semantic_name`.
- `CanvasViewport`, `CanvasSurface`, `FloatingViewConfig`,
  `TransientNotification`, `BrushPreviewSpec`, and `CoverageDotsConfig` are
  plain data: each implements `Default`, and their builders and constructors
  store values as given. The widgets clamp zoom, grid spacing, minimum view
  size, notification duration, and brush size and opacity when they use them.

### Breaking: a clearer `sui` facade

- `sui::Padding` is now the padding container widget at the crate root, as it
  already was in the prelude. The insets value is `sui::Insets` everywhere.
- Diagnostics plumbing moved from the crate root into `sui::diagnostics`:
  window performance snapshots and summaries, scene statistics, inspector
  snapshots and traces, cache and render diagnostics, renderer debug captures,
  output diagnostics, desktop automation, and accessibility audits. Import
  them from `sui::diagnostics` instead.
- The prelude is smaller. It no longer brings in the low-level `Application`
  and `WindowBuilder` (prefer `App` and `Window`, or import them), layout
  algorithm functions and results, compiled animation and editor internals,
  widget-specific `*Paint` structs and `paint_*` chrome painters (`paint_text`
  and `paint_text_line` stay), text render policy enums, `TEXT_COMMAND`,
  `WidgetDiagnosticsCtx`, or `set_window_render_options`. All remain at the
  crate root. It now includes `Error`, `SemanticTone`, `Modifiers`,
  `PointerButton`, `KeyState`, `EventPhase`, `Observer`, `Subscription`,
  `WeakObserver`, `SemanticRegion`, and `PlacementBadge`.
- `Result` takes an optional error type, `Result<T, E = Error>`, so a glob
  import of the prelude no longer breaks `Result<T, E>`.
- Text rendering settings are one set of types, defined in `sui-scene`:
  `TextRenderMode`, `TextSubpixelOrder`, `TextRenderHinting`,
  `TextRenderStemDarkening`, and `TextRenderCoveragePolicy`. The renderer's
  `TextHinting` and `StemDarkening` and the runtime's `WindowTextHinting`,
  `WindowStemDarkening`, `WindowTextCoveragePolicy`, and
  `WindowTextSubpixelOrder` are now aliases of them, and
  `WgpuRenderer::with_text_render_mode` takes the same `TextRenderMode` the
  facade exports.
- Deprecated the widget aliases `Modal`, `Drawer`, `BusyIndicator`,
  `Divider`, `SpinBox`, `MultilineTextInput`, `ComboBox`, `DataGrid`,
  `PathBar`, and `ResizablePane`; use `Dialog`, `SideSheet`, `Spinner`,
  `Separator`, `NumberInput`, `TextArea`, `Select`, `Table`, `Breadcrumb`, and
  `SplitView`. Documentation search still finds the canonical types by the
  old names.
- Deprecated `register_builtin_icon_resources`, which did nothing, and the
  facade's unused `Style`.
- Removed the `testing` Cargo feature of `sinomo-ui`, which enabled nothing.

### Breaking: redesigned widget book

- The widget book is now one scrolling page of 60 component stories in nine
  categories. Each story lays out its variants, tones, sizes, and interaction
  states side by side in labeled grids, so every variation is visible without
  clicking. Overlays such as menus, tooltips, popovers, selects, context
  menus, and dialogs are shown open in place.
- A navigation rail lists every category and component in page order, jumps to
  a component, and highlights the component currently at the top of the page,
  scrolling itself to keep that entry visible. A filter collapses
  non-matching components and categories, and a theme switch rebuilds every
  story in Light, Dark, Neutral, Neutral dark, or Void (plus App when the book
  is embedded in a host that owns the theme). The rail hides in narrow windows.
- Stories are registered in one ordered list that drives the page, the rail,
  search, tests, and visual artifacts.
- `WidgetBookState`, `default_widget_book_state`, and the widget-book summary
  were removed. `build_widget_book_application`, `build_widget_book_gallery`,
  and `build_widget_book_gallery_with_theme` no longer take state, and
  `build_theme_demo_application` and `build_theme_demo_surface*` no longer take
  it either.
- Moved the benchmark surfaces to `sui_demo_app::benchmarks`, the text and color
  validation surfaces to `sui_demo_app::validation`, the Themes page and HDR
  theme lab to `sui_demo_app::theme_demo` (`set_widget_book_hdr_theme_mode` is
  now `set_hdr_theme_lab_mode`), and `LivePerformanceRoot` to
  `sui_demo_app::live_performance`, which no longer watches widget-book state.
- The visual artifact bundle now holds `overview-light/`, `overview-dark/`,
  `narrow-light/`, `stories/<id>/{light,dark}.png` for every story,
  `themes-page/`, and `hdr-validation/` (formerly `hdr-widget-book/`).

### Added

- `InteractionPreview` and `interaction_preview` on `Button`, `IconButton`,
  `Checkbox`, `RadioButton`, `Switch`, `Slider`, `NumberInput`, `TextInput`,
  `PasswordInput`, `DateTimeInput`, `TextArea`, and `Select` pin hover, press,
  or focus visuals for galleries, documentation, and screenshots. Previews
  affect paint only; semantics keep reporting real state, and disabled controls
  ignore them.
- `show_inline` on `Tooltip`, `Popover`, `ContextMenu`, `Select`, and `Dialog`
  keeps the overlay open and lays it out in flow as part of the widget's own
  size instead of floating in the window overlay stack. Inline overlays ignore
  dismissal. `ContextMenu::highlighted_path` highlights an item and opens the
  submenus leading to it.
- `ScrollState::virtual_item_at`, `first_visible_item`, and
  `virtual_item_offset` report which `VirtualScrollView` item sits at a content
  offset, for scroll-spy navigation.
- `SimpleColorPickerMode::Oklch` edits perceptual lightness, chroma, and hue.
  Chroma beyond the editing space's gamut is reduced to fit at the same
  lightness and hue, the chroma track marks the gamut edge, and the hue is
  kept while chroma passes through zero. The Python and JavaScript bindings
  accept `"oklch"`.

### Motion

- Retargeting a transition mid-flight now keeps its momentum. `MotionScalar`,
  `AnimatedValue`, and the new `MotionValue<T>` blend the new animation from
  the one still running instead of restarting at zero speed, so a hover that
  ends halfway through its fade turns around smoothly. Built-in widgets get
  this automatically.
- Added `AnimationSpec` (a tween or a spring) and `SpringSpec`, a spring set by
  duration and bounce with `SMOOTH`, `SNAPPY`, and `BOUNCY` presets.
  `ThemeMotion` gains `hover_spec`, `press_spec`, `focus_spec`, `toggle_spec`,
  `entrance_spec`, and `tab_switch_spec`, and `MotionScalar` gains
  `set_target_with` and `set_target_event_with`.
- `SpringF32` now steps with the exact spring solution, so it follows the same
  path at any frame rate and stays stable across long frames. Added
  `SpringF32::from_spec`, `with_velocity`, `is_settled`, and `settle`.
- Added an app-wide motion policy. `MotionPreference` (`Full`, `Reduced`, or
  `Off`) follows the operating system on Windows and the web, and
  `set_app_motion_preference` overrides it. With reduced motion, popovers,
  tooltips, menus, dialogs, and side sheets appear in place instead of
  sliding, and tab, segmented-control, and list-reorder movement jumps; with
  motion off, transitions finish immediately. `set_motion_time_scale` slows
  every transition down for inspection. Widget contexts expose
  `motion_policy()`, and `MotionScalar::set_movement_target` marks transitions
  that move content.
- Color interpolation (`Interpolate for Color`, used by animated colors and
  widget state blends) now mixes in premultiplied OKLab via the new
  `Color::mix_oklab`, so fades from a transparent color no longer darken
  midway. Colors in different spaces blend correctly; `Color::to_space`
  converts between them.
- Transform interpolation blends translation, rotation, scale, and shear
  separately, so rotating shapes no longer shrink mid-turn.
- Added `Interpolate::extrapolate` for values that may overshoot, such as a
  bouncy spring.
- `AnimationEditorCommand::MoveKeyframe` moves a keyframe in time with
  snapping and undo.
- `sinomo-ui-testing` adds `TestApp::settle_animations`, `record_motion`,
  `has_running_animations`, `set_motion_preference`, and
  `set_motion_time_scale`. Each test app starts at full motion regardless of
  the machine's setting. `Runtime::has_pending_animation_frames` reports
  whether a window still has transitions running.

### Runtime-driven motion, frame pacing, and layer scale

- Added `Motion<T>` and `Progress`: values the runtime animates for a widget.
  `ctx.animate(&mut motion, target, spec)` (or `progress.animate(target, spec,
  ctx)`) starts a transition from an event, measure, or arrange context, and
  `get(ctx)` reads it for the frame being painted. The runtime invalidates the
  widget every frame until the transition ends, so the widget handles no
  animation frames. A motion can invalidate `Transform` or `Effect` instead of
  paint, and `Widget::layer_properties_at(frame_time)` reads it to move, fade,
  or scale a retained layer without repainting.
- Built-in controls, forms, toolbars, tab bars, segmented controls, tabs, list
  and table rows, color swatches and palettes, scroll bars and views, split
  views, canvases, text surfaces, reorderable lists, and `LayoutTransition`
  now use runtime-driven motion, removing 36 of the 47 animation-frame
  handlers in built-in widgets. (Tooltips, popovers, menus, context menus,
  dialogs, side sheets, and the select menu followed; see Choreography below.)
- On displays that present with vsync, animation frames now follow the
  display: the platform starts one frame per refresh and stamps it with the
  time the frame is expected on screen, on a steady cadence of whole refresh
  intervals. Previously frames came from a 120 Hz timer that beat against the
  display (skipping updates at 144 Hz) and carried the time the event loop
  woke. Hidden windows and headless hosts keep the timer. `FramePacing`,
  `Runtime::set_frame_pacing`, and `Runtime::begin_animation_frame` expose this
  to custom hosts, and `PaintCtx::frame_time` reports a frame's time.
- `LayerProperties` gains `scale` and `scale_anchor` (`with_scale`,
  `with_scale_xy`, `with_scale_anchor`), composed through nested layers by the
  retained renderer without re-rasterizing content. Popovers, menus, and select
  lists now grow from 96% at the edge next to their trigger as they appear;
  reduced motion shows them at full size. Constructing `LayerProperties` with
  a struct literal now needs `..LayerProperties::default()`.

### Choreography

- Added `Presence`, which shows or hides one child and animates it in and
  out: it fades and grows from 96% by default (`PresenceTransition` sets the
  opacity, scale, offset, and enter and exit specs), and with `collapse` the
  space it takes grows and shrinks too, so neighbors move smoothly. Drive it
  with `shown_from(observable)` or `shown_when(closure)`; `appear()` animates
  the child in on first layout. A hidden child stays retained and takes no
  space. Reduced motion only fades: space opens at once on entry and closes
  after an exit.
- Added `KeyedStack`, a row or column kept in step with a list of keyed items
  from an observable. Each item's widget is built once and updated through a
  `Signal`; new items enter one after another (`stagger`), removed items leave
  while the gap they leave closes, reordered items glide to their new places
  (`move_spec`), and an item added back while it is leaving turns around.
- Content that is leaving is inert: `WidgetPod::set_inert` keeps a subtree
  painted but takes it out of hit testing, focus, and the accessibility tree,
  and releases focus, pointer capture, and drags inside it. The widget graph
  reports it as `WidgetNodeSnapshot::inert`.
- Transitions can start after a delay: `ctx.animate_after(&mut motion,
  target, delay, spec)`, `Motion::start_after`, and
  `MotionValue::animate_to_after`. Delays follow the motion policy's time
  scale and are skipped when motion is off. `Stagger` (with `StaggerOrigin`
  first, last, center, or an index, and an optional `max_delay`) computes the
  delays for a group; `ThemeMotion::stagger` is the theme's cascade.
- `ThemeMotion` gains `exit_spec` (content leaving) and `layout_spec`
  (content gliding to a new place).
- Added `ctx.track_motion_for` to keep another widget's layer updated while it
  animates, and `ctx.track_motion_end` / `track_motion_end_for` to invalidate a
  widget once when a motion ends, such as a surface that stops taking space
  once it has faded out. Motion started for a widget that joins the widget
  graph at the next layout, such as a surface that is opening, now runs
  instead of being dropped by an animation frame that starts first.
- Tooltips, popovers, menus, context menus, dialogs, side sheets, and the
  select menu now use runtime-driven motion; no built-in widget advances its
  own animation from frames except the node editor's viewport and edge
  animations. A hiding tooltip or popover keeps its surface until it has
  faded out.
- Notifications slide in, fade out when dismissed or expired, and the rest
  glide into place. Closing a browser tab fades its label while its space
  collapses and the tabs after it slide over.
- Timelines: `LoopMode::PingPong` plays forward, then backward. Players can
  wait before starting (`PlaybackState::start_delay`) and hold at each end
  before looping (`loop_delay`). Timelines carry named `TimelineMarker`s;
  players report the markers the playhead passes each tick
  (`AnimationPlayer::passed_markers`, `TimelineTick::passed_markers`), once
  per pass and in order, including across loops. `PlaybackState::tick_spans`
  reports the stretches of timeline a tick covered, and the animation editor
  adds, moves, renames, and removes markers with undo. Animation documents are
  now version 2, which adds `marker` lines; version 1 documents still parse.
  `PlaybackState` gained fields, so struct literals need
  `..PlaybackState::default()`, and `stop()` now also resets a reversed
  playback rate.
- The JS and Python bindings add `Presence`, `Stagger`, `AnimationMarker`,
  `AnimationTimeline.addMarker`/`add_marker` and `markers`, player loop modes
  (`setLoopMode("ping-pong")`), start and loop delays, `passedMarkers`,
  marker editing on `AnimationEditor`, and `AnimatedValue.setTargetAfter`.
  `KeyedStack` is Rust-only for now.
- Widgets built by the bindings now keep their own layers, overlay behavior,
  intrinsic sizes, and commands: the binding wrapper forwarded only events,
  layout, paint, and semantics, so `LayoutTransition` never animated and
  overlays lost their dismissal and focus policy in JS and Python apps.
- The `sui` facade now exports `LayerProperties` and `LayerCompositionMode`,
  which custom widgets need to present retained layers.

### Redesigned animation demo

- The Animation page is one scroll. Its header sets the app-wide motion
  preference (following the system, full, reduced, or off) and playback speed
  (1× down to 0.1×), and toggles motion traces.
- Easing and springs: a card for each theme curve and spring preset plots the
  curve and loops a puck along it; clicking a card sends it back mid-flight.
- Interruptible motion: the same retargets played by restarting from rest and
  by keeping momentum, charted side by side, and a puck that springs home with
  the velocity it is flung at.
- Built-in widgets: buttons, choices, segmented controls, tabs, tooltips,
  popovers, selects, text fields, and sliders, each labelled with the theme
  token that times it.
- Timeline studio: keyframes drag along their tracks with snapping, the ruler
  scrubs, the selected keyframe's curve has draggable bezier handles, edits
  undo and redo, Space, Delete, and the arrow keys work when focused, and
  "Copy document" copies the animation document as text.
- Under the hood: a retained layer and a repainted chip move side by side with
  counts of frames, repaints, and layer moves, next to a graph of animation
  frame intervals.
- Choreography: a keyed task list that adds, removes, and shuffles with
  animation; a row of bars that cascades from the first, center, or last bar;
  a switch that shows and hides a panel with `Presence`; and closable tabs and
  notifications. The timeline studio plays once, repeats, or ping-pongs, can
  hold at each end, and shows its markers on the ruler, reporting the last one
  it passed.

### Redesigned theme editor demo

- Source colors are grouped by job (brand, status, surfaces, fills, borders,
  ink, and decorative), each with its hex value, an edit marker, and the WCAG
  contrast of "on" colors and text levels. Pressing a color opens an OKLCH,
  RGB, or HSL editor with a hex field right beneath it.
- Role overrides are added explicitly and listed with their own reset; spacing,
  corner radius, type size, and motion sliders show their values.
- The preview lays out contrast checks (failures first), every surface tier
  with each text level, the decorative palette, a looping motion sample, and
  15 widget book stories in the edited theme.
- "Copy as Rust" copies a function that rebuilds the theme, and "Use as app
  theme" applies it to the whole demo app, including the widget book's App
  theme; the dev shell's theme toggle shows "Custom" until it is used again.

### HDR output and validation

- Breaking: tone mapping now keeps hue and leaves SDR content alone. `Clamp`
  clips a highlight by scaling it until its brightest channel reaches SDR
  white, instead of clamping each channel (which turned bright orange yellow
  and bright blue cyan). `Reinhard` clips the same way, then turns the energy
  above SDR white toward white, so brighter highlights still read brighter;
  colors within SDR range pass through unchanged, where the old curve dimmed
  SDR white to half. Both apply in the output transform and, through a
  pipeline constant, when scenes draw straight into an sRGB surface, which
  previously clamped each channel in hardware.
- Display P3 colors now reach wide-gamut and scRGB outputs. The output
  transform clipped negative channels before converting to the output's
  primaries, so P3 colors collapsed to their sRGB counterparts on P3 displays
  and in native HDR. Colors are now clipped only after conversion, and only
  for outputs that cannot carry them; extended sRGB encoding mirrors negative
  channels.
- Added `fit_to_sdr`, the tone mapping math as the GPU runs it, to
  `sinomo-ui-render-wgpu` and the `sui` facade.
- Apps can capture their own window: `request_window_debug_capture` asks for a
  stage of the next frame, the platform makes it after redrawing and wakes the
  requesting widget, and `take_window_debug_capture` hands out the artifact.
  Captures are native-only; browsers report an error. Hosts that run their own
  event loop call `service_window_debug_captures` after each redraw and keep
  redrawing while `has_pending_window_debug_captures`; the live test harness
  does.
- The `sui` facade re-exports `DebugCaptureRequest`, `DebugCaptureStage`,
  `DebugCaptureEncoding`, `DebugSdrVisualization`, `DebugCaptureArtifact`,
  `DebugCaptureTicket`, `HdrRgbaImage`, `RgbaImage`, `OutputStrategy`,
  `DisplayCapabilities`, `DisplayColorPrimaries`, and
  `RequestedToneMappingMode`.
- Added `Checkbox::checked_when` for check state that other controls can also
  change.
- Added `window_output_diagnostics_signal`, the window's output diagnostics as
  a signal. Widgets that observe it are invalidated when a presented frame's
  diagnostics change, instead of showing the values of whichever frame they
  last happened to be measured in. The demo's Settings diagnostics panels and
  the HDR validation page use it.
- Screenshots, offscreen renders, and PNG previews of HDR captures now fit
  highlights with the window's tone mapping instead of always clamping. The
  headless platform passes each window's color management to the renderer,
  which keeps it for windows without a surface
  (`WgpuRenderer::window_color_management`).

### Redesigned HDR validation demo

- The HDR validation page is one scroll of probes, each saying what it should
  look like on the window's current output: a verdict naming the output
  (SDR, wide-gamut SDR, or native HDR with its headroom above SDR white); a
  headroom ramp and white ladder with the display's peak marked; a plot of
  each channel as a highlight brightens, with hue grids whose halves are fitted
  on the GPU and the CPU and should match; Display P3 tiles split against
  their sRGB-clipped selves beside a chromaticity diagram; gradient ramps for
  banding; and the same controls under each HDR theme mode.
- The page carries the window's output controls. They edit the same options
  as Settings, so each shows what the other changed, and the probes follow
  the output as soon as a frame is presented with the new options.
- "Capture frame" records the scene before output conversion and the final
  output, writes them as EXR with luminance, headroom, and clip maps and the
  diagnostics report under `target/ui-artifacts/sui-demo/hdr-validation`, and
  shows measurements and thumbnails. "Copy report" copies the diagnostics and
  the last capture's measurements for bug reports.
- `sui-demo-artifacts` writes its HDR bundle with the same code. Its
  `output-diagnostics.txt` now includes the display's reported luminance and
  SDR white, and `capture-metrics.txt` the share of pixels above SDR white and
  final-stage values relative to SDR white (native HDR finals are scRGB, where
  SDR white is its brightness over 80 nits).

### HDR themes follow the output, and tests can simulate displays

- HDR theme modes fall back to what the window's output can show: Constrained
  and Full HDR to wide-gamut only on wide-gamut SDR outputs, and every mode to
  the SDR baseline on sRGB outputs. HDR accents no longer reach SDR displays
  as clipped, near-white fills with unreadable labels. The platform records
  each window's `OutputColorRange` before rendering; widgets read it with
  `PaintCtx::output_color_range`, which repaints them when it changes, and
  apply it with `HdrThemeMode::limited_to` or `HdrThemeTokens::limited_to`.
  Buttons, switches, and popovers do.
- Windows rendered without a surface can be given a display with
  `WgpuRenderer::set_window_display_capabilities`: they choose the output
  strategy a surface on that display would, report it, and capture the final
  output accordingly. `HeadlessPlatform::with_display_capabilities` renders
  every window for a display, and the headless platform now reports its real
  output strategy instead of always an SDR surface.
- `TestApp::builder` configures a test app; its `display_capabilities` option
  runs the app headless for a simulated display, so SDR, wide-gamut, and HDR
  paths are testable on any machine. `DisplayCapabilities::sdr`,
  `wide_gamut`, and `hdr` build common displays.
- The desktop and headless platforms and the live test harness present frames
  through one routine, `present_window_frame`, instead of three copies. The
  live harness now applies a window's text rendering options like the app
  does, and resetting it no longer clears every other window's render options
  and diagnostics in the process.

### Redesigned Settings

- The demo's Settings groups every render option into Display, Text, Shapes,
  and Developer sections, under three rows summing up the window's output:
  what it presents, its SDR white, and the HDR theme mode widgets preview,
  including when the output limits it. Settings that only matter with another
  one, like the hinting size limit, show with it. Reset to defaults puts the
  options back.
- Settings is a closable view of a transparent `FloatingWorkspace` over the
  demo instead of its own floating window, and every control writes through
  the same options the HDR validation page edits, which now shows the HDR
  theme mode too. The hand-drawn diagnostics panels and the notes paragraph
  are gone; the HDR validation page keeps the full output details.
- `FloatingWorkspace::transparent` floats its views over other content: it
  paints no background, and points outside every view reach what is behind
  it. `FloatingViewConfig::closable` gives a view a close button in its title
  bar and closes it on Escape.
- `Widget::hit_test_self` lets a container hit only through its children, so
  points it covers but no child takes reach what is behind it.
- Changing a window's render options repaints the whole window on its next
  frame. Widgets that read an option while painting, like optical text
  centering, used to keep their old paint until something else invalidated
  them.
- `Switch::on_when` shows state that other controls can change too, like
  `Checkbox::checked_when`.
- `OutputColorRange`, `window_output_color_range`, and
  `window_output_color_range_signal` are exported from `sui`.
- `FloatingViewConfig` and `FloatingViewSnapshot` have a new `closable` field,
  so code building them with struct literals needs it.

### Output diagnostics, browser captures, and unique control names

- Output diagnostics report the colors the output shows in a new
  `output_gamut` field, from `OutputStrategy::gamut`: sRGB, Display P3, or the
  display's own gamut. Native HDR output (scRGB) is encoded in sRGB primaries,
  so `preferred_primaries` read as sRGB even though wide-gamut colors reach a
  wide-gamut display; the HDR validation page no longer guesses from
  `supports_wide_gamut`. `WindowOutputDiagnostics` gains the field, so code
  building it with a struct literal needs it.
- Debug captures work in browsers. The renderer begins a capture without
  waiting for the GPU (`WgpuRenderer::begin_debug_capture`) and hands it out
  once the copy is back (`take_finished_debug_captures`); browsers collect it
  on a later frame and wake the requesting widget then, where they used to
  return an error. Captures also accept 8-bit unorm targets, which browsers
  present to.
- Settings' copies of controls the HDR validation page shows have their own
  accessible names, like "Tone mapping in Settings", so automation and
  assistive technology can tell them apart when Settings is open over the
  page. `Switch::semantic_name` and `Checkbox::semantic_name` name a control
  apart from its label. The near-white swatches on the page are named "Near
  white", no longer repeating the ladder's "White 1×".

### Redesigned text demos

- The demo's four text pages are now three, grouped with the other text
  demos in the picker:
  - Text rendering carries the window's text settings, the same controls as
    Settings, above samples from 10 to 20 px on light, dark, and colored
    backgrounds drawn with them. Two render policies are shown side by side,
    with a magnifier that captures the window and shows a corner of each
    sample pixel for pixel. Probes for stem darkening, hinting, coverage on
    dark backgrounds, subpixel positions, and optical centering each say what
    should differ, and every per-text policy is listed with the code that
    sets it.
  - Scripts and shaping lays out samples of nine scripts with the fonts this
    system has and checks each: no glyph missing, Arabic letters joining,
    Hebrew points and a Devanagari conjunct forming one cluster, and an emoji
    family staying one cluster. Each row names the fonts that drew it. The
    page also shows mixed-direction text with its runs underlined by
    direction, line breaking at a width you choose, and a font's vertical
    metrics.
  - Text editor opens highlighted code, mixed-direction prose, input method
    practice, and a 20,000-line document, with wrap and direction controls
    and an inspector for the caret, the selection, the input method's
    composition, the lines on screen, and the size.
- Retained text is no longer in the picker; it remains a benchmark. The web
  presets are now `text-rendering` and `text-editor`; `text-comparison` and
  `text-editing` still open them, as do the old `demo=` names.
- `TextSurface::status` keeps a `Signal<TextSurfaceStatus>` up to date with
  the caret's line and column, the selection, the text an input method is
  composing, the lines on screen, and the text's size. `set_wrap`,
  `set_direction`, and `set_selection` change a surface in place. Ctrl+Home
  and Ctrl+End (Cmd on macOS) move to the start and end of the text, and
  select with Shift.
- `TextSurface` lays text out in its direction when it has no style spans;
  the direction was ignored. Wrapped text with style spans is laid out as one
  paragraph per line; it was one paragraph with line breaks inside, and drew
  nothing. Unwrapped lines start at the left edge in either direction, as in
  code editors; right-to-left lines were aligned to the far edge of a
  million-pixel box, off screen. The current line's band follows the caret
  when lines wrap.
- Paragraphs given a direction are aligned by that direction's edges. A
  right-to-left paragraph of Hebrew or Arabic started at the left edge,
  because its alignment followed the direction detected from its text.
- `ResolvedTextFace::family_name` reads a face's family from its name table.
  `ImageSampling` and `ImagePixelSnap` are exported from `sui`.

### Demo quick fixes

- The Themes page is gone. The HDR validation page already compares the
  controls under each HDR theme mode, and its preset gallery repeated the
  Theme editor's presets; the `themes` demo name opens the Theme editor. The
  mode the demo previews moved from `sui_demo_app::theme_demo` to
  `sui_demo_app::hdr_theme_mode` (`hdr_theme_mode` and `set_hdr_theme_mode`),
  and visual artifacts no longer write `themes-page/`.
- `IconGlyph` gains `Blocks`, `Palette`, `Type`, `Languages`, `FileCode`,
  `Newspaper`, `Sun`, `LayoutDashboard`, `Move`, `PenTool`, and `Workflow`,
  with binding names such as `file-code` and `layout-dashboard`. The demo
  picker gives each demo its own icon, and every description fits its card.
- The Editorial engine lays out the whole article and scrolls when it is
  longer than the window, with its status line in a footer that stays in
  view; the circles move within the first screen. Lines no longer start with
  the space the previous line broke at, which indented them by a space, and
  a slot a circle narrows too far for the next word is left empty instead of
  splitting the word. The pull quote no longer hides the circles behind it.
- Rich documents registers an image for its sample document, a frame-time
  sparkline drawn inline in a sentence; the chart it referred to never
  resolved, so only its alt text showed.
- The Node graphs decision node's subtitle is legible on dark themes. It
  used `palette.warning_text`, text for a warning fill, where plain surfaces
  need `surfaces.warn_text`.

### Large documents in TextSurface

- Wrapped text is laid out a line at a time, like unwrapped text, so
  measuring shapes only the lines on screen, a few around them, and the
  caret's line. It used to lay the whole document out again for every edit
  and resize. In the Text editor's 20,000-line document, turning wrapping on
  and each keystroke after took about 2.5 s and now take milliseconds.
- A wrapped line not shaped yet counts as one row until it is. When lines
  above the view turn out taller, the first line on screen stays in place,
  and the caret is revealed again once the lines it moved past are shaped.
  Up and Down move by rows on screen, Home and End go to the ends of a row,
  and clicks land on the row under the pointer.
- Selections are drawn only on the lines on screen. Selecting a whole large
  document computed rectangles for every selected line on every frame:
  about 75 ms a frame at 20,000 lines, now a few.
- Selections are drawn as a rectangle per piece of text on a row, so
  mixed-direction text is highlighted where it is. A row was one rectangle
  between the ends of the selection, which in a right-to-left paragraph could
  cover the wrong words or none.
- Finding the line of an offset while an input method composes is a binary
  search instead of a scan.

### Drag and drop

- Drags carry the modifier keys held. `DragEvent::modifiers` is updated from
  pointer and key events, and targets get a fresh `Over` when the keys
  change without the pointer moving, so what they accept can follow them.
  `DropEffect::for_modifiers` reads the platform convention (Control copies,
  Shift moves, both link; Option, Command, and both on macOS), and
  `DragEvent::preferred_effect` picks the effect they ask for when the source
  allows it.
- Breaking: sources can allow several effects. Added `DropEffects`,
  `Draggable::effects`, and `DragEvent::allowed_effects` and
  `DragPreview::allowed_effects`; `allowed_effect` is now the effect a drop
  takes when no key asks for another. `EventCtx::begin_drag` takes
  `impl Into<DropEffects>`, so a single `DropEffect` still works. Code that
  builds `DragEvent` or `DragPreview` values needs the new fields.
- Escape cancels a drag, and losing window focus cancels it too; the source
  now hears the cancelled `End` instead of the drag vanishing.
- `ScrollView` and `VirtualScrollView` scroll while a drag is held near an
  edge, faster closer to it; `auto_scroll_on_drag(false)` turns this off.
  Targets under a pointer that did not move update as content scrolls under
  it: scroll views call the new `EventCtx::refresh_drag_targets`.
- `DragDropHost::preview` builds the widget drawn under the pointer for a
  drag, such as a lifted card, in place of the label. Previews follow the
  pointer in the host's coordinates, so a source inside scrolled content no
  longer draws its label off by the scroll offset.
- `DropTarget::on_hover_state` reports `DropHover`: idle, accepting with the
  effect, or refusing a drag in its scope, so a target can show that
  something cannot land there.
- `ReorderableList` moves rows with the keyboard. Alt+Up and Alt+Down move
  the row that has focus, or the list's current row, one place, and Alt+Home
  and Alt+End move it to either end; Up, Down, Home, and End choose the
  current row while the list has focus. Each move reports through
  `on_reorder` and is announced by a status node, naming rows with
  `item_name`. The list now takes focus.
- The test harness's pointer events carry the modifiers they are given, and
  key events update the modifiers later pointer events report, as on a
  desktop. Both were dropped, so tests could not hold a key while dragging.

### Redesigned Layout and Drag and drop demos

- Layout shows its examples in frames whose width you set with a slider,
  Phone, Tablet, and Desktop presets, or by dragging a frame's edge; the
  widths the examples change at are marked under the slider, and Show sizes
  labels tiles with their sizes. A flex playground lays out five tiles by
  direction, justification, alignment, gap, and wrapping, with each tile
  switching between fixed, growing, and capped when pressed, and shows the
  builder code. A grid takes a sizing mode per column and reads each track's
  resolved width. The responsive examples name the container query rule that
  matched and the sidebar's mode, and the master-detail back button shows
  only when one pane shows at a time. Panes read their widths and reset; a
  phone shows its safe area insets, with a keyboard to raise them.
- Drag and drop is a board: a shelf of assets and text snippets, three
  columns of cards to reorder and move between (hold Ctrl to copy), assets
  that attach to a card and snippets that join its note, a trash with undo,
  a text-only field that visibly refuses anything else, a palette locked in
  its own scope, and files from the desktop that become cards. An event log
  follows each drag from start to drop or cancel, numbered by session. Cards
  move with Alt and the arrow keys, keeping focus across columns, or from
  their menu, and Delete trashes one.

### Command history, focus-keeping buttons, and a redesigned Commands demo

- `window_command_dispatches_signal(window_id)` holds a window's latest
  command dispatches, up to `COMMAND_HISTORY_LENGTH` (64), as a signal a
  widget can observe: the samples the performance snapshot and inspector
  record, with each command's sequence, target, and delivery, the listeners
  that ran, whether one handled it, and whether it was delivered at all. The
  history is dropped when its window closes.
- `CommandSender` and `UiHandle` sends return the command's sequence number,
  which handlers see as `Command::sequence` and the history records. A send
  used as a `()` expression, such as a match arm, needs a semicolon.
- `Button::focus_on_press(false)` and `IconButton::focus_on_press(false)` make
  clicking the button leave focus where it is, so a toolbar button can act on
  the editor being typed in without taking focus or its selection. The button
  stays focusable: Tab reaches it, Enter and Space press it, and assistive
  technology can focus and activate it, which leaves focus alone too. Custom
  widgets do the same with `EventCtx::keep_focus`.
- `FocusScopeState::last_focused_observable` follows the widget in a scope
  that had focus last, the editor a toolbar acts on.
- The demo's Commands page is redesigned. A trace under the header lists the
  window's dispatches as they happen, from its dispatch history. A route map
  sends a ping to a widget, the focused widget, a closed widget, the window,
  or the application, directed or broadcast, or wakes the controllers, from
  the UI thread or a worker; named application and window controllers, whose
  switches decide whether they handle it, a target card, and a notes field
  light up with what each did, and a line says whether the runtime delivered
  it at all. An edit toolbar sends text commands to the editor used last
  without taking focus, and an export on a worker thread reports each
  thumbnail to its progress bar with `send_widget` and its end to the window
  with `send_window`, and can be cancelled.

### Faster tests

- `TestApp` runs apps headless on every platform unless asked otherwise, so
  UI tests run in parallel instead of one at a time through real windows. A
  test that needs real windows, such as frame pacing or native input, asks for
  them with `TestAppBuilder::live(true)` or `visible(true)`; set
  `SUI_TEST_BACKEND=live` to run every test live. Tests that relied on the live
  backend by default need `live(true)`.
- Renderers without a window share one GPU device and the pipelines compiled
  on it. Acquiring a device and compiling pipelines took over a second for
  every headless test app and offscreen capture; after the first, they start
  in milliseconds.
- Workspace crates no longer run doc tests, which only re-analyzed each crate:
  the two crate-level examples are also example programs that `cargo test`
  builds.
- The widget book renders every story in each theme in a test per theme, and
  the desktop widget book scroll benchmark runs only when asked for. A full
  workspace test run takes about a quarter of the time it did.

### Node graph editing: typed ports, undo, copy and paste, and context menus

- `Handle::label` names a connection point: the graph draws the name inside
  the node beside it, and screen readers announce each handle with its node and
  connections. `Handle::color` colors a handle, and edges leaving a colored
  source handle take its color, so ports can show the type of value they carry.
  `NodeGraphConfig::handle_labels` turns the drawn names off.
- `NodeGraph::connection_rule` accepts or refuses connections with a reason.
  A connection being dragged snaps to the nearest handle within
  `NodeGraphConfig::connection_radius`; handles that would take it are ringed
  and the rest dim, and over a refusing handle the line turns the danger color
  with the reason beside the pointer. Dropping it there emits
  `NodeGraphEvent::ConnectionRefused`. A reconnected edge is checked against
  the graph without it. `is_valid_connection` still takes a yes or no.
- `NodeGraphState::with_history` keeps undo steps: `undo`, `redo`,
  `history_status`, `history_observable`, `undo_group`, `begin_undo_group`,
  `end_undo_group`, and `merge_undo`, which merges edits that arrive one at a
  time, such as a slider drag, into one step. Selection, the viewport, and
  measured sizes are not steps; a drag or a resize is one. The graph undoes and
  redoes with Control+Z, Control+Shift+Z, and Control+Y.
- `copy_selection` and `paste_clipboard` copy selected nodes with their
  descendants and the edges between them, and paste them with fresh ids as one
  step. The state's own clipboard backs `copy`, `cut`, `paste`, and
  `duplicate`, and Control+C, +X, +V, and +D.
- A right-click that does not drag, the Menu key, or Shift+F10 asks for a
  context menu for what is under the pointer or focused, selecting it first:
  `NodeGraph::on_context_menu` and `NodeGraphEvent::ContextMenu`. A right-drag
  still pans. New events report copies, pastes, undo, and redo.
- `NodeGraphEvent` has new variants, and `NodeGraphConfig` and `Handle` have
  new fields: exhaustive matches on the events, and struct literals without
  `..Default::default()`, need updating.
- `ContextMenuHandle` opens a `ContextMenu` from code at a point, for triggers
  that decide for themselves when a menu is asked for, and gives focus back to
  the widget that opened it when the menu closes.
- `Button::enabled_from` and `IconButton::enabled_from` follow an observable
  enabled state, repainting when it changes.
- The demo's Node graphs page is a color lab. Color and number nodes feed mix,
  lighten, and contrast nodes, and previews show the results, recomputed as
  you drag a slider in a node or rewire the graph. Ports are typed; the lab
  refuses a mismatched type, a loop, or a second edge into an input, and says
  which. A toolbar, context menus, and the keyboard add, undo, redo, copy,
  paste, duplicate, delete, and restyle; an inspector renames nodes and shows
  what reaches each input, or restyles and animates an edge; a log follows what
  happened.

### Breaking: shadows and glows

- A shadow's `blur` is the CSS blur radius: the box's edge is blurred by a
  Gaussian whose deviation is half of it, as browsers draw the Tailwind-style
  tokens the theme's shadows come from. Shadows used the blur as the
  deviation with a steeper falloff, so they were softer and shorter than
  their tokens. The renderer now draws the Gaussian blur of the rounded box,
  integrated across the box exactly, so a small box's shadow or glow is as
  faint as the blur makes it instead of solid under the box.
- `ShadowParams` gains `placement`: `Behind` its box, as before; `Outside`
  it only, leaving the box clear so a translucent fill does not show the
  shadow through it and a glow can be drawn over the box; or `Inside` it, as
  CSS `inset`, drawn over the box's fill. Added `ShadowParams::new`, `glow`,
  `inset`, and `with_placement`. Code that builds `ShadowParams` literally
  needs the new field.
- Inset theme shadows are painted: `ThemeShadowLayer::to_shadow_params` keeps
  `inset`, and `paint_theme_inset_shadow` draws the inset layers that
  `paint_theme_shadow` leaves out.
- Glows: `paint_theme_glow` draws a theme glow around a rounded rect, and
  `DefaultTheme::glow_for_output` resolves the `accent` or `secondary` glow
  (`GlowTone`) for an output. On HDR outputs, where the theme's HDR mode
  allows, the halo takes the tone's HDR color, as bright as an emissive
  indicator may be. `ThemeGlows` is exported.
- `Surface::shadow` casts a shadow chosen from the surface's theme in place of
  its elevation's, with inset layers over its fill, and `Surface::glow` adds a
  theme glow. `ShadowBox` casts shadows and a glow around a child that paints
  its own face.
- Enabled primary buttons and busy spinners glow in themes that have glows,
  which dark themes do; `Button::glow(false)` turns a button's glow off.
- Shadow offsets, blur, and spread, and the corner radii of rounded
  rectangles, scale with the current transform. Corners kept their radius in
  screen pixels, so zoomed or scaled content had corners too round or too
  sharp for its size.
- The JavaScript and Python `Shadow` classes take a `placement` of
  `"behind"`, `"outside"`, or `"inside"`.
- The widget book has a Shadows and glows story, and the HDR validation
  page's mode cards show a glowing live signal.

### Breaking: text painting helpers

- Replaced `paint_aligned_text` with `paint_text` and
  `paint_single_line_aligned_text` with `paint_text_line`. Both take a
  `TextAlign` instead of a 0-to-1 fraction and no longer take a line height,
  which only mattered when shaping failed. `paint_text` also takes a
  `TextPlacement` to put text at the top or bottom of its rect.
- `paint_text` places several lines as a block and aligns each line on its
  own. The old function centered the first line's baseline, so wrapped text
  hung below its rect, and centered lines were left-aligned as a group. A
  single line is placed exactly as before, and so is the first line of text
  that wraps in a rect with room for one line. A block taller than a larger
  rect starts at its top.
- The color picker's fields, menu rows, and slider values and the brush
  preview's description paint one line that never wraps. They wrapped, and
  showed only the first line, so a value too wide for its field could show
  as a shorter number.
- Added `Paragraph`, text laid out once to measure and then paint, with
  `VerticalAlign` for where it goes in its rect and `TextShaper` so the
  measure context, the paint context, and `LayoutContext` can all lay it out.
- Text laid out in a box with an infinite height now starts at the top and is
  sized to its lines. It was centered in the infinite box, so every glyph
  landed at infinity and nothing was drawn. An infinite width no longer wraps
  or aligns against infinity.
- `Label` places multi-line text with the shared paragraph placement, and
  table column alignments map to text alignment with
  `TableColumnAlignment::text_align`.

### Breaking: painting cannot request invalidations

- Removed `PaintCtx::request`, `request_paint`, `request_paint_rect`, and
  `invalidations`, and `ForeignPaintCtx::request_paint` and
  `request_paint_rect`. Requests made while painting only annotated the frame
  already being painted and were then dropped, so a widget asking to paint or
  measure again was never called. Request repaints from event handlers, ask
  for an animation frame from `measure` or `event`, or observe a signal while
  painting.

### Fixes

- A context menu's submenu covers the focus ring of the menu it opened from.
  Every panel's ring was drawn above all the panels, so the menu's ring ran
  across its submenu.
- The focus ring around an open context menu or popover floats with it.
  It was composited in page order, so content painted after the menu's
  trigger covered it: only the parts of the ring outside that content
  showed, as a border along one or two sides.
- Adaptive layouts no longer move focus when they are first laid out. A
  `ResponsiveSidebar`, `AdaptiveView`, `ConstraintView`, or `MasterDetail`
  restored focus into the pane its first layout showed, taking focus from
  nothing on a page that had just opened. `ResponsiveSidebar::on_mode_change`
  and `AdaptiveView::on_class_change` now report the first mode and class.
- A `FocusScope` restores focus only when focus was lost, not when it is on a
  widget outside the scope. Resizing across a breakpoint while typing in
  another field moved focus into the pane that appeared.
- Widgets that layout moves while an event is handled, before the next frame
  renders, are drawn where they went. A widget in a retained layer kept its
  old position until another event: the demo's open-demo button stayed where
  it was after closing a tab, beside the gap the closed tab left.
- A progress bar's value label now uses the tone's content color over the fill
  and body text over the track, so values below the midpoint stay readable.
- Disabled ghost buttons no longer reveal their transparent border as a dark
  outline on light themes.
- A selected filled icon button keeps its icon visible instead of painting it
  in the fill color.
- Color picker slider rows place the channel label and value beside the
  colored track instead of on it, so they stay readable and the marker no
  longer covers the value at the ends of the range.
- Linear gradients on rectangles and rounded rectangles now paint every stop
  in any direction, including hard stops and offsets that do not start at 0 or
  end at 1; the renderer previously blended only the first and last stops.
  Color picker tracks such as HSL lightness and OKLCH hue now render
  correctly.
- Gradient fills on rectangles and paths now move their gradient axis when a
  scene or retained layer is translated. Previously a scrolled layer left the
  axis behind, flattening the gradient to one end color.
- `Select` now handles the expand, collapse, and set-value accessibility
  actions it advertises.
- `Easing::CubicBezier` now solves the curve to within 1e-7 instead of about
  1e-3, so theme curves no longer step visibly when transitions are slowed
  down.
- The demo's Themes page sizes each row of preview cards to its tallest card,
  so wrapped descriptions no longer squash the color swatches.
- Demo widgets that ignored the live theme now follow it, including layout
  examples and editor toolbar separators, so nothing renders with light-theme
  colors in dark mode.
- Fixed a browser panic when opening the Editorial engine demo by using a
  WebAssembly-compatible monotonic clock for reflow timing. Added a browser
  regression check covering animation, controls, resizing, and tab switching.

## [0.3.0] - 2026-09-26

This release adds application and editor surfaces, improves text quality and
retained rendering, and expands the source-built language bindings.

### Highlights

- Added retained docking workspaces, editor cursor grabbing and raw mouse motion,
  safe initial desktop window placement, and GPU interoperability contracts.
- Expanded Python and JavaScript bindings for the newer Rust widget and editor
  surfaces. These bindings remain source-built and are not registry publications.
- Reused paragraph shaping and glyph measurements across layout widths, added
  size-only text measurement, and reduced repeated Flex layout probes.
- Retained widget output, scene fragments, renderer packets, GPU batches, and
  shared scene command storage to reduce repeated frame preparation work.
- Overlapped GPU preparation with CPU startup and added reproducible widget,
  frame, shrinkwrap-conversation, and editorial-flow benchmarks.
- Improved font hinting, transformed text rasterization, LCD coverage and atlas
  sampling; isolated node-edge animation repaint work and analytic grid dots.
- Added provider-neutral image icons, bounded browser tabs, compact dialogs,
  and reserved scrollbar gutters.
- Updated dependencies and bundled Lucide icons to 1.47.0.
- Made the Shrinkwrap and Editorial text demos follow shared theme colors,
  typography, and radii, including theme changes while paused and cached text
  layouts at unchanged window sizes.
- Added a size-focused web release profile and kept the demo within the existing
  12 MiB uncompressed Wasm budget without removing features.
- Restored Rust 1.90 compatibility, corrected offscreen demo-picker test
  interactions, and resolved strict workspace Clippy findings.
- Bundled complete Noto Sans Arabic and Hebrew fallback fonts for offline web
  and Android text, and corrected caret hit testing across right-to-left runs.
- Restored corrupted Unicode, emoji, and IME sample text in the demo and its
  text-editing benchmarks.
- Continued VSync presentation and animation/timer delivery inside native
  window move/resize loops, including while the title bar is held still.
- Preferred compatible Direct3D12 hardware on Windows to improve native window
  drag pacing, retaining explicit backend selection and automatic fallback.
- Kept focused, visible VSync windows presenting cached content so an idle UI
  does not lower a variable-refresh monitor's refresh rate.
- Reused GPU vertex buffers, upload staging storage, and output-transform
  resources to avoid repeated allocations and interaction-time frame stalls.
- Fixed inflated frame-time/FPS readings after idle mouse motion by attributing
  input costs and latency only to a pending redraw across native and test hosts.
- Made the demo FPS counter measure host frame cadence, including VSync and
  idle intervals, while reporting render work separately.
- Retained action cards independently so hover, press, and focus transitions
  repaint only the affected cards instead of rebuilding an entire picker grid.
- Centralized builder/runtime resource registration so rejected duplicate handles
  preserve the original resource, and shared native/web GPU resource setup.
- Shared button press, text-change, and caret lifecycle logic across controls;
  organized composites, binding infrastructure, and renderer internals by feature.
- Made binding signatures language-neutral with explicit numeric and identifier
  types, and changed export verification to follow Rust modules and declarations.
- Fixed selector observer write-back deadlocks and removed obsolete reactive
  dependencies after completed widget phases while preserving cached phases.
- Bounded text-layout and renderer path caches, preserving layouts and geometry
  still owned by live widgets or frames, and avoided full reindexing on collection
  appends.
- Matched node-graph spatial bounds to configured Bézier curvature so visible
  curves remain available for hit testing and culling.
- Added the optional `sinomo-ui-webview` crate for WRY-backed native child
  webviews with retained layout, lifecycle synchronization, thread-safe
  controls, typed page/title/IPC events, and application-owned browser policy.
- Prevented synchronous live-test flushes from advancing future animation
  frames based only on render wall time, avoiding timeouts in repeating demos.
- Added affine transformed widget subtrees and Canvas-hosted normal widgets.
  Canvas now uniformly scales paint, input, semantics, text, images, and nested
  layout by default, with screen-space/custom zoom policies and touch pinch;
  retained node widgets use the same path.
- Added the `sinomo-ui-nodes` graph-editor library with typed nodes, edges,
  handles, controlled and uncontrolled observable state, retained custom node
  widgets, dynamic measurement, incremental spatial indexing, subflows,
  resizing, edge reconnection, per-element semantics, lifecycle events,
  animated viewport and edge behavior, controls, minimap, and graph-owned
  appearance overrides.
- Added a comprehensive `sui-demo` node-editor workspace behind the
  default-enabled `nodes` feature, covering controlled and uncontrolled graphs,
  retained node controls, every edge family, subflows, editing, indexing,
  semantics, and viewport telemetry.

### Compatibility and release notes

- Update SUI crate dependencies together from `0.2` to `0.3`. Lucide keeps its
  independent `1.47.0` version and now depends on the `0.3` SUI family.
- `Event::RawMouseMotion` and `WindowEvent::Moved` require handling in exhaustive
  event matches. Public widget appearance/configuration structs gained fields,
  including `ControlPalette::selection_border`; update explicit struct literals.
- Rust 1.90 remains the minimum supported version.
- Linux webview builds require the system WebKitGTK 4.1 development libraries;
  the Rust bindings do not install the browser engine.
- Browser support remains alpha and Android remains experimental. Native
  webviews are an optional crate; Python wheels and Node addons are not part of
  this Rust registry release.

## [0.2.1]

This release refines interaction behavior and rendering consistency across
widgets, overlays, demos, and accessibility surfaces.

### Highlights

- Added configurable clipboard ownership and a shared editable-text controller
  for consistent text editing, selection, and copy behavior.
- Added recursive context-menu submenus with collision-aware cascade placement
  and corrected activation lifecycle handling.
- Improved list and tree row padding, typography, drop-target feedback,
  slider updates, status badges, and pixel-canvas brush defaults.
- Stabilized accessibility roots, empty-state actions, focus behavior, and
  synchronous event processing in the test harness.
- Kept nested retained layers synchronized with dialog and popover opacity and
  translation animations.
- Expanded the theme editor and motion demo while preserving scroll state and
  preventing focus-ring clipping.

## [0.2.0]

This release expands SUI from a retained widget and rendering foundation into
a fuller application toolkit while keeping the facade renderer-neutral.

### Highlights

- Added dependency-tracked `Signal<T>` bindings, observable selectors,
  keyed subtree reconciliation, automatic pass invalidation, retained local
  state, and rebuild diagnostics.
- Added the `VirtualCollection` foundation and keyed `VirtualList`, windowed
  collection models, variable-height extents, anchoring, follow-end behavior,
  selection, keyboard navigation, row retention, and virtual table state.
- Added `RichDocumentModel` and `RichDocumentView` with incremental streaming
  Markdown, selection across blocks, code actions and highlighting, links,
  images, attachments, extensible structured blocks, cached layouts, and rich
  accessibility semantics.
- Added typed widget, window, and application command routing, lifecycle-owned
  controllers and subscriptions, scheduler-only wakes, application multicast,
  and command/invalidation traces.
- Added window-managed overlay policy for dialogs, popovers, menus, tooltips,
  command palettes, notifications, drawers, and bottom sheets, including
  collision-aware placement, nesting, modality, dismissal, and focus restore.
- Added resizable split state, responsive sidebars, master-detail navigation,
  adaptive views, container queries, grid, intrinsic content extents, wrapping
  toolbars, aspect ratio, safe areas, and retained layout transitions.
- Added a live application inspector covering semantics and accessibility,
  stable widget IDs and bounds, event routes, rebuild and invalidation reasons,
  scheduler work, virtual collection statistics, and paint damage.
- Reduced widget-construction and retained text-layout overhead, refreshed
  dependencies, and made overlay scroll bars compact until pointer hover.
- Corrected Android window and `wgpu::Surface` creation to follow
  `Resumed`/`Suspended`, retaining the runtime while native surfaces are absent.

## [0.1.0]

Initial public alpha release of the Rust workspace.

### Highlights

- Added a retained-mode application runtime with explicit measure, arrange,
  event, paint, and accessibility passes.
- Added a renderer-neutral scene model and a retained `wgpu` renderer with
  text, image, vector path, clipping, compositing, and color-management
  support.
- Added the built-in widget library, responsive layout primitives, editable
  text controls, data views, overlays, drag and drop, canvas surfaces, and
  Mesh light, dark, high-contrast, OLED, and touch themes.
- Added desktop integration for Linux, macOS, and Windows, plus alpha browser
  support and experimental Android support behind explicit facade features.
- Added deterministic headless testing, semantic locators, screenshot and HDR
  artifact support, AccessKit integration, and an accessibility-tree generated
  terminal UI.
- Added runnable facade examples, the widget-book demo, bundled Lucide icon
  resources, AVIF/HDR helpers, and architecture and API guides.
- Added a language-neutral binding core and source-built native Python and
  Node/Electron bindings with a generated, coverage-checked widget surface.

### Release boundaries

- The Rust API is pre-release and may change before `1.0.0`.
- Browser support is alpha, Android support is experimental, and native HDR
  output is currently strongest on Windows.
- The Python and Node/Electron packages are not part of this registry release;
  their source remains available in the repository for local builds.
- Browser JavaScript bindings, prebuilt Python wheels, prebuilt Node/Electron
  addons, custom WGSL, and zero-copy external-surface composition are not yet
  published or supported release surfaces.

[0.1.0]: https://github.com/sinomo-lab/sui/releases/tag/v0.1.0
[0.2.0]: https://github.com/sinomo-lab/sui/compare/v0.1.0...v0.2.0
[0.2.1]: https://github.com/sinomo-lab/sui/compare/v0.2.0...v0.2.1
[0.3.0]: https://github.com/sinomo-lab/sui/compare/v0.2.1...v0.3.0
[Unreleased]: https://github.com/sinomo-lab/sui/compare/v0.3.0...HEAD
