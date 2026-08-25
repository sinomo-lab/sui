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

export interface RichAttachmentOptions {
  mediaType?: string;
  source?: string;
  sizeBytes?: string;
  description?: string;
}

export interface RichExtensionOptions {
  summary?: string;
  body?: string;
  status?: "neutral" | "pending" | "running" | "success" | "warning" | "error";
  initiallyExpanded?: boolean;
  metadata?: Record<string, string>;
}

export interface NotificationOptions {
  duration?: number;
  persistent?: boolean;
  urgency?: "polite" | "assertive";
}

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
  static rawMouseMotion(delta: Point, modifiers?: Modifiers): Event;
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
    suggestedSize?: Size,
    position?: Point
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

export class EventContext {
  readonly windowId: string;
  readonly widgetId: string;
  readonly bounds: Rect;
  readonly currentTime: number;
  readonly phase: "capture" | "target" | "bubble";
  readonly focused: boolean;
  readonly clipboardText?: string;
  setHandled(): void;
  requestFocus(): void;
  clearFocus(): void;
  requestMeasure(): void;
  requestArrange(): void;
  requestPaint(rect?: Rect): void;
  requestSemantics(): void;
  requestAnimationFrame(): void;
  capturePointer(pointerId: string): void;
  releasePointer(pointerId: string): void;
  setClipboardText(text: string): void;
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

export type AnimationValueKind =
  | "scalar"
  | "point"
  | "vector"
  | "size"
  | "rect"
  | "color"
  | "transform";

export class AnimationValue {
  static scalar(value: number): AnimationValue;
  static point(value: Point): AnimationValue;
  static vector(value: Point): AnimationValue;
  static size(value: Size): AnimationValue;
  static rect(value: Rect): AnimationValue;
  static color(value: Color): AnimationValue;
  static transform(value: Transform): AnimationValue;
  readonly kind: AnimationValueKind;
  readonly scalarValue?: number;
  readonly pointValue?: Point;
  readonly vectorValue?: Point;
  readonly sizeValue?: Size;
  readonly rectValue?: Rect;
  readonly colorValue?: Color;
  readonly transformValue?: Transform;
}

export interface TransitionOptions {
  startTime?: number;
  easing?: string;
}

export class Transition {
  constructor(
    start: AnimationValue,
    end: AnimationValue,
    duration: number,
    options?: TransitionOptions
  );
  progress(time: number): number;
  sample(time: number): AnimationValue;
  isComplete(time: number): boolean;
}

export interface SpringOptions {
  stiffness?: number;
  damping?: number;
}

export class Spring {
  constructor(value: number, options?: SpringOptions);
  step(target: number, deltaSeconds: number): number;
  readonly value: number;
  readonly velocity: number;
  readonly stiffness: number;
  readonly damping: number;
}

export interface AnimatedValueOptions {
  duration?: number;
  easing?: string;
}

export class AnimatedValue {
  constructor(initial: AnimationValue, options?: AnimatedValueOptions);
  setDuration(seconds: number): void;
  setEasing(easing: string): void;
  setTarget(target: AnimationValue): void;
  jumpTo(value: AnimationValue): void;
  tick(deltaSeconds: number): boolean;
  readonly value: AnimationValue;
  readonly target: AnimationValue;
  readonly isAnimating: boolean;
}

export interface KeyframeOptions {
  easing?: string;
}

export class Keyframe {
  constructor(time: number, value: AnimationValue, options?: KeyframeOptions);
  readonly time: number;
  readonly value: AnimationValue;
}

export class AnimationTrack {
  constructor(target: string, property: string);
  addKeyframe(keyframe: Keyframe): void;
  setEnabled(enabled: boolean): void;
  sample(time: number): AnimationValue | undefined;
  readonly target: string;
  readonly property: string;
  readonly keyframeCount: number;
}

export class AnimationClip {
  constructor(id: string, startTime: number, duration: number);
  addTrack(track: AnimationTrack): void;
  setEnabled(enabled: boolean): void;
  readonly id: string;
  readonly startTime: number;
  readonly duration: number;
  readonly trackCount: number;
}

export class AnimationSample {
  readonly clipId: string;
  readonly target: string;
  readonly property: string;
  readonly time: number;
  readonly value: AnimationValue;
}

export class AnimationTimeline {
  constructor(duration: number);
  addClip(clip: AnimationClip): void;
  sample(time: number): AnimationSample[];
  readonly duration: number;
  readonly clipCount: number;
}

export class AnimationPlayer {
  constructor(timeline: AnimationTimeline);
  play(): void;
  pause(): void;
  stop(): void;
  seek(time: number): void;
  setRepeat(repeat: boolean): void;
  setPlaybackRate(rate: number): void;
  sample(): AnimationSample[];
  tick(deltaSeconds: number): AnimationSample[];
  readonly playhead: number;
  readonly isPlaying: boolean;
}

export class AnimationDocument {
  constructor(name: string, timeline: AnimationTimeline);
  static parse(input: string): AnimationDocument;
  readonly name: string;
  readonly timeline: AnimationTimeline;
  toDocumentFormat(): string;
}

export class AnimationEditor {
  constructor(document: AnimationDocument);
  readonly document: AnimationDocument;
  setPlayhead(time: number): void;
  setZoom(zoom: number): void;
  setScroll(scroll: number): void;
  setSnapping(enabled?: boolean, interval?: number): void;
  addKeyframe(clipIndex: number, trackIndex: number, keyframe: Keyframe): boolean;
  updateKeyframeEasing(
    clipIndex: number,
    trackIndex: number,
    keyframeIndex: number,
    easing: string
  ): boolean;
  removeKeyframe(clipIndex: number, trackIndex: number, keyframeIndex: number): boolean;
  undo(): boolean;
  redo(): boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly playhead: number;
  readonly zoom: number;
  readonly scroll: number;
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
  measureWithChildren?(constraints: Constraints, childSizes: Size[]): Size;
  arrange?(bounds: Rect, childSizes: Size[]): Rect[];
  event?(event: Event, context: EventContext): boolean | void;
  paint?(paint: Paint): void;
  semantics?(semantics: Semantics): void;
}

export class Widget {
  constructor(callbacks: WidgetCallbacks, children?: Widget[]);
}

export class State {
  constructor(value: BindingValue);
  get(): BindingValue;
  set(value: BindingValue): void;
  select(selector: (value: BindingValue) => BindingValue): State;
  watch(callback: (value: BindingValue) => void): StateSubscription;
  readonly text: string;
}

export class StateSubscription {
  unsubscribe(): boolean;
}

export type ThemePreset =
  | "light"
  | "dark"
  | "neutral"
  | "neutral-dark"
  | "high-contrast"
  | "oled";

export class Theme {
  constructor(preset?: ThemePreset);
  static light(): Theme;
  static dark(): Theme;
  static neutral(): Theme;
  static neutralDark(): Theme;
  static highContrast(): Theme;
  static oled(): Theme;
  setPreset(preset: ThemePreset): void;
  setAccent(color: Color): void;
  setControlSize(size: "small" | "medium" | "large"): void;
  color(name: string): Color;
  setColor(name: string, color: Color): void;
  number(name: string): number;
  setNumber(name: string, value: number): void;
  readonly accent: Color;
}

export class Window {
  constructor(title: string);
  root(widget: Widget): void;
  setInitialSize(size: Size): void;
  setInitialPosition(position: Point): void;
  setIconSvg(svg: Uint8Array): void;
  removeIcon(): void;
}

export interface RenderOptions {
  feathering?: boolean;
  featherWidth?: number;
  opticalTextAlignment?: boolean;
  outputColorPrimaries?: "auto" | "srgb" | "display-p3";
  dynamicRange?: "auto" | "sdr" | "hdr";
  toneMapping?: "auto" | "clamp" | "reinhard";
  colorManagement?: "auto" | "force-sdr" | "prefer-wide-gamut" | "prefer-hdr";
  sdrContentBrightnessNits?: number;
  useSystemSdrBrightness?: boolean;
}

export class App {
  constructor();
  window(window: Window): void;
  configureRendering(options: RenderOptions): void;
  setTheme(theme: Theme): void;
  on(name: string, callback: (payload: BindingValue) => void): void;
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
  emit(name: string, payload: BindingValue): boolean;
  readonly pendingCount: number;
}

export interface FrameTiming {
  phase: string;
  durationMs: number;
}

export interface WidgetTiming {
  widgetId: string;
  widgetName: string;
  phase: string;
  durationMs: number;
  calls: number;
}

export interface EventRouteTrace {
  sequence: string;
  eventKind: string;
  targetId: string;
  path: string[];
  handled: boolean;
}

export interface ReactiveInvalidationTrace {
  widgetId: string;
  sourceName: string;
  version: string;
  kind: string;
  delivered: boolean;
}

export interface CommandDispatchTrace {
  sequence: string;
  name: string;
  payloadType: string;
  target: string;
  delivery: string;
  handlers: string[];
  handled: boolean;
  delivered: boolean;
}

export interface InvalidationTrace {
  target: string;
  kind: string;
  source: string;
  reason?: string;
}

export interface WidgetRebuildTrace {
  widgetId: string;
  widgetName: string;
  reason: string;
}

export class InspectorSnapshot {
  readonly windowId: string;
  readonly title: string;
  readonly tracingEnabled: boolean;
  readonly focusedWidgetId?: string;
  readonly windowFocused: boolean;
  readonly scheduledPhases: string[];
  readonly semanticsCount: number;
  readonly semanticsNodes: SemanticNode[];
  readonly widgetCount: number;
  readonly stackHostCount: number;
  readonly overlayCount: number;
  readonly timerCount: number;
  readonly asyncTaskCount: number;
  readonly requestedAnimationFrameCount: number;
  readonly widgetDiagnosticsCount: number;
  readonly eventRouteCount: number;
  readonly reactiveInvalidationCount: number;
  readonly commandDispatchCount: number;
  readonly invalidationCount: number;
  readonly widgetRebuildCount: number;
  readonly frameTimings: FrameTiming[];
  readonly widgetTimings: WidgetTiming[];
  readonly eventRoutes: EventRouteTrace[];
  readonly reactiveInvalidations: ReactiveInvalidationTrace[];
  readonly commandDispatches: CommandDispatchTrace[];
  readonly invalidations: InvalidationTrace[];
  readonly widgetRebuilds: WidgetRebuildTrace[];
}

export class RunningApp {
  uiHandle(): UiHandle;
  drain(): number;
  tick(frameTime: number): void;
  drainReadyEvents(): number;
  requestRedrawAll(): void;
  wakeWindow(window: WindowHandle): void;
  handleEventFor(window: WindowHandle, event: Event): void;
  setRenderOptions(window: WindowHandle, options: RenderOptions): void;
  setInspectorTracing(enabled?: boolean, index?: number): void;
  inspect(index?: number): InspectorSnapshot;
  render(index?: number): RenderSnapshot;
  renderWindow(window: WindowHandle): RenderSnapshot;
  needsRender(index?: number): boolean;
  requestRedraw(index?: number): void;
  handleEvent(event: Event, index?: number): void;
  hover(node: SemanticNode, index?: number): void;
  click(node: SemanticNode, index?: number): void;
  press(node: SemanticNode, key: string, index?: number): void;
  fill(node: SemanticNode, text: string, index?: number): void;
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

export interface SemanticsQuery {
  role?: string;
  name?: string;
  text?: string;
  description?: string;
  focused?: boolean;
  visible?: boolean;
}

export class SemanticNode {
  readonly id: string;
  readonly parentId?: string;
  readonly role: string;
  readonly name?: string;
  readonly value?: string;
  readonly description?: string;
  readonly bounds: Rect;
  readonly center: Point;
  readonly actions: string[];
  readonly checked?: string;
  readonly busy: boolean;
  readonly disabled: boolean;
  readonly focused: boolean;
  readonly hidden: boolean;
  readonly hovered: boolean;
  readonly selected: boolean;
  readonly expanded?: boolean;
  readonly editable: boolean;
  readonly multiline: boolean;
  readonly visible: boolean;
}

export class RenderSnapshot {
  commandCount: number;
  semanticsCount: number;
  readonly semanticsNodes: SemanticNode[];
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
  find(query?: SemanticsQuery): SemanticNode[];
  getOne(query?: SemanticsQuery): SemanticNode;
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

export function label(value: State | BindingValue): Widget;

export function Button(label: State | BindingValue, onPress?: () => void): Widget;

export interface ButtonOptions {
  onPress?: () => void;
}

export function button(label: State | BindingValue, options?: ButtonOptions): Widget;

export function Icon(glyph: IconGlyph | string, label?: string, size?: number, color?: Color): Widget;

export interface IconOptions {
  label?: string;
  size?: number;
  color?: Color;
}

export function icon(glyph: IconGlyph | string, options?: IconOptions): Widget;

export function IconButton(glyph: IconGlyph | string, label: State | BindingValue, selected?: State | boolean | number, enabled?: State | boolean | number, size?: number, iconSize?: number, description?: string, onPress?: () => void): Widget;

export interface IconButtonOptions {
  selected?: State | boolean | number;
  enabled?: State | boolean | number;
  size?: number;
  iconSize?: number;
  description?: string;
  onPress?: () => void;
}

export function iconButton(glyph: IconGlyph | string, label: State | BindingValue, options?: IconButtonOptions): Widget;

export function Link(label: State | BindingValue, url: State | BindingValue, semanticName?: string, enabled?: State | boolean | number, onOpen?: (url: string) => void): Widget;

export interface LinkOptions {
  semanticName?: string;
  enabled?: State | boolean | number;
  onOpen?: (url: string) => void;
}

export function link(label: State | BindingValue, url: State | BindingValue, options?: LinkOptions): Widget;

export function Checkbox(label: State | BindingValue, checked?: State | boolean | number, onToggle?: (checked: boolean) => void): Widget;

export interface CheckboxOptions {
  checked?: State | boolean | number;
  onToggle?: (checked: boolean) => void;
}

export function checkbox(label: State | BindingValue, options?: CheckboxOptions): Widget;

export function Switch(label: State | BindingValue, on?: State | boolean | number, onToggle?: (on: boolean) => void): Widget;

export interface SwitchOptions {
  on?: State | boolean | number;
  onToggle?: (on: boolean) => void;
}

export function switchControl(label: State | BindingValue, options?: SwitchOptions): Widget;

export function RadioButton(label: State | BindingValue, selected?: State | boolean | number, onSelect?: () => void): Widget;

export interface RadioButtonOptions {
  selected?: State | boolean | number;
  onSelect?: () => void;
}

export function radioButton(label: State | BindingValue, options?: RadioButtonOptions): Widget;

export function RadioGroup(name: State | BindingValue, options: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface RadioGroupOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function radioGroup(name: State | BindingValue, options: string[], config?: RadioGroupOptions): Widget;

export function SegmentedControl(name: State | BindingValue, items: SegmentedControlItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface SegmentedControlOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function segmentedControl(name: State | BindingValue, items: SegmentedControlItem[], options?: SegmentedControlOptions): Widget;

export function Breadcrumb(name: State | BindingValue, items: string[], current?: State | number | boolean, onActivate?: (index: number, value: string) => void): Widget;

export interface BreadcrumbOptions {
  current?: State | number | boolean;
  onActivate?: (index: number, value: string) => void;
}

export function breadcrumb(name: State | BindingValue, items: string[], options?: BreadcrumbOptions): Widget;

export function PathBar(name: State | BindingValue, items: string[], current?: State | number | boolean, onActivate?: (index: number, value: string) => void): Widget;

export interface PathBarOptions {
  current?: State | number | boolean;
  onActivate?: (index: number, value: string) => void;
}

export function pathBar(name: State | BindingValue, items: string[], options?: PathBarOptions): Widget;

export function ListView(name: State | BindingValue, items: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface ListViewOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function listView(name: State | BindingValue, items: string[], options?: ListViewOptions): Widget;

export function Table(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface TableOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function table(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], options?: TableOptions): Widget;

export function DataGrid(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface DataGridOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function dataGrid(name: State | BindingValue, columns: TableColumn[], rows: TableRow[], options?: DataGridOptions): Widget;

export function Slider(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, step?: number, onChange?: (value: number) => void): Widget;

export interface SliderOptions {
  value?: State | number | boolean;
  min?: number;
  max?: number;
  step?: number;
  onChange?: (value: number) => void;
}

export function slider(name: State | BindingValue, options?: SliderOptions): Widget;

export function NumberInput(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, step?: number, precision?: number, onChange?: (value: number) => void): Widget;

export interface NumberInputOptions {
  value?: State | number | boolean;
  min?: number;
  max?: number;
  step?: number;
  precision?: number;
  onChange?: (value: number) => void;
}

export function numberInput(name: State | BindingValue, options?: NumberInputOptions): Widget;

export function Select(name: State | BindingValue, options: string[], selected?: State | number | boolean, placeholder?: string, onChange?: (index: number, value: string) => void): Widget;

export interface SelectOptions {
  selected?: State | number | boolean;
  placeholder?: string;
  onChange?: (index: number, value: string) => void;
}

export function select(name: State | BindingValue, options: string[], config?: SelectOptions): Widget;

export function ProgressBar(name: State | BindingValue, value?: State | number | boolean, min?: number, max?: number, showValue?: boolean): Widget;

export interface ProgressBarOptions {
  value?: State | number | boolean;
  min?: number;
  max?: number;
  showValue?: boolean;
}

export function progressBar(name: State | BindingValue, options?: ProgressBarOptions): Widget;

export function SignalMeter(name: State | BindingValue, active?: State | boolean | number, description?: string, bars?: number, size?: Size): Widget;

export interface SignalMeterOptions {
  active?: State | boolean | number;
  description?: string;
  bars?: number;
  size?: Size;
}

export function signalMeter(name: State | BindingValue, options?: SignalMeterOptions): Widget;

export function StatusBadge(label: State | BindingValue, tone?: SemanticTone | string, icon?: IconGlyph | string, minWidth?: number): Widget;

export interface StatusBadgeOptions {
  tone?: SemanticTone | string;
  icon?: IconGlyph | string;
  minWidth?: number;
}

export function statusBadge(label: State | BindingValue, options?: StatusBadgeOptions): Widget;

export function StatusBar(segments: StatusBarSegment[], name?: string, description?: State | BindingValue, height?: number): Widget;

export interface StatusBarOptions {
  name?: string;
  description?: State | BindingValue;
  height?: number;
}

export function statusBar(segments: StatusBarSegment[], options?: StatusBarOptions): Widget;

export function DetailRow(label: State | BindingValue, value: State | BindingValue, maxValueLines?: number): Widget;

export interface DetailRowOptions {
  maxValueLines?: number;
}

export function detailRow(label: State | BindingValue, value: State | BindingValue, options?: DetailRowOptions): Widget;

export function BusyIndicator(name: State | BindingValue, label?: State | BindingValue, size?: number): Widget;

export interface BusyIndicatorOptions {
  label?: State | BindingValue;
  size?: number;
}

export function busyIndicator(name: State | BindingValue, options?: BusyIndicatorOptions): Widget;

export function TextInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export interface TextInputOptions {
  value?: State | BindingValue;
  placeholder?: string;
  onChange?: (value: string) => void;
}

export function textInput(name: State | BindingValue, options?: TextInputOptions): Widget;

export function TextArea(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export interface TextAreaOptions {
  value?: State | BindingValue;
  placeholder?: string;
  onChange?: (value: string) => void;
}

export function textArea(name: State | BindingValue, options?: TextAreaOptions): Widget;

export function RichText(spans: TextSpan[], semanticName?: string, minWidth?: number, minHeight?: number): Widget;

export interface RichTextOptions {
  semanticName?: string;
  minWidth?: number;
  minHeight?: number;
}

export function richText(spans: TextSpan[], options?: RichTextOptions): Widget;

export class RichDocument {
  constructor(markdown?: string);
  readonly revision: string;
  readonly markdown: string;
  setMarkdown(markdown: string): boolean;
  appendMarkdown(fragment: string): boolean;
  lastUpdate(): RichDocumentUpdate;
  appendAttachment(name: string, options?: RichAttachmentOptions): string;
  appendExtension(renderer: string, title: string, options?: RichExtensionOptions): string;
}

export class RichDocumentUpdate {
  readonly revision: string;
  readonly reparsedStart: number;
  readonly reparsedEnd: number;
  readonly reusedPrefixBlocks: number;
  readonly changedBlockIds: string[];
  readonly appendOnly: boolean;
}

export function RichDocumentView(document: RichDocument, onLink?: (destination: string) => void, onImage?: (source: string) => void, onAttachment?: (blockId: string) => void): Widget;

export interface RichDocumentViewOptions {
  onLink?: (destination: string) => void;
  onImage?: (source: string) => void;
  onAttachment?: (blockId: string) => void;
}

export function richDocumentView(document: RichDocument, options?: RichDocumentViewOptions): Widget;

export function Image(image: ImageHandle, label?: string, fit?: ImageFit, size?: Size): Widget;

export interface ImageOptions {
  label?: string;
  fit?: ImageFit;
  size?: Size;
}

export function image(image: ImageHandle, options?: ImageOptions): Widget;

export function ColorSwatch(name: string, color: Color, size?: Size, readOnly?: boolean, onPress?: () => void): Widget;

export interface ColorSwatchOptions {
  size?: Size;
  readOnly?: boolean;
  onPress?: () => void;
}

export function colorSwatch(name: string, color: Color, options?: ColorSwatchOptions): Widget;

export function Separator(axis?: Axis, name?: string, inset?: number, thickness?: number, length?: number): Widget;

export interface SeparatorOptions {
  axis?: Axis;
  name?: string;
  inset?: number;
  thickness?: number;
  length?: number;
}

export function separator(options?: SeparatorOptions): Widget;

export function EmptyState(title: string, description: string, name?: string, detail?: string, icon?: IconGlyph | string, action?: Widget, background?: Color, transparent?: boolean): Widget;

export interface EmptyStateOptions {
  name?: string;
  detail?: string;
  icon?: IconGlyph | string;
  action?: Widget;
  background?: Color;
  transparent?: boolean;
}

export function emptyState(title: string, description: string, options?: EmptyStateOptions): Widget;

export function Surface(child: Widget, role?: SurfaceRole | string, name?: string, border?: SurfaceBorder | string, elevation?: SurfaceElevation | string, radius?: number, padding?: number, fillWidth?: boolean, fillHeight?: boolean): Widget;

export interface SurfaceOptions {
  role?: SurfaceRole | string;
  name?: string;
  border?: SurfaceBorder | string;
  elevation?: SurfaceElevation | string;
  radius?: number;
  padding?: number;
  fillWidth?: boolean;
  fillHeight?: boolean;
}

export function surface(child: Widget, options?: SurfaceOptions): Widget;

export function Toolbar(children: Widget[], axis?: Axis, name?: string, extent?: number, padding?: number, spacing?: number, background?: Color, divider?: boolean): Widget;

export interface ToolbarOptions {
  axis?: Axis;
  name?: string;
  extent?: number;
  padding?: number;
  spacing?: number;
  background?: Color;
  divider?: boolean;
}

export function toolbar(children: Widget[], options?: ToolbarOptions): Widget;

export function Column(children: Widget[], gap?: number): Widget;

export interface ColumnOptions {
  gap?: number;
}

export function column(children: Widget[], options?: ColumnOptions): Widget;

export function Row(children: Widget[], gap?: number): Widget;

export interface RowOptions {
  gap?: number;
}

export function row(children: Widget[], options?: RowOptions): Widget;

export function Grid(children: Widget[], columns?: number, name?: string, gap?: number, columnGap?: number, rowGap?: number): Widget;

export interface GridOptions {
  columns?: number;
  name?: string;
  gap?: number;
  columnGap?: number;
  rowGap?: number;
}

export function grid(children: Widget[], options?: GridOptions): Widget;

export function AspectRatio(child: Widget, ratio: number, fit?: "contain" | "cover", horizontal?: "start" | "center" | "end" | "stretch", vertical?: "start" | "center" | "end" | "stretch"): Widget;

export interface AspectRatioOptions {
  fit?: "contain" | "cover";
  horizontal?: "start" | "center" | "end" | "stretch";
  vertical?: "start" | "center" | "end" | "stretch";
}

export function aspectRatio(child: Widget, ratio: number, options?: AspectRatioOptions): Widget;

export function SafeArea(child: Widget, edges?: string, minimumLeft?: number, minimumTop?: number, minimumRight?: number, minimumBottom?: number): Widget;

export interface SafeAreaOptions {
  edges?: string;
  minimumLeft?: number;
  minimumTop?: number;
  minimumRight?: number;
  minimumBottom?: number;
}

export function safeArea(child: Widget, options?: SafeAreaOptions): Widget;

export function LayoutTransition(child: Widget, duration?: number, easing?: "linear" | "ease-in" | "ease-out" | "ease-in-out"): Widget;

export interface LayoutTransitionOptions {
  duration?: number;
  easing?: "linear" | "ease-in" | "ease-out" | "ease-in-out";
}

export function layoutTransition(child: Widget, options?: LayoutTransitionOptions): Widget;

export function AdaptiveView(compact: Widget, medium: Widget, expanded: Widget, mediumBreakpoint?: number, expandedBreakpoint?: number, onClassChange?: (value: "compact" | "medium" | "expanded") => void): Widget;

export interface AdaptiveViewOptions {
  mediumBreakpoint?: number;
  expandedBreakpoint?: number;
  onClassChange?: (value: "compact" | "medium" | "expanded") => void;
}

export function adaptiveView(compact: Widget, medium: Widget, expanded: Widget, options?: AdaptiveViewOptions): Widget;

export class ConstraintCase {
  constructor(child: Widget, minWidth?: number, maxWidth?: number, minHeight?: number, maxHeight?: number, minAspectRatio?: number, maxAspectRatio?: number, orientation?: "any" | "portrait" | "landscape");
}

export function ConstraintView(cases: ConstraintCase[], fallback: Widget): Widget;

export function constraintView(cases: ConstraintCase[], fallback: Widget): Widget;

export class ResponsiveSidebarState {
  constructor(expanded?: boolean, overlayOpen?: boolean);
  readonly expanded: boolean;
  readonly overlayOpen: boolean;
  setExpanded(expanded: boolean): boolean;
  toggleExpanded(): boolean;
  openOverlay(): boolean;
  closeOverlay(): boolean;
  toggleOverlay(): boolean;
}

export function ResponsiveSidebar(state: ResponsiveSidebarState, sidebar: Widget, content: Widget, name?: string, mediumBreakpoint?: number, expandedBreakpoint?: number, railWidth?: number, overlayWidth?: number, dismissOnScrim?: boolean, onModeChange?: (mode: "overlay-closed" | "overlay-open" | "rail" | "inline") => void): Widget;

export interface ResponsiveSidebarOptions {
  name?: string;
  mediumBreakpoint?: number;
  expandedBreakpoint?: number;
  railWidth?: number;
  overlayWidth?: number;
  dismissOnScrim?: boolean;
  onModeChange?: (mode: "overlay-closed" | "overlay-open" | "rail" | "inline") => void;
}

export function responsiveSidebar(state: ResponsiveSidebarState, sidebar: Widget, content: Widget, options?: ResponsiveSidebarOptions): Widget;

export class MasterDetailState {
  constructor(route?: "master" | "detail");
  readonly route: "master" | "detail";
  setRoute(route: "master" | "detail"): boolean;
  showMaster(): boolean;
  showDetail(): boolean;
}

export function MasterDetail(state: MasterDetailState, master: Widget, detail: Widget, mediumBreakpoint?: number, expandedBreakpoint?: number, masterWidth?: number): Widget;

export interface MasterDetailOptions {
  mediumBreakpoint?: number;
  expandedBreakpoint?: number;
  masterWidth?: number;
}

export function masterDetail(state: MasterDetailState, master: Widget, detail: Widget, options?: MasterDetailOptions): Widget;

export function OverlayHost(child: Widget): Widget;

export function overlayHost(child: Widget): Widget;

export class NotificationCenter {
  constructor();
  notify(title: string, message: string, options?: NotificationOptions): string;
  dismiss(id: string): boolean;
  clear(): boolean;
  readonly size: number;
}

export function NotificationHost(center: NotificationCenter, width?: number): Widget;

export interface NotificationHostOptions {
  width?: number;
}

export function notificationHost(center: NotificationCenter, options?: NotificationHostOptions): Widget;

export class VirtualListItem {
  constructor(key: string, text: string);
  readonly key: string;
  readonly text: string;
}

export class VirtualListModel {
  constructor(name: string, items?: VirtualListItem[]);
  readonly size: number;
  append(item: VirtualListItem): boolean;
  prepend(items: VirtualListItem[]): boolean;
  update(item: VirtualListItem): boolean;
  remove(key: string): boolean;
  moveTo(key: string, index: number): boolean;
  replace(items: VirtualListItem[]): boolean;
}

export class CanvasViewport {
  constructor(panX?: number, panY?: number, zoom?: number, rotation?: number);
  readonly panX: number;
  readonly panY: number;
  readonly zoom: number;
  readonly rotation: number;
}

export class CanvasStroke {
  constructor(color: Color, width?: number);
  readonly color: Color;
  readonly width: number;
}

export class CanvasShape {
  static path(path: Path, fill?: Color, stroke?: CanvasStroke): CanvasShape;
  static rect(rect: Rect, fill?: Color, stroke?: CanvasStroke): CanvasShape;
  static circle(center: Point, radius: number, fill?: Color, stroke?: CanvasStroke): CanvasShape;
  static polyline(points: Point[], stroke: CanvasStroke): CanvasShape;
}

export function Canvas(name: string, shapes?: CanvasShape[], viewport?: CanvasViewport, drawStroke?: CanvasStroke, desiredSize?: Size): Widget;

export interface CanvasOptions {
  shapes?: CanvasShape[];
  viewport?: CanvasViewport;
  drawStroke?: CanvasStroke;
  desiredSize?: Size;
}

export function canvas(name: string, options?: CanvasOptions): Widget;

export function CanvasRuler(axis: Axis, name: string, documentSize: Size, viewport?: CanvasViewport, viewportSize?: Size, extent?: number): Widget;

export interface CanvasRulerOptions {
  viewport?: CanvasViewport;
  viewportSize?: Size;
  extent?: number;
}

export function canvasRuler(axis: Axis, name: string, documentSize: Size, options?: CanvasRulerOptions): Widget;

export class PixelCanvasExport {
  readonly revision: string;
  readonly name: string;
  readonly width: number;
  readonly height: number;
  readonly rgba8: Uint8Array;
}

export class PixelCanvasState {
  constructor();
  tool: "brush" | "eraser" | "fill" | "pan";
  brushColor: Color;
  brushSize: number;
  brushOpacity: number;
  brushShape: "square" | "round";
  blendMode: "normal" | "multiply" | "screen" | "overlay";
  editable: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly canClear: boolean;
  undo(): void;
  redo(): void;
  clear(): void;
  fitView(): void;
  actualSize(): void;
  zoomIn(): void;
  zoomOut(): void;
  requestExport(): void;
  latestExport(): PixelCanvasExport | undefined;
}

export function PixelCanvas(state: PixelCanvasState, name: string, width: number, height: number, paperColor?: Color, desiredSize?: Size, viewport?: CanvasViewport, fitOnFirstLayout?: boolean, pixels?: Color[]): Widget;

export interface PixelCanvasOptions {
  paperColor?: Color;
  desiredSize?: Size;
  viewport?: CanvasViewport;
  fitOnFirstLayout?: boolean;
  pixels?: Color[];
}

export function pixelCanvas(state: PixelCanvasState, name: string, width: number, height: number, options?: PixelCanvasOptions): Widget;

export class DragScope {
  constructor();
  readonly active: boolean;
}

export function DragDropHost(scope: DragScope, child: Widget, onExternalHover?: (paths: string[]) => void, onExternalDrop?: (path: string) => void, onExternalCancel?: () => void): Widget;

export interface DragDropHostOptions {
  onExternalHover?: (paths: string[]) => void;
  onExternalDrop?: (path: string) => void;
  onExternalCancel?: () => void;
}

export function dragDropHost(scope: DragScope, child: Widget, options?: DragDropHostOptions): Widget;

export function Draggable(scope: DragScope, child: Widget, payload: string, effect?: "copy" | "move" | "link", previewLabel?: string, threshold?: number, onStart?: (payload: string) => void, onEnd?: (payload: string) => void): Widget;

export interface DraggableOptions {
  effect?: "copy" | "move" | "link";
  previewLabel?: string;
  threshold?: number;
  onStart?: (payload: string) => void;
  onEnd?: (payload: string) => void;
}

export function draggable(scope: DragScope, child: Widget, payload: string, options?: DraggableOptions): Widget;

export function DropTarget(scope: DragScope, child: Widget, effect?: "none" | "copy" | "move" | "link", onDrop?: (payload: string) => void, onHoverChange?: (hovered: boolean) => void): Widget;

export interface DropTargetOptions {
  effect?: "none" | "copy" | "move" | "link";
  onDrop?: (payload: string) => void;
  onHoverChange?: (hovered: boolean) => void;
}

export function dropTarget(scope: DragScope, child: Widget, options?: DropTargetOptions): Widget;

export function VirtualList(name: string, model: VirtualListModel, estimatedRowHeight?: number, spacing?: number, padding?: number, rowPadding?: number, overscanViewports?: number, cacheCapacity?: number, selectable?: boolean, transparent?: boolean, stickToEnd?: boolean, overlayScrollBars?: boolean, onChange?: (key: string) => void, onNearStart?: () => void, onNearEnd?: () => void): Widget;

export interface VirtualListOptions {
  estimatedRowHeight?: number;
  spacing?: number;
  padding?: number;
  rowPadding?: number;
  overscanViewports?: number;
  cacheCapacity?: number;
  selectable?: boolean;
  transparent?: boolean;
  stickToEnd?: boolean;
  overlayScrollBars?: boolean;
  onChange?: (key: string) => void;
  onNearStart?: () => void;
  onNearEnd?: () => void;
}

export function virtualList(name: string, model: VirtualListModel, options?: VirtualListOptions): Widget;

export function ScrollView(child: Widget, axes?: ScrollAxes, name?: string): Widget;

export interface ScrollViewOptions {
  axes?: ScrollAxes;
  name?: string;
}

export function scrollView(child: Widget, options?: ScrollViewOptions): Widget;

export function ExternalSurface(texture: ExternalTextureDescriptor, desiredSize?: Size, name?: string): Widget;

export interface ExternalSurfaceOptions {
  desiredSize?: Size;
  name?: string;
}

export function externalSurface(texture: ExternalTextureDescriptor, options?: ExternalSurfaceOptions): Widget;

export class TreeItem {
  constructor(label: string, detail?: string, expanded?: boolean, disabled?: boolean, children?: TreeItem[]);
}

export function TreeView(name: State | BindingValue, items: TreeItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface TreeViewOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function treeView(name: State | BindingValue, items: TreeItem[], options?: TreeViewOptions): Widget;

export class LayerListItem {
  constructor(label: string, detail?: string, visible?: boolean, locked?: boolean, disabled?: boolean);
}

export function LayerList(name: State | BindingValue, items: LayerListItem[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface LayerListOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function layerList(name: State | BindingValue, items: LayerListItem[], options?: LayerListOptions): Widget;

export class MenuItem {
  constructor(label: string, shortcut?: string, disabled?: boolean, destructive?: boolean, separatorBefore?: boolean, submenu?: MenuItem[]);
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

export interface MenuOptions {
  highlighted?: State | number | boolean;
  onActivate?: (index: number, value: string) => void;
}

export function menu(name: State | BindingValue, items: MenuItem[], options?: MenuOptions): Widget;

export function ContextMenu(name: string, trigger: Widget, items: MenuItem[], onActivate?: (index: number, value: string) => void): Widget;

export interface ContextMenuOptions {
  onActivate?: (index: number, value: string) => void;
}

export function contextMenu(name: string, trigger: Widget, items: MenuItem[], options?: ContextMenuOptions): Widget;

export function TabBar(name: State | BindingValue, tabs: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void): Widget;

export interface TabBarOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
}

export function tabBar(name: State | BindingValue, tabs: string[], options?: TabBarOptions): Widget;

export function Tabs(name: State | BindingValue, tabs: string[], selected?: State | number | boolean): Widget;

export interface TabsOptions {
  selected?: State | number | boolean;
}

export function tabs(name: State | BindingValue, tabs: string[], options?: TabsOptions): Widget;

export function Dialog(title: State | BindingValue, content: Widget, shown?: State | boolean | number): Widget;

export interface DialogOptions {
  shown?: State | boolean | number;
}

export function dialog(title: State | BindingValue, content: Widget, options?: DialogOptions): Widget;

export function CommandPalette(name: string, content: Widget, description?: string, shown?: State | boolean | number, maxWidth?: number, onDismiss?: () => void): Widget;

export interface CommandPaletteOptions {
  description?: string;
  shown?: State | boolean | number;
  maxWidth?: number;
  onDismiss?: () => void;
}

export function commandPalette(name: string, content: Widget, options?: CommandPaletteOptions): Widget;

export function Padding(child: Widget, padding?: number, top?: number, right?: number, bottom?: number, fillChildWidth?: boolean, fillChildHeight?: boolean): Widget;

export interface PaddingOptions {
  padding?: number;
  top?: number;
  right?: number;
  bottom?: number;
  fillChildWidth?: boolean;
  fillChildHeight?: boolean;
}

export function padding(child: Widget, options?: PaddingOptions): Widget;

export function Align(child: Widget, horizontal?: "start" | "center" | "end" | "stretch", vertical?: "start" | "center" | "end" | "stretch"): Widget;

export interface AlignOptions {
  horizontal?: "start" | "center" | "end" | "stretch";
  vertical?: "start" | "center" | "end" | "stretch";
}

export function align(child: Widget, options?: AlignOptions): Widget;

export function Background(child: Widget, color: Color): Widget;

export function background(child: Widget, color: Color): Widget;

export function SizedBox(child?: Widget, width?: number, height?: number): Widget;

export interface SizedBoxOptions {
  child?: Widget;
  width?: number;
  height?: number;
}

export function sizedBox(options?: SizedBoxOptions): Widget;

export function Stack(children: Widget[], axis?: Axis, spacing?: number, alignment?: "start" | "center" | "end" | "stretch"): Widget;

export interface StackOptions {
  axis?: Axis;
  spacing?: number;
  alignment?: "start" | "center" | "end" | "stretch";
}

export function stack(children: Widget[], options?: StackOptions): Widget;

export function SemanticRegion(name: State | BindingValue, child: Widget, description?: State | BindingValue, role?: string): Widget;

export interface SemanticRegionOptions {
  description?: State | BindingValue;
  role?: string;
}

export function semanticRegion(name: State | BindingValue, child: Widget, options?: SemanticRegionOptions): Widget;

export function FormRow(label: string, control: Widget, stacked?: boolean, labelWidth?: number, controlWidth?: number, gap?: number): Widget;

export interface FormRowOptions {
  stacked?: boolean;
  labelWidth?: number;
  controlWidth?: number;
  gap?: number;
}

export function formRow(label: string, control: Widget, options?: FormRowOptions): Widget;

export function FieldGroup(children: Widget[], spacing?: number, padding?: number, maxWidth?: number, fillWidth?: boolean): Widget;

export interface FieldGroupOptions {
  spacing?: number;
  padding?: number;
  maxWidth?: number;
  fillWidth?: boolean;
}

export function fieldGroup(children: Widget[], options?: FieldGroupOptions): Widget;

export function FormSection(title: string, child: Widget, description?: string, headerAction?: Widget, padding?: number, bodyGap?: number, headerGap?: number, maxWidth?: number, fillWidth?: boolean, radius?: number, elevation?: SurfaceElevation | string): Widget;

export interface FormSectionOptions {
  description?: string;
  headerAction?: Widget;
  padding?: number;
  bodyGap?: number;
  headerGap?: number;
  maxWidth?: number;
  fillWidth?: boolean;
  radius?: number;
  elevation?: SurfaceElevation | string;
}

export function formSection(title: string, child: Widget, options?: FormSectionOptions): Widget;

export function PanelSection(title: string, child: Widget, headerAction?: Widget, gap?: number, actionGap?: number, collapsible?: boolean, expanded?: boolean): Widget;

export interface PanelSectionOptions {
  headerAction?: Widget;
  gap?: number;
  actionGap?: number;
  collapsible?: boolean;
  expanded?: boolean;
}

export function panelSection(title: string, child: Widget, options?: PanelSectionOptions): Widget;

export function DockPanel(title: string, child: Widget, name?: string, headerHeight?: number, padding?: number, background?: Color, headerBackground?: Color): Widget;

export interface DockPanelOptions {
  name?: string;
  headerHeight?: number;
  padding?: number;
  background?: Color;
  headerBackground?: Color;
}

export function dockPanel(title: string, child: Widget, options?: DockPanelOptions): Widget;

export function StatusBarHost(content: Widget, statusBar: Widget): Widget;

export function statusBarHost(content: Widget, statusBar: Widget): Widget;

export function Tooltip(text: string, child: Widget, placement?: "above" | "below"): Widget;

export interface TooltipOptions {
  placement?: "above" | "below";
}

export function tooltip(text: string, child: Widget, options?: TooltipOptions): Widget;

export function Popover(name: string, trigger: Widget, content: Widget, open?: boolean): Widget;

export interface PopoverOptions {
  open?: boolean;
}

export function popover(name: string, trigger: Widget, content: Widget, options?: PopoverOptions): Widget;

export function ToolPalette(name: string, items: ToolPaletteItem[], selected?: State | number | boolean, axis?: Axis, onChange?: (index: number, value: string) => void, extent?: number, padding?: number, spacing?: number, itemSize?: number, iconSize?: number, background?: Color, divider?: boolean): Widget;

export interface ToolPaletteOptions {
  selected?: State | number | boolean;
  axis?: Axis;
  onChange?: (index: number, value: string) => void;
  extent?: number;
  padding?: number;
  spacing?: number;
  itemSize?: number;
  iconSize?: number;
  background?: Color;
  divider?: boolean;
}

export function toolPalette(name: string, items: ToolPaletteItem[], options?: ToolPaletteOptions): Widget;

export function PresetStrip(name: string, presets: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void, itemWidth?: number, itemHeight?: number, gap?: number): Widget;

export interface PresetStripOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
  itemWidth?: number;
  itemHeight?: number;
  gap?: number;
}

export function presetStrip(name: string, presets: string[], options?: PresetStripOptions): Widget;

export function BrowserTabBar(name: string, tabs: string[], selected?: State | number | boolean, onChange?: (index: number, value: string) => void, onClose?: (index: number, value: string) => void): Widget;

export interface BrowserTabBarOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, value: string) => void;
  onClose?: (index: number, value: string) => void;
}

export function browserTabBar(name: string, tabs: string[], options?: BrowserTabBarOptions): Widget;

export function ColorPalette(name: string, swatches: ColorPaletteSwatch[], selected?: State | number | boolean, onChange?: (index: number, name: string, color: Color) => void, columns?: number, swatchSize?: number, gap?: number): Widget;

export interface ColorPaletteOptions {
  selected?: State | number | boolean;
  onChange?: (index: number, name: string, color: Color) => void;
  columns?: number;
  swatchSize?: number;
  gap?: number;
}

export function colorPalette(name: string, swatches: ColorPaletteSwatch[], options?: ColorPaletteOptions): Widget;

export function ColorPicker(name: string, color?: Color, onChange?: (color: Color) => void, showAlpha?: boolean, compact?: boolean): Widget;

export interface ColorPickerOptions {
  color?: Color;
  onChange?: (color: Color) => void;
  showAlpha?: boolean;
  compact?: boolean;
}

export function colorPicker(name: string, options?: ColorPickerOptions): Widget;

export function SimpleColorPicker(name: string, color?: Color, mode?: "hsl" | "hsv" | "rgb", onChange?: (color: Color) => void, showAlpha?: boolean, compact?: boolean): Widget;

export interface SimpleColorPickerOptions {
  color?: Color;
  mode?: "hsl" | "hsv" | "rgb";
  onChange?: (color: Color) => void;
  showAlpha?: boolean;
  compact?: boolean;
}

export function simpleColorPicker(name: string, options?: SimpleColorPickerOptions): Widget;

export class BrushPreviewSpec {
  constructor(color: Color, size?: number, opacity?: number, shape?: "round" | "square");
  readonly color: Color;
  readonly size: number;
  readonly opacity: number;
  readonly shape: "round" | "square";
}

export function PasswordInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export interface PasswordInputOptions {
  value?: State | BindingValue;
  placeholder?: string;
  onChange?: (value: string) => void;
}

export function passwordInput(name: State | BindingValue, options?: PasswordInputOptions): Widget;

export function DateTimeInput(name: State | BindingValue, value?: State | BindingValue, placeholder?: string, onChange?: (value: string) => void): Widget;

export interface DateTimeInputOptions {
  value?: State | BindingValue;
  placeholder?: string;
  onChange?: (value: string) => void;
}

export function dateTimeInput(name: State | BindingValue, options?: DateTimeInputOptions): Widget;

export function ActionCard(title: string, description: string, icon?: IconGlyph | string, tone?: SemanticTone | string, enabled?: State | boolean | number, onPress?: () => void): Widget;

export interface ActionCardOptions {
  icon?: IconGlyph | string;
  tone?: SemanticTone | string;
  enabled?: State | boolean | number;
  onPress?: () => void;
}

export function actionCard(title: string, description: string, options?: ActionCardOptions): Widget;

export function BrushPreview(name: string, spec: BrushPreviewSpec, kind?: string, size?: Size): Widget;

export interface BrushPreviewOptions {
  kind?: string;
  size?: Size;
}

export function brushPreview(name: string, spec: BrushPreviewSpec, options?: BrushPreviewOptions): Widget;

export function CommandGroup(name: string, children: Widget[], axis?: Axis, padding?: number, spacing?: number, cornerRadius?: number, background?: Color, border?: Color): Widget;

export interface CommandGroupOptions {
  axis?: Axis;
  padding?: number;
  spacing?: number;
  cornerRadius?: number;
  background?: Color;
  border?: Color;
}

export function commandGroup(name: string, children: Widget[], options?: CommandGroupOptions): Widget;

export function CoverageDots(name: string, current: number, target: number, tone?: SemanticTone | string, maxDots?: number, showLabel?: boolean, minWidth?: number): Widget;

export interface CoverageDotsOptions {
  tone?: SemanticTone | string;
  maxDots?: number;
  showLabel?: boolean;
  minWidth?: number;
}

export function coverageDots(name: string, current: number, target: number, options?: CoverageDotsOptions): Widget;

export function Dock(body: Widget, top?: Widget, topHeight?: number, bottom?: Widget, bottomHeight?: number, fallbackWidth?: number, fallbackBodyHeight?: number): Widget;

export interface DockOptions {
  top?: Widget;
  topHeight?: number;
  bottom?: Widget;
  bottomHeight?: number;
  fallbackWidth?: number;
  fallbackBodyHeight?: number;
}

export function dock(body: Widget, options?: DockOptions): Widget;

export function FixedPaneSplit(first: Widget, divider: Widget, second: Widget, axis?: Axis, fixedPane?: "first" | "second", fixedExtent?: number, dividerExtent?: number, fallbackFlexibleExtent?: number): Widget;

export interface FixedPaneSplitOptions {
  axis?: Axis;
  fixedPane?: "first" | "second";
  fixedExtent?: number;
  dividerExtent?: number;
  fallbackFlexibleExtent?: number;
}

export function fixedPaneSplit(first: Widget, divider: Widget, second: Widget, options?: FixedPaneSplitOptions): Widget;

export function FramedField(child: Widget, name?: string, description?: string, padding?: number, minHeight?: number, fillWidth?: boolean, focused?: State | boolean | number, invalid?: State | boolean | number): Widget;

export interface FramedFieldOptions {
  name?: string;
  description?: string;
  padding?: number;
  minHeight?: number;
  fillWidth?: boolean;
  focused?: State | boolean | number;
  invalid?: State | boolean | number;
}

export function framedField(child: Widget, options?: FramedFieldOptions): Widget;

export function MeasuredBottomDock(body: Widget, bottom: Widget, fallbackSize?: Size): Widget;

export interface MeasuredBottomDockOptions {
  fallbackSize?: Size;
}

export function measuredBottomDock(body: Widget, bottom: Widget, options?: MeasuredBottomDockOptions): Widget;

export function PlacementBadge(label: State | BindingValue, icon?: IconGlyph | string, tone?: SemanticTone | string, current?: number, target?: number, minWidth?: number): Widget;

export interface PlacementBadgeOptions {
  icon?: IconGlyph | string;
  tone?: SemanticTone | string;
  current?: number;
  target?: number;
  minWidth?: number;
}

export function placementBadge(label: State | BindingValue, options?: PlacementBadgeOptions): Widget;

export function PropertyRow(label: string, control: Widget, stacked?: boolean, labelWidth?: number, controlWidth?: number, gap?: number): Widget;

export interface PropertyRowOptions {
  stacked?: boolean;
  labelWidth?: number;
  controlWidth?: number;
  gap?: number;
}

export function propertyRow(label: string, control: Widget, options?: PropertyRowOptions): Widget;

export function SectionLabel(label: string, semanticName?: string, color?: Color): Widget;

export interface SectionLabelOptions {
  semanticName?: string;
  color?: Color;
}

export function sectionLabel(label: string, options?: SectionLabelOptions): Widget;

export function SideSheet(title: string, body: Widget, description?: string, shown?: State | boolean | number, modal?: boolean, dismissOnScrim?: boolean, placement?: "left" | "right", width?: number, headerAction?: Widget, actions?: Widget[], onDismiss?: () => void): Widget;

export interface SideSheetOptions {
  description?: string;
  shown?: State | boolean | number;
  modal?: boolean;
  dismissOnScrim?: boolean;
  placement?: "left" | "right";
  width?: number;
  headerAction?: Widget;
  actions?: Widget[];
  onDismiss?: () => void;
}

export function sideSheet(title: string, body: Widget, options?: SideSheetOptions): Widget;

export function BottomSheet(title: string, body: Widget, description?: string, shown?: State | boolean | number, modal?: boolean, dismissOnScrim?: boolean, height?: number, headerAction?: Widget, actions?: Widget[], onDismiss?: () => void): Widget;

export interface BottomSheetOptions {
  description?: string;
  shown?: State | boolean | number;
  modal?: boolean;
  dismissOnScrim?: boolean;
  height?: number;
  headerAction?: Widget;
  actions?: Widget[];
  onDismiss?: () => void;
}

export function bottomSheet(title: string, body: Widget, options?: BottomSheetOptions): Widget;

export function SplitView(first: Widget, second: Widget, axis?: Axis, name?: string, ratio?: State | number | boolean, minFirst?: number, minSecond?: number, dividerThickness?: number, onChange?: (ratio: number) => void): Widget;

export interface SplitViewOptions {
  axis?: Axis;
  name?: string;
  ratio?: State | number | boolean;
  minFirst?: number;
  minSecond?: number;
  dividerThickness?: number;
  onChange?: (ratio: number) => void;
}

export function splitView(first: Widget, second: Widget, options?: SplitViewOptions): Widget;

export function SwitchView(children: Widget[], selected?: State | number | boolean): Widget;

export interface SwitchViewOptions {
  selected?: State | number | boolean;
}

export function switchView(children: Widget[], options?: SwitchViewOptions): Widget;

export function TrailingSlotRow(body: Widget, trailing: Widget, trailingWidth?: number, trailingHeight?: number, gap?: number): Widget;

export interface TrailingSlotRowOptions {
  trailingWidth?: number;
  trailingHeight?: number;
  gap?: number;
}

export function trailingSlotRow(body: Widget, trailing: Widget, options?: TrailingSlotRowOptions): Widget;

export class FloatingStackWindow {
  constructor(bounds: Rect, child: Widget);
}

export class FloatingView {
  constructor(title: string, bounds: Rect, child: Widget, minSize?: Size, visible?: boolean);
  readonly title: string;
  readonly bounds: Rect;
  readonly minSize: Size;
  readonly visible: boolean;
}

export class FloatingViewSnapshot {
  readonly id: string;
  readonly title: string;
  readonly bounds: Rect;
  readonly minSize: Size;
  readonly visible: boolean;
  readonly maximized: boolean;
}

export class FloatingWorkspaceState {
  constructor();
  views(): FloatingViewSnapshot[];
  setVisible(id: string, visible: boolean): boolean;
  setBounds(id: string, bounds: Rect): boolean;
  bringToFront(id: string): boolean;
  setMaximized(id: string, maximized: boolean): boolean;
}

export function FloatingWorkspace(state: FloatingWorkspaceState, views: FloatingView[], name?: string): Widget;

export interface FloatingWorkspaceOptions {
  name?: string;
}

export function floatingWorkspace(state: FloatingWorkspaceState, views: FloatingView[], options?: FloatingWorkspaceOptions): Widget;

export class DockNode {
  static empty(): DockNode;
  static tabs(panelIds: string[], active?: string): DockNode;
  static split(axis: Axis, fraction: number, first: DockNode, second: DockNode): DockNode;
}

export class DockFloatingGroup {
  constructor(id: string, panelIds: string[], active: string, bounds: Rect);
  readonly id: string;
  readonly panelIds: string[];
  readonly active: string;
  readonly bounds: Rect;
}

export class DockLayout {
  constructor(root: DockNode, floating?: DockFloatingGroup[], hidden?: string[]);
  readonly root: DockNode;
  readonly floating: DockFloatingGroup[];
  readonly hidden: string[];
}

export class DockState {
  constructor(layout?: DockLayout);
  snapshot(): DockLayout;
  apply(layout: DockLayout): boolean;
  dock(panel: string, target: string, zone?: "center" | "left" | "right" | "top" | "bottom"): boolean;
  dockToRoot(panel: string, zone?: "center" | "left" | "right" | "top" | "bottom"): boolean;
  floatPanel(panel: string, bounds: Rect): string;
  hide(panel: string): boolean;
  show(panel: string): boolean;
  activate(panel: string): boolean;
}

export class DockPanelSpec {
  constructor(id: string, title: string, child: Widget);
  readonly id: string;
  readonly title: string;
}

export function DockWorkspace(state: DockState, panels: DockPanelSpec[], name?: string): Widget;

export interface DockWorkspaceOptions {
  name?: string;
}

export function dockWorkspace(state: DockState, panels: DockPanelSpec[], options?: DockWorkspaceOptions): Widget;

export function FloatingStack(windows: FloatingStackWindow[], name?: string): Widget;

export interface FloatingStackOptions {
  name?: string;
}

export function floatingStack(windows: FloatingStackWindow[], options?: FloatingStackOptions): Widget;

export function VirtualScrollView(children: Widget[], name?: string, padding?: number, spacing?: number): Widget;

export interface VirtualScrollViewOptions {
  name?: string;
  padding?: number;
  spacing?: number;
}

export function virtualScrollView(children: Widget[], options?: VirtualScrollViewOptions): Widget;

export function ReorderableList(name: string, children: Widget[], spacing?: number, dragThreshold?: number, previewLabel?: string, onReorder?: (item: number, fromIndex: number, toIndex: number) => void): Widget;

export interface ReorderableListOptions {
  spacing?: number;
  dragThreshold?: number;
  previewLabel?: string;
  onReorder?: (item: number, fromIndex: number, toIndex: number) => void;
}

export function reorderableList(name: string, children: Widget[], options?: ReorderableListOptions): Widget;

// END GENERATED SUI WIDGET BINDINGS
