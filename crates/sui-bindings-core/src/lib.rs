#![forbid(unsafe_code)]

use std::{
    collections::VecDeque,
    fmt,
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use sui::{
    ArrangeCtx, Axis, Border, Breadcrumb, BreadcrumbItem, Brush, BusyIndicator, Button, Checkbox,
    Color, ColorSpace, ColorSwatch, Constraints, CustomEvent, DetailRow, DpiInfo,
    EXTERNAL_WAKE_KIND, EmptyState, Event, EventCtx, EventPhase, Flex, FontHandle, Icon,
    IconButton, IconGlyph, Image, ImageFit, ImageHandle, ImageSource, ImeEvent, Insets,
    InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, KeyboardEvent, Label,
    Link, ListItem, ListView, MeasureCtx, Modifiers, NumberInput, PaintCtx, Path, Point,
    PointerButton, PointerButtons, PointerEvent, PointerEventKind, PointerKind, ProgressBar,
    RadioButton, RadioGroup, Rect, RegisteredFont, RegisteredImage, RichText, Runtime,
    SceneCommand, ScrollDelta, ScrollView, SegmentedControl, SegmentedControlItem, Select,
    SemanticTone, SemanticsCtx, SemanticsNode, SemanticsRole, SemanticsValue, Separator,
    ShadowParams, SignalMeter, Size, Slider, StatusBadge, StatusBar, StatusBarSegment, StrokeStyle,
    Surface, SurfaceBorder, SurfaceElevation, SurfaceRole, Switch, Table, TableColumn,
    TableColumnAlignment, TableRow, TextArea, TextInput, TextSpan, TextStyle, TimerToken,
    ToggleState, Toolbar, Transform, Vector, Widget, WidgetId, WidgetPod, WidgetPodMutVisitor,
    WidgetPodVisitor, WidgetShader, WindowBuilder, WindowEvent, WindowId,
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
}

impl BindingUiHandle {
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

#[derive(Debug, Clone)]
pub struct BindingState {
    inner: Arc<BindingStateInner>,
}

#[derive(Debug)]
struct BindingStateInner {
    value: Mutex<BindingValue>,
    ui_handle: Mutex<Option<BindingUiHandle>>,
}

impl BindingState {
    pub fn new(value: impl Into<BindingValue>) -> Self {
        Self {
            inner: Arc::new(BindingStateInner {
                value: Mutex::new(value.into()),
                ui_handle: Mutex::new(None),
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

    fn set_immediate(&self, value: BindingValue) {
        *recover_lock(&self.inner.value) = value;
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
pub struct BindingStringAction {
    callback: Arc<dyn Fn(String) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
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

#[derive(Clone)]
pub struct BindingWidget {
    inner: Arc<BindingWidgetKind>,
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
            BindingWidgetKind::TextArea { .. } => f.debug_tuple("BindingWidget::TextArea").finish(),
            BindingWidgetKind::RichText { .. } => f.debug_tuple("BindingWidget::RichText").finish(),
            BindingWidgetKind::Image { .. } => f.debug_tuple("BindingWidget::Image").finish(),
            BindingWidgetKind::ColorSwatch { .. } => {
                f.debug_tuple("BindingWidget::ColorSwatch").finish()
            }
            BindingWidgetKind::Separator { .. } => {
                f.debug_tuple("BindingWidget::Separator").finish()
            }
            BindingWidgetKind::EmptyState { title, action, .. } => f
                .debug_struct("BindingWidget::EmptyState")
                .field("title", title)
                .field("action", action)
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
            BindingWidgetKind::Foreign { .. } => f.debug_tuple("BindingWidget::Foreign").finish(),
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
        Self::from_kind(BindingWidgetKind::Foreign { callbacks })
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
            BindingWidgetKind::TextInput { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::TextArea { name, value, .. } => {
                name.bind_ui_handle(handle);
                value.bind_ui_handle(handle);
            }
            BindingWidgetKind::RichText { .. } => {}
            BindingWidgetKind::Image { .. } => {}
            BindingWidgetKind::ColorSwatch { .. } => {}
            BindingWidgetKind::Separator { .. } => {}
            BindingWidgetKind::EmptyState { action, .. } => {
                if let Some(action) = action {
                    action.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Surface { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::ExternalSurface { .. } => {}
            BindingWidgetKind::Toolbar { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::ScrollView { child, .. } => child.bind_ui_handle(handle),
            BindingWidgetKind::Flex { children, .. } => {
                for child in children {
                    child.bind_ui_handle(handle);
                }
            }
            BindingWidgetKind::Foreign { .. } => {}
        }
    }

    fn into_runtime_widget(&self, errors: ForeignErrorSink) -> BindingRuntimeWidget {
        match self.inner.as_ref() {
            BindingWidgetKind::Label { text } => {
                let label = Label::dynamic(text.resolve(), {
                    let text = text.clone();
                    move || text.resolve()
                });
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
                BindingRuntimeWidget::new(button)
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
                BindingRuntimeWidget::new(button)
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
                BindingRuntimeWidget::new(link)
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
                    inner: checkbox,
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
                    inner: switch,
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
                    inner: radio,
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
                BindingRuntimeWidget::new(radio_group)
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
                BindingRuntimeWidget::new(control)
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
                BindingRuntimeWidget::new(breadcrumb)
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
                BindingRuntimeWidget::new(list_view)
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
                BindingRuntimeWidget::new(table)
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
                BindingRuntimeWidget::new(signal_meter)
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
                BindingRuntimeWidget::new(badge)
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
                BindingRuntimeWidget::new(status_bar)
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
                BindingRuntimeWidget::new(detail_row)
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
                BindingRuntimeWidget::new(slider)
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
                BindingRuntimeWidget::new(number_input)
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
                BindingRuntimeWidget::new(select)
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
            }),
            BindingWidgetKind::BusyIndicator { name, label, size } => {
                BindingRuntimeWidget::new(BindingBusyIndicatorWidget {
                    name: name.clone(),
                    label: label.clone(),
                    size: *size,
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
                    inner: text_input,
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
                    inner: text_area,
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
                BindingRuntimeWidget::new(image)
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
                BindingRuntimeWidget::new(swatch)
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
                BindingRuntimeWidget::new(separator)
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
                BindingRuntimeWidget::new(empty_state)
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
                BindingRuntimeWidget::new(surface)
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
                BindingRuntimeWidget::new(toolbar)
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
                BindingRuntimeWidget::new(scroll_view)
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
            BindingWidgetKind::Foreign { callbacks } => BindingRuntimeWidget::new(
                ForeignWidget::from_arc(Arc::clone(callbacks)).with_error_sink(errors),
            ),
        }
    }
}

struct BindingBusyIndicatorWidget {
    name: BindingText,
    label: Option<BindingText>,
    size: f32,
}

impl BindingBusyIndicatorWidget {
    fn inner(&self) -> BusyIndicator {
        let mut indicator = BusyIndicator::new(self.name.resolve()).size(self.size);
        if let Some(label) = &self.label {
            indicator = indicator.label(label.resolve());
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
}

impl BindingProgressBarWidget {
    fn inner(&self) -> ProgressBar {
        ProgressBar::new(self.name.resolve())
            .range(self.min, self.max)
            .value(self.value.resolve())
            .show_value(self.show_value)
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
        let external_wake = matches!(
            event,
            Event::Custom(CustomEvent { kind, .. }) if kind == EXTERNAL_WAKE_KIND
        );
        if external_wake {
            let drained = self.drain_ui_tasks(ctx);
            if drained == 0 {
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
            }
        }

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

#[derive(Debug, Clone)]
pub struct BindingWindow {
    title: String,
    root: BindingWidget,
}

impl BindingWindow {
    pub fn new(title: impl Into<String>, root: BindingWidget) -> Self {
        Self {
            title: title.into(),
            root,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn root(&self) -> &BindingWidget {
        &self.root
    }
}

#[derive(Debug, Clone, Default)]
pub struct BindingApp {
    windows: Vec<BindingWindow>,
    font_resources: Vec<BindingFontResource>,
    next_font_slot: u64,
    image_resources: Vec<BindingImageResource>,
    next_image_slot: u64,
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
        let ui_handle = ui_tasks.handle();
        let mut runtime = Runtime::new();
        let mut window_ids = Vec::with_capacity(self.windows.len());

        self.register_font_resources(&mut runtime)?;
        self.register_image_resources(&mut runtime)?;

        for window in &self.windows {
            window.root.bind_ui_handle(&ui_handle);
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(self.errors.clone()),
                ui_tasks.clone(),
            );
            let window_id = runtime
                .add_window(WindowBuilder::new().title(window.title.clone()).root(root))
                .map_err(|error| error.to_string())?;
            window_ids.push(BindingWindowId::from(window_id));
        }

        Ok(BindingRuntime {
            runtime,
            window_ids,
            ui_tasks,
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
        let ui_handle = ui_tasks.handle();
        let mut app = SuiApp::new();

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
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(self.errors.clone()),
                ui_tasks.clone(),
            );
            app = app.window(SuiWindow::new(window.title.clone()).root(root));
        }

        let tasks_for_waker = ui_tasks.clone();
        app.run_with_handle(move |native_ui| {
            tasks_for_waker.set_waker(move || native_ui.wake());
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
        let window_id = runtime
            .add_window(
                WindowBuilder::new()
                    .title(window.title.clone())
                    .root(window.root.into_runtime_widget(self.errors.clone())),
            )
            .map_err(|error| error.to_string())?;
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
    let mut buffer = vec![0; reader.output_buffer_size()];
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
}

impl BindingRuntime {
    pub fn ui_handle(&self) -> BindingUiHandle {
        self.ui_tasks.handle()
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

    pub fn handle_event_at(&mut self, index: usize, event: BindingEvent) -> Result<(), String> {
        let window_id = self.window_id_at(index)?;
        self.handle_event(window_id, event)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRenderSnapshot {
    pub command_count: usize,
    pub semantics_count: usize,
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
        .map(|node| binding_semantics_value_text(node.value.as_ref()))
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
    ScaleFactorChanged {
        scale_factor: f64,
        raw_dpi: Option<f32>,
        suggested_size: Option<Size>,
    },
    Focused(bool),
    Occluded(bool),
    RedrawRequested,
}

impl From<&WindowEvent> for BindingWindowEvent {
    fn from(value: &WindowEvent) -> Self {
        match value {
            WindowEvent::CloseRequested => Self::CloseRequested,
            WindowEvent::Resized(size) => Self::Resized(*size),
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
            WindowEvent::RedrawRequested => Self::RedrawRequested,
        }
    }
}

impl From<BindingWindowEvent> for WindowEvent {
    fn from(value: BindingWindowEvent) -> Self {
        match value {
            BindingWindowEvent::CloseRequested => Self::CloseRequested,
            BindingWindowEvent::Resized(size) => Self::Resized(size),
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
            Event::Keyboard(event) => Self::Keyboard(BindingKeyboardEvent::from(event)),
            Event::Ime(event) => Self::Ime(BindingImeEvent::from(event)),
            Event::Window(event) => Self::Window(BindingWindowEvent::from(event)),
            Event::Custom(event) => Self::Custom(BindingCustomEvent::from(event)),
            Event::Drag(_) => Self::Unsupported {
                kind: "drag".to_string(),
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
                {
                    if handle.is_empty() {
                        return Err(ExternalTextureValidationError::EmptySyncHandle);
                    }
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

    pub fn set_handled(&mut self) {
        self.inner.set_handled();
    }

    pub fn request_focus(&mut self) {
        self.inner.request_focus();
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

#[derive(Debug, Clone, PartialEq)]
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

impl Default for PaintCommandBuilder {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            stack: PaintStackState::default(),
        }
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
            "3",
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
        assert_eq!(callbacks.measures.load(Ordering::Relaxed), 1);
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
