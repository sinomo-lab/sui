#![forbid(unsafe_code)]

//! Public Rust facade for SUI.
//!
//! Most applications can import [`prelude`] and construct an [`App`] with one
//! or more [`Window`] values. Lower-level runtime, scene, renderer, and widget
//! types are re-exported for custom widgets, embedding, tests, and tooling.
//!
//! ```no_run
//! use sui::prelude::*;
//!
//! fn main() -> Result<()> {
//!     App::new()
//!         .main_window("Hello SUI", Label::new("Ready"))
//!         .run()
//! }
//! ```

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    sync::Arc,
};

/// User-facing application, window, resource, and wake-handle builders.
pub mod app;
/// Composite widgets and application-shell building blocks.
pub mod composites;
/// Layout and containment widgets.
pub mod containers;
/// Primitive controls, labels, inputs, and icons.
pub mod controls;

#[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
pub use app::UiHandle;
pub use app::{App, ResourceRegistry, Window};

pub use composites::{
    ActionTilePaint, BottomSheet, BrowserTabBar, CalloutPaint, CodePanelPaint, CodeTextLine,
    CodeTextPaint, CodeTextSpan, CommandButtonFill, CommandButtonPaint, CommandGroup,
    CommandPalette, ContextMenu, ContextMenuHandle, Dialog, DisclosureButtonPaint, DockPanel,
    EmptyState, EmptyStatePaint, FieldGroup, FormRow, FormSection, FramedField, HairlineEdge, Menu,
    MenuItem, PanelSection, Popover, PopoverAlignment, PresetStrip, ProgressBar, PropertyRow,
    PropertyRowLayout, SectionLabel, SectionLabelPaint, SectionPanelGeometry, SectionPanelPaint,
    ShadowBox, SheetState, SideSheet, SideSheetPlacement, Spinner, StatusBar, StatusBarHost,
    StatusBarSegment, Surface, SurfaceAppearance, SurfaceBorder, SurfaceElevation, SurfaceRole,
    TabBar, TabBarItem, Tabs, ToolPalette, ToolPaletteItem, Toolbar, Tooltip, TooltipAlignment,
    TooltipPlacement, paint_action_tile, paint_border, paint_callout, paint_code_lines,
    paint_code_panel, paint_command_button, paint_disclosure_button, paint_empty_state,
    paint_hairline, paint_placement_badge_with, paint_rounded_panel, paint_rounded_rect,
    paint_section_label, paint_section_label_detail, paint_section_panel,
};
// Former names, kept so existing code keeps building; naming one warns.
#[allow(deprecated)]
pub use composites::{BusyIndicator, Drawer, Modal};
pub use containers::{
    Align, Background, ContentExtent, Dock, FixedPaneSplit, Flex, MeasuredBottomDock, Overflow,
    Padding, RebuildOnChange, RebuildOnConstraints, ScrollAxes, ScrollBar, ScrollState, ScrollView,
    SemanticRegion, SizedBox, Stack, SwitchView, TrailingSlotRow, VirtualScrollView,
};
#[allow(deprecated)]
pub use controls::register_builtin_icon_resources;
pub use controls::{
    BUILTIN_ICON_GLYPHS, Button, ButtonAppearance, Checkbox, CheckboxIndicatorState,
    ChoiceAppearance, DateTimeInput, FieldAppearance, Icon, IconButton, IconButtonPaint, IconGlyph,
    InteractionPreview, Label, Link, NumberInput, PasswordInput, RadioButton, RadioGroup, Select,
    Separator, Slider, Switch, TextArea, TextInput, draw_glyph, paint_checkbox_indicator,
    paint_icon_button,
};
// Former names, kept so existing code keeps building; naming one warns.
#[allow(deprecated)]
pub use controls::{ComboBox, Divider, MultilineTextInput, SpinBox};
pub use sui_core::{
    AsyncWakeToken, Clipboard, ClipboardBackend, Color, ColorSpace, CursorGrabMode, CustomEvent,
    DirtyRegion, DpiInfo, DragDropScope, DragEvent, DragEventKind, DragOutcome, DragPayload,
    DragPreview, DragScopeId, DragSessionId, DropEffect, DropEffects, Error, Event, FontHandle,
    ImageHandle, ImeEvent, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    KeyboardEvent, LocalClipboardBackend, Modifiers, MotionPolicy, MotionPreference, Oklch, Path,
    PathBuilder, PathElement, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    PointerKind, RawMouseMotionEvent, Rect, Result, SafeAreaInsets, ScrollDelta, SemanticsAction,
    SemanticsActionRequest, SemanticsEvent, SemanticsLiveRegion, SemanticsNode, SemanticsPopupKind,
    SemanticsRelations, SemanticsRole, SemanticsState, SemanticsTextRange, SemanticsValue, Size,
    SurfaceId, TimerToken, ToggleState, Transform, Vector, WakeEvent, WidgetId, WindowEvent,
    WindowId,
};
pub use sui_layout::Padding as Insets;
pub use sui_layout::{
    Alignment, Axis, Constraints, FlexAlignContent, FlexBasis, FlexItem, FlexItemLayout,
    FlexJustify, FlexLayout, FlexLineLayout, FlexStyle, FlexWrap, GridItem, GridItemLayout,
    GridLayout, GridPlacement, GridStyle, GridTrack, GridTrackMax, IntrinsicSize, LayoutContext,
    arrange_flex, flex_layout, grid_layout,
};
#[cfg(all(target_os = "android", feature = "mobile"))]
pub use sui_platform::AndroidApp;
#[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
pub use sui_platform::{
    DesktopExtension, DesktopExtensionContext, DesktopPlatform, DesktopWindow, FileDialogFilter,
    FileDialogFuture, FileDialogMode, FileDialogRequest, FileDialogSelection, FileDialogService,
    HeadlessPlatform, NativeFileDialogs, PlatformFile, PlatformWindow, Waker, show_file_dialog,
};
pub use sui_reactive::{
    Change as ObservableChange, Observable, Observer, Selector, Signal, SourceId, Subscription,
    WeakObserver,
};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{
    DisplayCapabilities, DisplayColorPrimaries, OutputGamut, RendererCapabilities, RendererInterop,
    RequestedToneMappingMode, RgbaImage, StemDarkening, TextCoveragePolicy, TextHinting,
    WgpuExternalTextureContext, WgpuExternalTextureRegistry, WgpuRenderer,
};
pub use sui_runtime::{
    AnimateCtx, Application as RuntimeApplication, ArrangeCtx, Command, CommandController,
    CommandCtx, CommandDelivery, CommandKey, CommandSender, CommandTarget, DEFAULT_SUI_LOGO_SVG,
    EXTERNAL_WAKE_KIND, EmbeddedSvgImageResource, EventCtx, EventPhase, EventRoutePhase,
    EventRouteStep, FocusRestorePolicy, FocusScope, FocusScopeState, FocusState, FrameClock,
    FramePacing, FrameSchedule, KeyedChildren, KeyedReconcile, LayerOptions, MeasureCtx, Motion,
    OVERLAY_DISMISS_REQUEST, OutputColorRange, OverlayDismissPolicy, OverlayDismissReason,
    OverlayDismissRequest, OverlayFocusBehavior, OverlayInitialFocus, OverlayKind, OverlayModality,
    OverlayOptions, PaintBoundaryMode, PaintCtx, REACTIVE_CHANGED, RenderOutput, Runtime,
    SemanticsCtx, SingleChild, StackHostOptions, StackOrderPolicy, StackSurfaceOptions, Widget,
    WidgetChildren, WidgetDiagnosticsCtx, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    WindowBuilder, WindowColorManagementMode, WindowCursorState, WindowDynamicRangeMode,
    WindowIcon, WindowOutputColorPrimaries, WindowRenderOptions, WindowStemDarkening,
    WindowTextCoveragePolicy, WindowTextHinting, WindowTextSubpixelOrder, WindowToneMappingMode,
    app_motion_preference, clear_window_render_options, default_sui_logo_image, motion_policy,
    motion_time_scale, reset_motion_settings, set_app_motion_preference, set_motion_time_scale,
    set_system_motion_preference, set_window_render_options, system_motion_preference,
    window_output_color_range, window_output_color_range_signal, window_render_options,
};
pub use sui_scene::{
    Border, Brush, GradientStop, ImagePixelSnap, ImageRegistry, ImageSampling, ImageSource,
    LayerCompositionMode, LayerProperties, RegisteredExternalImage, RegisteredImage,
    RegisteredImageFormat, Scene, SceneCommand, SceneFrame, ShadowParams, ShadowPlacement,
    StrokeStyle, TextRenderCoveragePolicy, TextRenderHinting, TextRenderMode, TextRenderPolicy,
    TextRenderStemDarkening, TextSubpixelOrder, WidgetShader,
};
pub use sui_text::{
    FontFamilyStack, FontFeature, FontFeatures, FontRegistry, FontStretch, FontStyle, FontWeight,
    PersistentTextLayout, RegisteredFont, ResolvedTextFace, ShapedGlyph, ShapedText,
    ShapedTextWindow, TextAffinity, TextAlign, TextCluster, TextCursor, TextDirection,
    TextDocument, TextFlowDirection, TextGlyphInstance, TextLayout, TextLayoutHandle, TextLayoutId,
    TextLayoutMetadata, TextLayoutRegistry, TextLayoutRequest, TextLayoutRun, TextLayoutVersion,
    TextLayoutView, TextLine, TextLineWindow, TextMeasurement, TextParagraph, TextParagraphLayout,
    TextParagraphStyle, TextRun, TextRunView, TextSelection, TextSelectionGeometry, TextSpan,
    TextSpanId, TextStyle, TextWrap, TextWritingMode,
};
pub use sui_widgets::SignalMeter;
pub use sui_widgets::animation::{
    ANIMATION_DOCUMENT_VERSION, AnimatedValue, AnimationBinding, AnimationBindingInvalidation,
    AnimationDocument, AnimationDocumentFormatError, AnimationEditorCommand, AnimationEditorState,
    AnimationPlayer, AnimationProperty, AnimationPropertyPath, AnimationSelection, AnimationSpec,
    AnimationTargetId, AnimationTick, AnimationValue, AnimationValueKind, Blink, Clip,
    CompiledClip, CompiledTimeline, CompiledTrack, Easing, Interpolate, Keyframe,
    KeyframeSelection, LoopMode, MotionScalar, MotionValue, PlaybackState, Progress, Pulse,
    SampleBatch, SampleBuffer, SampledAnimationValue, SharedCompiledTimeline, SpringF32,
    SpringSpec, Timeline, TimelineBindingSink, TimelinePlayer, TimelineSnap, TimelineTick, Track,
    Transition, invalidation_for_animation_property,
};
pub use sui_widgets::animation::{
    MIN_ANIMATION_DOCUMENT_VERSION, PlaybackSpan, Stagger, StaggerOrigin, TimelineMarker,
};
pub use sui_widgets::{
    ActionCard, AdaptiveBreakpoints, AdaptiveClass, AdaptiveView, AspectRatio, AspectRatioFit,
    BasicSyntaxHighlighter, Breadcrumb, BreadcrumbItem, BrushPreview, BrushPreviewShape,
    BrushPreviewSpec, Canvas, CanvasAppearance, CanvasGridStyle, CanvasRuler,
    CanvasRulerAppearance, CanvasRulerAxis, CanvasShape, CanvasStroke, CanvasSurface,
    CanvasViewport, CanvasZoomBehavior, CanvasZoomContext, CollectionAnchor,
    CollectionAnchorGravity, CollectionChange, CollectionDelta, CollectionExtentIndex,
    CollectionModelError, CollectionSync, CollectionWindow, ColorPalette, ColorPaletteSwatch,
    ColorPicker, ColorPickerAppearance, ColorSwatch, ConstraintOrientation, ConstraintQuery,
    ConstraintView, ControlMetrics, ControlPalette, ControlSize, ControlTypography, CoverageDots,
    CoverageDotsConfig, DecorativeColors, DecorativeHue, DecorativePalette, DefaultTheme,
    DetailRow, DockFloatingGroup, DockLayoutError, DockNode, DockPanelId, DockWorkspace,
    DockWorkspaceSnapshot, DockWorkspaceState, DockZone, DragDropHost, Draggable, DropHover,
    DropTarget, EffectToken, FloatingStack, FloatingViewConfig, FloatingViewSnapshot,
    FloatingWorkspace, FloatingWorkspaceState, GlowTone, Grid, GridCell, HdrColorRoles,
    HdrEffectTokens, HdrLuminanceTokens, HdrMaterialTokens, HdrPolicyTokens, HdrThemeMode,
    HdrThemeTokens, Image, ImageFit, LayerList, LayerListItem, LayerListReorderChange,
    LayoutTransition, LeadingLabelCellPaint, ListItem, ListView, MasterDetail, MasterDetailRoute,
    MasterDetailState, MaterialToken, NeutralRamp, NotificationCenter, NotificationHost,
    NotificationId, NotificationUrgency, OverlayAlignment, OverlayCollisionPolicy, OverlayHost,
    OverlayPlacement, OverlayPlacementRequest, OverlayPlacementResult, OverlaySide, PixelCanvas,
    PixelCanvasAppearance, PixelCanvasBlendMode, PixelCanvasBrushShape, PixelCanvasExportSnapshot,
    PixelCanvasState, PixelCanvasTool, PlacementBadge, PlacementBadgePaint, ReorderableList,
    ReorderableListChange, ResolvedEffectStyle, ResolvedHdrStyle, ResolvedMaterialStyle,
    ResponsiveSidebar, ResponsiveSidebarMode, ResponsiveSidebarSnapshot, ResponsiveSidebarState,
    RichAttachment, RichBlockId, RichDocumentBlock, RichDocumentBlockKind, RichDocumentModel,
    RichDocumentRenderContext, RichDocumentRendererRegistry, RichDocumentSnapshot,
    RichDocumentSpan, RichDocumentStatus, RichDocumentUpdate, RichDocumentView,
    RichDocumentViewState, RichExtensionBlock, RichInlineImage, RichInlineKind, RichInlineStyle,
    RichLink, RichListItem, RichSyntaxHighlighter, RichSyntaxSpan, RichSyntaxTokenKind, RichText,
    RichTextSourceMap, RichTextSourceSpan, SafeArea, SafeAreaEdges, ScrollAlignment,
    SegmentedControl, SegmentedControlItem, SelectionChange, SelectionClipboardBehavior,
    SelectionEntry, SelectionIntent, SelectionOrder, SelectionOwnerId, SelectionPayload,
    SelectionPoint, SelectionScope, SemanticColorToken, SemanticTone, SimpleColorPicker,
    SimpleColorPickerMode, SplitExtent, SplitPaneSide, SplitState, SplitStateSnapshot, SplitView,
    StatusBadge, SurfacePalette, TEXT_COMMAND, Table, TableColumn, TableColumnAlignment, TableRow,
    TextBlockPaint, TextCellPaint, TextCommand, TextSelectionInfo, TextSurface,
    TextSurfaceOverlayKind, TextSurfaceStatus, TextSurfaceStyleOverlay, TextSurfaceStyleSpan,
    ThemeAspectRatios, ThemeBlurScale, ThemeBreakpoints, ThemeColorScheme, ThemeColors,
    ThemeContainers, ThemeDensity, ThemeFontFamilies, ThemeFontStack, ThemeFontWeights, ThemeGlows,
    ThemeLeading, ThemeMotion, ThemePerspective, ThemeRadii, ThemeShadow, ThemeShadowLayer,
    ThemeShadows, ThemeTextScale, ThemeTextToken, ThemeTracking, ToneRoles, TransientNotification,
    TreeItem, TreeView, VirtualCollectionModel, VirtualCollectionSource, VirtualList,
    VirtualListChrome, VirtualListSelectionMode, VirtualListState, VirtualTable,
    VirtualTableColumn, VirtualTableRowActivationKind, VirtualTableRowContext,
    VirtualTableSortDirection, VirtualTableState, VirtualViewportSnapshot, WidgetColorRole,
    WidgetEffectRole, WidgetLuminanceRole, WidgetMaterialRole, detail_row_height_for_value,
    paint_canvas_grid, paint_coverage_dots, paint_coverage_dots_with_config, paint_detail_row_at,
    paint_leading_label_cell, paint_placement_badge, paint_progress_bar, paint_status_badge,
    paint_text, paint_text_block, paint_text_cell, paint_text_line, paint_theme_glow,
    paint_theme_inset_shadow, paint_theme_shadow, place_overlay, resolve_effect_role,
    resolve_luminance_role, resolve_material_role, resolve_semantic_color,
    resolve_widget_hdr_style, wrap_text_lines,
};
// Former names, kept so existing code keeps building; naming one warns.
#[allow(deprecated)]
pub use sui_widgets::{DataGrid, PathBar, ResizablePane};
pub use sui_widgets::{KeyedStack, Presence, PresenceTransition};
pub use sui_widgets::{Paragraph, TextPlacement, TextShaper, VerticalAlign};

/// Marker trait for type-indexed application theme extensions.
pub trait ThemeExtension: Any + Send + Sync {}

impl<T> ThemeExtension for T where T: Any + Send + Sync {}

/// Type-indexed storage for application-specific theme values.
#[derive(Clone, Default)]
pub struct ThemeExtensions {
    values: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ThemeExtensions {
    /// Create empty extension storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace an extension, returning the previous value when present.
    pub fn insert<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|previous| Arc::downcast::<T>(previous).ok())
    }

    /// Borrow an extension by its concrete type.
    pub fn get<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    /// Clone the shared pointer for an extension by its concrete type.
    pub fn get_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| Arc::clone(value).downcast::<T>().ok())
    }

    /// Return whether an extension of type `T` is registered.
    pub fn contains<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.values.contains_key(&TypeId::of::<T>())
    }

    /// Remove and return an extension by its concrete type.
    pub fn remove<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|value| Arc::downcast::<T>(value).ok())
    }

    /// Return whether no extensions are registered.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Return the number of registered extension types.
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

/// Application-level colors, built-in widget theme, and custom extensions.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Default application background color.
    pub background: Color,
    /// Default application foreground color.
    pub foreground: Color,
    /// Theme used by SUI's built-in widgets.
    pub default_widgets: DefaultTheme,
    /// Type-indexed application theme extensions.
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
    /// Create the default application theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the theme used by built-in widgets.
    pub fn with_default_widgets(mut self, theme: DefaultTheme) -> Self {
        self.default_widgets = theme;
        self
    }

    /// Add a typed extension using builder syntax.
    pub fn with_extension<T>(mut self, value: T) -> Self
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value);
        self
    }

    /// Insert or replace a typed theme extension.
    pub fn insert_extension<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value)
    }

    /// Borrow a typed theme extension.
    pub fn extension<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.extensions.get::<T>()
    }

    /// Clone the shared pointer for a typed theme extension.
    pub fn extension_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.get_arc::<T>()
    }

    /// Return whether a typed theme extension is present.
    pub fn has_extension<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.extensions.contains::<T>()
    }

    /// Remove and return a typed theme extension.
    pub fn remove_extension<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.remove::<T>()
    }
}

/// Lower-level application builder for runtime and platform integration.
///
/// Regular applications should prefer [`App`]. This type remains public for
/// custom embedding, debug tooling, and direct [`Runtime`] construction.
pub struct Application {
    inner: RuntimeApplication,
    #[cfg(feature = "wgpu")]
    feathering_enabled: bool,
    #[cfg(feature = "wgpu")]
    feather_width: f32,
    #[cfg(feature = "wgpu")]
    external_texture_registry: Option<WgpuExternalTextureRegistry>,
    initial_window_render_options: Option<WindowRenderOptions>,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            inner: RuntimeApplication::default(),
            #[cfg(feature = "wgpu")]
            feathering_enabled: sui_render_wgpu::FeatheringOptions::default().enabled,
            #[cfg(feature = "wgpu")]
            feather_width: sui_render_wgpu::FeatheringOptions::default().width,
            #[cfg(feature = "wgpu")]
            external_texture_registry: None,
            initial_window_render_options: None,
        }
    }
}

impl Application {
    /// Create an empty application.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a lower-level window builder.
    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    /// Attach an application-scoped controller outside the widget tree.
    pub fn controller(mut self, controller: impl CommandController + 'static) -> Self {
        self.inner = self.inner.controller(controller);
        self
    }

    /// Subscribe an application-scoped typed command handler.
    pub fn on_command<T, F>(mut self, key: CommandKey<T>, handler: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.inner = self.inner.on_command(key, handler);
        self
    }

    #[cfg(feature = "wgpu")]
    /// Enable or disable analytic-edge feathering in the WGPU renderer.
    pub fn with_feathering_enabled(mut self, enabled: bool) -> Self {
        self.feathering_enabled = enabled;
        self
    }

    #[cfg(feature = "wgpu")]
    /// Set the analytic-edge feather width in logical pixels.
    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.feather_width = feather_width.max(0.0);
        self
    }

    #[cfg(feature = "wgpu")]
    /// Attach an app-owned external texture registry.
    pub fn with_external_texture_registry(mut self, registry: WgpuExternalTextureRegistry) -> Self {
        self.external_texture_registry = Some(registry);
        self
    }

    #[cfg(feature = "wgpu")]
    /// Return whether renderer feathering is enabled.
    pub fn feathering_enabled(&self) -> bool {
        self.feathering_enabled
    }

    #[cfg(feature = "wgpu")]
    /// Return the configured renderer feather width.
    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    #[cfg(feature = "wgpu")]
    /// Borrow the configured external texture registry, if present.
    pub fn external_texture_registry(&self) -> Option<&WgpuExternalTextureRegistry> {
        self.external_texture_registry.as_ref()
    }

    /// Set initial render options applied to every window at startup.
    pub fn with_window_render_options(mut self, options: WindowRenderOptions) -> Self {
        self.initial_window_render_options = Some(options.clamped());
        self
    }

    /// Return the initial render options, if configured.
    pub fn initial_window_render_options(&self) -> Option<WindowRenderOptions> {
        self.initial_window_render_options
    }

    /// Register a font with an explicit handle.
    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        self.inner.register_font(handle, font)
    }

    /// Register font bytes and allocate a handle.
    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        self.inner.register_font_bytes(data)
    }

    /// Register an image with an explicit handle.
    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        self.inner.register_image(handle, image)
    }

    /// Register RGBA8 pixels and allocate an image handle.
    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        self.inner.register_rgba_image(width, height, data)
    }

    /// Register SVG bytes at their intrinsic size and allocate an image handle.
    pub fn register_svg_image(&mut self, data: impl AsRef<[u8]>) -> Result<ImageHandle> {
        self.inner.register_svg_image(data)
    }

    /// Register SVG bytes at their intrinsic size with an explicit handle.
    pub fn register_svg_image_with_handle(
        &mut self,
        handle: ImageHandle,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner.register_svg_image_with_handle(handle, data)
    }

    /// Rasterize SVG bytes at an explicit size and allocate an image handle.
    pub fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<ImageHandle> {
        self.inner.register_svg_image_at_size(width, height, data)
    }

    /// Rasterize SVG bytes at an explicit size and handle.
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

    /// Register one embedded SVG resource.
    pub fn register_embedded_svg_image(
        &mut self,
        resource: EmbeddedSvgImageResource,
    ) -> Result<()> {
        self.inner.register_embedded_svg_image(resource)
    }

    /// Register multiple embedded SVG resources.
    pub fn register_embedded_svg_images(
        &mut self,
        resources: impl IntoIterator<Item = EmbeddedSvgImageResource>,
    ) -> Result<()> {
        self.inner.register_embedded_svg_images(resources)
    }

    /// Build the retained runtime without starting a platform event loop.
    pub fn build(self) -> Result<Runtime> {
        self.inner.build()
    }

    #[cfg(any(feature = "desktop", feature = "web"))]
    /// Build and run the application on the default desktop or web platform.
    pub fn run(self) -> Result<()> {
        self.run_with_platform(DesktopPlatform::new())
    }

    #[cfg(any(feature = "desktop", feature = "web"))]
    /// Build and run the application with a configured desktop platform host.
    ///
    /// This is the integration point for optional native-surface libraries.
    /// Renderer settings already configured on the application are applied to
    /// the supplied platform before the event loop starts.
    pub fn run_with_platform(self, platform: DesktopPlatform) -> Result<()> {
        self.run_with_platform_and_handle(platform, |_| {})
    }

    /// Like [`run_with_platform`](Self::run_with_platform) but invokes
    /// `on_ready` with the application's typed command sender.
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run_with_platform_and_handle(
        self,
        mut platform: DesktopPlatform,
        on_ready: impl FnOnce(CommandSender),
    ) -> Result<()> {
        let feathering_enabled = self.feathering_enabled;
        let feather_width = self.feather_width;
        let external_texture_registry = self.external_texture_registry.clone();
        let initial_window_render_options = self.initial_window_render_options;
        let runtime = self.build()?;
        platform.set_feathering_enabled(feathering_enabled);
        platform.set_feather_width(feather_width);
        if let Some(registry) = external_texture_registry {
            platform.set_external_texture_registry(registry);
        }
        if let Some(options) = initial_window_render_options {
            for window_id in runtime.window_ids() {
                set_window_render_options(window_id, options);
            }
        }
        let _ = platform.run_with(runtime, on_ready)?;
        Ok(())
    }

    /// Like [`run`](Self::run) but invokes `on_ready` with a typed,
    /// thread-safe [`CommandSender`] once the event loop is created.
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run_with(self, on_ready: impl FnOnce(CommandSender)) -> Result<()> {
        self.run_with_platform_and_handle(DesktopPlatform::new(), on_ready)
    }

    #[cfg(all(target_os = "android", feature = "mobile"))]
    /// Build and run the application in an Android native activity.
    pub fn run_android(self, android_app: AndroidApp) -> Result<()> {
        self.run_android_with(android_app, |_| {})
    }

    #[cfg(all(target_os = "android", feature = "mobile"))]
    /// Run on Android and invoke `on_ready` with a cross-thread command handle.
    pub fn run_android_with(
        self,
        android_app: AndroidApp,
        on_ready: impl FnOnce(CommandSender),
    ) -> Result<()> {
        let feathering_enabled = self.feathering_enabled;
        let feather_width = self.feather_width;
        let external_texture_registry = self.external_texture_registry.clone();
        let initial_window_render_options = self.initial_window_render_options;
        let runtime = self.build()?;
        let mut platform = DesktopPlatform::new()
            .with_feathering_enabled(feathering_enabled)
            .with_feather_width(feather_width);
        if let Some(registry) = external_texture_registry {
            platform.set_external_texture_registry(registry);
        }
        if let Some(options) = initial_window_render_options {
            for window_id in runtime.window_ids() {
                set_window_render_options(window_id, options);
            }
        }
        let _ = platform.run_android_with(runtime, android_app, on_ready)?;
        Ok(())
    }

    #[cfg(not(any(feature = "desktop", feature = "web", feature = "mobile")))]
    /// Return an error when no platform event-loop feature is enabled.
    pub fn run(self) -> Result<()> {
        let _ = self;
        Err(Error::new(
            "Application::run requires the `desktop`, `web`, or `mobile` feature to provide a platform event loop",
        ))
    }
}

/// Minimal shared style values for custom facade-level components.
#[deprecated(note = "nothing in SUI reads `Style`; keep such values in your own type")]
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Foreground brush used to draw content.
    pub foreground: Brush,
    /// Insets around the styled content.
    pub padding: Insets,
}

#[allow(deprecated)]
impl Default for Style {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            foreground: Brush::Solid(theme.foreground),
            padding: Insets::all(0.0),
        }
    }
}

/// Common imports for ordinary SUI application and widget code.
/// Tooling plumbing for inspectors, tests, and benchmarks: frame timings and
/// scene statistics, inspector snapshots and traces, renderer debug captures,
/// desktop automation, and accessibility audits. Applications rarely need it.
pub mod diagnostics {
    #[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
    pub use sui_platform::{
        AccessibilityIssue, AccessibilityIssueSeverity, AccessibilityIssueTarget,
        AccessibilitySnapshot, DebugCaptureTicket, DesktopAutomationAction,
        DesktopAutomationConfig, WindowOutputDiagnostics, request_window_debug_capture,
        take_window_debug_capture, validate_accessibility_snapshot, window_output_diagnostics,
        window_output_diagnostics_signal,
    };
    #[cfg(feature = "wgpu")]
    pub use sui_render_wgpu::{
        DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureId, DebugCaptureRequest,
        DebugCaptureStage, DebugSdrVisualization, HdrRgbaImage, OutputStrategy, fit_to_sdr,
    };
    pub use sui_runtime::{
        AsyncTaskInspectorSnapshot, COMMAND_HISTORY_LENGTH, CacheMetrics, CacheMetricsDelta,
        CommandDispatchSample, EventRouteTraceSample, FramePhase, FramePhaseSample,
        InspectorHistorySnapshot, InvalidationTraceSample, OverlayManagerSnapshot, OverlaySnapshot,
        OverlayTraceKind, OverlayTraceSample, PresentationLatencyDiagnostics,
        ReactiveInvalidationSample, RenderDiagnostics, RendererSubmissionDiagnostics,
        RetainedPacketRebuildDiagnostics, SceneStatistics, SceneStatisticsDetailMode,
        SchedulerInspectorSnapshot, TextCacheDeltaDiagnostics, TextCacheDiagnostics,
        TimerInspectorSnapshot, WidgetDiagnostic, WidgetDiagnosticsSnapshot,
        WidgetGeometrySnapshot, WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetRebuildSample,
        WindowInspectorSnapshot, WindowPerformanceSnapshot, WindowPerformanceSummary,
        set_window_scene_statistics_detail_mode, window_command_dispatches_signal,
        window_performance_snapshot, window_performance_summary,
        window_scene_statistics_detail_mode,
    };
}

pub mod prelude {
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub use crate::UiHandle;
    #[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
    pub use crate::{
        FileDialogFilter, FileDialogMode, FileDialogRequest, FileDialogSelection,
        FileDialogService, NativeFileDialogs, PlatformFile, show_file_dialog,
    };
    // Former names, kept so existing code keeps building; naming one warns.
    #[allow(deprecated)]
    pub use crate::{
        BusyIndicator, ComboBox, DataGrid, Divider, Drawer, Modal, MultilineTextInput, PathBar,
        ResizablePane, SpinBox,
    };

    pub use crate::{
        ActionCard, AdaptiveBreakpoints, AdaptiveClass, AdaptiveView, Align, Alignment, AnimateCtx,
        AnimatedValue, AnimationBinding, AnimationDocument, AnimationDocumentFormatError,
        AnimationPlayer, AnimationProperty, AnimationPropertyPath, AnimationSpec,
        AnimationTargetId, AnimationTick, AnimationValue, AnimationValueKind, App, ArrangeCtx,
        AspectRatio, AspectRatioFit, AsyncWakeToken, Axis, Background, BasicSyntaxHighlighter,
        Blink, BottomSheet, Breadcrumb, BreadcrumbItem, BrowserTabBar, Brush, BrushPreview,
        BrushPreviewShape, BrushPreviewSpec, Button, ButtonAppearance, Canvas, CanvasAppearance,
        CanvasGridStyle, CanvasRuler, CanvasRulerAppearance, CanvasRulerAxis, CanvasShape,
        CanvasStroke, CanvasSurface, CanvasViewport, CanvasZoomBehavior, CanvasZoomContext,
        Checkbox, ChoiceAppearance, Clip, CollectionAnchor, CollectionAnchorGravity,
        CollectionChange, CollectionDelta, CollectionExtentIndex, CollectionModelError,
        CollectionSync, CollectionWindow, Color, ColorPalette, ColorPaletteSwatch, ColorPicker,
        ColorPickerAppearance, ColorSwatch, Command, CommandController, CommandCtx,
        CommandDelivery, CommandGroup, CommandKey, CommandPalette, CommandSender, CommandTarget,
        ConstraintOrientation, ConstraintQuery, ConstraintView, Constraints, ContentExtent,
        ContextMenu, ContextMenuHandle, ControlMetrics, ControlPalette, ControlSize,
        ControlTypography, CoverageDots, CoverageDotsConfig, CursorGrabMode, DateTimeInput,
        DecorativeHue, DefaultTheme, DetailRow, Dialog, Dock, DockFloatingGroup, DockLayoutError,
        DockNode, DockPanel, DockPanelId, DockWorkspace, DockWorkspaceSnapshot, DockWorkspaceState,
        DockZone, DragDropHost, DragDropScope, DragEvent, DragEventKind, DragOutcome, DragPayload,
        DragPreview, DragScopeId, DragSessionId, Draggable, DropEffect, DropEffects, DropHover,
        DropTarget, Easing, EmptyState, Error, Event, EventCtx, EventPhase, FieldAppearance,
        FieldGroup, FixedPaneSplit, Flex, FlexAlignContent, FlexBasis, FlexItem, FlexJustify,
        FlexStyle, FlexWrap, FloatingViewConfig, FloatingViewSnapshot, FloatingWorkspace,
        FloatingWorkspaceState, FocusRestorePolicy, FocusScope, FocusScopeState, FontFeature,
        FontFeatures, FontHandle, FontStretch, FontStyle, FontWeight, FormRow, FormSection,
        FrameClock, FramedField, GlowTone, Grid, GridCell, GridItem, GridPlacement, GridStyle,
        GridTrack, GridTrackMax, Icon, IconButton, IconGlyph, Image, ImageFit, ImageHandle,
        ImeEvent, Insets, InteractionPreview, Interpolate, IntrinsicSize, KeyState, KeyboardEvent,
        KeyedChildren, KeyedReconcile, KeyedStack, Keyframe, Label, LayerList, LayerListItem,
        LayerListReorderChange, LayoutTransition, Link, ListItem, ListView, LoopMode, MasterDetail,
        MasterDetailRoute, MasterDetailState, MeasureCtx, MeasuredBottomDock, Menu, MenuItem,
        Modifiers, Motion, MotionPolicy, MotionPreference, MotionScalar, MotionValue, NeutralRamp,
        NotificationCenter, NotificationHost, NotificationId, NotificationUrgency, NumberInput,
        Observable, Observer, Overflow, OverlayAlignment, OverlayCollisionPolicy,
        OverlayDismissPolicy, OverlayDismissReason, OverlayFocusBehavior, OverlayHost, OverlayKind,
        OverlayOptions, OverlayPlacement, OverlayPlacementRequest, OverlayPlacementResult,
        OverlaySide, Padding, PaintCtx, PanelSection, PasswordInput, Path, PathBuilder,
        PixelCanvas, PixelCanvasAppearance, PixelCanvasBlendMode, PixelCanvasBrushShape,
        PixelCanvasExportSnapshot, PixelCanvasState, PixelCanvasTool, PlacementBadge,
        PlaybackState, Point, PointerButton, PointerEvent, Popover, Presence, PresenceTransition,
        PresetStrip, Progress, ProgressBar, PropertyRow, PropertyRowLayout, Pulse, RadioButton,
        RadioGroup, RawMouseMotionEvent, RebuildOnChange, RebuildOnConstraints, Rect,
        RegisteredFont, RegisteredImage, ReorderableList, ReorderableListChange, ResourceRegistry,
        ResponsiveSidebar, ResponsiveSidebarMode, ResponsiveSidebarSnapshot,
        ResponsiveSidebarState, Result, RichAttachment, RichBlockId, RichDocumentBlock,
        RichDocumentBlockKind, RichDocumentModel, RichDocumentRenderContext,
        RichDocumentRendererRegistry, RichDocumentSnapshot, RichDocumentSpan, RichDocumentStatus,
        RichDocumentUpdate, RichDocumentView, RichDocumentViewState, RichExtensionBlock,
        RichInlineImage, RichInlineKind, RichInlineStyle, RichLink, RichListItem,
        RichSyntaxHighlighter, RichSyntaxSpan, RichSyntaxTokenKind, RichText, RichTextSourceMap,
        RichTextSourceSpan, SafeArea, SafeAreaEdges, SafeAreaInsets, ScrollAlignment, ScrollAxes,
        ScrollBar, ScrollState, ScrollView, SectionLabel, SegmentedControl, SegmentedControlItem,
        Select, SelectionChange, SelectionClipboardBehavior, SelectionEntry, SelectionIntent,
        SelectionOrder, SelectionOwnerId, SelectionPayload, SelectionPoint, SelectionScope,
        Selector, SemanticRegion, SemanticTone, SemanticsCtx, Separator, ShadowBox, ShadowParams,
        ShadowPlacement, ShapedText, SheetState, SideSheet, SideSheetPlacement, Signal,
        SimpleColorPicker, SimpleColorPickerMode, SingleChild, Size, SizedBox, Slider, SourceId,
        Spinner, SplitExtent, SplitPaneSide, SplitState, SplitStateSnapshot, SplitView, SpringF32,
        SpringSpec, Stack, Stagger, StaggerOrigin, StatusBar, StatusBarHost, StatusBarSegment,
        StrokeStyle, Subscription, Surface, SurfaceAppearance, SurfaceBorder, SurfaceElevation,
        SurfacePalette, SurfaceRole, Switch, SwitchView, TabBar, TabBarItem, Table, TableColumn,
        TableColumnAlignment, TableRow, Tabs, TextArea, TextCommand, TextDocument, TextInput,
        TextLayout, TextMeasurement, TextParagraph, TextParagraphStyle, TextSelectionInfo,
        TextSpan, TextSpanId, TextStyle, TextWrap, Theme, ThemeAspectRatios, ThemeBlurScale,
        ThemeBreakpoints, ThemeColorScheme, ThemeColors, ThemeContainers, ThemeDensity,
        ThemeExtension, ThemeExtensions, ThemeFontFamilies, ThemeFontStack, ThemeFontWeights,
        ThemeGlows, ThemeLeading, ThemeMotion, ThemePerspective, ThemeRadii, ThemeShadow,
        ThemeShadowLayer, ThemeShadows, ThemeTextScale, ThemeTextToken, ThemeTracking, Timeline,
        TimelineMarker, TimelinePlayer, TimelineSnap, TimelineTick, TimerToken, ToolPalette,
        ToolPaletteItem, Toolbar, Tooltip, TooltipAlignment, TooltipPlacement, Track,
        TrailingSlotRow, Transform, TransientNotification, Transition, TreeItem, TreeView,
        VirtualCollectionModel, VirtualCollectionSource, VirtualList, VirtualListChrome,
        VirtualListSelectionMode, VirtualListState, VirtualScrollView, VirtualTable,
        VirtualTableColumn, VirtualTableRowActivationKind, VirtualTableRowContext,
        VirtualTableSortDirection, VirtualTableState, VirtualViewportSnapshot, WakeEvent,
        WeakObserver, Widget, WidgetChildren, WidgetPod, WidgetShader, Window, WindowCursorState,
        WindowRenderOptions, paint_text, paint_text_line,
    };
    pub use crate::{Paragraph, TextAlign, TextPlacement, TextShaper, VerticalAlign};
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{DefaultTheme, HdrThemeMode, HdrThemeTokens, Theme};
    #[cfg(feature = "wgpu")]
    use crate::{
        Application, WgpuExternalTextureRegistry, WindowColorManagementMode,
        WindowDynamicRangeMode, WindowOutputColorPrimaries, WindowRenderOptions,
        WindowToneMappingMode,
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

    #[test]
    fn prelude_exports_rich_text_document_types() {
        use crate::prelude::*;

        let style = TextStyle::new(Color::WHITE);
        let document = TextDocument {
            paragraphs: vec![TextParagraph::from_spans(vec![TextSpan::new(
                "Hello rich text",
                style,
            )])],
        };
        let _widget = RichText::new(document);
        let _theme = DefaultTheme::default().with_density(ThemeDensity::Compact);
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

    #[cfg(feature = "wgpu")]
    #[test]
    fn application_accepts_external_texture_registry() {
        let registry = WgpuExternalTextureRegistry::new();
        let app = Application::new().with_external_texture_registry(registry.clone());

        assert!(app.external_texture_registry().is_some());
        assert!(registry.context().is_none());
    }
}
