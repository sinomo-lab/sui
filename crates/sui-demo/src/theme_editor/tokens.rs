//! Every color the editor can change: the source colors of [`ThemeColors`],
//! grouped by job, and the derived palette roles that can be overridden.

use sui::prelude::*;

/// Source color groups, in panel order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TokenGroup {
    Brand,
    Status,
    Surfaces,
    Fills,
    Borders,
    Ink,
    Decorative,
}

impl TokenGroup {
    pub(super) const ALL: [Self; 7] = [
        Self::Brand,
        Self::Status,
        Self::Surfaces,
        Self::Fills,
        Self::Borders,
        Self::Ink,
        Self::Decorative,
    ];

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Brand => "Brand",
            Self::Status => "Status",
            Self::Surfaces => "Surfaces",
            Self::Fills => "Fills",
            Self::Borders => "Borders",
            Self::Ink => "Ink",
            Self::Decorative => "Decorative",
        }
    }

    pub(super) const fn summary(self) -> &'static str {
        match self {
            Self::Brand => "Primary actions, links, focus, and live emphasis.",
            Self::Status => "Information, success, warning, and danger.",
            Self::Surfaces => "Window, sidebar, content, and floating tiers.",
            Self::Fills => "Neutral control faces, tracks, and text fields.",
            Self::Borders => "Hairlines, dividers, and control outlines.",
            Self::Ink => "Text levels, from body to disabled.",
            Self::Decorative => "Categorical hues for tags, charts, and avatars.",
        }
    }

    pub(super) fn tokens(self) -> impl Iterator<Item = SourceToken> {
        SourceToken::ALL
            .iter()
            .copied()
            .filter(move |token| token.group() == self)
    }

    /// Where the group's fields live in [`ThemeColors`].
    pub(super) const fn container(self) -> SourceContainer {
        match self {
            Self::Brand | Self::Status => SourceContainer::Colors,
            Self::Surfaces | Self::Fills | Self::Borders | Self::Ink => SourceContainer::Neutrals,
            Self::Decorative => SourceContainer::Decorative,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceContainer {
    Colors,
    Neutrals,
    Decorative,
}

macro_rules! source_tokens {
    ($( $variant:ident => $group:ident, $label:literal, $field:ident, $($path:ident).+; )+) => {
        /// A source color in [`ThemeColors`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(super) enum SourceToken {
            $( $variant, )+
        }

        impl SourceToken {
            pub(super) const ALL: &'static [Self] = &[$( Self::$variant, )+];

            pub(super) const fn group(self) -> TokenGroup {
                match self {
                    $( Self::$variant => TokenGroup::$group, )+
                }
            }

            pub(super) const fn label(self) -> &'static str {
                match self {
                    $( Self::$variant => $label, )+
                }
            }

            /// The field name inside the token's [`SourceContainer`].
            pub(super) const fn field(self) -> &'static str {
                match self {
                    $( Self::$variant => stringify!($field), )+
                }
            }

            pub(super) fn get(self, colors: &ThemeColors) -> Color {
                match self {
                    $( Self::$variant => colors.$($path).+, )+
                }
            }

            pub(super) fn set(self, colors: &mut ThemeColors, color: Color) {
                match self {
                    $( Self::$variant => colors.$($path).+ = color, )+
                }
            }
        }
    };
}

source_tokens! {
    Primary => Brand, "Primary", primary, primary;
    OnPrimary => Brand, "On primary", on_primary, on_primary;
    Secondary => Brand, "Secondary", secondary, secondary;
    OnSecondary => Brand, "On secondary", on_secondary, on_secondary;
    Info => Status, "Info", info, info;
    OnInfo => Status, "On info", on_info, on_info;
    Success => Status, "Success", success, success;
    OnSuccess => Status, "On success", on_success, on_success;
    Warning => Status, "Warning", warning, warning;
    OnWarning => Status, "On warning", on_warning, on_warning;
    Danger => Status, "Danger", danger, danger;
    OnDanger => Status, "On danger", on_danger, on_danger;
    Window => Surfaces, "Window", window, neutrals.window;
    Subtle => Surfaces, "Subtle", subtle, neutrals.subtle;
    Panel => Surfaces, "Panel", panel, neutrals.panel;
    Overlay => Surfaces, "Overlay", overlay, neutrals.overlay;
    Control => Fills, "Control", control, neutrals.control;
    ControlHover => Fills, "Control hover", control_hover, neutrals.control_hover;
    ControlActive => Fills, "Control active", control_active, neutrals.control_active;
    Button => Fills, "Button", button, neutrals.button;
    ButtonHover => Fills, "Button hover", button_hover, neutrals.button_hover;
    ButtonActive => Fills, "Button active", button_active, neutrals.button_active;
    Field => Fills, "Field", field, neutrals.field;
    BorderSubtle => Borders, "Border subtle", border_subtle, neutrals.border_subtle;
    Border => Borders, "Border", border, neutrals.border;
    BorderStrong => Borders, "Border strong", border_strong, neutrals.border_strong;
    ControlOutline => Borders, "Control outline", border_control, neutrals.border_control;
    Text => Ink, "Text", text, neutrals.text;
    TextSecondary => Ink, "Text secondary", text_secondary, neutrals.text_secondary;
    TextTertiary => Ink, "Text tertiary", text_tertiary, neutrals.text_tertiary;
    TextDisabled => Ink, "Text disabled", text_disabled, neutrals.text_disabled;
    Red => Decorative, "Red", red, decorative.red;
    Orange => Decorative, "Orange", orange, decorative.orange;
    Amber => Decorative, "Amber", amber, decorative.amber;
    Green => Decorative, "Green", green, decorative.green;
    Teal => Decorative, "Teal", teal, decorative.teal;
    Cyan => Decorative, "Cyan", cyan, decorative.cyan;
    Blue => Decorative, "Blue", blue, decorative.blue;
    Violet => Decorative, "Violet", violet, decorative.violet;
    Magenta => Decorative, "Magenta", magenta, decorative.magenta;
}

impl SourceToken {
    /// For an "on" color, the fill it sits on.
    pub(super) const fn fill(self) -> Option<Self> {
        match self {
            Self::OnPrimary => Some(Self::Primary),
            Self::OnSecondary => Some(Self::Secondary),
            Self::OnInfo => Some(Self::Info),
            Self::OnSuccess => Some(Self::Success),
            Self::OnWarning => Some(Self::Warning),
            Self::OnDanger => Some(Self::Danger),
            _ => None,
        }
    }
}

macro_rules! role_tokens {
    ($( $variant:ident => $label:literal, $field:ident; )+) => {
        /// A derived [`ControlPalette`] role. Roles follow the source colors
        /// unless overridden.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(super) enum RoleToken {
            $( $variant, )+
        }

        impl RoleToken {
            pub(super) const ALL: &'static [Self] = &[$( Self::$variant, )+];

            pub(super) const fn label(self) -> &'static str {
                match self {
                    $( Self::$variant => $label, )+
                }
            }

            pub(super) const fn field(self) -> &'static str {
                match self {
                    $( Self::$variant => stringify!($field), )+
                }
            }

            pub(super) fn get(self, palette: &ControlPalette) -> Color {
                match self {
                    $( Self::$variant => palette.$field, )+
                }
            }

            pub(super) fn set(self, palette: &mut ControlPalette, color: Color) {
                match self {
                    $( Self::$variant => palette.$field = color, )+
                }
            }
        }
    };
}

role_tokens! {
    Text => "Text", text;
    TextMuted => "Text muted", text_muted;
    Placeholder => "Placeholder", placeholder;
    TextDisabled => "Text disabled", text_disabled;
    Surface => "Surface", surface;
    SurfaceRaised => "Surface raised", surface_raised;
    Control => "Control", control;
    ControlHover => "Control hover", control_hover;
    ControlActive => "Control active", control_active;
    Button => "Button", button;
    ButtonHover => "Button hover", button_hover;
    ButtonPressed => "Button pressed", button_pressed;
    ButtonBorder => "Button border", button_border;
    Field => "Field", field;
    SurfaceHover => "Surface hover", surface_hover;
    SurfacePressed => "Surface pressed", surface_pressed;
    SurfaceFocus => "Surface focus", surface_focus;
    Border => "Border", border;
    BorderStrong => "Border strong", border_strong;
    BorderHover => "Border hover", border_hover;
    BorderFocus => "Border focus", border_focus;
    BorderControl => "Control outline", border_control;
    Focus => "Focus", focus;
    FocusRing => "Focus ring", focus_ring;
    Caret => "Caret", caret;
    Selection => "Selection", selection;
    SelectionBorder => "Selection border", selection_border;
    Accent => "Accent", accent;
    AccentHover => "Accent hover", accent_hover;
    AccentPressed => "Accent pressed", accent_pressed;
    AccentBorder => "Accent border", accent_border;
    AccentBorderHover => "Accent border hover", accent_border_hover;
    AccentBorderFocus => "Accent border focus", accent_border_focus;
    AccentText => "On accent", accent_text;
    AccentSoft => "Accent soft", accent_soft;
    AccentSoftText => "Accent soft text", accent_soft_text;
    Info => "Info", info;
    InfoText => "On info", info_text;
    InfoSoft => "Info soft", info_soft;
    InfoSoftText => "Info soft text", info_soft_text;
    Success => "Success", success;
    SuccessText => "On success", success_text;
    SuccessSoft => "Success soft", success_soft;
    SuccessSoftText => "Success soft text", success_soft_text;
    Warning => "Warning", warning;
    WarningText => "On warning", warning_text;
    WarningSoft => "Warning soft", warning_soft;
    WarningSoftText => "Warning soft text", warning_soft_text;
    Danger => "Danger", danger;
    DangerText => "On danger", danger_text;
    DangerSoft => "Danger soft", danger_soft;
    DangerSoftText => "Danger soft text", danger_soft_text;
    DangerHover => "Danger hover", danger_hover;
}

/// Any color the editor can select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Token {
    Source(SourceToken),
    Role(RoleToken),
}

impl Token {
    /// The accessible name of the token's row. Role rows say so, since a
    /// few roles share a label with a source color.
    pub(super) fn row_name(self) -> String {
        match self {
            Self::Source(token) => token.label().to_string(),
            Self::Role(role) => format!("{} role", role.label()),
        }
    }
}
