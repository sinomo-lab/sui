# Widget API Conventions Migration

The [widget API conventions](../api/conventions.md) describe how built-in
widgets are configured. This plan moves the existing widgets onto them in
phases. Each phase keeps the workspace building, deprecates old names where
Rust allows it so existing code keeps compiling with a warning, and records
breaking changes in the changelog. Deprecated names are removed one minor
release after their replacement ships.

A change that can't be deprecated, such as a callback's argument order, is made
in place and listed in the changelog's breaking section.

## Phase 1: Correctness and the Most Visible Differences

Status: done.

- `Label::theme` supplies defaults instead of clearing the color, font size,
  and line height set before it. `Label` gains `theme_when`, and
  `style`/`style_when` become `text_style`/`text_style_when`.
- Every `_with_ctx` callback receives the `EventCtx` first. `ListView`,
  `VirtualList`, `BrowserTabBar`, and `SegmentedControl` passed it last.
- Two-state inputs share one vocabulary: `Switch` gains `checked`,
  `checked_when`, `set_checked`, and `is_checked`, `RadioButton` gains
  `checked`, `set_checked`, and `is_checked`, and all three two-state inputs
  report `on_change(bool)` with an `on_change_with_ctx` twin. A radio button
  reports only actual changes. `Switch::on`, `on_when`, `set_on`, `is_on`,
  `RadioButton::selected`, `set_selected`, `is_selected`, `on_select`, and
  `on_toggle` are deprecated.

## Phase 2: Selection and Availability

Status: done.

- Two-state inputs gain `checked_from`, and `RadioButton` gains
  `checked_when`.
- `selected` takes `impl Into<Option<usize>>` on every collection, and
  `selected_index()` returns `Option<usize>` everywhere. Widgets that always
  show one choice (`TabBar`, `Tabs`, `SegmentedControl`) treat `None` as their
  first item. `TreeView` gains `selected_path` for nested rows.
  `SwitchView` and `Breadcrumb::current` keep their words, which mean
  something different; the conventions say why.
- The inputs take `enabled`, `enabled_when`, and `enabled_from`: `Checkbox`,
  `Switch`, `RadioButton`, `Slider`, `NumberInput`, `Select`, `TextInput`,
  `TextArea`, `PasswordInput`, and `DateTimeInput`. Disabled, they ignore
  input, can't take focus, paint with `DefaultTheme::for_disabled_control()`,
  and report themselves disabled. `Button` and `IconButton` hold their enabled
  state the same way and expose `is_enabled()`.
- Items take `enabled(bool)` in place of the deprecated `disabled()`, and
  inputs take `read_only(bool)` in place of `read_only()`.

## Phase 2b: Availability Beyond the Inputs

Status: done.

- The other interactive widgets take `enabled`, `enabled_when`, and
  `enabled_from`: `RadioGroup`, `SegmentedControl`, `TabBar`, `Tabs`,
  `ListView`, `Table`, `TreeView`, `LayerList`, `ColorPicker`,
  `ColorPalette`, `ToolPalette`, `PresetStrip`, and `Menu`.

## Phase 3: Events Everywhere

Status: done.

- Every `on_<event>` gains its `_with_ctx` twin. Missing today on `RadioGroup`,
  `NumberInput`, `TreeView`, `Table`, `VirtualTable`, `Breadcrumb`, `TabBar`,
  `Tabs`, `PresetStrip`, `SplitView`, `TextSurface`, `Popover`, `ColorPicker`,
  and the `on_dismiss` callbacks.
- `LayerList::on_select` becomes `on_change`. `VirtualTable` reports selection
  through `on_change`.
- `TextInput` gains `on_submit`, and `PasswordInput` and `DateTimeInput` gain
  the `on_focus_change`, `text_style`, and `appearance` that `TextInput` has.

## Phase 4: Overlays and Themes

Status: done. `FloatingStack::theme` never had an effect, so it is deprecated
instead of gaining `theme_when`.

- Overlays share `open`, `open_when`, `open_from`, and `on_open_change`.
  `Dialog::shown` and `Select::expanded` become `open`.
- `theme_when` is added where it is missing: `Icon`, `Tooltip`, `Popover`,
  `CommandPalette`, `NotificationHost`, and `FloatingStack`.
- `Dialog::max_width` and `Dialog::primary_action` resolve theme values when
  the dialog lays out, so they no longer depend on whether `theme` came
  first.
- Widgets that bind values through `dynamic(...)` constructors or
  `new_observable` move to `_when` and `_from` builders: `Label::dynamic`,
  `StatusBadge::dynamic`, `PlacementBadge::dynamic`, and
  `RebuildOnChange::new_observable`.

## Phase 5: Names, Layout Words, Children, and Value Types

Status: done, with these decisions:

- Every container's `child()` getter became `child_pod()`, since `child` is
  now a builder. `spacing` became `gap`, and `radius` became `corner_radius`,
  on every widget and paint helper that had them, not only those listed.
- All six value types are plain data: their builders store what they are
  given, they implement `Default`, and the widgets clamp at use.
  `TransientNotification` could not be encapsulated without getters that
  clash with its `duration` and `urgency` builders.
- `Spinner::label` keeps reporting the visible label as the spinner's
  description, so assistive technology reads what is shown; the spinner's
  name still comes from `new`.

Follow-up, also done: `Flex` reads each child's layout settings with
`flex_item`, `flex_items`, `flex_items_mut`, and `set_flex_item`. `Icon` and
`Image` take their accessible name through `semantic_name`, since they show
no text. Item types keep reading their labels with `label()`: the convention
now says data types read their own fields under the field's name.

- Getters stop sharing names with builders: `Button::label`,
  `TextInput::name`, `ColorPicker::color`, `FloatingWorkspace::state`, and
  `DockWorkspace::state`. `Link::url` becomes a builder on `Link::new`.
- Layout words: `spacing` becomes `gap` on `Stack`, `VirtualList`,
  `FieldGroup`, and `Toolbar` (`line_spacing` becomes `cross_gap`).
  `Surface::radius` and `FormSection::radius` become `corner_radius`.
  `ColorPicker`, `Canvas`, and `PixelCanvas` take their color overrides as
  `colors(...)`.
- Children: single-child widgets that use `with_child` (`SizedBox`,
  `ListItem`) take `child`, `TreeItem` nodes are added with `item`, and
  `Canvas::content` becomes `child`. Duplicate builders such as
  `ListItem::subtitle` and `activate_with_content` are deprecated.
- Value types choose between plain data and encapsulation: `CanvasViewport`,
  `CanvasSurface`, `FloatingViewConfig`, `TransientNotification`,
  `BrushPreviewSpec`, and `CoverageDotsConfig` have public fields and
  validating builders today.
- Names and labels: `RadioButton` gains `semantic_name`, and `Spinner::label`
  sets only visible text.

## Phase 6: Flags Take a Bool

Status: done.

- Single on/off settings take a `bool` in place: `fill_width` on `Surface`,
  `FieldGroup`, `FormSection`, and `FramedField`; `Surface::fill_height`;
  `Padding::fill_child_width` and `fill_child_height`; `Label::single_line`;
  `MenuItem::destructive` and `separator_before`; `activate_with_child` on
  `ListItem` and `TreeItem`; `appear` on `Presence` and `KeyedStack`;
  `PixelCanvas::fit_on_first_layout`; and `TextCellPaint::numeric`.
- `Image::without_border()` becomes `show_border(bool)`, and
  `PanelSection::collapsed()` is deprecated in favor of `expanded(false)`.
- Presets that set several properties keep taking no argument, such as
  `show_inline()`, `Surface::fill()`, and `TableColumn::numeric()`, which
  also right-aligns the column. `ScrollView::retain_content_layer()`,
  `Motion::movement()`, `SingleChild::with_paint_boundary()`, and
  `RegisteredImage::without_mipmaps()` stay as they are for now.
