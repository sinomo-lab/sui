#![forbid(unsafe_code)]

//! Native embedded webviews for SUI desktop applications.
//!
//! This crate deliberately stops at the embedding boundary. It synchronizes a
//! native child webview with a retained SUI widget, forwards lifecycle/page/IPC
//! notifications through the widget command API, and exposes a thread-safe
//! control handle. Content policy, navigation decisions, permissions, custom
//! protocols, storage, and other security choices remain with the application.
//!
//! ```no_run
//! use sui::prelude::*;
//! use sui_webview::WebViewHost;
//!
//! fn main() -> Result<()> {
//!     let webviews = WebViewHost::new();
//!     let browser = webviews.url("https://example.com");
//!     webviews.run(App::new().main_window("Web", browser))
//! }
//! ```

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    error::Error as StdError,
    fmt,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use sui::{
    App, ArrangeCtx, Command, CommandKey, CommandSender, Constraints, DesktopExtension,
    DesktopExtensionContext, DesktopPlatform, Event, EventCtx, EventPhase, MeasureCtx,
    PointerEventKind, Result as SuiResult, SemanticsCtx, SemanticsNode, SemanticsRole, Size,
    Widget, WidgetId, WindowId,
};

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use wry;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod backend;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod backend {
    use super::*;

    #[derive(Debug, Default)]
    pub(super) struct Backend;

    impl Backend {
        pub(super) const fn live_len(&self) -> usize {
            0
        }

        pub(super) fn sync(
            &mut self,
            _context: &DesktopExtensionContext<'_>,
            registry: &mut WebViewRegistry,
        ) -> SuiResult<()> {
            if registry.entries.is_empty() {
                return Ok(());
            }
            Err(sui::Error::new(
                "sui-webview supports native desktop targets only (Windows, macOS, and Linux)",
            ))
        }

        pub(super) fn clear(&mut self) {}
    }
}

/// Internal typed delivery used by native webview callbacks.
///
/// Applications normally consume these values through [`WebView::on_event`].
/// The key is public for custom widgets that intentionally reuse the bridge.
pub const WEBVIEW_EVENT: CommandKey<WebViewEvent> = CommandKey::new("sui.webview.event");

const APPLY_PENDING_WEBVIEW_ACTIONS: CommandKey<()> =
    CommandKey::new("sui.webview.apply-pending-actions");

/// Initial content loaded into an embedded webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewContent {
    /// Navigate to a URL.
    Url(String),
    /// Load an HTML string. WRY gives string-loaded pages a null origin.
    Html(String),
}

impl WebViewContent {
    /// Construct URL-backed content.
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

    /// Construct inline HTML content.
    pub fn html(html: impl Into<String>) -> Self {
        Self::Html(html.into())
    }
}

/// Page-loading phase reported by the platform webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLoadState {
    Started,
    Finished,
}

/// A control operation whose platform execution failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebViewOperation {
    LoadUrl,
    LoadHtml,
    EvaluateScript,
    Reload,
    GoBack,
    GoForward,
    Focus,
    FocusParent,
    SetVisible,
    Print,
    Zoom,
}

/// Event delivered to an embedded webview widget on SUI's UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewEvent {
    /// The native child webview was created successfully.
    Created,
    /// A top-level page started or finished loading.
    PageLoad { state: PageLoadState, url: String },
    /// The page's document title changed.
    TitleChanged(String),
    /// JavaScript called `window.ipc.postMessage(...)`.
    ///
    /// IPC is disabled until [`WebView::enable_ipc`] is selected explicitly.
    Ipc { uri: String, body: String },
    /// A queued control operation failed in the platform backend.
    OperationFailed {
        operation: WebViewOperation,
        error: String,
    },
}

/// Error returned when a control handle no longer belongs to a live widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebViewClosed;

impl fmt::Display for WebViewClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the embedded webview widget has been dropped")
    }
}

impl StdError for WebViewClosed {}

#[derive(Debug, Clone)]
struct ControllerBinding {
    window_id: WindowId,
    widget_id: WidgetId,
    commands: Option<CommandSender>,
}

#[derive(Debug)]
struct ControllerState {
    binding: Option<ControllerBinding>,
    actions: VecDeque<WebViewAction>,
    desired_visible: bool,
    closed: bool,
}

impl Default for ControllerState {
    fn default() -> Self {
        Self {
            binding: None,
            actions: VecDeque::new(),
            desired_visible: true,
            closed: false,
        }
    }
}

/// Cloneable, thread-safe control surface for one embedded webview.
///
/// Calls enqueue work for the UI thread and wake the SUI event loop once the
/// widget is attached. Calls made before the first layout pass are retained and
/// applied after native creation.
#[derive(Clone, Default)]
pub struct WebViewHandle {
    state: Arc<Mutex<ControllerState>>,
}

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
impl WebViewHandle {
    /// Navigate to a URL.
    pub fn load_url(&self, url: impl Into<String>) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::LoadUrl(url.into()))
    }

    /// Replace the document with an HTML string.
    pub fn load_html(&self, html: impl Into<String>) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::LoadHtml(html.into()))
    }

    /// Evaluate JavaScript without returning its serialized value.
    pub fn evaluate_script(&self, script: impl Into<String>) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::EvaluateScript(script.into()))
    }

    /// Reload the current page.
    pub fn reload(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::Reload)
    }

    /// Navigate backward in history.
    pub fn go_back(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::GoBack)
    }

    /// Navigate forward in history.
    pub fn go_forward(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::GoForward)
    }

    /// Ask the native webview to take keyboard focus.
    pub fn focus(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::Focus)
    }

    /// Move native focus back to the parent window.
    pub fn focus_parent(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::FocusParent)
    }

    /// Open the platform print UI for the current page.
    pub fn print(&self) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::Print)
    }

    /// Set the page zoom factor.
    pub fn zoom(&self, scale_factor: f64) -> Result<(), WebViewClosed> {
        self.queue(WebViewAction::Zoom(scale_factor))
    }

    /// Show or hide the native child while preserving its page state.
    pub fn set_visible(&self, visible: bool) -> Result<(), WebViewClosed> {
        let wake = {
            let mut state = self.lock_state();
            if state.closed {
                return Err(WebViewClosed);
            }
            state.desired_visible = visible;
            state.binding.clone()
        };
        wake_binding(wake);
        Ok(())
    }

    /// Return the SUI widget identifier after the first layout pass.
    pub fn widget_id(&self) -> Option<WidgetId> {
        self.lock_state()
            .binding
            .as_ref()
            .map(|binding| binding.widget_id)
    }

    /// Return the SUI parent window identifier after the first layout pass.
    pub fn window_id(&self) -> Option<WindowId> {
        self.lock_state()
            .binding
            .as_ref()
            .map(|binding| binding.window_id)
    }

    /// Return whether the owning widget still exists.
    pub fn is_open(&self) -> bool {
        !self.lock_state().closed
    }

    fn queue(&self, action: WebViewAction) -> Result<(), WebViewClosed> {
        let wake = {
            let mut state = self.lock_state();
            if state.closed {
                return Err(WebViewClosed);
            }
            state.actions.push_back(action);
            state.binding.clone()
        };
        wake_binding(wake);
        Ok(())
    }

    fn attach(&self, window_id: WindowId, widget_id: WidgetId) {
        let mut state = self.lock_state();
        state.binding = Some(ControllerBinding {
            window_id,
            widget_id,
            commands: None,
        });
    }

    fn bind_commands(&self, commands: &CommandSender) {
        let wake = {
            let mut state = self.lock_state();
            let has_actions = !state.actions.is_empty();
            let binding = state.binding.as_mut();
            if let Some(binding) = binding {
                binding.commands = Some(commands.clone());
            }
            has_actions.then(|| state.binding.clone()).flatten()
        };
        wake_binding(wake);
    }

    fn desired_visible(&self) -> bool {
        self.lock_state().desired_visible
    }

    fn set_initial_visibility(&self, visible: bool) {
        self.lock_state().desired_visible = visible;
    }

    fn take_actions(&self) -> VecDeque<WebViewAction> {
        std::mem::take(&mut self.lock_state().actions)
    }

    fn detach(&self) {
        let mut state = self.lock_state();
        state.closed = true;
        state.binding = None;
        state.actions.clear();
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ControllerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl fmt::Debug for WebViewHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.lock_state();
        formatter
            .debug_struct("WebViewHandle")
            .field(
                "window_id",
                &state.binding.as_ref().map(|binding| binding.window_id),
            )
            .field(
                "widget_id",
                &state.binding.as_ref().map(|binding| binding.widget_id),
            )
            .field("pending_action_count", &state.actions.len())
            .field("desired_visible", &state.desired_visible)
            .field("closed", &state.closed)
            .finish()
    }
}

fn wake_binding(binding: Option<ControllerBinding>) {
    let Some(ControllerBinding {
        window_id,
        widget_id,
        commands: Some(commands),
    }) = binding
    else {
        return;
    };
    commands.send_widget(window_id, widget_id, APPLY_PENDING_WEBVIEW_ACTIONS, ());
}

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
#[derive(Debug)]
enum WebViewAction {
    LoadUrl(String),
    LoadHtml(String),
    EvaluateScript(String),
    Reload,
    GoBack,
    GoForward,
    Focus,
    FocusParent,
    Print,
    Zoom(f64),
}

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
impl WebViewAction {
    const fn operation(&self) -> WebViewOperation {
        match self {
            Self::LoadUrl(_) => WebViewOperation::LoadUrl,
            Self::LoadHtml(_) => WebViewOperation::LoadHtml,
            Self::EvaluateScript(_) => WebViewOperation::EvaluateScript,
            Self::Reload => WebViewOperation::Reload,
            Self::GoBack => WebViewOperation::GoBack,
            Self::GoForward => WebViewOperation::GoForward,
            Self::Focus => WebViewOperation::Focus,
            Self::FocusParent => WebViewOperation::FocusParent,
            Self::Print => WebViewOperation::Print,
            Self::Zoom(_) => WebViewOperation::Zoom,
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
type BuilderConfigurator =
    dyn for<'a> Fn(wry::WebViewBuilder<'a>) -> wry::WebViewBuilder<'a> + 'static;

struct PendingRegistration {
    content: WebViewContent,
    ipc_enabled: bool,
    emit_events: bool,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    configure: Option<Rc<BuilderConfigurator>>,
}

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
struct Registration {
    window_id: WindowId,
    widget_id: WidgetId,
    content: WebViewContent,
    ipc_enabled: bool,
    emit_events: bool,
    handle: WebViewHandle,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    configure: Option<Rc<BuilderConfigurator>>,
}

#[derive(Default)]
struct WebViewRegistry {
    entries: HashMap<WidgetId, Registration>,
}

type SharedRegistry = Rc<RefCell<WebViewRegistry>>;

/// Owns native webviews and connects them to SUI's desktop event loop.
///
/// Create widgets from this host, then either call [`run`](Self::run) or attach
/// it to a custom [`DesktopPlatform`] with [`attach`](Self::attach).
pub struct WebViewHost {
    registry: SharedRegistry,
    backend: backend::Backend,
}

impl WebViewHost {
    /// Create an empty native webview host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a URL-backed webview widget attached to this host.
    pub fn url(&self, url: impl Into<String>) -> WebView {
        WebView::new(self, WebViewContent::url(url))
    }

    /// Construct an inline-HTML webview widget attached to this host.
    pub fn html(&self, html: impl Into<String>) -> WebView {
        WebView::new(self, WebViewContent::html(html))
    }

    /// Use an application-owned WRY context for all views in this host.
    ///
    /// This lets the application choose data-directory and context behavior
    /// without `sui-webview` imposing storage policy. The host retains the
    /// context for at least as long as every native view it creates.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub fn with_web_context(mut self, context: wry::WebContext) -> Self {
        self.backend.set_web_context(context);
        self
    }

    /// Attach this host to an already configured desktop platform.
    pub fn attach(self, platform: DesktopPlatform) -> DesktopPlatform {
        platform.with_extension(self)
    }

    /// Run a SUI application with native webview support.
    pub fn run(self, app: App) -> SuiResult<()> {
        app.run_with_platform(self.attach(DesktopPlatform::new()))
    }

    /// Run with native webview support and receive the normal SUI UI handle.
    pub fn run_with_handle(self, app: App, on_ready: impl FnOnce(sui::UiHandle)) -> SuiResult<()> {
        app.run_with_platform_and_handle(self.attach(DesktopPlatform::new()), on_ready)
    }
}

impl Default for WebViewHost {
    fn default() -> Self {
        Self {
            registry: Rc::new(RefCell::new(WebViewRegistry::default())),
            backend: backend::Backend::default(),
        }
    }
}

impl fmt::Debug for WebViewHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let registrations = self
            .registry
            .try_borrow()
            .map(|registry| registry.entries.len())
            .ok();
        formatter
            .debug_struct("WebViewHost")
            .field("registration_count", &registrations)
            .field("live_view_count", &self.backend.live_len())
            .finish()
    }
}

impl DesktopExtension for WebViewHost {
    fn update(&mut self, context: DesktopExtensionContext<'_>) -> SuiResult<()> {
        self.backend.sync(&context, &mut self.registry.borrow_mut())
    }

    fn suspended(&mut self) -> SuiResult<()> {
        self.backend.clear();
        Ok(())
    }

    fn poll_interval(&self) -> Option<Duration> {
        #[cfg(target_os = "linux")]
        {
            if self
                .registry
                .try_borrow()
                .is_ok_and(|registry| !registry.entries.is_empty())
            {
                return Some(Duration::from_millis(16));
            }
        }
        None
    }
}

type EventHandler = Box<dyn FnMut(&mut EventCtx, &WebViewEvent)>;

/// Retained SUI widget backed by a native child webview.
pub struct WebView {
    registry: SharedRegistry,
    pending: Option<PendingRegistration>,
    registered_id: Option<WidgetId>,
    handle: WebViewHandle,
    preferred_size: Size,
    accessible_name: String,
    on_event: Option<EventHandler>,
}

impl WebView {
    /// Construct a webview with explicit initial content.
    pub fn new(host: &WebViewHost, content: WebViewContent) -> Self {
        let handle = WebViewHandle::default();
        Self {
            registry: Rc::clone(&host.registry),
            pending: Some(PendingRegistration {
                content,
                ipc_enabled: false,
                emit_events: false,
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                configure: None,
            }),
            registered_id: None,
            handle,
            preferred_size: Size::new(640.0, 480.0),
            accessible_name: "Web content".to_string(),
            on_event: None,
        }
    }

    /// Return a cloneable control handle.
    pub fn handle(&self) -> WebViewHandle {
        self.handle.clone()
    }

    /// Set the size used when parent constraints are not tight.
    pub fn preferred_size(mut self, size: Size) -> Self {
        self.preferred_size = Size::new(size.width.max(0.0), size.height.max(0.0));
        self
    }

    /// Set the semantic name exposed for the native document placeholder.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = name.into();
        self
    }

    /// Choose whether the child is visible when first created.
    pub fn initially_visible(self, visible: bool) -> Self {
        self.handle.set_initial_visibility(visible);
        self
    }

    /// Enable WRY's JavaScript-to-Rust IPC bridge.
    ///
    /// This is opt-in because every loaded document can call
    /// `window.ipc.postMessage`. The application is responsible for validating
    /// the current origin and message body in its event handler.
    pub fn enable_ipc(mut self) -> Self {
        if let Some(pending) = &mut self.pending {
            pending.ipc_enabled = true;
        }
        self
    }

    /// Receive creation, page, title, IPC, and operation-error events on the
    /// SUI UI thread.
    pub fn on_event(mut self, handler: impl FnMut(&mut EventCtx, &WebViewEvent) + 'static) -> Self {
        if let Some(pending) = &mut self.pending {
            pending.emit_events = true;
        }
        self.on_event = Some(Box::new(handler));
        self
    }

    /// Apply application-owned WRY builder configuration before native
    /// creation.
    ///
    /// The callback is invoked again if a suspended or structurally detached
    /// native child must be recreated. SUI applies its current bounds and
    /// visibility after this callback. The callback runs after the optional SUI
    /// page/title/IPC handlers, so selecting the same WRY handler intentionally
    /// replaces that part of the built-in event bridge.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub fn configure<F>(mut self, configure: F) -> Self
    where
        F: for<'a> Fn(wry::WebViewBuilder<'a>) -> wry::WebViewBuilder<'a> + 'static,
    {
        if let Some(pending) = &mut self.pending {
            pending.configure = Some(Rc::new(configure));
        }
        self
    }

    fn ensure_registered(&mut self, window_id: WindowId, widget_id: WidgetId) {
        if self.registered_id == Some(widget_id) {
            return;
        }

        let Some(pending) = self.pending.take() else {
            return;
        };
        self.handle.attach(window_id, widget_id);
        self.registry.borrow_mut().entries.insert(
            widget_id,
            Registration {
                window_id,
                widget_id,
                content: pending.content,
                ipc_enabled: pending.ipc_enabled,
                emit_events: pending.emit_events,
                handle: self.handle.clone(),
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                configure: pending.configure,
            },
        );
        self.registered_id = Some(widget_id);
    }
}

impl Drop for WebView {
    fn drop(&mut self) {
        if let Some(widget_id) = self.registered_id
            && let Ok(mut registry) = self.registry.try_borrow_mut()
        {
            registry.entries.remove(&widget_id);
        }
        self.handle.detach();
    }
}

impl Widget for WebView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() == EventPhase::Target
            && matches!(event, Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down)
        {
            ctx.request_focus();
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(APPLY_PENDING_WEBVIEW_ACTIONS).is_some() {
            ctx.set_handled();
        }
        if let Some(event) = command.get(WEBVIEW_EVENT) {
            ctx.set_handled();
            if let Some(handler) = &mut self.on_event {
                handler(ctx, event);
            }
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.preferred_size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, _bounds: sui::Rect) {
        self.ensure_registered(ctx.window_id(), ctx.widget_id());
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Document, ctx.bounds());
        node.name = Some(self.accessible_name.clone());
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, _ctx: &mut EventCtx, focused: bool) {
        if focused {
            let _ = self.handle.focus();
        }
    }
}

impl fmt::Debug for WebView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebView")
            .field("registered_id", &self.registered_id)
            .field("preferred_size", &self.preferred_size)
            .field("accessible_name", &self.accessible_name)
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

/// Common imports for applications using embedded webviews.
pub mod prelude {
    pub use crate::{
        PageLoadState, WebView, WebViewClosed, WebViewContent, WebViewEvent, WebViewHandle,
        WebViewHost, WebViewOperation,
    };
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use sui::{CommandDelivery, CommandTarget, Runtime};

    use super::*;

    fn render(widget: WebView) -> (Runtime, WindowId) {
        let mut runtime = App::new()
            .main_window("webview test", widget)
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).expect("render should succeed");
        (runtime, window_id)
    }

    #[test]
    fn control_handle_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WebViewHandle>();
    }

    #[test]
    fn widget_registers_and_exposes_document_semantics() {
        let host = WebViewHost::new();
        let widget = host
            .url("https://example.com")
            .preferred_size(Size::new(320.0, 180.0))
            .accessible_name("Preview");
        let (mut runtime, window_id) = render(widget);

        assert_eq!(host.registry.borrow().entries.len(), 1);
        let output = runtime
            .render(window_id)
            .expect("second render should succeed");
        let document = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Document)
            .expect("document semantics should exist");
        assert_eq!(document.name.as_deref(), Some("Preview"));
    }

    #[test]
    fn handle_queues_before_native_creation_and_closes_with_widget() {
        let host = WebViewHost::new();
        let widget = host.html("<p>initial</p>");
        let handle = widget.handle();
        handle
            .load_html("<p>replacement</p>")
            .expect("pre-creation action should queue");
        let (mut runtime, window_id) = render(widget);

        let registration = host.registry.borrow();
        let entry = registration
            .entries
            .values()
            .next()
            .expect("widget should register");
        assert_eq!(entry.handle.lock_state().actions.len(), 1);
        drop(registration);

        runtime
            .remove_window(window_id)
            .expect("window removal should succeed");
        assert_eq!(handle.reload(), Err(WebViewClosed));
        assert!(host.registry.borrow().entries.is_empty());
    }

    #[test]
    fn typed_native_events_reach_the_widget_handler() {
        let host = WebViewHost::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_for_handler = Arc::clone(&received);
        let widget = host.url("about:blank").on_event(move |_ctx, event| {
            received_for_handler
                .lock()
                .expect("event store should lock")
                .push(event.clone());
        });
        let (mut runtime, window_id) = render(widget);
        let widget_id = host
            .registry
            .borrow()
            .entries
            .keys()
            .next()
            .copied()
            .expect("widget should register");

        runtime.handle_command(
            CommandTarget::Widget {
                window_id,
                widget_id,
            },
            CommandDelivery::Directed,
            WEBVIEW_EVENT,
            WebViewEvent::TitleChanged("Example".to_string()),
        );

        assert_eq!(
            received.lock().expect("event store should lock").as_slice(),
            &[WebViewEvent::TitleChanged("Example".to_string())]
        );
    }
}
