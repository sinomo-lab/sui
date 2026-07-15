export type BindingValue = string | number | boolean;
export type PointerAction =
  | "down"
  | "up"
  | "move"
  | "scroll"
  | "enter"
  | "leave"
  | "cancel";
export type KeyState = "pressed" | "released";
export type InteropTier = "cpuUpload" | "sharedTexture" | "sharedRenderTarget";
export type ImageFit = "fill" | "contain" | "cover" | "none";
export type Axis = "horizontal" | "vertical";
export type ScrollAxes = Axis | "both";
export type SurfaceRole = "window" | "sidebar" | "panel" | "titlebar" | "field";
export type SurfaceBorder = "none" | "all" | "top" | "right" | "bottom" | "left";
export type SurfaceElevation = "none" | "small" | "medium" | "large";
export type SemanticTone =
  | "neutral"
  | "accent"
  | "info"
  | "success"
  | "warning"
  | "danger";
export type IconGlyph =
  | "add"
  | "remove"
  | "check"
  | "chevron-down"
  | "chevron-up"
  | "chevron-left"
  | "chevron-right"
  | "close"
  | "maximize"
  | "restore"
  | "fit-view"
  | "actual-size"
  | "more-horizontal"
  | "more-vertical"
  | "search"
  | "undo"
  | "redo"
  | "brush"
  | "eraser"
  | "paint-bucket"
  | "hand"
  | "lock"
  | "unlock"
  | "trash"
  | "download"
  | "sparkles"
  | "chat"
  | "history"
  | "folder"
  | "file"
  | "link"
  | "send"
  | "alert"
  | "storage"
  | "audio-lines"
  | "mic"
  | "mic-off"
  | "camera"
  | "camera-off"
  | "video"
  | "video-off"
  | "phone"
  | "phone-off"
  | "monitor"
  | "screen-share";
export type NativeBackend =
  | "cpu"
  | "wgpu"
  | "webgpu"
  | "d3d12"
  | "metal"
  | "vulkan"
  | "opengl"
  | "unknown";
export type FontStyle = "normal" | "italic" | "oblique";
export type FontStretch =
  | "ultraCondensed"
  | "extraCondensed"
  | "condensed"
  | "semiCondensed"
  | "normal"
  | "semiExpanded"
  | "expanded"
  | "extraExpanded"
  | "ultraExpanded";
export type SemanticsRole =
  | "window"
  | "root"
  | "genericContainer"
  | "separator"
  | "list"
  | "listItem"
  | "tree"
  | "table"
  | "splitter"
  | "breadcrumb"
  | "tabBar"
  | "tabs"
  | "button"
  | "link"
  | "checkbox"
  | "switch"
  | "radioButton"
  | "radioGroup"
  | "menu"
  | "menuItem"
  | "contextMenu"
  | "tooltip"
  | "dialog"
  | "popover"
  | "slider"
  | "progressBar"
  | "busyIndicator"
  | "text"
  | "textInput"
  | "spinBox"
  | "comboBox"
  | "image"
  | "colorSwatch"
  | "colorPicker"
  | "canvas"
  | "scrollView";
export type ToggleState = "checked" | "unchecked" | "mixed";

export class Point {
  constructor(x: number, y: number);
  x: number;
  y: number;
}

export class Modifiers {
  constructor(shift?: boolean, control?: boolean, alt?: boolean, meta?: boolean);
  shift: boolean;
  control: boolean;
  alt: boolean;
  meta: boolean;
}

export class Event {
  static pointer(
    kind: PointerAction,
    position: Point,
    pointerId?: string,
    delta?: Point,
    button?: string,
    buttons?: number,
    pointerKind?: "mouse" | "touch" | "pen" | "unknown",
    isPrimary?: boolean,
    modifiers?: Modifiers
  ): Event;
  static scroll(
    position: Point,
    delta: Point,
    mode?: "pixels" | "lines",
    pointerId?: string,
    modifiers?: Modifiers
  ): Event;
  static keyboard(
    key: string,
    state?: KeyState,
    code?: string,
    text?: string,
    repeat?: boolean,
    isComposing?: boolean,
    modifiers?: Modifiers
  ): Event;
  static ime(kind: string, text?: string, cursorStart?: number, cursorEnd?: number): Event;
  static window(
    kind: string,
    value?: boolean,
    size?: Size,
    scaleFactor?: number,
    rawDpi?: number,
    suggestedSize?: Size
  ): Event;
  static custom(kind: string, payload?: string): Event;
  readonly kind: string;
  readonly action?: string;
  readonly pointerId?: string;
  readonly position?: Point;
  readonly delta?: Point;
  readonly scrollMode?: string;
  readonly button?: string;
  readonly buttons?: number;
  readonly modifiers?: Modifiers;
  readonly deviceKind?: string;
  readonly isPrimary?: boolean;
  readonly key?: string;
  readonly code?: string;
  readonly text?: string;
  readonly state?: string;
  readonly repeat?: boolean;
  readonly isComposing?: boolean;
  readonly customKind?: string;
  readonly payload?: string;
  readonly cursorStart?: number;
  readonly cursorEnd?: number;
  readonly value?: boolean;
  readonly size?: Size;
  readonly scaleFactor?: number;
  readonly rawDpi?: number;
  readonly suggestedSize?: Size;
  readonly filePath?: string;
}

export class Size {
  constructor(width: number, height: number);
  width: number;
  height: number;
}

export class Rect {
  constructor(x: number, y: number, width: number, height: number);
  x: number;
  y: number;
  width: number;
  height: number;
  readonly origin: Point;
  readonly size: Size;
}

export class Path {
  constructor();
  static rect(rect: Rect): Path;
  static circle(center: Point, radius: number): Path;
  static roundedRect(rect: Rect, radius: number): Path;
  static arc(center: Point, radius: number, startAngle: number, sweepAngle: number): Path;
  readonly bounds: Rect;
  readonly elementCount: number;
  isEmpty(): boolean;
}

export class PathBuilder {
  constructor();
  moveTo(point: Point): void;
  lineTo(point: Point): void;
  quadTo(ctrl: Point, to: Point): void;
  cubicTo(ctrl1: Point, ctrl2: Point, to: Point): void;
  close(): void;
  pushRect(rect: Rect): void;
  pushCircle(center: Point, radius: number): void;
  pushRoundedRect(rect: Rect, radius: number): void;
  pushArc(center: Point, radius: number, startAngle: number, sweepAngle: number): void;
  build(): Path;
}

export class Transform {
  constructor(xx: number, yx: number, xy: number, yy: number, dx: number, dy: number);
  static identity(): Transform;
  static translation(x: number, y: number): Transform;
  static scale(x: number, y: number): Transform;
  static rotation(radians: number): Transform;
  then(next: Transform): Transform;
  xx: number;
  yx: number;
  xy: number;
  yy: number;
  dx: number;
  dy: number;
}

export class Color {
  constructor(red: number, green: number, blue: number, alpha?: number);
  red: number;
  green: number;
  blue: number;
  alpha: number;
}

export class Shadow {
  constructor(offsetX: number, offsetY: number, blur: number, spread: number, color: Color);
  readonly offsetX: number;
  readonly offsetY: number;
  readonly blur: number;
  readonly spread: number;
  readonly color: Color;
}

export class Constraints {
  constructor(min: Size, max: Size);
  readonly min: Size;
  readonly max: Size;
  clamp(size: Size): Size;
  loosen(): Constraints;
}

export class Paint {
  readonly bounds: Rect;
  readonly commandCount: number;
  clear(color: Color): void;
  fillRect(rect: Rect, color: Color): void;
  strokeRect(rect: Rect, color: Color, width?: number): void;
  fillPath(path: Path, color: Color): void;
  strokePath(path: Path, color: Color, width?: number): void;
  fillRoundedRect(rect: Rect, color: Color, radius?: number): void;
  drawShadow(rect: Rect, shadow: Shadow, radius?: number): void;
  fillRoundedRectWithShadow(rect: Rect, color: Color, shadow: Shadow, radius?: number): void;
  fillBounds(color: Color): void;
  drawText(
    rect: Rect,
    text: string,
    color?: Color,
    fontSize?: number,
    lineHeight?: number,
    font?: FontHandle,
    weight?: number,
    style?: FontStyle,
    stretch?: FontStretch
  ): void;
  drawShaderRect(rect: Rect, shader: Shader): void;
  rgbaImage(slot: number, width: number, height: number, pixels: Uint8Array): ImageHandle;
  drawImage(rect: Rect, image: ImageHandle): void;
  drawImageQuad(points: [Point, Point, Point, Point], image: ImageHandle): void;
  pushClipRect(rect: Rect): void;
  pushClipPath(path: Path): void;
  popClip(): void;
  pushTransform(transform: Transform): void;
  popTransform(): void;
}

export class Semantics {
  readonly bounds: Rect;
  readonly focused: boolean;
  readonly childCount: number;
  node(
    role?: SemanticsRole,
    name?: string,
    value?: string | number | boolean,
    description?: string,
    bounds?: Rect,
    disabled?: boolean,
    checked?: ToggleState,
    selected?: boolean,
    expanded?: boolean,
    busy?: boolean,
    minValue?: number,
    maxValue?: number
  ): void;
  child(index: number): boolean;
}

export class FontHandle {
  constructor(id: string);
  readonly id: string;
}






export class ImageHandle {
  constructor(id: string);
  static local(slot: number): ImageHandle;
  readonly id: string;
  readonly localSlot?: number;
}

export class Shader {
  static colorWheel(): Shader;
  static hueBar(): Shader;
  static saturationValuePlane(hue: number, maxValue?: number, colorSpace?: string): Shader;
  static saturationBar(hue: number, value: number, colorSpace?: string): Shader;
  static valueBar(hue: number, saturation: number, maxValue?: number, colorSpace?: string): Shader;
  static alphaBar(color: Color): Shader;
  static rgbChannelBar(color: Color, channel: number, maxValue?: number): Shader;
}

export interface WidgetCallbacks {
  name?: string;
  measure?(constraints: Constraints): Size;
  event?(event: Event): boolean | void;
  paint?(paint: Paint): void;
  semantics?(semantics: Semantics): void;
}

export class Widget {
  constructor(callbacks: WidgetCallbacks);
}

export class State {
  constructor(value: BindingValue);
  get(): BindingValue;
  set(value: BindingValue): void;
  readonly text: string;
}

export class Window {
  constructor(title: string);
  root(widget: Widget): void;
}

export class App {
  constructor();
  window(window: Window): void;
  render(index?: number): RenderSnapshot;
  start(): RunningApp;
  rgbaImage(width: number, height: number, pixels: Uint8Array): ImageHandle;
  pngImage(png: Uint8Array): ImageHandle;
  pngFile(path: string): ImageHandle;
  svgImage(svg: Uint8Array): ImageHandle;
  svgFile(path: string): ImageHandle;
  svgImageAtSize(width: number, height: number, svg: Uint8Array): ImageHandle;
  svgFileAtSize(width: number, height: number, path: string): ImageHandle;
  fontBytes(data: Uint8Array): FontHandle;
  fontFile(path: string): FontHandle;
  run(): void;
  runWithHandle(callback: (ui: UiHandle) => void): void;
  readonly windowCount: number;
  readonly fontResourceCount: number;
  readonly imageResourceCount: number;
}

export class WindowHandle {
  constructor(id: string);
  readonly id: string;
}

export class UiHandle {
  post(callback: () => void): void;
  readonly pendingCount: number;
}

export class RunningApp {
  uiHandle(): UiHandle;
  drain(): number;
  render(index?: number): RenderSnapshot;
  renderWindow(window: WindowHandle): RenderSnapshot;
  needsRender(index?: number): boolean;
  requestRedraw(index?: number): void;
  handleEvent(event: Event, index?: number): void;
  readonly windowCount: number;
  windowId(index: number): WindowHandle;
  windowIds(): string[];
  readonly pendingCount: number;
}

export class RendererInteropCapabilities {
  constructor(
    backend: NativeBackend,
    cpuUpload?: boolean,
    sharedTexture?: boolean,
    sharedRenderTarget?: boolean
  );
  static cpuOnly(backend: NativeBackend): RendererInteropCapabilities;
  supports(tier: InteropTier): boolean;
  readonly backend: NativeBackend;
  readonly cpuUpload: boolean;
  readonly sharedTexture: boolean;
  readonly sharedRenderTarget: boolean;
}

export class ExternalBackendHandle {
  constructor(id: string);
  readonly id: string;
  readonly isEmpty: boolean;
}

export class ExternalSync {
  static none(): ExternalSync;
  static generation(generation: string): ExternalSync;
  static timelineValue(handle: ExternalBackendHandle, value: string): ExternalSync;
  static fence(handle: ExternalBackendHandle): ExternalSync;
  readonly kind: string;
  readonly value?: string;
}

export class ExternalTextureDescriptor {
  static cpuRgba8(size: Size, pixels: Uint8Array, generation?: string): ExternalTextureDescriptor;
  static sharedTexture(
    backend: NativeBackend,
    size: Size,
    format: string,
    handle: ExternalBackendHandle,
    sync: ExternalSync,
    colorSpace?: string
  ): ExternalTextureDescriptor;
  static sharedRenderTarget(
    backend: NativeBackend,
    size: Size,
    format: string,
    handle: ExternalBackendHandle,
    sync: ExternalSync,
    colorSpace?: string
  ): ExternalTextureDescriptor;
  validate(): void;
  readonly tier: InteropTier;
  readonly size: Size;
}

export class UiTaskQueue {
  constructor();
  post(callback: () => void): void;
  drain(): number;
  readonly pendingCount: number;
}

export class RenderSnapshot {
  commandCount: number;
  semanticsCount: number;
  semanticsRoles: string[];
  semanticsNames: string[];
  semanticsValues: string[];
  semanticsDescriptions: string[];
  semanticsChecked: string[];
  semanticsBusy: boolean[];
  semanticsEditableMultiline: boolean[];
  semanticsDisabled: boolean[];
  semanticsFocused: boolean[];
  semanticsHidden: boolean[];
  semanticsHovered: boolean[];
  semanticsSelected: boolean[];
  semanticsExpanded: string[];
  fillRectCount: number;
  drawImageCount: number;
  registeredFontCount: number;
  registeredImageCount: number;
}

export function renderWidget(widget: Widget, event?: Event): RenderSnapshot;

// BEGIN GENERATED SUI WIDGET BINDINGS
// Generated by `cargo xtask bindings generate` from bindings/widgets.sui.
// Do not edit this section by hand.

export class TextSpan {
  constructor(text: string, color?: Color, fontSize?: number, lineHeight?: number, font?: FontHandle, weight?: number, style?: FontStyle, stretch?: FontStretch);
  readonly text: string;
}

export class StatusBarSegment {
  constructor(text: State | BindingValue, tone?: SemanticTone | string, minWidth?: number, expand?: boolean);
}

export class SegmentedControlItem {
  constructor(label: string, semanticName?: string, description?: string, disabled?: boolean);
}

export class TableColumn {
  constructor(title: string, width?: number, minWidth?: number, alignment?: "start" | "center" | "end" | "left" | "right", numeric?: boolean);
}

export class TableRow {
  constructor(cells: string[]);
}

export function Label(value: State | BindingValue): Widget;

export function Button(label: State | BindingValue, onPress?: () => void): Widget;

export function Icon(glyph: IconGlyph | string, label?: string, size?: number, color?: Color): Widget;

export function IconButton(glyph: IconGlyph | string, label: State | BindingValue, selected?: State | boolean | number, enabled?: State | boolean | number, size?: number, iconSize?: number, description?: string, onPress?: () => void): Widget;

export function Link(label: State | BindingValue, url: State | BindingValue, semanticName?: string, enabled?: State | boolean | number, onOpen?: (url: string) => void): Widget;

export function Checkbox(label: State | BindingValue, checked?: State | boolean | number, onToggle?: (checked: boolean) => void): Widget;

export function Switch(label: State | BindingValue, on?: State | boolean | number, onToggle?: (on: boolean) => void): Widget;

export function RadioButton(label: State | BindingValue, selected?: State | boolean | number, onSelect?: () => void): Widget;

export function RadioGroup(name: State | BindingValue, options: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function SegmentedControl(name: State | BindingValue, items: SegmentedControlItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function Breadcrumb(name: State | BindingValue, items: string[], current?: State | number | boolean, onActivate?: (index: number, value: string) => void): Widget;

export function PathBar(name: State | BindingValue, items: string[], current?: State | number | boolean, onActivate?: (index: number, value: string) => void): Widget;

export function ListView(name: State | BindingValue, items: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function Table(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function DataGrid(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function Slider(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, step?: number, onChange?: (value: number) => void): Widget;

export function NumberInput(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, step?: number, precision?: number, onChange?: (value: number) => void): Widget;

export function Select(name: State | BindingValue, options: string[], selected?: State | number | boolean, placeholder?: string, onChange?: (index: number, value: string) => void): Widget;

export function ProgressBar(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, showValue?: boolean): Widget;

export function SignalMeter(name: State | BindingValue, active?: State | boolean | number, description?: string, bars?: number, size?: Size): Widget;

export function StatusBadge(label: State | BindingValue, tone?: SemanticTone | string, icon?: IconGlyph | string, minWidth?: number): Widget;

export function StatusBar(segments: StatusBarSegment[], name?: string, description?: State | BindingValue, height?: number): Widget;

export function DetailRow(label: State | BindingValue, value: State | BindingValue, maxValueLines?: number): Widget;

export function BusyIndicator(name: State | BindingValue, label?: State | BindingValue, size?: number): Widget;

export function TextInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export function TextArea(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export function RichText(spans: TextSpan[], semanticName?: string, minWidth?: number, minHeight?: number): Widget;

export function Image(image: ImageHandle, label?: string, fit?: ImageFit, size?: Size): Widget;

export function ColorSwatch(name: string, color: Color, size?: Size, readOnly?: boolean, onPress?: () => void): Widget;

export function Separator(axis?: Axis, name?: string, inset?: number, thickness?: number, length?: number): Widget;

export function EmptyState(title: string, description: string, name?: string, detail?: string, icon?: IconGlyph | string, action?: Widget, background?: Color, transparent?: boolean): Widget;

export function Surface(child: Widget, role?: SurfaceRole | string, name?: string, border?: SurfaceBorder | string, elevation?: SurfaceElevation | string, radius?: number, padding?: number, fillWidth?: boolean, fillHeight?: boolean): Widget;

export function Toolbar(children: Widget[], axis?: Axis, name?: string, extent?: number, padding?: number, spacing?: number, background?: Color, divider?: boolean): Widget;

export function Column(children: Widget[], gap?: number): Widget;

export function Row(children: Widget[], gap?: number): Widget;

export function ScrollView(child: Widget, axes?: ScrollAxes, name?: string): Widget;

export function ExternalSurface(texture: ExternalTextureDescriptor, desiredSize?: Size, name?: string): Widget;

export class TreeItem {
  constructor(label: string, detail?: string, expanded?: boolean, disabled?: boolean, children?: TreeItem[]);
}

export function TreeView(name: State | BindingValue, items: TreeItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export class LayerListItem {
  constructor(label: string, detail?: string, visible?: boolean, locked?: boolean, disabled?: boolean);
}

export function LayerList(name: State | BindingValue, items: LayerListItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export class MenuItem {
  constructor(label: string, shortcut?: string, disabled?: boolean, destructive?: boolean, separatorBefore?: boolean);
}

export class ToolPaletteItem {
  constructor(icon: IconGlyph | string, label: string, disabled?: boolean);
}

export class ColorPaletteSwatch {
  constructor(name: string, color: Color);
  readonly name: string;
  readonly color: Color;
}

export function Menu(name: State | BindingValue, items: MenuItem[], highlighted?: State | number | boolean, onActivate?: (index: number, value: string) => void): Widget;

export function ContextMenu(name: string, trigger: Widget, items: MenuItem[], onActivate?: (index: number, value: string) => void): Widget;

export function TabBar(name: State | BindingValue, tabs: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export function Tabs(name: State | BindingValue, tabs: string[], selected?: State | number | boolean): Widget;

export function Dialog(title: State | BindingValue, content: Widget, shown?: State | boolean | number): Widget;

export function Padding(child: Widget, padding?: number, top?: number, right?: number, bottom?: number, fillChildWidth?: boolean, fillChildHeight?: boolean): Widget;

export function Align(child: Widget, horizontal?: "start" | "center" | "end" | "stretch", vertical?: "start" | "center" | "end" | "stretch"): Widget;

export function Background(child: Widget, color: Color): Widget;

export function SizedBox(child?: Widget, width?: number, height?: number): Widget;

export function Stack(children: Widget[], axis?: Axis, spacing?: number, alignment?: "start" | "center" | "end" | "stretch"): Widget;

export function SemanticRegion(name: State | BindingValue, child: Widget, description?: State | BindingValue, role?: string): Widget;

export function FormRow(label: string, control: Widget, stacked?: boolean, labelWidth?: number, controlWidth?: number, gap?: number): Widget;

export function FieldGroup(children: Widget[], spacing?: number, padding?: number, maxWidth?: number, fillWidth?: boolean): Widget;

export function FormSection(title: string, child: Widget, description?: string, headerAction?: Widget, padding?: number, bodyGap?: number, headerGap?: number, maxWidth?: number, fillWidth?: boolean, radius?: number, elevation?: SurfaceElevation | string): Widget;

export function PanelSection(title: string, child: Widget, headerAction?: Widget, gap?: number, actionGap?: number, collapsible?: boolean, expanded?: boolean): Widget;

export function DockPanel(title: string, child: Widget, name?: string, headerHeight?: number, padding?: number, background?: Color, headerBackground?: Color): Widget;

export function StatusBarHost(content: Widget, statusBar: Widget): Widget;

export function Tooltip(text: string, child: Widget, placement?: "above" | "below"): Widget;

export function Popover(name: string, trigger: Widget, content: Widget, open?: boolean): Widget;

export function ToolPalette(name: string, items: ToolPaletteItem[], selected?: State | number | boolean, axis?: Axis, onChange?: (index: number, value: string) => void, extent?: number, padding?: number, spacing?: number, itemSize?: number, iconSize?: number, background?: Color, divider?: boolean): Widget;

export function PresetStrip(name: string, presets: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void, itemWidth?: number, itemHeight?: number, gap?: number): Widget;

export function BrowserTabBar(name: string, tabs: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void, onClose?: (index: number, value: string) => void): Widget;

export function ColorPalette(name: string, swatches: ColorPaletteSwatch[], selected?: State | number | boolean, onChange?: (index: number, name: string, color: Color) => void, columns?: number, swatchSize?: number, gap?: number): Widget;

export function ColorPicker(name: string, color?: Color, onChange?: (color: Color) => void, showAlpha?: boolean, compact?: boolean): Widget;

export class BrushPreviewSpec {
  constructor(color: Color, size?: number, opacity?: number, shape?: "round" | "square");
  readonly color: Color;
  readonly size: number;
  readonly opacity: number;
  readonly shape: "round" | "square";
}

export function PasswordInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export function DateTimeInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export function ActionCard(title: string, description: string, icon?: IconGlyph | string, tone?: SemanticTone | string, enabled?: State | boolean | number, onPress?: () => void): Widget;

export function BrushPreview(name: string, spec: BrushPreviewSpec, kind?: string, size?: Size): Widget;

export function CommandGroup(name: string, children: Widget[], axis?: Axis, padding?: number, spacing?: number, cornerRadius?: number, background?: Color, border?: Color): Widget;

export function CoverageDots(name: string, current: number, target: number, tone?: SemanticTone | string, maxDots?: number, showLabel?: boolean, minWidth?: number): Widget;

export function Dock(body: Widget, top?: Widget, topHeight?: number, bottom?: Widget, bottomHeight?: number, fallbackWidth?: number, fallbackBodyHeight?: number): Widget;

export function FixedPaneSplit(first: Widget, divider: Widget, second: Widget, axis?: Axis, fixedPane?: "first" | "second", fixedExtent?: number, dividerExtent?: number, fallbackFlexibleExtent?: number): Widget;

export function FramedField(child: Widget, name?: string, description?: string, padding?: number, minHeight?: number, fillWidth?: boolean, focused?: State | boolean | number, invalid?: State | boolean | number): Widget;

export function MeasuredBottomDock(body: Widget, bottom: Widget, fallbackSize?: Size): Widget;

export function PlacementBadge(label: State | BindingValue, icon?: IconGlyph | string, tone?: SemanticTone | string, current?: number, target?: number, minWidth?: number): Widget;

export function PropertyRow(label: string, control: Widget, stacked?: boolean, labelWidth?: number, controlWidth?: number, gap?: number): Widget;

export function SectionLabel(label: string, semanticName?: string, color?: Color): Widget;

export function SideSheet(title: string, body: Widget, description?: string, shown?: State | boolean | number, modal?: boolean, dismissOnScrim?: boolean, placement?: "left" | "right", width?: number, headerAction?: Widget, actions?: Widget[], onDismiss?: () => void): Widget;

export function SplitView(first: Widget, second: Widget, axis?: Axis, name?: string, ratio?: State | number | boolean, minFirst?: number, minSecond?: number, dividerThickness?: number, onChange?: (ratio: number) => void): Widget;

export function SwitchView(children: Widget[], selected?: State | number | boolean): Widget;

export function TrailingSlotRow(body: Widget, trailing: Widget, trailingWidth?: number, trailingHeight?: number, gap?: number): Widget;

export class FloatingStackWindow {
  constructor(bounds: Rect, child: Widget);
}

export function FloatingStack(windows: FloatingStackWindow[], name?: string): Widget;

export function VirtualScrollView(children: Widget[], name?: string, padding?: number, spacing?: number): Widget;

export function ReorderableList(name: string, children: Widget[], spacing?: number, dragThreshold?: number, previewLabel?: string, onReorder?: (item: number, fromIndex: number, toIndex: number) => void): Widget;

// END GENERATED SUI WIDGET BINDINGS
