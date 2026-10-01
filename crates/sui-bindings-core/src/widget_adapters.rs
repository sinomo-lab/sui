use crate::interop::ExternalTextureDescriptor;
use crate::tasks::UiTaskQueue;
use crate::theme::BindingTheme;
use crate::values::{BindingBool, BindingNumber, BindingText};
use std::sync::Arc;
use sui::ArrangeCtx;
use sui::Checkbox;
#[cfg(feature = "desktop")]
use sui::CommandKey;
use sui::CommandPalette;
use sui::Constraints;
use sui::DateTimeInput;
use sui::Event;
use sui::EventCtx;
use sui::MeasureCtx;
use sui::PaintCtx;
use sui::PasswordInput;
use sui::ProgressBar;
use sui::RadioButton;
use sui::Rect;
use sui::RegisteredImage;
use sui::SemanticsCtx;
use sui::SemanticsNode;
use sui::SemanticsRole;
use sui::SideSheet;
use sui::Size;
use sui::Spinner;
use sui::SplitView;
use sui::Switch;
use sui::TextArea;
use sui::TextInput;
use sui::Widget;
use sui::WidgetPodMutVisitor;
use sui::WidgetPodVisitor;

pub(crate) struct BindingSideSheetWidget {
    pub(crate) inner: SideSheet,
    pub(crate) shown: BindingBool,
    pub(crate) last_shown: bool,
}

impl BindingSideSheetWidget {
    pub(crate) fn new(
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

    pub(crate) fn sync_state(&mut self) {
        let shown = self.shown.resolve();
        if shown != self.last_shown {
            self.inner.set_open(shown);
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

pub(crate) struct BindingCommandPaletteWidget {
    pub(crate) inner: CommandPalette,
    pub(crate) shown: BindingBool,
    pub(crate) last_shown: bool,
}

impl BindingCommandPaletteWidget {
    pub(crate) fn new(
        shown: BindingBool,
        build: impl Fn(bool) -> CommandPalette + 'static,
    ) -> Self {
        let last_shown = shown.resolve();
        Self {
            inner: build(last_shown),
            shown,
            last_shown,
        }
    }

    pub(crate) fn sync_state(&mut self) {
        let shown = self.shown.resolve();
        if shown != self.last_shown {
            self.inner.set_open(shown);
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

pub(crate) struct BindingSplitViewWidget {
    pub(crate) inner: SplitView,
    pub(crate) ratio: BindingNumber,
}

impl BindingSplitViewWidget {
    pub(crate) fn new(
        ratio: BindingNumber,
        build: impl Fn(f32) -> SplitView + 'static,
    ) -> BindingSplitViewWidget {
        let inner = build(ratio.resolve() as f32);
        Self { inner, ratio }
    }

    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingBusyIndicatorWidget {
    pub(crate) name: BindingText,
    pub(crate) label: Option<BindingText>,
    pub(crate) size: f32,
    pub(crate) theme: Option<BindingTheme>,
}

impl BindingBusyIndicatorWidget {
    pub(crate) fn inner(&self) -> Spinner {
        let mut indicator = Spinner::new(self.name.resolve()).size(self.size);
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

pub(crate) struct BindingProgressBarWidget {
    pub(crate) name: BindingText,
    pub(crate) value: BindingNumber,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) show_value: bool,
    pub(crate) theme: Option<BindingTheme>,
}

impl BindingProgressBarWidget {
    pub(crate) fn inner(&self) -> ProgressBar {
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

pub(crate) struct BindingCheckboxWidget {
    pub(crate) inner: Checkbox,
    pub(crate) checked: BindingBool,
}

impl BindingCheckboxWidget {
    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingSwitchWidget {
    pub(crate) inner: Switch,
    pub(crate) on: BindingBool,
}

impl BindingSwitchWidget {
    pub(crate) fn sync_state(&mut self) {
        if matches!(self.on, BindingBool::State(_)) {
            self.inner.set_checked(self.on.resolve());
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

pub(crate) struct BindingRadioButtonWidget {
    pub(crate) inner: RadioButton,
    pub(crate) selected: BindingBool,
}

impl BindingRadioButtonWidget {
    pub(crate) fn sync_state(&mut self) {
        if matches!(self.selected, BindingBool::State(_)) {
            self.inner.set_checked(self.selected.resolve());
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

pub(crate) struct BindingTextInputWidget {
    pub(crate) inner: TextInput,
    pub(crate) value: BindingText,
}

impl BindingTextInputWidget {
    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingPasswordInputWidget {
    pub(crate) inner: PasswordInput,
    pub(crate) value: BindingText,
}

impl BindingPasswordInputWidget {
    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingDateTimeInputWidget {
    pub(crate) inner: DateTimeInput,
    pub(crate) value: BindingText,
}

impl BindingDateTimeInputWidget {
    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingTextAreaWidget {
    pub(crate) inner: TextArea,
    pub(crate) value: BindingText,
}

impl BindingTextAreaWidget {
    pub(crate) fn sync_state(&mut self) {
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

pub(crate) struct BindingExternalSurfaceWidget {
    pub(crate) descriptor: ExternalTextureDescriptor,
    pub(crate) desired_size: Size,
    pub(crate) name: Option<String>,
}

impl BindingExternalSurfaceWidget {
    pub(crate) fn draw_cpu_fallback(&self, ctx: &mut PaintCtx, size: Size, pixels: &Arc<[u8]>) {
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

pub(crate) struct BindingRuntimeWidget {
    pub(crate) inner: Box<dyn Widget>,
}

impl BindingRuntimeWidget {
    pub(crate) fn new(widget: impl Widget + 'static) -> Self {
        Self {
            inner: Box::new(widget),
        }
    }
}

/// Transparent: the built widget's layout, layers, overlay behavior, and
/// commands all pass through, as if it were in the tree directly. Output
/// reuse stays off, so a binding subtree repaints and re-reads its state
/// whenever it paints.
impl Widget for BindingRuntimeWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &sui::Command<'_>) {
        self.inner.command(ctx, command);
    }

    fn diagnostics(&self, ctx: &mut sui::WidgetDiagnosticsCtx) {
        self.inner.diagnostics(ctx);
    }

    fn measure_size(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure_size(ctx, constraints)
    }

    fn measure_axis(
        &mut self,
        ctx: &mut MeasureCtx,
        constraints: Constraints,
        axis: sui::Axis,
    ) -> f32 {
        self.inner.measure_axis(ctx, constraints, axis)
    }

    fn intrinsic_size(
        &mut self,
        ctx: &mut MeasureCtx,
        axis: sui::Axis,
        available_cross: f32,
    ) -> sui::IntrinsicSize {
        self.inner.intrinsic_size(ctx, axis, available_cross)
    }

    fn layer_options(&self) -> sui::LayerOptions {
        self.inner.layer_options()
    }

    fn layer_properties(&self) -> sui::LayerProperties {
        self.inner.layer_properties()
    }

    fn layer_properties_at(&self, frame_time: f64) -> sui::LayerProperties {
        self.inner.layer_properties_at(frame_time)
    }

    fn stack_host_options(&self) -> Option<sui::StackHostOptions> {
        self.inner.stack_host_options()
    }

    fn stack_surface_options(&self) -> Option<sui::StackSurfaceOptions> {
        self.inner.stack_surface_options()
    }

    fn overlay_options(&self) -> Option<sui::OverlayOptions> {
        self.inner.overlay_options()
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

pub(crate) struct BindingUiTaskRootWidget {
    pub(crate) inner: BindingRuntimeWidget,
    pub(crate) ui_tasks: UiTaskQueue,
}

impl BindingUiTaskRootWidget {
    pub(crate) fn new(inner: BindingRuntimeWidget, ui_tasks: UiTaskQueue) -> Self {
        Self { inner, ui_tasks }
    }

    pub(crate) fn drain_ui_tasks(&self, ctx: &mut EventCtx) -> usize {
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

    fn command(&mut self, ctx: &mut EventCtx, command: &sui::Command<'_>) {
        self.inner.command(ctx, command);
    }

    fn diagnostics(&self, ctx: &mut sui::WidgetDiagnosticsCtx) {
        self.inner.diagnostics(ctx);
    }

    fn measure_size(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure_size(ctx, constraints)
    }

    fn measure_axis(
        &mut self,
        ctx: &mut MeasureCtx,
        constraints: Constraints,
        axis: sui::Axis,
    ) -> f32 {
        self.inner.measure_axis(ctx, constraints, axis)
    }

    fn intrinsic_size(
        &mut self,
        ctx: &mut MeasureCtx,
        axis: sui::Axis,
        available_cross: f32,
    ) -> sui::IntrinsicSize {
        self.inner.intrinsic_size(ctx, axis, available_cross)
    }

    fn layer_options(&self) -> sui::LayerOptions {
        self.inner.layer_options()
    }

    fn layer_properties(&self) -> sui::LayerProperties {
        self.inner.layer_properties()
    }

    fn layer_properties_at(&self, frame_time: f64) -> sui::LayerProperties {
        self.inner.layer_properties_at(frame_time)
    }

    fn stack_host_options(&self) -> Option<sui::StackHostOptions> {
        self.inner.stack_host_options()
    }

    fn stack_surface_options(&self) -> Option<sui::StackSurfaceOptions> {
        self.inner.stack_surface_options()
    }

    fn overlay_options(&self) -> Option<sui::OverlayOptions> {
        self.inner.overlay_options()
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
pub(crate) const BINDING_UI_TASKS_READY: CommandKey<()> =
    CommandKey::new("sui.bindings.ui-tasks-ready");
