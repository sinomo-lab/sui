//! Public exports for the feature modules below.

mod dialogs;
mod forms;
mod indicators;
mod navigation;
mod painting;
mod popups;
mod status;
mod surfaces;
mod toolbars;

pub use dialogs::BottomSheet;
pub use dialogs::CommandPalette;
pub use dialogs::Dialog;
pub use dialogs::Drawer;
pub use dialogs::Modal;
pub use dialogs::SheetState;
pub use dialogs::SideSheet;
pub use dialogs::SideSheetPlacement;
pub use forms::ActionCard;
pub use forms::DetailRow;
pub use forms::DockPanel;
pub use forms::FieldGroup;
pub use forms::FormRow;
pub use forms::FormSection;
pub use forms::PanelSection;
pub use forms::PropertyRow;
pub use forms::PropertyRowLayout;
pub use forms::SectionLabel;
pub use forms::SectionLabelPaint;
pub use forms::detail_row_height_for_value;
pub use forms::paint_detail_row_at;
pub use forms::paint_section_label;
pub use forms::paint_section_label_detail;
pub use indicators::BusyIndicator;
pub use indicators::CoverageDots;
pub use indicators::CoverageDotsConfig;
pub use indicators::PlacementBadge;
pub use indicators::PlacementBadgePaint;
pub use indicators::ProgressBar;
pub use indicators::Spinner;
pub use indicators::paint_coverage_dots;
pub use indicators::paint_coverage_dots_with_config;
pub use indicators::paint_placement_badge;
pub use indicators::paint_placement_badge_with;
pub use indicators::paint_progress_bar;
pub use navigation::BrowserTabBar;
pub use navigation::SegmentedControl;
pub use navigation::SegmentedControlItem;
pub use navigation::TabBar;
pub use navigation::TabBarItem;
pub use navigation::Tabs;
pub use painting::ActionTilePaint;
pub use painting::CalloutPaint;
pub use painting::CodePanelPaint;
pub use painting::CodeTextLine;
pub use painting::CodeTextPaint;
pub use painting::CodeTextSpan;
pub use painting::CommandButtonFill;
pub use painting::CommandButtonPaint;
pub use painting::DisclosureButtonPaint;
pub use painting::EmptyStatePaint;
pub use painting::HairlineEdge;
pub use painting::SectionPanelGeometry;
pub use painting::SectionPanelPaint;
pub use painting::paint_action_tile;
pub use painting::paint_border;
pub use painting::paint_callout;
pub use painting::paint_code_lines;
pub use painting::paint_code_panel;
pub use painting::paint_command_button;
pub use painting::paint_disclosure_button;
pub use painting::paint_empty_state;
pub use painting::paint_hairline;
pub use painting::paint_rounded_panel;
pub use painting::paint_rounded_rect;
pub use painting::paint_section_panel;
pub use popups::ContextMenu;
pub use popups::Menu;
pub use popups::Popover;
pub use popups::PopoverAlignment;
pub use popups::Tooltip;
pub use popups::TooltipAlignment;
pub use popups::TooltipPlacement;
pub use status::EmptyState;
pub use status::PresetStrip;
pub use status::StatusBadge;
pub use status::StatusBar;
pub use status::StatusBarHost;
pub use status::StatusBarSegment;
pub use status::paint_status_badge;
pub use surfaces::FramedField;
pub use surfaces::Surface;
pub use surfaces::SurfaceAppearance;
pub use surfaces::SurfaceBorder;
pub use surfaces::SurfaceElevation;
pub use surfaces::SurfaceRole;
pub use toolbars::CommandGroup;
pub use toolbars::MenuItem;
pub use toolbars::ToolPalette;
pub use toolbars::ToolPaletteItem;
pub use toolbars::Toolbar;

#[cfg(test)]
use crate::composites::forms::{
    detail_row_label_style, detail_row_value_style, dock_panel_title_id, panel_section_title_id,
    property_row_label_id, section_label_text_style,
};
#[cfg(test)]
use crate::composites::indicators::{inset_rect, mix_color, rect_center, text_token_style};
#[cfg(test)]
use crate::composites::navigation::{browser_tab_close_semantics_id, browser_tab_semantics_id};

#[cfg(test)]
use crate::composites::popups::AnimatedScalar;
#[cfg(test)]
use crate::composites::status::status_bar_segment_id;

#[cfg(test)]
mod tests;
