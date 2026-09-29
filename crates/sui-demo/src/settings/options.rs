//! The render options Settings and the HDR validation page edit.

use std::{cell::Cell, rc::Rc};

use sui::prelude::*;
use sui::{
    TextCoveragePolicy, TextHinting, WgpuRenderer, WidgetPodMutVisitor, WidgetPodVisitor, WindowId,
    WindowRenderOptions, WindowTextCoveragePolicy, WindowTextHinting, set_window_render_options,
    window_render_options,
};

/// The options a window renders with when none are set: the renderer's own
/// defaults.
pub(crate) fn default_render_options() -> WindowRenderOptions {
    let renderer = WgpuRenderer::new();
    WindowRenderOptions::new(renderer.feathering_enabled(), renderer.feather_width())
        .with_text_hinting(match renderer.text_hinting().normalized() {
            TextHinting::None => WindowTextHinting::None,
            TextHinting::Slight { max_ppem } => WindowTextHinting::Slight { max_ppem },
        })
        .with_text_coverage_policy(match renderer.text_coverage_policy().normalized() {
            TextCoveragePolicy::Perceptual | TextCoveragePolicy::PerceptualLuminance { .. } => {
                WindowTextCoveragePolicy::Perceptual
            }
            TextCoveragePolicy::Linear => WindowTextCoveragePolicy::Linear,
            TextCoveragePolicy::Gamma(gamma) => WindowTextCoveragePolicy::Gamma(gamma),
            TextCoveragePolicy::CoverageBoost(amount) => {
                WindowTextCoveragePolicy::CoverageBoost(amount)
            }
            TextCoveragePolicy::TwoCoverageMinusCoverageSq => {
                WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq
            }
        })
}

/// A source that follows one yes-or-no fact about the options.
pub(crate) type OptionFlag = Selector<Signal<WindowRenderOptions>, WindowRenderOptions, bool>;

/// One window's render options, as Settings and the HDR validation page edit
/// them. Clones share the options, so each editor shows what the others
/// changed.
#[derive(Clone)]
pub(crate) struct RenderOptions {
    options: Signal<WindowRenderOptions>,
    window: Rc<Cell<Option<WindowId>>>,
    /// Whether to take the window's options when first bound to it, for an
    /// editor that is not sharing them with Settings.
    adopt_window: Rc<Cell<bool>>,
}

impl RenderOptions {
    /// Options the window already renders with.
    pub(crate) fn shared(options: WindowRenderOptions) -> Self {
        Self {
            options: Signal::named("Render options", options),
            window: Rc::new(Cell::new(None)),
            adopt_window: Rc::new(Cell::new(false)),
        }
    }

    /// Options that start as whatever the window renders with.
    pub(crate) fn from_window() -> Self {
        let options = Self::shared(default_render_options());
        options.adopt_window.set(true);
        options
    }

    pub(crate) fn get(&self) -> WindowRenderOptions {
        self.options.get()
    }

    /// Change the options and apply them to the window, which repaints with
    /// them.
    pub(crate) fn update(&self, update: impl FnOnce(&mut WindowRenderOptions)) {
        let mut options = self.get();
        update(&mut options);
        self.options.set(options);
        if let Some(window_id) = self.window.get() {
            set_window_render_options(window_id, options.clamped());
        }
    }

    /// Whether `test` holds for the options, as a source to follow.
    pub(crate) fn flag(
        &self,
        name: &'static str,
        test: fn(&WindowRenderOptions) -> bool,
    ) -> OptionFlag {
        Selector::new(name, self.options.clone(), test)
    }

    /// Apply changes to `window_id` from now on. Options made with
    /// [`from_window`](Self::from_window) take the window's options first.
    fn bind_window(&self, window_id: WindowId) {
        if self.window.replace(Some(window_id)) == Some(window_id) {
            return;
        }
        if self.adopt_window.replace(false)
            && let Some(options) = window_render_options(window_id)
        {
            self.options.set(options);
        }
    }
}

/// Hosts an editor of `options`, applying them to the window it is in.
pub(crate) struct RenderOptionsScope {
    options: RenderOptions,
    content: SingleChild,
}

impl RenderOptionsScope {
    pub(crate) fn new<W>(options: RenderOptions, content: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            options,
            content: SingleChild::new(content),
        }
    }
}

impl Widget for RenderOptionsScope {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.options.bind_window(ctx.window_id());
        self.content.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}
