#![forbid(unsafe_code)]
// Conversion helpers borrow generated binding models, and binding constructors mirror the
// stable cross-language API rather than Rust-only builder conventions.
#![allow(clippy::too_many_arguments, clippy::wrong_self_convention)]

mod actions;
mod animation;
mod application;
mod collections;
mod diagnostics;
mod docking;
mod documents;
mod drag;
mod errors;
mod events;
mod foreign_widget;
mod graphics;
mod handles;
mod interop;
mod layout;
mod messages;
mod paint;
mod runtime;
mod shader;
mod state;
mod support;
mod tasks;
mod theme;
mod values;
mod widget_adapters;
mod widget_bindings;
mod widget_build;
mod widget_descriptor;
mod widget_factory;

pub use actions::BindingAction;
pub use actions::BindingBoolAction;
pub use actions::BindingColorAction;
pub use actions::BindingColorSelectAction;
pub use actions::BindingIdAction;
pub use actions::BindingNumberAction;
pub use actions::BindingReorderAction;
pub use actions::BindingSelectAction;
pub use actions::BindingStringAction;
pub use actions::BindingStringsAction;
pub use animation::BindingAnimatedValue;
pub use animation::BindingAnimationClip;
pub use animation::BindingAnimationDocument;
pub use animation::BindingAnimationEditor;
pub use animation::BindingAnimationKeyframe;
pub use animation::BindingAnimationPlayer;
pub use animation::BindingAnimationSample;
pub use animation::BindingAnimationTimeline;
pub use animation::BindingAnimationTrack;
pub use animation::BindingAnimationValue;
pub use animation::BindingSpring;
pub use animation::BindingTransition;
pub use animation::binding_table_column_alignment_from_name;
pub use application::BindingApp;
pub use application::BindingRenderOptions;
pub use application::BindingWindow;
pub use application::registered_image_from_png;
pub use collections::BindingNotificationCenter;
pub use collections::BindingVirtualListItem;
pub use collections::BindingVirtualListModel;
pub use diagnostics::BindingCommandDispatchTrace;
pub use diagnostics::BindingEventRouteTrace;
pub use diagnostics::BindingFrameTiming;
pub use diagnostics::BindingInspectorSnapshot;
pub use diagnostics::BindingInvalidationTrace;
pub use diagnostics::BindingReactiveInvalidationTrace;
pub use diagnostics::BindingRenderSnapshot;
pub use diagnostics::BindingSemanticNode;
pub use diagnostics::BindingWidgetRebuildTrace;
pub use diagnostics::BindingWidgetTiming;
pub use diagnostics::binding_semantics_busy;
pub use diagnostics::binding_semantics_checked;
pub use diagnostics::binding_semantics_descriptions;
pub use diagnostics::binding_semantics_disabled;
pub use diagnostics::binding_semantics_editable_multiline;
pub use diagnostics::binding_semantics_expanded;
pub use diagnostics::binding_semantics_focused;
pub use diagnostics::binding_semantics_hidden;
pub use diagnostics::binding_semantics_hovered;
pub use diagnostics::binding_semantics_names;
pub use diagnostics::binding_semantics_nodes;
pub use diagnostics::binding_semantics_role_from_name;
pub use diagnostics::binding_semantics_role_name;
pub use diagnostics::binding_semantics_roles;
pub use diagnostics::binding_semantics_selected;
pub use diagnostics::binding_semantics_value_text;
pub use diagnostics::binding_semantics_values;
pub use diagnostics::binding_toggle_state_from_name;
pub use diagnostics::binding_toggle_state_name;
pub use docking::BindingDockFloatingGroup;
pub use docking::BindingDockLayout;
pub use docking::BindingDockNode;
pub use docking::BindingDockPanel;
pub use docking::BindingDockState;
pub use docking::BindingFloatingView;
pub use docking::BindingFloatingViewSnapshot;
pub use docking::BindingFloatingWorkspaceState;
pub use documents::BindingRichDocument;
pub use documents::BindingRichDocumentUpdate;
pub use drag::BindingDragScope;
pub use errors::ForeignCallbackError;
pub use errors::ForeignCallbackFailure;
pub use errors::ForeignCallbackPhase;
pub use errors::ForeignCallbackResult;
pub use errors::ForeignErrorSink;
pub use errors::ForeignWidgetId;
pub use events::BindingCustomEvent;
pub use events::BindingEvent;
pub use events::BindingImeEvent;
pub use events::BindingKeyState;
pub use events::BindingKeyboardEvent;
pub use events::BindingModifiers;
pub use events::BindingPointerButton;
pub use events::BindingPointerEvent;
pub use events::BindingPointerEventKind;
pub use events::BindingPointerKind;
pub use events::BindingRawMouseMotionEvent;
pub use events::BindingScrollDelta;
pub use events::BindingWindowEvent;
pub use foreign_widget::BindingEventContext;
pub use foreign_widget::ForeignArrangeCtx;
pub use foreign_widget::ForeignEventCtx;
pub use foreign_widget::ForeignMeasureCtx;
pub use foreign_widget::ForeignPaintCtx;
pub use foreign_widget::ForeignSemanticsCtx;
pub use foreign_widget::ForeignWidget;
pub use foreign_widget::ForeignWidgetCallbacks;
pub use graphics::BindingBrushPreviewSpec;
pub use graphics::BindingCanvasShape;
pub use graphics::BindingCanvasStroke;
pub use graphics::BindingCanvasViewport;
pub use graphics::BindingFloatingStackWindow;
pub use graphics::BindingImageFit;
pub use graphics::BindingPixelCanvasExport;
pub use graphics::BindingPixelCanvasState;
pub use graphics::BindingScrollAxes;
pub use handles::BindingFontHandle;
pub use handles::BindingImageHandle;
pub use handles::BindingWindowId;
pub use handles::resolve_binding_image_slots;
pub use interop::ExternalBackendHandle;
pub use interop::ExternalSync;
pub use interop::ExternalTextureDescriptor;
pub use interop::ExternalTextureFormat;
pub use interop::ExternalTextureValidationError;
pub use interop::NativeGraphicsBackend;
pub use interop::RendererInteropCapabilities;
pub use interop::RendererInteropTier;
pub use layout::BindingConstraintCase;
pub use layout::BindingMasterDetailState;
pub use layout::BindingResponsiveSidebarState;
pub use messages::BindingMessageAction;
pub use messages::BindingValue;
pub use paint::PaintCommand;
pub use paint::PaintCommandBuilder;
pub use paint::PaintValidationError;
pub use paint::PaintValidationErrorKind;
pub use paint::PaintValidationResult;
pub use runtime::BindingRuntime;
pub use shader::BindingShader;
pub use state::BindingState;
pub use state::BindingStateSubscription;
pub use support::widget_invalidation;
pub use tasks::BindingUiHandle;
pub use tasks::UiTaskQueue;
pub use theme::BindingTheme;
pub use values::BindingBool;
pub use values::BindingColorPaletteSwatch;
pub use values::BindingLayerListItem;
pub use values::BindingMenuItem;
pub use values::BindingNumber;
pub use values::BindingSegmentedControlItem;
pub use values::BindingStatusBarSegment;
pub use values::BindingTableColumn;
pub use values::BindingTableRow;
pub use values::BindingText;
pub use values::BindingTextSpan;
pub use values::BindingToolPaletteItem;
pub use values::BindingTreeItem;
pub use values::binding_alignment_from_name;
pub use values::binding_animation_property_from_path;
pub use values::binding_aspect_ratio_fit_from_name;
pub use values::binding_easing_from_name;
pub use values::binding_icon_glyph_from_name;
pub use values::binding_icon_glyph_name;
pub use values::binding_safe_area_edges_from_name;
pub use values::binding_semantic_tone_from_name;
pub use values::binding_simple_color_picker_mode_from_name;
pub use values::binding_surface_border_from_name;
pub use values::binding_surface_elevation_from_name;
pub use values::binding_surface_role_from_name;
pub use values::binding_tooltip_placement_from_name;
pub use widget_descriptor::BindingWidget;

#[cfg(test)]
use sui::Axis;
#[cfg(test)]
use sui::Brush;
#[cfg(test)]
use sui::BrushPreviewShape;
#[cfg(test)]
use sui::Color;
#[cfg(test)]
use sui::ColorSpace;
#[cfg(test)]
use sui::Constraints;
#[cfg(test)]
use sui::Easing;
#[cfg(test)]
use sui::Event;
#[cfg(test)]
use sui::FontHandle;
#[cfg(test)]
use sui::IconGlyph;
#[cfg(test)]
use sui::ImageHandle;
#[cfg(test)]
use sui::Insets;
#[cfg(test)]
use sui::Path;
#[cfg(test)]
use sui::Point;
#[cfg(test)]
use sui::Rect;
#[cfg(test)]
use sui::SemanticTone;
#[cfg(test)]
use sui::SemanticsNode;
#[cfg(test)]
use sui::ShadowParams;
#[cfg(test)]
use sui::SideSheetPlacement;
#[cfg(test)]
use sui::Size;
#[cfg(test)]
use sui::StrokeStyle;
#[cfg(test)]
use sui::SurfaceElevation;
#[cfg(test)]
use sui::SurfaceRole;
#[cfg(test)]
use sui::TableColumnAlignment;
#[cfg(test)]
use sui::TextStyle;
#[cfg(test)]
use sui::Transform;
#[cfg(test)]
use sui::Vector;
#[cfg(test)]
use sui::WidgetShader;
#[cfg(test)]
use sui::WindowEvent;

#[cfg(test)]
use crate::support::recover_lock;

#[cfg(test)]
mod tests;
