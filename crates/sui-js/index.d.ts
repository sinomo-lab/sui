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
  readonly x: number;
  readonly y: number;
}

export class Modifiers {
  constructor(shift?: boolean, control?: boolean, alt?: boolean, meta?: boolean);
  readonly shift: boolean;
  readonly control: boolean;
  readonly alt: boolean;
  readonly meta: boolean;
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
    isPrimary?: boolean
  ): Event;
  static scroll(position: Point, delta: Point, mode?: "pixels" | "lines", pointerId?: string): Event;
  static keyboard(
    key: string,
    state?: KeyState,
    code?: string,
    text?: string,
    repeat?: boolean,
    isComposing?: boolean
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
}

export class Size {
  constructor(width: number, height: number);
  readonly width: number;
  readonly height: number;
}

export class Rect {
  constructor(x: number, y: number, width: number, height: number);
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
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
  readonly xx: number;
  readonly yx: number;
  readonly xy: number;
  readonly yy: number;
  readonly dx: number;
  readonly dy: number;
}

export class Color {
  constructor(red: number, green: number, blue: number, alpha?: number);
  readonly red: number;
  readonly green: number;
  readonly blue: number;
  readonly alpha: number;
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

export class TextSpan {
  constructor(
    text: string,
    color?: Color,
    fontSize?: number,
    lineHeight?: number,
    font?: FontHandle,
    weight?: number,
    style?: FontStyle,
    stretch?: FontStretch
  );
  readonly text: string;
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
  static saturationValuePlane(colorSpace: string, hue: number, maxValue?: number): Shader;
  static saturationBar(colorSpace: string, hue: number, value: number, maxValue?: number): Shader;
  static valueBar(colorSpace: string, hue: number, saturation: number, maxValue?: number): Shader;
  static alphaBar(color: Color, colorSpace?: string): Shader;
  static rgbChannelBar(color: Color, channel: number, colorSpace?: string): Shader;
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
  readonly commandCount: number;
  readonly semanticsCount: number;
  readonly semanticsRoles: string[];
  readonly semanticsNames: string[];
  readonly semanticsValues: string[];
  readonly semanticsDescriptions: string[];
  readonly semanticsChecked: string[];
  readonly semanticsBusy: boolean[];
  readonly semanticsEditableMultiline: boolean[];
  readonly semanticsDisabled: boolean[];
  readonly semanticsFocused: boolean[];
  readonly semanticsHidden: boolean[];
  readonly semanticsHovered: boolean[];
  readonly semanticsSelected: boolean[];
  readonly semanticsExpanded: string[];
  readonly fillRectCount: number;
  readonly drawImageCount: number;
  readonly registeredFontCount: number;
  readonly registeredImageCount: number;
}

export function Label(value: State | BindingValue): Widget;
export function Button(label: State | BindingValue, onPress?: () => void): Widget;
export function Link(
  label: State | BindingValue,
  url: State | BindingValue,
  semanticName?: string,
  enabled?: State | boolean | number,
  onOpen?: (url: string) => void
): Widget;
export function Checkbox(
  label: State | BindingValue,
  checked?: State | boolean | number,
  onToggle?: (checked: boolean) => void
): Widget;
export function Switch(
  label: State | BindingValue,
  on?: State | boolean | number,
  onToggle?: (on: boolean) => void
): Widget;
export function RadioButton(
  label: State | BindingValue,
  selected?: State | boolean | number,
  onSelect?: () => void
): Widget;
export function Slider(
  name: State | BindingValue,
  value?: State | number | boolean,
  min?: number,
  max?: number,
  step?: number,
  onChange?: (value: number) => void
): Widget;
export function NumberInput(
  name: State | BindingValue,
  value?: State | number | boolean,
  min?: number,
  max?: number,
  step?: number,
  precision?: number,
  onChange?: (value: number) => void
): Widget;
export function Select(
  name: State | BindingValue,
  options: string[],
  selected?: State | number | boolean,
  placeholder?: string,
  onChange?: (index: number, value: string) => void
): Widget;
export function ProgressBar(
  name: State | BindingValue,
  value?: State | number | boolean,
  min?: number,
  max?: number,
  showValue?: boolean
): Widget;
export function BusyIndicator(
  name: State | BindingValue,
  label?: State | BindingValue,
  size?: number
): Widget;
export function TextInput(
  name: State | BindingValue,
  value?: State | BindingValue,
  placeholder?: string,
  onChange?: (value: string) => void
): Widget;
export function TextArea(
  name: State | BindingValue,
  value?: State | BindingValue,
  placeholder?: string,
  onChange?: (value: string) => void
): Widget;
export function RichText(
  spans: TextSpan[],
  semanticName?: string,
  minWidth?: number,
  minHeight?: number
): Widget;
export function Image(
  image: ImageHandle,
  label?: string,
  fit?: ImageFit,
  size?: Size
): Widget;
export function ColorSwatch(
  name: string,
  color: Color,
  size?: Size,
  readOnly?: boolean,
  onPress?: () => void
): Widget;
export function Separator(
  axis?: Axis,
  name?: string,
  inset?: number,
  thickness?: number,
  length?: number
): Widget;
export function Column(children: Widget[], gap?: number): Widget;
export function Row(children: Widget[], gap?: number): Widget;
export function ScrollView(child: Widget, axes?: ScrollAxes, name?: string): Widget;
export function ExternalSurface(
  texture: ExternalTextureDescriptor,
  desiredSize?: Size,
  name?: string
): Widget;
export function renderWidget(widget: Widget, event?: Event): RenderSnapshot;
