#![forbid(unsafe_code)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    sync::Arc,
};

pub mod app;
pub mod composites;
pub mod containers;
pub mod controls;

#[cfg(any(feature = "desktop", feature = "web"))]
pub use app::UiHandle;
pub use app::{App, ResourceRegistry, Window};

pub use composites::{
    BusyIndicator, CommandGroup, ContextMenu, Dialog, DockPanel, FieldGroup, FormRow, FormSection,
    Menu, MenuItem, Modal, PanelSection, Popover, PresetStrip, ProgressBar, PropertyRow,
    PropertyRowLayout, Spinner, StatusBar, StatusBarHost, StatusBarSegment, Surface, SurfaceBorder,
    SurfaceElevation, SurfaceRole, TabBar, Tabs, ToolPalette, ToolPaletteItem, Toolbar, Tooltip,
    TooltipPlacement,
};
pub use containers::{
    Align, Background, Overflow, ScrollAxes, ScrollBar, ScrollState, ScrollView, SizedBox, Stack,
    SwitchView, VirtualScrollView,
};
pub use controls::{
    BUILTIN_ICON_GLYPHS, Button, Checkbox, ComboBox, Divider, Icon, IconButton, IconGlyph, Label,
    MultilineTextInput, NumberInput, RadioButton, RadioGroup, Select, Separator, Slider, SpinBox,
    Switch, TextArea, TextInput, draw_glyph, register_builtin_icon_resources,
};
pub use sui_core::{
    AsyncWakeToken, Color, ColorSpace, CustomEvent, DirtyRegion, DpiInfo, Error, Event, FontHandle,
    ImageHandle, ImeEvent, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    KeyboardEvent, Modifiers, Path, PathBuilder, PathElement, Point, PointerButton, PointerButtons,
    PointerEvent, PointerEventKind, PointerKind, Rect, Result, ScrollDelta, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, Size, SurfaceId, TimerToken,
    ToggleState, Transform, Vector, WakeEvent, WidgetId, WindowEvent, WindowId,
};
pub use sui_layout::Padding as Insets;
pub use sui_layout::{Alignment, Axis, Constraints, LayoutContext, Padding};
#[cfg(any(feature = "desktop", feature = "web"))]
pub use sui_platform::{
    AccessibilitySnapshot, DesktopAutomationAction, DesktopAutomationConfig, DesktopPlatform,
    HeadlessPlatform, PlatformWindow, Waker, WindowOutputDiagnostics, window_output_diagnostics,
};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{
    RendererCapabilities, RendererInterop, StemDarkening, TextCoveragePolicy, TextHinting,
    WgpuRenderer,
};
pub use sui_runtime::{
    Application as RuntimeApplication, ArrangeCtx, CacheMetrics, CacheMetricsDelta,
    DEFAULT_SUI_LOGO_SVG, EXTERNAL_WAKE_KIND, EmbeddedSvgImageResource, EventCtx, EventPhase,
    FocusState, FramePhase, FramePhaseSample, FrameSchedule, MeasureCtx, PaintCtx,
    PresentationLatencyDiagnostics, RenderDiagnostics, RenderOutput, RendererSubmissionDiagnostics,
    RetainedPacketRebuildDiagnostics, Runtime, SceneStatistics, SceneStatisticsDetailMode,
    SemanticsCtx, SingleChild, StackHostOptions, StackOrderPolicy, StackSurfaceOptions,
    TextCacheDeltaDiagnostics, TextCacheDiagnostics, Widget, WidgetChildren,
    WidgetGeometrySnapshot, WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder, WindowColorManagementMode,
    WindowDynamicRangeMode, WindowIcon, WindowOutputColorPrimaries, WindowPerformanceSnapshot,
    WindowPerformanceSummary, WindowRenderOptions, WindowStemDarkening, WindowTextHinting,
    WindowToneMappingMode, clear_window_render_options, default_sui_logo_image,
    set_window_render_options, set_window_scene_statistics_detail_mode,
    window_performance_snapshot, window_performance_summary, window_render_options,
    window_scene_statistics_detail_mode,
};
pub use sui_scene::{
    Border, Brush, GradientStop, ImageRegistry, ImageSource, RegisteredImage,
    RegisteredImageFormat, Scene, SceneCommand, SceneFrame, ShadowParams, StrokeStyle,
    WidgetShader,
};
pub use sui_text::{
    FontFeature, FontFeatures, FontRegistry, FontStretch, FontStyle, FontWeight,
    PersistentTextLayout, RegisteredFont, ResolvedTextFace, ShapedGlyph, ShapedText,
    ShapedTextWindow, TextAffinity, TextAlign, TextCluster, TextCursor, TextDirection,
    TextDocument, TextFlowDirection, TextGlyphInstance, TextLayout, TextLayoutHandle, TextLayoutId,
    TextLayoutMetadata, TextLayoutRegistry, TextLayoutRequest, TextLayoutRun, TextLayoutVersion,
    TextLayoutView, TextLine, TextLineWindow, TextMeasurement, TextParagraph, TextParagraphLayout,
    TextParagraphStyle, TextRun, TextRunView, TextSelection, TextSelectionGeometry, TextSpan,
    TextSpanId, TextStyle, TextWrap, TextWritingMode,
};
pub use sui_widgets::animation::{
    ANIMATION_DOCUMENT_VERSION, AnimatedValue, AnimationBinding, AnimationBindingInvalidation,
    AnimationDocument, AnimationDocumentFormatError, AnimationEditorCommand, AnimationEditorState,
    AnimationPlayer, AnimationProperty, AnimationPropertyPath, AnimationSelection,
    AnimationTargetId, AnimationTick, AnimationValue, AnimationValueKind, Blink, Clip,
    CompiledClip, CompiledTimeline, CompiledTrack, Easing, Interpolate, Keyframe,
    KeyframeSelection, LoopMode, PlaybackState, Pulse, SampleBatch, SampleBuffer,
    SampledAnimationValue, SharedCompiledTimeline, SpringF32, Timeline, TimelineBindingSink,
    TimelinePlayer, TimelineSnap, TimelineTick, Track, Transition,
    invalidation_for_animation_property,
};
pub use sui_widgets::{
    ActionCard, Breadcrumb, BreadcrumbItem, BrushPreview, BrushPreviewShape, BrushPreviewSpec,
    Canvas, CanvasRuler, CanvasRulerAxis, CanvasShape, CanvasStroke, CanvasViewport, ColorPalette,
    ColorPaletteSwatch, ColorPicker, ColorSwatch, ControlMetrics, ControlPalette,
    ControlTypography, DataGrid, DefaultTheme, EffectToken, FloatingStack, FloatingViewConfig,
    FloatingViewSnapshot, FloatingWorkspace, FloatingWorkspaceState, HdrColorRoles,
    HdrEffectTokens, HdrLuminanceTokens, HdrMaterialTokens, HdrPolicyTokens, HdrThemeMode,
    HdrThemeTokens, Image, ImageFit, LayerList, LayerListItem, ListItem, ListView, MaterialToken,
    PathBar, PixelCanvas, PixelCanvasBlendMode, PixelCanvasBrushShape, PixelCanvasExportSnapshot,
    PixelCanvasState, PixelCanvasTool, ResizablePane, ResolvedEffectStyle, ResolvedHdrStyle,
    ResolvedMaterialStyle, SemanticColorToken, SplitView, SurfacePalette, Table, TableColumn,
    TableColumnAlignment, TableRow, TextSurface, TextSurfaceOverlayKind, TextSurfaceStyleOverlay,
    TextSurfaceStyleSpan, ThemeAspectRatios, ThemeBlurScale, ThemeBreakpoints, ThemeColorScheme,
    ThemeColors, ThemeContainers, ThemeFontFamilies, ThemeFontStack, ThemeFontWeights,
    ThemeLeading, ThemeMotion, ThemePerspective, ThemeRadii, ThemeShadow, ThemeShadowLayer,
    ThemeShadows, ThemeTextScale, ThemeTextToken, ThemeTracking, TreeItem, TreeView,
    WidgetColorRole, WidgetEffectRole, WidgetLuminanceRole, WidgetMaterialRole, paint_theme_shadow,
    resolve_effect_role, resolve_luminance_role, resolve_material_role, resolve_semantic_color,
    resolve_widget_hdr_style,
};

pub trait ThemeExtension: Any + Send + Sync {}

impl<T> ThemeExtension for T where T: Any + Send + Sync {}

#[derive(Clone, Default)]
pub struct ThemeExtensions {
    values: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ThemeExtensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|previous| Arc::downcast::<T>(previous).ok())
    }

    pub fn get<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn get_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| Arc::clone(value).downcast::<T>().ok())
    }

    pub fn contains<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.values.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|value| Arc::downcast::<T>(value).ok())
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

impl fmt::Debug for ThemeExtensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThemeExtensions")
            .field("len", &self.values.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub default_widgets: DefaultTheme,
    pub extensions: ThemeExtensions,
}

impl Default for Theme {
    fn default() -> Self {
        let default_widgets = DefaultTheme::default();
        Self {
            background: default_widgets.palette.surface,
            foreground: default_widgets.palette.text,
            default_widgets,
            extensions: ThemeExtensions::default(),
        }
    }
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_widgets(mut self, theme: DefaultTheme) -> Self {
        self.default_widgets = theme;
        self
    }

    pub fn with_extension<T>(mut self, value: T) -> Self
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value);
        self
    }

    pub fn insert_extension<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value)
    }

    pub fn extension<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.extensions.get::<T>()
    }

    pub fn extension_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.get_arc::<T>()
    }

    pub fn has_extension<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.extensions.contains::<T>()
    }

    pub fn remove_extension<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.remove::<T>()
    }
}

pub struct Application {
    inner: RuntimeApplication,
    #[cfg(feature = "wgpu")]
    feathering_enabled: bool,
    #[cfg(feature = "wgpu")]
    feather_width: f32,
    initial_window_render_options: Option<WindowRenderOptions>,
}

impl Default for Application {
    fn default() -> Self {
        let mut inner = RuntimeApplication::default();
        sui_widgets::register_builtin_icon_resources(&mut inner)
            .expect("built-in Lucide icon resources should be valid");
        Self {
            inner,
            #[cfg(feature = "wgpu")]
            feathering_enabled: WgpuRenderer::new().feathering_enabled(),
            #[cfg(feature = "wgpu")]
            feather_width: WgpuRenderer::new().feather_width(),
            initial_window_render_options: None,
        }
    }
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    #[cfg(feature = "wgpu")]
    pub fn with_feathering_enabled(mut self, enabled: bool) -> Self {
        self.feathering_enabled = enabled;
        self
    }

    #[cfg(feature = "wgpu")]
    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.feather_width = feather_width.max(0.0);
        self
    }

    #[cfg(feature = "wgpu")]
    pub fn feathering_enabled(&self) -> bool {
        self.feathering_enabled
    }

    #[cfg(feature = "wgpu")]
    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    pub fn with_window_render_options(mut self, options: WindowRenderOptions) -> Self {
        self.initial_window_render_options = Some(options.clamped());
        self
    }

    pub fn initial_window_render_options(&self) -> Option<WindowRenderOptions> {
        self.initial_window_render_options
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        self.inner.register_font(handle, font)
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        self.inner.register_font_bytes(data)
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        self.inner.register_image(handle, image)
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        self.inner.register_rgba_image(width, height, data)
    }

    pub fn register_svg_image(&mut self, data: impl AsRef<[u8]>) -> Result<ImageHandle> {
        self.inner.register_svg_image(data)
    }

    pub fn register_svg_image_with_handle(
        &mut self,
        handle: ImageHandle,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner.register_svg_image_with_handle(handle, data)
    }

    pub fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<ImageHandle> {
        self.inner.register_svg_image_at_size(width, height, data)
    }

    pub fn register_svg_image_at_size_with_handle(
        &mut self,
        handle: ImageHandle,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner
            .register_svg_image_at_size_with_handle(handle, width, height, data)
    }

    pub fn register_embedded_svg_image(
        &mut self,
        resource: EmbeddedSvgImageResource,
    ) -> Result<()> {
        self.inner.register_embedded_svg_image(resource)
    }

    pub fn register_embedded_svg_images(
        &mut self,
        resources: impl IntoIterator<Item = EmbeddedSvgImageResource>,
    ) -> Result<()> {
        self.inner.register_embedded_svg_images(resources)
    }

    pub fn build(self) -> Result<Runtime> {
        self.inner.build()
    }

    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run(self) -> Result<()> {
        let feathering_enabled = self.feathering_enabled;
        let feather_width = self.feather_width;
        let initial_window_render_options = self.initial_window_render_options;
        let runtime = self.build()?;
        let platform = DesktopPlatform::new()
            .with_feathering_enabled(feathering_enabled)
            .with_feather_width(feather_width);
        if let Some(options) = initial_window_render_options {
            for window_id in runtime.window_ids() {
                set_window_render_options(window_id, options);
            }
        }
        let _ = platform.run(runtime)?;
        Ok(())
    }

    /// Like [`run`](Self::run) but invokes `on_ready` with a [`Waker`] once the event loop is
    /// created (before it starts running), so the caller can wake the UI from a background
    /// thread — e.g. run startup work off the UI thread and refresh the UI when it finishes.
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run_with(self, on_ready: impl FnOnce(Waker)) -> Result<()> {
        let feathering_enabled = self.feathering_enabled;
        let feather_width = self.feather_width;
        let initial_window_render_options = self.initial_window_render_options;
        let runtime = self.build()?;
        let platform = DesktopPlatform::new()
            .with_feathering_enabled(feathering_enabled)
            .with_feather_width(feather_width);
        if let Some(options) = initial_window_render_options {
            for window_id in runtime.window_ids() {
                set_window_render_options(window_id, options);
            }
        }
        let _ = platform.run_with(runtime, on_ready)?;
        Ok(())
    }

    #[cfg(not(any(feature = "desktop", feature = "web")))]
    pub fn run(self) -> Result<()> {
        let _ = self;
        Err(Error::new(
            "Application::run requires the `desktop` or `web` feature to provide a platform event loop",
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub foreground: Brush,
    pub padding: Padding,
}

impl Default for Style {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            foreground: Brush::Solid(theme.foreground),
            padding: Padding::all(0.0),
        }
    }
}

pub mod prelude {
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub use crate::UiHandle;

    pub use crate::{
        ActionCard, Align, Alignment, AnimatedValue, AnimationBinding, AnimationDocument,
        AnimationDocumentFormatError, AnimationEditorCommand, AnimationEditorState,
        AnimationPlayer, AnimationProperty, AnimationPropertyPath, AnimationSelection,
        AnimationTargetId, AnimationTick, AnimationValue, AnimationValueKind, App, Application,
        ArrangeCtx, AsyncWakeToken, Axis, Background, Blink, Breadcrumb, BreadcrumbItem, Brush,
        BrushPreview, BrushPreviewShape, BrushPreviewSpec, BusyIndicator, Button, Canvas,
        CanvasRuler, CanvasRulerAxis, CanvasShape, CanvasStroke, CanvasViewport, Checkbox, Clip,
        Color, ColorPalette, ColorPaletteSwatch, ColorPicker, ColorSwatch, ComboBox, CommandGroup,
        CompiledClip, CompiledTimeline, CompiledTrack, Constraints, ContextMenu, ControlMetrics,
        ControlPalette, ControlTypography, DataGrid, DefaultTheme, Dialog, Divider, DockPanel,
        Easing, Event, EventCtx, FieldGroup, FloatingViewConfig, FloatingViewSnapshot,
        FloatingWorkspace, FloatingWorkspaceState, FontFeature, FontFeatures, FontHandle,
        FontStretch, FontStyle, FontWeight, FormRow, FormSection, Icon, IconButton, IconGlyph,
        Image, ImageFit, ImageHandle, ImeEvent, Insets, Interpolate, KeyboardEvent, Keyframe,
        KeyframeSelection, Label, LayerList, LayerListItem, ListItem, ListView, LoopMode,
        MeasureCtx, Menu, MenuItem, Modal, MultilineTextInput, NumberInput, Overflow, PaintCtx,
        PanelSection, Path, PathBar, PathBuilder, PixelCanvas, PixelCanvasBlendMode,
        PixelCanvasBrushShape, PixelCanvasExportSnapshot, PixelCanvasState, PixelCanvasTool,
        PlaybackState, Point, PointerEvent, Popover, PresetStrip, ProgressBar, PropertyRow,
        PropertyRowLayout, Pulse, RadioButton, RadioGroup, Rect, RegisteredFont, RegisteredImage,
        ResizablePane, ResourceRegistry, Result, SampleBatch, SampleBuffer, SampledAnimationValue,
        ScrollAxes, ScrollBar, ScrollState, ScrollView, Select, SemanticsCtx, Separator,
        ShapedText, SharedCompiledTimeline, SingleChild, Size, SizedBox, Slider, SpinBox, Spinner,
        SplitView, SpringF32, Stack, StatusBar, StatusBarHost, StatusBarSegment, StrokeStyle,
        Style, Surface, SurfaceBorder, SurfaceElevation, SurfacePalette, SurfaceRole, Switch,
        SwitchView, TabBar, Table, TableColumn, TableColumnAlignment, TableRow, Tabs, TextArea,
        TextInput, TextLayout, TextMeasurement, TextStyle, Theme, ThemeAspectRatios,
        ThemeBlurScale, ThemeBreakpoints, ThemeColorScheme, ThemeColors, ThemeContainers,
        ThemeExtension, ThemeExtensions, ThemeFontFamilies, ThemeFontStack, ThemeFontWeights,
        ThemeLeading, ThemeMotion, ThemePerspective, ThemeRadii, ThemeShadow, ThemeShadowLayer,
        ThemeShadows, ThemeTextScale, ThemeTextToken, ThemeTracking, Timeline, TimelineBindingSink,
        TimelinePlayer, TimelineSnap, TimelineTick, TimerToken, ToolPalette, ToolPaletteItem,
        Toolbar, Tooltip, TooltipPlacement, Track, Transform, Transition, TreeItem, TreeView,
        VirtualScrollView, WakeEvent, Widget, WidgetChildren, WidgetPod, WidgetShader, Window,
        WindowBuilder, WindowRenderOptions, containers::Padding,
        invalidation_for_animation_property, register_builtin_icon_resources,
        set_window_render_options,
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{DefaultTheme, HdrThemeMode, HdrThemeTokens, Theme};
    #[cfg(feature = "wgpu")]
    use crate::{
        Application, WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
        WindowRenderOptions, WindowToneMappingMode,
    };

    #[derive(Debug, PartialEq)]
    struct CustomWidgetTheme {
        radius: f32,
        density: u8,
    }

    #[test]
    fn theme_stores_default_widget_theme_separately_from_extensions() {
        let mut defaults = DefaultTheme::default();
        defaults.metrics.min_height = 24.0;

        let theme = Theme::new()
            .with_default_widgets(defaults)
            .with_extension(CustomWidgetTheme {
                radius: 6.0,
                density: 2,
            });

        assert_eq!(theme.default_widgets.metrics.min_height, 24.0);
        assert!(theme.has_extension::<CustomWidgetTheme>());
        assert_eq!(
            theme.extension::<CustomWidgetTheme>(),
            Some(&CustomWidgetTheme {
                radius: 6.0,
                density: 2,
            })
        );
    }

    #[test]
    fn theme_extensions_round_trip_hdr_theme_tokens() {
        let mut defaults = DefaultTheme::dark();
        defaults.metrics.min_height = 30.0;
        defaults.hdr.mode = HdrThemeMode::WideGamutOnly;

        let hdr_tokens = HdrThemeTokens {
            mode: HdrThemeMode::FullHdr,
            ..HdrThemeTokens::default()
        };

        let theme = Theme::new()
            .with_default_widgets(defaults)
            .with_extension(hdr_tokens);

        assert_eq!(theme.default_widgets.metrics.min_height, 30.0);
        assert_eq!(theme.default_widgets.hdr.mode, HdrThemeMode::WideGamutOnly);
        assert!(theme.has_extension::<HdrThemeTokens>());
        assert_eq!(theme.extension::<HdrThemeTokens>(), Some(&hdr_tokens));
        assert_eq!(
            Arc::as_ref(
                &theme
                    .extension_arc::<HdrThemeTokens>()
                    .expect("hdr theme tokens extension present")
            ),
            &hdr_tokens
        );
    }

    #[test]
    fn theme_extensions_support_arc_access_and_removal() {
        let mut theme = Theme::new();
        theme.insert_extension(CustomWidgetTheme {
            radius: 12.0,
            density: 3,
        });

        let extension = theme
            .extension_arc::<CustomWidgetTheme>()
            .expect("custom widget theme present");
        assert_eq!(
            Arc::as_ref(&extension),
            &CustomWidgetTheme {
                radius: 12.0,
                density: 3,
            }
        );

        let removed = theme
            .remove_extension::<CustomWidgetTheme>()
            .expect("custom widget theme removed");
        assert_eq!(
            Arc::as_ref(&removed),
            &CustomWidgetTheme {
                radius: 12.0,
                density: 3,
            }
        );
        assert!(!theme.has_extension::<CustomWidgetTheme>());
    }

    #[cfg(feature = "wgpu")]
    #[test]
    fn application_feather_width_is_configurable() {
        let options = WindowRenderOptions::new(true, 1.5)
            .with_color_management_mode(WindowColorManagementMode::PreferHdr)
            .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
            .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
            .with_tone_mapping_mode(WindowToneMappingMode::Reinhard);
        let app = Application::new()
            .with_feathering_enabled(false)
            .with_feather_width(2.25)
            .with_window_render_options(options);
        let clamped = Application::new().with_feather_width(-1.0);

        assert!(!app.feathering_enabled());
        assert_eq!(app.feather_width(), 2.25);
        assert_eq!(clamped.feather_width(), 0.0);
        assert_eq!(app.initial_window_render_options(), Some(options));
    }
}
