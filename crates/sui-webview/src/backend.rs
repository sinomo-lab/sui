use std::collections::{HashMap, HashSet};

use sui::{
    CommandSender, DesktopExtensionContext, DesktopWindow, Error, Rect as SuiRect, Result,
    WidgetGraphSnapshot, WidgetId, WindowId,
};
use wry::{
    Rect, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

use super::{
    PageLoadState, Registration, WEBVIEW_EVENT, WebViewAction, WebViewEvent, WebViewOperation,
    WebViewRegistry,
};

#[derive(Default)]
pub(super) struct Backend {
    views: HashMap<WidgetId, LiveWebView>,
    web_context: Option<wry::WebContext>,
    #[cfg(target_os = "linux")]
    gtk_initialized: bool,
}

impl Backend {
    pub(super) fn live_len(&self) -> usize {
        self.views.len()
    }

    pub(super) fn set_web_context(&mut self, context: wry::WebContext) {
        self.web_context = Some(context);
    }

    pub(super) fn sync(
        &mut self,
        context: &DesktopExtensionContext<'_>,
        registry: &mut WebViewRegistry,
    ) -> Result<()> {
        if registry.entries.is_empty() {
            self.views.clear();
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        self.prepare_gtk()?;

        let mut graphs = HashMap::<WindowId, WidgetGraphSnapshot>::new();
        let mut active = HashSet::new();

        for registration in registry.entries.values_mut() {
            registration.handle.bind_commands(context.command_sender());

            let Some(parent) = context.window(registration.window_id) else {
                continue;
            };
            if let std::collections::hash_map::Entry::Vacant(entry) =
                graphs.entry(registration.window_id)
            {
                entry.insert(context.runtime().widget_graph(registration.window_id)?);
            }
            let Some(node) = graphs[&registration.window_id]
                .nodes
                .iter()
                .find(|node| node.id == registration.widget_id)
            else {
                continue;
            };

            active.insert(registration.widget_id);
            let bounds = native_bounds(node.geometry.layout_bounds);
            let desired_visible = registration.handle.desired_visible();

            if self
                .views
                .get(&registration.widget_id)
                .is_some_and(|live| live.window_id != registration.window_id)
            {
                self.views.remove(&registration.widget_id);
            }

            let Some(bounds) = bounds else {
                if let Some(live) = self.views.get_mut(&registration.widget_id) {
                    set_visibility(live, false, context.command_sender(), registration);
                }
                continue;
            };

            if !self.views.contains_key(&registration.widget_id) {
                let live = build_webview(
                    parent,
                    bounds,
                    desired_visible,
                    context.command_sender(),
                    registration,
                    self.web_context.as_mut(),
                )?;
                self.views.insert(registration.widget_id, live);
                emit_event(
                    context.command_sender(),
                    registration,
                    WebViewEvent::Created,
                );
            }

            if let Some(live) = self.views.get_mut(&registration.widget_id) {
                if live.bounds != bounds {
                    live.view.set_bounds(bounds).map_err(|error| {
                        Error::new(format!(
                            "failed to update embedded webview {} bounds: {error}",
                            registration.widget_id.get()
                        ))
                    })?;
                    live.bounds = bounds;
                }
                set_visibility(
                    live,
                    desired_visible,
                    context.command_sender(),
                    registration,
                );
                apply_actions(live, context.command_sender(), registration);
            }
        }

        self.views
            .retain(|widget_id, _view| active.contains(widget_id));

        #[cfg(target_os = "linux")]
        pump_gtk();

        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.views.clear();
        #[cfg(target_os = "linux")]
        if self.gtk_initialized {
            pump_gtk();
        }
    }

    #[cfg(target_os = "linux")]
    fn prepare_gtk(&mut self) -> Result<()> {
        use gtk::prelude::DisplayExtManual;

        if !self.gtk_initialized {
            gtk::init().map_err(|error| {
                Error::new(format!(
                    "failed to initialize GTK for embedded webviews: {error}"
                ))
            })?;
            let display = gtk::gdk::Display::default()
                .ok_or_else(|| Error::new("GTK did not provide a display for embedded webviews"))?;
            if display.backend().is_wayland() {
                return Err(Error::new(
                    "WRY child webviews attached to winit windows currently require X11 on Linux; Wayland is not supported by this embedding path",
                ));
            }
            self.gtk_initialized = true;
        }
        pump_gtk();
        Ok(())
    }
}

impl std::fmt::Debug for Backend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebViewBackend")
            .field("live_view_count", &self.views.len())
            .finish()
    }
}

struct LiveWebView {
    window_id: WindowId,
    view: wry::WebView,
    bounds: Rect,
    visible: bool,
}

fn native_bounds(bounds: SuiRect) -> Option<Rect> {
    let values = [bounds.x(), bounds.y(), bounds.width(), bounds.height()];
    if values.iter().any(|value| !value.is_finite()) || bounds.is_empty() {
        return None;
    }

    Some(Rect {
        position: LogicalPosition::new(f64::from(bounds.x()), f64::from(bounds.y())).into(),
        // A native X11 child cannot have a zero physical extent. Preserve the
        // logical placement while ensuring tiny positive widgets remain valid.
        size: LogicalSize::new(
            f64::from(bounds.width()).max(1.0),
            f64::from(bounds.height()).max(1.0),
        )
        .into(),
    })
}

fn build_webview(
    parent: DesktopWindow<'_>,
    bounds: Rect,
    visible: bool,
    commands: &CommandSender,
    registration: &Registration,
    web_context: Option<&mut wry::WebContext>,
) -> Result<LiveWebView> {
    let mut builder = match web_context {
        Some(context) => WebViewBuilder::new_with_web_context(context),
        None => WebViewBuilder::new(),
    };
    builder = match &registration.content {
        super::WebViewContent::Url(url) => builder.with_url(url),
        super::WebViewContent::Html(html) => builder.with_html(html),
    };

    if registration.emit_events {
        let page_commands = commands.clone();
        let page_window_id = registration.window_id;
        let page_widget_id = registration.widget_id;
        builder = builder.with_on_page_load_handler(move |state, url| {
            let state = match state {
                wry::PageLoadEvent::Started => PageLoadState::Started,
                wry::PageLoadEvent::Finished => PageLoadState::Finished,
            };
            page_commands.send_widget(
                page_window_id,
                page_widget_id,
                WEBVIEW_EVENT,
                WebViewEvent::PageLoad { state, url },
            );
        });

        let title_commands = commands.clone();
        let title_window_id = registration.window_id;
        let title_widget_id = registration.widget_id;
        builder = builder.with_document_title_changed_handler(move |title| {
            title_commands.send_widget(
                title_window_id,
                title_widget_id,
                WEBVIEW_EVENT,
                WebViewEvent::TitleChanged(title),
            );
        });
    }

    if registration.emit_events && registration.ipc_enabled {
        let ipc_commands = commands.clone();
        let ipc_window_id = registration.window_id;
        let ipc_widget_id = registration.widget_id;
        builder = builder.with_ipc_handler(move |request| {
            let uri = request.uri().to_string();
            let body = request.into_body();
            ipc_commands.send_widget(
                ipc_window_id,
                ipc_widget_id,
                WEBVIEW_EVENT,
                WebViewEvent::Ipc { uri, body },
            );
        });
    }

    if let Some(configure) = &registration.configure {
        builder = configure(builder);
    }

    let view = builder
        .with_bounds(bounds)
        .with_visible(visible)
        .build_as_child(parent.host_window())
        .map_err(|error| {
            Error::new(format!(
                "failed to create embedded webview {}: {error}",
                registration.widget_id.get()
            ))
        })?;

    Ok(LiveWebView {
        window_id: parent.id(),
        view,
        bounds,
        visible,
    })
}

fn set_visibility(
    live: &mut LiveWebView,
    visible: bool,
    commands: &CommandSender,
    registration: &Registration,
) {
    if live.visible == visible {
        return;
    }
    match live.view.set_visible(visible) {
        Ok(()) => live.visible = visible,
        Err(error) => emit_operation_error(
            commands,
            registration,
            WebViewOperation::SetVisible,
            format!("failed to set embedded webview visibility: {error}"),
        ),
    }
}

fn apply_actions(live: &mut LiveWebView, commands: &CommandSender, registration: &Registration) {
    for action in registration.handle.take_actions() {
        let operation = action.operation();
        let result = match action {
            WebViewAction::LoadUrl(url) => live.view.load_url(&url),
            WebViewAction::LoadHtml(html) => live.view.load_html(&html),
            WebViewAction::EvaluateScript(script) => live.view.evaluate_script(&script),
            WebViewAction::Reload => live.view.reload(),
            WebViewAction::GoBack => live.view.go_back(),
            WebViewAction::GoForward => live.view.go_forward(),
            WebViewAction::Focus => live.view.focus(),
            WebViewAction::FocusParent => live.view.focus_parent(),
            WebViewAction::Print => live.view.print(),
            WebViewAction::Zoom(scale_factor) => live.view.zoom(scale_factor),
        };
        if let Err(error) = result {
            emit_operation_error(commands, registration, operation, error.to_string());
        }
    }
}

fn emit_operation_error(
    commands: &CommandSender,
    registration: &Registration,
    operation: WebViewOperation,
    error: String,
) {
    emit_event(
        commands,
        registration,
        WebViewEvent::OperationFailed { operation, error },
    );
}

fn emit_event(commands: &CommandSender, registration: &Registration, event: WebViewEvent) {
    if registration.emit_events {
        commands.send_widget(
            registration.window_id,
            registration.widget_id,
            WEBVIEW_EVENT,
            event,
        );
    }
}

#[cfg(target_os = "linux")]
fn pump_gtk() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}
