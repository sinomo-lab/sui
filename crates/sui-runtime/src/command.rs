use std::{
    any::{Any, TypeId, type_name},
    collections::VecDeque,
    fmt,
    marker::PhantomData,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use sui_core::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, Rect, WidgetId, WindowId,
};

static NEXT_COMMAND_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A process-local, strongly typed command identifier.
///
/// The stable name is retained for diagnostics and foreign-language adapters;
/// Rust delivery additionally verifies the payload's [`TypeId`].
pub struct CommandKey<T: 'static> {
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<T: 'static> CommandKey<T> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    fn erased(self) -> ErasedCommandKey {
        ErasedCommandKey {
            name: self.name,
            payload_type: TypeId::of::<T>(),
            payload_type_name: type_name::<T>(),
        }
    }
}

impl<T: 'static> Clone for CommandKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for CommandKey<T> {}

impl<T: 'static> fmt::Debug for CommandKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandKey")
            .field("name", &self.name)
            .field("payload", &type_name::<T>())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandTarget {
    Widget {
        window_id: WindowId,
        widget_id: WidgetId,
    },
    FocusedWidget(WindowId),
    Window(WindowId),
    Application,
}

impl CommandTarget {
    pub const fn window_id(self) -> Option<WindowId> {
        match self {
            Self::Widget { window_id, .. }
            | Self::FocusedWidget(window_id)
            | Self::Window(window_id) => Some(window_id),
            Self::Application => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandDelivery {
    Directed,
    Broadcast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ErasedCommandKey {
    pub(crate) name: &'static str,
    pub(crate) payload_type: TypeId,
    pub(crate) payload_type_name: &'static str,
}

#[derive(Clone)]
pub(crate) struct QueuedCommand {
    pub(crate) sequence: u64,
    pub(crate) key: ErasedCommandKey,
    pub(crate) target: CommandTarget,
    pub(crate) delivery: CommandDelivery,
    pub(crate) payload: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for QueuedCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueuedCommand")
            .field("sequence", &self.sequence)
            .field("name", &self.key.name)
            .field("payload_type", &self.key.payload_type_name)
            .field("target", &self.target)
            .field("delivery", &self.delivery)
            .finish_non_exhaustive()
    }
}

/// Borrowed view of a command while it is delivered on the UI thread.
pub struct Command<'a> {
    inner: &'a QueuedCommand,
}

impl<'a> Command<'a> {
    pub(crate) const fn new(inner: &'a QueuedCommand) -> Self {
        Self { inner }
    }

    pub const fn sequence(&self) -> u64 {
        self.inner.sequence
    }

    pub const fn name(&self) -> &'static str {
        self.inner.key.name
    }

    pub const fn payload_type_name(&self) -> &'static str {
        self.inner.key.payload_type_name
    }

    pub const fn target(&self) -> CommandTarget {
        self.inner.target
    }

    pub const fn delivery(&self) -> CommandDelivery {
        self.inner.delivery
    }

    pub fn is<T: 'static>(&self, key: CommandKey<T>) -> bool {
        self.inner.key == key.erased()
    }

    pub fn get<T: Send + Sync + 'static>(&self, key: CommandKey<T>) -> Option<&T> {
        self.is(key)
            .then(|| self.inner.payload.downcast_ref::<T>())
            .flatten()
    }
}

impl fmt::Debug for Command<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(formatter)
    }
}

type ExternalWaker = dyn Fn() + Send + Sync + 'static;

struct CommandHub {
    state: Mutex<CommandHubState>,
    wake_pending: AtomicBool,
    waker: RwLock<Option<Arc<ExternalWaker>>>,
}

#[derive(Default)]
struct CommandHubState {
    pending: VecDeque<QueuedCommand>,
    manual_wake_pending: bool,
}

impl CommandHub {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(CommandHubState::default()),
            wake_pending: AtomicBool::new(false),
            waker: RwLock::new(None),
        })
    }

    fn set_waker(&self, waker: Option<Arc<ExternalWaker>>) {
        let wake = waker.clone();
        *self
            .waker
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = waker;
        // A producer may enqueue before a platform installs its callback. In
        // that case `wake_pending` is already true, so invoke the newly
        // installed callback directly instead of waiting for another command.
        if self.has_pending()
            && let Some(wake) = wake
        {
            wake();
        }
    }

    fn enqueue(&self, command: QueuedCommand) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending
            .push_back(command);
        self.request_wake();
    }

    fn wake(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .manual_wake_pending = true;
        self.request_wake();
    }

    fn request_wake(&self) {
        if self.wake_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(waker) = self
            .waker
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            waker();
        }
    }

    fn drain(&self) -> (bool, Vec<QueuedCommand>) {
        let (manual_wake, commands) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let commands = state.pending.drain(..).collect();
            let manual_wake = std::mem::take(&mut state.manual_wake_pending);
            // Clear while holding the same lock producers use. A producer that
            // arrives after this point observes false and emits a fresh wake.
            self.wake_pending.store(false, Ordering::Release);
            (manual_wake, commands)
        };
        (manual_wake, commands)
    }

    fn has_pending(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.manual_wake_pending || !state.pending.is_empty()
    }
}

/// Cloneable, thread-safe producer for application commands.
///
/// Enqueueing a command wakes the platform loop. [`wake`](Self::wake) only
/// schedules the runtime; it never synthesizes a widget event.
#[derive(Clone)]
pub struct CommandSender {
    hub: Arc<CommandHub>,
}

impl CommandSender {
    pub(crate) fn new() -> Self {
        Self {
            hub: CommandHub::new(),
        }
    }

    pub fn wake(&self) {
        self.hub.wake();
    }

    pub fn send<T>(&self, target: CommandTarget, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.enqueue(target, CommandDelivery::Directed, key, payload);
    }

    pub fn broadcast<T>(&self, target: CommandTarget, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.enqueue(target, CommandDelivery::Broadcast, key, payload);
    }

    pub fn send_widget<T>(
        &self,
        window_id: WindowId,
        widget_id: WidgetId,
        key: CommandKey<T>,
        payload: T,
    ) where
        T: Send + Sync + 'static,
    {
        self.send(
            CommandTarget::Widget {
                window_id,
                widget_id,
            },
            key,
            payload,
        );
    }

    pub fn send_focused<T>(&self, window_id: WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.send(CommandTarget::FocusedWidget(window_id), key, payload);
    }

    pub fn send_window<T>(&self, window_id: WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.send(CommandTarget::Window(window_id), key, payload);
    }

    pub fn send_application<T>(&self, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.send(CommandTarget::Application, key, payload);
    }

    pub fn broadcast_window<T>(&self, window_id: WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.broadcast(CommandTarget::Window(window_id), key, payload);
    }

    pub fn broadcast_application<T>(&self, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.broadcast(CommandTarget::Application, key, payload);
    }

    fn enqueue<T>(
        &self,
        target: CommandTarget,
        delivery: CommandDelivery,
        key: CommandKey<T>,
        payload: T,
    ) where
        T: Send + Sync + 'static,
    {
        self.hub
            .enqueue(queued_command(target, delivery, key, payload));
    }

    pub(crate) fn set_waker(&self, waker: Option<Arc<ExternalWaker>>) {
        self.hub.set_waker(waker);
    }

    pub(crate) fn drain(&self) -> (bool, Vec<QueuedCommand>) {
        self.hub.drain()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.hub.has_pending()
    }
}

impl fmt::Debug for CommandSender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandSender")
            .field("has_pending", &self.has_pending())
            .finish_non_exhaustive()
    }
}

pub(crate) fn queued_command<T>(
    target: CommandTarget,
    delivery: CommandDelivery,
    key: CommandKey<T>,
    payload: T,
) -> QueuedCommand
where
    T: Send + Sync + 'static,
{
    QueuedCommand {
        sequence: NEXT_COMMAND_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        key: key.erased(),
        target,
        delivery,
        payload: Arc::new(payload),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CommandInvalidation {
    pub(crate) request: InvalidationRequest,
    pub(crate) reason: Option<String>,
}

/// Context supplied to window and application command controllers.
pub struct CommandCtx {
    sender: CommandSender,
    window_id: Option<WindowId>,
    handled: bool,
    invalidations: Vec<CommandInvalidation>,
    animation_targets: Vec<(WindowId, WidgetId)>,
}

impl CommandCtx {
    pub(crate) fn new(sender: CommandSender, window_id: Option<WindowId>) -> Self {
        Self {
            sender,
            window_id,
            handled: false,
            invalidations: Vec::new(),
            animation_targets: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    pub const fn is_handled(&self) -> bool {
        self.handled
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    pub fn sender(&self) -> &CommandSender {
        &self.sender
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(CommandInvalidation {
            request,
            reason: None,
        });
    }

    pub fn request_with_reason(&mut self, request: InvalidationRequest, reason: impl Into<String>) {
        self.invalidations.push(CommandInvalidation {
            request,
            reason: Some(reason.into()),
        });
    }

    pub fn request_window(&mut self, kind: InvalidationKind) {
        let window_id = self
            .window_id
            .expect("window invalidation requires a window-scoped command controller");
        self.request(InvalidationRequest::new(
            InvalidationTarget::Window(window_id),
            kind,
        ));
    }

    pub fn request_window_with_reason(
        &mut self,
        kind: InvalidationKind,
        reason: impl Into<String>,
    ) {
        let window_id = self
            .window_id
            .expect("window invalidation requires a window-scoped command controller");
        self.request_with_reason(
            InvalidationRequest::new(InvalidationTarget::Window(window_id), kind),
            reason,
        );
    }

    pub fn request_measure(&mut self) {
        self.request_window(InvalidationKind::Measure);
    }

    pub fn request_arrange(&mut self) {
        self.request_window(InvalidationKind::Arrange);
    }

    pub fn request_paint(&mut self) {
        self.request_window(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_window(InvalidationKind::Semantics);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        let window_id = self
            .window_id
            .expect("window invalidation requires a window-scoped command controller");
        self.request(
            InvalidationRequest::new(
                InvalidationTarget::Window(window_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn request_animation_frame(&mut self, widget_id: WidgetId) {
        let window_id = self
            .window_id
            .expect("animation requests require a window-scoped command controller");
        self.animation_targets.push((window_id, widget_id));
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<CommandInvalidation> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn take_animation_targets(&mut self) -> Vec<(WindowId, WidgetId)> {
        std::mem::take(&mut self.animation_targets)
    }
}

/// Non-widget application service that receives commands on the UI thread.
/// Controllers are owned by their application/window builder and are dropped
/// automatically with that owner.
pub trait CommandController {
    fn command(&mut self, _ctx: &mut CommandCtx, _command: &Command<'_>) {}

    /// Called for an explicit scheduler-only [`CommandSender::wake`].
    fn wake(&mut self, _ctx: &mut CommandCtx) {}

    fn debug_name(&self) -> &'static str {
        type_name::<Self>()
    }
}

type ErasedCommandCallback = dyn FnMut(&mut CommandCtx, &dyn Any);

pub(crate) struct CommandSubscription {
    key: ErasedCommandKey,
    handler_name: &'static str,
    callback: Box<ErasedCommandCallback>,
}

impl CommandSubscription {
    pub(crate) fn new<T, F>(key: CommandKey<T>, mut callback: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        Self {
            key: key.erased(),
            handler_name: type_name::<F>(),
            callback: Box::new(move |ctx, payload| {
                if let Some(payload) = payload.downcast_ref::<T>() {
                    callback(ctx, payload);
                }
            }),
        }
    }
}

pub(crate) enum CommandListener {
    Controller(Box<dyn CommandController>),
    Subscription(CommandSubscription),
}

#[derive(Default)]
pub(crate) struct CommandListeners {
    listeners: Vec<CommandListener>,
}

impl CommandListeners {
    pub(crate) fn push_controller(&mut self, controller: impl CommandController + 'static) {
        self.listeners
            .push(CommandListener::Controller(Box::new(controller)));
    }

    pub(crate) fn push_subscription<T, F>(&mut self, key: CommandKey<T>, callback: F)
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.listeners
            .push(CommandListener::Subscription(CommandSubscription::new(
                key, callback,
            )));
    }

    pub(crate) fn dispatch(
        &mut self,
        ctx: &mut CommandCtx,
        command: &Command<'_>,
        broadcast: bool,
    ) -> Vec<String> {
        let mut invoked = Vec::new();
        for listener in &mut self.listeners {
            match listener {
                CommandListener::Controller(controller) => {
                    invoked.push(controller.debug_name().to_string());
                    controller.command(ctx, command);
                }
                CommandListener::Subscription(subscription)
                    if subscription.key == command.inner.key =>
                {
                    invoked.push(subscription.handler_name.to_string());
                    (subscription.callback)(ctx, command.inner.payload.as_ref());
                }
                CommandListener::Subscription(_) => continue,
            }
            if ctx.is_handled() && !broadcast {
                break;
            }
        }
        invoked
    }

    pub(crate) fn wake(&mut self, ctx: &mut CommandCtx) -> Vec<String> {
        let mut invoked = Vec::new();
        for listener in &mut self.listeners {
            if let CommandListener::Controller(controller) = listener {
                invoked.push(controller.debug_name().to_string());
                controller.wake(ctx);
            }
        }
        invoked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNT: CommandKey<u32> = CommandKey::new("test.count");

    #[test]
    fn typed_keys_reject_a_same_name_different_payload() {
        static STRING_COUNT: CommandKey<String> = CommandKey::new("test.count");
        let queued = queued_command(
            CommandTarget::Application,
            CommandDelivery::Directed,
            COUNT,
            7,
        );
        let command = Command::new(&queued);
        assert_eq!(command.get(COUNT), Some(&7));
        assert!(command.get(STRING_COUNT).is_none());
    }

    #[test]
    fn command_sender_coalesces_scheduler_wakes_but_keeps_payloads() {
        let sender = CommandSender::new();
        let wake_count = Arc::new(AtomicU64::new(0));
        let wake_count_for_callback = Arc::clone(&wake_count);
        sender.set_waker(Some(Arc::new(move || {
            wake_count_for_callback.fetch_add(1, Ordering::Relaxed);
        })));

        sender.send_application(COUNT, 1);
        sender.send_application(COUNT, 2);
        assert_eq!(wake_count.load(Ordering::Relaxed), 1);
        let (_, commands) = sender.drain();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn installing_a_waker_notifies_previously_queued_work() {
        let sender = CommandSender::new();
        sender.send_application(COUNT, 1);
        let wake_count = Arc::new(AtomicU64::new(0));
        let wake_count_for_callback = Arc::clone(&wake_count);

        sender.set_waker(Some(Arc::new(move || {
            wake_count_for_callback.fetch_add(1, Ordering::Relaxed);
        })));

        assert_eq!(wake_count.load(Ordering::Relaxed), 1);
    }
}
