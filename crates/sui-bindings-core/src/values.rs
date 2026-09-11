use crate::application::normalized_option_name;
use crate::messages::BindingValue;
use crate::state::BindingState;
use crate::tasks::BindingUiHandle;
use sui::Alignment;
use sui::AnimationProperty;
use sui::AnimationPropertyPath;
use sui::AspectRatioFit;
use sui::Color;
use sui::ColorPaletteSwatch;
use sui::Easing;
use sui::IconGlyph;
use sui::LayerListItem;
use sui::MenuItem;
use sui::SafeAreaEdges;
use sui::SegmentedControlItem;
use sui::SemanticTone;
use sui::SimpleColorPickerMode;
use sui::StatusBarSegment;
use sui::SurfaceBorder;
use sui::SurfaceElevation;
use sui::SurfaceRole;
use sui::TableColumn;
use sui::TableColumnAlignment;
use sui::TableRow;
use sui::TextSpan;
use sui::TextStyle;
use sui::ToolPaletteItem;
use sui::TooltipPlacement;
use sui::TreeItem;

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

    pub(crate) fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
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

    pub(crate) fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
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

    pub(crate) fn state(&self) -> Option<BindingState> {
        match self {
            Self::State(state) => Some(state.clone()),
            Self::Static(_) => None,
        }
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
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

    pub(crate) fn into_sui(&self) -> TextSpan {
        TextSpan::new(self.text.clone(), self.style.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BindingStatusBarSegment {
    pub(crate) text: BindingText,
    pub(crate) tone: SemanticTone,
    pub(crate) min_width: Option<f32>,
    pub(crate) expand: bool,
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

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        self.text.bind_ui_handle(handle);
    }

    pub(crate) fn into_sui(&self) -> StatusBarSegment {
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
    pub(crate) label: String,
    pub(crate) semantic_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) disabled: bool,
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

    pub(crate) fn into_sui(&self) -> SegmentedControlItem {
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
    pub(crate) title: String,
    pub(crate) width: Option<f32>,
    pub(crate) min_width: Option<f32>,
    pub(crate) alignment: TableColumnAlignment,
    pub(crate) numeric: bool,
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

    pub(crate) fn into_sui(&self) -> TableColumn {
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
    pub(crate) cells: Vec<String>,
}

impl BindingTableRow {
    pub fn new(cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }

    pub(crate) fn into_sui(&self) -> TableRow {
        TableRow::new(self.cells.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BindingTreeItem {
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) expanded: bool,
    pub(crate) disabled: bool,
    pub(crate) children: Vec<BindingTreeItem>,
}

impl BindingTreeItem {
    pub fn new(
        label: impl Into<String>,
        detail: Option<String>,
        expanded: bool,
        disabled: bool,
        children: impl IntoIterator<Item = BindingTreeItem>,
    ) -> Self {
        Self {
            label: label.into(),
            detail,
            expanded,
            disabled,
            children: children.into_iter().collect(),
        }
    }

    pub(crate) fn into_sui(&self) -> TreeItem {
        let mut item = TreeItem::new(self.label.clone()).expanded(self.expanded);
        if let Some(detail) = &self.detail {
            item = item.detail(detail.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        item.children(self.children.iter().map(BindingTreeItem::into_sui))
    }
}

#[derive(Debug, Clone)]
pub struct BindingLayerListItem {
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) visible: bool,
    pub(crate) locked: bool,
    pub(crate) disabled: bool,
}

impl BindingLayerListItem {
    pub fn new(
        label: impl Into<String>,
        detail: Option<String>,
        visible: bool,
        locked: bool,
        disabled: bool,
    ) -> Self {
        Self {
            label: label.into(),
            detail,
            visible,
            locked,
            disabled,
        }
    }

    pub(crate) fn into_sui(&self) -> LayerListItem {
        let mut item = LayerListItem::new(self.label.clone())
            .visible(self.visible)
            .locked(self.locked);
        if let Some(detail) = &self.detail {
            item = item.detail(detail.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        item
    }
}

#[derive(Debug, Clone)]
pub struct BindingMenuItem {
    pub(crate) label: String,
    pub(crate) shortcut: Option<String>,
    pub(crate) disabled: bool,
    pub(crate) destructive: bool,
    pub(crate) separator_before: bool,
    pub(crate) submenu: Vec<BindingMenuItem>,
}

impl BindingMenuItem {
    pub fn new(
        label: impl Into<String>,
        shortcut: Option<String>,
        disabled: bool,
        destructive: bool,
        separator_before: bool,
        submenu: Vec<BindingMenuItem>,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut,
            disabled,
            destructive,
            separator_before,
            submenu,
        }
    }

    pub(crate) fn into_sui(&self) -> MenuItem {
        let mut item = MenuItem::new(self.label.clone());
        if let Some(shortcut) = &self.shortcut {
            item = item.shortcut(shortcut.clone());
        }
        if self.disabled {
            item = item.disabled();
        }
        if self.destructive {
            item = item.destructive();
        }
        if self.separator_before {
            item = item.separator_before();
        }
        if !self.submenu.is_empty() {
            item = item.submenu(self.submenu.iter().map(BindingMenuItem::into_sui));
        }
        item
    }
}

#[derive(Debug, Clone)]
pub struct BindingToolPaletteItem {
    pub(crate) icon: IconGlyph,
    pub(crate) label: String,
    pub(crate) disabled: bool,
}

impl BindingToolPaletteItem {
    pub fn new(icon: IconGlyph, label: impl Into<String>, disabled: bool) -> Self {
        Self {
            icon,
            label: label.into(),
            disabled,
        }
    }

    pub(crate) fn into_sui(&self) -> ToolPaletteItem {
        let item = ToolPaletteItem::new(self.icon, self.label.clone());
        if self.disabled { item.disabled() } else { item }
    }
}

#[derive(Debug, Clone)]
pub struct BindingColorPaletteSwatch {
    pub(crate) name: String,
    pub(crate) color: Color,
}

impl BindingColorPaletteSwatch {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    pub(crate) fn into_sui(&self) -> ColorPaletteSwatch {
        ColorPaletteSwatch::new(self.name.clone(), self.color)
    }
}

pub(crate) fn binding_number_to_index(value: f64) -> Option<usize> {
    if value.is_finite() && value >= 0.0 {
        Some(value.floor() as usize)
    } else {
        None
    }
}

pub(crate) fn normalize_binding_name(value: &str) -> String {
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

pub fn binding_alignment_from_name(value: &str) -> Option<Alignment> {
    match normalize_binding_name(value).as_str() {
        "start" | "left" | "top" => Some(Alignment::Start),
        "center" | "centre" | "middle" => Some(Alignment::Center),
        "end" | "right" | "bottom" => Some(Alignment::End),
        "stretch" | "fill" => Some(Alignment::Stretch),
        _ => None,
    }
}

pub fn binding_tooltip_placement_from_name(value: &str) -> Option<TooltipPlacement> {
    match normalize_binding_name(value).as_str() {
        "above" | "top" => Some(TooltipPlacement::Above),
        "below" | "bottom" => Some(TooltipPlacement::Below),
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

pub fn binding_simple_color_picker_mode_from_name(value: &str) -> Option<SimpleColorPickerMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hsl" => Some(SimpleColorPickerMode::Hsl),
        "hsv" | "hsb" => Some(SimpleColorPickerMode::Hsv),
        "rgb" => Some(SimpleColorPickerMode::Rgb),
        _ => None,
    }
}

pub fn binding_aspect_ratio_fit_from_name(value: &str) -> Option<AspectRatioFit> {
    match normalized_option_name(value).as_str() {
        "contain" => Some(AspectRatioFit::Contain),
        "cover" => Some(AspectRatioFit::Cover),
        _ => None,
    }
}

pub fn binding_safe_area_edges_from_name(value: &str) -> Option<SafeAreaEdges> {
    let normalized = normalized_option_name(value);
    match normalized.as_str() {
        "none" => return Some(SafeAreaEdges::NONE),
        "all" | "" => return Some(SafeAreaEdges::ALL),
        "horizontal" => return Some(SafeAreaEdges::HORIZONTAL),
        "vertical" => return Some(SafeAreaEdges::VERTICAL),
        _ => {}
    }
    let mut edges = SafeAreaEdges::NONE;
    for edge in value.split([',', '|', ' ']).filter(|edge| !edge.is_empty()) {
        edges = edges.union(match normalized_option_name(edge).as_str() {
            "left" => SafeAreaEdges::LEFT,
            "top" => SafeAreaEdges::TOP,
            "right" => SafeAreaEdges::RIGHT,
            "bottom" => SafeAreaEdges::BOTTOM,
            _ => return None,
        });
    }
    Some(edges)
}

pub fn binding_easing_from_name(value: &str) -> Option<Easing> {
    match normalized_option_name(value).as_str() {
        "linear" => Some(Easing::Linear),
        "easein" => Some(Easing::EaseIn),
        "easeout" => Some(Easing::EaseOut),
        "easeinout" => Some(Easing::EaseInOut),
        _ => None,
    }
}

pub fn binding_animation_property_from_path(path: &str) -> AnimationProperty {
    match normalize_binding_name(path).as_str() {
        "layeropacity" | "opacity" => AnimationProperty::LayerOpacity,
        "layertranslation" | "translation" => AnimationProperty::LayerTranslation,
        "fillcolor" | "color" => AnimationProperty::FillColor,
        "bounds" => AnimationProperty::Bounds,
        _ => AnimationProperty::Custom(AnimationPropertyPath::new(path)),
    }
}
