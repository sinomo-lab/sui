#![forbid(unsafe_code)]
// Conversion helpers borrow generated binding models, and binding constructors mirror the
// stable cross-language API rather than Rust-only builder conventions.
#![allow(clippy::too_many_arguments, clippy::wrong_self_convention)]

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

#[cfg(feature = "desktop")]
use sui::CommandKey;
use sui::containers::Padding as PaddingWidget;
use sui::{
    ActionCard, AdaptiveBreakpoints, AdaptiveClass, AdaptiveView, Align, Alignment, AnimatedValue,
    AnimationBinding, AnimationDocument, AnimationEditorCommand, AnimationEditorState,
    AnimationPlayer, AnimationProperty, AnimationPropertyPath, AnimationTargetId, AnimationValue,
    ArrangeCtx, AspectRatio, AspectRatioFit, Axis, Background, Border, Breadcrumb, BreadcrumbItem,
    BrowserTabBar, Brush, BrushPreview, BrushPreviewShape, BrushPreviewSpec, BusyIndicator, Button,
    Canvas, CanvasRuler, CanvasRulerAxis, CanvasShape, CanvasStroke, CanvasViewport, Checkbox,
    Clip, Color, ColorPalette, ColorPaletteSwatch, ColorPicker, ColorSpace, ColorSwatch,
    CommandGroup, CommandPalette, ConstraintOrientation, ConstraintQuery, ConstraintView,
    Constraints, ContextMenu, ControlSize, CoverageDots, CustomEvent, DateTimeInput, DefaultTheme,
    DetailRow, Dialog, Dock, DockFloatingGroup, DockNode, DockPanel, DockPanelId, DockWorkspace,
    DockWorkspaceSnapshot, DockWorkspaceState, DockZone, DpiInfo, DragDropHost, DragDropScope,
    DragEvent, DragPayload, Draggable, DropEffect, DropTarget, Easing, EmptyState, Event, EventCtx,
    EventPhase, FieldGroup, FixedPaneSplit, Flex, FloatingStack, FloatingViewConfig,
    FloatingViewSnapshot, FloatingWorkspace, FloatingWorkspaceState, FontHandle, FormRow,
    FormSection, FramedField, Grid, GridTrack, Icon, IconButton, IconGlyph, Image, ImageFit,
    ImageHandle, ImageSource, ImeEvent, Insets, InvalidationKind, InvalidationRequest,
    InvalidationTarget, KeyState, KeyboardEvent, Keyframe, Label, LayerList, LayerListItem,
    LayoutTransition, Link, ListItem, ListView, LoopMode, MasterDetail, MasterDetailRoute,
    MasterDetailState, MeasureCtx, MeasuredBottomDock, Menu, MenuItem, Modifiers,
    NotificationCenter, NotificationHost, NotificationId, NotificationUrgency, NumberInput,
    OverlayHost, PaintCtx, PanelSection, PasswordInput, Path, PixelCanvas, PixelCanvasBlendMode,
    PixelCanvasBrushShape, PixelCanvasExportSnapshot, PixelCanvasState, PixelCanvasTool,
    PlacementBadge, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    PointerKind, Popover, PresetStrip, ProgressBar, PropertyRow, RadioButton, RadioGroup,
    RawMouseMotionEvent, Rect, RegisteredFont, RegisteredImage, ReorderableList, ResponsiveSidebar,
    ResponsiveSidebarMode, ResponsiveSidebarState, RichAttachment, RichDocumentModel,
    RichDocumentStatus, RichDocumentUpdate, RichDocumentView, RichExtensionBlock, RichText,
    Runtime, SafeArea, SafeAreaEdges, SafeAreaInsets, SceneCommand, ScrollDelta, ScrollView,
    SectionLabel, SegmentedControl, SegmentedControlItem, Select, SemanticRegion, SemanticTone,
    SemanticsCtx, SemanticsNode, SemanticsRole, SemanticsValue, Separator, ShadowParams, SideSheet,
    SideSheetPlacement, SignalMeter, SimpleColorPicker, SimpleColorPickerMode, Size, SizedBox,
    Slider, SplitState, SplitView, SpringF32, Stack, StatusBadge, StatusBar, StatusBarHost,
    StatusBarSegment, StrokeStyle, Surface, SurfaceBorder, SurfaceElevation, SurfaceRole, Switch,
    SwitchView, TabBar, Table, TableColumn, TableColumnAlignment, TableRow, Tabs, TextArea,
    TextInput, TextSpan, TextStyle, Timeline, TimerToken, ToggleState, ToolPalette,
    ToolPaletteItem, Toolbar, Tooltip, TooltipPlacement, Track, TrailingSlotRow, Transform,
    TransientNotification, Transition, TreeItem, TreeView, Vector, VirtualCollectionModel,
    VirtualList, VirtualListChrome, VirtualListSelectionMode, VirtualScrollView, Widget, WidgetId,
    WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor, WidgetShader, WindowBuilder,
    WindowColorManagementMode, WindowDynamicRangeMode, WindowEvent, WindowId,
    WindowOutputColorPrimaries, WindowRenderOptions, WindowToneMappingMode,
};

#[cfg(feature = "desktop")]
use sui::{App as SuiApp, Window as SuiWindow};

static NEXT_FOREIGN_WIDGET_ID: AtomicU64 = AtomicU64::new(1);
const BINDING_APP_FONT_HANDLE_NAMESPACE: u64 = 1 << 60;
const BINDING_APP_FONT_SLOT_MASK: u64 = BINDING_APP_FONT_HANDLE_NAMESPACE - 1;
const BINDING_APP_IMAGE_HANDLE_NAMESPACE: u64 = 1 << 61;
const BINDING_APP_IMAGE_SLOT_MASK: u64 = BINDING_APP_IMAGE_HANDLE_NAMESPACE - 1;
const BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE: u64 = 1 << 62;
const BINDING_LOCAL_IMAGE_SLOT_MASK: u64 = BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE - 1;

type UiTask = Box<dyn FnOnce() + Send + 'static>;
type UiWake = Arc<dyn Fn() + Send + Sync + 'static>;

macro_rules! themed_widget {
    ($widget:expr, $context:expr) => {{
        let widget = $widget;
        if let Some(theme) = $context.theme.clone() {
            widget.theme_when(move || theme.snapshot())
        } else {
            widget
        }
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForeignWidgetId(u64);

impl ForeignWidgetId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for ForeignWidgetId {
    fn default() -> Self {
        Self::new(NEXT_FOREIGN_WIDGET_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackPhase {
    DebugName,
    Event,
    Measure,
    Arrange,
    Paint,
    Semantics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallbackFailure {
    message: String,
}

impl ForeignCallbackFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ForeignCallbackFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ForeignCallbackFailure {}

impl From<String> for ForeignCallbackFailure {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ForeignCallbackFailure {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<PaintValidationError> for ForeignCallbackFailure {
    fn from(value: PaintValidationError) -> Self {
        Self::new(value.to_string())
    }
}

pub type ForeignCallbackResult<T> = std::result::Result<T, ForeignCallbackFailure>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallbackError {
    pub widget_id: ForeignWidgetId,
    pub phase: ForeignCallbackPhase,
    pub message: String,
}

impl ForeignCallbackError {
    pub fn new(
        widget_id: ForeignWidgetId,
        phase: ForeignCallbackPhase,
        message: impl Into<String>,
    ) -> Self {
        Self {
            widget_id,
            phase,
            message: message.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ForeignErrorSink {
    errors: Arc<Mutex<Vec<ForeignCallbackError>>>,
}

impl ForeignErrorSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, error: ForeignCallbackError) {
        recover_lock(&self.errors).push(error);
    }

    pub fn drain(&self) -> Vec<ForeignCallbackError> {
        std::mem::take(&mut *recover_lock(&self.errors))
    }

    pub fn snapshot(&self) -> Vec<ForeignCallbackError> {
        recover_lock(&self.errors).clone()
    }

    pub fn is_empty(&self) -> bool {
        recover_lock(&self.errors).is_empty()
    }
}

impl fmt::Debug for ForeignErrorSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForeignErrorSink")
            .field("len", &recover_lock(&self.errors).len())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct UiTaskQueue {
    inner: Arc<UiTaskQueueInner>,
}

#[derive(Default)]
struct UiTaskQueueInner {
    tasks: Mutex<VecDeque<UiTask>>,
    wake: Mutex<Option<UiWake>>,
    draining_depth: AtomicUsize,
}

impl UiTaskQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_waker(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let queue = Self::new();
        queue.set_waker(wake);
        queue
    }

    pub fn handle(&self) -> BindingUiHandle {
        BindingUiHandle {
            inner: Arc::clone(&self.inner),
            messages: None,
        }
    }

    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        *recover_lock(&self.inner.wake) = Some(Arc::new(wake));
    }

    pub fn clear_waker(&self) {
        *recover_lock(&self.inner.wake) = None;
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        self.handle().post(task);
    }

    pub fn drain(&self) -> usize {
        self.inner.draining_depth.fetch_add(1, Ordering::SeqCst);
        let _guard = UiTaskDrainGuard { inner: &self.inner };
        let mut drained = 0;
        loop {
            let task = recover_lock(&self.inner.tasks).pop_front();
            let Some(task) = task else {
                break;
            };
            task();
            drained += 1;
        }
        drained
    }

    pub fn pending_count(&self) -> usize {
        recover_lock(&self.inner.tasks).len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }
}

struct UiTaskDrainGuard<'a> {
    inner: &'a UiTaskQueueInner,
}

impl Drop for UiTaskDrainGuard<'_> {
    fn drop(&mut self) {
        self.inner.draining_depth.fetch_sub(1, Ordering::SeqCst);
    }
}

impl fmt::Debug for UiTaskQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiTaskQueue")
            .field("pending_count", &self.pending_count())
            .finish()
    }
}

#[derive(Clone)]
pub struct BindingUiHandle {
    inner: Arc<UiTaskQueueInner>,
    messages: Option<BindingMessageBus>,
}

impl BindingUiHandle {
    fn with_message_bus(mut self, messages: BindingMessageBus) -> Self {
        self.messages = Some(messages);
        self
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        recover_lock(&self.inner.tasks).push_back(Box::new(task));
        let wake = recover_lock(&self.inner.wake).clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    pub fn pending_count(&self) -> usize {
        recover_lock(&self.inner.tasks).len()
    }

    pub fn emit(&self, name: impl Into<String>, payload: BindingValue) -> bool {
        let Some(messages) = &self.messages else {
            return false;
        };
        let name = name.into();
        let actions = messages.actions(&name);
        if actions.is_empty() {
            return false;
        }
        let errors = messages.errors.clone();
        self.post(move || {
            for action in actions {
                if let Err(error) = action.run(payload.clone()) {
                    errors.push(ForeignCallbackError::new(
                        ForeignWidgetId::new(0),
                        ForeignCallbackPhase::Event,
                        error.message,
                    ));
                }
            }
        });
        true
    }

    fn is_draining(&self) -> bool {
        self.inner.draining_depth.load(Ordering::SeqCst) > 0
    }
}

impl fmt::Debug for BindingUiHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingUiHandle")
            .field("pending_count", &self.pending_count())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingValue {
    String(String),
    Number(f64),
    Bool(bool),
}

#[derive(Clone)]
pub struct BindingMessageAction {
    callback: Arc<dyn Fn(BindingValue) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingMessageAction {
    pub fn new(
        callback: impl Fn(BindingValue) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, payload: BindingValue) -> ForeignCallbackResult<()> {
        (self.callback)(payload)
    }
}

impl fmt::Debug for BindingMessageAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingMessageAction")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
struct BindingMessageBus {
    handlers: Arc<Mutex<BTreeMap<String, Vec<BindingMessageAction>>>>,
    errors: ForeignErrorSink,
}

impl BindingMessageBus {
    fn on(&self, name: impl Into<String>, action: BindingMessageAction) {
        recover_lock(&self.handlers)
            .entry(name.into())
            .or_default()
            .push(action);
    }

    fn actions(&self, name: &str) -> Vec<BindingMessageAction> {
        recover_lock(&self.handlers)
            .get(name)
            .cloned()
            .unwrap_or_default()
    }
}

impl BindingValue {
    pub fn as_label_text(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => {
                let mut text = value.to_string();
                if text.ends_with(".0") {
                    text.truncate(text.len() - 2);
                }
                text
            }
            Self::Bool(value) => value.to_string(),
        }
    }
}

impl From<String> for BindingValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for BindingValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<f64> for BindingValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for BindingValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Live, thread-safe handle to SUI's built-in theme tokens.
///
/// Binding widgets capture this handle rather than a theme snapshot, so preset,
/// accent, and control-size changes propagate without rebuilding the foreign tree.
#[derive(Debug, Clone)]
pub struct BindingTheme {
    inner: Arc<BindingThemeInner>,
}

#[derive(Debug)]
struct BindingThemeInner {
    value: Mutex<DefaultTheme>,
    ui_handle: Mutex<Option<BindingUiHandle>>,
}

impl BindingTheme {
    pub fn preset(name: &str) -> Result<Self, String> {
        Ok(Self {
            inner: Arc::new(BindingThemeInner {
                value: Mutex::new(binding_theme_preset(name)?),
                ui_handle: Mutex::new(None),
            }),
        })
    }

    pub fn snapshot(&self) -> DefaultTheme {
        *recover_lock(&self.inner.value)
    }

    pub fn set_preset(&self, name: &str) -> Result<(), String> {
        self.publish(binding_theme_preset(name)?);
        Ok(())
    }

    pub fn set_accent(&self, color: Color) {
        let mut theme = self.snapshot();
        theme.colors.primary = color;
        theme.colors.accent = color;
        theme.sync_derived_fields();
        self.publish(theme);
    }

    pub fn accent(&self) -> Color {
        self.snapshot().palette.accent
    }

    pub fn set_control_size(&self, size: &str) -> Result<(), String> {
        let size = match normalized_option_name(size).as_str() {
            "small" | "compact" => ControlSize::Small,
            "medium" | "standard" => ControlSize::Medium,
            "large" | "touch" => ControlSize::Large,
            _ => {
                return Err(format!(
                    "control size must be 'small', 'medium', or 'large', got '{size}'"
                ));
            }
        };
        self.publish(self.snapshot().with_size(size));
        Ok(())
    }

    pub fn color(&self, name: &str) -> Result<Color, String> {
        let colors = self.snapshot().colors;
        match normalized_option_name(name).as_str() {
            "base100" | "background" => Ok(colors.base_100),
            "base200" | "surface" => Ok(colors.base_200),
            "base300" | "border" => Ok(colors.base_300),
            "basecontent" | "foreground" | "text" => Ok(colors.base_content),
            "primary" => Ok(colors.primary),
            "primarycontent" => Ok(colors.primary_content),
            "secondary" => Ok(colors.secondary),
            "secondarycontent" => Ok(colors.secondary_content),
            "accent" => Ok(colors.accent),
            "accentcontent" => Ok(colors.accent_content),
            "neutral" => Ok(colors.neutral),
            "neutralcontent" => Ok(colors.neutral_content),
            "info" => Ok(colors.info),
            "infocontent" => Ok(colors.info_content),
            "success" => Ok(colors.success),
            "successcontent" => Ok(colors.success_content),
            "warning" => Ok(colors.warning),
            "warningcontent" => Ok(colors.warning_content),
            "error" | "danger" => Ok(colors.error),
            "errorcontent" | "dangercontent" => Ok(colors.error_content),
            _ => Err(format!("unknown theme color token '{name}'")),
        }
    }

    pub fn set_color(&self, name: &str, color: Color) -> Result<(), String> {
        let mut theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "base100" | "background" => theme.colors.base_100 = color,
            "base200" | "surface" => theme.colors.base_200 = color,
            "base300" | "border" => theme.colors.base_300 = color,
            "basecontent" | "foreground" | "text" => theme.colors.base_content = color,
            "primary" => theme.colors.primary = color,
            "primarycontent" => theme.colors.primary_content = color,
            "secondary" => theme.colors.secondary = color,
            "secondarycontent" => theme.colors.secondary_content = color,
            "accent" => theme.colors.accent = color,
            "accentcontent" => theme.colors.accent_content = color,
            "neutral" => theme.colors.neutral = color,
            "neutralcontent" => theme.colors.neutral_content = color,
            "info" => theme.colors.info = color,
            "infocontent" => theme.colors.info_content = color,
            "success" => theme.colors.success = color,
            "successcontent" => theme.colors.success_content = color,
            "warning" => theme.colors.warning = color,
            "warningcontent" => theme.colors.warning_content = color,
            "error" | "danger" => theme.colors.error = color,
            "errorcontent" | "dangercontent" => theme.colors.error_content = color,
            _ => return Err(format!("unknown theme color token '{name}'")),
        }
        theme.sync_derived_fields();
        self.publish(theme);
        Ok(())
    }

    pub fn number(&self, name: &str) -> Result<f32, String> {
        let theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "spacing" => Ok(theme.spacing),
            "radiusxs" => Ok(theme.radius.xs),
            "radiussm" => Ok(theme.radius.sm),
            "radiusmd" => Ok(theme.radius.md),
            "radiuslg" => Ok(theme.radius.lg),
            "radiusxl" => Ok(theme.radius.xl),
            "breakpointsm" | "breakpointmedium" => Ok(theme.breakpoints.sm),
            "breakpointlg" | "breakpointexpanded" => Ok(theme.breakpoints.lg),
            "motionfast" => Ok(theme.motion.duration_fast),
            "motionnormal" => Ok(theme.motion.duration_normal),
            "motionslow" => Ok(theme.motion.duration_slow),
            "motionslower" => Ok(theme.motion.duration_slower),
            _ => Err(format!("unknown theme number token '{name}'")),
        }
    }

    pub fn set_number(&self, name: &str, value: f32) -> Result<(), String> {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "theme number token '{name}' must be finite and non-negative"
            ));
        }
        let mut theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "spacing" => theme.spacing = value,
            "radiusxs" => theme.radius.xs = value,
            "radiussm" => theme.radius.sm = value,
            "radiusmd" => theme.radius.md = value,
            "radiuslg" => theme.radius.lg = value,
            "radiusxl" => theme.radius.xl = value,
            "breakpointsm" | "breakpointmedium" => theme.breakpoints.sm = value,
            "breakpointlg" | "breakpointexpanded" => theme.breakpoints.lg = value,
            "motionfast" => theme.motion.duration_fast = value,
            "motionnormal" => theme.motion.duration_normal = value,
            "motionslow" => theme.motion.duration_slow = value,
            "motionslower" => theme.motion.duration_slower = value,
            _ => return Err(format!("unknown theme number token '{name}'")),
        }
        if matches!(
            normalized_option_name(name).as_str(),
            "spacing" | "radiusxs" | "radiussm" | "radiusmd" | "radiuslg" | "radiusxl"
        ) {
            theme.sync_derived_fields();
        }
        self.publish(theme);
        Ok(())
    }

    pub fn bind_ui_handle(&self, handle: BindingUiHandle) {
        *recover_lock(&self.inner.ui_handle) = Some(handle);
    }

    fn publish(&self, value: DefaultTheme) {
        if let Some(handle) = recover_lock(&self.inner.ui_handle).clone()
            && !handle.is_draining()
        {
            let theme = self.clone();
            handle.post(move || theme.publish_immediate(value));
        } else {
            self.publish_immediate(value);
        }
    }

    fn publish_immediate(&self, value: DefaultTheme) {
        *recover_lock(&self.inner.value) = value;
    }
}

fn binding_theme_preset(name: &str) -> Result<DefaultTheme, String> {
    match normalized_option_name(name).as_str() {
        "sui" | "light" | "default" => Ok(DefaultTheme::light()),
        "dark" => Ok(DefaultTheme::dark()),
        "neutral" | "neutrallight" => Ok(DefaultTheme::neutral()),
        "neutraldark" => Ok(DefaultTheme::neutral_dark()),
        "highcontrast" => Ok(DefaultTheme::high_contrast()),
        "oled" | "void" => Ok(DefaultTheme::void()),
        _ => Err(format!(
            "unknown theme preset '{name}'; expected light, dark, neutral, neutral-dark, high-contrast, or oled"
        )),
    }
}

#[derive(Clone)]
pub struct BindingRichDocument {
    inner: RichDocumentModel,
}

impl BindingRichDocument {
    pub fn new(markdown: impl Into<String>) -> Self {
        Self {
            inner: RichDocumentModel::from_markdown(markdown),
        }
    }

    pub fn revision(&self) -> u64 {
        self.inner.revision()
    }

    pub fn markdown(&self) -> String {
        self.inner.markdown()
    }

    pub fn set_markdown(&self, markdown: impl Into<String>) -> bool {
        self.inner.set_markdown(markdown)
    }

    pub fn append_markdown(&self, fragment: &str) -> bool {
        self.inner.append_markdown(fragment)
    }

    pub fn last_update(&self) -> BindingRichDocumentUpdate {
        self.inner.last_update().into()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_attachment(
        &self,
        name: impl Into<String>,
        media_type: Option<String>,
        source: Option<String>,
        size_bytes: Option<u64>,
        description: Option<String>,
    ) -> u64 {
        let mut attachment = RichAttachment::new(name);
        attachment.media_type = media_type;
        attachment.source = source;
        attachment.size_bytes = size_bytes;
        attachment.description = description;
        self.inner.append_attachment(attachment).get()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_extension(
        &self,
        renderer: impl Into<String>,
        title: impl Into<String>,
        summary: Option<String>,
        body: impl Into<String>,
        status: &str,
        initially_expanded: bool,
        metadata: Vec<(String, String)>,
    ) -> Result<u64, String> {
        let mut extension = RichExtensionBlock::new(renderer, title);
        extension.summary = summary;
        extension.body = body.into();
        extension.status = binding_rich_document_status(status)?;
        extension.initially_expanded = initially_expanded;
        extension.metadata = metadata;
        Ok(self.inner.append_extension(extension).get())
    }
}

impl fmt::Debug for BindingRichDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingRichDocument")
            .field("revision", &self.revision())
            .field("markdown_len", &self.markdown().len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRichDocumentUpdate {
    pub revision: u64,
    pub reparsed_start: usize,
    pub reparsed_end: usize,
    pub reused_prefix_blocks: usize,
    pub changed_block_ids: Vec<u64>,
    pub append_only: bool,
}

impl From<RichDocumentUpdate> for BindingRichDocumentUpdate {
    fn from(value: RichDocumentUpdate) -> Self {
        Self {
            revision: value.revision,
            reparsed_start: value.reparsed_source.start,
            reparsed_end: value.reparsed_source.end,
            reused_prefix_blocks: value.reused_prefix_blocks,
            changed_block_ids: value
                .changed_block_ids
                .into_iter()
                .map(|id| id.get())
                .collect(),
            append_only: value.append_only,
        }
    }
}

fn binding_rich_document_status(value: &str) -> Result<RichDocumentStatus, String> {
    match normalized_option_name(value).as_str() {
        "neutral" => Ok(RichDocumentStatus::Neutral),
        "pending" => Ok(RichDocumentStatus::Pending),
        "running" | "active" => Ok(RichDocumentStatus::Running),
        "success" | "complete" => Ok(RichDocumentStatus::Success),
        "warning" | "warn" => Ok(RichDocumentStatus::Warning),
        "error" | "failed" => Ok(RichDocumentStatus::Error),
        _ => Err(format!(
            "rich document status must be neutral, pending, running, success, warning, or error; got '{value}'"
        )),
    }
}

#[derive(Debug, Clone)]
pub struct BindingState {
    inner: Arc<BindingStateInner>,
}

#[derive(Debug)]
struct BindingStateInner {
    value: Mutex<BindingValue>,
    ui_handle: Mutex<Option<BindingUiHandle>>,
    observers: Mutex<BTreeMap<u64, BindingStateObserver>>,
    next_observer_id: AtomicU64,
    retained_subscriptions: Mutex<Vec<BindingStateSubscription>>,
}

#[derive(Clone)]
struct BindingStateObserver {
    callback: Arc<dyn Fn(BindingValue) + Send + Sync + 'static>,
}

impl fmt::Debug for BindingStateObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingStateObserver")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct BindingStateSubscription {
    source: Weak<BindingStateInner>,
    id: u64,
    active: bool,
}

impl BindingStateSubscription {
    pub fn unsubscribe(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.source
            .upgrade()
            .is_some_and(|source| recover_lock(&source.observers).remove(&self.id).is_some())
    }
}

impl Drop for BindingStateSubscription {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

impl BindingState {
    pub fn new(value: impl Into<BindingValue>) -> Self {
        Self {
            inner: Arc::new(BindingStateInner {
                value: Mutex::new(value.into()),
                ui_handle: Mutex::new(None),
                observers: Mutex::new(BTreeMap::new()),
                next_observer_id: AtomicU64::new(1),
                retained_subscriptions: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn get(&self) -> BindingValue {
        recover_lock(&self.inner.value).clone()
    }

    pub fn set(&self, value: impl Into<BindingValue>) {
        let value = value.into();
        if let Some(handle) = recover_lock(&self.inner.ui_handle).clone()
            && !handle.is_draining()
        {
            let state = self.clone();
            handle.post(move || state.set_immediate(value));
        } else {
            self.set_immediate(value);
        }
    }

    pub fn label_text(&self) -> String {
        self.get().as_label_text()
    }

    pub fn bind_ui_handle(&self, handle: BindingUiHandle) {
        *recover_lock(&self.inner.ui_handle) = Some(handle);
    }

    pub fn unbind_ui_handle(&self) {
        *recover_lock(&self.inner.ui_handle) = None;
    }

    pub fn is_ui_bound(&self) -> bool {
        recover_lock(&self.inner.ui_handle).is_some()
    }

    pub fn observe(
        &self,
        callback: impl Fn(BindingValue) + Send + Sync + 'static,
    ) -> BindingStateSubscription {
        let id = self
            .inner
            .next_observer_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        recover_lock(&self.inner.observers).insert(
            id,
            BindingStateObserver {
                callback: Arc::new(callback),
            },
        );
        BindingStateSubscription {
            source: Arc::downgrade(&self.inner),
            id,
            active: true,
        }
    }

    pub fn retain_subscription(&self, subscription: BindingStateSubscription) {
        recover_lock(&self.inner.retained_subscriptions).push(subscription);
    }

    fn set_immediate(&self, value: BindingValue) {
        {
            let mut current = recover_lock(&self.inner.value);
            if *current == value {
                return;
            }
            *current = value.clone();
        }
        let observers = recover_lock(&self.inner.observers)
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for observer in observers {
            (observer.callback)(value.clone());
        }
    }
}

#[derive(Clone)]
pub struct BindingAction {
    callback: Arc<dyn Fn() -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingAction {
    pub fn new(callback: impl Fn() -> ForeignCallbackResult<()> + Send + Sync + 'static) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self) -> ForeignCallbackResult<()> {
        (self.callback)()
    }
}

impl fmt::Debug for BindingAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingBoolAction {
    callback: Arc<dyn Fn(bool) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingBoolAction {
    pub fn new(
        callback: impl Fn(bool) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: bool) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingBoolAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingBoolAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingNumberAction {
    callback: Arc<dyn Fn(f64) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

#[derive(Clone)]
pub struct BindingIdAction {
    callback: Arc<dyn Fn(u64) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingIdAction {
    pub fn new(
        callback: impl Fn(u64) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: u64) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingIdAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingIdAction")
            .finish_non_exhaustive()
    }
}

impl BindingNumberAction {
    pub fn new(
        callback: impl Fn(f64) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: f64) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingNumberAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingNumberAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingReorderAction {
    callback: Arc<dyn Fn(usize, usize, usize) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingReorderAction {
    pub fn new(
        callback: impl Fn(usize, usize, usize) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, item: usize, from: usize, to: usize) -> ForeignCallbackResult<()> {
        (self.callback)(item, from, to)
    }
}

impl fmt::Debug for BindingReorderAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingReorderAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingStringAction {
    callback: Arc<dyn Fn(String) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

#[derive(Clone)]
pub struct BindingStringsAction {
    callback: Arc<dyn Fn(Vec<String>) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingStringsAction {
    pub fn new(
        callback: impl Fn(Vec<String>) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, values: Vec<String>) -> ForeignCallbackResult<()> {
        (self.callback)(values)
    }
}

impl fmt::Debug for BindingStringsAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingStringsAction")
            .finish_non_exhaustive()
    }
}

impl BindingStringAction {
    pub fn new(
        callback: impl Fn(String) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: String) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingStringAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingStringAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingSelectAction {
    callback: Arc<dyn Fn(usize, String) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingSelectAction {
    pub fn new(
        callback: impl Fn(usize, String) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, index: usize, value: String) -> ForeignCallbackResult<()> {
        (self.callback)(index, value)
    }
}

impl fmt::Debug for BindingSelectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingSelectAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingColorAction {
    callback: Arc<dyn Fn(Color) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingColorAction {
    pub fn new(
        callback: impl Fn(Color) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, color: Color) -> ForeignCallbackResult<()> {
        (self.callback)(color)
    }
}

impl fmt::Debug for BindingColorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingColorAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingColorSelectAction {
    callback:
        Arc<dyn Fn(usize, String, Color) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingColorSelectAction {
    pub fn new(
        callback: impl Fn(usize, String, Color) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, index: usize, name: String, color: Color) -> ForeignCallbackResult<()> {
        (self.callback)(index, name, color)
    }
}

impl fmt::Debug for BindingColorSelectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingColorSelectAction")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub enum BindingText {
    Static(String),
    State(BindingState),
}

impl BindingText {
    pub fn resolve(&self) -> String {
        match self {
            Self::Static(value) => value.clone(),
            Self::State(state) => state.label_text(),
        }
    }

    fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        if let Self::State(state) = self {
            state.bind_ui_handle(handle.clone());
        }
    }
}

impl From<String> for BindingText {
    fn from(value: String) -> Self {
        Self::Static(value)
    }
}

impl From<&str> for BindingText {
    fn from(value: &str) -> Self {
        Self::Static(value.to_owned())
    }
}

impl From<BindingState> for BindingText {
    fn from(value: BindingState) -> Self {
        Self::State(value)
    }
}

#[derive(Debug, Clone)]
pub enum BindingBool {
    Static(bool),
    State(BindingState),
}

impl BindingBool {
    pub fn resolve(&self) -> bool {
        match self {
            Self::Static(value) => *value,
            Self::State(state) => matches!(state.get(), BindingValue::Bool(true)),
        }
    }

    fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        if let Self::State(state) = self {
            state.bind_ui_handle(handle.clone());
        }
    }
}

impl From<bool> for BindingBool {
    fn from(value: bool) -> Self {
        Self::Static(value)
    }
}

impl From<BindingState> for BindingBool {
    fn from(value: BindingState) -> Self {
        Self::State(value)
    }
}

#[derive(Debug, Clone)]
pub enum BindingNumber {
    Static(f64),
    State(BindingState),
}

impl BindingNumber {
    pub fn resolve(&self) -> f64 {
        match self {
            Self::Static(value) => *value,
            Self::State(state) => match state.get() {
                BindingValue::Number(value) => value,
                BindingValue::Bool(value) => {
                    if value {
                        1.0
                    } else {
                        0.0
                    }
                }
                BindingValue::String(value) => value.parse::<f64>().unwrap_or(0.0),
            },
        }
    }

    fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        if let Self::State(state) = self {
            state.bind_ui_handle(handle.clone());
        }
    }
}

impl From<f64> for BindingNumber {
    fn from(value: f64) -> Self {
        Self::Static(value)
    }
}

impl From<BindingState> for BindingNumber {
    fn from(value: BindingState) -> Self {
        Self::State(value)
    }
}

#[derive(Debug, Clone)]
pub struct BindingTextSpan {
    pub text: String,
    pub style: TextStyle,
}

impl BindingTextSpan {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    fn into_sui(&self) -> TextSpan {
        TextSpan::new(self.text.clone(), self.style.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BindingStatusBarSegment {
    text: BindingText,
    tone: SemanticTone,
    min_width: Option<f32>,
    expand: bool,
}

impl BindingStatusBarSegment {
    pub fn new(
        text: impl Into<BindingText>,
        tone: SemanticTone,
        min_width: Option<f32>,
        expand: bool,
    ) -> Self {
        Self {
            text: text.into(),
            tone,
            min_width,
            expand,
        }
    }

    fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        self.text.bind_ui_handle(handle);
    }

    fn into_sui(&self) -> StatusBarSegment {
        let mut segment = if matches!(self.text, BindingText::State(_)) {
            StatusBarSegment::dynamic(self.text.resolve(), {
                let text = self.text.clone();
                move || text.resolve()
            })
        } else {
            StatusBarSegment::new(self.text.resolve())
        }
        .tone(self.tone)
        .expand(self.expand);
        if let Some(min_width) = self.min_width {
            segment = segment.min_width(min_width);
        }
        segment
    }
}

#[derive(Debug, Clone)]
pub struct BindingSegmentedControlItem {
    label: String,
    semantic_name: Option<String>,
    description: Option<String>,
    disabled: bool,
}

impl BindingSegmentedControlItem {
    pub fn new(
        label: impl Into<String>,
        semantic_name: Option<String>,
        description: Option<String>,
        disabled: bool,
    ) -> Self {
        Self {
            label: label.into(),
            semantic_name,
            description,
            disabled,
        }
    }

    fn into_sui(&self) -> SegmentedControlItem {
        let mut item = SegmentedControlItem::new(self.label.clone());
        if let Some(semantic_name) = &self.semantic_name {
            item = item.semantic_name(semantic_name.clone());
        }
        if let Some(description) = &self.description {
            item = item.description(description.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        item
    }
}

#[derive(Debug, Clone)]
pub struct BindingTableColumn {
    title: String,
    width: Option<f32>,
    min_width: Option<f32>,
    alignment: TableColumnAlignment,
    numeric: bool,
}

impl BindingTableColumn {
    pub fn new(
        title: impl Into<String>,
        width: Option<f32>,
        min_width: Option<f32>,
        alignment: TableColumnAlignment,
        numeric: bool,
    ) -> Self {
        Self {
            title: title.into(),
            width,
            min_width,
            alignment,
            numeric,
        }
    }

    fn into_sui(&self) -> TableColumn {
        let mut column = TableColumn::new(self.title.clone());
        if let Some(width) = self.width {
            column = column.width(width);
        }
        if let Some(min_width) = self.min_width {
            column = column.min_width(min_width);
        }
        if self.numeric {
            column = column.numeric();
        } else {
            column = column.alignment(self.alignment);
        }
        column
    }
}

#[derive(Debug, Clone)]
pub struct BindingTableRow {
    cells: Vec<String>,
}

impl BindingTableRow {
    pub fn new(cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }

    fn into_sui(&self) -> TableRow {
        TableRow::new(self.cells.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BindingTreeItem {
    label: String,
    detail: Option<String>,
    expanded: bool,
    disabled: bool,
    children: Vec<BindingTreeItem>,
}

impl BindingTreeItem {
    pub fn new(
        label: impl Into<String>,
        detail: Option<String>,
        expanded: bool,
        disabled: bool,
        children: impl IntoIterator<Item = BindingTreeItem>,
    ) -> Self {
        Self {
            label: label.into(),
            detail,
            expanded,
            disabled,
            children: children.into_iter().collect(),
        }
    }

    fn into_sui(&self) -> TreeItem {
        let mut item = TreeItem::new(self.label.clone()).expanded(self.expanded);
        if let Some(detail) = &self.detail {
            item = item.detail(detail.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        item.children(self.children.iter().map(BindingTreeItem::into_sui))
    }
}

#[derive(Debug, Clone)]
pub struct BindingLayerListItem {
    label: String,
    detail: Option<String>,
    visible: bool,
    locked: bool,
    disabled: bool,
}

impl BindingLayerListItem {
    pub fn new(
        label: impl Into<String>,
        detail: Option<String>,
        visible: bool,
        locked: bool,
        disabled: bool,
    ) -> Self {
        Self {
            label: label.into(),
            detail,
            visible,
            locked,
            disabled,
        }
    }

    fn into_sui(&self) -> LayerListItem {
        let mut item = LayerListItem::new(self.label.clone())
            .visible(self.visible)
            .locked(self.locked);
        if let Some(detail) = &self.detail {
            item = item.detail(detail.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        item
    }
}

#[derive(Debug, Clone)]
pub struct BindingMenuItem {
    label: String,
    shortcut: Option<String>,
    disabled: bool,
    destructive: bool,
    separator_before: bool,
    submenu: Vec<BindingMenuItem>,
}

impl BindingMenuItem {
    pub fn new(
        label: impl Into<String>,
        shortcut: Option<String>,
        disabled: bool,
        destructive: bool,
        separator_before: bool,
        submenu: Vec<BindingMenuItem>,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut,
            disabled,
            destructive,
            separator_before,
            submenu,
        }
    }

    fn into_sui(&self) -> MenuItem {
        let mut item = MenuItem::new(self.label.clone());
        if let Some(shortcut) = &self.shortcut {
            item = item.shortcut(shortcut.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        if self.destructive {
            item = item.destructive();
        }
        if self.separator_before {
            item = item.separator_before();
        }
        if !self.submenu.is_empty() {
            item = item.submenu(self.submenu.iter().map(BindingMenuItem::into_sui));
        }
        item
    }
}

#[derive(Debug, Clone)]
pub struct BindingToolPaletteItem {
    icon: IconGlyph,
    label: String,
    disabled: bool,
}

impl BindingToolPaletteItem {
    pub fn new(icon: IconGlyph, label: impl Into<String>, disabled: bool) -> Self {
        Self {
            icon,
            label: label.into(),
            disabled,
        }
    }

    fn into_sui(&self) -> ToolPaletteItem {
        let item = ToolPaletteItem::new(self.icon, self.label.clone());
        if self.disabled { item.disabled() } else { item }
    }
}

#[derive(Debug, Clone)]
pub struct BindingColorPaletteSwatch {
    name: String,
    color: Color,
}

impl BindingColorPaletteSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    fn into_sui(&self) -> ColorPaletteSwatch {
        ColorPaletteSwatch::new(self.name.clone(), self.color)
    }
}

fn binding_number_to_index(value: f64) -> Option<usize> {
    if value.is_finite() && value >= 0.0 {
        Some(value.floor() as usize)
    } else {
        None
    }
}

fn normalize_binding_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn binding_icon_glyph_from_name(value: &str) -> Option<IconGlyph> {
    match normalize_binding_name(value).as_str() {
        "add" | "plus" => Some(IconGlyph::Add),
        "remove" | "minus" => Some(IconGlyph::Remove),
        "check" => Some(IconGlyph::Check),
        "chevrondown" => Some(IconGlyph::ChevronDown),
        "chevronup" => Some(IconGlyph::ChevronUp),
        "chevronleft" => Some(IconGlyph::ChevronLeft),
        "chevronright" => Some(IconGlyph::ChevronRight),
        "close" | "x" => Some(IconGlyph::Close),
        "maximize" => Some(IconGlyph::Maximize),
        "restore" => Some(IconGlyph::Restore),
        "fitview" => Some(IconGlyph::FitView),
        "actualsize" => Some(IconGlyph::ActualSize),
        "morehorizontal" => Some(IconGlyph::MoreHorizontal),
        "morevertical" => Some(IconGlyph::MoreVertical),
        "search" => Some(IconGlyph::Search),
        "undo" => Some(IconGlyph::Undo),
        "redo" => Some(IconGlyph::Redo),
        "brush" => Some(IconGlyph::Brush),
        "eraser" => Some(IconGlyph::Eraser),
        "paintbucket" => Some(IconGlyph::PaintBucket),
        "hand" => Some(IconGlyph::Hand),
        "lock" => Some(IconGlyph::Lock),
        "unlock" => Some(IconGlyph::Unlock),
        "trash" => Some(IconGlyph::Trash),
        "download" => Some(IconGlyph::Download),
        "sparkles" => Some(IconGlyph::Sparkles),
        "chat" => Some(IconGlyph::Chat),
        "history" => Some(IconGlyph::History),
        "folder" => Some(IconGlyph::Folder),
        "file" => Some(IconGlyph::File),
        "filetext" => Some(IconGlyph::FileText),
        "link" => Some(IconGlyph::Link),
        "send" => Some(IconGlyph::Send),
        "arrowup" => Some(IconGlyph::ArrowUp),
        "stop" => Some(IconGlyph::Stop),
        "attach" => Some(IconGlyph::Attach),
        "hourglass" => Some(IconGlyph::Hourglass),
        "alert" => Some(IconGlyph::Alert),
        "storage" => Some(IconGlyph::Storage),
        "audiolines" => Some(IconGlyph::AudioLines),
        "mic" => Some(IconGlyph::Mic),
        "micoff" => Some(IconGlyph::MicOff),
        "camera" => Some(IconGlyph::Camera),
        "cameraoff" => Some(IconGlyph::CameraOff),
        "video" => Some(IconGlyph::Video),
        "videooff" => Some(IconGlyph::VideoOff),
        "phone" => Some(IconGlyph::Phone),
        "phoneoff" => Some(IconGlyph::PhoneOff),
        "monitor" => Some(IconGlyph::Monitor),
        "screenshare" => Some(IconGlyph::ScreenShare),
        _ => None,
    }
}

pub fn binding_icon_glyph_name(glyph: IconGlyph) -> &'static str {
    match glyph {
        IconGlyph::Add => "add",
        IconGlyph::Remove => "remove",
        IconGlyph::Check => "check",
        IconGlyph::ChevronDown => "chevron-down",
        IconGlyph::ChevronUp => "chevron-up",
        IconGlyph::ChevronLeft => "chevron-left",
        IconGlyph::ChevronRight => "chevron-right",
        IconGlyph::Close => "close",
        IconGlyph::Maximize => "maximize",
        IconGlyph::Restore => "restore",
        IconGlyph::FitView => "fit-view",
        IconGlyph::ActualSize => "actual-size",
        IconGlyph::MoreHorizontal => "more-horizontal",
        IconGlyph::MoreVertical => "more-vertical",
        IconGlyph::Search => "search",
        IconGlyph::Undo => "undo",
        IconGlyph::Redo => "redo",
        IconGlyph::Brush => "brush",
        IconGlyph::Eraser => "eraser",
        IconGlyph::PaintBucket => "paint-bucket",
        IconGlyph::Hand => "hand",
        IconGlyph::Lock => "lock",
        IconGlyph::Unlock => "unlock",
        IconGlyph::Trash => "trash",
        IconGlyph::Download => "download",
        IconGlyph::Sparkles => "sparkles",
        IconGlyph::Chat => "chat",
        IconGlyph::History => "history",
        IconGlyph::Folder => "folder",
        IconGlyph::File => "file",
        IconGlyph::FileText => "file-text",
        IconGlyph::Link => "link",
        IconGlyph::Send => "send",
        IconGlyph::ArrowUp => "arrow-up",
        IconGlyph::Stop => "stop",
        IconGlyph::Attach => "attach",
        IconGlyph::Hourglass => "hourglass",
        IconGlyph::Alert => "alert",
        IconGlyph::Storage => "storage",
        IconGlyph::AudioLines => "audio-lines",
        IconGlyph::Mic => "mic",
        IconGlyph::MicOff => "mic-off",
        IconGlyph::Camera => "camera",
        IconGlyph::CameraOff => "camera-off",
        IconGlyph::Video => "video",
        IconGlyph::VideoOff => "video-off",
        IconGlyph::Phone => "phone",
        IconGlyph::PhoneOff => "phone-off",
        IconGlyph::Monitor => "monitor",
        IconGlyph::ScreenShare => "screen-share",
    }
}

pub fn binding_surface_role_from_name(value: &str) -> Option<SurfaceRole> {
    match normalize_binding_name(value).as_str() {
        "window" => Some(SurfaceRole::Window),
        "sidebar" | "side" => Some(SurfaceRole::Sidebar),
        "panel" => Some(SurfaceRole::Panel),
        "titlebar" | "title" => Some(SurfaceRole::Titlebar),
        "field" => Some(SurfaceRole::Field),
        _ => None,
    }
}

pub fn binding_surface_border_from_name(value: &str) -> Option<SurfaceBorder> {
    match normalize_binding_name(value).as_str() {
        "none" | "false" | "off" => Some(SurfaceBorder::None),
        "all" | "true" | "on" => Some(SurfaceBorder::All),
        "top" => Some(SurfaceBorder::Top),
        "right" => Some(SurfaceBorder::Right),
        "bottom" => Some(SurfaceBorder::Bottom),
        "left" => Some(SurfaceBorder::Left),
        _ => None,
    }
}

pub fn binding_surface_elevation_from_name(value: &str) -> Option<SurfaceElevation> {
    match normalize_binding_name(value).as_str() {
        "none" | "flat" => Some(SurfaceElevation::None),
        "small" | "sm" => Some(SurfaceElevation::Small),
        "medium" | "md" => Some(SurfaceElevation::Medium),
        "large" | "lg" => Some(SurfaceElevation::Large),
        _ => None,
    }
}

pub fn binding_alignment_from_name(value: &str) -> Option<Alignment> {
    match normalize_binding_name(value).as_str() {
        "start" | "left" | "top" => Some(Alignment::Start),
        "center" | "centre" | "middle" => Some(Alignment::Center),
        "end" | "right" | "bottom" => Some(Alignment::End),
        "stretch" | "fill" => Some(Alignment::Stretch),
        _ => None,
    }
}

pub fn binding_tooltip_placement_from_name(value: &str) -> Option<TooltipPlacement> {
    match normalize_binding_name(value).as_str() {
        "above" | "top" => Some(TooltipPlacement::Above),
        "below" | "bottom" => Some(TooltipPlacement::Below),
        _ => None,
    }
}

pub fn binding_semantic_tone_from_name(value: &str) -> Option<SemanticTone> {
    match normalize_binding_name(value).as_str() {
        "neutral" => Some(SemanticTone::Neutral),
        "accent" | "primary" => Some(SemanticTone::Accent),
        "info" | "information" => Some(SemanticTone::Info),
        "success" | "ok" => Some(SemanticTone::Success),
        "warning" | "warn" => Some(SemanticTone::Warning),
        "danger" | "error" | "critical" => Some(SemanticTone::Danger),
        _ => None,
    }
}

pub fn binding_simple_color_picker_mode_from_name(value: &str) -> Option<SimpleColorPickerMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hsl" => Some(SimpleColorPickerMode::Hsl),
        "hsv" | "hsb" => Some(SimpleColorPickerMode::Hsv),
        "rgb" => Some(SimpleColorPickerMode::Rgb),
        _ => None,
    }
}

pub fn binding_aspect_ratio_fit_from_name(value: &str) -> Option<AspectRatioFit> {
    match normalized_option_name(value).as_str() {
        "contain" => Some(AspectRatioFit::Contain),
        "cover" => Some(AspectRatioFit::Cover),
        _ => None,
    }
}

pub fn binding_safe_area_edges_from_name(value: &str) -> Option<SafeAreaEdges> {
    let normalized = normalized_option_name(value);
    match normalized.as_str() {
        "none" => return Some(SafeAreaEdges::NONE),
        "all" | "" => return Some(SafeAreaEdges::ALL),
        "horizontal" => return Some(SafeAreaEdges::HORIZONTAL),
        "vertical" => return Some(SafeAreaEdges::VERTICAL),
        _ => {}
    }
    let mut edges = SafeAreaEdges::NONE;
    for edge in value.split([',', '|', ' ']).filter(|edge| !edge.is_empty()) {
        edges = edges.union(match normalized_option_name(edge).as_str() {
            "left" => SafeAreaEdges::LEFT,
            "top" => SafeAreaEdges::TOP,
            "right" => SafeAreaEdges::RIGHT,
            "bottom" => SafeAreaEdges::BOTTOM,
            _ => return None,
        });
    }
    Some(edges)
}

pub fn binding_easing_from_name(value: &str) -> Option<Easing> {
    match normalized_option_name(value).as_str() {
        "linear" => Some(Easing::Linear),
        "easein" => Some(Easing::EaseIn),
        "easeout" => Some(Easing::EaseOut),
        "easeinout" => Some(Easing::EaseInOut),
        _ => None,
    }
}

pub fn binding_animation_property_from_path(path: &str) -> AnimationProperty {
    match normalize_binding_name(path).as_str() {
        "layeropacity" | "opacity" => AnimationProperty::LayerOpacity,
        "layertranslation" | "translation" => AnimationProperty::LayerTranslation,
        "fillcolor" | "color" => AnimationProperty::FillColor,
        "bounds" => AnimationProperty::Bounds,
        _ => AnimationProperty::Custom(AnimationPropertyPath::new(path)),
    }
}

/// A language-neutral animation value. Host bindings expose named constructors instead of the
/// Rust enum so Python and JavaScript callers can work with ordinary geometry and color objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingAnimationValue {
    Scalar(f32),
    Point(Point),
    Vector(Vector),
    Size(Size),
    Rect(Rect),
    Color(Color),
    Transform(Transform),
}

impl BindingAnimationValue {
    pub const fn scalar(value: f32) -> Self {
        Self::Scalar(value)
    }

    pub const fn point(value: Point) -> Self {
        Self::Point(value)
    }

    pub const fn vector(value: Vector) -> Self {
        Self::Vector(value)
    }

    pub const fn size(value: Size) -> Self {
        Self::Size(value)
    }

    pub const fn rect(value: Rect) -> Self {
        Self::Rect(value)
    }

    pub const fn color(value: Color) -> Self {
        Self::Color(value)
    }

    pub const fn transform(value: Transform) -> Self {
        Self::Transform(value)
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Point(_) => "point",
            Self::Vector(_) => "vector",
            Self::Size(_) => "size",
            Self::Rect(_) => "rect",
            Self::Color(_) => "color",
            Self::Transform(_) => "transform",
        }
    }

    pub const fn as_scalar(&self) -> Option<f32> {
        match self {
            Self::Scalar(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_point(&self) -> Option<Point> {
        match self {
            Self::Point(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_vector(&self) -> Option<Vector> {
        match self {
            Self::Vector(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_size(&self) -> Option<Size> {
        match self {
            Self::Size(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_rect(&self) -> Option<Rect> {
        match self {
            Self::Rect(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_transform(&self) -> Option<Transform> {
        match self {
            Self::Transform(value) => Some(*value),
            _ => None,
        }
    }
}

impl From<BindingAnimationValue> for AnimationValue {
    fn from(value: BindingAnimationValue) -> Self {
        match value {
            BindingAnimationValue::Scalar(value) => Self::Scalar(value),
            BindingAnimationValue::Point(value) => Self::Point(value),
            BindingAnimationValue::Vector(value) => Self::Vector(value),
            BindingAnimationValue::Size(value) => Self::Size(value),
            BindingAnimationValue::Rect(value) => Self::Rect(value),
            BindingAnimationValue::Color(value) => Self::Color(value),
            BindingAnimationValue::Transform(value) => Self::Transform(value),
        }
    }
}

impl From<AnimationValue> for BindingAnimationValue {
    fn from(value: AnimationValue) -> Self {
        match value {
            AnimationValue::Scalar(value) => Self::Scalar(value),
            AnimationValue::Point(value) => Self::Point(value),
            AnimationValue::Vector(value) => Self::Vector(value),
            AnimationValue::Size(value) => Self::Size(value),
            AnimationValue::Rect(value) => Self::Rect(value),
            AnimationValue::Color(value) => Self::Color(value),
            AnimationValue::Transform(value) => Self::Transform(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingTransition {
    inner: Transition<AnimationValue>,
}

impl BindingTransition {
    pub fn new(
        start: BindingAnimationValue,
        end: BindingAnimationValue,
        start_time: f64,
        duration: f64,
        easing: Easing,
    ) -> Self {
        Self {
            inner: Transition::new(start.into(), end.into(), start_time, duration, easing),
        }
    }

    pub fn progress(&self, time: f64) -> f32 {
        self.inner.progress(time)
    }

    pub fn sample(&self, time: f64) -> BindingAnimationValue {
        self.inner.sample(time).into()
    }

    pub fn is_complete(&self, time: f64) -> bool {
        self.inner.is_complete(time)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingSpring {
    inner: SpringF32,
}

impl BindingSpring {
    pub fn new(value: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            inner: SpringF32::new(value).with_config(stiffness, damping),
        }
    }

    pub fn step(&mut self, target: f32, delta_seconds: f64) -> f32 {
        self.inner.step(target, delta_seconds)
    }

    pub const fn value(&self) -> f32 {
        self.inner.value
    }

    pub const fn velocity(&self) -> f32 {
        self.inner.velocity
    }

    pub const fn stiffness(&self) -> f32 {
        self.inner.stiffness
    }

    pub const fn damping(&self) -> f32 {
        self.inner.damping
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingAnimatedValue {
    inner: AnimatedValue<AnimationValue>,
}

impl BindingAnimatedValue {
    pub fn new(initial: BindingAnimationValue, duration: f32, easing: Easing) -> Self {
        Self {
            inner: AnimatedValue::new(initial.into())
                .with_duration(duration)
                .with_easing(easing),
        }
    }

    pub fn set_duration(&mut self, seconds: f32) {
        self.inner.set_duration(seconds);
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.inner.set_easing(easing);
    }

    pub fn set_target(&mut self, target: BindingAnimationValue) {
        self.inner.set_target(target.into());
    }

    pub fn jump_to(&mut self, value: BindingAnimationValue) {
        self.inner.jump_to(value.into());
    }

    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        self.inner.tick(delta_seconds)
    }

    pub fn value(&self) -> BindingAnimationValue {
        self.inner.value().into()
    }

    pub fn target(&self) -> BindingAnimationValue {
        self.inner.target().into()
    }

    pub fn is_animating(&self) -> bool {
        self.inner.is_animating()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingAnimationKeyframe {
    inner: Keyframe<AnimationValue>,
}

impl BindingAnimationKeyframe {
    pub fn new(time: f64, value: BindingAnimationValue, easing: Easing) -> Self {
        Self {
            inner: Keyframe::new(time, value.into()).with_easing(easing),
        }
    }

    pub const fn time(&self) -> f64 {
        self.inner.time
    }

    pub fn value(&self) -> BindingAnimationValue {
        self.inner.value.into()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationTrack {
    inner: Track<AnimationValue>,
}

impl BindingAnimationTrack {
    pub fn new(target: impl Into<String>, property: impl Into<String>) -> Self {
        let target = target.into();
        let property = property.into();
        Self {
            inner: Track::new(AnimationBinding::new(
                AnimationTargetId::new(target),
                binding_animation_property_from_path(&property),
            )),
        }
    }

    pub fn add_keyframe(&mut self, keyframe: BindingAnimationKeyframe) {
        self.inner.push_keyframe(keyframe.inner);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.enabled = enabled;
    }

    pub fn sample(&self, time: f64) -> Option<BindingAnimationValue> {
        self.inner.sample(time).map(Into::into)
    }

    pub fn target(&self) -> &str {
        self.inner.binding.target.as_str()
    }

    pub fn property(&self) -> &str {
        self.inner.binding.property.path()
    }

    pub fn keyframe_count(&self) -> usize {
        self.inner.keyframes.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationClip {
    inner: Clip<AnimationValue>,
}

impl BindingAnimationClip {
    pub fn new(id: impl Into<String>, start_time: f64, duration: f64) -> Self {
        Self {
            inner: Clip::new(id, start_time, duration),
        }
    }

    pub fn add_track(&mut self, track: BindingAnimationTrack) {
        self.inner.push_track(track.inner);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.enabled = enabled;
    }

    pub fn id(&self) -> &str {
        &self.inner.id
    }

    pub fn start_time(&self) -> f64 {
        self.inner.start_time
    }

    pub fn duration(&self) -> f64 {
        self.inner.duration
    }

    pub fn track_count(&self) -> usize {
        self.inner.tracks.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationSample {
    pub clip_id: String,
    pub target: String,
    pub property: String,
    pub time: f64,
    pub value: BindingAnimationValue,
}

fn binding_animation_samples(
    samples: Vec<sui::SampledAnimationValue>,
) -> Vec<BindingAnimationSample> {
    samples
        .into_iter()
        .map(|sample| BindingAnimationSample {
            clip_id: sample.clip_id,
            target: sample.binding.target.as_str().to_owned(),
            property: sample.binding.property.path().to_owned(),
            time: sample.time,
            value: sample.value.into(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationTimeline {
    inner: Timeline<AnimationValue>,
}

impl BindingAnimationTimeline {
    pub fn new(duration: f64) -> Self {
        Self {
            inner: Timeline::new(duration),
        }
    }

    pub fn add_clip(&mut self, clip: BindingAnimationClip) {
        self.inner.push_clip(clip.inner);
    }

    pub fn duration(&self) -> f64 {
        self.inner.duration
    }

    pub fn clip_count(&self) -> usize {
        self.inner.clips.len()
    }

    pub fn sample(&self, time: f64) -> Vec<BindingAnimationSample> {
        binding_animation_samples(self.inner.sample(time))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationPlayer {
    inner: AnimationPlayer<AnimationValue>,
}

impl BindingAnimationPlayer {
    pub fn new(timeline: &BindingAnimationTimeline) -> Self {
        Self {
            inner: AnimationPlayer::from_compiled(timeline.inner.compile()),
        }
    }

    pub fn play(&mut self) {
        self.inner.play();
    }

    pub fn pause(&mut self) {
        self.inner.pause();
    }

    pub fn stop(&mut self) {
        self.inner.stop();
    }

    pub fn seek(&mut self, time: f64) {
        self.inner.seek(time);
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        self.inner.playback_mut().loop_mode = if repeat {
            LoopMode::Repeat
        } else {
            LoopMode::Once
        };
    }

    pub fn set_playback_rate(&mut self, rate: f64) {
        self.inner.playback_mut().playback_rate = rate;
    }

    pub fn playhead(&self) -> f64 {
        self.inner.playback().playhead
    }

    pub fn is_playing(&self) -> bool {
        self.inner.playback().playing
    }

    pub fn sample(&self) -> Vec<BindingAnimationSample> {
        binding_animation_samples(self.inner.sample())
    }

    pub fn tick(&mut self, delta_seconds: f64) -> Vec<BindingAnimationSample> {
        let duration = self.inner.timeline().duration();
        self.inner.playback_mut().tick(delta_seconds, duration);
        self.sample()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationDocument {
    inner: AnimationDocument,
}

impl BindingAnimationDocument {
    pub fn new(name: impl Into<String>, timeline: BindingAnimationTimeline) -> Self {
        Self {
            inner: AnimationDocument::new(name, timeline.inner),
        }
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        AnimationDocument::from_document_format(input)
            .map(|inner| Self { inner })
            .map_err(|error| error.to_string())
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn timeline(&self) -> BindingAnimationTimeline {
        BindingAnimationTimeline {
            inner: self.inner.timeline.clone(),
        }
    }

    pub fn to_document_format(&self) -> String {
        self.inner.to_document_format()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationEditor {
    inner: AnimationEditorState,
}

impl BindingAnimationEditor {
    pub fn new(document: BindingAnimationDocument) -> Self {
        Self {
            inner: AnimationEditorState::new(document.inner),
        }
    }

    pub fn document(&self) -> BindingAnimationDocument {
        BindingAnimationDocument {
            inner: self.inner.document.clone(),
        }
    }

    pub fn set_playhead(&mut self, time: f64) {
        self.inner
            .apply_command(AnimationEditorCommand::SetPlayhead(time));
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.inner
            .apply_command(AnimationEditorCommand::SetZoom(zoom));
    }

    pub fn set_scroll(&mut self, scroll: f32) {
        self.inner
            .apply_command(AnimationEditorCommand::SetScroll(scroll));
    }

    pub fn set_snapping(&mut self, enabled: bool, interval: f64) {
        self.inner
            .apply_command(AnimationEditorCommand::SetSnapping(if enabled {
                sui::TimelineSnap::new(interval)
            } else {
                sui::TimelineSnap::disabled()
            }));
    }

    pub fn add_keyframe(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe: BindingAnimationKeyframe,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::AddKeyframe {
                clip_index,
                track_index,
                keyframe: keyframe.inner,
            })
    }

    pub fn update_keyframe_easing(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe_index: usize,
        easing: Easing,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection: sui::KeyframeSelection {
                    clip_index,
                    track_index,
                    keyframe_index,
                },
                easing,
            })
    }

    pub fn remove_keyframe(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe_index: usize,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::RemoveKeyframe(
                sui::KeyframeSelection {
                    clip_index,
                    track_index,
                    keyframe_index,
                },
            ))
    }

    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn playhead(&self) -> f64 {
        self.inner.playback.playhead
    }

    pub fn zoom(&self) -> f32 {
        self.inner.zoom
    }

    pub fn scroll(&self) -> f32 {
        self.inner.scroll
    }
}

pub fn binding_table_column_alignment_from_name(value: &str) -> Option<TableColumnAlignment> {
    match normalize_binding_name(value).as_str() {
        "start" | "left" => Some(TableColumnAlignment::Start),
        "center" | "centre" | "middle" => Some(TableColumnAlignment::Center),
        "end" | "right" => Some(TableColumnAlignment::End),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    None,
}

impl From<BindingImageFit> for ImageFit {
    fn from(value: BindingImageFit) -> Self {
        match value {
            BindingImageFit::Fill => Self::Fill,
            BindingImageFit::Contain => Self::Contain,
            BindingImageFit::Cover => Self::Cover,
            BindingImageFit::None => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingScrollAxes {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

/// Portable brush metadata used by [`BindingWidget::brush_preview`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingBrushPreviewSpec {
    color: Color,
    size: f32,
    opacity: f32,
    shape: BrushPreviewShape,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingCanvasViewport {
    pub pan: Vector,
    pub zoom: f32,
    pub rotation: f32,
}

impl BindingCanvasViewport {
    pub fn new(pan: Vector, zoom: f32, rotation: f32) -> Self {
        Self {
            pan,
            zoom: zoom.max(0.01),
            rotation,
        }
    }

    fn into_sui(self) -> CanvasViewport {
        CanvasViewport::new()
            .pan(self.pan)
            .zoom(self.zoom)
            .rotation(self.rotation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingCanvasStroke {
    pub color: Color,
    pub width: f32,
}

impl BindingCanvasStroke {
    pub fn new(color: Color, width: f32) -> Self {
        Self {
            color,
            width: width.max(0.1),
        }
    }

    fn into_sui(self) -> CanvasStroke {
        CanvasStroke::new(self.color, self.width)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingCanvasShape {
    inner: CanvasShape,
}

impl BindingCanvasShape {
    pub fn path(path: Path, fill: Option<Color>, stroke: Option<BindingCanvasStroke>) -> Self {
        Self {
            inner: CanvasShape::Path {
                path,
                fill,
                stroke: stroke.map(BindingCanvasStroke::into_sui),
            },
        }
    }

    pub fn rect(rect: Rect, fill: Option<Color>, stroke: Option<BindingCanvasStroke>) -> Self {
        Self {
            inner: CanvasShape::rect(rect, fill, stroke.map(BindingCanvasStroke::into_sui)),
        }
    }

    pub fn circle(
        center: Point,
        radius: f32,
        fill: Option<Color>,
        stroke: Option<BindingCanvasStroke>,
    ) -> Self {
        Self {
            inner: CanvasShape::circle(
                center,
                radius,
                fill,
                stroke.map(BindingCanvasStroke::into_sui),
            ),
        }
    }

    pub fn polyline(points: &[Point], stroke: BindingCanvasStroke) -> Result<Self, String> {
        CanvasShape::polyline(points, stroke.into_sui())
            .map(|inner| Self { inner })
            .ok_or_else(|| "canvas polyline requires at least two distinct points".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BindingPixelCanvasExport {
    pub revision: u64,
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub rgba8: Vec<u8>,
}

impl From<PixelCanvasExportSnapshot> for BindingPixelCanvasExport {
    fn from(value: PixelCanvasExportSnapshot) -> Self {
        Self {
            revision: value.revision(),
            name: value.name().to_owned(),
            width: value.width(),
            height: value.height(),
            rgba8: value.rgba8().to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingPixelCanvasState {
    inner: PixelCanvasState,
}

impl BindingPixelCanvasState {
    pub fn new() -> Self {
        Self {
            inner: PixelCanvasState::new(),
        }
    }

    pub fn tool(&self) -> &'static str {
        match self.inner.tool() {
            PixelCanvasTool::Brush => "brush",
            PixelCanvasTool::Eraser => "eraser",
            PixelCanvasTool::Fill => "fill",
            PixelCanvasTool::Pan => "pan",
        }
    }

    pub fn set_tool(&self, value: &str) -> Result<(), String> {
        self.inner
            .set_tool(match normalized_option_name(value).as_str() {
                "brush" | "paint" => PixelCanvasTool::Brush,
                "eraser" | "erase" => PixelCanvasTool::Eraser,
                "fill" | "bucket" => PixelCanvasTool::Fill,
                "pan" | "hand" => PixelCanvasTool::Pan,
                _ => {
                    return Err(format!(
                        "pixel canvas tool must be brush, eraser, fill, or pan; got '{value}'"
                    ));
                }
            });
        Ok(())
    }

    pub fn brush_color(&self) -> Color {
        self.inner.brush_color()
    }

    pub fn set_brush_color(&self, color: Color) {
        self.inner.set_brush_color(color);
    }

    pub fn brush_size(&self) -> f32 {
        self.inner.brush_size()
    }

    pub fn set_brush_size(&self, size: f32) {
        self.inner.set_brush_size(size);
    }

    pub fn brush_opacity(&self) -> f32 {
        self.inner.brush_opacity()
    }

    pub fn set_brush_opacity(&self, opacity: f32) {
        self.inner.set_brush_opacity(opacity);
    }

    pub fn brush_shape(&self) -> &'static str {
        match self.inner.brush_shape() {
            PixelCanvasBrushShape::Square => "square",
            PixelCanvasBrushShape::Round => "round",
        }
    }

    pub fn set_brush_shape(&self, value: &str) -> Result<(), String> {
        self.inner
            .set_brush_shape(match normalized_option_name(value).as_str() {
                "square" => PixelCanvasBrushShape::Square,
                "round" | "circle" => PixelCanvasBrushShape::Round,
                _ => {
                    return Err(format!(
                        "pixel brush shape must be square or round; got '{value}'"
                    ));
                }
            });
        Ok(())
    }

    pub fn blend_mode(&self) -> &'static str {
        binding_pixel_blend_mode_name(self.inner.blend_mode())
    }

    pub fn set_blend_mode(&self, value: &str) -> Result<(), String> {
        self.inner.set_blend_mode(binding_pixel_blend_mode(value)?);
        Ok(())
    }

    pub fn editable(&self) -> bool {
        self.inner.is_editable()
    }

    pub fn set_editable(&self, editable: bool) -> bool {
        self.inner.set_editable(editable)
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn can_clear(&self) -> bool {
        self.inner.can_clear()
    }

    pub fn request_undo(&self) {
        self.inner.request_undo();
    }

    pub fn request_redo(&self) {
        self.inner.request_redo();
    }

    pub fn request_clear(&self) {
        self.inner.request_clear();
    }

    pub fn request_fit_view(&self) {
        self.inner.request_fit_view();
    }

    pub fn request_actual_size(&self) {
        self.inner.request_actual_size_view();
    }

    pub fn request_zoom_in(&self) {
        self.inner.request_zoom_in();
    }

    pub fn request_zoom_out(&self) {
        self.inner.request_zoom_out();
    }

    pub fn request_export(&self) {
        self.inner.request_export_snapshot();
    }

    pub fn latest_export(&self) -> Option<BindingPixelCanvasExport> {
        self.inner.latest_export_snapshot().map(Into::into)
    }
}

impl Default for BindingPixelCanvasState {
    fn default() -> Self {
        Self::new()
    }
}

fn binding_pixel_blend_mode(value: &str) -> Result<PixelCanvasBlendMode, String> {
    match normalized_option_name(value).as_str() {
        "normal" => Ok(PixelCanvasBlendMode::Normal),
        "multiply" => Ok(PixelCanvasBlendMode::Multiply),
        "screen" => Ok(PixelCanvasBlendMode::Screen),
        "overlay" => Ok(PixelCanvasBlendMode::Overlay),
        _ => Err(format!(
            "pixel blend mode must be normal, multiply, screen, or overlay; got '{value}'"
        )),
    }
}

fn binding_pixel_blend_mode_name(value: PixelCanvasBlendMode) -> &'static str {
    match value {
        PixelCanvasBlendMode::Normal => "normal",
        PixelCanvasBlendMode::Multiply => "multiply",
        PixelCanvasBlendMode::Screen => "screen",
        PixelCanvasBlendMode::Overlay => "overlay",
    }
}

impl BindingBrushPreviewSpec {
    pub fn new(color: Color, size: f32, opacity: f32, shape: BrushPreviewShape) -> Self {
        Self {
            color,
            size,
            opacity,
            shape,
        }
    }

    fn into_sui(self) -> BrushPreviewSpec {
        BrushPreviewSpec::new(self.color, self.size, self.opacity, self.shape)
    }
}

#[derive(Debug, Clone)]
pub struct BindingFloatingStackWindow {
    bounds: Rect,
    child: BindingWidget,
}

impl BindingFloatingStackWindow {
    pub fn new(bounds: Rect, child: BindingWidget) -> Self {
        Self { bounds, child }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BindingDockNode {
    inner: DockNode,
}

impl BindingDockNode {
    pub fn empty() -> Self {
        Self {
            inner: DockNode::empty(),
        }
    }

    pub fn tabs(
        panel_ids: impl IntoIterator<Item = u64>,
        active: Option<u64>,
    ) -> Result<Self, String> {
        let panel_ids = panel_ids.into_iter().collect::<Vec<_>>();
        if panel_ids.is_empty() {
            return Err("dock tabs require at least one panel id".to_string());
        }
        if panel_ids.iter().any(|id| *id == 0) {
            return Err("dock panel ids must be non-zero".to_string());
        }
        let active = active.unwrap_or(panel_ids[0]);
        if !panel_ids.contains(&active) {
            return Err(format!(
                "active dock panel {active} is not present in the tab group"
            ));
        }
        Ok(Self {
            inner: DockNode::tabs(
                panel_ids.into_iter().map(DockPanelId::new),
                DockPanelId::new(active),
            ),
        })
    }

    pub fn split(axis: Axis, fraction: f32, first: Self, second: Self) -> Result<Self, String> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err("dock split fraction must be finite and between 0 and 1".to_string());
        }
        Ok(Self {
            inner: DockNode::split(axis, fraction, first.inner, second.inner),
        })
    }

    fn into_sui(self) -> DockNode {
        self.inner
    }

    fn from_sui(inner: DockNode) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingDockFloatingGroup {
    pub id: u64,
    pub panel_ids: Vec<u64>,
    pub active: u64,
    pub bounds: Rect,
}

impl BindingDockFloatingGroup {
    pub fn new(
        id: u64,
        panel_ids: impl IntoIterator<Item = u64>,
        active: u64,
        bounds: Rect,
    ) -> Self {
        Self {
            id,
            panel_ids: panel_ids.into_iter().collect(),
            active,
            bounds,
        }
    }

    fn into_sui(self) -> DockFloatingGroup {
        DockFloatingGroup::new(
            self.id,
            self.panel_ids.into_iter().map(DockPanelId::new),
            DockPanelId::new(self.active),
            self.bounds,
        )
    }

    fn from_sui(value: DockFloatingGroup) -> Self {
        Self {
            id: value.id,
            panel_ids: value.panels.into_iter().map(DockPanelId::get).collect(),
            active: value.active.get(),
            bounds: value.bounds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BindingDockLayout {
    pub root: BindingDockNode,
    pub floating: Vec<BindingDockFloatingGroup>,
    pub hidden: Vec<u64>,
}

impl BindingDockLayout {
    pub fn new(
        root: BindingDockNode,
        floating: impl IntoIterator<Item = BindingDockFloatingGroup>,
        hidden: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            root,
            floating: floating.into_iter().collect(),
            hidden: hidden.into_iter().collect(),
        }
    }

    fn into_sui(self) -> DockWorkspaceSnapshot {
        DockWorkspaceSnapshot {
            root: self.root.into_sui(),
            floating: self
                .floating
                .into_iter()
                .map(BindingDockFloatingGroup::into_sui)
                .collect(),
            hidden: self.hidden.into_iter().map(DockPanelId::new).collect(),
        }
    }

    fn from_sui(value: DockWorkspaceSnapshot) -> Self {
        Self {
            root: BindingDockNode::from_sui(value.root),
            floating: value
                .floating
                .into_iter()
                .map(BindingDockFloatingGroup::from_sui)
                .collect(),
            hidden: value.hidden.into_iter().map(DockPanelId::get).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingDockState {
    inner: DockWorkspaceState,
}

impl BindingDockState {
    pub fn new(layout: BindingDockLayout) -> Result<Self, String> {
        DockWorkspaceState::new(layout.into_sui())
            .map(|inner| Self { inner })
            .map_err(|error| error.to_string())
    }

    pub fn empty() -> Self {
        Self {
            inner: DockWorkspaceState::empty(),
        }
    }

    pub fn snapshot(&self) -> BindingDockLayout {
        BindingDockLayout::from_sui(self.inner.snapshot())
    }

    pub fn apply(&self, layout: BindingDockLayout) -> Result<bool, String> {
        self.inner
            .apply_snapshot(layout.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn dock(&self, panel: u64, target: u64, zone: &str) -> Result<bool, String> {
        self.inner
            .dock(
                DockPanelId::new(panel),
                DockPanelId::new(target),
                binding_dock_zone_from_name(zone)?,
            )
            .map_err(|error| error.to_string())
    }

    pub fn dock_to_root(&self, panel: u64, zone: &str) -> Result<bool, String> {
        self.inner
            .dock_to_root(DockPanelId::new(panel), binding_dock_zone_from_name(zone)?)
            .map_err(|error| error.to_string())
    }

    pub fn float_panel(&self, panel: u64, bounds: Rect) -> Result<u64, String> {
        self.inner
            .float_panel(DockPanelId::new(panel), bounds)
            .map_err(|error| error.to_string())
    }

    pub fn hide(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .hide(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    pub fn show(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .show(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    pub fn activate(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .activate(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }
}

fn binding_dock_zone_from_name(value: &str) -> Result<DockZone, String> {
    match normalized_option_name(value).as_str() {
        "center" | "tab" => Ok(DockZone::Center),
        "left" => Ok(DockZone::Left),
        "right" => Ok(DockZone::Right),
        "top" => Ok(DockZone::Top),
        "bottom" => Ok(DockZone::Bottom),
        _ => Err(format!(
            "dock zone must be 'center', 'left', 'right', 'top', or 'bottom', got '{value}'"
        )),
    }
}

#[derive(Debug, Clone)]
pub struct BindingDockPanel {
    pub id: u64,
    pub title: String,
    pub child: BindingWidget,
}

impl BindingDockPanel {
    pub fn new(id: u64, title: impl Into<String>, child: BindingWidget) -> Result<Self, String> {
        if id == 0 {
            return Err("dock panel id must be non-zero".to_string());
        }
        Ok(Self {
            id,
            title: title.into(),
            child,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BindingFloatingView {
    id: Option<u64>,
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
    pub child: BindingWidget,
}

impl BindingFloatingView {
    pub fn new(
        title: impl Into<String>,
        bounds: Rect,
        min_size: Size,
        visible: bool,
        child: BindingWidget,
    ) -> Self {
        Self {
            id: None,
            title: title.into(),
            bounds,
            min_size,
            visible,
            child,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingFloatingViewSnapshot {
    pub id: u64,
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
    pub maximized: bool,
}

impl From<FloatingViewSnapshot> for BindingFloatingViewSnapshot {
    fn from(value: FloatingViewSnapshot) -> Self {
        Self {
            id: value.id,
            title: value.title,
            bounds: value.bounds,
            min_size: value.min_size,
            visible: value.visible,
            maximized: value.maximized,
        }
    }
}

#[derive(Clone)]
pub struct BindingFloatingWorkspaceState {
    inner: FloatingWorkspaceState,
}

impl fmt::Debug for BindingFloatingWorkspaceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingFloatingWorkspaceState")
            .field("views", &self.views())
            .finish()
    }
}

impl BindingFloatingWorkspaceState {
    pub fn new() -> Self {
        Self {
            inner: FloatingWorkspaceState::new(),
        }
    }

    pub fn views(&self) -> Vec<BindingFloatingViewSnapshot> {
        self.inner.snapshots().into_iter().map(Into::into).collect()
    }

    pub fn set_visible(&self, id: u64, visible: bool) -> bool {
        self.inner.set_view_visible(id, visible)
    }

    pub fn set_bounds(&self, id: u64, bounds: Rect) -> bool {
        self.inner.set_view_bounds(id, bounds)
    }

    pub fn bring_to_front(&self, id: u64) -> bool {
        self.inner.bring_to_front(id)
    }

    pub fn set_maximized(&self, id: u64, maximized: bool) -> bool {
        self.inner.set_view_maximized(id, maximized)
    }
}

impl Default for BindingFloatingWorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BindingConstraintCase {
    query: ConstraintQuery,
    child: BindingWidget,
}

impl BindingConstraintCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        child: BindingWidget,
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        min_aspect_ratio: Option<f32>,
        max_aspect_ratio: Option<f32>,
        orientation: &str,
    ) -> Result<Self, String> {
        let mut query = ConstraintQuery::new();
        if let Some(value) = min_width {
            query = query.min_width(value);
        }
        if let Some(value) = max_width {
            query = query.max_width(value);
        }
        if let Some(value) = min_height {
            query = query.min_height(value);
        }
        if let Some(value) = max_height {
            query = query.max_height(value);
        }
        if let Some(value) = min_aspect_ratio {
            query = query.min_aspect_ratio(value);
        }
        if let Some(value) = max_aspect_ratio {
            query = query.max_aspect_ratio(value);
        }
        query = query.orientation(match normalized_option_name(orientation).as_str() {
            "any" => ConstraintOrientation::Any,
            "portrait" => ConstraintOrientation::Portrait,
            "landscape" => ConstraintOrientation::Landscape,
            _ => {
                return Err(format!(
                    "constraint orientation must be 'any', 'portrait', or 'landscape', got '{orientation}'"
                ));
            }
        });
        Ok(Self { query, child })
    }
}

#[derive(Debug, Clone)]
pub struct BindingResponsiveSidebarState {
    inner: ResponsiveSidebarState,
}

impl BindingResponsiveSidebarState {
    pub fn new(expanded: bool, overlay_open: bool) -> Self {
        let inner = ResponsiveSidebarState::new();
        inner.set_expanded(expanded);
        if overlay_open {
            inner.open_overlay();
        }
        Self { inner }
    }

    pub fn expanded(&self) -> bool {
        self.inner.snapshot().expanded
    }

    pub fn overlay_open(&self) -> bool {
        self.inner.snapshot().overlay_open
    }

    pub fn set_expanded(&self, expanded: bool) -> bool {
        self.inner.set_expanded(expanded)
    }

    pub fn toggle_expanded(&self) -> bool {
        self.inner.toggle_expanded()
    }

    pub fn open_overlay(&self) -> bool {
        self.inner.open_overlay()
    }

    pub fn close_overlay(&self) -> bool {
        self.inner.close_overlay()
    }

    pub fn toggle_overlay(&self) -> bool {
        self.inner.toggle_overlay()
    }
}

#[derive(Debug, Clone)]
pub struct BindingMasterDetailState {
    inner: MasterDetailState,
}

impl BindingMasterDetailState {
    pub fn new(route: &str) -> Result<Self, String> {
        Ok(Self {
            inner: MasterDetailState::new(binding_master_detail_route(route)?),
        })
    }

    pub fn route(&self) -> &'static str {
        match self.inner.route() {
            MasterDetailRoute::Master => "master",
            MasterDetailRoute::Detail => "detail",
        }
    }

    pub fn set_route(&self, route: &str) -> Result<bool, String> {
        Ok(self.inner.set_route(binding_master_detail_route(route)?))
    }

    pub fn show_master(&self) -> bool {
        self.inner.show_master()
    }

    pub fn show_detail(&self) -> bool {
        self.inner.show_detail()
    }
}

fn binding_master_detail_route(value: &str) -> Result<MasterDetailRoute, String> {
    match normalized_option_name(value).as_str() {
        "master" | "list" => Ok(MasterDetailRoute::Master),
        "detail" => Ok(MasterDetailRoute::Detail),
        _ => Err(format!(
            "master-detail route must be 'master' or 'detail', got '{value}'"
        )),
    }
}

#[derive(Clone)]
pub struct BindingNotificationCenter {
    inner: NotificationCenter,
}

impl BindingNotificationCenter {
    pub fn new() -> Self {
        Self {
            inner: NotificationCenter::new(),
        }
    }

    pub fn notify(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        duration: Option<f64>,
        urgency: &str,
    ) -> Result<u64, String> {
        let mut notification = TransientNotification::new(title, message);
        notification = match duration {
            Some(duration) => notification.duration(duration),
            None => notification.persistent(),
        };
        notification = notification.urgency(match normalized_option_name(urgency).as_str() {
            "polite" => NotificationUrgency::Polite,
            "assertive" | "urgent" => NotificationUrgency::Assertive,
            _ => {
                return Err(format!(
                    "notification urgency must be 'polite' or 'assertive', got '{urgency}'"
                ));
            }
        });
        Ok(self.inner.push(notification).get())
    }

    pub fn dismiss(&self, id: u64) -> bool {
        self.inner.dismiss(NotificationId::new(id))
    }

    pub fn clear(&self) -> bool {
        self.inner.clear()
    }

    pub fn len(&self) -> usize {
        self.inner.snapshot().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for BindingNotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BindingNotificationCenter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingNotificationCenter")
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingVirtualListItem {
    pub key: u64,
    pub text: String,
}

impl BindingVirtualListItem {
    pub fn new(key: u64, text: impl Into<String>) -> Result<Self, String> {
        if key == 0 {
            return Err("virtual-list keys must be non-zero".to_string());
        }
        Ok(Self {
            key,
            text: text.into(),
        })
    }
}

#[derive(Clone)]
pub struct BindingVirtualListModel {
    inner: VirtualCollectionModel<u64, String>,
}

impl BindingVirtualListModel {
    pub fn new(
        name: impl Into<String>,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<Self, String> {
        let name = name.into();
        VirtualCollectionModel::from_items(
            name,
            items.into_iter().map(|item| (item.key, item.text)),
        )
        .map(|inner| Self { inner })
        .map_err(|error| error.to_string())
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn append(&self, item: BindingVirtualListItem) -> Result<bool, String> {
        self.inner
            .append(item.key, item.text)
            .map_err(|error| error.to_string())
    }

    pub fn prepend(
        &self,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<bool, String> {
        self.inner
            .prepend(items.into_iter().map(|item| (item.key, item.text)))
            .map_err(|error| error.to_string())
    }

    pub fn update(&self, item: BindingVirtualListItem) -> Result<bool, String> {
        self.inner
            .update(item.key, item.text)
            .map_err(|error| error.to_string())
    }

    pub fn remove(&self, key: u64) -> Result<bool, String> {
        self.inner.remove(key).map_err(|error| error.to_string())
    }

    pub fn move_to(&self, key: u64, index: usize) -> Result<bool, String> {
        self.inner
            .move_to(key, index)
            .map_err(|error| error.to_string())
    }

    pub fn replace(
        &self,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<bool, String> {
        self.inner
            .replace(items.into_iter().map(|item| (item.key, item.text)))
            .map_err(|error| error.to_string())
    }
}

impl fmt::Debug for BindingVirtualListModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingVirtualListModel")
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct BindingDragScope {
    inner: DragDropScope,
}

impl BindingDragScope {
    pub fn new() -> Self {
        Self {
            inner: DragDropScope::new(),
        }
    }

    pub fn active(&self) -> bool {
        self.inner.active_drag().is_some()
    }
}

impl Default for BindingDragScope {
    fn default() -> Self {
        Self::new()
    }
}

fn binding_drop_effect(value: &str) -> Result<DropEffect, String> {
    match normalized_option_name(value).as_str() {
        "none" | "reject" => Ok(DropEffect::None),
        "copy" => Ok(DropEffect::Copy),
        "move" => Ok(DropEffect::Move),
        "link" => Ok(DropEffect::Link),
        _ => Err(format!(
            "drop effect must be 'none', 'copy', 'move', or 'link', got '{value}'"
        )),
    }
}

fn binding_drag_payload_text(event: &DragEvent) -> String {
    match &event.payload {
        DragPayload::Text(text) => text.clone(),
        DragPayload::Image { handle, .. } => format!("image:{}", handle.get()),
        DragPayload::Custom { kind, .. } => kind.to_string(),
    }
}

#[derive(Clone)]
pub struct BindingWidget {
    inner: Arc<BindingWidgetKind>,
}

#[derive(Clone)]
struct BindingBuildContext {
    errors: ForeignErrorSink,
    theme: Option<BindingTheme>,
}

impl BindingBuildContext {
    fn new(errors: ForeignErrorSink, theme: Option<BindingTheme>) -> Self {
        Self { errors, theme }
    }
}

impl std::ops::Deref for BindingBuildContext {
    type Target = ForeignErrorSink;

    fn deref(&self) -> &Self::Target {
        &self.errors
    }
}

impl fmt::Debug for BindingWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.as_ref() {
            BindingWidgetKind::Label { .. } => f.debug_tuple("BindingWidget::Label").finish(),
            BindingWidgetKind::Button { .. } => f.debug_tuple("BindingWidget::Button").finish(),
            BindingWidgetKind::Icon { glyph, .. } => f
                .debug_struct("BindingWidget::Icon")
                .field("glyph", &binding_icon_glyph_name(*glyph))
                .finish(),
            BindingWidgetKind::IconButton { glyph, .. } => f
                .debug_struct("BindingWidget::IconButton")
                .field("glyph", &binding_icon_glyph_name(*glyph))
                .finish(),
            BindingWidgetKind::Link { .. } => f.debug_tuple("BindingWidget::Link").finish(),
            BindingWidgetKind::Checkbox { .. } => f.debug_tuple("BindingWidget::Checkbox").finish(),
            BindingWidgetKind::Switch { .. } => f.debug_tuple("BindingWidget::Switch").finish(),
            BindingWidgetKind::RadioButton { .. } => {
                f.debug_tuple("BindingWidget::RadioButton").finish()
            }
            BindingWidgetKind::RadioGroup { .. } => {
                f.debug_tuple("BindingWidget::RadioGroup").finish()
            }
            BindingWidgetKind::SegmentedControl { items, .. } => f
                .debug_struct("BindingWidget::SegmentedControl")
                .field("items", items)
                .finish(),
            BindingWidgetKind::Breadcrumb { items, .. } => f
                .debug_struct("BindingWidget::Breadcrumb")
                .field("items", items)
                .finish(),
            BindingWidgetKind::ListView { .. } => f.debug_tuple("BindingWidget::ListView").finish(),
            BindingWidgetKind::Table { columns, rows, .. } => f
                .debug_struct("BindingWidget::Table")
                .field("columns", columns)
                .field("rows", rows)
                .finish(),
            BindingWidgetKind::TreeView { items, .. } => f
                .debug_struct("BindingWidget::TreeView")
                .field("items", items)
                .finish(),
            BindingWidgetKind::LayerList { items, .. } => f
                .debug_struct("BindingWidget::LayerList")
                .field("items", items)
                .finish(),
            BindingWidgetKind::Menu { items, .. } => f
                .debug_struct("BindingWidget::Menu")
                .field("items", items)
                .finish(),
            BindingWidgetKind::ContextMenu { items, trigger, .. } => f
                .debug_struct("BindingWidget::ContextMenu")
                .field("items", items)
                .field("trigger", trigger)
                .finish(),
            BindingWidgetKind::TabBar { tabs, .. } => f
                .debug_struct("BindingWidget::TabBar")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::Tabs { tabs, .. } => f
                .debug_struct("BindingWidget::Tabs")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::Dialog { title, content, .. } => f
                .debug_struct("BindingWidget::Dialog")
                .field("title", title)
                .field("content", content)
                .finish(),
            BindingWidgetKind::SignalMeter { .. } => {
                f.debug_tuple("BindingWidget::SignalMeter").finish()
            }
            BindingWidgetKind::StatusBadge { tone, .. } => f
                .debug_struct("BindingWidget::StatusBadge")
                .field("tone", tone)
                .finish(),
            BindingWidgetKind::StatusBar { segments, .. } => f
                .debug_struct("BindingWidget::StatusBar")
                .field("segments", segments)
                .finish(),
            BindingWidgetKind::DetailRow { label, value, .. } => f
                .debug_struct("BindingWidget::DetailRow")
                .field("label", label)
                .field("value", value)
                .finish(),
            BindingWidgetKind::Slider { .. } => f.debug_tuple("BindingWidget::Slider").finish(),
            BindingWidgetKind::NumberInput { .. } => {
                f.debug_tuple("BindingWidget::NumberInput").finish()
            }
            BindingWidgetKind::Select { .. } => f.debug_tuple("BindingWidget::Select").finish(),
            BindingWidgetKind::ProgressBar { .. } => {
                f.debug_tuple("BindingWidget::ProgressBar").finish()
            }
            BindingWidgetKind::BusyIndicator { .. } => {
                f.debug_tuple("BindingWidget::BusyIndicator").finish()
            }
            BindingWidgetKind::TextInput { .. } => {
                f.debug_tuple("BindingWidget::TextInput").finish()
            }
            BindingWidgetKind::PasswordInput { .. } => {
                f.debug_tuple("BindingWidget::PasswordInput").finish()
            }
            BindingWidgetKind::DateTimeInput { .. } => {
                f.debug_tuple("BindingWidget::DateTimeInput").finish()
            }
            BindingWidgetKind::TextArea { .. } => f.debug_tuple("BindingWidget::TextArea").finish(),
            BindingWidgetKind::RichText { .. } => f.debug_tuple("BindingWidget::RichText").finish(),
            BindingWidgetKind::RichDocument { document, .. } => f
                .debug_struct("BindingWidget::RichDocument")
                .field("document", document)
                .finish(),
            BindingWidgetKind::Image { .. } => f.debug_tuple("BindingWidget::Image").finish(),
            BindingWidgetKind::ColorSwatch { .. } => {
                f.debug_tuple("BindingWidget::ColorSwatch").finish()
            }
            BindingWidgetKind::ColorPalette { swatches, .. } => f
                .debug_struct("BindingWidget::ColorPalette")
                .field("swatches", swatches)
                .finish(),
            BindingWidgetKind::ColorPicker { .. } => {
                f.debug_tuple("BindingWidget::ColorPicker").finish()
            }
            BindingWidgetKind::SimpleColorPicker { .. } => {
                f.debug_tuple("BindingWidget::SimpleColorPicker").finish()
            }
            BindingWidgetKind::Separator { .. } => {
                f.debug_tuple("BindingWidget::Separator").finish()
            }
            BindingWidgetKind::EmptyState { title, action, .. } => f
                .debug_struct("BindingWidget::EmptyState")
                .field("title", title)
                .field("action", action)
                .finish(),
            BindingWidgetKind::ActionCard { title, .. } => f
                .debug_struct("BindingWidget::ActionCard")
                .field("title", title)
                .finish(),
            BindingWidgetKind::BrushPreview { name, spec, .. } => f
                .debug_struct("BindingWidget::BrushPreview")
                .field("name", name)
                .field("spec", spec)
                .finish(),
            BindingWidgetKind::CommandGroup {
                name,
                axis,
                children,
                ..
            } => f
                .debug_struct("BindingWidget::CommandGroup")
                .field("name", name)
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::CoverageDots { name, .. } => f
                .debug_struct("BindingWidget::CoverageDots")
                .field("name", name)
                .finish(),
            BindingWidgetKind::Dock {
                body, top, bottom, ..
            } => f
                .debug_struct("BindingWidget::Dock")
                .field("body", body)
                .field("top", top)
                .field("bottom", bottom)
                .finish(),
            BindingWidgetKind::FixedPaneSplit {
                axis,
                first,
                divider,
                second,
                ..
            } => f
                .debug_struct("BindingWidget::FixedPaneSplit")
                .field("axis", axis)
                .field("first", first)
                .field("divider", divider)
                .field("second", second)
                .finish(),
            BindingWidgetKind::FramedField { child, name, .. } => f
                .debug_struct("BindingWidget::FramedField")
                .field("name", name)
                .field("child", child)
                .finish(),
            BindingWidgetKind::MeasuredBottomDock { body, bottom, .. } => f
                .debug_struct("BindingWidget::MeasuredBottomDock")
                .field("body", body)
                .field("bottom", bottom)
                .finish(),
            BindingWidgetKind::PlacementBadge { label, .. } => f
                .debug_struct("BindingWidget::PlacementBadge")
                .field("label", label)
                .finish(),
            BindingWidgetKind::PropertyRow { label, control, .. } => f
                .debug_struct("BindingWidget::PropertyRow")
                .field("label", label)
                .field("control", control)
                .finish(),
            BindingWidgetKind::SectionLabel { label, .. } => f
                .debug_struct("BindingWidget::SectionLabel")
                .field("label", label)
                .finish(),
            BindingWidgetKind::SideSheet { title, body, .. } => f
                .debug_struct("BindingWidget::SideSheet")
                .field("title", title)
                .field("body", body)
                .finish(),
            BindingWidgetKind::SplitView {
                name,
                axis,
                first,
                second,
                ..
            } => f
                .debug_struct("BindingWidget::SplitView")
                .field("name", name)
                .field("axis", axis)
                .field("first", first)
                .field("second", second)
                .finish(),
            BindingWidgetKind::SwitchView {
                children, selected, ..
            } => f
                .debug_struct("BindingWidget::SwitchView")
                .field("children", children)
                .field("selected", selected)
                .finish(),
            BindingWidgetKind::TrailingSlotRow { body, trailing, .. } => f
                .debug_struct("BindingWidget::TrailingSlotRow")
                .field("body", body)
                .field("trailing", trailing)
                .finish(),
            BindingWidgetKind::VirtualScrollView { children, name, .. } => f
                .debug_struct("BindingWidget::VirtualScrollView")
                .field("name", name)
                .field("children", children)
                .finish(),
            BindingWidgetKind::FloatingStack { windows, name } => f
                .debug_struct("BindingWidget::FloatingStack")
                .field("name", name)
                .field("windows", windows)
                .finish(),
            BindingWidgetKind::ReorderableList { name, children, .. } => f
                .debug_struct("BindingWidget::ReorderableList")
                .field("name", name)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Surface { role, child, .. } => f
                .debug_struct("BindingWidget::Surface")
                .field("role", role)
                .field("child", child)
                .finish(),
            BindingWidgetKind::ExternalSurface { tier, .. } => f
                .debug_struct("BindingWidget::ExternalSurface")
                .field("tier", tier)
                .finish(),
            BindingWidgetKind::Toolbar { axis, children, .. } => f
                .debug_struct("BindingWidget::Toolbar")
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Grid {
                columns, children, ..
            } => f
                .debug_struct("BindingWidget::Grid")
                .field("columns", columns)
                .field("children", children)
                .finish(),
            BindingWidgetKind::AspectRatio { child, ratio, .. } => f
                .debug_struct("BindingWidget::AspectRatio")
                .field("ratio", ratio)
                .field("child", child)
                .finish(),
            BindingWidgetKind::SafeArea { child, .. } => f
                .debug_struct("BindingWidget::SafeArea")
                .field("child", child)
                .finish(),
            BindingWidgetKind::LayoutTransition {
                child, duration, ..
            } => f
                .debug_struct("BindingWidget::LayoutTransition")
                .field("duration", duration)
                .field("child", child)
                .finish(),
            BindingWidgetKind::AdaptiveView { compact, .. } => f
                .debug_struct("BindingWidget::AdaptiveView")
                .field("compact", compact)
                .finish_non_exhaustive(),
            BindingWidgetKind::ConstraintView { cases, fallback } => f
                .debug_struct("BindingWidget::ConstraintView")
                .field("cases", cases)
                .field("fallback", fallback)
                .finish(),
            BindingWidgetKind::ResponsiveSidebar {
                sidebar, content, ..
            } => f
                .debug_struct("BindingWidget::ResponsiveSidebar")
                .field("sidebar", sidebar)
                .field("content", content)
                .finish_non_exhaustive(),
            BindingWidgetKind::MasterDetail { master, detail, .. } => f
                .debug_struct("BindingWidget::MasterDetail")
                .field("master", master)
                .field("detail", detail)
                .finish_non_exhaustive(),
            BindingWidgetKind::OverlayHost { child } => f
                .debug_struct("BindingWidget::OverlayHost")
                .field("child", child)
                .finish(),
            BindingWidgetKind::NotificationHost { center, width } => f
                .debug_struct("BindingWidget::NotificationHost")
                .field("center", center)
                .field("width", width)
                .finish(),
            BindingWidgetKind::CommandPalette { name, content, .. } => f
                .debug_struct("BindingWidget::CommandPalette")
                .field("name", name)
                .field("content", content)
                .finish_non_exhaustive(),
            BindingWidgetKind::VirtualList { name, model, .. } => f
                .debug_struct("BindingWidget::VirtualList")
                .field("name", name)
                .field("model", model)
                .finish_non_exhaustive(),
            BindingWidgetKind::Canvas { name, shapes, .. } => f
                .debug_struct("BindingWidget::Canvas")
                .field("name", name)
                .field("shapes", shapes)
                .finish_non_exhaustive(),
            BindingWidgetKind::CanvasRuler { name, axis, .. } => f
                .debug_struct("BindingWidget::CanvasRuler")
                .field("name", name)
                .field("axis", axis)
                .finish_non_exhaustive(),
            BindingWidgetKind::DragDropHost { child, .. } => f
                .debug_struct("BindingWidget::DragDropHost")
                .field("child", child)
                .finish_non_exhaustive(),
            BindingWidgetKind::Draggable { child, payload, .. } => f
                .debug_struct("BindingWidget::Draggable")
                .field("child", child)
                .field("payload", payload)
                .finish_non_exhaustive(),
            BindingWidgetKind::DropTarget { child, .. } => f
                .debug_struct("BindingWidget::DropTarget")
                .field("child", child)
                .finish_non_exhaustive(),
            BindingWidgetKind::FloatingWorkspace { name, views, .. } => f
                .debug_struct("BindingWidget::FloatingWorkspace")
                .field("name", name)
                .field("views", views)
                .finish_non_exhaustive(),
            BindingWidgetKind::PixelCanvas {
                name,
                width,
                height,
                ..
            } => f
                .debug_struct("BindingWidget::PixelCanvas")
                .field("name", name)
                .field("width", width)
                .field("height", height)
                .finish_non_exhaustive(),
            BindingWidgetKind::Padding { child, insets, .. } => f
                .debug_struct("BindingWidget::Padding")
                .field("child", child)
                .field("insets", insets)
                .finish(),
            BindingWidgetKind::Align {
                child,
                horizontal,
                vertical,
            } => f
                .debug_struct("BindingWidget::Align")
                .field("child", child)
                .field("horizontal", horizontal)
                .field("vertical", vertical)
                .finish(),
            BindingWidgetKind::Background { child, .. } => f
                .debug_struct("BindingWidget::Background")
                .field("child", child)
                .finish(),
            BindingWidgetKind::SizedBox {
                child,
                width,
                height,
            } => f
                .debug_struct("BindingWidget::SizedBox")
                .field("child", child)
                .field("width", width)
                .field("height", height)
                .finish(),
            BindingWidgetKind::Stack { axis, children, .. } => f
                .debug_struct("BindingWidget::Stack")
                .field("axis", axis)
                .field("children", children)
                .finish(),
            BindingWidgetKind::SemanticRegion { name, child, .. } => f
                .debug_struct("BindingWidget::SemanticRegion")
                .field("name", name)
                .field("child", child)
                .finish(),
            BindingWidgetKind::FormRow { label, control, .. } => f
                .debug_struct("BindingWidget::FormRow")
                .field("label", label)
                .field("control", control)
                .finish(),
            BindingWidgetKind::FieldGroup { children, .. } => f
                .debug_struct("BindingWidget::FieldGroup")
                .field("children", children)
                .finish(),
            BindingWidgetKind::FormSection { title, child, .. } => f
                .debug_struct("BindingWidget::FormSection")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::PanelSection { title, child, .. } => f
                .debug_struct("BindingWidget::PanelSection")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::DockPanel { title, child, .. } => f
                .debug_struct("BindingWidget::DockPanel")
                .field("title", title)
                .field("child", child)
                .finish(),
            BindingWidgetKind::DockWorkspace { name, panels, .. } => f
                .debug_struct("BindingWidget::DockWorkspace")
                .field("name", name)
                .field("panels", panels)
                .finish(),
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => f
                .debug_struct("BindingWidget::StatusBarHost")
                .field("content", content)
                .field("status_bar", status_bar)
                .finish(),
            BindingWidgetKind::Tooltip { text, child, .. } => f
                .debug_struct("BindingWidget::Tooltip")
                .field("text", text)
                .field("child", child)
                .finish(),
            BindingWidgetKind::Popover {
                name,
                trigger,
                content,
                ..
            } => f
                .debug_struct("BindingWidget::Popover")
                .field("name", name)
                .field("trigger", trigger)
                .field("content", content)
                .finish(),
            BindingWidgetKind::ToolPalette { items, .. } => f
                .debug_struct("BindingWidget::ToolPalette")
                .field("items", items)
                .finish(),
            BindingWidgetKind::PresetStrip { presets, .. } => f
                .debug_struct("BindingWidget::PresetStrip")
                .field("presets", presets)
                .finish(),
            BindingWidgetKind::BrowserTabBar { tabs, .. } => f
                .debug_struct("BindingWidget::BrowserTabBar")
                .field("tabs", tabs)
                .finish(),
            BindingWidgetKind::ScrollView { axes, child, .. } => f
                .debug_struct("BindingWidget::ScrollView")
                .field("axes", axes)
                .field("child", child)
                .finish(),
            BindingWidgetKind::Flex {
                axis,
                gap,
                children,
            } => f
                .debug_struct("BindingWidget::Flex")
                .field("axis", axis)
                .field("gap", gap)
                .field("children", children)
                .finish(),
            BindingWidgetKind::Foreign { children, .. } => f
                .debug_struct("BindingWidget::Foreign")
                .field("children", children)
                .finish(),
        }
    }
}

#[derive(Clone)]
enum BindingWidgetKind {
    Label {
        text: BindingText,
    },
    Button {
        label: BindingText,
        action: Option<BindingAction>,
    },
    Icon {
        glyph: IconGlyph,
        label: Option<String>,
        size: Option<f32>,
        color: Option<Color>,
    },
    IconButton {
        glyph: IconGlyph,
        label: BindingText,
        selected: BindingBool,
        enabled: BindingBool,
        size: Option<f32>,
        icon_size: Option<f32>,
        description: Option<String>,
        action: Option<BindingAction>,
    },
    Link {
        label: BindingText,
        url: BindingText,
        semantic_name: Option<String>,
        enabled: BindingBool,
        action: Option<BindingStringAction>,
    },
    Checkbox {
        label: BindingText,
        checked: BindingBool,
        action: Option<BindingBoolAction>,
    },
    Switch {
        label: BindingText,
        on: BindingBool,
        action: Option<BindingBoolAction>,
    },
    RadioButton {
        label: BindingText,
        selected: BindingBool,
        action: Option<BindingAction>,
    },
    RadioGroup {
        name: BindingText,
        options: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    SegmentedControl {
        name: BindingText,
        items: Vec<BindingSegmentedControlItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Breadcrumb {
        name: BindingText,
        items: Vec<String>,
        current: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    ListView {
        name: BindingText,
        items: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Table {
        name: BindingText,
        columns: Vec<BindingTableColumn>,
        rows: Vec<BindingTableRow>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    TreeView {
        name: BindingText,
        items: Vec<BindingTreeItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    LayerList {
        name: BindingText,
        items: Vec<BindingLayerListItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Menu {
        name: BindingText,
        items: Vec<BindingMenuItem>,
        highlighted: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    ContextMenu {
        name: String,
        trigger: BindingWidget,
        items: Vec<BindingMenuItem>,
        action: Option<BindingSelectAction>,
    },
    TabBar {
        name: BindingText,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    },
    Tabs {
        name: BindingText,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
    },
    Dialog {
        title: BindingText,
        content: BindingWidget,
        shown: BindingBool,
    },
    SignalMeter {
        name: BindingText,
        active: BindingBool,
        description: Option<String>,
        bars: usize,
        size: Option<Size>,
    },
    StatusBadge {
        label: BindingText,
        tone: SemanticTone,
        icon: Option<IconGlyph>,
        min_width: Option<f32>,
    },
    StatusBar {
        segments: Vec<BindingStatusBarSegment>,
        name: Option<String>,
        description: Option<BindingText>,
        height: Option<f32>,
    },
    DetailRow {
        label: BindingText,
        value: BindingText,
        max_value_lines: Option<usize>,
    },
    Slider {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        step: f64,
        action: Option<BindingNumberAction>,
    },
    NumberInput {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        step: f64,
        precision: usize,
        action: Option<BindingNumberAction>,
    },
    Select {
        name: BindingText,
        options: Vec<String>,
        selected: Option<BindingNumber>,
        placeholder: Option<String>,
        action: Option<BindingSelectAction>,
    },
    ProgressBar {
        name: BindingText,
        value: BindingNumber,
        min: f64,
        max: f64,
        show_value: bool,
    },
    BusyIndicator {
        name: BindingText,
        label: Option<BindingText>,
        size: f32,
    },
    TextInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    PasswordInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    DateTimeInput {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    TextArea {
        name: BindingText,
        value: BindingText,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    },
    RichText {
        spans: Vec<BindingTextSpan>,
        semantic_name: Option<String>,
        min_width: f32,
        min_height: f32,
    },
    RichDocument {
        document: BindingRichDocument,
        on_link: Option<BindingStringAction>,
        on_image: Option<BindingStringAction>,
        on_attachment: Option<BindingIdAction>,
    },
    Image {
        image: BindingImageHandle,
        label: Option<String>,
        fit: BindingImageFit,
        size: Option<Size>,
    },
    ColorSwatch {
        name: String,
        color: Color,
        size: Option<Size>,
        read_only: bool,
        action: Option<BindingAction>,
    },
    ColorPalette {
        name: String,
        swatches: Vec<BindingColorPaletteSwatch>,
        selected: Option<BindingNumber>,
        action: Option<BindingColorSelectAction>,
        columns: Option<usize>,
        swatch_size: Option<f32>,
        gap: Option<f32>,
    },
    ColorPicker {
        name: String,
        color: Option<Color>,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    },
    SimpleColorPicker {
        name: String,
        color: Option<Color>,
        mode: SimpleColorPickerMode,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    },
    Separator {
        axis: Axis,
        name: Option<String>,
        inset: f32,
        thickness: Option<f32>,
        length: Option<f32>,
    },
    EmptyState {
        title: String,
        description: String,
        name: Option<String>,
        detail: Option<String>,
        icon: Option<IconGlyph>,
        action: Option<BindingWidget>,
        background: Option<Color>,
        transparent: bool,
    },
    ActionCard {
        title: String,
        description: String,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        enabled: BindingBool,
        action: Option<BindingAction>,
    },
    BrushPreview {
        name: String,
        kind: String,
        spec: BindingBrushPreviewSpec,
        size: Option<Size>,
    },
    CommandGroup {
        name: String,
        children: Vec<BindingWidget>,
        axis: Axis,
        padding: Option<Insets>,
        spacing: Option<f32>,
        corner_radius: Option<f32>,
        background: Option<Color>,
        border: Option<Color>,
    },
    CoverageDots {
        name: String,
        current: usize,
        target: usize,
        tone: SemanticTone,
        max_dots: usize,
        show_label: bool,
        min_width: Option<f32>,
    },
    Dock {
        body: BindingWidget,
        top: Option<(f32, BindingWidget)>,
        bottom: Option<(f32, BindingWidget)>,
        fallback_width: f32,
        fallback_body_height: f32,
    },
    FixedPaneSplit {
        axis: Axis,
        first: BindingWidget,
        divider: BindingWidget,
        second: BindingWidget,
        fixed_second: bool,
        fixed_extent: f32,
        divider_extent: f32,
        fallback_flexible_extent: f32,
    },
    FramedField {
        child: BindingWidget,
        name: Option<String>,
        description: Option<String>,
        padding: Option<Insets>,
        min_height: Option<f32>,
        fill_width: bool,
        focused: BindingBool,
        invalid: BindingBool,
    },
    MeasuredBottomDock {
        body: BindingWidget,
        bottom: BindingWidget,
        fallback_size: Size,
    },
    PlacementBadge {
        label: BindingText,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        current: Option<usize>,
        target: Option<usize>,
        min_width: Option<f32>,
    },
    PropertyRow {
        label: String,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    },
    SectionLabel {
        label: String,
        semantic_name: Option<String>,
        color: Option<Color>,
    },
    SideSheet {
        title: String,
        body: BindingWidget,
        description: Option<String>,
        shown: BindingBool,
        modal: bool,
        dismiss_on_scrim: bool,
        placement: SideSheetPlacement,
        width: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: Vec<BindingWidget>,
        on_dismiss: Option<BindingAction>,
    },
    SplitView {
        name: Option<String>,
        axis: Axis,
        first: BindingWidget,
        second: BindingWidget,
        ratio: BindingNumber,
        min_first: f32,
        min_second: f32,
        divider_thickness: Option<f32>,
        on_change: Option<BindingNumberAction>,
    },
    SwitchView {
        children: Vec<BindingWidget>,
        selected: BindingNumber,
    },
    TrailingSlotRow {
        body: BindingWidget,
        trailing: BindingWidget,
        trailing_width: f32,
        trailing_height: f32,
        gap: f32,
    },
    VirtualScrollView {
        children: Vec<BindingWidget>,
        name: Option<String>,
        padding: Option<Insets>,
        spacing: Option<f32>,
    },
    FloatingStack {
        windows: Vec<BindingFloatingStackWindow>,
        name: Option<String>,
    },
    ReorderableList {
        name: String,
        children: Vec<BindingWidget>,
        spacing: f32,
        drag_threshold: f32,
        preview_label: Option<String>,
        on_reorder: Option<BindingReorderAction>,
    },
    Surface {
        child: BindingWidget,
        role: SurfaceRole,
        name: Option<String>,
        border: Option<SurfaceBorder>,
        elevation: Option<SurfaceElevation>,
        radius: Option<f32>,
        padding: Option<f32>,
        fill_width: bool,
        fill_height: bool,
    },
    ExternalSurface {
        descriptor: ExternalTextureDescriptor,
        desired_size: Size,
        name: Option<String>,
        tier: RendererInteropTier,
    },
    Toolbar {
        children: Vec<BindingWidget>,
        axis: Axis,
        name: Option<String>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        background: Option<Color>,
        divider: bool,
    },
    Grid {
        columns: usize,
        children: Vec<BindingWidget>,
        name: Option<String>,
        column_gap: f32,
        row_gap: f32,
    },
    AspectRatio {
        child: BindingWidget,
        ratio: f32,
        fit: AspectRatioFit,
        horizontal: Alignment,
        vertical: Alignment,
    },
    SafeArea {
        child: BindingWidget,
        edges: SafeAreaEdges,
        minimum: SafeAreaInsets,
    },
    LayoutTransition {
        child: BindingWidget,
        duration: f64,
        easing: Easing,
    },
    AdaptiveView {
        compact: BindingWidget,
        medium: BindingWidget,
        expanded: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        on_class_change: Option<BindingStringAction>,
    },
    ConstraintView {
        cases: Vec<BindingConstraintCase>,
        fallback: BindingWidget,
    },
    ResponsiveSidebar {
        state: BindingResponsiveSidebarState,
        sidebar: BindingWidget,
        content: BindingWidget,
        name: Option<String>,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        rail_width: f32,
        overlay_width: f32,
        dismiss_on_scrim: bool,
        on_mode_change: Option<BindingStringAction>,
    },
    MasterDetail {
        state: BindingMasterDetailState,
        master: BindingWidget,
        detail: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        master_width: f32,
    },
    OverlayHost {
        child: BindingWidget,
    },
    NotificationHost {
        center: BindingNotificationCenter,
        width: f32,
    },
    CommandPalette {
        name: String,
        content: BindingWidget,
        description: Option<String>,
        shown: BindingBool,
        max_width: Option<f32>,
        on_dismiss: Option<BindingAction>,
    },
    VirtualList {
        name: String,
        model: BindingVirtualListModel,
        estimated_row_height: f32,
        spacing: f32,
        padding: Option<Insets>,
        row_padding: Option<Insets>,
        overscan_viewports: f32,
        cache_capacity: usize,
        selectable: bool,
        transparent: bool,
        stick_to_end: bool,
        overlay_scroll_bars: bool,
        on_change: Option<BindingIdAction>,
        on_near_start: Option<BindingAction>,
        on_near_end: Option<BindingAction>,
    },
    Canvas {
        name: String,
        viewport: BindingCanvasViewport,
        shapes: Vec<BindingCanvasShape>,
        draw_stroke: BindingCanvasStroke,
        desired_size: Size,
    },
    CanvasRuler {
        axis: CanvasRulerAxis,
        name: String,
        document_size: Size,
        viewport: BindingCanvasViewport,
        viewport_size: Size,
        extent: Option<f32>,
    },
    DragDropHost {
        scope: BindingDragScope,
        child: BindingWidget,
        on_external_hover: Option<BindingStringsAction>,
        on_external_drop: Option<BindingStringAction>,
        on_external_cancel: Option<BindingAction>,
    },
    Draggable {
        scope: BindingDragScope,
        child: BindingWidget,
        payload: String,
        effect: DropEffect,
        preview_label: Option<String>,
        threshold: f32,
        on_start: Option<BindingStringAction>,
        on_end: Option<BindingStringAction>,
    },
    DropTarget {
        scope: BindingDragScope,
        child: BindingWidget,
        effect: DropEffect,
        on_drop: Option<BindingStringAction>,
        on_hover_change: Option<BindingBoolAction>,
    },
    FloatingWorkspace {
        state: BindingFloatingWorkspaceState,
        views: Vec<BindingFloatingView>,
        name: Option<String>,
    },
    PixelCanvas {
        state: BindingPixelCanvasState,
        name: String,
        width: usize,
        height: usize,
        paper_color: Option<Color>,
        desired_size: Size,
        viewport: BindingCanvasViewport,
        fit_on_first_layout: bool,
        pixels: Vec<Color>,
    },
    Padding {
        child: BindingWidget,
        insets: Insets,
        fill_child_width: bool,
        fill_child_height: bool,
    },
    Align {
        child: BindingWidget,
        horizontal: Alignment,
        vertical: Alignment,
    },
    Background {
        child: BindingWidget,
        color: Color,
    },
    SizedBox {
        child: Option<BindingWidget>,
        width: Option<f32>,
        height: Option<f32>,
    },
    Stack {
        children: Vec<BindingWidget>,
        axis: Axis,
        spacing: f32,
        alignment: Alignment,
    },
    SemanticRegion {
        name: BindingText,
        child: BindingWidget,
        description: Option<BindingText>,
        role: SemanticsRole,
    },
    FormRow {
        label: String,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    },
    FieldGroup {
        children: Vec<BindingWidget>,
        spacing: Option<f32>,
        padding: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
    },
    FormSection {
        title: String,
        child: BindingWidget,
        description: Option<String>,
        header_action: Option<BindingWidget>,
        padding: Option<f32>,
        body_gap: Option<f32>,
        header_gap: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
        radius: Option<f32>,
        elevation: Option<SurfaceElevation>,
    },
    PanelSection {
        title: String,
        child: BindingWidget,
        header_action: Option<BindingWidget>,
        gap: Option<f32>,
        action_gap: Option<f32>,
        collapsible: bool,
        expanded: bool,
    },
    DockPanel {
        title: String,
        child: BindingWidget,
        name: Option<String>,
        header_height: Option<f32>,
        padding: Option<f32>,
        background: Option<Color>,
        header_background: Option<Color>,
    },
    DockWorkspace {
        state: BindingDockState,
        panels: Vec<BindingDockPanel>,
        name: String,
    },
    StatusBarHost {
        content: BindingWidget,
        status_bar: BindingWidget,
    },
    Tooltip {
        text: String,
        child: BindingWidget,
        placement: TooltipPlacement,
    },
    Popover {
        name: String,
        trigger: BindingWidget,
        content: BindingWidget,
        open: bool,
    },
    ToolPalette {
        name: String,
        items: Vec<BindingToolPaletteItem>,
        selected: Option<BindingNumber>,
        axis: Axis,
        action: Option<BindingSelectAction>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        item_size: Option<f32>,
        icon_size: Option<f32>,
        background: Option<Color>,
        divider: bool,
    },
    PresetStrip {
        name: String,
        presets: Vec<String>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
        item_width: Option<f32>,
        item_height: Option<f32>,
        gap: Option<f32>,
    },
    BrowserTabBar {
        name: String,
        tabs: Vec<String>,
        selected: Option<BindingNumber>,
        on_change: Option<BindingSelectAction>,
        on_close: Option<BindingSelectAction>,
    },
    ScrollView {
        child: BindingWidget,
        axes: BindingScrollAxes,
        name: Option<String>,
    },
    Flex {
        axis: Axis,
        gap: f32,
        children: Vec<BindingWidget>,
    },
    Foreign {
        callbacks: Arc<dyn ForeignWidgetCallbacks>,
        children: Vec<BindingWidget>,
    },
}

impl BindingWidget {
    pub fn label(text: impl Into<BindingText>) -> Self {
        Self::from_kind(BindingWidgetKind::Label { text: text.into() })
    }

    pub fn label_state(state: BindingState) -> Self {
        Self::label(BindingText::State(state))
    }

    pub fn button(label: impl Into<BindingText>, action: Option<BindingAction>) -> Self {
        Self::from_kind(BindingWidgetKind::Button {
            label: label.into(),
            action,
        })
    }

    pub fn icon(
        glyph: IconGlyph,
        label: Option<String>,
        size: Option<f32>,
        color: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Icon {
            glyph,
            label,
            size,
            color,
        })
    }

    pub fn icon_button(
        glyph: IconGlyph,
        label: impl Into<BindingText>,
        selected: impl Into<BindingBool>,
        enabled: impl Into<BindingBool>,
        size: Option<f32>,
        icon_size: Option<f32>,
        description: Option<String>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::IconButton {
            glyph,
            label: label.into(),
            selected: selected.into(),
            enabled: enabled.into(),
            size,
            icon_size,
            description,
            action,
        })
    }

    pub fn link(
        label: impl Into<BindingText>,
        url: impl Into<BindingText>,
        semantic_name: Option<String>,
        enabled: impl Into<BindingBool>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Link {
            label: label.into(),
            url: url.into(),
            semantic_name,
            enabled: enabled.into(),
            action,
        })
    }

    pub fn checkbox(
        label: impl Into<BindingText>,
        checked: impl Into<BindingBool>,
        action: Option<BindingBoolAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Checkbox {
            label: label.into(),
            checked: checked.into(),
            action,
        })
    }

    pub fn switch(
        label: impl Into<BindingText>,
        on: impl Into<BindingBool>,
        action: Option<BindingBoolAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Switch {
            label: label.into(),
            on: on.into(),
            action,
        })
    }

    pub fn radio_button(
        label: impl Into<BindingText>,
        selected: impl Into<BindingBool>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RadioButton {
            label: label.into(),
            selected: selected.into(),
            action,
        })
    }

    pub fn radio_group(
        name: impl Into<BindingText>,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RadioGroup {
            name: name.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn segmented_control(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingSegmentedControlItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SegmentedControl {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn breadcrumb(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = impl Into<String>>,
        current: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Breadcrumb {
            name: name.into(),
            items: items.into_iter().map(Into::into).collect(),
            current,
            action,
        })
    }

    pub fn list_view(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ListView {
            name: name.into(),
            items: items.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn table(
        name: impl Into<BindingText>,
        columns: impl IntoIterator<Item = BindingTableColumn>,
        rows: impl IntoIterator<Item = BindingTableRow>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Table {
            name: name.into(),
            columns: columns.into_iter().collect(),
            rows: rows.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn tree_view(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingTreeItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TreeView {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn layer_list(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingLayerListItem>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::LayerList {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            action,
        })
    }

    pub fn menu(
        name: impl Into<BindingText>,
        items: impl IntoIterator<Item = BindingMenuItem>,
        highlighted: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Menu {
            name: name.into(),
            items: items.into_iter().collect(),
            highlighted,
            action,
        })
    }

    pub fn context_menu(
        name: impl Into<String>,
        trigger: BindingWidget,
        items: impl IntoIterator<Item = BindingMenuItem>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ContextMenu {
            name: name.into(),
            trigger,
            items: items.into_iter().collect(),
            action,
        })
    }

    pub fn tab_bar(
        name: impl Into<BindingText>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TabBar {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
            action,
        })
    }

    pub fn tabs(
        name: impl Into<BindingText>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Tabs {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
        })
    }

    pub fn dialog(
        title: impl Into<BindingText>,
        content: BindingWidget,
        shown: impl Into<BindingBool>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Dialog {
            title: title.into(),
            content,
            shown: shown.into(),
        })
    }

    pub fn signal_meter(
        name: impl Into<BindingText>,
        active: impl Into<BindingBool>,
        description: Option<String>,
        bars: usize,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SignalMeter {
            name: name.into(),
            active: active.into(),
            description,
            bars,
            size,
        })
    }

    pub fn status_badge(
        label: impl Into<BindingText>,
        tone: SemanticTone,
        icon: Option<IconGlyph>,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBadge {
            label: label.into(),
            tone,
            icon,
            min_width,
        })
    }

    pub fn status_bar(
        segments: impl IntoIterator<Item = BindingStatusBarSegment>,
        name: Option<String>,
        description: Option<BindingText>,
        height: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBar {
            segments: segments.into_iter().collect(),
            name,
            description,
            height,
        })
    }

    pub fn detail_row(
        label: impl Into<BindingText>,
        value: impl Into<BindingText>,
        max_value_lines: Option<usize>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DetailRow {
            label: label.into(),
            value: value.into(),
            max_value_lines,
        })
    }

    pub fn slider(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        step: f64,
        action: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Slider {
            name: name.into(),
            value: value.into(),
            min,
            max,
            step,
            action,
        })
    }

    pub fn number_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        step: f64,
        precision: usize,
        action: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::NumberInput {
            name: name.into(),
            value: value.into(),
            min,
            max,
            step,
            precision,
            action,
        })
    }

    pub fn progress_bar(
        name: impl Into<BindingText>,
        value: impl Into<BindingNumber>,
        min: f64,
        max: f64,
        show_value: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ProgressBar {
            name: name.into(),
            value: value.into(),
            min,
            max,
            show_value,
        })
    }

    pub fn select(
        name: impl Into<BindingText>,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        placeholder: Option<String>,
        action: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Select {
            name: name.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            placeholder,
            action,
        })
    }

    pub fn busy_indicator(
        name: impl Into<BindingText>,
        label: Option<BindingText>,
        size: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BusyIndicator {
            name: name.into(),
            label,
            size,
        })
    }

    pub fn text_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TextInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn password_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PasswordInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn datetime_input(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DateTimeInput {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn text_area(
        name: impl Into<BindingText>,
        value: impl Into<BindingText>,
        placeholder: Option<String>,
        action: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TextArea {
            name: name.into(),
            value: value.into(),
            placeholder,
            action,
        })
    }

    pub fn rich_text(
        spans: impl IntoIterator<Item = BindingTextSpan>,
        semantic_name: Option<String>,
        min_width: f32,
        min_height: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RichText {
            spans: spans.into_iter().collect(),
            semantic_name,
            min_width: min_width.max(0.0),
            min_height: min_height.max(0.0),
        })
    }

    pub fn rich_document(
        document: BindingRichDocument,
        on_link: Option<BindingStringAction>,
        on_image: Option<BindingStringAction>,
        on_attachment: Option<BindingIdAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::RichDocument {
            document,
            on_link,
            on_image,
            on_attachment,
        })
    }

    pub fn image(
        image: BindingImageHandle,
        label: Option<String>,
        fit: BindingImageFit,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Image {
            image,
            label,
            fit,
            size,
        })
    }

    pub fn color_swatch(
        name: impl Into<String>,
        color: Color,
        size: Option<Size>,
        read_only: bool,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorSwatch {
            name: name.into(),
            color,
            size,
            read_only,
            action,
        })
    }

    pub fn color_palette(
        name: impl Into<String>,
        swatches: impl IntoIterator<Item = BindingColorPaletteSwatch>,
        selected: Option<BindingNumber>,
        action: Option<BindingColorSelectAction>,
        columns: Option<usize>,
        swatch_size: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorPalette {
            name: name.into(),
            swatches: swatches.into_iter().collect(),
            selected,
            action,
            columns,
            swatch_size,
            gap,
        })
    }

    pub fn color_picker(
        name: impl Into<String>,
        color: Option<Color>,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ColorPicker {
            name: name.into(),
            color,
            action,
            show_alpha,
            compact,
        })
    }

    pub fn simple_color_picker(
        name: impl Into<String>,
        color: Option<Color>,
        mode: SimpleColorPickerMode,
        action: Option<BindingColorAction>,
        show_alpha: bool,
        compact: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SimpleColorPicker {
            name: name.into(),
            color,
            mode,
            action,
            show_alpha,
            compact,
        })
    }

    pub fn separator(
        axis: Axis,
        name: Option<String>,
        inset: f32,
        thickness: Option<f32>,
        length: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Separator {
            axis,
            name,
            inset,
            thickness,
            length,
        })
    }

    pub fn empty_state(
        title: impl Into<String>,
        description: impl Into<String>,
        name: Option<String>,
        detail: Option<String>,
        icon: Option<IconGlyph>,
        action: Option<BindingWidget>,
        background: Option<Color>,
        transparent: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::EmptyState {
            title: title.into(),
            description: description.into(),
            name,
            detail,
            icon,
            action,
            background,
            transparent,
        })
    }

    pub fn action_card(
        title: impl Into<String>,
        description: impl Into<String>,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        enabled: impl Into<BindingBool>,
        action: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ActionCard {
            title: title.into(),
            description: description.into(),
            icon,
            tone,
            enabled: enabled.into(),
            action,
        })
    }

    pub fn brush_preview(
        name: impl Into<String>,
        kind: impl Into<String>,
        spec: BindingBrushPreviewSpec,
        size: Option<Size>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BrushPreview {
            name: name.into(),
            kind: kind.into(),
            spec,
            size,
        })
    }

    pub fn command_group(
        name: impl Into<String>,
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        padding: Option<Insets>,
        spacing: Option<f32>,
        corner_radius: Option<f32>,
        background: Option<Color>,
        border: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CommandGroup {
            name: name.into(),
            children: children.into_iter().collect(),
            axis,
            padding,
            spacing,
            corner_radius,
            background,
            border,
        })
    }

    pub fn coverage_dots(
        name: impl Into<String>,
        current: usize,
        target: usize,
        tone: SemanticTone,
        max_dots: usize,
        show_label: bool,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CoverageDots {
            name: name.into(),
            current,
            target,
            tone,
            max_dots,
            show_label,
            min_width,
        })
    }

    pub fn dock(
        body: BindingWidget,
        top: Option<(f32, BindingWidget)>,
        bottom: Option<(f32, BindingWidget)>,
        fallback_width: f32,
        fallback_body_height: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Dock {
            body,
            top,
            bottom,
            fallback_width,
            fallback_body_height,
        })
    }

    pub fn fixed_pane_split(
        axis: Axis,
        first: BindingWidget,
        divider: BindingWidget,
        second: BindingWidget,
        fixed_second: bool,
        fixed_extent: f32,
        divider_extent: f32,
        fallback_flexible_extent: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FixedPaneSplit {
            axis,
            first,
            divider,
            second,
            fixed_second,
            fixed_extent,
            divider_extent,
            fallback_flexible_extent,
        })
    }

    pub fn framed_field(
        child: BindingWidget,
        name: Option<String>,
        description: Option<String>,
        padding: Option<Insets>,
        min_height: Option<f32>,
        fill_width: bool,
        focused: impl Into<BindingBool>,
        invalid: impl Into<BindingBool>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FramedField {
            child,
            name,
            description,
            padding,
            min_height,
            fill_width,
            focused: focused.into(),
            invalid: invalid.into(),
        })
    }

    pub fn measured_bottom_dock(
        body: BindingWidget,
        bottom: BindingWidget,
        fallback_size: Size,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::MeasuredBottomDock {
            body,
            bottom,
            fallback_size,
        })
    }

    pub fn placement_badge(
        label: impl Into<BindingText>,
        icon: Option<IconGlyph>,
        tone: SemanticTone,
        current: Option<usize>,
        target: Option<usize>,
        min_width: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PlacementBadge {
            label: label.into(),
            icon,
            tone,
            current,
            target,
            min_width,
        })
    }

    pub fn property_row(
        label: impl Into<String>,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PropertyRow {
            label: label.into(),
            control,
            stacked,
            label_width,
            control_width,
            gap,
        })
    }

    pub fn section_label(
        label: impl Into<String>,
        semantic_name: Option<String>,
        color: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SectionLabel {
            label: label.into(),
            semantic_name,
            color,
        })
    }

    pub fn side_sheet(
        title: impl Into<String>,
        body: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        modal: bool,
        dismiss_on_scrim: bool,
        placement: SideSheetPlacement,
        width: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: impl IntoIterator<Item = BindingWidget>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SideSheet {
            title: title.into(),
            body,
            description,
            shown: shown.into(),
            modal,
            dismiss_on_scrim,
            placement,
            width,
            header_action,
            actions: actions.into_iter().collect(),
            on_dismiss,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bottom_sheet(
        title: impl Into<String>,
        body: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        modal: bool,
        dismiss_on_scrim: bool,
        height: Option<f32>,
        header_action: Option<BindingWidget>,
        actions: impl IntoIterator<Item = BindingWidget>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::side_sheet(
            title,
            body,
            description,
            shown,
            modal,
            dismiss_on_scrim,
            SideSheetPlacement::Bottom,
            height,
            header_action,
            actions,
            on_dismiss,
        )
    }

    pub fn split_view(
        name: Option<String>,
        axis: Axis,
        first: BindingWidget,
        second: BindingWidget,
        ratio: impl Into<BindingNumber>,
        min_first: f32,
        min_second: f32,
        divider_thickness: Option<f32>,
        on_change: Option<BindingNumberAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SplitView {
            name,
            axis,
            first,
            second,
            ratio: ratio.into(),
            min_first,
            min_second,
            divider_thickness,
            on_change,
        })
    }

    pub fn switch_view(
        children: impl IntoIterator<Item = BindingWidget>,
        selected: impl Into<BindingNumber>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SwitchView {
            children: children.into_iter().collect(),
            selected: selected.into(),
        })
    }

    pub fn trailing_slot_row(
        body: BindingWidget,
        trailing: BindingWidget,
        trailing_width: f32,
        trailing_height: f32,
        gap: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::TrailingSlotRow {
            body,
            trailing,
            trailing_width,
            trailing_height,
            gap,
        })
    }

    pub fn virtual_scroll_view(
        children: impl IntoIterator<Item = BindingWidget>,
        name: Option<String>,
        padding: Option<Insets>,
        spacing: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::VirtualScrollView {
            children: children.into_iter().collect(),
            name,
            padding,
            spacing,
        })
    }

    pub fn floating_stack(
        windows: impl IntoIterator<Item = BindingFloatingStackWindow>,
        name: Option<String>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FloatingStack {
            windows: windows.into_iter().collect(),
            name,
        })
    }

    pub fn reorderable_list(
        name: impl Into<String>,
        children: impl IntoIterator<Item = BindingWidget>,
        spacing: f32,
        drag_threshold: f32,
        preview_label: Option<String>,
        on_reorder: Option<BindingReorderAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ReorderableList {
            name: name.into(),
            children: children.into_iter().collect(),
            spacing,
            drag_threshold,
            preview_label,
            on_reorder,
        })
    }

    pub fn surface(
        child: BindingWidget,
        role: SurfaceRole,
        name: Option<String>,
        border: Option<SurfaceBorder>,
        elevation: Option<SurfaceElevation>,
        radius: Option<f32>,
        padding: Option<f32>,
        fill_width: bool,
        fill_height: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Surface {
            child,
            role,
            name,
            border,
            elevation,
            radius,
            padding,
            fill_width,
            fill_height,
        })
    }

    pub fn external_surface(
        descriptor: ExternalTextureDescriptor,
        desired_size: Option<Size>,
        name: Option<String>,
    ) -> Result<Self, ExternalTextureValidationError> {
        descriptor.validate()?;
        let desired_size = desired_size.unwrap_or_else(|| descriptor.size());
        validate_external_size(desired_size)?;
        Ok(Self::from_kind(BindingWidgetKind::ExternalSurface {
            tier: descriptor.tier(),
            descriptor,
            desired_size,
            name,
        }))
    }

    pub fn toolbar(
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        name: Option<String>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        background: Option<Color>,
        divider: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Toolbar {
            children: children.into_iter().collect(),
            axis,
            name,
            extent,
            padding,
            spacing,
            background,
            divider,
        })
    }

    pub fn grid(
        columns: usize,
        children: impl IntoIterator<Item = BindingWidget>,
        name: Option<String>,
        column_gap: f32,
        row_gap: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Grid {
            columns: columns.max(1),
            children: children.into_iter().collect(),
            name,
            column_gap: column_gap.max(0.0),
            row_gap: row_gap.max(0.0),
        })
    }

    pub fn aspect_ratio(
        child: BindingWidget,
        ratio: f32,
        fit: AspectRatioFit,
        horizontal: Alignment,
        vertical: Alignment,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::AspectRatio {
            child,
            ratio,
            fit,
            horizontal,
            vertical,
        })
    }

    pub fn safe_area(child: BindingWidget, edges: SafeAreaEdges, minimum: SafeAreaInsets) -> Self {
        Self::from_kind(BindingWidgetKind::SafeArea {
            child,
            edges,
            minimum,
        })
    }

    pub fn layout_transition(child: BindingWidget, duration: f64, easing: Easing) -> Self {
        Self::from_kind(BindingWidgetKind::LayoutTransition {
            child,
            duration: duration.max(0.0),
            easing,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adaptive_view(
        compact: BindingWidget,
        medium: BindingWidget,
        expanded: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        on_class_change: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::AdaptiveView {
            compact,
            medium,
            expanded,
            medium_breakpoint,
            expanded_breakpoint,
            on_class_change,
        })
    }

    pub fn constraint_view(
        cases: impl IntoIterator<Item = BindingConstraintCase>,
        fallback: BindingWidget,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ConstraintView {
            cases: cases.into_iter().collect(),
            fallback,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn responsive_sidebar(
        state: BindingResponsiveSidebarState,
        sidebar: BindingWidget,
        content: BindingWidget,
        name: Option<String>,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        rail_width: f32,
        overlay_width: f32,
        dismiss_on_scrim: bool,
        on_mode_change: Option<BindingStringAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ResponsiveSidebar {
            state,
            sidebar,
            content,
            name,
            medium_breakpoint,
            expanded_breakpoint,
            rail_width,
            overlay_width,
            dismiss_on_scrim,
            on_mode_change,
        })
    }

    pub fn master_detail(
        state: BindingMasterDetailState,
        master: BindingWidget,
        detail: BindingWidget,
        medium_breakpoint: f32,
        expanded_breakpoint: f32,
        master_width: f32,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::MasterDetail {
            state,
            master,
            detail,
            medium_breakpoint,
            expanded_breakpoint,
            master_width,
        })
    }

    pub fn overlay_host(child: BindingWidget) -> Self {
        Self::from_kind(BindingWidgetKind::OverlayHost { child })
    }

    pub fn notification_host(center: BindingNotificationCenter, width: f32) -> Self {
        Self::from_kind(BindingWidgetKind::NotificationHost {
            center,
            width: width.max(120.0),
        })
    }

    pub fn command_palette(
        name: impl Into<String>,
        content: BindingWidget,
        description: Option<String>,
        shown: impl Into<BindingBool>,
        max_width: Option<f32>,
        on_dismiss: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CommandPalette {
            name: name.into(),
            content,
            description,
            shown: shown.into(),
            max_width,
            on_dismiss,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn virtual_list(
        name: impl Into<String>,
        model: BindingVirtualListModel,
        estimated_row_height: f32,
        spacing: f32,
        padding: Option<Insets>,
        row_padding: Option<Insets>,
        overscan_viewports: f32,
        cache_capacity: usize,
        selectable: bool,
        transparent: bool,
        stick_to_end: bool,
        overlay_scroll_bars: bool,
        on_change: Option<BindingIdAction>,
        on_near_start: Option<BindingAction>,
        on_near_end: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::VirtualList {
            name: name.into(),
            model,
            estimated_row_height,
            spacing,
            padding,
            row_padding,
            overscan_viewports,
            cache_capacity,
            selectable,
            transparent,
            stick_to_end,
            overlay_scroll_bars,
            on_change,
            on_near_start,
            on_near_end,
        })
    }

    pub fn canvas(
        name: impl Into<String>,
        viewport: BindingCanvasViewport,
        shapes: impl IntoIterator<Item = BindingCanvasShape>,
        draw_stroke: BindingCanvasStroke,
        desired_size: Size,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Canvas {
            name: name.into(),
            viewport,
            shapes: shapes.into_iter().collect(),
            draw_stroke,
            desired_size,
        })
    }

    pub fn canvas_ruler(
        axis: Axis,
        name: impl Into<String>,
        document_size: Size,
        viewport: BindingCanvasViewport,
        viewport_size: Size,
        extent: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::CanvasRuler {
            axis: match axis {
                Axis::Horizontal => CanvasRulerAxis::Horizontal,
                Axis::Vertical => CanvasRulerAxis::Vertical,
            },
            name: name.into(),
            document_size,
            viewport,
            viewport_size,
            extent,
        })
    }

    pub fn drag_drop_host(
        scope: BindingDragScope,
        child: BindingWidget,
        on_external_hover: Option<BindingStringsAction>,
        on_external_drop: Option<BindingStringAction>,
        on_external_cancel: Option<BindingAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DragDropHost {
            scope,
            child,
            on_external_hover,
            on_external_drop,
            on_external_cancel,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draggable(
        scope: BindingDragScope,
        child: BindingWidget,
        payload: impl Into<String>,
        effect: &str,
        preview_label: Option<String>,
        threshold: f32,
        on_start: Option<BindingStringAction>,
        on_end: Option<BindingStringAction>,
    ) -> Result<Self, String> {
        Ok(Self::from_kind(BindingWidgetKind::Draggable {
            scope,
            child,
            payload: payload.into(),
            effect: binding_drop_effect(effect)?,
            preview_label,
            threshold: threshold.max(0.0),
            on_start,
            on_end,
        }))
    }

    pub fn drop_target(
        scope: BindingDragScope,
        child: BindingWidget,
        effect: &str,
        on_drop: Option<BindingStringAction>,
        on_hover_change: Option<BindingBoolAction>,
    ) -> Result<Self, String> {
        Ok(Self::from_kind(BindingWidgetKind::DropTarget {
            scope,
            child,
            effect: binding_drop_effect(effect)?,
            on_drop,
            on_hover_change,
        }))
    }

    pub fn floating_workspace(
        state: BindingFloatingWorkspaceState,
        views: impl IntoIterator<Item = BindingFloatingView>,
        name: Option<String>,
    ) -> Self {
        let views = views
            .into_iter()
            .map(|mut view| {
                let config = FloatingViewConfig::new(view.title.clone(), view.bounds)
                    .min_size(view.min_size)
                    .visible(view.visible);
                view.id = Some(state.inner.add_view(config));
                view
            })
            .collect();
        Self::from_kind(BindingWidgetKind::FloatingWorkspace { state, views, name })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pixel_canvas(
        state: BindingPixelCanvasState,
        name: impl Into<String>,
        width: usize,
        height: usize,
        paper_color: Option<Color>,
        desired_size: Size,
        viewport: BindingCanvasViewport,
        fit_on_first_layout: bool,
        pixels: Vec<Color>,
    ) -> Result<Self, String> {
        let expected = width.max(1).saturating_mul(height.max(1));
        if !pixels.is_empty() && pixels.len() != expected {
            return Err(format!(
                "pixel canvas expected {expected} colors for {}x{}, got {}",
                width.max(1),
                height.max(1),
                pixels.len()
            ));
        }
        Ok(Self::from_kind(BindingWidgetKind::PixelCanvas {
            state,
            name: name.into(),
            width: width.max(1),
            height: height.max(1),
            paper_color,
            desired_size,
            viewport,
            fit_on_first_layout,
            pixels,
        }))
    }

    pub fn padding(
        child: BindingWidget,
        insets: Insets,
        fill_child_width: bool,
        fill_child_height: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Padding {
            child,
            insets,
            fill_child_width,
            fill_child_height,
        })
    }

    pub fn align(child: BindingWidget, horizontal: Alignment, vertical: Alignment) -> Self {
        Self::from_kind(BindingWidgetKind::Align {
            child,
            horizontal,
            vertical,
        })
    }

    pub fn background(child: BindingWidget, color: Color) -> Self {
        Self::from_kind(BindingWidgetKind::Background { child, color })
    }

    pub fn sized_box(
        child: Option<BindingWidget>,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SizedBox {
            child,
            width,
            height,
        })
    }

    pub fn stack(
        children: impl IntoIterator<Item = BindingWidget>,
        axis: Axis,
        spacing: f32,
        alignment: Alignment,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Stack {
            children: children.into_iter().collect(),
            axis,
            spacing,
            alignment,
        })
    }

    pub fn semantic_region(
        name: impl Into<BindingText>,
        child: BindingWidget,
        description: Option<BindingText>,
        role: SemanticsRole,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::SemanticRegion {
            name: name.into(),
            child,
            description,
            role,
        })
    }

    pub fn form_row(
        label: impl Into<String>,
        control: BindingWidget,
        stacked: bool,
        label_width: Option<f32>,
        control_width: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FormRow {
            label: label.into(),
            control,
            stacked,
            label_width,
            control_width,
            gap,
        })
    }

    pub fn field_group(
        children: impl IntoIterator<Item = BindingWidget>,
        spacing: Option<f32>,
        padding: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FieldGroup {
            children: children.into_iter().collect(),
            spacing,
            padding,
            max_width,
            fill_width,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn form_section(
        title: impl Into<String>,
        child: BindingWidget,
        description: Option<String>,
        header_action: Option<BindingWidget>,
        padding: Option<f32>,
        body_gap: Option<f32>,
        header_gap: Option<f32>,
        max_width: Option<f32>,
        fill_width: bool,
        radius: Option<f32>,
        elevation: Option<SurfaceElevation>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::FormSection {
            title: title.into(),
            child,
            description,
            header_action,
            padding,
            body_gap,
            header_gap,
            max_width,
            fill_width,
            radius,
            elevation,
        })
    }

    pub fn panel_section(
        title: impl Into<String>,
        child: BindingWidget,
        header_action: Option<BindingWidget>,
        gap: Option<f32>,
        action_gap: Option<f32>,
        collapsible: bool,
        expanded: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PanelSection {
            title: title.into(),
            child,
            header_action,
            gap,
            action_gap,
            collapsible,
            expanded,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dock_panel(
        title: impl Into<String>,
        child: BindingWidget,
        name: Option<String>,
        header_height: Option<f32>,
        padding: Option<f32>,
        background: Option<Color>,
        header_background: Option<Color>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::DockPanel {
            title: title.into(),
            child,
            name,
            header_height,
            padding,
            background,
            header_background,
        })
    }

    pub fn dock_workspace(
        state: BindingDockState,
        panels: impl IntoIterator<Item = BindingDockPanel>,
        name: impl Into<String>,
    ) -> Result<Self, String> {
        let panels = panels.into_iter().collect::<Vec<_>>();
        let mut ids = std::collections::BTreeSet::new();
        for panel in &panels {
            if !ids.insert(panel.id) {
                return Err(format!(
                    "dock panel {} is registered more than once",
                    panel.id
                ));
            }
        }
        Ok(Self::from_kind(BindingWidgetKind::DockWorkspace {
            state,
            panels,
            name: name.into(),
        }))
    }

    pub fn status_bar_host(content: BindingWidget, status_bar: BindingWidget) -> Self {
        Self::from_kind(BindingWidgetKind::StatusBarHost {
            content,
            status_bar,
        })
    }

    pub fn tooltip(
        text: impl Into<String>,
        child: BindingWidget,
        placement: TooltipPlacement,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Tooltip {
            text: text.into(),
            child,
            placement,
        })
    }

    pub fn popover(
        name: impl Into<String>,
        trigger: BindingWidget,
        content: BindingWidget,
        open: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Popover {
            name: name.into(),
            trigger,
            content,
            open,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tool_palette(
        name: impl Into<String>,
        items: impl IntoIterator<Item = BindingToolPaletteItem>,
        selected: Option<BindingNumber>,
        axis: Axis,
        action: Option<BindingSelectAction>,
        extent: Option<f32>,
        padding: Option<f32>,
        spacing: Option<f32>,
        item_size: Option<f32>,
        icon_size: Option<f32>,
        background: Option<Color>,
        divider: bool,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ToolPalette {
            name: name.into(),
            items: items.into_iter().collect(),
            selected,
            axis,
            action,
            extent,
            padding,
            spacing,
            item_size,
            icon_size,
            background,
            divider,
        })
    }

    pub fn preset_strip(
        name: impl Into<String>,
        presets: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        action: Option<BindingSelectAction>,
        item_width: Option<f32>,
        item_height: Option<f32>,
        gap: Option<f32>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::PresetStrip {
            name: name.into(),
            presets: presets.into_iter().map(Into::into).collect(),
            selected,
            action,
            item_width,
            item_height,
            gap,
        })
    }

    pub fn browser_tab_bar(
        name: impl Into<String>,
        tabs: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<BindingNumber>,
        on_change: Option<BindingSelectAction>,
        on_close: Option<BindingSelectAction>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::BrowserTabBar {
            name: name.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
            on_change,
            on_close,
        })
    }

    pub fn scroll_view(
        child: BindingWidget,
        axes: BindingScrollAxes,
        name: Option<String>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::ScrollView { child, axes, name })
    }

    pub fn column(children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::flex(Axis::Vertical, children, gap)
    }

    pub fn row(children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::flex(Axis::Horizontal, children, gap)
    }

    pub fn flex(axis: Axis, children: impl IntoIterator<Item = BindingWidget>, gap: f32) -> Self {
        Self::from_kind(BindingWidgetKind::Flex {
            axis,
            gap: gap.max(0.0),
            children: children.into_iter().collect(),
        })
    }

    pub fn foreign(callbacks: impl ForeignWidgetCallbacks) -> Self {
        Self::foreign_arc(Arc::new(callbacks))
    }

    pub fn foreign_arc(callbacks: Arc<dyn ForeignWidgetCallbacks>) -> Self {
        Self::foreign_arc_with_children(callbacks, [])
    }

    pub fn foreign_arc_with_children(
        callbacks: Arc<dyn ForeignWidgetCallbacks>,
        children: impl IntoIterator<Item = BindingWidget>,
    ) -> Self {
        Self::from_kind(BindingWidgetKind::Foreign {
            callbacks,
            children: children.into_iter().collect(),
        })
    }

    fn from_kind(kind: BindingWidgetKind) -> Self {
        Self {
            inner: Arc::new(kind),
        }
    }

    fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        match self.inner.as_ref() {
            BindingWidgetKind::Label { text } => text.bind_ui_handle(handle),
            BindingWidgetKind::Button { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::Icon { .. } => {}
            BindingWidgetKind::IconButton {
                label,
                selected,
                enabled,
                ..
            } => {
                label.bind_ui_handle(handle);
                selected.bind_ui_handle(handle);
                enabled.bind_ui_handle(handle);
            }
            BindingWidgetKind::Link {
                label,
                url,
                enabled,
                ..
            } => {
                label.bind_ui_handle(handle);
                url.bind_ui_handle(handle);
                enabled.bind_ui_handle(handle);
            }
            BindingWidgetKind::Checkbox { label, checked, .. } => {
                label.bind_ui_handle(handle);
                checked.bind_ui_handle(handle);
            }
            BindingWidgetKind::Switch { label, on, .. } => {
                label.bind_ui_handle(handle);
                on.bind_ui_handle(handle);
            }
            BindingWidgetKind::RadioButton {
                label, selected, ..
            } => {
                label.bind_ui_handle(handle);
                selected.bind_ui_handle(handle);
            }
            BindingWidgetKind::RadioGroup { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SegmentedControl { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Breadcrumb { name, current, .. } => {
                name.bind_ui_handle(handle);
                if let Some(current) = current {
                    current.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ListView { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Table { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::TreeView { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::LayerList { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Menu {
                name, highlighted, ..
            } => {
                name.bind_ui_handle(handle);
                if let Some(highlighted) = highlighted {
                    highlighted.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ContextMenu { trigger, .. } => {
                trigger.bind_ui_handle(handle);
            }
            BindingWidgetKind::TabBar { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Tabs { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Dialog {
                title,
                content,
                shown,
            } => {
                title.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
            }
            BindingWidgetKind::SignalMeter { name, active, .. } => {
                name.bind_ui_handle(handle);
                active.bind_ui_handle(handle);
            }
            BindingWidgetKind::StatusBadge { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::StatusBar {
                segments,
                description,
                ..
            } => {
                for segment in segments {
                    segment.bind_ui_handle(handle);
                }
                if let Some(description) = description {
                    description.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::DetailRow { label, value, .. } => {
                label.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::Slider { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::NumberInput { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::Select { name, selected, .. } => {
                name.bind_ui_handle(handle);
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ProgressBar { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::BusyIndicator { name, label, .. } => {
                name.bind_ui_handle(handle);
                if let Some(label) = label {
                    label.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::TextInput { name, value, .. }
            | BindingWidgetKind::PasswordInput { name, value, .. }
            | BindingWidgetKind::DateTimeInput { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::TextArea { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::RichText { .. } | BindingWidgetKind::RichDocument { .. } => {}
            BindingWidgetKind::Image { .. } => {}
            BindingWidgetKind::ColorSwatch { .. } => {}
            BindingWidgetKind::ColorPalette { selected, .. } => {
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ColorPicker { .. } | BindingWidgetKind::SimpleColorPicker { .. } => {
            }
            BindingWidgetKind::Separator { .. } => {}
            BindingWidgetKind::EmptyState { action, .. } => {
                if let Some(action) = action {
                    action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ActionCard { enabled, .. } => enabled.bind_ui_handle(handle),
            BindingWidgetKind::BrushPreview { .. }
            | BindingWidgetKind::CoverageDots { .. }
            | BindingWidgetKind::SectionLabel { .. } => {}
            BindingWidgetKind::CommandGroup { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SwitchView { children, selected } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
                selected.bind_ui_handle(handle);
            }
            BindingWidgetKind::Dock {
                body, top, bottom, ..
            } => {
                body.bind_ui_handle(handle);
                if let Some((_, top)) = top {
                    top.bind_ui_handle(handle);
                }
                if let Some((_, bottom)) = bottom {
                    bottom.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FixedPaneSplit {
                first,
                divider,
                second,
                ..
            } => {
                first.bind_ui_handle(handle);
                divider.bind_ui_handle(handle);
                second.bind_ui_handle(handle);
            }
            BindingWidgetKind::FramedField {
                child,
                focused,
                invalid,
                ..
            } => {
                child.bind_ui_handle(handle);
                focused.bind_ui_handle(handle);
                invalid.bind_ui_handle(handle);
            }
            BindingWidgetKind::MeasuredBottomDock { body, bottom, .. } => {
                body.bind_ui_handle(handle);
                bottom.bind_ui_handle(handle);
            }
            BindingWidgetKind::PlacementBadge { label, .. } => label.bind_ui_handle(handle),
            BindingWidgetKind::PropertyRow { control, .. } => control.bind_ui_handle(handle),
            BindingWidgetKind::SideSheet {
                body,
                shown,
                header_action,
                actions,
                ..
            } => {
                body.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
                if let Some(header_action) = header_action {
                    header_action.bind_ui_handle(handle);
                }
                for action in actions {
                    action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SplitView {
                first,
                second,
                ratio,
                ..
            } => {
                first.bind_ui_handle(handle);
                second.bind_ui_handle(handle);
                ratio.bind_ui_handle(handle);
            }
            BindingWidgetKind::TrailingSlotRow { body, trailing, .. } => {
                body.bind_ui_handle(handle);
                trailing.bind_ui_handle(handle);
            }
            BindingWidgetKind::VirtualScrollView { children, .. }
            | BindingWidgetKind::ReorderableList { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FloatingStack { windows, .. } => {
                for window in windows {
                    window.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Surface { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::ExternalSurface { .. } => {}
            BindingWidgetKind::Toolbar { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Grid { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::AdaptiveView {
                compact,
                medium,
                expanded,
                ..
            } => {
                compact.bind_ui_handle(handle);
                medium.bind_ui_handle(handle);
                expanded.bind_ui_handle(handle);
            }
            BindingWidgetKind::ConstraintView { cases, fallback } => {
                for case in cases {
                    case.child.bind_ui_handle(handle);
                }
                fallback.bind_ui_handle(handle);
            }
            BindingWidgetKind::ResponsiveSidebar {
                sidebar, content, ..
            } => {
                sidebar.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
            }
            BindingWidgetKind::MasterDetail { master, detail, .. } => {
                master.bind_ui_handle(handle);
                detail.bind_ui_handle(handle);
            }
            BindingWidgetKind::OverlayHost { child } => child.bind_ui_handle(handle),
            BindingWidgetKind::NotificationHost { .. } => {}
            BindingWidgetKind::CommandPalette { content, shown, .. } => {
                content.bind_ui_handle(handle);
                shown.bind_ui_handle(handle);
            }
            BindingWidgetKind::VirtualList { .. } => {}
            BindingWidgetKind::Canvas { .. } | BindingWidgetKind::CanvasRuler { .. } => {}
            BindingWidgetKind::DragDropHost { child, .. }
            | BindingWidgetKind::Draggable { child, .. }
            | BindingWidgetKind::DropTarget { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::FloatingWorkspace { views, .. } => {
                for view in views {
                    view.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::PixelCanvas { .. } => {}
            BindingWidgetKind::Padding { child, .. }
            | BindingWidgetKind::Align { child, .. }
            | BindingWidgetKind::Background { child, .. }
            | BindingWidgetKind::AspectRatio { child, .. }
            | BindingWidgetKind::SafeArea { child, .. }
            | BindingWidgetKind::LayoutTransition { child, .. }
            | BindingWidgetKind::DockPanel { child, .. }
            | BindingWidgetKind::Tooltip { child, .. } => {
                child.bind_ui_handle(handle);
            }
            BindingWidgetKind::DockWorkspace { panels, .. } => {
                for panel in panels {
                    panel.child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SizedBox { child, .. } => {
                if let Some(child) = child {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Stack { children, .. }
            | BindingWidgetKind::FieldGroup { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::SemanticRegion {
                name,
                child,
                description,
                ..
            } => {
                name.bind_ui_handle(handle);
                child.bind_ui_handle(handle);
                if let Some(description) = description {
                    description.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::FormRow { control, .. } => control.bind_ui_handle(handle),
            BindingWidgetKind::FormSection {
                child,
                header_action,
                ..
            }
            | BindingWidgetKind::PanelSection {
                child,
                header_action,
                ..
            } => {
                child.bind_ui_handle(handle);
                if let Some(header_action) = header_action {
                    header_action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => {
                content.bind_ui_handle(handle);
                status_bar.bind_ui_handle(handle);
            }
            BindingWidgetKind::Popover {
                trigger, content, ..
            } => {
                trigger.bind_ui_handle(handle);
                content.bind_ui_handle(handle);
            }
            BindingWidgetKind::ToolPalette { selected, .. }
            | BindingWidgetKind::PresetStrip { selected, .. }
            | BindingWidgetKind::BrowserTabBar { selected, .. } => {
                if let Some(selected) = selected {
                    selected.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ScrollView { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::Flex { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Foreign { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
        }
    }

    fn into_runtime_widget(&self, context: BindingBuildContext) -> BindingRuntimeWidget {
        let errors = context;
        match self.inner.as_ref() {
            BindingWidgetKind::Label { text } => {
                let mut label = Label::dynamic(text.resolve(), {
                    let text = text.clone();
                    move || text.resolve()
                });
                if let Some(theme) = errors.theme.clone() {
                    label = label.style_when(move || theme.snapshot().body_text_style());
                }
                BindingRuntimeWidget::new(label)
            }
            BindingWidgetKind::Button { label, action } => {
                let mut button = Button::new(label.resolve());
                if let Some(action) = action.clone() {
                    button = button.on_press({
                        let errors = errors.clone();
                        move || {
                            if let Err(error) = action.run() {
                                errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(button, errors))
            }
            BindingWidgetKind::Icon {
                glyph,
                label,
                size,
                color,
            } => {
                let mut icon = Icon::new(*glyph);
                if let Some(label) = label {
                    icon = icon.label(label.clone());
                }
                if let Some(size) = size {
                    icon = icon.size(*size);
                }
                if let Some(color) = color {
                    icon = icon.color(*color);
                } else if let Some(theme) = errors.theme.clone() {
                    icon = icon.color_when(move || theme.snapshot().palette.text);
                }
                BindingRuntimeWidget::new(icon)
            }
            BindingWidgetKind::IconButton {
                glyph,
                label,
                selected,
                enabled,
                size,
                icon_size,
                description,
                action,
            } => {
                let mut button = IconButton::new(*glyph, label.resolve())
                    .selected(selected.resolve())
                    .enabled(enabled.resolve());
                if let Some(size) = size {
                    button = button.size(*size);
                }
                if let Some(icon_size) = icon_size {
                    button = button.icon_size(*icon_size);
                }
                if let Some(description) = description {
                    button = button.description(description.clone());
                }
                if matches!(selected, BindingBool::State(_)) {
                    let selected = selected.clone();
                    button = button.selected_when(move || selected.resolve());
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    button = button.enabled_when(move || enabled.resolve());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    button = button.on_press(move || {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(button, errors))
            }
            BindingWidgetKind::Link {
                label,
                url,
                semantic_name,
                enabled,
                action,
            } => {
                let mut link = Link::new(label.resolve(), url.resolve()).enabled(enabled.resolve());
                if matches!(label, BindingText::State(_)) {
                    let label = label.clone();
                    link = link.label_when(move || label.resolve());
                }
                if matches!(url, BindingText::State(_)) {
                    let url = url.clone();
                    link = link.url_when(move || url.resolve());
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    link = link.enabled_when(move || enabled.resolve());
                }
                if let Some(semantic_name) = semantic_name {
                    link = link.semantic_name(semantic_name.clone());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    link = link.on_open(move |url| {
                        if let Err(error) = action.run(url.to_string()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(link, errors))
            }
            BindingWidgetKind::Checkbox {
                label,
                checked,
                action,
            } => {
                let mut checkbox = Checkbox::new(label.resolve()).checked(checked.resolve());
                let state = checked.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    checkbox = checkbox.on_toggle(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingCheckboxWidget {
                    inner: themed_widget!(checkbox, errors),
                    checked: checked.clone(),
                })
            }
            BindingWidgetKind::Switch { label, on, action } => {
                let mut switch = Switch::new(label.resolve()).on(on.resolve());
                let state = on.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    switch = switch.on_toggle(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingSwitchWidget {
                    inner: themed_widget!(switch, errors),
                    on: on.clone(),
                })
            }
            BindingWidgetKind::RadioButton {
                label,
                selected,
                action,
            } => {
                let mut radio = RadioButton::new(label.resolve()).selected(selected.resolve());
                let state = selected.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    radio = radio.on_select(move || {
                        if let Some(state) = &state {
                            state.set(true);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run()
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingRadioButtonWidget {
                    inner: themed_widget!(radio, errors),
                    selected: selected.clone(),
                })
            }
            BindingWidgetKind::RadioGroup {
                name,
                options,
                selected,
                action,
            } => {
                let mut radio_group = RadioGroup::new(name.resolve()).options(options.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        radio_group = radio_group.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        radio_group = radio_group
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    radio_group = radio_group.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(radio_group, errors))
            }
            BindingWidgetKind::SegmentedControl {
                name,
                items,
                selected,
                action,
            } => {
                let mut control = SegmentedControl::new(name.resolve())
                    .items(items.iter().map(|item| item.into_sui()));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        control = control.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        control = control
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    control = control.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(control, errors))
            }
            BindingWidgetKind::Breadcrumb {
                name,
                items,
                current,
                action,
            } => {
                let mut breadcrumb = Breadcrumb::new(name.resolve())
                    .items(items.iter().cloned().map(BreadcrumbItem::new));
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    breadcrumb = breadcrumb.name_when(move || name.resolve());
                }
                if let Some(current) = current {
                    if let Some(index) = binding_number_to_index(current.resolve()) {
                        breadcrumb = breadcrumb.current(index);
                    }
                    if matches!(current, BindingNumber::State(_)) {
                        let current = current.clone();
                        breadcrumb = breadcrumb
                            .current_when(move || binding_number_to_index(current.resolve()));
                    }
                }
                let state = current.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    breadcrumb = breadcrumb.on_activate(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(breadcrumb, errors))
            }
            BindingWidgetKind::ListView {
                name,
                items,
                selected,
                action,
            } => {
                let mut list_view =
                    ListView::new(name.resolve()).items(items.iter().cloned().map(ListItem::new));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        list_view = list_view.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        list_view = list_view
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    list_view = list_view.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(list_view, errors))
            }
            BindingWidgetKind::Table {
                name,
                columns,
                rows,
                selected,
                action,
            } => {
                let row_values: Vec<String> = rows
                    .iter()
                    .map(|row| row.cells.first().cloned().unwrap_or_default())
                    .collect();
                let mut table = Table::new(name.resolve())
                    .columns(columns.iter().map(BindingTableColumn::into_sui))
                    .rows(rows.iter().map(BindingTableRow::into_sui));
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    table = table.name_when(move || name.resolve());
                }
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        table = table.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        table = table
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    table = table.on_change(move |index| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        let value = row_values.get(index).cloned().unwrap_or_default();
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(table, errors))
            }
            BindingWidgetKind::TreeView {
                name,
                items,
                selected,
                action,
            } => {
                let mut tree_view = TreeView::new(name.resolve())
                    .items(items.iter().map(BindingTreeItem::into_sui));
                if let Some(selected) = selected
                    && let Some(index) = binding_number_to_index(selected.resolve())
                {
                    tree_view = tree_view.selected(index);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    tree_view = tree_view.on_change(move |path, value| {
                        let index = path.first().copied().unwrap_or(0);
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(tree_view, errors))
            }
            BindingWidgetKind::LayerList {
                name,
                items,
                selected,
                action,
            } => {
                let mut layer_list = LayerList::new(name.resolve())
                    .layers(items.iter().map(BindingLayerListItem::into_sui));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        layer_list = layer_list.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        layer_list = layer_list
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    layer_list = layer_list.on_select(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(layer_list, errors))
            }
            BindingWidgetKind::Menu {
                name,
                items,
                highlighted,
                action,
            } => {
                let mut menu =
                    Menu::new(name.resolve()).items(items.iter().map(BindingMenuItem::into_sui));
                if let Some(highlighted) = highlighted
                    && let Some(index) = binding_number_to_index(highlighted.resolve())
                {
                    menu = menu.highlighted(index);
                }
                let state = highlighted.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    menu = menu.on_activate(move |index, item| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, item.label().to_string())
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(menu, errors))
            }
            BindingWidgetKind::ContextMenu {
                name,
                trigger,
                items,
                action,
            } => {
                let mut menu =
                    ContextMenu::new(name.clone(), trigger.into_runtime_widget(errors.clone()))
                        .items(items.iter().map(BindingMenuItem::into_sui));
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    menu = menu.on_activate(move |index, item| {
                        if let Err(error) = action.run(index, item.label().to_string()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(menu, errors))
            }
            BindingWidgetKind::TabBar {
                name,
                tabs,
                selected,
                action,
            } => {
                let mut tab_bar = TabBar::new(name.resolve()).tabs(tabs.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        tab_bar = tab_bar.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        tab_bar = tab_bar
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(tab_bar, errors))
            }
            BindingWidgetKind::Tabs {
                name,
                tabs,
                selected,
            } => {
                let mut tab_widget = Tabs::new(name.resolve());
                if let Some(selected) = selected
                    && let Some(index) = binding_number_to_index(selected.resolve())
                {
                    tab_widget = tab_widget.selected(index);
                }
                for label in tabs {
                    tab_widget = tab_widget.tab(label.clone(), Label::new(label.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(tab_widget, errors))
            }
            BindingWidgetKind::Dialog {
                title,
                content,
                shown,
            } => {
                let dialog =
                    Dialog::new(title.resolve(), content.into_runtime_widget(errors.clone()))
                        .shown(shown.resolve());
                BindingRuntimeWidget::new(dialog)
            }
            BindingWidgetKind::SignalMeter {
                name,
                active,
                description,
                bars,
                size,
            } => {
                let mut signal_meter = SignalMeter::new(name.resolve())
                    .active(active.resolve())
                    .bars(*bars);
                if let Some(description) = description {
                    signal_meter = signal_meter.description(description.clone());
                }
                if let Some(size) = size {
                    signal_meter = signal_meter.size(*size);
                }
                if matches!(active, BindingBool::State(_)) {
                    let active = active.clone();
                    signal_meter = signal_meter.active_when(move || active.resolve());
                }
                BindingRuntimeWidget::new(themed_widget!(signal_meter, errors))
            }
            BindingWidgetKind::StatusBadge {
                label,
                tone,
                icon,
                min_width,
            } => {
                let mut badge = if matches!(label, BindingText::State(_)) {
                    StatusBadge::dynamic(label.resolve(), {
                        let label = label.clone();
                        move || label.resolve()
                    })
                } else {
                    StatusBadge::new(label.resolve())
                }
                .tone(*tone);
                if let Some(icon) = icon {
                    badge = badge.icon(*icon);
                }
                if let Some(min_width) = min_width {
                    badge = badge.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(badge, errors))
            }
            BindingWidgetKind::StatusBar {
                segments,
                name,
                description,
                height,
            } => {
                let mut status_bar = StatusBar::new();
                if let Some(name) = name {
                    status_bar = status_bar.name(name.clone());
                }
                if let Some(description) = description {
                    status_bar = status_bar.description(description.resolve());
                    if matches!(description, BindingText::State(_)) {
                        let description = description.clone();
                        status_bar = status_bar.description_when(move || description.resolve());
                    }
                }
                if let Some(height) = height {
                    status_bar = status_bar.height(*height);
                }
                for segment in segments {
                    status_bar = status_bar.segment(segment.into_sui());
                }
                BindingRuntimeWidget::new(themed_widget!(status_bar, errors))
            }
            BindingWidgetKind::DetailRow {
                label,
                value,
                max_value_lines,
            } => {
                let mut detail_row = DetailRow::new(label.resolve(), value.resolve());
                if matches!(label, BindingText::State(_)) {
                    let label = label.clone();
                    detail_row = detail_row.label_when(move || label.resolve());
                }
                if matches!(value, BindingText::State(_)) {
                    let value = value.clone();
                    detail_row = detail_row.value_when(move || value.resolve());
                }
                if let Some(max_value_lines) = max_value_lines {
                    detail_row = detail_row.max_value_lines(*max_value_lines);
                }
                BindingRuntimeWidget::new(themed_widget!(detail_row, errors))
            }
            BindingWidgetKind::Slider {
                name,
                value,
                min,
                max,
                step,
                action,
            } => {
                let mut slider = Slider::new(name.resolve())
                    .range(*min, *max)
                    .step(*step)
                    .value(value.resolve());
                if matches!(value, BindingNumber::State(_)) {
                    let value = value.clone();
                    slider = slider.value_when(move || value.resolve());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    slider = slider.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(slider, errors))
            }
            BindingWidgetKind::NumberInput {
                name,
                value,
                min,
                max,
                step,
                precision,
                action,
            } => {
                let mut number_input = NumberInput::new(name.resolve())
                    .range(*min, *max)
                    .step(*step)
                    .precision(*precision)
                    .value(value.resolve());
                if matches!(value, BindingNumber::State(_)) {
                    let value = value.clone();
                    number_input = number_input.value_when(move || value.resolve());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    number_input = number_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(number_input, errors))
            }
            BindingWidgetKind::Select {
                name,
                options,
                selected,
                placeholder,
                action,
            } => {
                let mut select = Select::new(name.resolve()).options(options.clone());
                if let Some(placeholder) = placeholder {
                    select = select.placeholder(placeholder.clone());
                }
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        select = select.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        select = select
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    select = select.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(select, errors))
            }
            BindingWidgetKind::ProgressBar {
                name,
                value,
                min,
                max,
                show_value,
            } => BindingRuntimeWidget::new(BindingProgressBarWidget {
                name: name.clone(),
                value: value.clone(),
                min: *min,
                max: *max,
                show_value: *show_value,
                theme: errors.theme.clone(),
            }),
            BindingWidgetKind::BusyIndicator { name, label, size } => {
                BindingRuntimeWidget::new(BindingBusyIndicatorWidget {
                    name: name.clone(),
                    label: label.clone(),
                    size: *size,
                    theme: errors.theme.clone(),
                })
            }
            BindingWidgetKind::TextInput {
                name,
                value,
                placeholder,
                action,
            } => {
                let mut text_input = TextInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    text_input = text_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    text_input = text_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingTextInputWidget {
                    inner: themed_widget!(text_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::PasswordInput {
                name,
                value,
                placeholder,
                action,
            } => {
                let mut password_input = PasswordInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    password_input = password_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    password_input = password_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingPasswordInputWidget {
                    inner: themed_widget!(password_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::DateTimeInput {
                name,
                value,
                placeholder,
                action,
            } => {
                let mut datetime_input = DateTimeInput::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    datetime_input = datetime_input.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    datetime_input = datetime_input.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingDateTimeInputWidget {
                    inner: themed_widget!(datetime_input, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::TextArea {
                name,
                value,
                placeholder,
                action,
            } => {
                let mut text_area = TextArea::new(name.resolve()).value(value.resolve());
                if let Some(placeholder) = placeholder {
                    text_area = text_area.placeholder(placeholder.clone());
                }
                let state = value.state();
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    text_area = text_area.on_change(move |value| {
                        if let Some(state) = &state {
                            state.set(value.clone());
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(BindingTextAreaWidget {
                    inner: themed_widget!(text_area, errors),
                    value: value.clone(),
                })
            }
            BindingWidgetKind::RichText {
                spans,
                semantic_name,
                min_width,
                min_height,
            } => {
                let mut rich_text =
                    RichText::from_spans(spans.iter().map(BindingTextSpan::into_sui).collect());
                if let Some(semantic_name) = semantic_name {
                    rich_text = rich_text.semantic_name(semantic_name.clone());
                }
                if *min_width > 0.0 {
                    rich_text = rich_text.min_width(*min_width);
                }
                if *min_height > 0.0 {
                    rich_text = rich_text.min_height(*min_height);
                }
                BindingRuntimeWidget::new(rich_text)
            }
            BindingWidgetKind::RichDocument {
                document,
                on_link,
                on_image,
                on_attachment,
            } => {
                let mut view = RichDocumentView::new(document.inner.clone());
                if let Some(action) = on_link.clone() {
                    let errors = errors.clone();
                    view = view.on_link(move |destination| {
                        if let Err(error) = action.run(destination.to_owned()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_image.clone() {
                    let errors = errors.clone();
                    view = view.on_image(move |source| {
                        if let Err(error) = action.run(source.to_owned()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_attachment.clone() {
                    let errors = errors.clone();
                    view = view.on_attachment(move |id| {
                        if let Err(error) = action.run(id.get()) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(view, errors))
            }
            BindingWidgetKind::Image {
                image,
                label,
                fit,
                size,
            } => {
                let mut image = Image::new(image.into_sui()).fit((*fit).into());
                if let Some(label) = label {
                    image = image.label(label.clone());
                }
                if let Some(size) = size {
                    image = image.size(*size);
                }
                BindingRuntimeWidget::new(themed_widget!(image, errors))
            }
            BindingWidgetKind::ColorSwatch {
                name,
                color,
                size,
                read_only,
                action,
            } => {
                let mut swatch = ColorSwatch::new(name.clone(), *color);
                if let Some(size) = size {
                    swatch = swatch.size(*size);
                }
                if *read_only {
                    swatch = swatch.read_only();
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    swatch = swatch.on_press(move |_| {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(swatch, errors))
            }
            BindingWidgetKind::ColorPalette {
                name,
                swatches,
                selected,
                action,
                columns,
                swatch_size,
                gap,
            } => {
                let mut palette = ColorPalette::new(name.clone())
                    .swatches(swatches.iter().map(BindingColorPaletteSwatch::into_sui));
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        palette = palette.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        palette = palette
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(columns) = columns {
                    palette = palette.columns(*columns);
                }
                if let Some(swatch_size) = swatch_size {
                    palette = palette.swatch_size(*swatch_size);
                }
                if let Some(gap) = gap {
                    palette = palette.gap(*gap);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    palette = palette.on_change(move |index, name, color| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, name, color)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(palette, errors))
            }
            BindingWidgetKind::ColorPicker {
                name,
                color,
                action,
                show_alpha,
                compact,
            } => {
                let mut picker = if let Some(color) = color {
                    ColorPicker::from_color(name.clone(), *color)
                } else {
                    ColorPicker::new(name.clone())
                }
                .show_alpha(*show_alpha)
                .compact(*compact);
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    picker = picker.on_change(move |color| {
                        if let Err(error) = action.run(color) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(picker, errors))
            }
            BindingWidgetKind::SimpleColorPicker {
                name,
                color,
                mode,
                action,
                show_alpha,
                compact,
            } => {
                let mut picker = if let Some(color) = color {
                    SimpleColorPicker::from_color(name.clone(), *color)
                } else {
                    SimpleColorPicker::new(name.clone())
                }
                .mode(*mode)
                .show_alpha(*show_alpha)
                .compact(*compact);
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    picker = picker.on_change(move |color| {
                        if let Err(error) = action.run(color) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(picker, errors))
            }
            BindingWidgetKind::Separator {
                axis,
                name,
                inset,
                thickness,
                length,
            } => {
                let mut separator = Separator::new(*axis).inset(*inset);
                if let Some(name) = name {
                    separator = separator.name(name.clone());
                }
                if let Some(thickness) = thickness {
                    separator = separator.thickness(*thickness);
                }
                if let Some(length) = length {
                    separator = separator.length(*length);
                }
                BindingRuntimeWidget::new(themed_widget!(separator, errors))
            }
            BindingWidgetKind::EmptyState {
                title,
                description,
                name,
                detail,
                icon,
                action,
                background,
                transparent,
            } => {
                let mut empty_state = EmptyState::new(title.clone(), description.clone());
                if let Some(name) = name {
                    empty_state = empty_state.name(name.clone());
                }
                if let Some(detail) = detail {
                    empty_state = empty_state.detail(detail.clone());
                }
                if let Some(icon) = icon {
                    empty_state = empty_state.icon(*icon);
                }
                if *transparent {
                    empty_state = empty_state.transparent();
                } else if let Some(background) = background {
                    empty_state = empty_state.background(*background);
                }
                if let Some(action) = action {
                    empty_state = empty_state.action(action.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(empty_state, errors))
            }
            BindingWidgetKind::ActionCard {
                title,
                description,
                icon,
                tone,
                enabled,
                action,
            } => {
                let mut card = ActionCard::new(title.clone(), description.clone())
                    .tone(*tone)
                    .enabled(enabled.resolve());
                if let Some(icon) = icon {
                    card = card.icon(*icon);
                }
                if matches!(enabled, BindingBool::State(_)) {
                    let enabled = enabled.clone();
                    card = card.enabled_when(move || enabled.resolve());
                }
                if let Some(action) = action.clone() {
                    let errors = errors.clone();
                    card = card.on_press(move || {
                        if let Err(error) = action.run() {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(card, errors))
            }
            BindingWidgetKind::BrushPreview {
                name,
                kind,
                spec,
                size,
            } => {
                let mut preview = BrushPreview::new(name.clone())
                    .kind(kind.clone())
                    .spec(spec.into_sui());
                if let Some(size) = size {
                    preview = preview.size(*size);
                }
                BindingRuntimeWidget::new(themed_widget!(preview, errors))
            }
            BindingWidgetKind::CommandGroup {
                name,
                children,
                axis,
                padding,
                spacing,
                corner_radius,
                background,
                border,
            } => {
                let mut group = CommandGroup::new(*axis, name.clone());
                if let Some(padding) = padding {
                    group = group.padding(*padding);
                }
                if let Some(spacing) = spacing {
                    group = group.spacing(*spacing);
                }
                if let Some(corner_radius) = corner_radius {
                    group = group.corner_radius(*corner_radius);
                }
                if let Some(background) = background {
                    group = group.background(*background);
                }
                if let Some(border) = border {
                    group = group.border(*border);
                }
                for child in children {
                    group = group.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(group, errors))
            }
            BindingWidgetKind::CoverageDots {
                name,
                current,
                target,
                tone,
                max_dots,
                show_label,
                min_width,
            } => {
                let mut dots = CoverageDots::new(name.clone(), *current, *target)
                    .tone(*tone)
                    .max_dots(*max_dots)
                    .show_label(*show_label);
                if let Some(min_width) = min_width {
                    dots = dots.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(dots, errors))
            }
            BindingWidgetKind::Dock {
                body,
                top,
                bottom,
                fallback_width,
                fallback_body_height,
            } => {
                let mut dock = Dock::new(body.into_runtime_widget(errors.clone()))
                    .fallback_width(*fallback_width)
                    .fallback_body_height(*fallback_body_height);
                if let Some((height, top)) = top {
                    dock = dock.top(*height, top.into_runtime_widget(errors.clone()));
                }
                if let Some((height, bottom)) = bottom {
                    dock = dock.bottom(*height, bottom.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(dock)
            }
            BindingWidgetKind::FixedPaneSplit {
                axis,
                first,
                divider,
                second,
                fixed_second,
                fixed_extent,
                divider_extent,
                fallback_flexible_extent,
            } => {
                let mut split = FixedPaneSplit::new(
                    *axis,
                    first.into_runtime_widget(errors.clone()),
                    divider.into_runtime_widget(errors.clone()),
                    second.into_runtime_widget(errors.clone()),
                );
                split = if *fixed_second {
                    split.fixed_second(*fixed_extent)
                } else {
                    split.fixed_first(*fixed_extent)
                };
                BindingRuntimeWidget::new(
                    split
                        .divider_extent(*divider_extent)
                        .fallback_flexible_extent(*fallback_flexible_extent),
                )
            }
            BindingWidgetKind::FramedField {
                child,
                name,
                description,
                padding,
                min_height,
                fill_width,
                focused,
                invalid,
            } => {
                let mut field = FramedField::new(child.into_runtime_widget(errors.clone()))
                    .focused(focused.resolve())
                    .invalid(invalid.resolve());
                if let Some(name) = name {
                    field = field.name(name.clone());
                }
                if let Some(description) = description {
                    field = field.description(description.clone());
                }
                if let Some(padding) = padding {
                    field = field.padding(*padding);
                }
                if let Some(min_height) = min_height {
                    field = field.min_height(*min_height);
                }
                if *fill_width {
                    field = field.fill_width();
                }
                if matches!(focused, BindingBool::State(_)) {
                    let focused = focused.clone();
                    field = field.focused_when(move || focused.resolve());
                }
                if matches!(invalid, BindingBool::State(_)) {
                    let invalid = invalid.clone();
                    field = field.invalid_when(move || invalid.resolve());
                }
                BindingRuntimeWidget::new(themed_widget!(field, errors))
            }
            BindingWidgetKind::MeasuredBottomDock {
                body,
                bottom,
                fallback_size,
            } => BindingRuntimeWidget::new(
                MeasuredBottomDock::new(
                    body.into_runtime_widget(errors.clone()),
                    bottom.into_runtime_widget(errors.clone()),
                )
                .fallback_size(*fallback_size),
            ),
            BindingWidgetKind::PlacementBadge {
                label,
                icon,
                tone,
                current,
                target,
                min_width,
            } => {
                let mut badge = if matches!(label, BindingText::State(_)) {
                    PlacementBadge::dynamic(label.resolve(), {
                        let label = label.clone();
                        move || label.resolve()
                    })
                } else {
                    PlacementBadge::new(label.resolve())
                }
                .tone(*tone);
                if let Some(icon) = icon {
                    badge = badge.icon(*icon);
                }
                if let (Some(current), Some(target)) = (current, target) {
                    badge = badge.coverage(*current, *target);
                }
                if let Some(min_width) = min_width {
                    badge = badge.min_width(*min_width);
                }
                BindingRuntimeWidget::new(themed_widget!(badge, errors))
            }
            BindingWidgetKind::PropertyRow {
                label,
                control,
                stacked,
                label_width,
                control_width,
                gap,
            } => {
                let mut row =
                    PropertyRow::new(label.clone(), control.into_runtime_widget(errors.clone()));
                if !stacked {
                    row = row.inline();
                }
                if let Some(label_width) = label_width {
                    row = row.label_width(*label_width);
                }
                if let Some(control_width) = control_width {
                    row = row.control_width(*control_width);
                }
                if let Some(gap) = gap {
                    row = row.gap(*gap);
                }
                BindingRuntimeWidget::new(themed_widget!(row, errors))
            }
            BindingWidgetKind::SectionLabel {
                label,
                semantic_name,
                color,
            } => {
                let mut section = SectionLabel::new(label.clone());
                if let Some(semantic_name) = semantic_name {
                    section = section.semantic_name(semantic_name.clone());
                }
                if let Some(color) = color {
                    section = section.color(*color);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::SideSheet {
                title,
                body,
                description,
                shown,
                modal,
                dismiss_on_scrim,
                placement,
                width,
                header_action,
                actions,
                on_dismiss,
            } => {
                let title = title.clone();
                let body = body.clone();
                let description = description.clone();
                let shown_state = shown.state();
                let placement = *placement;
                let width = *width;
                let header_action = header_action.clone();
                let actions = actions.clone();
                let on_dismiss = on_dismiss.clone();
                let modal = *modal;
                let dismiss_on_scrim = *dismiss_on_scrim;
                let build_errors = errors.clone();
                let build = move |is_shown: bool| {
                    let mut sheet = SideSheet::new(
                        title.clone(),
                        body.into_runtime_widget(build_errors.clone()),
                    )
                    .shown(is_shown)
                    .modal(modal)
                    .dismiss_on_scrim(dismiss_on_scrim)
                    .placement(placement);
                    if let Some(description) = &description {
                        sheet = sheet.description(description.clone());
                    }
                    if let Some(width) = width {
                        sheet = if placement == SideSheetPlacement::Bottom {
                            sheet.height(width)
                        } else {
                            sheet.width(width)
                        };
                    }
                    if let Some(header_action) = &header_action {
                        sheet = sheet
                            .header_action(header_action.into_runtime_widget(build_errors.clone()));
                    }
                    for action in &actions {
                        sheet = sheet.action(action.into_runtime_widget(build_errors.clone()));
                    }
                    if shown_state.is_some() || on_dismiss.is_some() {
                        let shown_state = shown_state.clone();
                        let on_dismiss = on_dismiss.clone();
                        let callback_errors = build_errors.clone();
                        sheet = sheet.on_dismiss(move || {
                            if let Some(state) = &shown_state {
                                state.set(false);
                            }
                            if let Some(action) = &on_dismiss
                                && let Err(error) = action.run()
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    themed_widget!(sheet, build_errors)
                };
                if matches!(shown, BindingBool::State(_)) {
                    BindingRuntimeWidget::new(BindingSideSheetWidget::new(shown.clone(), build))
                } else {
                    BindingRuntimeWidget::new(build(shown.resolve()))
                }
            }
            BindingWidgetKind::SplitView {
                name,
                axis,
                first,
                second,
                ratio,
                min_first,
                min_second,
                divider_thickness,
                on_change,
            } => {
                let name = name.clone();
                let axis = *axis;
                let first = first.clone();
                let second = second.clone();
                let min_first = *min_first;
                let min_second = *min_second;
                let divider_thickness = *divider_thickness;
                let ratio_state = ratio.state();
                let on_change = on_change.clone();
                let build_errors = errors.clone();
                let build = move |resolved_ratio: f32| {
                    let mut split = SplitView::new(
                        axis,
                        first.into_runtime_widget(build_errors.clone()),
                        second.into_runtime_widget(build_errors.clone()),
                    )
                    .ratio(resolved_ratio)
                    .min_first(min_first)
                    .min_second(min_second);
                    if let Some(name) = &name {
                        split = split.name(name.clone());
                    }
                    if let Some(divider_thickness) = divider_thickness {
                        split = split.divider_thickness(divider_thickness);
                    }
                    if ratio_state.is_some() || on_change.is_some() {
                        let ratio_state = ratio_state.clone();
                        let on_change = on_change.clone();
                        let callback_errors = build_errors.clone();
                        split = split.on_change(move |value| {
                            if let Some(state) = &ratio_state {
                                state.set(f64::from(value));
                            }
                            if let Some(action) = &on_change
                                && let Err(error) = action.run(f64::from(value))
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    themed_widget!(split, build_errors)
                };
                if matches!(ratio, BindingNumber::State(_)) {
                    BindingRuntimeWidget::new(BindingSplitViewWidget::new(ratio.clone(), build))
                } else {
                    BindingRuntimeWidget::new(build(ratio.resolve() as f32))
                }
            }
            BindingWidgetKind::SwitchView { children, selected } => {
                let mut view = SwitchView::new()
                    .selected(binding_number_to_index(selected.resolve()).unwrap_or(usize::MAX));
                if matches!(selected, BindingNumber::State(_)) {
                    let selected = selected.clone();
                    view = view.selected_when(move || {
                        binding_number_to_index(selected.resolve()).unwrap_or(usize::MAX)
                    });
                }
                for child in children {
                    view = view.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::TrailingSlotRow {
                body,
                trailing,
                trailing_width,
                trailing_height,
                gap,
            } => BindingRuntimeWidget::new(
                TrailingSlotRow::new(
                    body.into_runtime_widget(errors.clone()),
                    trailing.into_runtime_widget(errors.clone()),
                )
                .trailing_width(*trailing_width)
                .trailing_height(*trailing_height)
                .gap(*gap),
            ),
            BindingWidgetKind::VirtualScrollView {
                children,
                name,
                padding,
                spacing,
            } => {
                let mut view = VirtualScrollView::new();
                if let Some(name) = name {
                    view = view.name(name.clone());
                }
                if let Some(padding) = padding {
                    view = view.padding(*padding);
                }
                if let Some(spacing) = spacing {
                    view = view.spacing(*spacing);
                }
                for child in children {
                    view = view.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::FloatingStack { windows, name } => {
                let mut stack = FloatingStack::new();
                if let Some(name) = name {
                    stack = stack.name(name.clone());
                }
                for window in windows {
                    stack = stack.with_window(
                        window.bounds,
                        window.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(stack)
            }
            BindingWidgetKind::ReorderableList {
                name,
                children,
                spacing,
                drag_threshold,
                preview_label,
                on_reorder,
            } => {
                let mut list = ReorderableList::new(name.clone())
                    .spacing(*spacing)
                    .drag_threshold(*drag_threshold);
                if let Some(preview_label) = preview_label {
                    list = list.preview_label(preview_label.clone());
                }
                for child in children {
                    list = list.item(child.into_runtime_widget(errors.clone()));
                }
                if let Some(action) = on_reorder.clone() {
                    let errors = errors.clone();
                    list = list.on_reorder(move |change| {
                        if let Err(error) = action.run(change.item, change.from, change.to) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(list)
            }
            BindingWidgetKind::Surface {
                child,
                role,
                name,
                border,
                elevation,
                radius,
                padding,
                fill_width,
                fill_height,
            } => {
                let child = child.into_runtime_widget(errors.clone());
                let mut surface = match role {
                    SurfaceRole::Window => Surface::window(child),
                    SurfaceRole::Sidebar => Surface::sidebar(child),
                    SurfaceRole::Panel => Surface::panel(child),
                    SurfaceRole::Titlebar => Surface::titlebar(child),
                    SurfaceRole::Field => Surface::field(child),
                };
                if let Some(name) = name {
                    surface = surface.name(name.clone());
                }
                if let Some(border) = border {
                    surface = surface.border(*border);
                }
                if let Some(elevation) = elevation {
                    surface = surface.elevation(*elevation);
                }
                if let Some(radius) = radius {
                    surface = surface.radius(*radius);
                }
                if let Some(padding) = padding {
                    surface = surface.padding(Insets::all(padding.max(0.0)));
                }
                if *fill_width && *fill_height {
                    surface = surface.fill();
                } else if *fill_width {
                    surface = surface.fill_width();
                } else if *fill_height {
                    surface = surface.fill_height();
                }
                BindingRuntimeWidget::new(themed_widget!(surface, errors))
            }
            BindingWidgetKind::ExternalSurface {
                descriptor,
                desired_size,
                name,
                ..
            } => BindingRuntimeWidget::new(BindingExternalSurfaceWidget {
                descriptor: descriptor.clone(),
                desired_size: *desired_size,
                name: name.clone(),
            }),
            BindingWidgetKind::Toolbar {
                children,
                axis,
                name,
                extent,
                padding,
                spacing,
                background,
                divider,
            } => {
                let mut toolbar = Toolbar::new(*axis).divider(*divider);
                if let Some(name) = name {
                    toolbar = toolbar.name(name.clone());
                }
                if let Some(extent) = extent {
                    toolbar = toolbar.extent(*extent);
                }
                if let Some(padding) = padding {
                    toolbar = toolbar.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(spacing) = spacing {
                    toolbar = toolbar.spacing(*spacing);
                }
                if let Some(background) = background {
                    toolbar = toolbar.background(*background);
                }
                for child in children {
                    toolbar = toolbar.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(toolbar, errors))
            }
            BindingWidgetKind::Grid {
                columns,
                children,
                name,
                column_gap,
                row_gap,
            } => {
                let mut grid = Grid::new(std::iter::repeat_n(GridTrack::fraction(1.0), *columns))
                    .column_gap(*column_gap)
                    .row_gap(*row_gap);
                if let Some(name) = name {
                    grid = grid.name(name.clone());
                }
                for child in children {
                    grid = grid.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(grid)
            }
            BindingWidgetKind::AspectRatio {
                child,
                ratio,
                fit,
                horizontal,
                vertical,
            } => BindingRuntimeWidget::new(
                AspectRatio::new(*ratio, child.into_runtime_widget(errors.clone()))
                    .fit(*fit)
                    .align(*horizontal, *vertical),
            ),
            BindingWidgetKind::SafeArea {
                child,
                edges,
                minimum,
            } => BindingRuntimeWidget::new(
                SafeArea::new(child.into_runtime_widget(errors.clone()))
                    .edges(*edges)
                    .minimum(*minimum),
            ),
            BindingWidgetKind::LayoutTransition {
                child,
                duration,
                easing,
            } => BindingRuntimeWidget::new(
                LayoutTransition::new(child.into_runtime_widget(errors.clone()))
                    .duration(*duration)
                    .easing(*easing),
            ),
            BindingWidgetKind::AdaptiveView {
                compact,
                medium,
                expanded,
                medium_breakpoint,
                expanded_breakpoint,
                on_class_change,
            } => {
                let mut view = AdaptiveView::new(
                    compact.into_runtime_widget(errors.clone()),
                    medium.into_runtime_widget(errors.clone()),
                    expanded.into_runtime_widget(errors.clone()),
                )
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ));
                if let Some(action) = on_class_change.clone() {
                    let callback_errors = errors.clone();
                    view = view.on_class_change(move |class| {
                        let value = match class {
                            AdaptiveClass::Compact => "compact",
                            AdaptiveClass::Medium => "medium",
                            AdaptiveClass::Expanded => "expanded",
                        };
                        if let Err(error) = action.run(value.to_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::ConstraintView { cases, fallback } => {
                let mut view = ConstraintView::new(fallback.into_runtime_widget(errors.clone()));
                for case in cases {
                    view = view.when(case.query, case.child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::ResponsiveSidebar {
                state,
                sidebar,
                content,
                name,
                medium_breakpoint,
                expanded_breakpoint,
                rail_width,
                overlay_width,
                dismiss_on_scrim,
                on_mode_change,
            } => {
                let mut view = ResponsiveSidebar::new(
                    sidebar.into_runtime_widget(errors.clone()),
                    content.into_runtime_widget(errors.clone()),
                )
                .state(state.inner.clone())
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ))
                .rail_width(*rail_width)
                .overlay_width(*overlay_width)
                .dismiss_on_scrim(*dismiss_on_scrim);
                if let Some(name) = name {
                    view = view.name(name.clone());
                }
                if let Some(theme) = errors.theme.clone() {
                    view = view.theme(theme.snapshot());
                }
                if let Some(action) = on_mode_change.clone() {
                    let callback_errors = errors.clone();
                    view = view.on_mode_change(move |mode| {
                        let value = match mode {
                            ResponsiveSidebarMode::OverlayClosed => "overlay-closed",
                            ResponsiveSidebarMode::OverlayOpen => "overlay-open",
                            ResponsiveSidebarMode::Rail => "rail",
                            ResponsiveSidebarMode::Inline => "inline",
                        };
                        if let Err(error) = action.run(value.to_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(view)
            }
            BindingWidgetKind::MasterDetail {
                state,
                master,
                detail,
                medium_breakpoint,
                expanded_breakpoint,
                master_width,
            } => BindingRuntimeWidget::new(
                MasterDetail::new(
                    master.into_runtime_widget(errors.clone()),
                    detail.into_runtime_widget(errors.clone()),
                )
                .state(state.inner.clone())
                .breakpoints(AdaptiveBreakpoints::new(
                    *medium_breakpoint,
                    *expanded_breakpoint,
                ))
                .split_state(SplitState::pixels(*master_width)),
            ),
            BindingWidgetKind::OverlayHost { child } => BindingRuntimeWidget::new(
                OverlayHost::new(child.into_runtime_widget(errors.clone())),
            ),
            BindingWidgetKind::NotificationHost { center, width } => {
                let mut host = NotificationHost::new(center.inner.clone()).width(*width);
                if let Some(theme) = errors.theme.clone() {
                    host = host.theme(theme.snapshot());
                }
                BindingRuntimeWidget::new(host)
            }
            BindingWidgetKind::CommandPalette {
                name,
                content,
                description,
                shown,
                max_width,
                on_dismiss,
            } => {
                let name = name.clone();
                let content = content.clone();
                let description = description.clone();
                let max_width = *max_width;
                let shown_state = shown.state();
                let on_dismiss = on_dismiss.clone();
                let build_errors = errors.clone();
                let build = move |is_shown: bool| {
                    let mut palette = CommandPalette::new(
                        name.clone(),
                        content.into_runtime_widget(build_errors.clone()),
                    )
                    .shown(is_shown);
                    if let Some(description) = &description {
                        palette = palette.description(description.clone());
                    }
                    if let Some(max_width) = max_width {
                        palette = palette.max_width(max_width);
                    }
                    if let Some(theme) = build_errors.theme.clone() {
                        palette = palette.theme(theme.snapshot());
                    }
                    if shown_state.is_some() || on_dismiss.is_some() {
                        let shown_state = shown_state.clone();
                        let action = on_dismiss.clone();
                        let callback_errors = build_errors.clone();
                        palette = palette.on_dismiss(move || {
                            if let Some(state) = &shown_state {
                                state.set(false);
                            }
                            if let Some(action) = &action
                                && let Err(error) = action.run()
                            {
                                callback_errors.push(ForeignCallbackError::new(
                                    ForeignWidgetId::new(0),
                                    ForeignCallbackPhase::Event,
                                    error.message,
                                ));
                            }
                        });
                    }
                    palette
                };
                if matches!(shown, BindingBool::State(_)) {
                    BindingRuntimeWidget::new(BindingCommandPaletteWidget::new(
                        shown.clone(),
                        build,
                    ))
                } else {
                    BindingRuntimeWidget::new(build(shown.resolve()))
                }
            }
            BindingWidgetKind::VirtualList {
                name,
                model,
                estimated_row_height,
                spacing,
                padding,
                row_padding,
                overscan_viewports,
                cache_capacity,
                selectable,
                transparent,
                stick_to_end,
                overlay_scroll_bars,
                on_change,
                on_near_start,
                on_near_end,
            } => {
                let mut list =
                    VirtualList::new(name.clone(), model.inner.clone(), |_key, value| {
                        Label::new("").text_from(value)
                    })
                    .estimated_row_height(*estimated_row_height)
                    .spacing(*spacing)
                    .overscan_viewports(*overscan_viewports)
                    .cache_capacity(*cache_capacity)
                    .selection_mode(if *selectable {
                        VirtualListSelectionMode::Single
                    } else {
                        VirtualListSelectionMode::None
                    })
                    .chrome(if *transparent {
                        VirtualListChrome::Transparent
                    } else {
                        VirtualListChrome::Default
                    })
                    .stick_to_end(*stick_to_end)
                    .overlay_scroll_bars(*overlay_scroll_bars)
                    .row_name(|_, value| value.clone());
                if let Some(padding) = padding {
                    list = list.padding(*padding);
                }
                if let Some(row_padding) = row_padding {
                    list = list.row_padding(*row_padding);
                }
                if let Some(action) = on_change.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_change(move |key| {
                        if let Err(error) = action.run(key) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_near_start.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_near_start(move || {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_near_end.clone() {
                    let callback_errors = errors.clone();
                    list = list.on_near_end(move || {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(list, errors))
            }
            BindingWidgetKind::Canvas {
                name,
                viewport,
                shapes,
                draw_stroke,
                desired_size,
            } => {
                let canvas = Canvas::new(name.clone())
                    .viewport(viewport.into_sui())
                    .shapes(shapes.iter().map(|shape| shape.inner.clone()))
                    .draw_stroke(draw_stroke.into_sui())
                    .desired_size(*desired_size);
                BindingRuntimeWidget::new(themed_widget!(canvas, errors))
            }
            BindingWidgetKind::CanvasRuler {
                axis,
                name,
                document_size,
                viewport,
                viewport_size,
                extent,
            } => {
                let mut ruler = CanvasRuler::new(*axis, name.clone(), *document_size)
                    .viewport(viewport.into_sui(), *viewport_size);
                if let Some(extent) = extent {
                    ruler = ruler.extent(*extent);
                }
                BindingRuntimeWidget::new(themed_widget!(ruler, errors))
            }
            BindingWidgetKind::DragDropHost {
                scope,
                child,
                on_external_hover,
                on_external_drop,
                on_external_cancel,
            } => {
                let mut host = DragDropHost::new(
                    scope.inner.clone(),
                    child.into_runtime_widget(errors.clone()),
                );
                if let Some(theme) = errors.theme.clone() {
                    host = host.theme_when(move || theme.snapshot());
                }
                if let Some(action) = on_external_hover.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_hover(move |_ctx, paths| {
                        let paths = paths
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect();
                        if let Err(error) = action.run(paths) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_external_drop.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_drop(move |_ctx, path| {
                        if let Err(error) = action.run(path.to_string_lossy().into_owned()) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_external_cancel.clone() {
                    let callback_errors = errors.clone();
                    host = host.on_external_file_hover_cancelled(move |_ctx| {
                        if let Err(error) = action.run() {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(host)
            }
            BindingWidgetKind::Draggable {
                scope,
                child,
                payload,
                effect,
                preview_label,
                threshold,
                on_start,
                on_end,
            } => {
                let payload_value = payload.clone();
                let mut draggable = Draggable::new(child.into_runtime_widget(errors.clone()))
                    .scope(scope.inner.clone())
                    .payload(move || DragPayload::text(payload_value.clone()))
                    .effect(*effect)
                    .threshold(*threshold);
                if let Some(preview_label) = preview_label {
                    draggable = draggable.preview_label(preview_label.clone());
                }
                if let Some(action) = on_start.clone() {
                    let callback_errors = errors.clone();
                    draggable = draggable.on_drag_start(move |_ctx, preview| {
                        let value = match &preview.payload {
                            DragPayload::Text(text) => text.clone(),
                            DragPayload::Image { handle, .. } => format!("image:{}", handle.get()),
                            DragPayload::Custom { kind, .. } => kind.to_string(),
                        };
                        if let Err(error) = action.run(value) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_end.clone() {
                    let callback_errors = errors.clone();
                    draggable = draggable.on_drag_end(move |_ctx, event| {
                        if let Err(error) = action.run(binding_drag_payload_text(event)) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(draggable)
            }
            BindingWidgetKind::DropTarget {
                scope,
                child,
                effect,
                on_drop,
                on_hover_change,
            } => {
                let effect = *effect;
                let mut target = DropTarget::new(child.into_runtime_widget(errors.clone()))
                    .scope(scope.inner.clone())
                    .accept(move |_| effect);
                if let Some(action) = on_drop.clone() {
                    let callback_errors = errors.clone();
                    target = target.on_drop(move |_ctx, event| {
                        if let Err(error) = action.run(binding_drag_payload_text(event)) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_hover_change.clone() {
                    let callback_errors = errors.clone();
                    target = target.on_hover_change(move |hovered| {
                        if let Err(error) = action.run(hovered) {
                            callback_errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(target)
            }
            BindingWidgetKind::FloatingWorkspace { state, views, name } => {
                let mut workspace = FloatingWorkspace::new(state.inner.clone());
                if let Some(name) = name {
                    workspace = workspace.name(name.clone());
                }
                if let Some(theme) = errors.theme.clone() {
                    workspace = workspace.theme_when(move || theme.snapshot());
                }
                for view in views {
                    workspace = workspace.with_registered_view(
                        view.id.expect("binding floating view id assigned"),
                        view.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(workspace)
            }
            BindingWidgetKind::PixelCanvas {
                state,
                name,
                width,
                height,
                paper_color,
                desired_size,
                viewport,
                fit_on_first_layout,
                pixels,
            } => {
                let mut canvas = PixelCanvas::new(name.clone(), *width, *height)
                    .state(state.inner.clone())
                    .desired_size(*desired_size)
                    .viewport(viewport.into_sui());
                if let Some(paper_color) = paper_color {
                    canvas = canvas.paper_color(*paper_color);
                }
                if *fit_on_first_layout {
                    canvas = canvas.fit_on_first_layout();
                }
                if !pixels.is_empty() {
                    canvas = canvas.with_pixels(pixels.clone());
                }
                BindingRuntimeWidget::new(themed_widget!(canvas, errors))
            }
            BindingWidgetKind::Padding {
                child,
                insets,
                fill_child_width,
                fill_child_height,
            } => {
                let mut padding =
                    PaddingWidget::new(*insets, child.into_runtime_widget(errors.clone()));
                if *fill_child_width && *fill_child_height {
                    padding = padding.fill_child();
                } else if *fill_child_width {
                    padding = padding.fill_child_width();
                } else if *fill_child_height {
                    padding = padding.fill_child_height();
                }
                BindingRuntimeWidget::new(padding)
            }
            BindingWidgetKind::Align {
                child,
                horizontal,
                vertical,
            } => BindingRuntimeWidget::new(Align::new(
                *horizontal,
                *vertical,
                child.into_runtime_widget(errors.clone()),
            )),
            BindingWidgetKind::Background { child, color } => BindingRuntimeWidget::new(
                Background::new(*color, child.into_runtime_widget(errors.clone())),
            ),
            BindingWidgetKind::SizedBox {
                child,
                width,
                height,
            } => {
                let mut sized_box = SizedBox::new();
                if let Some(width) = width {
                    sized_box = sized_box.width(*width);
                }
                if let Some(height) = height {
                    sized_box = sized_box.height(*height);
                }
                if let Some(child) = child {
                    sized_box = sized_box.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(sized_box)
            }
            BindingWidgetKind::Stack {
                children,
                axis,
                spacing,
                alignment,
            } => {
                let mut stack = Stack::new(*axis).spacing(*spacing).alignment(*alignment);
                for child in children {
                    stack = stack.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(stack)
            }
            BindingWidgetKind::SemanticRegion {
                name,
                child,
                description,
                role,
            } => {
                let mut region =
                    SemanticRegion::new(name.resolve(), child.into_runtime_widget(errors.clone()))
                        .role(role.clone());
                if matches!(name, BindingText::State(_)) {
                    let name = name.clone();
                    region = region.name_when(move || name.resolve());
                }
                if let Some(description) = description {
                    region = region.description(description.resolve());
                    if matches!(description, BindingText::State(_)) {
                        let description = description.clone();
                        region = region.description_when(move || description.resolve());
                    }
                }
                BindingRuntimeWidget::new(region)
            }
            BindingWidgetKind::FormRow {
                label,
                control,
                stacked,
                label_width,
                control_width,
                gap,
            } => {
                let mut row =
                    FormRow::new(label.clone(), control.into_runtime_widget(errors.clone()));
                if *stacked {
                    row = row.stacked();
                }
                if let Some(label_width) = label_width {
                    row = row.label_width(*label_width);
                }
                if let Some(control_width) = control_width {
                    row = row.control_width(*control_width);
                }
                if let Some(gap) = gap {
                    row = row.gap(*gap);
                }
                BindingRuntimeWidget::new(themed_widget!(row, errors))
            }
            BindingWidgetKind::FieldGroup {
                children,
                spacing,
                padding,
                max_width,
                fill_width,
            } => {
                let mut group = FieldGroup::new();
                if let Some(spacing) = spacing {
                    group = group.spacing(*spacing);
                }
                if let Some(padding) = padding {
                    group = group.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(max_width) = max_width {
                    group = group.max_width(*max_width);
                }
                if *fill_width {
                    group = group.fill_width();
                }
                for child in children {
                    group = group.with_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(themed_widget!(group, errors))
            }
            BindingWidgetKind::FormSection {
                title,
                child,
                description,
                header_action,
                padding,
                body_gap,
                header_gap,
                max_width,
                fill_width,
                radius,
                elevation,
            } => {
                let mut section =
                    FormSection::new(title.clone(), child.into_runtime_widget(errors.clone()));
                if let Some(description) = description {
                    section = section.description(description.clone());
                }
                if let Some(header_action) = header_action {
                    section =
                        section.header_action(header_action.into_runtime_widget(errors.clone()));
                }
                if let Some(padding) = padding {
                    section = section.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(body_gap) = body_gap {
                    section = section.body_gap(*body_gap);
                }
                if let Some(header_gap) = header_gap {
                    section = section.header_gap(*header_gap);
                }
                if let Some(max_width) = max_width {
                    section = section.max_width(*max_width);
                }
                if *fill_width {
                    section = section.fill_width();
                }
                if let Some(radius) = radius {
                    section = section.radius(*radius);
                }
                if let Some(elevation) = elevation {
                    section = section.elevation(*elevation);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::PanelSection {
                title,
                child,
                header_action,
                gap,
                action_gap,
                collapsible,
                expanded,
            } => {
                let mut section =
                    PanelSection::new(title.clone(), child.into_runtime_widget(errors.clone()))
                        .collapsible(*collapsible)
                        .expanded(*expanded);
                if let Some(header_action) = header_action {
                    section =
                        section.header_action(header_action.into_runtime_widget(errors.clone()));
                }
                if let Some(gap) = gap {
                    section = section.gap(*gap);
                }
                if let Some(action_gap) = action_gap {
                    section = section.action_gap(*action_gap);
                }
                BindingRuntimeWidget::new(themed_widget!(section, errors))
            }
            BindingWidgetKind::DockPanel {
                title,
                child,
                name,
                header_height,
                padding,
                background,
                header_background,
            } => {
                let mut panel =
                    DockPanel::new(title.clone(), child.into_runtime_widget(errors.clone()));
                if let Some(name) = name {
                    panel = panel.name(name.clone());
                }
                if let Some(header_height) = header_height {
                    panel = panel.header_height(*header_height);
                }
                if let Some(padding) = padding {
                    panel = panel.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(background) = background {
                    panel = panel.background(*background);
                }
                if let Some(header_background) = header_background {
                    panel = panel.header_background(*header_background);
                }
                BindingRuntimeWidget::new(themed_widget!(panel, errors))
            }
            BindingWidgetKind::DockWorkspace {
                state,
                panels,
                name,
            } => {
                let mut workspace = DockWorkspace::new(state.inner.clone()).name(name.clone());
                for panel in panels {
                    workspace = workspace.with_panel(
                        DockPanelId::new(panel.id),
                        panel.title.clone(),
                        panel.child.into_runtime_widget(errors.clone()),
                    );
                }
                BindingRuntimeWidget::new(themed_widget!(workspace, errors))
            }
            BindingWidgetKind::StatusBarHost {
                content,
                status_bar,
            } => BindingRuntimeWidget::new(StatusBarHost::new(
                content.into_runtime_widget(errors.clone()),
                status_bar.into_runtime_widget(errors.clone()),
            )),
            BindingWidgetKind::Tooltip {
                text,
                child,
                placement,
            } => BindingRuntimeWidget::new(
                Tooltip::new(text.clone(), child.into_runtime_widget(errors.clone()))
                    .placement(*placement),
            ),
            BindingWidgetKind::Popover {
                name,
                trigger,
                content,
                open,
            } => BindingRuntimeWidget::new(
                Popover::new(
                    name.clone(),
                    trigger.into_runtime_widget(errors.clone()),
                    content.into_runtime_widget(errors.clone()),
                )
                .open(*open),
            ),
            BindingWidgetKind::ToolPalette {
                name,
                items,
                selected,
                axis,
                action,
                extent,
                padding,
                spacing,
                item_size,
                icon_size,
                background,
                divider,
            } => {
                let mut palette = ToolPalette::new(*axis, name.clone())
                    .items(items.iter().map(BindingToolPaletteItem::into_sui))
                    .divider(*divider);
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        palette = palette.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        palette = palette
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(extent) = extent {
                    palette = palette.extent(*extent);
                }
                if let Some(padding) = padding {
                    palette = palette.padding(Insets::all(padding.max(0.0)));
                }
                if let Some(spacing) = spacing {
                    palette = palette.spacing(*spacing);
                }
                if let Some(item_size) = item_size {
                    palette = palette.item_size(*item_size);
                }
                if let Some(icon_size) = icon_size {
                    palette = palette.icon_size(*icon_size);
                }
                if let Some(background) = background {
                    palette = palette.background(*background);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    palette = palette.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(palette, errors))
            }
            BindingWidgetKind::PresetStrip {
                name,
                presets,
                selected,
                action,
                item_width,
                item_height,
                gap,
            } => {
                let mut strip = PresetStrip::new(name.clone()).presets(presets.clone());
                if let Some(selected) = selected {
                    if let Some(index) = binding_number_to_index(selected.resolve()) {
                        strip = strip.selected(index);
                    }
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        strip = strip
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                if let Some(item_width) = item_width {
                    strip = strip.item_width(*item_width);
                }
                if let Some(item_height) = item_height {
                    strip = strip.item_height(*item_height);
                }
                if let Some(gap) = gap {
                    strip = strip.gap(*gap);
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || action.is_some() {
                    let action = action.clone();
                    let errors = errors.clone();
                    strip = strip.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(strip, errors))
            }
            BindingWidgetKind::BrowserTabBar {
                name,
                tabs,
                selected,
                on_change,
                on_close,
            } => {
                let mut tab_bar = BrowserTabBar::new(name.clone()).tabs(tabs.clone());
                if let Some(selected) = selected {
                    let index = binding_number_to_index(selected.resolve());
                    tab_bar = tab_bar.selected(index);
                    if matches!(selected, BindingNumber::State(_)) {
                        let selected = selected.clone();
                        tab_bar = tab_bar
                            .selected_when(move || binding_number_to_index(selected.resolve()));
                    }
                }
                let state = selected.as_ref().and_then(BindingNumber::state);
                if state.is_some() || on_change.is_some() {
                    let action = on_change.clone();
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_change(move |index, value| {
                        if let Some(state) = &state {
                            state.set(index as f64);
                        }
                        if let Some(action) = &action
                            && let Err(error) = action.run(index, value)
                        {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                if let Some(action) = on_close.clone() {
                    let errors = errors.clone();
                    tab_bar = tab_bar.on_close(move |index, value| {
                        if let Err(error) = action.run(index, value) {
                            errors.push(ForeignCallbackError::new(
                                ForeignWidgetId::new(0),
                                ForeignCallbackPhase::Event,
                                error.message,
                            ));
                        }
                    });
                }
                BindingRuntimeWidget::new(themed_widget!(tab_bar, errors))
            }
            BindingWidgetKind::ScrollView { child, axes, name } => {
                let child = child.into_runtime_widget(errors.clone());
                let mut scroll_view = match axes {
                    BindingScrollAxes::Vertical => ScrollView::vertical(child),
                    BindingScrollAxes::Horizontal => ScrollView::horizontal(child),
                    BindingScrollAxes::Both => ScrollView::both(child),
                };
                if let Some(name) = name {
                    scroll_view = scroll_view.name(name.clone());
                }
                BindingRuntimeWidget::new(themed_widget!(scroll_view, errors))
            }
            BindingWidgetKind::Flex {
                axis,
                gap,
                children,
            } => {
                let mut flex = Flex::new(*axis).gap(*gap);
                for child in children {
                    flex.push(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(flex)
            }
            BindingWidgetKind::Foreign {
                callbacks,
                children,
            } => {
                let mut widget = ForeignWidget::from_arc(Arc::clone(callbacks))
                    .with_error_sink(errors.errors.clone());
                for child in children {
                    widget.push_child(child.into_runtime_widget(errors.clone()));
                }
                BindingRuntimeWidget::new(widget)
            }
        }
    }
}

struct BindingSideSheetWidget {
    inner: SideSheet,
    shown: BindingBool,
    last_shown: bool,
}

impl BindingSideSheetWidget {
    fn new(
        shown: BindingBool,
        build: impl Fn(bool) -> SideSheet + 'static,
    ) -> BindingSideSheetWidget {
        let last_shown = shown.resolve();
        let inner = build(last_shown);
        Self {
            inner,
            shown,
            last_shown,
        }
    }

    fn sync_state(&mut self) {
        let shown = self.shown.resolve();
        if shown != self.last_shown {
            self.inner.set_shown(shown);
            self.last_shown = shown;
        }
    }
}

impl Widget for BindingSideSheetWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingSideSheetWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_state();
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.sync_state();
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.sync_state();
        self.inner.visit_children_mut(visitor);
    }
}

struct BindingCommandPaletteWidget {
    inner: CommandPalette,
    shown: BindingBool,
    last_shown: bool,
}

impl BindingCommandPaletteWidget {
    fn new(shown: BindingBool, build: impl Fn(bool) -> CommandPalette + 'static) -> Self {
        let last_shown = shown.resolve();
        Self {
            inner: build(last_shown),
            shown,
            last_shown,
        }
    }

    fn sync_state(&mut self) {
        let shown = self.shown.resolve();
        if shown != self.last_shown {
            self.inner.set_shown(shown);
            self.last_shown = shown;
        }
    }
}

impl Widget for BindingCommandPaletteWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingCommandPaletteWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_state();
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.sync_state();
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.sync_state();
        self.inner.visit_children_mut(visitor);
    }
}

struct BindingSplitViewWidget {
    inner: SplitView,
    ratio: BindingNumber,
}

impl BindingSplitViewWidget {
    fn new(
        ratio: BindingNumber,
        build: impl Fn(f32) -> SplitView + 'static,
    ) -> BindingSplitViewWidget {
        let inner = build(ratio.resolve() as f32);
        Self { inner, ratio }
    }

    fn sync_state(&mut self) {
        let ratio = (self.ratio.resolve() as f32).clamp(0.0, 1.0);
        if (ratio - self.inner.current_ratio()).abs() > f32::EPSILON {
            self.inner.set_ratio(ratio);
        }
    }
}

impl Widget for BindingSplitViewWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingSplitViewWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_state();
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.sync_state();
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.sync_state();
        self.inner.visit_children_mut(visitor);
    }
}

struct BindingBusyIndicatorWidget {
    name: BindingText,
    label: Option<BindingText>,
    size: f32,
    theme: Option<BindingTheme>,
}

impl BindingBusyIndicatorWidget {
    fn inner(&self) -> BusyIndicator {
        let mut indicator = BusyIndicator::new(self.name.resolve()).size(self.size);
        if let Some(label) = &self.label {
            indicator = indicator.label(label.resolve());
        }
        if let Some(theme) = &self.theme {
            let theme = theme.clone();
            indicator = indicator.theme_when(move || theme.snapshot());
        }
        indicator
    }
}

impl Widget for BindingBusyIndicatorWidget {
    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingBusyIndicatorWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner().measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner().arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner().paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner().semantics(ctx);
    }
}

struct BindingProgressBarWidget {
    name: BindingText,
    value: BindingNumber,
    min: f64,
    max: f64,
    show_value: bool,
    theme: Option<BindingTheme>,
}

impl BindingProgressBarWidget {
    fn inner(&self) -> ProgressBar {
        let mut progress = ProgressBar::new(self.name.resolve())
            .range(self.min, self.max)
            .value(self.value.resolve())
            .show_value(self.show_value);
        if let Some(theme) = &self.theme {
            let theme = theme.clone();
            progress = progress.theme_when(move || theme.snapshot());
        }
        progress
    }
}

impl Widget for BindingProgressBarWidget {
    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingProgressBarWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner().measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner().arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner().paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner().semantics(ctx);
    }
}

struct BindingCheckboxWidget {
    inner: Checkbox,
    checked: BindingBool,
}

impl BindingCheckboxWidget {
    fn sync_state(&mut self) {
        if matches!(self.checked, BindingBool::State(_)) {
            self.inner.set_checked(self.checked.resolve());
        }
    }
}

impl Widget for BindingCheckboxWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingCheckboxWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingSwitchWidget {
    inner: Switch,
    on: BindingBool,
}

impl BindingSwitchWidget {
    fn sync_state(&mut self) {
        if matches!(self.on, BindingBool::State(_)) {
            self.inner.set_on(self.on.resolve());
        }
    }
}

impl Widget for BindingSwitchWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingSwitchWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingRadioButtonWidget {
    inner: RadioButton,
    selected: BindingBool,
}

impl BindingRadioButtonWidget {
    fn sync_state(&mut self) {
        if matches!(self.selected, BindingBool::State(_)) {
            self.inner.set_selected(self.selected.resolve());
        }
    }
}

impl Widget for BindingRadioButtonWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingRadioButtonWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingTextInputWidget {
    inner: TextInput,
    value: BindingText,
}

impl BindingTextInputWidget {
    fn sync_state(&mut self) {
        if matches!(self.value, BindingText::State(_)) {
            self.inner.set_value(self.value.resolve());
        }
    }
}

impl Widget for BindingTextInputWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingTextInputWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingPasswordInputWidget {
    inner: PasswordInput,
    value: BindingText,
}

impl BindingPasswordInputWidget {
    fn sync_state(&mut self) {
        if matches!(self.value, BindingText::State(_)) {
            self.inner.set_value(self.value.resolve());
        }
    }
}

impl Widget for BindingPasswordInputWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingPasswordInputWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingDateTimeInputWidget {
    inner: DateTimeInput,
    value: BindingText,
}

impl BindingDateTimeInputWidget {
    fn sync_state(&mut self) {
        if matches!(self.value, BindingText::State(_)) {
            self.inner.set_value(self.value.resolve());
        }
    }
}

impl Widget for BindingDateTimeInputWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingDateTimeInputWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingTextAreaWidget {
    inner: TextArea,
    value: BindingText,
}

impl BindingTextAreaWidget {
    fn sync_state(&mut self) {
        if matches!(self.value, BindingText::State(_)) {
            self.inner.set_value(self.value.resolve());
        }
    }
}

impl Widget for BindingTextAreaWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_state();
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingTextAreaWidget"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_state();
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }
}

struct BindingExternalSurfaceWidget {
    descriptor: ExternalTextureDescriptor,
    desired_size: Size,
    name: Option<String>,
}

impl BindingExternalSurfaceWidget {
    fn draw_cpu_fallback(&self, ctx: &mut PaintCtx, size: Size, pixels: &Arc<[u8]>) {
        let width = size.width as u32;
        let height = size.height as u32;
        if let Ok(image) = RegisteredImage::from_rgba8(width, height, pixels.to_vec()) {
            let handle = ctx.widget_image_handle(0);
            ctx.register_image(handle, image);
            ctx.draw_image(ctx.bounds(), handle);
        }
    }
}

impl Widget for BindingExternalSurfaceWidget {
    fn debug_name(&self) -> &'static str {
        "sui_bindings_core::BindingExternalSurfaceWidget"
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.desired_size)
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}

    fn paint(&self, ctx: &mut PaintCtx) {
        if let ExternalTextureDescriptor::CpuRgba8 { size, pixels, .. } = &self.descriptor {
            self.draw_cpu_fallback(ctx, *size, pixels);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        if let Some(name) = &self.name {
            node.name = Some(name.clone());
        }
        ctx.push(node);
    }
}

struct BindingRuntimeWidget {
    inner: Box<dyn Widget>,
}

impl BindingRuntimeWidget {
    fn new(widget: impl Widget + 'static) -> Self {
        Self {
            inner: Box::new(widget),
        }
    }
}

impl Widget for BindingRuntimeWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        self.inner.debug_name()
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.inner.visit_children_mut(visitor);
    }
}

struct BindingUiTaskRootWidget {
    inner: BindingRuntimeWidget,
    ui_tasks: UiTaskQueue,
}

impl BindingUiTaskRootWidget {
    fn new(inner: BindingRuntimeWidget, ui_tasks: UiTaskQueue) -> Self {
        Self { inner, ui_tasks }
    }

    fn drain_ui_tasks(&self, ctx: &mut EventCtx) -> usize {
        let drained = self.ui_tasks.drain();
        if drained > 0 {
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        }
        drained
    }
}

impl Widget for BindingUiTaskRootWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
        self.drain_ui_tasks(ctx);
    }

    fn debug_name(&self) -> &'static str {
        self.inner.debug_name()
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.inner.visit_children_mut(visitor);
    }
}

#[cfg(feature = "desktop")]
const BINDING_UI_TASKS_READY: CommandKey<()> = CommandKey::new("sui.bindings.ui-tasks-ready");

#[derive(Debug, Clone)]
pub struct BindingWindow {
    title: String,
    root: BindingWidget,
    initial_size: Option<Size>,
    initial_position: Option<Point>,
    icon: BindingWindowIcon,
}

#[derive(Debug, Clone, Default)]
enum BindingWindowIcon {
    #[default]
    Default,
    None,
    Svg(Vec<u8>),
}

impl BindingWindow {
    pub fn new(title: impl Into<String>, root: BindingWidget) -> Self {
        Self {
            title: title.into(),
            root,
            initial_size: None,
            initial_position: None,
            icon: BindingWindowIcon::Default,
        }
    }

    pub fn with_initial_size(mut self, size: Size) -> Self {
        self.initial_size = Some(size);
        self
    }

    pub fn with_initial_position(mut self, position: Point) -> Self {
        self.initial_position = Some(position);
        self
    }

    pub fn with_icon_svg(mut self, svg: impl Into<Vec<u8>>) -> Self {
        self.icon = BindingWindowIcon::Svg(svg.into());
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = BindingWindowIcon::None;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn root(&self) -> &BindingWidget {
        &self.root
    }

    pub fn initial_size(&self) -> Option<Size> {
        self.initial_size
    }

    pub fn initial_position(&self) -> Option<Point> {
        self.initial_position
    }

    fn configure_builder(&self, mut builder: WindowBuilder) -> WindowBuilder {
        if let Some(size) = self.initial_size {
            builder = builder.initial_size(size);
        }
        if let Some(position) = self.initial_position {
            builder = builder.initial_position(position);
        }
        match &self.icon {
            BindingWindowIcon::Default => builder,
            BindingWindowIcon::None => builder.without_icon(),
            BindingWindowIcon::Svg(svg) => builder.icon_svg(svg.clone()),
        }
    }

    #[cfg(feature = "desktop")]
    fn configure_app_window(&self, mut window: SuiWindow) -> SuiWindow {
        if let Some(size) = self.initial_size {
            window = window.initial_size(size);
        }
        if let Some(position) = self.initial_position {
            window = window.initial_position(position);
        }
        match &self.icon {
            BindingWindowIcon::Default => window,
            BindingWindowIcon::None => window.without_icon(),
            BindingWindowIcon::Svg(svg) => window.icon_svg(svg.clone()),
        }
    }
}

/// Renderer-neutral window output policy shared by the Python and JavaScript APIs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingRenderOptions {
    inner: WindowRenderOptions,
}

impl BindingRenderOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        feathering_enabled: bool,
        feather_width: f32,
        optical_text_alignment: bool,
        output_color_primaries: &str,
        dynamic_range: &str,
        tone_mapping: &str,
        color_management: &str,
        sdr_content_brightness_nits: f32,
        use_system_sdr_brightness: bool,
    ) -> Result<Self, String> {
        let options = WindowRenderOptions::new(feathering_enabled, feather_width)
            .with_optical_vertical_text_alignment_enabled(optical_text_alignment)
            .with_output_color_primaries(parse_output_color_primaries(output_color_primaries)?)
            .with_dynamic_range_mode(parse_dynamic_range(dynamic_range)?)
            .with_tone_mapping_mode(parse_tone_mapping(tone_mapping)?)
            .with_color_management_mode(parse_color_management(color_management)?)
            .with_sdr_content_brightness_nits(sdr_content_brightness_nits)
            .with_system_sdr_content_brightness_enabled(use_system_sdr_brightness)
            .clamped();
        Ok(Self { inner: options })
    }

    pub const fn into_sui(self) -> WindowRenderOptions {
        self.inner
    }

    pub const fn feathering_enabled(self) -> bool {
        self.inner.feathering_enabled
    }

    pub const fn feather_width(self) -> f32 {
        self.inner.feather_width
    }
}

fn parse_output_color_primaries(value: &str) -> Result<WindowOutputColorPrimaries, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowOutputColorPrimaries::Automatic),
        "srgb" => Ok(WindowOutputColorPrimaries::Srgb),
        "displayp3" | "p3" => Ok(WindowOutputColorPrimaries::DisplayP3),
        _ => Err(format!(
            "output_color_primaries must be 'auto', 'srgb', or 'display-p3', got '{value}'"
        )),
    }
}

fn parse_dynamic_range(value: &str) -> Result<WindowDynamicRangeMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowDynamicRangeMode::Automatic),
        "sdr" | "standard" | "standarddynamicrange" => {
            Ok(WindowDynamicRangeMode::StandardDynamicRange)
        }
        "hdr" | "high" | "highdynamicrange" => Ok(WindowDynamicRangeMode::HighDynamicRange),
        _ => Err(format!(
            "dynamic_range must be 'auto', 'sdr', or 'hdr', got '{value}'"
        )),
    }
}

fn parse_tone_mapping(value: &str) -> Result<WindowToneMappingMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowToneMappingMode::Automatic),
        "clamp" => Ok(WindowToneMappingMode::Clamp),
        "reinhard" => Ok(WindowToneMappingMode::Reinhard),
        _ => Err(format!(
            "tone_mapping must be 'auto', 'clamp', or 'reinhard', got '{value}'"
        )),
    }
}

fn parse_color_management(value: &str) -> Result<WindowColorManagementMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowColorManagementMode::Automatic),
        "forcesdr" | "sdr" => Ok(WindowColorManagementMode::ForceSdr),
        "preferwidegamut" | "widegamut" => Ok(WindowColorManagementMode::PreferWideGamut),
        "preferhdr" | "hdr" => Ok(WindowColorManagementMode::PreferHdr),
        _ => Err(format!(
            "color_management must be 'auto', 'force-sdr', 'prefer-wide-gamut', or 'prefer-hdr', got '{value}'"
        )),
    }
}

fn normalized_option_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct BindingApp {
    windows: Vec<BindingWindow>,
    theme: Option<BindingTheme>,
    font_resources: Vec<BindingFontResource>,
    next_font_slot: u64,
    image_resources: Vec<BindingImageResource>,
    next_image_slot: u64,
    render_options: Option<BindingRenderOptions>,
    messages: BindingMessageBus,
    errors: ForeignErrorSink,
}

#[derive(Debug, Clone)]
struct BindingFontResource {
    handle: BindingFontHandle,
    font: RegisteredFont,
}

#[derive(Debug, Clone)]
struct BindingImageResource {
    handle: BindingImageHandle,
    image: RegisteredImage,
}

impl BindingApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window(mut self, window: BindingWindow) -> Self {
        self.push_window(window);
        self
    }

    pub fn push_window(&mut self, window: BindingWindow) {
        self.windows.push(window);
    }

    pub fn set_theme(&mut self, theme: BindingTheme) {
        self.theme = Some(theme);
    }

    pub fn theme(&self) -> Option<BindingTheme> {
        self.theme.clone()
    }

    pub fn set_render_options(&mut self, options: BindingRenderOptions) {
        self.render_options = Some(options);
    }

    pub fn render_options(&self) -> Option<BindingRenderOptions> {
        self.render_options
    }

    pub fn on_message(&mut self, name: impl Into<String>, action: BindingMessageAction) {
        self.messages.on(name, action);
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn register_font_bytes(
        &mut self,
        data: impl Into<Vec<u8>>,
    ) -> Result<BindingFontHandle, String> {
        let handle = BindingFontHandle::app_resource(self.next_font_slot);
        self.next_font_slot = self.next_font_slot.saturating_add(1);
        self.font_resources.push(BindingFontResource {
            handle,
            font: RegisteredFont::from_bytes(data),
        });
        Ok(handle)
    }

    pub fn font_resource_count(&self) -> usize {
        self.font_resources.len()
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<BindingImageHandle, String> {
        let image =
            RegisteredImage::from_rgba8(width, height, data).map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_png_image(
        &mut self,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = registered_image_from_png(data)?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_svg_image(
        &mut self,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = RegisteredImage::from_svg(data).map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = RegisteredImage::from_svg_at_size(width, height, data)
            .map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn image_resource_count(&self) -> usize {
        self.image_resources.len()
    }

    fn push_image_resource(&mut self, image: RegisteredImage) -> BindingImageHandle {
        let handle = BindingImageHandle::app_resource(self.next_image_slot);
        self.next_image_slot = self.next_image_slot.saturating_add(1);
        self.image_resources
            .push(BindingImageResource { handle, image });
        handle
    }

    fn register_image_resources(&self, runtime: &mut Runtime) -> Result<(), String> {
        for resource in &self.image_resources {
            runtime
                .register_image(resource.handle.into_sui(), resource.image.clone())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn register_font_resources(&self, runtime: &mut Runtime) -> Result<(), String> {
        for resource in &self.font_resources {
            runtime
                .register_font(resource.handle.into_sui(), resource.font.clone())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn error_sink(&self) -> ForeignErrorSink {
        self.errors.clone()
    }

    pub fn start(&self) -> Result<BindingRuntime, String> {
        let ui_tasks = UiTaskQueue::new();
        let ui_handle = ui_tasks.handle().with_message_bus(self.messages.clone());
        let mut runtime = Runtime::new();
        let mut window_ids = Vec::with_capacity(self.windows.len());

        self.register_font_resources(&mut runtime)?;
        self.register_image_resources(&mut runtime)?;

        for window in &self.windows {
            window.root.bind_ui_handle(&ui_handle);
            if let Some(theme) = &self.theme {
                theme.bind_ui_handle(ui_handle.clone());
            }
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
                ui_tasks.clone(),
            );
            let builder = window
                .configure_builder(WindowBuilder::new().title(window.title.clone()).root(root));
            let window_id = runtime
                .add_window(builder)
                .map_err(|error| error.to_string())?;
            if let Some(options) = self.render_options {
                sui::set_window_render_options(window_id, options.into_sui());
            }
            window_ids.push(BindingWindowId::from(window_id));
        }

        Ok(BindingRuntime {
            runtime,
            window_ids,
            ui_tasks,
            messages: self.messages.clone(),
        })
    }

    #[cfg(feature = "desktop")]
    pub fn run(&self) -> Result<(), String> {
        self.run_with_handle(|_| {})
    }

    #[cfg(not(feature = "desktop"))]
    pub fn run(&self) -> Result<(), String> {
        Err("BindingApp::run requires the `desktop` feature".to_string())
    }

    #[cfg(feature = "desktop")]
    pub fn run_with_handle(&self, on_ready: impl FnOnce(BindingUiHandle)) -> Result<(), String> {
        let ui_tasks = UiTaskQueue::new();
        let ui_handle = ui_tasks.handle().with_message_bus(self.messages.clone());
        let mut app = SuiApp::new();
        if let Some(options) = self.render_options {
            app = app.render_options(options.into_sui());
        }

        {
            let mut resources = app.resources();
            for resource in &self.font_resources {
                resources
                    .register_font(resource.handle.into_sui(), resource.font.clone())
                    .map_err(|error| error.to_string())?;
            }
            for resource in &self.image_resources {
                resources
                    .image(resource.handle.into_sui(), resource.image.clone())
                    .map_err(|error| error.to_string())?;
            }
        }

        for window in &self.windows {
            window.root.bind_ui_handle(&ui_handle);
            if let Some(theme) = &self.theme {
                theme.bind_ui_handle(ui_handle.clone());
            }
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
                ui_tasks.clone(),
            );
            let tasks_for_window = ui_tasks.clone();
            let app_window = window
                .configure_app_window(SuiWindow::new(window.title.clone()).root(root))
                .on_command(BINDING_UI_TASKS_READY, move |ctx, _| {
                    tasks_for_window.drain();
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                });
            app = app.window(app_window);
        }

        let tasks_for_waker = ui_tasks.clone();
        app.run_with_handle(move |native_ui| {
            tasks_for_waker.set_waker(move || {
                native_ui.broadcast_application(BINDING_UI_TASKS_READY, ());
            });
            on_ready(ui_handle);
        })
        .map_err(|error| error.to_string())
    }

    #[cfg(not(feature = "desktop"))]
    pub fn run_with_handle(&self, _on_ready: impl FnOnce(BindingUiHandle)) -> Result<(), String> {
        Err("BindingApp::run_with_handle requires the `desktop` feature".to_string())
    }

    pub fn render_window(&self, index: usize) -> Result<BindingRenderSnapshot, String> {
        let window = self
            .windows
            .get(index)
            .ok_or_else(|| format!("window index {index} is out of range"))?;
        let mut runtime = Runtime::new();
        self.register_font_resources(&mut runtime)?;
        self.register_image_resources(&mut runtime)?;
        let builder =
            window.configure_builder(WindowBuilder::new().title(window.title.clone()).root(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
            ));
        let window_id = runtime
            .add_window(builder)
            .map_err(|error| error.to_string())?;
        if let Some(options) = self.render_options {
            sui::set_window_render_options(window_id, options.into_sui());
        }
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
        let mut command_count = 0;
        let mut fill_rect_count = 0;
        let mut draw_image_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            match command {
                SceneCommand::FillRect { .. } => fill_rect_count += 1,
                SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                    draw_image_count += 1;
                }
                _ => {}
            }
        });
        Ok(BindingRenderSnapshot {
            command_count,
            semantics_count: output.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&output.semantics),
            semantics_roles: binding_semantics_roles(&output.semantics),
            semantics_names: binding_semantics_names(&output.semantics),
            semantics_values: binding_semantics_values(&output.semantics),
            semantics_descriptions: binding_semantics_descriptions(&output.semantics),
            semantics_checked: binding_semantics_checked(&output.semantics),
            semantics_busy: binding_semantics_busy(&output.semantics),
            semantics_editable_multiline: binding_semantics_editable_multiline(&output.semantics),
            semantics_disabled: binding_semantics_disabled(&output.semantics),
            semantics_focused: binding_semantics_focused(&output.semantics),
            semantics_hidden: binding_semantics_hidden(&output.semantics),
            semantics_hovered: binding_semantics_hovered(&output.semantics),
            semantics_selected: binding_semantics_selected(&output.semantics),
            semantics_expanded: binding_semantics_expanded(&output.semantics),
            fill_rect_count,
            draw_image_count,
            registered_font_count: output.frame.font_registry.len(),
            registered_image_count: output.frame.image_registry.len(),
        })
    }
}

pub fn registered_image_from_png(data: impl AsRef<[u8]>) -> Result<RegisteredImage, String> {
    let mut decoder = png::Decoder::new(Cursor::new(data.as_ref()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "PNG image exceeds the decoder output size limit".to_string())?;
    let mut buffer = vec![0; output_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "expected 8-bit PNG data after decoding, got {:?}",
            info.bit_depth
        ));
    }
    let rgba = png_frame_to_rgba8(info.color_type, &buffer[..info.buffer_size()])?;
    RegisteredImage::from_rgba8(info.width, info.height, rgba).map_err(|error| error.to_string())
}

fn png_frame_to_rgba8(color_type: png::ColorType, data: &[u8]) -> Result<Vec<u8>, String> {
    match color_type {
        png::ColorType::Rgba => Ok(data.to_vec()),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((data.len() / 3) * 4);
            for chunk in data.chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            Ok(rgba)
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity(data.len() * 4);
            for value in data {
                rgba.extend_from_slice(&[*value, *value, *value, 255]);
            }
            Ok(rgba)
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba = Vec::with_capacity((data.len() / 2) * 4);
            for chunk in data.chunks_exact(2) {
                rgba.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            Ok(rgba)
        }
        png::ColorType::Indexed => Err("indexed PNG data was not expanded to RGBA".to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingWindowId(WindowId);

impl BindingWindowId {
    pub const fn new(raw: u64) -> Self {
        Self(WindowId::new(raw))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    fn into_sui(self) -> WindowId {
        self.0
    }
}

impl From<WindowId> for BindingWindowId {
    fn from(value: WindowId) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingFontHandle(FontHandle);

impl BindingFontHandle {
    pub const fn new(raw: u64) -> Self {
        Self(FontHandle::new(raw))
    }

    pub const fn app_resource(slot: u64) -> Self {
        Self(FontHandle::new(
            BINDING_APP_FONT_HANDLE_NAMESPACE | (slot & BINDING_APP_FONT_SLOT_MASK),
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub const fn into_sui(self) -> FontHandle {
        self.0
    }
}

impl From<FontHandle> for BindingFontHandle {
    fn from(value: FontHandle) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingImageHandle(ImageHandle);

impl BindingImageHandle {
    pub const fn new(raw: u64) -> Self {
        Self(ImageHandle::new(raw))
    }

    pub const fn local(slot: u64) -> Self {
        Self(ImageHandle::new(
            BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE | (slot & BINDING_LOCAL_IMAGE_SLOT_MASK),
        ))
    }

    pub const fn app_resource(slot: u64) -> Self {
        Self(ImageHandle::new(
            BINDING_APP_IMAGE_HANDLE_NAMESPACE | (slot & BINDING_APP_IMAGE_SLOT_MASK),
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub const fn local_slot(self) -> Option<u64> {
        binding_local_image_slot(self.0)
    }

    pub const fn into_sui(self) -> ImageHandle {
        self.0
    }
}

impl From<ImageHandle> for BindingImageHandle {
    fn from(value: ImageHandle) -> Self {
        Self(value)
    }
}

const fn binding_local_image_slot(handle: ImageHandle) -> Option<u64> {
    let raw = handle.get();
    if raw & BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE == BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE {
        Some(raw & BINDING_LOCAL_IMAGE_SLOT_MASK)
    } else {
        None
    }
}

pub fn resolve_binding_image_slots(
    commands: &mut [PaintCommand],
    mut resolve: impl FnMut(u64) -> ImageHandle,
) {
    for command in commands {
        match command {
            PaintCommand::DrawImage { source, .. } => {
                if let Some(slot) = binding_local_image_slot(source.image) {
                    source.image = resolve(slot);
                }
            }
            PaintCommand::DrawImageQuad { source, .. } => {
                if let Some(slot) = binding_local_image_slot(source.image) {
                    source.image = resolve(slot);
                }
            }
            _ => {}
        }
    }
}

pub struct BindingRuntime {
    runtime: Runtime,
    window_ids: Vec<BindingWindowId>,
    ui_tasks: UiTaskQueue,
    messages: BindingMessageBus,
}

impl BindingRuntime {
    pub fn ui_handle(&self) -> BindingUiHandle {
        self.ui_tasks
            .handle()
            .with_message_bus(self.messages.clone())
    }

    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.ui_tasks.set_waker(wake);
    }

    pub fn clear_waker(&self) {
        self.ui_tasks.clear_waker();
    }

    pub fn pending_ui_task_count(&self) -> usize {
        self.ui_tasks.pending_count()
    }

    pub fn drain_ui_tasks(&mut self) -> Result<usize, String> {
        let pending = self.ui_tasks.pending_count();
        if pending > 0 {
            for window_id in self.window_ids.clone() {
                self.runtime
                    .wake_root(window_id.into_sui())
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(pending.saturating_sub(self.ui_tasks.pending_count()))
    }

    pub fn window_count(&self) -> usize {
        self.window_ids.len()
    }

    pub fn window_ids(&self) -> Vec<BindingWindowId> {
        self.window_ids.clone()
    }

    pub fn window_id_at(&self, index: usize) -> Result<BindingWindowId, String> {
        self.window_ids
            .get(index)
            .copied()
            .ok_or_else(|| format!("window index {index} is out of range"))
    }

    pub fn tick(&mut self, frame_time: f64) {
        self.runtime.tick(frame_time);
    }

    pub fn drain_ready_event_count(&mut self) -> usize {
        self.runtime.drain_ready_events().len()
    }

    pub fn request_redraw_all(&mut self) -> Result<(), String> {
        for window_id in self.window_ids.clone() {
            self.request_redraw(window_id)?;
        }
        Ok(())
    }

    pub fn request_redraw(&mut self, window_id: BindingWindowId) -> Result<(), String> {
        self.runtime
            .handle_event(
                window_id.into_sui(),
                Event::Window(WindowEvent::RedrawRequested),
            )
            .map_err(|error| error.to_string())
    }

    pub fn set_render_options(
        &mut self,
        window_id: BindingWindowId,
        options: BindingRenderOptions,
    ) -> Result<(), String> {
        if !self.window_ids.contains(&window_id) {
            return Err(format!(
                "window {} does not belong to this application",
                window_id.get()
            ));
        }
        sui::set_window_render_options(window_id.into_sui(), options.into_sui());
        self.request_redraw(window_id)
    }

    pub fn set_inspector_tracing(
        &mut self,
        window_id: BindingWindowId,
        enabled: bool,
    ) -> Result<(), String> {
        self.runtime
            .set_inspector_tracing(window_id.into_sui(), enabled)
            .map_err(|error| error.to_string())
    }

    pub fn inspector_snapshot(
        &self,
        window_id: BindingWindowId,
    ) -> Result<BindingInspectorSnapshot, String> {
        self.runtime
            .inspector_snapshot(window_id.into_sui())
            .map(BindingInspectorSnapshot::from)
            .map_err(|error| error.to_string())
    }

    pub fn handle_event_at(&mut self, index: usize, event: BindingEvent) -> Result<(), String> {
        let window_id = self.window_id_at(index)?;
        self.handle_event(window_id, event)
    }

    pub fn hover_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
    ) -> Result<(), String> {
        if !node.visible() || node.disabled {
            return Err("semantic node is not actionable".to_owned());
        }
        self.handle_event_at(
            index,
            BindingEvent::Pointer(BindingPointerEvent::new(
                BindingPointerEventKind::Move,
                node.center(),
            )),
        )
    }

    pub fn click_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
    ) -> Result<(), String> {
        self.hover_node_at(index, node)?;
        let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, node.center());
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        self.handle_event_at(index, BindingEvent::Pointer(down))?;
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, node.center());
        up.button = Some(BindingPointerButton::Primary);
        self.handle_event_at(index, BindingEvent::Pointer(up))
    }

    pub fn press_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
        key: impl Into<String>,
    ) -> Result<(), String> {
        self.click_node_at(index, node)?;
        let key = key.into();
        self.handle_event_at(
            index,
            BindingEvent::Keyboard(BindingKeyboardEvent::new(
                key.clone(),
                BindingKeyState::Pressed,
            )),
        )?;
        self.handle_event_at(
            index,
            BindingEvent::Keyboard(BindingKeyboardEvent::new(key, BindingKeyState::Released)),
        )
    }

    pub fn fill_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
        text: impl Into<String>,
    ) -> Result<(), String> {
        self.click_node_at(index, node)?;
        let text = text.into();
        self.handle_event_at(index, BindingEvent::Ime(BindingImeEvent::CompositionStart))?;
        self.handle_event_at(
            index,
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate {
                text: text.clone(),
                cursor_start: None,
                cursor_end: None,
            }),
        )?;
        self.handle_event_at(
            index,
            BindingEvent::Ime(BindingImeEvent::CompositionCommit { text }),
        )?;
        self.handle_event_at(index, BindingEvent::Ime(BindingImeEvent::CompositionEnd))
    }

    pub fn handle_event(
        &mut self,
        window_id: BindingWindowId,
        event: BindingEvent,
    ) -> Result<(), String> {
        let event = event.into_sui_event()?;
        self.runtime
            .handle_event(window_id.into_sui(), event)
            .map_err(|error| error.to_string())?;
        self.drain_ui_tasks()?;
        Ok(())
    }

    pub fn wake_window(&mut self, window_id: BindingWindowId) -> Result<(), String> {
        self.runtime
            .wake_root(window_id.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn needs_render(&self, window_id: BindingWindowId) -> Result<bool, String> {
        self.runtime
            .needs_render(window_id.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn render_window_at(&mut self, index: usize) -> Result<BindingRenderSnapshot, String> {
        let window_id = self.window_id_at(index)?;
        self.render_window(window_id)
    }

    pub fn render_window(
        &mut self,
        window_id: BindingWindowId,
    ) -> Result<BindingRenderSnapshot, String> {
        self.drain_ui_tasks()?;
        let output = self
            .runtime
            .render(window_id.into_sui())
            .map_err(|error| error.to_string())?;
        let mut command_count = 0;
        let mut fill_rect_count = 0;
        let mut draw_image_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            match command {
                SceneCommand::FillRect { .. } => fill_rect_count += 1,
                SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                    draw_image_count += 1;
                }
                _ => {}
            }
        });
        Ok(BindingRenderSnapshot {
            command_count,
            semantics_count: output.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&output.semantics),
            semantics_roles: binding_semantics_roles(&output.semantics),
            semantics_names: binding_semantics_names(&output.semantics),
            semantics_values: binding_semantics_values(&output.semantics),
            semantics_descriptions: binding_semantics_descriptions(&output.semantics),
            semantics_checked: binding_semantics_checked(&output.semantics),
            semantics_busy: binding_semantics_busy(&output.semantics),
            semantics_editable_multiline: binding_semantics_editable_multiline(&output.semantics),
            semantics_disabled: binding_semantics_disabled(&output.semantics),
            semantics_focused: binding_semantics_focused(&output.semantics),
            semantics_hidden: binding_semantics_hidden(&output.semantics),
            semantics_hovered: binding_semantics_hovered(&output.semantics),
            semantics_selected: binding_semantics_selected(&output.semantics),
            semantics_expanded: binding_semantics_expanded(&output.semantics),
            fill_rect_count,
            draw_image_count,
            registered_font_count: output.frame.font_registry.len(),
            registered_image_count: output.frame.image_registry.len(),
        })
    }
}

impl fmt::Debug for BindingRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingRuntime")
            .field("window_ids", &self.window_ids)
            .field("pending_ui_task_count", &self.pending_ui_task_count())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingRenderSnapshot {
    pub command_count: usize,
    pub semantics_count: usize,
    pub semantics_nodes: Vec<BindingSemanticNode>,
    pub semantics_roles: Vec<String>,
    pub semantics_names: Vec<String>,
    pub semantics_values: Vec<String>,
    pub semantics_descriptions: Vec<String>,
    pub semantics_checked: Vec<String>,
    pub semantics_busy: Vec<bool>,
    pub semantics_editable_multiline: Vec<bool>,
    pub semantics_disabled: Vec<bool>,
    pub semantics_focused: Vec<bool>,
    pub semantics_hidden: Vec<bool>,
    pub semantics_hovered: Vec<bool>,
    pub semantics_selected: Vec<bool>,
    pub semantics_expanded: Vec<String>,
    pub fill_rect_count: usize,
    pub draw_image_count: usize,
    pub registered_font_count: usize,
    pub registered_image_count: usize,
}

impl BindingRenderSnapshot {
    pub fn find_nodes(
        &self,
        role: Option<&str>,
        name: Option<&str>,
        text: Option<&str>,
        description: Option<&str>,
        focused: Option<bool>,
        visible: Option<bool>,
    ) -> Vec<BindingSemanticNode> {
        self.semantics_nodes
            .iter()
            .filter(|node| role.is_none_or(|role| node.role == role))
            .filter(|node| name.is_none_or(|name| node.name.as_deref() == Some(name)))
            .filter(|node| {
                text.is_none_or(|text| {
                    node.name.as_deref() == Some(text) || node.value.as_deref() == Some(text)
                })
            })
            .filter(|node| {
                description
                    .is_none_or(|description| node.description.as_deref() == Some(description))
            })
            .filter(|node| focused.is_none_or(|focused| node.focused == focused))
            .filter(|node| visible.is_none_or(|visible| node.visible() == visible))
            .cloned()
            .collect()
    }

    pub fn get_one(
        &self,
        role: Option<&str>,
        name: Option<&str>,
        text: Option<&str>,
    ) -> Result<BindingSemanticNode, String> {
        let nodes = self.find_nodes(role, name, text, None, None, Some(true));
        match nodes.as_slice() {
            [node] => Ok(node.clone()),
            [] => Err("semantic query did not match any visible nodes".to_owned()),
            _ => Err(format!(
                "semantic query matched {} visible nodes instead of exactly one",
                nodes.len()
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingSemanticNode {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub actions: Vec<String>,
    pub checked: Option<String>,
    pub busy: bool,
    pub disabled: bool,
    pub focused: bool,
    pub hidden: bool,
    pub hovered: bool,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub editable: bool,
    pub multiline: bool,
}

impl BindingSemanticNode {
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    pub fn visible(&self) -> bool {
        !self.hidden && self.width > 0.0 && self.height > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingInspectorSnapshot {
    pub window_id: u64,
    pub title: String,
    pub tracing_enabled: bool,
    pub focused_widget_id: Option<u64>,
    pub window_focused: bool,
    pub scheduled_phases: Vec<String>,
    pub semantics_count: usize,
    pub semantics_nodes: Vec<BindingSemanticNode>,
    pub widget_count: usize,
    pub stack_host_count: usize,
    pub overlay_count: usize,
    pub timer_count: usize,
    pub async_task_count: usize,
    pub requested_animation_frame_count: usize,
    pub widget_diagnostics_count: usize,
    pub event_route_count: usize,
    pub reactive_invalidation_count: usize,
    pub command_dispatch_count: usize,
    pub invalidation_count: usize,
    pub widget_rebuild_count: usize,
    pub frame_timings: Vec<BindingFrameTiming>,
    pub widget_timings: Vec<BindingWidgetTiming>,
    pub event_routes: Vec<BindingEventRouteTrace>,
    pub reactive_invalidations: Vec<BindingReactiveInvalidationTrace>,
    pub command_dispatches: Vec<BindingCommandDispatchTrace>,
    pub invalidations: Vec<BindingInvalidationTrace>,
    pub widget_rebuilds: Vec<BindingWidgetRebuildTrace>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingFrameTiming {
    pub phase: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingWidgetTiming {
    pub widget_id: u64,
    pub widget_name: String,
    pub phase: String,
    pub duration_ms: f64,
    pub calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEventRouteTrace {
    pub sequence: u64,
    pub event_kind: String,
    pub target_id: u64,
    pub path: Vec<u64>,
    pub handled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingReactiveInvalidationTrace {
    pub widget_id: u64,
    pub source_name: String,
    pub version: u64,
    pub kind: String,
    pub delivered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCommandDispatchTrace {
    pub sequence: u64,
    pub name: String,
    pub payload_type: String,
    pub target: String,
    pub delivery: String,
    pub handlers: Vec<String>,
    pub handled: bool,
    pub delivered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingInvalidationTrace {
    pub target: String,
    pub kind: String,
    pub source: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingWidgetRebuildTrace {
    pub widget_id: u64,
    pub widget_name: String,
    pub reason: String,
}

impl From<sui::WindowInspectorSnapshot> for BindingInspectorSnapshot {
    fn from(value: sui::WindowInspectorSnapshot) -> Self {
        let schedule = value.schedule;
        let mut scheduled_phases = Vec::new();
        for (name, scheduled) in [
            ("measure", schedule.measure),
            ("arrange", schedule.arrange),
            ("ordering", schedule.ordering),
            ("paint", schedule.paint),
            ("semantics", schedule.semantics),
            ("hit_test", schedule.hit_test),
            ("text", schedule.text),
            ("resources", schedule.resources),
        ] {
            if scheduled {
                scheduled_phases.push(name.to_owned());
            }
        }
        let frame_timings = value
            .last_render_diagnostics
            .phase_timings
            .iter()
            .map(|sample| BindingFrameTiming {
                phase: sample.phase.label().to_owned(),
                duration_ms: sample.duration_ms,
            })
            .collect();
        let widget_timings = value
            .last_render_diagnostics
            .widget_timings
            .iter()
            .map(|sample| BindingWidgetTiming {
                widget_id: sample.widget_id.get(),
                widget_name: sample.widget_name.to_owned(),
                phase: sample.phase.label().to_owned(),
                duration_ms: sample.duration_ms,
                calls: sample.calls,
            })
            .collect();
        let event_routes = value
            .history
            .event_routes
            .iter()
            .map(|sample| BindingEventRouteTrace {
                sequence: sample.sequence,
                event_kind: sample.event_kind.to_owned(),
                target_id: sample.target.get(),
                path: sample.path.iter().map(|id| id.get()).collect(),
                handled: sample.handled,
            })
            .collect();
        let reactive_invalidations = value
            .history
            .reactive_invalidations
            .iter()
            .map(|sample| BindingReactiveInvalidationTrace {
                widget_id: sample.widget_id.get(),
                source_name: sample.source_name.clone(),
                version: sample.version,
                kind: format!("{:?}", sample.kind),
                delivered: sample.delivered,
            })
            .collect();
        let command_dispatches = value
            .history
            .command_dispatches
            .iter()
            .map(|sample| BindingCommandDispatchTrace {
                sequence: sample.sequence,
                name: sample.name.clone(),
                payload_type: sample.payload_type.clone(),
                target: format!("{:?}", sample.target),
                delivery: format!("{:?}", sample.delivery),
                handlers: sample.handlers.clone(),
                handled: sample.handled,
                delivered: sample.delivered,
            })
            .collect();
        let invalidations = value
            .history
            .invalidations
            .iter()
            .map(|sample| BindingInvalidationTrace {
                target: format!("{:?}", sample.target),
                kind: format!("{:?}", sample.kind),
                source: sample.source.clone(),
                reason: sample.reason.clone(),
            })
            .collect();
        let widget_rebuilds = value
            .history
            .widget_rebuilds
            .iter()
            .map(|sample| BindingWidgetRebuildTrace {
                widget_id: sample.widget_id.get(),
                widget_name: sample.widget_name.to_owned(),
                reason: sample.reason.clone(),
            })
            .collect();
        Self {
            window_id: value.window_id.get(),
            title: value.title,
            tracing_enabled: value.tracing_enabled,
            focused_widget_id: value.focus_state.focused_widget.map(WidgetId::get),
            window_focused: value.focus_state.window_focused,
            scheduled_phases,
            semantics_count: value.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&value.semantics),
            widget_count: value.widget_graph.nodes.len(),
            stack_host_count: value.widget_graph.stack_hosts.len(),
            overlay_count: value.overlays.overlays.len(),
            timer_count: value.scheduler.timers.len(),
            async_task_count: value.scheduler.async_tasks.len(),
            requested_animation_frame_count: value.scheduler.requested_animation_frames.len(),
            widget_diagnostics_count: value.widget_diagnostics.len(),
            event_route_count: value.history.event_routes.len(),
            reactive_invalidation_count: value.history.reactive_invalidations.len(),
            command_dispatch_count: value.history.command_dispatches.len(),
            invalidation_count: value.history.invalidations.len(),
            widget_rebuild_count: value.history.widget_rebuilds.len(),
            frame_timings,
            widget_timings,
            event_routes,
            reactive_invalidations,
            command_dispatches,
            invalidations,
            widget_rebuilds,
        }
    }
}

pub fn binding_semantics_role_name(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::Window => "window",
        SemanticsRole::Root => "root",
        SemanticsRole::GenericContainer => "generic_container",
        SemanticsRole::Separator => "separator",
        SemanticsRole::List => "list",
        SemanticsRole::ListItem => "list_item",
        SemanticsRole::Tree => "tree",
        SemanticsRole::Table => "table",
        SemanticsRole::Splitter => "splitter",
        SemanticsRole::Breadcrumb => "breadcrumb",
        SemanticsRole::TabBar => "tab_bar",
        SemanticsRole::Tabs => "tabs",
        SemanticsRole::Button => "button",
        SemanticsRole::Link => "link",
        SemanticsRole::CheckBox => "checkbox",
        SemanticsRole::Switch => "switch",
        SemanticsRole::RadioButton => "radio_button",
        SemanticsRole::RadioGroup => "radio_group",
        SemanticsRole::Menu => "menu",
        SemanticsRole::MenuItem => "menu_item",
        SemanticsRole::ContextMenu => "context_menu",
        SemanticsRole::Tooltip => "tooltip",
        SemanticsRole::Dialog => "dialog",
        SemanticsRole::Popover => "popover",
        SemanticsRole::Slider => "slider",
        SemanticsRole::ProgressBar => "progress_bar",
        SemanticsRole::BusyIndicator => "busy_indicator",
        SemanticsRole::Document => "document",
        SemanticsRole::Paragraph => "paragraph",
        SemanticsRole::Heading => "heading",
        SemanticsRole::Code => "code",
        SemanticsRole::Status => "status",
        SemanticsRole::Attachment => "attachment",
        SemanticsRole::Text => "text",
        SemanticsRole::TextInput => "text_input",
        SemanticsRole::SpinBox => "spin_box",
        SemanticsRole::ComboBox => "combo_box",
        SemanticsRole::Image => "image",
        SemanticsRole::ColorSwatch => "color_swatch",
        SemanticsRole::ColorPicker => "color_picker",
        SemanticsRole::Canvas => "canvas",
        SemanticsRole::ScrollView => "scroll_view",
    }
}

pub fn binding_semantics_role_from_name(value: &str) -> Option<SemanticsRole> {
    match value {
        "window" => Some(SemanticsRole::Window),
        "root" => Some(SemanticsRole::Root),
        "generic_container" | "generic-container" | "genericContainer" | "generic" => {
            Some(SemanticsRole::GenericContainer)
        }
        "separator" => Some(SemanticsRole::Separator),
        "list" => Some(SemanticsRole::List),
        "list_item" | "list-item" | "listItem" => Some(SemanticsRole::ListItem),
        "tree" => Some(SemanticsRole::Tree),
        "table" => Some(SemanticsRole::Table),
        "splitter" => Some(SemanticsRole::Splitter),
        "breadcrumb" => Some(SemanticsRole::Breadcrumb),
        "tab_bar" | "tab-bar" | "tabBar" => Some(SemanticsRole::TabBar),
        "tabs" => Some(SemanticsRole::Tabs),
        "button" => Some(SemanticsRole::Button),
        "link" => Some(SemanticsRole::Link),
        "checkbox" | "check_box" | "check-box" | "checkBox" => Some(SemanticsRole::CheckBox),
        "switch" => Some(SemanticsRole::Switch),
        "radio_button" | "radio-button" | "radioButton" => Some(SemanticsRole::RadioButton),
        "radio_group" | "radio-group" | "radioGroup" => Some(SemanticsRole::RadioGroup),
        "menu" => Some(SemanticsRole::Menu),
        "menu_item" | "menu-item" | "menuItem" => Some(SemanticsRole::MenuItem),
        "context_menu" | "context-menu" | "contextMenu" => Some(SemanticsRole::ContextMenu),
        "tooltip" => Some(SemanticsRole::Tooltip),
        "dialog" => Some(SemanticsRole::Dialog),
        "popover" => Some(SemanticsRole::Popover),
        "slider" => Some(SemanticsRole::Slider),
        "progress_bar" | "progress-bar" | "progressBar" => Some(SemanticsRole::ProgressBar),
        "busy_indicator" | "busy-indicator" | "busyIndicator" => Some(SemanticsRole::BusyIndicator),
        "document" => Some(SemanticsRole::Document),
        "paragraph" => Some(SemanticsRole::Paragraph),
        "heading" => Some(SemanticsRole::Heading),
        "code" => Some(SemanticsRole::Code),
        "status" => Some(SemanticsRole::Status),
        "attachment" => Some(SemanticsRole::Attachment),
        "text" => Some(SemanticsRole::Text),
        "text_input" | "text-input" | "textInput" => Some(SemanticsRole::TextInput),
        "spin_box" | "spin-box" | "spinBox" => Some(SemanticsRole::SpinBox),
        "combo_box" | "combo-box" | "comboBox" => Some(SemanticsRole::ComboBox),
        "image" => Some(SemanticsRole::Image),
        "color_swatch" | "color-swatch" | "colorSwatch" => Some(SemanticsRole::ColorSwatch),
        "color_picker" | "color-picker" | "colorPicker" => Some(SemanticsRole::ColorPicker),
        "canvas" => Some(SemanticsRole::Canvas),
        "scroll_view" | "scroll-view" | "scrollView" => Some(SemanticsRole::ScrollView),
        _ => None,
    }
}

pub fn binding_toggle_state_name(state: ToggleState) -> &'static str {
    match state {
        ToggleState::Unchecked => "unchecked",
        ToggleState::Checked => "checked",
        ToggleState::Mixed => "mixed",
    }
}

pub fn binding_toggle_state_from_name(value: &str) -> Option<ToggleState> {
    match value {
        "unchecked" | "false" | "off" => Some(ToggleState::Unchecked),
        "checked" | "true" | "on" => Some(ToggleState::Checked),
        "mixed" | "indeterminate" => Some(ToggleState::Mixed),
        _ => None,
    }
}

pub fn binding_semantics_value_text(value: Option<&SemanticsValue>) -> String {
    match value {
        Some(SemanticsValue::Text(value)) => value.clone(),
        Some(SemanticsValue::Number(value)) => value.to_string(),
        Some(SemanticsValue::Range { value, min, max }) => format!("{value}:{min}:{max}"),
        None => String::new(),
    }
}

pub fn binding_semantics_roles(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| binding_semantics_role_name(&node.role).to_owned())
        .collect()
}

pub fn binding_semantics_names(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node.name.clone().unwrap_or_default())
        .collect()
}

pub fn binding_semantics_values(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            if node
                .editable_text
                .as_ref()
                .is_some_and(|editable| editable.password)
            {
                return match node.value.as_ref() {
                    Some(SemanticsValue::Text(value)) => "•".repeat(value.chars().count()),
                    _ => String::new(),
                };
            }
            binding_semantics_value_text(node.value.as_ref())
        })
        .collect()
}

pub fn binding_semantics_nodes(nodes: &[SemanticsNode]) -> Vec<BindingSemanticNode> {
    nodes
        .iter()
        .map(|node| {
            let value = if node
                .editable_text
                .as_ref()
                .is_some_and(|editable| editable.password)
            {
                match node.value.as_ref() {
                    Some(SemanticsValue::Text(value)) => Some("•".repeat(value.chars().count())),
                    _ => None,
                }
            } else {
                node.value
                    .as_ref()
                    .map(|value| binding_semantics_value_text(Some(value)))
            };
            BindingSemanticNode {
                id: node.id.get(),
                parent_id: node.parent.map(WidgetId::get),
                role: binding_semantics_role_name(&node.role).to_owned(),
                name: node.name.clone(),
                value,
                description: node.description.clone(),
                x: node.bounds.x(),
                y: node.bounds.y(),
                width: node.bounds.width(),
                height: node.bounds.height(),
                actions: node
                    .actions
                    .iter()
                    .map(|action| format!("{action:?}"))
                    .collect(),
                checked: node
                    .state
                    .checked
                    .map(binding_toggle_state_name)
                    .map(str::to_owned),
                busy: node.state.busy,
                disabled: node.state.disabled,
                focused: node.state.focused,
                hidden: node.state.hidden,
                hovered: node.state.hovered,
                selected: node.state.selected,
                expanded: node.state.expanded,
                editable: node.editable_text.is_some(),
                multiline: node
                    .editable_text
                    .as_ref()
                    .is_some_and(|editable| editable.multiline),
            }
        })
        .collect()
}

pub fn binding_semantics_descriptions(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node.description.clone().unwrap_or_default())
        .collect()
}

pub fn binding_semantics_checked(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            node.state
                .checked
                .map(binding_toggle_state_name)
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

pub fn binding_semantics_busy(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.busy).collect()
}

pub fn binding_semantics_editable_multiline(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes
        .iter()
        .map(|node| {
            node.editable_text
                .as_ref()
                .is_some_and(|editable| editable.multiline)
        })
        .collect()
}

pub fn binding_semantics_disabled(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.disabled).collect()
}

pub fn binding_semantics_focused(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.focused).collect()
}

pub fn binding_semantics_hidden(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.hidden).collect()
}

pub fn binding_semantics_hovered(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.hovered).collect()
}

pub fn binding_semantics_selected(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.selected).collect()
}

pub fn binding_semantics_expanded(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node.state.expanded {
            Some(true) => "expanded",
            Some(false) => "collapsed",
            None => "",
        })
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BindingModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl From<Modifiers> for BindingModifiers {
    fn from(value: Modifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

impl From<BindingModifiers> for Modifiers {
    fn from(value: BindingModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingPointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<PointerButton> for BindingPointerButton {
    fn from(value: PointerButton) -> Self {
        match value {
            PointerButton::Primary => Self::Primary,
            PointerButton::Secondary => Self::Secondary,
            PointerButton::Middle => Self::Middle,
            PointerButton::Back => Self::Back,
            PointerButton::Forward => Self::Forward,
            PointerButton::Other(button) => Self::Other(button),
        }
    }
}

impl From<BindingPointerButton> for PointerButton {
    fn from(value: BindingPointerButton) -> Self {
        match value {
            BindingPointerButton::Primary => Self::Primary,
            BindingPointerButton::Secondary => Self::Secondary,
            BindingPointerButton::Middle => Self::Middle,
            BindingPointerButton::Back => Self::Back,
            BindingPointerButton::Forward => Self::Forward,
            BindingPointerButton::Other(button) => Self::Other(button),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingPointerKind {
    #[default]
    Mouse,
    Touch,
    Pen,
    Unknown,
}

impl From<PointerKind> for BindingPointerKind {
    fn from(value: PointerKind) -> Self {
        match value {
            PointerKind::Mouse => Self::Mouse,
            PointerKind::Touch => Self::Touch,
            PointerKind::Pen => Self::Pen,
            PointerKind::Unknown => Self::Unknown,
        }
    }
}

impl From<BindingPointerKind> for PointerKind {
    fn from(value: BindingPointerKind) -> Self {
        match value {
            BindingPointerKind::Mouse => Self::Mouse,
            BindingPointerKind::Touch => Self::Touch,
            BindingPointerKind::Pen => Self::Pen,
            BindingPointerKind::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingPointerEventKind {
    Down,
    Up,
    Move,
    Scroll,
    Enter,
    Leave,
    Cancel,
}

impl From<PointerEventKind> for BindingPointerEventKind {
    fn from(value: PointerEventKind) -> Self {
        match value {
            PointerEventKind::Down => Self::Down,
            PointerEventKind::Up => Self::Up,
            PointerEventKind::Move => Self::Move,
            PointerEventKind::Scroll => Self::Scroll,
            PointerEventKind::Enter => Self::Enter,
            PointerEventKind::Leave => Self::Leave,
            PointerEventKind::Cancel => Self::Cancel,
        }
    }
}

impl From<BindingPointerEventKind> for PointerEventKind {
    fn from(value: BindingPointerEventKind) -> Self {
        match value {
            BindingPointerEventKind::Down => Self::Down,
            BindingPointerEventKind::Up => Self::Up,
            BindingPointerEventKind::Move => Self::Move,
            BindingPointerEventKind::Scroll => Self::Scroll,
            BindingPointerEventKind::Enter => Self::Enter,
            BindingPointerEventKind::Leave => Self::Leave,
            BindingPointerEventKind::Cancel => Self::Cancel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingScrollDelta {
    Lines(Vector),
    Pixels(Vector),
}

impl From<ScrollDelta> for BindingScrollDelta {
    fn from(value: ScrollDelta) -> Self {
        match value {
            ScrollDelta::Lines(delta) => Self::Lines(delta),
            ScrollDelta::Pixels(delta) => Self::Pixels(delta),
        }
    }
}

impl From<BindingScrollDelta> for ScrollDelta {
    fn from(value: BindingScrollDelta) -> Self {
        match value {
            BindingScrollDelta::Lines(delta) => Self::Lines(delta),
            BindingScrollDelta::Pixels(delta) => Self::Pixels(delta),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingPointerEvent {
    pub pointer_id: u64,
    pub kind: BindingPointerEventKind,
    pub position: Point,
    pub delta: Vector,
    pub scroll_delta: Option<BindingScrollDelta>,
    pub button: Option<BindingPointerButton>,
    pub buttons: u8,
    pub modifiers: BindingModifiers,
    pub pointer_kind: BindingPointerKind,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingRawMouseMotionEvent {
    pub delta: Vector,
    pub modifiers: BindingModifiers,
}

impl From<&RawMouseMotionEvent> for BindingRawMouseMotionEvent {
    fn from(value: &RawMouseMotionEvent) -> Self {
        Self {
            delta: value.delta,
            modifiers: value.modifiers.into(),
        }
    }
}

impl From<BindingRawMouseMotionEvent> for RawMouseMotionEvent {
    fn from(value: BindingRawMouseMotionEvent) -> Self {
        Self {
            delta: value.delta,
            modifiers: value.modifiers.into(),
        }
    }
}

impl BindingPointerEvent {
    pub fn new(kind: BindingPointerEventKind, position: Point) -> Self {
        let event = PointerEvent::new(kind.into(), position);
        Self::from(&event)
    }
}

impl From<&PointerEvent> for BindingPointerEvent {
    fn from(value: &PointerEvent) -> Self {
        Self {
            pointer_id: value.pointer_id,
            kind: value.kind.into(),
            position: value.position,
            delta: value.delta,
            scroll_delta: value.scroll_delta.map(Into::into),
            button: value.button.map(Into::into),
            buttons: value.buttons.bits(),
            modifiers: value.modifiers.into(),
            pointer_kind: value.pointer_kind.into(),
            is_primary: value.is_primary,
        }
    }
}

impl From<BindingPointerEvent> for PointerEvent {
    fn from(value: BindingPointerEvent) -> Self {
        Self {
            pointer_id: value.pointer_id,
            kind: value.kind.into(),
            position: value.position,
            delta: value.delta,
            scroll_delta: value.scroll_delta.map(Into::into),
            button: value.button.map(Into::into),
            buttons: PointerButtons::new(value.buttons),
            modifiers: value.modifiers.into(),
            pointer_kind: value.pointer_kind.into(),
            is_primary: value.is_primary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKeyState {
    Pressed,
    Released,
}

impl From<KeyState> for BindingKeyState {
    fn from(value: KeyState) -> Self {
        match value {
            KeyState::Pressed => Self::Pressed,
            KeyState::Released => Self::Released,
        }
    }
}

impl From<BindingKeyState> for KeyState {
    fn from(value: BindingKeyState) -> Self {
        match value {
            BindingKeyState::Pressed => Self::Pressed,
            BindingKeyState::Released => Self::Released,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingKeyboardEvent {
    pub key: String,
    pub code: String,
    pub text: Option<String>,
    pub state: BindingKeyState,
    pub modifiers: BindingModifiers,
    pub repeat: bool,
    pub is_composing: bool,
}

impl BindingKeyboardEvent {
    pub fn new(key: impl Into<String>, state: BindingKeyState) -> Self {
        let event = KeyboardEvent::new(key, state.into());
        Self::from(&event)
    }
}

impl From<&KeyboardEvent> for BindingKeyboardEvent {
    fn from(value: &KeyboardEvent) -> Self {
        Self {
            key: value.key.clone(),
            code: value.code.clone(),
            text: value.text.clone(),
            state: value.state.into(),
            modifiers: value.modifiers.into(),
            repeat: value.repeat,
            is_composing: value.is_composing,
        }
    }
}

impl From<BindingKeyboardEvent> for KeyboardEvent {
    fn from(value: BindingKeyboardEvent) -> Self {
        Self {
            key: value.key,
            code: value.code,
            text: value.text,
            state: value.state.into(),
            modifiers: value.modifiers.into(),
            repeat: value.repeat,
            is_composing: value.is_composing,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingImeEvent {
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor_start: Option<usize>,
        cursor_end: Option<usize>,
    },
    CompositionCommit {
        text: String,
    },
    CompositionEnd,
}

impl From<&ImeEvent> for BindingImeEvent {
    fn from(value: &ImeEvent) -> Self {
        match value {
            ImeEvent::CompositionStart => Self::CompositionStart,
            ImeEvent::CompositionUpdate { text, cursor_range } => Self::CompositionUpdate {
                text: text.clone(),
                cursor_start: cursor_range.as_ref().map(|range| range.start),
                cursor_end: cursor_range.as_ref().map(|range| range.end),
            },
            ImeEvent::CompositionCommit { text } => Self::CompositionCommit { text: text.clone() },
            ImeEvent::CompositionEnd => Self::CompositionEnd,
        }
    }
}

impl From<BindingImeEvent> for ImeEvent {
    fn from(value: BindingImeEvent) -> Self {
        match value {
            BindingImeEvent::CompositionStart => Self::CompositionStart,
            BindingImeEvent::CompositionUpdate {
                text,
                cursor_start,
                cursor_end,
            } => Self::CompositionUpdate {
                text,
                cursor_range: match (cursor_start, cursor_end) {
                    (Some(start), Some(end)) => Some(start..end),
                    _ => None,
                },
            },
            BindingImeEvent::CompositionCommit { text } => Self::CompositionCommit { text },
            BindingImeEvent::CompositionEnd => Self::CompositionEnd,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingWindowEvent {
    CloseRequested,
    Resized(Size),
    Moved(Point),
    ScaleFactorChanged {
        scale_factor: f64,
        raw_dpi: Option<f32>,
        suggested_size: Option<Size>,
    },
    Focused(bool),
    Occluded(bool),
    SafeAreaChanged {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    ExternalFileHovered(String),
    ExternalFileHoverCancelled,
    ExternalFileDropped(String),
    RedrawRequested,
}

impl From<&WindowEvent> for BindingWindowEvent {
    fn from(value: &WindowEvent) -> Self {
        match value {
            WindowEvent::CloseRequested => Self::CloseRequested,
            WindowEvent::Resized(size) => Self::Resized(*size),
            WindowEvent::Moved(position) => Self::Moved(*position),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            } => Self::ScaleFactorChanged {
                scale_factor: *scale_factor,
                raw_dpi: *raw_dpi,
                suggested_size: *suggested_size,
            },
            WindowEvent::Focused(focused) => Self::Focused(*focused),
            WindowEvent::Occluded(occluded) => Self::Occluded(*occluded),
            WindowEvent::SafeAreaChanged(insets) => Self::SafeAreaChanged {
                left: insets.left,
                top: insets.top,
                right: insets.right,
                bottom: insets.bottom,
            },
            WindowEvent::ExternalFileHovered(path) => {
                Self::ExternalFileHovered(path.to_string_lossy().into_owned())
            }
            WindowEvent::ExternalFileHoverCancelled => Self::ExternalFileHoverCancelled,
            WindowEvent::ExternalFileDropped(path) => {
                Self::ExternalFileDropped(path.to_string_lossy().into_owned())
            }
            WindowEvent::RedrawRequested => Self::RedrawRequested,
        }
    }
}

impl From<BindingWindowEvent> for WindowEvent {
    fn from(value: BindingWindowEvent) -> Self {
        match value {
            BindingWindowEvent::CloseRequested => Self::CloseRequested,
            BindingWindowEvent::Resized(size) => Self::Resized(size),
            BindingWindowEvent::Moved(position) => Self::Moved(position),
            BindingWindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            } => Self::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            },
            BindingWindowEvent::Focused(focused) => Self::Focused(focused),
            BindingWindowEvent::Occluded(occluded) => Self::Occluded(occluded),
            BindingWindowEvent::SafeAreaChanged {
                left,
                top,
                right,
                bottom,
            } => Self::SafeAreaChanged(sui::SafeAreaInsets::new(left, top, right, bottom)),
            BindingWindowEvent::ExternalFileHovered(path) => Self::ExternalFileHovered(path.into()),
            BindingWindowEvent::ExternalFileHoverCancelled => Self::ExternalFileHoverCancelled,
            BindingWindowEvent::ExternalFileDropped(path) => Self::ExternalFileDropped(path.into()),
            BindingWindowEvent::RedrawRequested => Self::RedrawRequested,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCustomEvent {
    pub kind: String,
    pub payload: Option<String>,
}

impl From<&CustomEvent> for BindingCustomEvent {
    fn from(value: &CustomEvent) -> Self {
        Self {
            kind: value.kind.clone(),
            payload: value.payload.clone(),
        }
    }
}

impl From<BindingCustomEvent> for CustomEvent {
    fn from(value: BindingCustomEvent) -> Self {
        Self {
            kind: value.kind,
            payload: value.payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingEvent {
    Pointer(BindingPointerEvent),
    RawMouseMotion(BindingRawMouseMotionEvent),
    Keyboard(BindingKeyboardEvent),
    Ime(BindingImeEvent),
    Window(BindingWindowEvent),
    Custom(BindingCustomEvent),
    Unsupported { kind: String },
}

impl BindingEvent {
    pub fn kind(&self) -> &str {
        match self {
            Self::Pointer(_) => "pointer",
            Self::RawMouseMotion(_) => "raw_mouse_motion",
            Self::Keyboard(_) => "keyboard",
            Self::Ime(_) => "ime",
            Self::Window(_) => "window",
            Self::Custom(_) => "custom",
            Self::Unsupported { kind } => kind,
        }
    }

    pub fn into_sui_event(self) -> Result<Event, String> {
        match self {
            Self::Pointer(event) => Ok(Event::Pointer(event.into())),
            Self::RawMouseMotion(event) => Ok(Event::RawMouseMotion(event.into())),
            Self::Keyboard(event) => Ok(Event::Keyboard(event.into())),
            Self::Ime(event) => Ok(Event::Ime(event.into())),
            Self::Window(event) => Ok(Event::Window(event.into())),
            Self::Custom(event) => Ok(Event::Custom(event.into())),
            Self::Unsupported { kind } => {
                Err(format!("{kind} events cannot be dispatched from bindings"))
            }
        }
    }
}

impl From<&Event> for BindingEvent {
    fn from(value: &Event) -> Self {
        match value {
            Event::Pointer(event) => Self::Pointer(BindingPointerEvent::from(event)),
            Event::RawMouseMotion(event) => {
                Self::RawMouseMotion(BindingRawMouseMotionEvent::from(event))
            }
            Event::Keyboard(event) => Self::Keyboard(BindingKeyboardEvent::from(event)),
            Event::Ime(event) => Self::Ime(BindingImeEvent::from(event)),
            Event::Window(event) => Self::Window(BindingWindowEvent::from(event)),
            Event::Custom(event) => Self::Custom(BindingCustomEvent::from(event)),
            Event::Drag(_) => Self::Unsupported {
                kind: "drag".to_string(),
            },
            Event::Semantics(_) => Self::Unsupported {
                kind: "semantics".to_string(),
            },
            Event::Wake(_) => Self::Unsupported {
                kind: "wake".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingShader {
    shader: WidgetShader,
}

impl BindingShader {
    pub const fn from_widget_shader(shader: WidgetShader) -> Self {
        Self { shader }
    }

    pub const fn color_wheel() -> Self {
        Self::from_widget_shader(WidgetShader::ColorWheel)
    }

    pub const fn hue_bar() -> Self {
        Self::from_widget_shader(WidgetShader::ColorPickerHueBar)
    }

    pub fn saturation_value_plane(
        color_space: ColorSpace,
        hue: f32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerSaturationValuePlane {
            color_space,
            hue,
            max_value,
        })
    }

    pub fn saturation_bar(
        color_space: ColorSpace,
        hue: f32,
        value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerSaturationBar {
            color_space,
            hue,
            value,
        })
    }

    pub fn value_bar(
        color_space: ColorSpace,
        hue: f32,
        saturation: f32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerValueBar {
            color_space,
            hue,
            saturation,
            max_value,
        })
    }

    pub fn alpha_bar(color: Color) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerAlphaBar { color })
    }

    pub fn rgb_channel_bar(
        color: Color,
        channel: u32,
        max_value: f32,
    ) -> PaintValidationResult<Self> {
        Self::new_validated(WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        })
    }

    pub const fn widget_shader(self) -> WidgetShader {
        self.shader
    }

    fn new_validated(shader: WidgetShader) -> PaintValidationResult<Self> {
        validate_widget_shader(shader)?;
        Ok(Self::from_widget_shader(shader))
    }
}

impl From<BindingShader> for WidgetShader {
    fn from(value: BindingShader) -> Self {
        value.widget_shader()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeGraphicsBackend {
    Cpu,
    Wgpu,
    WebGpu,
    D3d12,
    Metal,
    Vulkan,
    OpenGl,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererInteropTier {
    CpuUpload,
    SharedTexture,
    SharedRenderTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererInteropCapabilities {
    pub backend: NativeGraphicsBackend,
    pub cpu_upload: bool,
    pub shared_texture: bool,
    pub shared_render_target: bool,
}

impl RendererInteropCapabilities {
    pub const fn cpu_only(backend: NativeGraphicsBackend) -> Self {
        Self {
            backend,
            cpu_upload: true,
            shared_texture: false,
            shared_render_target: false,
        }
    }

    pub const fn supports(self, tier: RendererInteropTier) -> bool {
        match tier {
            RendererInteropTier::CpuUpload => self.cpu_upload,
            RendererInteropTier::SharedTexture => self.shared_texture,
            RendererInteropTier::SharedRenderTarget => self.shared_render_target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalTextureFormat {
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba16Float,
}

impl ExternalTextureFormat {
    pub const fn bytes_per_pixel(self) -> Option<usize> {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm => Some(4),
            Self::Rgba16Float => Some(8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalBackendHandle {
    id: u64,
}

impl ExternalBackendHandle {
    pub const fn new(id: u64) -> Self {
        Self { id }
    }

    pub const fn id(self) -> u64 {
        self.id
    }

    pub const fn is_empty(self) -> bool {
        self.id == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalSync {
    None,
    Generation(u64),
    TimelineValue {
        handle: ExternalBackendHandle,
        value: u64,
    },
    Fence {
        handle: ExternalBackendHandle,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExternalTextureDescriptor {
    CpuRgba8 {
        size: Size,
        pixels: Arc<[u8]>,
        generation: u64,
    },
    SharedTexture {
        backend: NativeGraphicsBackend,
        size: Size,
        format: ExternalTextureFormat,
        color_space: ColorSpace,
        handle: ExternalBackendHandle,
        sync: ExternalSync,
    },
    SharedRenderTarget {
        backend: NativeGraphicsBackend,
        size: Size,
        format: ExternalTextureFormat,
        color_space: ColorSpace,
        handle: ExternalBackendHandle,
        sync: ExternalSync,
    },
}

impl ExternalTextureDescriptor {
    pub fn cpu_rgba8(size: Size, pixels: impl Into<Arc<[u8]>>, generation: u64) -> Self {
        Self::CpuRgba8 {
            size,
            pixels: pixels.into(),
            generation,
        }
    }

    pub fn size(&self) -> Size {
        match self {
            Self::CpuRgba8 { size, .. }
            | Self::SharedTexture { size, .. }
            | Self::SharedRenderTarget { size, .. } => *size,
        }
    }

    pub const fn tier(&self) -> RendererInteropTier {
        match self {
            Self::CpuRgba8 { .. } => RendererInteropTier::CpuUpload,
            Self::SharedTexture { .. } => RendererInteropTier::SharedTexture,
            Self::SharedRenderTarget { .. } => RendererInteropTier::SharedRenderTarget,
        }
    }

    pub fn validate(&self) -> Result<(), ExternalTextureValidationError> {
        validate_external_size(self.size())?;
        match self {
            Self::CpuRgba8 { size, pixels, .. } => {
                let expected = external_pixel_len(*size, 4)?;
                if pixels.len() != expected {
                    return Err(ExternalTextureValidationError::InvalidPixelLength {
                        expected,
                        actual: pixels.len(),
                    });
                }
            }
            Self::SharedTexture {
                handle,
                sync,
                format,
                ..
            }
            | Self::SharedRenderTarget {
                handle,
                sync,
                format,
                ..
            } => {
                if handle.is_empty() {
                    return Err(ExternalTextureValidationError::EmptyHandle);
                }
                if let ExternalSync::TimelineValue { handle, .. } | ExternalSync::Fence { handle } =
                    sync
                    && handle.is_empty()
                {
                    return Err(ExternalTextureValidationError::EmptySyncHandle);
                }
                if format.bytes_per_pixel().is_none() {
                    return Err(ExternalTextureValidationError::UnsupportedFormat);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalTextureValidationError {
    NonFiniteSize,
    NonPositiveSize,
    NonIntegerSize,
    SizeOverflow,
    InvalidPixelLength { expected: usize, actual: usize },
    EmptyHandle,
    EmptySyncHandle,
    UnsupportedFormat,
}

impl fmt::Display for ExternalTextureValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteSize => f.write_str("external texture size must be finite"),
            Self::NonPositiveSize => f.write_str("external texture size must be positive"),
            Self::NonIntegerSize => f.write_str("external texture size must use whole pixels"),
            Self::SizeOverflow => f.write_str("external texture byte length overflowed"),
            Self::InvalidPixelLength { expected, actual } => write!(
                f,
                "external CPU texture has {actual} bytes, expected {expected}"
            ),
            Self::EmptyHandle => f.write_str("external texture handle must be non-empty"),
            Self::EmptySyncHandle => f.write_str("external sync handle must be non-empty"),
            Self::UnsupportedFormat => f.write_str("external texture format is unsupported"),
        }
    }
}

impl std::error::Error for ExternalTextureValidationError {}

fn validate_external_size(size: Size) -> Result<(), ExternalTextureValidationError> {
    if !size.width.is_finite() || !size.height.is_finite() {
        return Err(ExternalTextureValidationError::NonFiniteSize);
    }
    if size.width <= 0.0 || size.height <= 0.0 {
        return Err(ExternalTextureValidationError::NonPositiveSize);
    }
    if size.width.fract() != 0.0 || size.height.fract() != 0.0 {
        return Err(ExternalTextureValidationError::NonIntegerSize);
    }
    if size.width > u32::MAX as f32 || size.height > u32::MAX as f32 {
        return Err(ExternalTextureValidationError::SizeOverflow);
    }
    Ok(())
}

fn external_pixel_len(
    size: Size,
    bytes_per_pixel: usize,
) -> Result<usize, ExternalTextureValidationError> {
    validate_external_size(size)?;
    let width = size.width as usize;
    let height = size.height as usize;
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or(ExternalTextureValidationError::SizeOverflow)
}

pub trait ForeignWidgetCallbacks: Send + Sync + 'static {
    fn debug_name(&self, _id: ForeignWidgetId) -> &'static str {
        "sui_bindings_core::ForeignWidget"
    }

    fn event(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignEventCtx<'_>,
        _event: &Event,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Ok(constraints.max)
    }

    fn arrange(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignArrangeCtx<'_>,
        _bounds: Rect,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn semantics(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignSemanticsCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }
}

pub struct ForeignWidget {
    id: ForeignWidgetId,
    callbacks: Arc<dyn ForeignWidgetCallbacks>,
    children: Vec<WidgetPod>,
    errors: ForeignErrorSink,
}

impl ForeignWidget {
    pub fn new(callbacks: impl ForeignWidgetCallbacks) -> Self {
        Self::from_arc(Arc::new(callbacks))
    }

    pub fn from_arc(callbacks: Arc<dyn ForeignWidgetCallbacks>) -> Self {
        Self {
            id: ForeignWidgetId::default(),
            callbacks,
            children: Vec::new(),
            errors: ForeignErrorSink::new(),
        }
    }

    pub fn with_id(mut self, id: ForeignWidgetId) -> Self {
        self.id = id;
        self
    }

    pub fn with_error_sink(mut self, errors: ForeignErrorSink) -> Self {
        self.errors = errors;
        self
    }

    pub fn with_child(mut self, child: impl Widget + 'static) -> Self {
        self.push_child(child);
        self
    }

    pub fn push_child(&mut self, child: impl Widget + 'static) {
        self.children.push(WidgetPod::new(child));
    }

    pub fn push_child_pod(&mut self, child: WidgetPod) {
        self.children.push(child);
    }

    pub const fn foreign_id(&self) -> ForeignWidgetId {
        self.id
    }

    pub fn error_sink(&self) -> ForeignErrorSink {
        self.errors.clone()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    fn record_panic(&self, phase: ForeignCallbackPhase, payload: Box<dyn std::any::Any + Send>) {
        self.errors.push(ForeignCallbackError::new(
            self.id,
            phase,
            panic_message(payload),
        ));
    }

    fn run_callback<T>(
        &self,
        phase: ForeignCallbackPhase,
        fallback: T,
        callback: impl FnOnce() -> ForeignCallbackResult<T>,
    ) -> T {
        run_foreign_callback(self.id, &self.errors, phase, fallback, callback)
    }
}

impl Widget for ForeignWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignEventCtx { inner: ctx };
        self.run_callback(ForeignCallbackPhase::Event, (), || {
            callbacks.event(self.id, &mut foreign_ctx, event)
        });
    }

    fn debug_name(&self) -> &'static str {
        let callbacks = Arc::clone(&self.callbacks);
        match catch_unwind(AssertUnwindSafe(|| callbacks.debug_name(self.id))) {
            Ok(name) => name,
            Err(payload) => {
                self.record_panic(ForeignCallbackPhase::DebugName, payload);
                "sui_bindings_core::ForeignWidget"
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let callbacks = Arc::clone(&self.callbacks);
        let id = self.id;
        let errors = self.errors.clone();
        let mut foreign_ctx = ForeignMeasureCtx {
            inner: ctx,
            children: &mut self.children,
        };
        let fallback = constraints.clamp(Size::ZERO);
        run_foreign_callback(id, &errors, ForeignCallbackPhase::Measure, fallback, || {
            callbacks.measure(id, &mut foreign_ctx, constraints)
        })
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let callbacks = Arc::clone(&self.callbacks);
        let id = self.id;
        let errors = self.errors.clone();
        let mut foreign_ctx = ForeignArrangeCtx {
            inner: ctx,
            children: &mut self.children,
        };
        run_foreign_callback(id, &errors, ForeignCallbackPhase::Arrange, (), || {
            callbacks.arrange(id, &mut foreign_ctx, bounds)
        });
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignPaintCtx {
            inner: ctx,
            children: &self.children,
        };
        self.run_callback(ForeignCallbackPhase::Paint, (), || {
            callbacks.paint(self.id, &mut foreign_ctx)
        });
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignSemanticsCtx {
            inner: ctx,
            children: &self.children,
        };
        self.run_callback(ForeignCallbackPhase::Semantics, (), || {
            callbacks.semantics(self.id, &mut foreign_ctx)
        });
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for child in &self.children {
            visitor.visit(child);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for child in &mut self.children {
            visitor.visit(child);
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingEventContext {
    pub window_id: u64,
    pub widget_id: u64,
    pub bounds: Rect,
    pub current_time: f64,
    pub phase: &'static str,
    pub focused: bool,
    pub clipboard_text: Option<String>,
    handled: bool,
    focus_request: BindingFocusRequest,
    request_measure: bool,
    request_arrange: bool,
    request_paint: bool,
    request_paint_rect: Option<Rect>,
    request_semantics: bool,
    request_animation_frame: bool,
    capture_pointers: Vec<u64>,
    release_pointers: Vec<u64>,
    next_clipboard_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
enum BindingFocusRequest {
    #[default]
    None,
    Focus,
    Clear,
}

impl BindingEventContext {
    pub fn from_foreign(ctx: &ForeignEventCtx<'_>) -> Self {
        Self {
            window_id: ctx.window_id().get(),
            widget_id: ctx.widget_id().get(),
            bounds: ctx.bounds(),
            current_time: ctx.current_time(),
            phase: match ctx.phase() {
                EventPhase::Capture => "capture",
                EventPhase::Target => "target",
                EventPhase::Bubble => "bubble",
            },
            focused: ctx.is_focused(),
            clipboard_text: ctx.clipboard_text(),
            handled: false,
            focus_request: BindingFocusRequest::None,
            request_measure: false,
            request_arrange: false,
            request_paint: false,
            request_paint_rect: None,
            request_semantics: false,
            request_animation_frame: false,
            capture_pointers: Vec::new(),
            release_pointers: Vec::new(),
            next_clipboard_text: None,
        }
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    pub fn request_focus(&mut self) {
        self.focus_request = BindingFocusRequest::Focus;
    }

    pub fn clear_focus(&mut self) {
        self.focus_request = BindingFocusRequest::Clear;
    }

    pub fn request_measure(&mut self) {
        self.request_measure = true;
    }

    pub fn request_arrange(&mut self) {
        self.request_arrange = true;
    }

    pub fn request_paint(&mut self) {
        self.request_paint = true;
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request_paint_rect = Some(rect);
    }

    pub fn request_semantics(&mut self) {
        self.request_semantics = true;
    }

    pub fn request_animation_frame(&mut self) {
        self.request_animation_frame = true;
    }

    pub fn capture_pointer(&mut self, pointer_id: u64) {
        self.capture_pointers.push(pointer_id);
    }

    pub fn release_pointer(&mut self, pointer_id: u64) {
        self.release_pointers.push(pointer_id);
    }

    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.next_clipboard_text = Some(text.into());
    }

    pub fn apply(&self, ctx: &mut ForeignEventCtx<'_>) {
        if self.handled {
            ctx.set_handled();
        }
        match self.focus_request {
            BindingFocusRequest::None => {}
            BindingFocusRequest::Focus => ctx.request_focus(),
            BindingFocusRequest::Clear => ctx.clear_focus(),
        }
        if self.request_measure {
            ctx.request_measure();
        }
        if self.request_arrange {
            ctx.request_arrange();
        }
        if self.request_paint {
            ctx.request_paint();
        }
        if let Some(rect) = self.request_paint_rect {
            ctx.request_paint_rect(rect);
        }
        if self.request_semantics {
            ctx.request_semantics();
        }
        if self.request_animation_frame {
            ctx.request_animation_frame();
        }
        for pointer_id in &self.capture_pointers {
            ctx.request_pointer_capture(*pointer_id);
        }
        for pointer_id in &self.release_pointers {
            ctx.release_pointer_capture(*pointer_id);
        }
        if let Some(text) = &self.next_clipboard_text {
            ctx.set_clipboard_text(text);
        }
    }
}

pub struct ForeignEventCtx<'a> {
    inner: &'a mut EventCtx,
}

impl ForeignEventCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn current_time(&self) -> f64 {
        self.inner.current_time()
    }

    pub fn phase(&self) -> EventPhase {
        self.inner.phase()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.inner.clipboard_text()
    }

    pub fn set_clipboard_text(&mut self, text: impl AsRef<str>) {
        self.inner.set_clipboard_text(text);
    }

    pub fn set_handled(&mut self) {
        self.inner.set_handled();
    }

    pub fn request_focus(&mut self) {
        self.inner.request_focus();
    }

    pub fn request_focus_for(&mut self, widget_id: WidgetId) {
        self.inner.request_focus_for(widget_id);
    }

    pub fn clear_focus(&mut self) {
        self.inner.clear_focus();
    }

    pub fn request_measure(&mut self) {
        self.inner.request_measure();
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.inner.request_paint_rect(rect);
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }

    pub fn request_animation_frame(&mut self) {
        self.inner.request_animation_frame();
    }

    pub fn request_pointer_capture(&mut self, pointer_id: u64) {
        self.inner.request_pointer_capture(pointer_id);
    }

    pub fn release_pointer_capture(&mut self, pointer_id: u64) {
        self.inner.release_pointer_capture(pointer_id);
    }

    pub fn schedule_timer_after(&mut self, delay: f64) -> TimerToken {
        self.inner.schedule_timer_after(delay)
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.inner.request(request);
    }
}

pub struct ForeignMeasureCtx<'a> {
    inner: &'a mut MeasureCtx,
    children: &'a mut [WidgetPod],
}

impl ForeignMeasureCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn current_time(&self) -> f64 {
        self.inner.current_time()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Size> {
        self.children
            .get_mut(index)
            .map(|child| child.measure(self.inner, constraints))
    }

    pub fn request_measure(&mut self) {
        self.inner.request_measure();
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }
}

pub struct ForeignArrangeCtx<'a> {
    inner: &'a mut ArrangeCtx,
    children: &'a mut [WidgetPod],
}

impl ForeignArrangeCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn arrange_child(&mut self, index: usize, bounds: Rect) -> bool {
        let Some(child) = self.children.get_mut(index) else {
            return false;
        };
        child.arrange(self.inner, bounds);
        true
    }

    pub fn set_child_bounds(&mut self, index: usize, bounds: Rect) -> bool {
        let Some(child) = self.children.get_mut(index) else {
            return false;
        };
        child.set_bounds(bounds);
        true
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }
}

pub struct ForeignPaintCtx<'a> {
    inner: &'a mut PaintCtx,
    children: &'a [WidgetPod],
}

impl ForeignPaintCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn paint_child(&mut self, index: usize) -> bool {
        let Some(child) = self.children.get(index) else {
            return false;
        };
        child.paint(self.inner);
        true
    }

    pub fn apply(&mut self, command: PaintCommand) -> PaintValidationResult<()> {
        validate_paint_command(&command)?;
        command.apply(self.inner);
        Ok(())
    }

    pub fn apply_all(
        &mut self,
        commands: impl IntoIterator<Item = PaintCommand>,
    ) -> PaintValidationResult<()> {
        let mut stack = PaintStackState::default();
        let commands = commands.into_iter().collect::<Vec<_>>();
        for command in &commands {
            validate_paint_command_with_stack(command, &mut stack)?;
        }
        stack.finish()?;
        for command in commands {
            command.apply(self.inner);
        }
        Ok(())
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) {
        self.inner.register_image(handle, image);
    }

    pub fn widget_image_handle(&self, slot: u64) -> ImageHandle {
        self.inner.widget_image_handle(slot)
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.inner.request_paint_rect(rect);
    }
}

pub struct ForeignSemanticsCtx<'a> {
    inner: &'a mut SemanticsCtx,
    children: &'a [WidgetPod],
}

impl ForeignSemanticsCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.inner.push(node);
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn semantics_child(&mut self, index: usize) -> bool {
        let Some(child) = self.children.get(index) else {
            return false;
        };
        child.semantics(self.inner);
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        brush: Brush,
    },
    StrokeRect {
        rect: Rect,
        brush: Brush,
        stroke: StrokeStyle,
    },
    FillPath {
        path: Path,
        brush: Brush,
    },
    StrokePath {
        path: Path,
        brush: Brush,
        stroke: StrokeStyle,
    },
    FillRoundedRect {
        rect: Rect,
        radii: [f32; 4],
        brush: Brush,
        border: Option<Border>,
        shadow: Option<ShadowParams>,
    },
    DrawText {
        rect: Rect,
        text: String,
        style: TextStyle,
    },
    DrawImage {
        rect: Rect,
        source: ImageSource,
    },
    DrawImageQuad {
        points: [Point; 4],
        source: ImageSource,
    },
    DrawShaderRect {
        rect: Rect,
        shader: WidgetShader,
    },
    PushClipRect(Rect),
    PushClipPath(Path),
    PopClip,
    PushTransform(Transform),
    PopTransform,
}

impl PaintCommand {
    pub fn apply(self, ctx: &mut PaintCtx) {
        match self {
            Self::Clear(color) => ctx.clear(color),
            Self::FillRect { rect, brush } => ctx.fill_rect(rect, brush),
            Self::StrokeRect {
                rect,
                brush,
                stroke,
            } => ctx.stroke_rect(rect, brush, stroke),
            Self::FillPath { path, brush } => ctx.fill(path, brush),
            Self::StrokePath {
                path,
                brush,
                stroke,
            } => ctx.stroke(path, brush, stroke),
            Self::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            } => ctx.push(sui::SceneCommand::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            }),
            Self::DrawText { rect, text, style } => ctx.draw_text(rect, text, style),
            Self::DrawImage { rect, source } => ctx.draw_image_source(rect, source),
            Self::DrawImageQuad { points, source } => ctx.draw_image_quad_source(points, source),
            Self::DrawShaderRect { rect, shader } => ctx.draw_shader_rect(rect, shader),
            Self::PushClipRect(rect) => ctx.push_clip_rect(rect),
            Self::PushClipPath(path) => ctx.push_clip(path),
            Self::PopClip => ctx.pop_clip(),
            Self::PushTransform(transform) => ctx.push_transform(transform),
            Self::PopTransform => ctx.pop_transform(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaintCommandBuilder {
    commands: Vec<PaintCommand>,
    stack: PaintStackState,
}

impl PaintCommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: PaintCommand) -> PaintValidationResult<&mut Self> {
        validate_paint_command_with_stack(&command, &mut self.stack)?;
        self.commands.push(command);
        Ok(self)
    }

    pub fn clear(&mut self, color: Color) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::Clear(color))
    }

    pub fn fill_rect(
        &mut self,
        rect: Rect,
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRect {
            rect,
            brush: brush.into(),
        })
    }

    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        brush: impl Into<Brush>,
        stroke: StrokeStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::StrokeRect {
            rect,
            brush: brush.into(),
            stroke,
        })
    }

    pub fn fill_path(
        &mut self,
        path: Path,
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillPath {
            path,
            brush: brush.into(),
        })
    }

    pub fn stroke_path(
        &mut self,
        path: Path,
        brush: impl Into<Brush>,
        stroke: StrokeStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::StrokePath {
            path,
            brush: brush.into(),
            stroke,
        })
    }

    pub fn fill_rrect(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: None,
        })
    }

    pub fn draw_shadow(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        shadow: ShadowParams,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: Brush::Solid(Color::TRANSPARENT),
            border: None,
            shadow: Some(shadow),
        })
    }

    pub fn fill_rrect_with_shadow(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
        shadow: ShadowParams,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: Some(shadow),
        })
    }

    pub fn draw_text(
        &mut self,
        rect: Rect,
        text: impl Into<String>,
        style: TextStyle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawText {
            rect,
            text: text.into(),
            style,
        })
    }

    pub fn draw_image(
        &mut self,
        rect: Rect,
        image: ImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImage {
            rect,
            source: ImageSource::new(image),
        })
    }

    pub fn draw_binding_image(
        &mut self,
        rect: Rect,
        image: BindingImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_image(rect, image.into_sui())
    }

    pub fn draw_image_source(
        &mut self,
        rect: Rect,
        source: ImageSource,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImage { rect, source })
    }

    pub fn draw_image_quad(
        &mut self,
        points: [Point; 4],
        image: ImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawImageQuad {
            points,
            source: ImageSource::new(image),
        })
    }

    pub fn draw_binding_image_quad(
        &mut self,
        points: [Point; 4],
        image: BindingImageHandle,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_image_quad(points, image.into_sui())
    }

    pub fn draw_shader_rect(
        &mut self,
        rect: Rect,
        shader: WidgetShader,
    ) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::DrawShaderRect { rect, shader })
    }

    pub fn draw_binding_shader_rect(
        &mut self,
        rect: Rect,
        shader: BindingShader,
    ) -> PaintValidationResult<&mut Self> {
        self.draw_shader_rect(rect, shader.widget_shader())
    }

    pub fn push_clip_rect(&mut self, rect: Rect) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushClipRect(rect))
    }

    pub fn push_clip_path(&mut self, path: Path) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushClipPath(path))
    }

    pub fn pop_clip(&mut self) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PopClip)
    }

    pub fn push_transform(&mut self, transform: Transform) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PushTransform(transform))
    }

    pub fn pop_transform(&mut self) -> PaintValidationResult<&mut Self> {
        self.push(PaintCommand::PopTransform)
    }

    pub fn finish(self) -> PaintValidationResult<Vec<PaintCommand>> {
        self.stack.finish()?;
        Ok(self.commands)
    }
}

pub type PaintValidationResult<T> = std::result::Result<T, PaintValidationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintValidationErrorKind {
    NonFiniteGeometry,
    NegativeSize,
    PathTooComplex,
    InvalidStroke,
    InvalidBrush,
    InvalidShader,
    InvalidImage,
    InvalidTextStyle,
    InvalidStackOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintValidationError {
    pub kind: PaintValidationErrorKind,
    pub message: String,
}

impl PaintValidationError {
    fn new(kind: PaintValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for PaintValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PaintValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct PaintStackState {
    clip_depth: usize,
    transform_depth: usize,
}

impl PaintStackState {
    fn finish(self) -> PaintValidationResult<()> {
        if self.clip_depth != 0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::InvalidStackOperation,
                format!(
                    "paint command stream has {} unclosed clip scope(s)",
                    self.clip_depth
                ),
            ));
        }
        if self.transform_depth != 0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::InvalidStackOperation,
                format!(
                    "paint command stream has {} unclosed transform scope(s)",
                    self.transform_depth
                ),
            ));
        }
        Ok(())
    }
}

const MAX_BINDING_PATH_ELEMENTS: usize = 4096;
const MAX_BINDING_GRADIENT_STOPS: usize = 8;

fn validate_paint_command(command: &PaintCommand) -> PaintValidationResult<()> {
    validate_paint_command_with_stack(command, &mut PaintStackState::default())
}

fn validate_paint_command_with_stack(
    command: &PaintCommand,
    stack: &mut PaintStackState,
) -> PaintValidationResult<()> {
    match command {
        PaintCommand::Clear(color) => validate_color(*color),
        PaintCommand::FillRect { rect, brush } => {
            validate_rect(*rect)?;
            validate_brush(brush)
        }
        PaintCommand::StrokeRect {
            rect,
            brush,
            stroke,
        } => {
            validate_rect(*rect)?;
            validate_stroke(*stroke)?;
            validate_brush(brush)
        }
        PaintCommand::FillPath { path, brush } => {
            validate_path(path)?;
            validate_brush(brush)
        }
        PaintCommand::StrokePath {
            path,
            brush,
            stroke,
        } => {
            validate_path(path)?;
            validate_stroke(*stroke)?;
            validate_brush(brush)
        }
        PaintCommand::FillRoundedRect {
            rect,
            radii,
            brush,
            border,
            shadow,
        } => {
            validate_rect(*rect)?;
            validate_radii(*radii)?;
            validate_brush(brush)?;
            if let Some(border) = border {
                validate_border(*border)?;
            }
            if let Some(shadow) = shadow {
                validate_shadow(*shadow)?;
            }
            Ok(())
        }
        PaintCommand::DrawText { rect, style, .. } => {
            validate_rect(*rect)?;
            validate_text_style(style)
        }
        PaintCommand::DrawImage { rect, source } => {
            validate_rect(*rect)?;
            validate_image_source(source)
        }
        PaintCommand::DrawImageQuad { points, source } => {
            for point in points {
                validate_point(*point)?;
            }
            validate_image_source(source)
        }
        PaintCommand::DrawShaderRect { rect, shader } => {
            validate_rect(*rect)?;
            validate_widget_shader(*shader)
        }
        PaintCommand::PushClipRect(rect) => {
            validate_rect(*rect)?;
            stack.clip_depth += 1;
            Ok(())
        }
        PaintCommand::PushClipPath(path) => {
            validate_path(path)?;
            stack.clip_depth += 1;
            Ok(())
        }
        PaintCommand::PopClip => {
            if stack.clip_depth == 0 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidStackOperation,
                    "paint command stream popped a clip without a matching push",
                ));
            }
            stack.clip_depth -= 1;
            Ok(())
        }
        PaintCommand::PushTransform(transform) => {
            validate_transform(*transform)?;
            stack.transform_depth += 1;
            Ok(())
        }
        PaintCommand::PopTransform => {
            if stack.transform_depth == 0 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidStackOperation,
                    "paint command stream popped a transform without a matching push",
                ));
            }
            stack.transform_depth -= 1;
            Ok(())
        }
    }
}

fn validate_point(point: Point) -> PaintValidationResult<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "point contains a non-finite coordinate",
        ));
    }
    Ok(())
}

fn validate_rect(rect: Rect) -> PaintValidationResult<()> {
    validate_point(rect.origin)?;
    if !rect.size.width.is_finite() || !rect.size.height.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "rect contains a non-finite size",
        ));
    }
    if rect.size.width < 0.0 || rect.size.height < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NegativeSize,
            "rect size must be non-negative",
        ));
    }
    Ok(())
}

fn validate_color(color: Color) -> PaintValidationResult<()> {
    if color
        .to_array()
        .into_iter()
        .any(|channel| !channel.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidBrush,
            "color contains a non-finite channel",
        ));
    }
    Ok(())
}

fn validate_image_source(source: &ImageSource) -> PaintValidationResult<()> {
    if source.image.get() == 0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidImage,
            "image handle must be non-zero",
        ));
    }
    if let Some(source_rect) = source.source_rect {
        validate_rect(source_rect)?;
    }
    if let Some(tint) = source.tint {
        validate_color(tint)?;
    }
    Ok(())
}

fn validate_text_style(style: &TextStyle) -> PaintValidationResult<()> {
    validate_color(style.color)?;
    if !style.font_size.is_finite() || style.font_size <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text font size must be finite and positive",
        ));
    }
    if !style.line_height.is_finite() || style.line_height <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text line height must be finite and positive",
        ));
    }
    if let Some(font) = style.font
        && font.get() == 0
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidTextStyle,
            "text font handle must be non-zero",
        ));
    }
    Ok(())
}

fn validate_widget_shader(shader: WidgetShader) -> PaintValidationResult<()> {
    match shader {
        WidgetShader::ColorWheel | WidgetShader::ColorPickerHueBar => Ok(()),
        WidgetShader::ColorPickerSaturationValuePlane { hue, max_value, .. } => {
            validate_shader_param("hue", hue)?;
            validate_positive_shader_param("max_value", max_value)
        }
        WidgetShader::ColorPickerSaturationBar { hue, value, .. } => {
            validate_shader_param("hue", hue)?;
            validate_shader_param("value", value)
        }
        WidgetShader::ColorPickerValueBar {
            hue,
            saturation,
            max_value,
            ..
        } => {
            validate_shader_param("hue", hue)?;
            validate_shader_param("saturation", saturation)?;
            validate_positive_shader_param("max_value", max_value)
        }
        WidgetShader::ColorPickerAlphaBar { color } => validate_shader_color(color),
        WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        } => {
            validate_shader_color(color)?;
            if channel > 2 {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidShader,
                    "rgb channel shader channel must be 0, 1, or 2",
                ));
            }
            validate_positive_shader_param("max_value", max_value)
        }
    }
}

fn validate_shader_color(color: Color) -> PaintValidationResult<()> {
    if color
        .to_array()
        .into_iter()
        .any(|channel| !channel.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            "shader color contains a non-finite channel",
        ));
    }
    Ok(())
}

fn validate_shader_param(name: &str, value: f32) -> PaintValidationResult<()> {
    if !value.is_finite() {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            format!("shader parameter `{name}` must be finite"),
        ));
    }
    Ok(())
}

fn validate_positive_shader_param(name: &str, value: f32) -> PaintValidationResult<()> {
    validate_shader_param(name, value)?;
    if value <= 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidShader,
            format!("shader parameter `{name}` must be positive"),
        ));
    }
    Ok(())
}

fn validate_brush(brush: &Brush) -> PaintValidationResult<()> {
    match brush {
        Brush::Solid(color) => validate_color(*color),
        Brush::LinearGradient { start, end, stops } => {
            validate_point(*start)?;
            validate_point(*end)?;
            if stops.len() > MAX_BINDING_GRADIENT_STOPS {
                return Err(PaintValidationError::new(
                    PaintValidationErrorKind::InvalidBrush,
                    format!(
                        "linear gradient has {} stops but the binding limit is {}",
                        stops.len(),
                        MAX_BINDING_GRADIENT_STOPS
                    ),
                ));
            }
            for stop in stops {
                if !stop.offset.is_finite() {
                    return Err(PaintValidationError::new(
                        PaintValidationErrorKind::InvalidBrush,
                        "linear gradient stop offset must be finite",
                    ));
                }
                validate_color(stop.color)?;
            }
            Ok(())
        }
    }
}

fn validate_path(path: &Path) -> PaintValidationResult<()> {
    if path.elements().len() > MAX_BINDING_PATH_ELEMENTS {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::PathTooComplex,
            format!(
                "path has {} elements but the binding limit is {}",
                path.elements().len(),
                MAX_BINDING_PATH_ELEMENTS
            ),
        ));
    }
    validate_rect(path.bounds())
}

fn validate_stroke(stroke: StrokeStyle) -> PaintValidationResult<()> {
    if !stroke.width.is_finite() || stroke.width < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidStroke,
            "stroke width must be finite and non-negative",
        ));
    }
    Ok(())
}

fn validate_radii(radii: [f32; 4]) -> PaintValidationResult<()> {
    for radius in radii {
        if !radius.is_finite() || radius < 0.0 {
            return Err(PaintValidationError::new(
                PaintValidationErrorKind::NonFiniteGeometry,
                "rounded-rect radii must be finite and non-negative",
            ));
        }
    }
    Ok(())
}

fn validate_border(border: Border) -> PaintValidationResult<()> {
    if !border.width.is_finite() || border.width < 0.0 {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::InvalidStroke,
            "border width must be finite and non-negative",
        ));
    }
    validate_color(border.color)
}

fn validate_shadow(shadow: ShadowParams) -> PaintValidationResult<()> {
    if [shadow.offset_x, shadow.offset_y, shadow.blur, shadow.spread]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "shadow geometry must be finite",
        ));
    }
    validate_color(shadow.color)
}

fn validate_transform(transform: Transform) -> PaintValidationResult<()> {
    if [
        transform.xx,
        transform.yx,
        transform.xy,
        transform.yy,
        transform.dx,
        transform.dy,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
    {
        return Err(PaintValidationError::new(
            PaintValidationErrorKind::NonFiniteGeometry,
            "transform contains a non-finite component",
        ));
    }
    Ok(())
}

fn run_foreign_callback<T>(
    id: ForeignWidgetId,
    errors: &ForeignErrorSink,
    phase: ForeignCallbackPhase,
    fallback: T,
    callback: impl FnOnce() -> ForeignCallbackResult<T>,
) -> T {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            errors.push(ForeignCallbackError::new(id, phase, error.message));
            fallback
        }
        Err(payload) => {
            errors.push(ForeignCallbackError::new(id, phase, panic_message(payload)));
            fallback
        }
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return format!("foreign callback panicked: {message}");
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return format!("foreign callback panicked: {message}");
    }
    "foreign callback panicked with a non-string payload".to_string()
}

fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn widget_invalidation(widget_id: WidgetId, kind: InvalidationKind) -> InvalidationRequest {
    InvalidationRequest::new(InvalidationTarget::Widget(widget_id), kind)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use super::*;
    use sui::{RuntimeApplication, SceneCommand, SemanticsRole, WindowBuilder};

    #[test]
    fn binding_menu_item_preserves_recursive_submenus() {
        let item = BindingMenuItem::new(
            "Move to",
            None,
            false,
            false,
            false,
            vec![BindingMenuItem::new(
                "Shared",
                None,
                false,
                false,
                false,
                vec![BindingMenuItem::new(
                    "Team workspace",
                    None,
                    false,
                    false,
                    false,
                    Vec::new(),
                )],
            )],
        )
        .into_sui();

        assert_eq!(item.submenu_items()[0].label(), "Shared");
        assert_eq!(
            item.submenu_items()[0].submenu_items()[0].label(),
            "Team workspace"
        );
    }

    #[derive(Default)]
    struct MockCallbacks {
        events: AtomicUsize,
        measures: AtomicUsize,
        paints: AtomicUsize,
    }

    impl ForeignWidgetCallbacks for MockCallbacks {
        fn debug_name(&self, _id: ForeignWidgetId) -> &'static str {
            "MockForeignWidget"
        }

        fn event(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignEventCtx<'_>,
            _event: &Event,
        ) -> ForeignCallbackResult<()> {
            self.events.fetch_add(1, Ordering::Relaxed);
            ctx.request_paint();
            ctx.set_handled();
            Ok(())
        }

        fn measure(
            &self,
            _id: ForeignWidgetId,
            _ctx: &mut ForeignMeasureCtx<'_>,
            constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            self.measures.fetch_add(1, Ordering::Relaxed);
            Ok(constraints.clamp(Size::new(80.0, 24.0)))
        }

        fn paint(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            self.paints.fetch_add(1, Ordering::Relaxed);
            let mut builder = PaintCommandBuilder::new();
            builder
                .fill_rect(ctx.bounds(), Color::rgba(0.2, 0.3, 0.4, 1.0))
                .unwrap();
            ctx.apply_all(builder.finish().unwrap())?;
            Ok(())
        }

        fn semantics(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignSemanticsCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
            node.name = Some("Foreign canvas".to_string());
            node.state.disabled = true;
            node.state.hidden = true;
            node.state.hovered = true;
            node.state.selected = true;
            node.state.expanded = Some(true);
            ctx.push(node);
            Ok(())
        }
    }

    fn test_png_rgba(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        encoded
    }

    struct AppImageCallbacks {
        image: BindingImageHandle,
    }

    impl ForeignWidgetCallbacks for AppImageCallbacks {
        fn measure(
            &self,
            _id: ForeignWidgetId,
            _ctx: &mut ForeignMeasureCtx<'_>,
            constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            Ok(constraints.clamp(Size::new(32.0, 16.0)))
        }

        fn paint(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            let mut builder = PaintCommandBuilder::new();
            builder.draw_binding_image(ctx.bounds(), self.image)?;
            ctx.apply_all(builder.finish()?)?;
            Ok(())
        }
    }

    struct ChildCallbacks;

    impl ForeignWidgetCallbacks for ChildCallbacks {
        fn measure(
            &self,
            _id: ForeignWidgetId,
            _ctx: &mut ForeignMeasureCtx<'_>,
            constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            Ok(constraints.clamp(Size::new(40.0, 12.0)))
        }

        fn paint(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            ctx.apply(PaintCommand::FillRect {
                rect: ctx.bounds(),
                brush: Brush::Solid(Color::WHITE),
            })?;
            Ok(())
        }
    }

    struct ContainerCallbacks;

    impl ForeignWidgetCallbacks for ContainerCallbacks {
        fn measure(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignMeasureCtx<'_>,
            constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            let child = ctx
                .measure_child(0, constraints.loosen())
                .expect("child should be present");
            Ok(constraints.clamp(Size::new(child.width + 4.0, child.height + 4.0)))
        }

        fn arrange(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignArrangeCtx<'_>,
            bounds: Rect,
        ) -> ForeignCallbackResult<()> {
            assert!(
                ctx.arrange_child(0, Rect::new(bounds.x() + 2.0, bounds.y() + 2.0, 40.0, 12.0))
            );
            Ok(())
        }

        fn paint(
            &self,
            _id: ForeignWidgetId,
            ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            assert!(ctx.paint_child(0));
            Ok(())
        }
    }

    struct FailingCallbacks;

    impl ForeignWidgetCallbacks for FailingCallbacks {
        fn measure(
            &self,
            _id: ForeignWidgetId,
            _ctx: &mut ForeignMeasureCtx<'_>,
            _constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            Err(ForeignCallbackFailure::new("measure failed"))
        }

        fn paint(
            &self,
            _id: ForeignWidgetId,
            _ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            panic!("paint failed")
        }
    }

    #[test]
    fn ui_task_queue_posts_wakes_and_drains_tasks() {
        let woke = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let queue = UiTaskQueue::with_waker({
            let woke = Arc::clone(&woke);
            move || {
                woke.store(true, Ordering::Relaxed);
            }
        });
        let handle = queue.handle();

        handle.post({
            let completed = Arc::clone(&completed);
            move || {
                completed.store(true, Ordering::Relaxed);
            }
        });

        assert!(woke.load(Ordering::Relaxed));
        assert_eq!(queue.pending_count(), 1);
        assert_eq!(queue.drain(), 1);
        assert!(completed.load(Ordering::Relaxed));
        assert!(queue.is_empty());
    }

    #[test]
    fn paint_command_builder_validates_stack_balance_and_geometry() {
        let mut builder = PaintCommandBuilder::new();
        builder
            .push_clip_rect(Rect::new(0.0, 0.0, 10.0, 10.0))
            .unwrap()
            .fill_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::WHITE)
            .unwrap();

        let error = builder.finish().unwrap_err();
        assert_eq!(error.kind, PaintValidationErrorKind::InvalidStackOperation);

        let mut invalid = PaintCommandBuilder::new();
        let error = invalid
            .fill_rect(Rect::new(0.0, 0.0, f32::NAN, 1.0), Color::WHITE)
            .unwrap_err();
        assert_eq!(error.kind, PaintValidationErrorKind::NonFiniteGeometry);
    }

    #[test]
    fn paint_command_builder_validates_shader_commands() {
        let shader = BindingShader::saturation_value_plane(ColorSpace::Srgb, 0.25, 1.0).unwrap();
        let mut builder = PaintCommandBuilder::new();
        builder
            .draw_binding_shader_rect(Rect::new(0.0, 0.0, 20.0, 10.0), shader)
            .unwrap();

        assert!(matches!(
            builder.finish().unwrap().as_slice(),
            [PaintCommand::DrawShaderRect { .. }]
        ));

        let invalid_max =
            BindingShader::saturation_value_plane(ColorSpace::Srgb, 0.25, 0.0).unwrap_err();
        assert_eq!(invalid_max.kind, PaintValidationErrorKind::InvalidShader);

        let invalid_channel = BindingShader::rgb_channel_bar(Color::WHITE, 4, 1.0).unwrap_err();
        assert_eq!(
            invalid_channel.kind,
            PaintValidationErrorKind::InvalidShader
        );

        let mut invalid_builder = PaintCommandBuilder::new();
        let invalid_hue = invalid_builder
            .draw_shader_rect(
                Rect::new(0.0, 0.0, 20.0, 10.0),
                WidgetShader::ColorPickerSaturationBar {
                    color_space: ColorSpace::Srgb,
                    hue: f32::NAN,
                    value: 1.0,
                },
            )
            .unwrap_err();
        assert_eq!(invalid_hue.kind, PaintValidationErrorKind::InvalidShader);
    }

    #[test]
    fn paint_command_builder_validates_and_resolves_image_commands() {
        let local = BindingImageHandle::local(7);
        assert_eq!(local.local_slot(), Some(7));

        let mut builder = PaintCommandBuilder::new();
        builder
            .draw_binding_image(Rect::new(0.0, 0.0, 20.0, 10.0), local)
            .unwrap();
        let mut commands = builder.finish().unwrap();

        resolve_binding_image_slots(&mut commands, |slot| {
            assert_eq!(slot, 7);
            ImageHandle::new(99)
        });

        assert!(matches!(
            commands.as_slice(),
            [PaintCommand::DrawImage { source, .. }] if source.image == ImageHandle::new(99)
        ));

        let mut invalid_builder = PaintCommandBuilder::new();
        let error = invalid_builder
            .draw_image(Rect::new(0.0, 0.0, 20.0, 10.0), ImageHandle::new(0))
            .unwrap_err();
        assert_eq!(error.kind, PaintValidationErrorKind::InvalidImage);
    }

    #[test]
    fn paint_command_builder_records_rich_low_level_commands() {
        let path = Path::circle(Point::new(8.0, 8.0), 4.0);
        let local = BindingImageHandle::local(3);
        let shadow = ShadowParams {
            offset_x: 1.0,
            offset_y: 2.0,
            blur: 3.0,
            spread: 0.5,
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
        };

        let mut builder = PaintCommandBuilder::new();
        builder
            .push_clip_path(path.clone())
            .unwrap()
            .push_transform(Transform::translation(2.0, 3.0))
            .unwrap()
            .fill_path(path.clone(), Color::WHITE)
            .unwrap()
            .stroke_path(path, Color::BLACK, StrokeStyle::new(1.5))
            .unwrap()
            .draw_shadow(Rect::new(0.0, 0.0, 20.0, 12.0), [4.0; 4], shadow)
            .unwrap()
            .fill_rrect_with_shadow(
                Rect::new(2.0, 2.0, 16.0, 8.0),
                [3.0; 4],
                Color::rgba(0.2, 0.4, 0.8, 1.0),
                shadow,
            )
            .unwrap()
            .draw_binding_image_quad(
                [
                    Point::new(0.0, 0.0),
                    Point::new(16.0, 0.0),
                    Point::new(16.0, 16.0),
                    Point::new(0.0, 16.0),
                ],
                local,
            )
            .unwrap()
            .pop_transform()
            .unwrap()
            .pop_clip()
            .unwrap();
        let mut commands = builder.finish().unwrap();

        resolve_binding_image_slots(&mut commands, |slot| {
            assert_eq!(slot, 3);
            ImageHandle::new(42)
        });

        assert!(matches!(
            commands.as_slice(),
            [
                PaintCommand::PushClipPath(_),
                PaintCommand::PushTransform(_),
                PaintCommand::FillPath { .. },
                PaintCommand::StrokePath { .. },
                PaintCommand::FillRoundedRect { shadow: Some(_), .. },
                PaintCommand::FillRoundedRect { shadow: Some(_), .. },
                PaintCommand::DrawImageQuad { source, .. },
                PaintCommand::PopTransform,
                PaintCommand::PopClip,
            ] if source.image == ImageHandle::new(42)
        ));
    }

    #[test]
    fn paint_command_builder_validates_text_style() {
        let mut style = TextStyle::new(Color::WHITE);
        style.font_size = f32::NAN;
        let mut builder = PaintCommandBuilder::new();
        let error = builder
            .draw_text(Rect::new(0.0, 0.0, 100.0, 20.0), "Bad text", style)
            .unwrap_err();
        assert_eq!(error.kind, PaintValidationErrorKind::InvalidTextStyle);

        let mut style = TextStyle::new(Color::WHITE);
        style.font = Some(FontHandle::new(0));
        let error = PaintCommandBuilder::new()
            .draw_text(Rect::new(0.0, 0.0, 100.0, 20.0), "Bad font", style)
            .unwrap_err();
        assert_eq!(error.kind, PaintValidationErrorKind::InvalidTextStyle);
    }

    #[test]
    fn foreign_widget_adapter_renders_and_records_semantics() {
        let callbacks = Arc::new(MockCallbacks::default());
        let widget = ForeignWidget::from_arc(callbacks.clone());
        let mut runtime = RuntimeApplication::new()
            .window(WindowBuilder::new().title("Foreign").root(widget))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];

        let output = runtime.render(window_id).unwrap();

        assert_eq!(callbacks.measures.load(Ordering::Relaxed), 1);
        assert_eq!(callbacks.paints.load(Ordering::Relaxed), 1);
        assert!(
            output
                .frame
                .scene
                .commands()
                .iter()
                .any(|command| matches!(command, SceneCommand::FillRect { .. }))
        );
        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Canvas
                    && node.name.as_deref() == Some("Foreign canvas"))
        );

        let mut pointer =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(8.0, 8.0));
        pointer.button = Some(BindingPointerButton::Primary);
        pointer.buttons = 1;
        runtime
            .handle_event(
                window_id,
                BindingEvent::Pointer(pointer).into_sui_event().unwrap(),
            )
            .unwrap();
        assert_eq!(callbacks.events.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn foreign_widget_can_measure_arrange_and_paint_retained_children() {
        let widget =
            ForeignWidget::new(ContainerCallbacks).with_child(ForeignWidget::new(ChildCallbacks));
        let mut runtime = RuntimeApplication::new()
            .window(WindowBuilder::new().title("Children").root(widget))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];

        let output = runtime.render(window_id).unwrap();

        assert!(
            output
                .frame
                .scene
                .commands()
                .iter()
                .any(|command| matches!(command, SceneCommand::FillRect { .. }))
        );
        let graph = runtime.widget_graph(window_id).unwrap();
        assert_eq!(graph.nodes.len(), 2);
    }

    #[test]
    fn foreign_widget_callback_failures_are_captured() {
        let sink = ForeignErrorSink::new();
        let widget = ForeignWidget::new(FailingCallbacks).with_error_sink(sink.clone());
        let mut runtime = RuntimeApplication::new()
            .window(WindowBuilder::new().title("Errors").root(widget))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];

        let _ = runtime.render(window_id).unwrap();
        let errors = sink.snapshot();

        assert!(
            errors
                .iter()
                .any(|error| error.phase == ForeignCallbackPhase::Measure
                    && error.message.contains("measure failed"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.phase == ForeignCallbackPhase::Paint
                    && error.message.contains("paint failed"))
        );
    }

    #[test]
    fn external_cpu_texture_descriptor_validates_pixel_length() {
        let texture = ExternalTextureDescriptor::cpu_rgba8(
            Size::new(2.0, 2.0),
            Arc::<[u8]>::from(vec![0; 16]),
            7,
        );

        assert_eq!(texture.tier(), RendererInteropTier::CpuUpload);
        assert!(texture.validate().is_ok());

        let invalid = ExternalTextureDescriptor::cpu_rgba8(
            Size::new(2.0, 2.0),
            Arc::<[u8]>::from(vec![0; 15]),
            8,
        );

        assert_eq!(
            invalid.validate().unwrap_err(),
            ExternalTextureValidationError::InvalidPixelLength {
                expected: 16,
                actual: 15,
            }
        );
    }

    #[test]
    fn renderer_interop_capabilities_report_supported_tiers() {
        let cpu = RendererInteropCapabilities::cpu_only(NativeGraphicsBackend::Cpu);
        assert!(cpu.supports(RendererInteropTier::CpuUpload));
        assert!(!cpu.supports(RendererInteropTier::SharedTexture));
        assert!(!cpu.supports(RendererInteropTier::SharedRenderTarget));

        let gpu = RendererInteropCapabilities {
            backend: NativeGraphicsBackend::Wgpu,
            cpu_upload: true,
            shared_texture: true,
            shared_render_target: false,
        };
        assert!(gpu.supports(RendererInteropTier::CpuUpload));
        assert!(gpu.supports(RendererInteropTier::SharedTexture));
        assert!(!gpu.supports(RendererInteropTier::SharedRenderTarget));
    }

    #[test]
    fn binding_app_renders_basic_widget_tree() {
        let state = BindingState::new("Ready");
        let pressed = Arc::new(AtomicBool::new(false));
        let button_action = BindingAction::new({
            let pressed = Arc::clone(&pressed);
            move || {
                pressed.store(true, Ordering::Relaxed);
                Ok(())
            }
        });
        let root = BindingWidget::column(
            [
                BindingWidget::label_state(state.clone()),
                BindingWidget::button("Apply", Some(button_action)),
            ],
            8.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Bindings", root));

        let snapshot = app.render_window(0).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 2);
        state.set("Updated");
        assert_eq!(state.label_text(), "Updated");
        assert!(!pressed.load(Ordering::Relaxed));
    }

    fn assert_cross_language_snapshot_signature(snapshot: &BindingRenderSnapshot) {
        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 30);

        for role in [
            "generic_container",
            "text",
            "button",
            "link",
            "checkbox",
            "switch",
            "radio_button",
            "radio_group",
            "breadcrumb",
            "list",
            "list_item",
            "table",
            "slider",
            "spin_box",
            "combo_box",
            "progress_bar",
            "busy_indicator",
            "text_input",
            "image",
            "scroll_view",
            "color_swatch",
            "separator",
        ] {
            assert!(
                snapshot.semantics_roles.iter().any(|value| value == role),
                "missing semantics role {role:?} in {:?}",
                snapshot.semantics_roles
            );
        }

        for name in [
            "Ready",
            "Apply",
            "Search icon",
            "Download",
            "Main surface",
            "Surface content",
            "Main toolbar",
            "Toolbar action",
            "Toolbar search",
            "Documentation",
            "Enabled",
            "Airplane mode",
            "Manual",
            "Priority",
            "View mode",
            "Show list view",
            "Gallery",
            "Show map view",
            "Workspace path",
            "Assets",
            "Brush",
            "Canvas",
            "Export",
            "Build table",
            "Input signal",
            "Online",
            "Editor status",
            "Ln 12",
            "Writable",
            "UTF-8",
            "Build",
            "Opacity",
            "Count",
            "Mode",
            "Load progress",
            "Background work",
            "Name",
            "Password",
            "Scheduled for",
            "Notes",
            "Scrollable content",
            "Rich summary",
            "Accent",
            "Section divider",
            "Projects empty",
            "New project",
        ] {
            assert!(
                snapshot.semantics_names.iter().any(|value| value == name),
                "missing semantics name {name:?} in {:?}",
                snapshot.semantics_names
            );
        }

        for value in [
            "https://example.invalid/docs",
            "0.5:0:1",
            "3:0:10",
            "Medium",
            "Gallery",
            "List",
            "Map",
            "sui",
            "Canvas",
            "Bindings",
            "active",
            "Online",
            "All systems nominal",
            "Ln 12",
            "Writable",
            "UTF-8",
            "Debug profile with local bindings",
            "Final",
            "0.25:0:1",
            "Ada",
            "••••••",
            "2026-07-15 09:30",
            "Line one\nLine two",
            "Warm cool",
            "#4080BFFF",
        ] {
            assert!(
                snapshot.semantics_values.iter().any(|found| found == value),
                "missing semantics value {value:?} in {:?}",
                snapshot.semantics_values
            );
        }

        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Loading assets"),
            "missing busy indicator description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Download file"),
            "missing icon button description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Live audio input"),
            "missing signal meter description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Compact rows"),
            "missing segmented control description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "All systems nominal"),
            "missing status bar description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Create a project to get started. Templates are available"),
            "missing empty state description in {:?}",
            snapshot.semantics_descriptions
        );
        for checked in ["checked", "unchecked"] {
            assert!(
                snapshot
                    .semantics_checked
                    .iter()
                    .any(|value| value == checked),
                "missing checked state {checked:?} in {:?}",
                snapshot.semantics_checked
            );
        }
        assert!(
            snapshot.semantics_busy.iter().any(|value| *value),
            "missing busy semantics state in {:?}",
            snapshot.semantics_busy
        );
        assert!(
            snapshot
                .semantics_editable_multiline
                .iter()
                .any(|value| *value),
            "missing multiline editable semantics in {:?}",
            snapshot.semantics_editable_multiline
        );
        assert!(
            snapshot.semantics_selected.iter().any(|value| *value),
            "missing selected semantics state in {:?}",
            snapshot.semantics_selected
        );
    }

    #[test]
    fn binding_app_renders_cross_language_compatibility_signature() {
        let opacity = BindingState::new(0.5);
        let count = BindingState::new(3.0);
        let progress = BindingState::new(0.25);
        let text = BindingState::new("Ada");
        let password = BindingState::new("sëcret");
        let scheduled_for = BindingState::new("2026-07-15 09:30");
        let notes = BindingState::new("Line one\nLine two");
        let root = BindingWidget::column(
            [
                BindingWidget::label("Ready"),
                BindingWidget::button("Apply", None),
                BindingWidget::icon(
                    IconGlyph::Search,
                    Some("Search icon".to_owned()),
                    None,
                    None,
                ),
                BindingWidget::icon_button(
                    IconGlyph::Download,
                    "Download",
                    true,
                    true,
                    Some(28.0),
                    Some(16.0),
                    Some("Download file".to_owned()),
                    None,
                ),
                BindingWidget::surface(
                    BindingWidget::label("Surface content"),
                    SurfaceRole::Panel,
                    Some("Main surface".to_owned()),
                    None,
                    Some(SurfaceElevation::Small),
                    None,
                    Some(6.0),
                    false,
                    false,
                ),
                BindingWidget::toolbar(
                    [
                        BindingWidget::button("Toolbar action", None),
                        BindingWidget::icon(
                            IconGlyph::Search,
                            Some("Toolbar search".to_owned()),
                            None,
                            None,
                        ),
                    ],
                    Axis::Horizontal,
                    Some("Main toolbar".to_owned()),
                    Some(32.0),
                    Some(4.0),
                    Some(4.0),
                    None,
                    true,
                ),
                BindingWidget::link(
                    "Documentation",
                    "https://example.invalid/docs",
                    None,
                    true,
                    None,
                ),
                BindingWidget::checkbox("Enabled", true, None),
                BindingWidget::switch("Airplane mode", false, None),
                BindingWidget::radio_button("Manual", true, None),
                BindingWidget::radio_group(
                    "Priority",
                    ["Low", "Medium", "High"],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::segmented_control(
                    "View mode",
                    [
                        BindingSegmentedControlItem::new(
                            "List",
                            Some("Show list view".to_string()),
                            Some("Compact rows".to_string()),
                            false,
                        ),
                        BindingSegmentedControlItem::new("Gallery", None, None, false),
                        BindingSegmentedControlItem::new(
                            "Map",
                            Some("Show map view".to_string()),
                            None,
                            true,
                        ),
                    ],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::breadcrumb(
                    "Workspace path",
                    ["D:", "Workspace", "sui"],
                    Some(BindingNumber::Static(2.0)),
                    None,
                ),
                BindingWidget::list_view(
                    "Assets",
                    ["Brush", "Canvas", "Export"],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::table(
                    "Build table",
                    [
                        BindingTableColumn::new(
                            "Task",
                            Some(160.0),
                            None,
                            TableColumnAlignment::Start,
                            false,
                        ),
                        BindingTableColumn::new(
                            "Owner",
                            Some(96.0),
                            None,
                            TableColumnAlignment::Center,
                            false,
                        ),
                    ],
                    [
                        BindingTableRow::new(["Bindings", "IX"]),
                        BindingTableRow::new(["Renderer", "Core"]),
                    ],
                    Some(BindingNumber::Static(0.0)),
                    None,
                ),
                BindingWidget::signal_meter(
                    "Input signal",
                    true,
                    Some("Live audio input".to_string()),
                    8,
                    Some(Size::new(76.0, 16.0)),
                ),
                BindingWidget::status_badge(
                    "Online",
                    SemanticTone::Success,
                    Some(IconGlyph::Check),
                    Some(72.0),
                ),
                BindingWidget::status_bar(
                    [
                        BindingStatusBarSegment::new("Ln 12", SemanticTone::Neutral, None, false),
                        BindingStatusBarSegment::new(
                            "Writable",
                            SemanticTone::Success,
                            Some(84.0),
                            false,
                        ),
                        BindingStatusBarSegment::new("UTF-8", SemanticTone::Info, None, true),
                    ],
                    Some("Editor status".to_string()),
                    Some("All systems nominal".into()),
                    Some(24.0),
                ),
                BindingWidget::detail_row("Build", "Debug profile with local bindings", Some(2)),
                BindingWidget::slider("Opacity", opacity, 0.0, 1.0, 0.25, None),
                BindingWidget::number_input("Count", count, 0.0, 10.0, 1.0, 0, None),
                BindingWidget::select(
                    "Mode",
                    ["Draft", "Final", "Review"],
                    Some(BindingNumber::Static(1.0)),
                    Some("Choose mode".to_string()),
                    None,
                ),
                BindingWidget::progress_bar("Load progress", progress, 0.0, 1.0, true),
                BindingWidget::busy_indicator(
                    "Background work",
                    Some("Loading assets".into()),
                    20.0,
                ),
                BindingWidget::text_input("Name", text, Some("Type a name".to_string()), None),
                BindingWidget::password_input(
                    "Password",
                    password,
                    Some("Enter a password".to_string()),
                    None,
                ),
                BindingWidget::datetime_input("Scheduled for", scheduled_for, None, None),
                BindingWidget::text_area("Notes", notes, Some("Type notes".to_string()), None),
                BindingWidget::scroll_view(
                    BindingWidget::rich_text(
                        [
                            BindingTextSpan::new(
                                "Warm",
                                TextStyle::new(Color::rgba(0.9, 0.35, 0.2, 1.0)),
                            ),
                            BindingTextSpan::new(
                                " cool",
                                TextStyle::new(Color::rgba(0.25, 0.55, 0.9, 1.0)),
                            ),
                        ],
                        Some("Rich summary".to_string()),
                        0.0,
                        0.0,
                    ),
                    BindingScrollAxes::Vertical,
                    Some("Scrollable content".to_string()),
                ),
                BindingWidget::color_swatch(
                    "Accent",
                    Color::rgba(0.25, 0.5, 0.75, 1.0),
                    Some(Size::new(24.0, 24.0)),
                    false,
                    None,
                ),
                BindingWidget::separator(
                    Axis::Horizontal,
                    Some("Section divider".to_string()),
                    0.0,
                    None,
                    Some(24.0),
                ),
                BindingWidget::action_card(
                    "Create document",
                    "Start from a blank canvas",
                    Some(IconGlyph::Add),
                    SemanticTone::Accent,
                    true,
                    None,
                ),
                BindingWidget::brush_preview(
                    "Brush preview",
                    "Ink brush",
                    BindingBrushPreviewSpec::new(
                        Color::rgba(0.2, 0.45, 0.8, 1.0),
                        18.0,
                        0.75,
                        BrushPreviewShape::Round,
                    ),
                    Some(Size::new(48.0, 48.0)),
                ),
                BindingWidget::command_group(
                    "Editing commands",
                    [BindingWidget::button("Duplicate", None)],
                    Axis::Horizontal,
                    None,
                    Some(4.0),
                    None,
                    None,
                    None,
                ),
                BindingWidget::coverage_dots(
                    "Replica coverage",
                    2,
                    3,
                    SemanticTone::Success,
                    4,
                    true,
                    Some(84.0),
                ),
                BindingWidget::dock(
                    BindingWidget::label("Dock body"),
                    Some((24.0, BindingWidget::label("Dock top"))),
                    Some((24.0, BindingWidget::label("Dock bottom"))),
                    240.0,
                    120.0,
                ),
                BindingWidget::fixed_pane_split(
                    Axis::Horizontal,
                    BindingWidget::label("Fixed pane"),
                    BindingWidget::separator(Axis::Vertical, None, 0.0, None, None),
                    BindingWidget::label("Flexible pane"),
                    false,
                    96.0,
                    1.0,
                    160.0,
                ),
                BindingWidget::framed_field(
                    BindingWidget::label("Framed value"),
                    Some("Framed field".to_string()),
                    Some("Reusable editor frame".to_string()),
                    Some(Insets::all(6.0)),
                    Some(32.0),
                    true,
                    false,
                    false,
                ),
                BindingWidget::measured_bottom_dock(
                    BindingWidget::label("Measured dock body"),
                    BindingWidget::label("Measured dock footer"),
                    Size::new(240.0, 120.0),
                ),
                BindingWidget::placement_badge(
                    "Primary replica",
                    Some(IconGlyph::Check),
                    SemanticTone::Success,
                    Some(2),
                    Some(3),
                    Some(120.0),
                ),
                BindingWidget::property_row(
                    "Property",
                    BindingWidget::label("Property value"),
                    false,
                    Some(100.0),
                    Some(160.0),
                    Some(8.0),
                ),
                BindingWidget::section_label(
                    "Advanced",
                    Some("Advanced section".to_string()),
                    None,
                ),
                BindingWidget::side_sheet(
                    "Inspector",
                    BindingWidget::label("Inspector body"),
                    Some("Document settings".to_string()),
                    true,
                    false,
                    true,
                    SideSheetPlacement::Right,
                    Some(280.0),
                    Some(BindingWidget::button("Close inspector", None)),
                    [BindingWidget::button("Save inspector", None)],
                    None,
                ),
                BindingWidget::split_view(
                    Some("Document split".to_string()),
                    Axis::Horizontal,
                    BindingWidget::label("Split first"),
                    BindingWidget::label("Split second"),
                    0.4,
                    40.0,
                    40.0,
                    Some(2.0),
                    None,
                ),
                BindingWidget::switch_view(
                    [
                        BindingWidget::label("Switch first"),
                        BindingWidget::label("Switch second"),
                    ],
                    0.0,
                ),
                BindingWidget::trailing_slot_row(
                    BindingWidget::label("Trailing row body"),
                    BindingWidget::button("Trailing action", None),
                    96.0,
                    28.0,
                    6.0,
                ),
                BindingWidget::virtual_scroll_view(
                    [
                        BindingWidget::label("Virtual row one"),
                        BindingWidget::label("Virtual row two"),
                    ],
                    Some("Virtual rows".to_string()),
                    Some(Insets::all(4.0)),
                    Some(2.0),
                ),
                BindingWidget::floating_stack(
                    [
                        BindingFloatingStackWindow::new(
                            Rect::new(0.0, 0.0, 160.0, 48.0),
                            BindingWidget::label("Floating first"),
                        ),
                        BindingFloatingStackWindow::new(
                            Rect::new(24.0, 16.0, 160.0, 48.0),
                            BindingWidget::label("Floating second"),
                        ),
                    ],
                    Some("Floating windows".to_string()),
                ),
                BindingWidget::reorderable_list(
                    "Reorderable tasks",
                    [
                        BindingWidget::label("First task"),
                        BindingWidget::label("Second task"),
                    ],
                    4.0,
                    4.0,
                    Some("Reordering task".to_string()),
                    None,
                ),
                BindingWidget::empty_state(
                    "No projects",
                    "Create a project to get started.",
                    Some("Projects empty".to_string()),
                    Some("Templates are available".to_string()),
                    Some(IconGlyph::Folder),
                    Some(BindingWidget::button("New project", None)),
                    None,
                    true,
                ),
            ],
            6.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Compatibility", root));

        let snapshot = app.render_window(0).unwrap();

        assert_cross_language_snapshot_signature(&snapshot);
    }

    #[test]
    fn binding_side_sheet_reads_bound_shown_state() {
        let shown = BindingState::new(false);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Side sheet state",
            BindingWidget::side_sheet(
                "Bound inspector",
                BindingWidget::label("Inspector content"),
                Some("State-driven drawer".to_string()),
                shown.clone(),
                true,
                true,
                SideSheetPlacement::Right,
                Some(280.0),
                None,
                std::iter::empty(),
                None,
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let hidden = runtime.render_window(window_id).unwrap();
        assert!(shown.is_ui_bound());
        assert!(!hidden.semantics_roles.iter().any(|role| role == "dialog"));
        assert!(
            !hidden
                .semantics_names
                .iter()
                .any(|name| name == "Bound inspector")
        );

        shown.set(true);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let visible = runtime.render_window(window_id).unwrap();
        assert!(visible.semantics_roles.iter().any(|role| role == "dialog"));
        assert!(
            visible
                .semantics_names
                .iter()
                .any(|name| name == "Bound inspector")
        );
        let content_id = runtime
            .runtime
            .semantics(window_id.into_sui())
            .unwrap()
            .iter()
            .find(|node| node.name.as_deref() == Some("Inspector content"))
            .unwrap()
            .id;

        shown.set(false);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let hidden_again = runtime.render_window(window_id).unwrap();
        assert!(
            !hidden_again
                .semantics_names
                .iter()
                .any(|name| name == "Bound inspector")
        );

        shown.set(true);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let _ = runtime.render_window(window_id).unwrap();
        assert_eq!(
            runtime
                .runtime
                .semantics(window_id.into_sui())
                .unwrap()
                .iter()
                .find(|node| node.name.as_deref() == Some("Inspector content"))
                .unwrap()
                .id,
            content_id,
            "bound visibility updates must retain sheet content identity"
        );
    }

    #[test]
    fn binding_split_view_reads_bound_ratio_state() {
        let ratio = BindingState::new(0.25);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Split view state",
            BindingWidget::split_view(
                Some("Bound split".to_string()),
                Axis::Horizontal,
                BindingWidget::label("First pane"),
                BindingWidget::label("Second pane"),
                ratio.clone(),
                20.0,
                20.0,
                Some(2.0),
                None,
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let initial = runtime.render_window(window_id).unwrap();
        assert!(ratio.is_ui_bound());
        assert!(
            initial
                .semantics_roles
                .iter()
                .any(|role| role == "splitter")
        );
        assert!(
            initial
                .semantics_names
                .iter()
                .any(|name| name == "Bound split")
        );
        assert!(initial.semantics_values.iter().any(|value| value == "0.25"));
        let first_pane_id = runtime
            .runtime
            .semantics(window_id.into_sui())
            .unwrap()
            .iter()
            .find(|node| node.name.as_deref() == Some("First pane"))
            .unwrap()
            .id;

        ratio.set(0.75);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let updated = runtime.render_window(window_id).unwrap();
        assert!(updated.semantics_values.iter().any(|value| value == "0.75"));
        assert!(!updated.semantics_values.iter().any(|value| value == "0.25"));
        assert_eq!(
            runtime
                .runtime
                .semantics(window_id.into_sui())
                .unwrap()
                .iter()
                .find(|node| node.name.as_deref() == Some("First pane"))
                .unwrap()
                .id,
            first_pane_id,
            "bound ratio updates must retain pane widget identity"
        );
    }

    #[test]
    fn binding_virtual_scroll_floating_stack_and_reorderable_list_render_children() {
        let root = BindingWidget::column(
            [
                BindingWidget::sized_box(
                    Some(BindingWidget::virtual_scroll_view(
                        [
                            BindingWidget::label("Virtual child one"),
                            BindingWidget::label("Virtual child two"),
                        ],
                        Some("Virtual collection".to_string()),
                        Some(Insets::all(4.0)),
                        Some(2.0),
                    )),
                    Some(240.0),
                    Some(80.0),
                ),
                BindingWidget::sized_box(
                    Some(BindingWidget::floating_stack(
                        [
                            BindingFloatingStackWindow::new(
                                Rect::new(0.0, 0.0, 160.0, 36.0),
                                BindingWidget::label("Floating child one"),
                            ),
                            BindingFloatingStackWindow::new(
                                Rect::new(20.0, 28.0, 160.0, 36.0),
                                BindingWidget::label("Floating child two"),
                            ),
                        ],
                        Some("Floating workspace".to_string()),
                    )),
                    Some(240.0),
                    Some(80.0),
                ),
                BindingWidget::reorderable_list(
                    "Queued work",
                    [
                        BindingWidget::sized_box(
                            Some(BindingWidget::label("Queued first")),
                            Some(200.0),
                            Some(28.0),
                        ),
                        BindingWidget::sized_box(
                            Some(BindingWidget::label("Queued second")),
                            Some(200.0),
                            Some(28.0),
                        ),
                    ],
                    2.0,
                    4.0,
                    Some("Moving queued work".to_string()),
                    None,
                ),
            ],
            6.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Portable containers", root));

        let snapshot = app.render_window(0).unwrap();

        assert!(snapshot.command_count > 0);
        for name in [
            "Virtual collection",
            "Virtual child one",
            "Floating workspace",
            "Floating child one",
            "Floating child two",
            "Queued work",
            "Queued first",
            "Queued second",
        ] {
            assert!(
                snapshot.semantics_names.iter().any(|found| found == name),
                "missing semantics name {name:?} in {:?}",
                snapshot.semantics_names
            );
        }
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "scroll_view")
        );
        assert!(snapshot.semantics_roles.iter().any(|role| role == "list"));
    }

    #[test]
    fn binding_reorderable_list_reports_reorder_and_captures_callback_error() {
        let changes = Arc::new(Mutex::new(Vec::new()));
        let action = BindingReorderAction::new({
            let changes = Arc::clone(&changes);
            move |item, from, to| {
                recover_lock(&changes).push((item, from, to));
                Err(ForeignCallbackFailure::new("reorder callback failed"))
            }
        });
        let root = BindingWidget::reorderable_list(
            "Tasks",
            [
                BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
                BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
                BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
            ],
            0.0,
            4.0,
            Some("Moving task".to_string()),
            Some(action),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Reorder", root));
        let errors = app.error_sink();
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();
        let _ = runtime.render_window(window_id).unwrap();

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(10.0, 15.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();

        for position in [Point::new(10.0, 48.0), Point::new(10.0, 78.0)] {
            let mut moved = BindingPointerEvent::new(BindingPointerEventKind::Move, position);
            moved.button = Some(BindingPointerButton::Primary);
            moved.buttons = 1;
            runtime
                .handle_event(window_id, BindingEvent::Pointer(moved))
                .unwrap();
        }

        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(10.0, 78.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(&*recover_lock(&changes), &[(0, 0, 2)]);
        let errors = errors.snapshot();
        assert!(errors.iter().any(|error| {
            error.phase == ForeignCallbackPhase::Event
                && error.message.contains("reorder callback failed")
        }));
    }

    #[test]
    fn binding_app_renders_form_controls_and_updates_bound_checkbox() {
        let checked = BindingState::new(false);
        let slider_value = BindingState::new(0.25);
        let toggled = Arc::new(AtomicBool::new(false));
        let toggle_action = BindingBoolAction::new({
            let toggled = Arc::clone(&toggled);
            move |value| {
                toggled.store(value, Ordering::Relaxed);
                Ok(())
            }
        });
        let root = BindingWidget::column(
            [
                BindingWidget::checkbox("Enabled", checked.clone(), Some(toggle_action)),
                BindingWidget::switch("Airplane mode", false, None),
                BindingWidget::slider("Opacity", slider_value.clone(), 0.0, 1.0, 0.05, None),
            ],
            8.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Controls", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 3);
        assert_eq!(checked.get(), BindingValue::Bool(false));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(32.0, 18.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(checked.get(), BindingValue::Bool(true));
        assert!(toggled.load(Ordering::Relaxed));
    }

    #[test]
    fn binding_radio_button_updates_bound_state_from_pointer() {
        let selected = BindingState::new(false);
        let selected_action = Arc::new(AtomicBool::new(false));
        let action = BindingAction::new({
            let selected_action = Arc::clone(&selected_action);
            move || {
                selected_action.store(true, Ordering::Relaxed);
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Radio",
            BindingWidget::radio_button("Manual", selected.clone(), Some(action)),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "radio_button")
        );
        assert_eq!(selected.get(), BindingValue::Bool(false));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(32.0, 18.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(selected.get(), BindingValue::Bool(true));
        assert!(selected_action.load(Ordering::Relaxed));
    }

    #[test]
    fn binding_link_invokes_open_callback_from_pointer() {
        let opened = Arc::new(Mutex::new(None::<String>));
        let action = BindingStringAction::new({
            let opened = Arc::clone(&opened);
            move |url| {
                *opened.lock().unwrap() = Some(url);
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Link",
            BindingWidget::link(
                "Documentation",
                "https://example.invalid/docs",
                None,
                true,
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.semantics_roles.iter().any(|role| role == "link"));
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "https://example.invalid/docs")
        );

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(4.0, 4.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(4.0, 4.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(
            opened.lock().unwrap().as_deref(),
            Some("https://example.invalid/docs")
        );
    }

    #[test]
    fn binding_color_swatch_invokes_press_callback_from_pointer() {
        let pressed = Arc::new(AtomicBool::new(false));
        let action = BindingAction::new({
            let pressed = Arc::clone(&pressed);
            move || {
                pressed.store(true, Ordering::Relaxed);
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Swatch",
            BindingWidget::color_swatch(
                "Accent",
                Color::rgba(0.25, 0.5, 0.75, 1.0),
                Some(Size::new(24.0, 24.0)),
                false,
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "color_swatch")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "#4080BFFF")
        );

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(12.0, 12.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(12.0, 12.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert!(pressed.load(Ordering::Relaxed));
    }

    #[test]
    fn binding_rich_text_exposes_plain_text_semantics() {
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Rich text",
            BindingWidget::rich_text(
                [
                    BindingTextSpan::new("Warm", TextStyle::new(Color::rgba(0.9, 0.35, 0.2, 1.0))),
                    BindingTextSpan::new(
                        " cool",
                        TextStyle::new(Color::rgba(0.25, 0.55, 0.9, 1.0)),
                    ),
                ],
                Some("Rich summary".to_string()),
                80.0,
                0.0,
            ),
        ));

        let snapshot = app.render_window(0).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_roles.iter().any(|role| role == "text"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Rich summary")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Warm cool")
        );
    }

    #[test]
    fn binding_scroll_view_exposes_container_and_child_semantics() {
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Scroll",
            BindingWidget::scroll_view(
                BindingWidget::label("Inside"),
                BindingScrollAxes::Vertical,
                Some("Scrollable content".to_string()),
            ),
        ));

        let snapshot = app.render_window(0).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "scroll_view")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Scrollable content")
        );
        assert!(snapshot.semantics_names.iter().any(|name| name == "Inside"));
    }

    #[test]
    fn binding_breadcrumb_reads_bound_state() {
        let name = BindingState::new("Workspace path");
        let current = BindingState::new(0.0);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Breadcrumb",
            BindingWidget::breadcrumb(
                BindingText::State(name.clone()),
                ["D:", "Workspace", "sui"],
                Some(BindingNumber::State(current.clone())),
                None,
            ),
        ));

        let snapshot = app.render_window(0).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "breadcrumb")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|found| found == "Workspace path")
        );
        assert!(snapshot.semantics_values.iter().any(|value| value == "D:"));

        name.set("Project path");
        current.set(2.0);
        let snapshot = app.render_window(0).unwrap();
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|found| found == "Project path")
        );
        assert!(snapshot.semantics_values.iter().any(|value| value == "sui"));
    }

    #[test]
    fn binding_table_reads_bound_state() {
        let name = BindingState::new("Build table");
        let selected = BindingState::new(1.0);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Table",
            BindingWidget::table(
                BindingText::State(name.clone()),
                [
                    BindingTableColumn::new("Task", None, None, TableColumnAlignment::Start, false),
                    BindingTableColumn::new(
                        "Owner",
                        None,
                        None,
                        TableColumnAlignment::Center,
                        false,
                    ),
                ],
                [
                    BindingTableRow::new(["Bindings", "IX"]),
                    BindingTableRow::new(["Renderer", "Core"]),
                ],
                Some(BindingNumber::State(selected.clone())),
                None,
            ),
        ));

        let snapshot = app.render_window(0).unwrap();
        assert!(snapshot.semantics_roles.iter().any(|role| role == "table"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|found| found == "Build table")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Renderer")
        );

        name.set("Task table");
        selected.set(0.0);
        let snapshot = app.render_window(0).unwrap();
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|found| found == "Task table")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Bindings")
        );
    }

    #[test]
    fn binding_select_updates_bound_state_from_keyboard() {
        let selected = BindingState::new(0.0);
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Select",
            BindingWidget::select(
                "Mode",
                ["Draft", "Final", "Review"],
                Some(BindingNumber::State(selected.clone())),
                Some("Choose mode".to_string()),
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "combo_box")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Draft")
        );
        assert_eq!(selected.get(), BindingValue::Number(0.0));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(20.0, 20.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(20.0, 20.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new(
                    "ArrowDown",
                    BindingKeyState::Pressed,
                )),
            )
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new(
                    "Enter",
                    BindingKeyState::Pressed,
                )),
            )
            .unwrap();

        assert_eq!(selected.get(), BindingValue::Number(1.0));
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Final".to_string())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Final")
        );
    }

    #[test]
    fn binding_radio_group_updates_bound_state_from_pointer() {
        let selected = BindingState::new(0.0);
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Radio group",
            BindingWidget::radio_group(
                "Priority",
                ["Low", "Medium", "High"],
                Some(BindingNumber::State(selected.clone())),
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "radio_group")
        );
        assert!(snapshot.semantics_values.iter().any(|value| value == "Low"));
        assert_eq!(selected.get(), BindingValue::Number(0.0));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(20.0, 52.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(20.0, 52.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(selected.get(), BindingValue::Number(1.0));
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Medium".to_string())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Medium")
        );
    }

    #[test]
    fn binding_segmented_control_updates_bound_state_from_pointer() {
        let selected = BindingState::new(0.0);
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Segmented control",
            BindingWidget::segmented_control(
                "View mode",
                [
                    BindingSegmentedControlItem::new(
                        "List",
                        Some("Show list view".to_string()),
                        Some("Compact rows".to_string()),
                        false,
                    ),
                    BindingSegmentedControlItem::new("Gallery", None, None, false),
                    BindingSegmentedControlItem::new(
                        "Map",
                        Some("Show map view".to_string()),
                        None,
                        true,
                    ),
                ],
                Some(BindingNumber::State(selected.clone())),
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "radio_group")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Show list view")
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|description| description == "Compact rows")
        );
        assert_eq!(selected.get(), BindingValue::Number(0.0));

        'scan: for y in [4.0, 12.0, 20.0, 32.0, 48.0, 64.0] {
            for x in (0..=2000).step_by(24) {
                let point = Point::new(x as f32, y);
                let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, point);
                down.button = Some(BindingPointerButton::Primary);
                down.buttons = 1;
                runtime
                    .handle_event(window_id, BindingEvent::Pointer(down))
                    .unwrap();
                let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, point);
                up.button = Some(BindingPointerButton::Primary);
                runtime
                    .handle_event(window_id, BindingEvent::Pointer(up))
                    .unwrap();
                if selected.get() == BindingValue::Number(1.0) {
                    break 'scan;
                }
            }
        }

        assert_eq!(selected.get(), BindingValue::Number(1.0));
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Gallery".to_string())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Gallery")
        );
        assert!(
            snapshot.semantics_disabled.iter().any(|disabled| *disabled),
            "missing disabled segmented-control item in {:?}",
            snapshot.semantics_disabled
        );
    }

    #[test]
    fn binding_list_view_updates_bound_state_from_pointer() {
        let selected = BindingState::new(0.0);
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "List view",
            BindingWidget::list_view(
                "Assets",
                ["Brush", "Canvas", "Export"],
                Some(BindingNumber::State(selected.clone())),
                Some(action),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.semantics_roles.iter().any(|role| role == "list"));
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "list_item")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Brush")
        );
        assert_eq!(selected.get(), BindingValue::Number(0.0));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(44.0, 44.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(44.0, 44.0));
        up.button = Some(BindingPointerButton::Primary);
        runtime
            .handle_event(window_id, BindingEvent::Pointer(up))
            .unwrap();

        assert_eq!(selected.get(), BindingValue::Number(1.0));
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Canvas".to_string())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Canvas")
        );
        assert!(
            snapshot.semantics_selected.iter().any(|selected| *selected),
            "missing selected list item state in {:?}",
            snapshot.semantics_selected
        );
    }

    #[test]
    fn binding_signal_meter_reads_bound_active_state() {
        let active = BindingState::new(true);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Signal meter",
            BindingWidget::signal_meter(
                "Input signal",
                active.clone(),
                Some("Live audio input".to_string()),
                8,
                Some(Size::new(76.0, 16.0)),
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "generic_container")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Input signal")
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|description| description == "Live audio input")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "active")
        );

        active.set(false);
        assert_eq!(runtime.pending_ui_task_count(), 1);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "idle")
        );
    }

    #[test]
    fn binding_icon_button_reads_bound_state() {
        let selected = BindingState::new(false);
        let enabled = BindingState::new(true);
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Icon button",
            BindingWidget::icon_button(
                IconGlyph::Download,
                "Download",
                selected.clone(),
                enabled.clone(),
                Some(28.0),
                Some(16.0),
                Some("Download file".to_string()),
                None,
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_roles.iter().any(|role| role == "button"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Download")
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|description| description == "Download file")
        );
        assert!(!snapshot.semantics_selected.iter().any(|value| *value));
        assert!(!snapshot.semantics_disabled.iter().any(|value| *value));

        selected.set(true);
        enabled.set(false);
        assert_eq!(runtime.pending_ui_task_count(), 2);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 2);
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot.semantics_selected.iter().any(|value| *value),
            "missing selected icon button state in {:?}",
            snapshot.semantics_selected
        );
        assert!(
            snapshot.semantics_disabled.iter().any(|value| *value),
            "missing disabled icon button state in {:?}",
            snapshot.semantics_disabled
        );
    }

    #[test]
    fn binding_text_input_updates_bound_state_from_keyboard() {
        let text = BindingState::new("");
        let root =
            BindingWidget::text_input("Name", text.clone(), Some("Type here".to_string()), None);
        let app = BindingApp::new().with_window(BindingWindow::new("Text input", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert_eq!(text.get(), BindingValue::String(String::new()));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new("a", BindingKeyState::Pressed)),
            )
            .unwrap();

        assert_eq!(text.get(), BindingValue::String("a".to_string()));
    }

    #[test]
    fn binding_password_input_masks_semantics_and_updates_state_and_action() {
        let text = BindingState::new("");
        let changed = Arc::new(Mutex::new(None::<String>));
        let action = BindingStringAction::new({
            let changed = Arc::clone(&changed);
            move |value| {
                *changed.lock().unwrap() = Some(value);
                Ok(())
            }
        });
        let root = BindingWidget::password_input(
            "Password",
            text.clone(),
            Some("Enter a password".to_string()),
            Some(action),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Password input", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        let password_index = snapshot
            .semantics_names
            .iter()
            .position(|name| name == "Password")
            .expect("password input semantics");
        assert_eq!(snapshot.semantics_values[password_index], "");

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new("s", BindingKeyState::Pressed)),
            )
            .unwrap();

        assert_eq!(text.get(), BindingValue::String("s".to_string()));
        assert_eq!(changed.lock().unwrap().as_deref(), Some("s"));
        let snapshot = runtime.render_window(window_id).unwrap();
        let password_index = snapshot
            .semantics_names
            .iter()
            .position(|name| name == "Password")
            .expect("password input semantics");
        assert_eq!(snapshot.semantics_values[password_index], "•");
        assert!(!snapshot.semantics_values.iter().any(|value| value == "s"));

        text.set("sëcret");
        let snapshot = runtime.render_window(window_id).unwrap();
        let password_index = snapshot
            .semantics_names
            .iter()
            .position(|name| name == "Password")
            .expect("password input semantics");
        assert_eq!(snapshot.semantics_values[password_index], "••••••");
        assert!(
            !snapshot
                .semantics_values
                .iter()
                .any(|value| value == "sëcret")
        );
    }

    #[test]
    fn binding_datetime_input_preserves_local_string_and_updates_action() {
        let text = BindingState::new("");
        let changed = Arc::new(Mutex::new(None::<String>));
        let action = BindingStringAction::new({
            let changed = Arc::clone(&changed);
            move |value| {
                *changed.lock().unwrap() = Some(value);
                Ok(())
            }
        });
        let root = BindingWidget::datetime_input("Scheduled for", text.clone(), None, Some(action));
        let app = BindingApp::new().with_window(BindingWindow::new("Date/time input", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Scheduled for")
        );

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new("2", BindingKeyState::Pressed)),
            )
            .unwrap();

        assert_eq!(text.get(), BindingValue::String("2".to_string()));
        assert_eq!(changed.lock().unwrap().as_deref(), Some("2"));

        text.set("2026-07-15 09:30");
        let snapshot = runtime.render_window(window_id).unwrap();
        let datetime_index = snapshot
            .semantics_names
            .iter()
            .position(|name| name == "Scheduled for")
            .expect("date/time input semantics");
        assert_eq!(
            snapshot.semantics_values[datetime_index],
            "2026-07-15 09:30"
        );
    }

    #[test]
    fn binding_text_area_updates_bound_state_from_keyboard() {
        let text = BindingState::new("");
        let root =
            BindingWidget::text_area("Notes", text.clone(), Some("Type notes".to_string()), None);
        let app = BindingApp::new().with_window(BindingWindow::new("Text area", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert!(
            snapshot
                .semantics_editable_multiline
                .iter()
                .any(|value| *value)
        );
        assert_eq!(text.get(), BindingValue::String(String::new()));

        let mut down =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(down))
            .unwrap();
        runtime
            .handle_event(
                window_id,
                BindingEvent::Keyboard(BindingKeyboardEvent::new("a", BindingKeyState::Pressed)),
            )
            .unwrap();

        assert_eq!(text.get(), BindingValue::String("a".to_string()));
    }

    #[test]
    fn binding_app_renders_foreign_widget_tree_and_dispatches_events() {
        let callbacks = Arc::new(MockCallbacks::default());
        let root = BindingWidget::column(
            [
                BindingWidget::foreign_arc(callbacks.clone()),
                BindingWidget::label("Tail"),
            ],
            4.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Foreign binding", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 2);
        assert!(snapshot.semantics_disabled.iter().any(|value| *value));
        assert!(snapshot.semantics_hidden.iter().any(|value| *value));
        assert!(snapshot.semantics_hovered.iter().any(|value| *value));
        assert!(snapshot.semantics_selected.iter().any(|value| *value));
        assert!(
            snapshot
                .semantics_expanded
                .iter()
                .any(|value| value == "expanded")
        );
        assert!(callbacks.measures.load(Ordering::Relaxed) >= 1);
        assert_eq!(callbacks.paints.load(Ordering::Relaxed), 1);

        let mut pointer =
            BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(8.0, 8.0));
        pointer.button = Some(BindingPointerButton::Primary);
        pointer.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(pointer))
            .unwrap();

        assert_eq!(callbacks.events.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn binding_app_registers_app_level_image_resources() {
        let mut app = BindingApp::new();
        let image = app
            .register_rgba_image(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap();
        let root = BindingWidget::foreign(AppImageCallbacks { image });
        app.push_window(BindingWindow::new("Image resource", root));

        assert_eq!(app.image_resource_count(), 1);

        let snapshot = app.render_window(0).unwrap();

        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
    }

    #[test]
    fn binding_app_renders_high_level_image_widget() {
        let mut app = BindingApp::new();
        let image = app
            .register_rgba_image(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap();
        let root = BindingWidget::image(
            image,
            Some("Preview".to_string()),
            BindingImageFit::Contain,
            Some(Size::new(32.0, 16.0)),
        );
        app.push_window(BindingWindow::new("Image widget", root));

        let snapshot = app.render_window(0).unwrap();

        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
        assert!(snapshot.semantics_roles.iter().any(|role| role == "image"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Preview")
        );
    }

    #[test]
    fn binding_app_registers_app_level_png_resources() {
        let mut app = BindingApp::new();
        let png = test_png_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
        let image = app.register_png_image(png).unwrap();
        let root = BindingWidget::foreign(AppImageCallbacks { image });
        app.push_window(BindingWindow::new("PNG resource", root));

        assert_eq!(app.image_resource_count(), 1);

        let snapshot = app.render_window(0).unwrap();

        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
    }

    #[test]
    fn binding_app_registers_app_level_font_resources() {
        let mut app = BindingApp::new();
        let font = app.register_font_bytes(vec![0, 1, 2, 3]).unwrap();
        assert!(font.get() > 0);
        app.push_window(BindingWindow::new("Fonts", BindingWidget::label("Text")));

        assert_eq!(app.font_resource_count(), 1);

        let snapshot = app.render_window(0).unwrap();

        assert_eq!(snapshot.registered_font_count, 1);
    }

    #[test]
    fn binding_external_surface_draws_cpu_fallback() {
        let texture = ExternalTextureDescriptor::cpu_rgba8(
            Size::new(2.0, 1.0),
            vec![255, 0, 0, 255, 0, 0, 255, 255],
            3,
        );
        let root = BindingWidget::external_surface(
            texture,
            Some(Size::new(64.0, 32.0)),
            Some("External preview".to_string()),
        )
        .unwrap();
        let app = BindingApp::new().with_window(BindingWindow::new("External", root));

        let snapshot = app.render_window(0).unwrap();

        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
        assert_eq!(snapshot.semantics_count, 1);
    }

    #[test]
    fn binding_runtime_queues_bound_state_updates_and_marks_redraw() {
        let state = BindingState::new("Ready");
        let root = BindingWidget::column(
            [
                BindingWidget::label_state(state.clone()),
                BindingWidget::button("Apply", None),
            ],
            8.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Bindings", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        assert!(state.is_ui_bound());
        assert_eq!(runtime.window_count(), 1);
        assert_eq!(runtime.window_ids(), vec![window_id]);
        assert!(runtime.needs_render(window_id).unwrap());

        let initial = runtime.render_window(window_id).unwrap();
        assert!(initial.command_count > 0);
        runtime
            .handle_event(
                window_id,
                BindingEvent::Custom(BindingCustomEvent {
                    kind: "binding-smoke".to_string(),
                    payload: Some("ok".to_string()),
                }),
            )
            .unwrap();

        let woke = Arc::new(AtomicBool::new(false));
        runtime.set_waker({
            let woke = Arc::clone(&woke);
            move || woke.store(true, Ordering::Relaxed)
        });
        state.set("Updated");

        assert!(woke.load(Ordering::Relaxed));
        assert_eq!(state.label_text(), "Ready");
        assert_eq!(runtime.pending_ui_task_count(), 1);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert_eq!(state.label_text(), "Updated");
        assert!(runtime.needs_render(window_id).unwrap());

        let updated = runtime.render_window_at(0).unwrap();
        assert!(updated.command_count > 0);
    }

    #[test]
    fn binding_runtime_external_wake_drains_bound_state_updates() {
        let state = BindingState::new("Idle");
        let root = BindingWidget::label_state(state.clone());
        let app = BindingApp::new().with_window(BindingWindow::new("Wake", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        state.set("Awake");
        assert_eq!(runtime.pending_ui_task_count(), 1);

        runtime.wake_window(window_id).unwrap();

        assert_eq!(runtime.pending_ui_task_count(), 0);
        assert_eq!(state.label_text(), "Awake");
        assert!(runtime.needs_render(window_id).unwrap());
    }

    #[test]
    fn binding_theme_updates_are_live_and_use_the_ui_queue() {
        let theme = BindingTheme::preset("dark").unwrap();
        let mut app = BindingApp::new().with_window(BindingWindow::new(
            "Themed",
            BindingWidget::button("Save", None),
        ));
        app.set_theme(theme.clone());
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        runtime.render_window(window_id).unwrap();
        theme.set_preset("light").unwrap();
        assert_eq!(runtime.pending_ui_task_count(), 1);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert!(runtime.needs_render(window_id).unwrap());

        theme.set_accent(Color::rgba(0.2, 0.5, 0.9, 1.0));
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert!(theme.accent().blue > theme.accent().red);
        theme
            .set_color("success", Color::rgba(0.1, 0.8, 0.3, 1.0))
            .unwrap();
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert!(theme.color("success").unwrap().green > 0.7);
        theme.set_number("radius-md", 9.0).unwrap();
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert_eq!(theme.number("radius-md").unwrap(), 9.0);
    }

    #[test]
    fn binding_state_observers_are_distinct_and_unsubscribable() {
        let state = BindingState::new(1.0);
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut subscription = state.observe({
            let observed = Arc::clone(&observed);
            move |value| recover_lock(&observed).push(value)
        });

        state.set(1.0);
        assert!(recover_lock(&observed).is_empty());
        state.set(2.0);
        assert_eq!(
            recover_lock(&observed).as_slice(),
            &[BindingValue::Number(2.0)]
        );
        assert!(subscription.unsubscribe());
        assert!(!subscription.unsubscribe());
        state.set(3.0);
        assert_eq!(recover_lock(&observed).len(), 1);
    }

    #[test]
    fn binding_message_bus_posts_named_payloads_to_the_ui_queue() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let mut app = BindingApp::new().with_window(BindingWindow::new(
            "Messages",
            BindingWidget::label("Ready"),
        ));
        app.on_message(
            "background.complete",
            BindingMessageAction::new({
                let received = Arc::clone(&received);
                move |payload| {
                    recover_lock(&received).push(payload);
                    Ok(())
                }
            }),
        );
        let mut runtime = app.start().unwrap();
        let handle = runtime.ui_handle();

        assert!(handle.emit("background.complete", "Loaded".into()));
        assert!(!handle.emit("unknown", true.into()));
        assert_eq!(runtime.pending_ui_task_count(), 1);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        assert_eq!(
            recover_lock(&received).as_slice(),
            &[BindingValue::String("Loaded".to_owned())]
        );
    }

    #[test]
    fn binding_runtime_exposes_renderer_neutral_inspector_counts() {
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Inspector",
            BindingWidget::column(
                [
                    BindingWidget::label("Ready"),
                    BindingWidget::button("Run", None),
                ],
                4.0,
            ),
        ));
        let mut runtime = app.start().unwrap();
        let window = runtime.window_id_at(0).unwrap();
        runtime.set_inspector_tracing(window, true).unwrap();
        runtime.render_window(window).unwrap();
        runtime
            .handle_event(
                window,
                BindingEvent::Custom(BindingCustomEvent {
                    kind: "inspect".to_owned(),
                    payload: None,
                }),
            )
            .unwrap();

        let snapshot = runtime.inspector_snapshot(window).unwrap();
        assert!(snapshot.tracing_enabled);
        assert_eq!(snapshot.title, "Inspector");
        assert!(snapshot.widget_count >= 3);
        assert!(snapshot.semantics_count >= 2);
        assert!(snapshot.event_route_count >= 1);
        assert_eq!(snapshot.semantics_nodes.len(), snapshot.semantics_count);
        assert_eq!(snapshot.event_routes.len(), snapshot.event_route_count);
        assert!(
            snapshot
                .event_routes
                .iter()
                .any(|event| event.event_kind == "custom")
        );
    }

    #[test]
    fn adaptive_workspace_bindings_retain_state_and_switch_local_presentations() {
        let classes = Arc::new(Mutex::new(Vec::new()));
        let adaptive = BindingWidget::adaptive_view(
            BindingWidget::label("Compact branch"),
            BindingWidget::label("Medium branch"),
            BindingWidget::label("Expanded branch"),
            300.0,
            600.0,
            Some(BindingStringAction::new({
                let classes = Arc::clone(&classes);
                move |value| {
                    recover_lock(&classes).push(value);
                    Ok(())
                }
            })),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Adaptive", adaptive));
        let mut runtime = app.start().unwrap();
        let window = runtime.window_id_at(0).unwrap();

        runtime
            .handle_event(
                window,
                BindingEvent::Window(BindingWindowEvent::Resized(Size::new(200.0, 300.0))),
            )
            .unwrap();
        let compact = runtime.render_window(window).unwrap();
        assert!(
            compact
                .semantics_names
                .iter()
                .any(|name| name == "Compact branch")
        );

        runtime
            .handle_event(
                window,
                BindingEvent::Window(BindingWindowEvent::Resized(Size::new(800.0, 300.0))),
            )
            .unwrap();
        let expanded = runtime.render_window(window).unwrap();
        assert!(
            expanded
                .semantics_names
                .iter()
                .any(|name| name == "Expanded branch")
        );
        assert!(
            recover_lock(&classes)
                .iter()
                .any(|class| class == "expanded")
        );

        let sidebar = BindingResponsiveSidebarState::new(true, false);
        assert!(sidebar.set_expanded(false));
        assert!(!sidebar.expanded());
        assert!(sidebar.open_overlay());
        assert!(sidebar.overlay_open());

        let master_detail = BindingMasterDetailState::new("master").unwrap();
        assert!(master_detail.show_detail());
        assert_eq!(master_detail.route(), "detail");
    }

    #[test]
    fn binding_virtual_list_model_updates_keyed_rows_without_realizing_the_dataset() {
        let items = (1..=500)
            .map(|key| BindingVirtualListItem::new(key, format!("Row {key}")).unwrap())
            .collect::<Vec<_>>();
        let model = BindingVirtualListModel::new("Rows", items).unwrap();
        let root = BindingWidget::virtual_list(
            "Rows",
            model.clone(),
            28.0,
            0.0,
            None,
            None,
            1.0,
            64,
            true,
            false,
            false,
            true,
            None,
            None,
            None,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Virtual", root));
        let mut runtime = app.start().unwrap();
        let window = runtime.window_id_at(0).unwrap();
        let initial = runtime.render_window(window).unwrap();

        assert_eq!(model.len(), 500);
        assert!(initial.semantics_count < 100);
        assert!(
            model
                .update(BindingVirtualListItem::new(1, "Updated row").unwrap())
                .unwrap()
        );
        let updated = runtime.render_window(window).unwrap();
        assert!(
            updated
                .semantics_names
                .iter()
                .any(|name| name == "Updated row")
        );
    }

    #[test]
    fn binding_pixel_canvas_state_is_thread_safe_and_exports_rgba_bytes() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BindingPixelCanvasState>();

        let state = BindingPixelCanvasState::new();
        state.set_tool("fill").unwrap();
        state.set_brush_color(Color::rgba(0.2, 0.5, 0.9, 1.0));
        state.set_brush_size(2.0);
        state.request_export();
        let root = BindingWidget::pixel_canvas(
            state.clone(),
            "Pixel editor",
            4,
            4,
            None,
            Size::new(240.0, 180.0),
            BindingCanvasViewport::new(Vector::ZERO, 14.0, 0.0),
            true,
            Vec::new(),
        )
        .unwrap();
        let app = BindingApp::new().with_window(BindingWindow::new("Pixels", root));
        let snapshot = app.render_window(0).unwrap();

        assert!(snapshot.command_count > 0);
        let export = state.latest_export().expect("pixel export");
        assert_eq!((export.width, export.height), (4, 4));
        assert_eq!(export.rgba8.len(), 4 * 4 * 4);
    }

    #[test]
    fn binding_render_options_validate_host_facing_names() {
        let options = BindingRenderOptions::new(
            true,
            -2.0,
            true,
            "display-p3",
            "hdr",
            "reinhard",
            "prefer-hdr",
            203.0,
            true,
        )
        .unwrap();
        assert_eq!(options.feather_width(), 0.0);
        assert!(
            BindingRenderOptions::new(
                true, 1.0, true, "rec2020", "auto", "auto", "auto", 203.0, true,
            )
            .is_err()
        );
    }

    #[test]
    fn binding_window_retains_geometry_and_icon_configuration() {
        let window = BindingWindow::new("Configured", BindingWidget::label("Ready"))
            .with_initial_size(Size::new(800.0, 600.0))
            .with_initial_position(Point::new(40.0, 60.0))
            .without_icon();
        assert_eq!(window.initial_size(), Some(Size::new(800.0, 600.0)));
        assert_eq!(window.initial_position(), Some(Point::new(40.0, 60.0)));
    }

    #[test]
    fn raw_mouse_motion_and_window_movement_round_trip() {
        let raw = BindingEvent::RawMouseMotion(BindingRawMouseMotionEvent {
            delta: Vector::new(3.0, -2.0),
            modifiers: BindingModifiers {
                shift: true,
                ..BindingModifiers::default()
            },
        });
        let Event::RawMouseMotion(raw) = raw.into_sui_event().unwrap() else {
            panic!("raw mouse motion event expected");
        };
        assert_eq!(raw.delta, Vector::new(3.0, -2.0));
        assert!(raw.modifiers.shift);

        let moved = BindingEvent::from(&Event::Window(WindowEvent::Moved(Point::new(9.0, 12.0))));
        assert!(matches!(
            moved,
            BindingEvent::Window(BindingWindowEvent::Moved(Point { x: 9.0, y: 12.0 }))
        ));
    }

    #[test]
    fn binding_dock_workspace_preserves_portable_state_and_panels() {
        let layout =
            BindingDockLayout::new(BindingDockNode::tabs([1, 2], Some(1)).unwrap(), [], []);
        let state = BindingDockState::new(layout).unwrap();
        let workspace = BindingWidget::dock_workspace(
            state.clone(),
            [
                BindingDockPanel::new(1, "Files", BindingWidget::label("Files panel")).unwrap(),
                BindingDockPanel::new(2, "Search", BindingWidget::label("Search panel")).unwrap(),
            ],
            "Editor workspace",
        )
        .unwrap();
        let app = BindingApp::new().with_window(BindingWindow::new("Docking", workspace));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.semantics_names.iter().any(|name| name == "Files"));
        assert!(state.activate(2).unwrap());
        assert!(state.hide(1).unwrap());
        assert!(state.snapshot().hidden.contains(&1));
        assert!(state.show(1).unwrap());
        assert!(!state.snapshot().hidden.contains(&1));
    }

    #[test]
    fn binding_rich_document_streams_across_threads_and_renders_structured_blocks() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BindingRichDocument>();

        let document = BindingRichDocument::new("# Report\n\nWaiting");
        let root = BindingWidget::rich_document(document.clone(), None, None, None);
        let app = BindingApp::new().with_window(BindingWindow::new("Document", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let initial = runtime.render_window(window_id).unwrap();
        assert!(initial.semantics_names.iter().any(|name| name == "Report"));

        let producer = document.clone();
        std::thread::spawn(move || {
            producer.append_markdown("\n\n```text\nready\n```");
        })
        .join()
        .unwrap();
        let update = document.last_update();
        assert!(update.append_only);
        assert!(update.reparsed_end > update.reparsed_start);

        let attachment = document.append_attachment(
            "trace.json",
            Some("application/json".to_owned()),
            Some("artifact:trace".to_owned()),
            Some(128),
            Some("Execution trace".to_owned()),
        );
        assert!(attachment > 0);
        let extension = document
            .append_extension(
                "tool-call",
                "Build",
                Some("Completed".to_owned()),
                "cargo test",
                "success",
                false,
                vec![("exit_code".to_owned(), "0".to_owned())],
            )
            .unwrap();
        assert!(extension > attachment);

        let updated = runtime.render_window(window_id).unwrap();
        assert!(updated.semantics_names.iter().any(|name| name == "Build"));
    }

    #[test]
    fn portable_animation_values_timelines_documents_and_editor_round_trip() {
        let zero = BindingAnimationValue::scalar(0.0);
        let ten = BindingAnimationValue::scalar(10.0);
        let transition = BindingTransition::new(zero, ten, 0.0, 1.0, Easing::Linear);
        assert_eq!(transition.sample(0.5).as_scalar(), Some(5.0));

        let mut track = BindingAnimationTrack::new("card", "layer.opacity");
        track.add_keyframe(BindingAnimationKeyframe::new(0.0, zero, Easing::Linear));
        track.add_keyframe(BindingAnimationKeyframe::new(1.0, ten, Easing::Linear));
        let mut clip = BindingAnimationClip::new("fade", 0.0, 1.0);
        clip.add_track(track);
        let mut timeline = BindingAnimationTimeline::new(1.0);
        timeline.add_clip(clip);
        let samples = timeline.sample(0.5);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].target, "card");
        assert_eq!(samples[0].value.as_scalar(), Some(5.0));

        let document = BindingAnimationDocument::new("Card motion", timeline.clone());
        let encoded = document.to_document_format();
        let decoded = BindingAnimationDocument::parse(&encoded).unwrap();
        assert_eq!(decoded.name(), "Card motion");
        assert_eq!(
            decoded.timeline().sample(0.5)[0].value.as_scalar(),
            Some(5.0)
        );

        let mut player = BindingAnimationPlayer::new(&timeline);
        player.play();
        assert_eq!(player.tick(0.25)[0].value.as_scalar(), Some(2.5));

        let mut editor = BindingAnimationEditor::new(decoded);
        assert!(editor.add_keyframe(
            0,
            0,
            BindingAnimationKeyframe::new(0.75, ten, Easing::EaseOut),
        ));
        assert!(editor.can_undo());
        assert!(editor.undo());
        assert!(editor.can_redo());
    }

    #[test]
    fn semantic_queries_and_locator_style_actions_drive_the_runtime() {
        let pressed = Arc::new(AtomicBool::new(false));
        let action = BindingAction::new({
            let pressed = Arc::clone(&pressed);
            move || {
                pressed.store(true, Ordering::Relaxed);
                Ok(())
            }
        });
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Testing",
            BindingWidget::button("Save", Some(action)),
        ));
        let mut runtime = app.start().unwrap();
        let snapshot = runtime.render_window_at(0).unwrap();
        let node = snapshot
            .get_one(Some("button"), Some("Save"), None)
            .unwrap();
        assert!(node.visible());
        assert_eq!(
            snapshot
                .find_nodes(Some("button"), None, None, None, None, Some(true))
                .len(),
            1
        );

        runtime.click_node_at(0, &node).unwrap();
        assert!(pressed.load(Ordering::Relaxed));
    }

    #[cfg(not(feature = "desktop"))]
    #[test]
    fn binding_app_run_reports_missing_desktop_feature() {
        let app = BindingApp::new().with_window(BindingWindow::new(
            "Headless",
            BindingWidget::label("No desktop"),
        ));

        assert!(app.run().unwrap_err().contains("desktop"));
        assert!(app.run_with_handle(|_| {}).unwrap_err().contains("desktop"));
    }
}
