use sui_core::Color;

use crate::theme::{DefaultTheme, ThemeColors};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HdrThemeMode {
    #[default]
    Disabled,
    WideGamutOnly,
    ConstrainedHdr,
    FullHdr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticColorToken {
    pub sdr: Color,
    pub wide_gamut: Option<Color>,
    pub hdr: Option<Color>,
}

impl SemanticColorToken {
    pub const fn new(sdr: Color, wide_gamut: Option<Color>, hdr: Option<Color>) -> Self {
        Self {
            sdr,
            wide_gamut,
            hdr,
        }
    }

    pub const fn from_sdr(sdr: Color) -> Self {
        Self::new(sdr, None, None)
    }

    pub const fn with_wide_gamut(self, wide_gamut: Color) -> Self {
        Self {
            wide_gamut: Some(wide_gamut),
            ..self
        }
    }

    pub const fn with_hdr(self, hdr: Color) -> Self {
        Self {
            hdr: Some(hdr),
            ..self
        }
    }

    pub fn resolve(self, mode: HdrThemeMode) -> Color {
        match mode {
            HdrThemeMode::Disabled => self.sdr,
            HdrThemeMode::WideGamutOnly => self.wide_gamut.unwrap_or(self.sdr),
            HdrThemeMode::ConstrainedHdr | HdrThemeMode::FullHdr => {
                self.hdr.or(self.wide_gamut).unwrap_or(self.sdr)
            }
        }
    }
}

impl Default for SemanticColorToken {
    fn default() -> Self {
        Self::from_sdr(Color::TRANSPARENT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetColorRole {
    Surface,
    SurfaceElevated,
    SurfaceOutline,
    Text,
    TextMuted,
    Accent,
    AccentText,
    Secondary,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrColorRoles {
    pub surface: SemanticColorToken,
    pub surface_elevated: SemanticColorToken,
    pub surface_outline: SemanticColorToken,
    pub text: SemanticColorToken,
    pub text_muted: SemanticColorToken,
    pub accent: SemanticColorToken,
    pub accent_text: SemanticColorToken,
    pub secondary: SemanticColorToken,
    pub info: SemanticColorToken,
    pub success: SemanticColorToken,
    pub warning: SemanticColorToken,
    pub danger: SemanticColorToken,
}

impl HdrColorRoles {
    pub fn from_colors(colors: ThemeColors) -> Self {
        Self {
            surface: SemanticColorToken::from_sdr(colors.base_100),
            surface_elevated: SemanticColorToken::from_sdr(colors.base_200),
            surface_outline: SemanticColorToken::from_sdr(colors.base_300),
            text: SemanticColorToken::from_sdr(colors.base_content),
            text_muted: SemanticColorToken::from_sdr(colors.base_content.with_alpha(0.72)),
            accent: SemanticColorToken::from_sdr(colors.primary),
            accent_text: SemanticColorToken::from_sdr(colors.primary_content),
            secondary: SemanticColorToken::from_sdr(colors.secondary),
            info: SemanticColorToken::from_sdr(colors.info),
            success: SemanticColorToken::from_sdr(colors.success),
            warning: SemanticColorToken::from_sdr(colors.warning),
            danger: SemanticColorToken::from_sdr(colors.error),
        }
    }

    pub fn from_default_theme(theme: DefaultTheme) -> Self {
        Self::from_colors(theme.colors)
    }

    pub fn sync_sdr_from_colors(&mut self, colors: ThemeColors) {
        let derived = Self::from_colors(colors);
        self.surface.sdr = derived.surface.sdr;
        self.surface_elevated.sdr = derived.surface_elevated.sdr;
        self.surface_outline.sdr = derived.surface_outline.sdr;
        self.text.sdr = derived.text.sdr;
        self.text_muted.sdr = derived.text_muted.sdr;
        self.accent.sdr = derived.accent.sdr;
        self.accent_text.sdr = derived.accent_text.sdr;
        self.secondary.sdr = derived.secondary.sdr;
        self.info.sdr = derived.info.sdr;
        self.success.sdr = derived.success.sdr;
        self.warning.sdr = derived.warning.sdr;
        self.danger.sdr = derived.danger.sdr;
    }

    pub fn for_widget_role(self, role: WidgetColorRole) -> SemanticColorToken {
        match role {
            WidgetColorRole::Surface => self.surface,
            WidgetColorRole::SurfaceElevated => self.surface_elevated,
            WidgetColorRole::SurfaceOutline => self.surface_outline,
            WidgetColorRole::Text => self.text,
            WidgetColorRole::TextMuted => self.text_muted,
            WidgetColorRole::Accent => self.accent,
            WidgetColorRole::AccentText => self.accent_text,
            WidgetColorRole::Secondary => self.secondary,
            WidgetColorRole::Info => self.info,
            WidgetColorRole::Success => self.success,
            WidgetColorRole::Warning => self.warning,
            WidgetColorRole::Danger => self.danger,
        }
    }
}

impl Default for HdrColorRoles {
    fn default() -> Self {
        Self::from_colors(ThemeColors::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrLuminanceTokens {
    pub reference_white: f32,
    pub focused: f32,
    pub semantic_accent: f32,
    pub emissive_indicator: f32,
    pub alert_pulse: f32,
}

impl HdrLuminanceTokens {
    pub const fn constrained_defaults() -> Self {
        let reference_white = 1.0;
        Self {
            reference_white,
            focused: 1.05,
            semantic_accent: 1.1,
            emissive_indicator: 1.25,
            alert_pulse: 1.15,
        }
    }
}

impl Default for HdrLuminanceTokens {
    fn default() -> Self {
        Self::constrained_defaults()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialToken {
    pub opacity: f32,
    pub blur_radius: f32,
    pub specular_strength: f32,
    pub rim_light_strength: f32,
}

impl MaterialToken {
    pub const fn new(
        opacity: f32,
        blur_radius: f32,
        specular_strength: f32,
        rim_light_strength: f32,
    ) -> Self {
        Self {
            opacity,
            blur_radius,
            specular_strength,
            rim_light_strength,
        }
    }
}

impl Default for MaterialToken {
    fn default() -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrMaterialTokens {
    pub flat: MaterialToken,
    pub raised: MaterialToken,
    pub glass: MaterialToken,
    pub glossy: MaterialToken,
    pub stylized: MaterialToken,
}

impl Default for HdrMaterialTokens {
    fn default() -> Self {
        Self {
            flat: MaterialToken::new(1.0, 0.0, 0.0, 0.0),
            raised: MaterialToken::new(0.98, 0.0, 0.08, 0.04),
            glass: MaterialToken::new(0.84, 6.0, 0.14, 0.10),
            glossy: MaterialToken::new(0.92, 0.0, 0.24, 0.18),
            stylized: MaterialToken::new(0.88, 4.0, 0.28, 0.22),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectToken {
    pub intensity: f32,
    pub speed: f32,
    pub color: Option<Color>,
}

impl EffectToken {
    pub const fn new(intensity: f32, speed: f32, color: Option<Color>) -> Self {
        Self {
            intensity,
            speed,
            color,
        }
    }
}

impl Default for EffectToken {
    fn default() -> Self {
        Self::new(0.0, 0.0, None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrEffectTokens {
    pub focus_ring: EffectToken,
    pub glow: EffectToken,
    pub pulse: EffectToken,
}

impl Default for HdrEffectTokens {
    fn default() -> Self {
        Self {
            focus_ring: EffectToken::new(0.35, 0.0, None),
            glow: EffectToken::new(0.24, 0.0, None),
            pulse: EffectToken::new(0.4, 1.0, None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrPolicyTokens {
    pub max_large_area_lift: f32,
    pub max_constrained_lift: f32,
    pub max_emissive_lift: f32,
}

impl Default for HdrPolicyTokens {
    fn default() -> Self {
        Self {
            max_large_area_lift: 1.2,
            max_constrained_lift: 1.35,
            max_emissive_lift: 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrThemeTokens {
    pub mode: HdrThemeMode,
    pub color_roles: HdrColorRoles,
    pub luminance: HdrLuminanceTokens,
    pub materials: HdrMaterialTokens,
    pub effects: HdrEffectTokens,
    pub policy: HdrPolicyTokens,
}

impl HdrThemeTokens {
    pub fn from_colors(colors: ThemeColors) -> Self {
        Self {
            mode: HdrThemeMode::Disabled,
            color_roles: HdrColorRoles::from_colors(colors),
            luminance: HdrLuminanceTokens::default(),
            materials: HdrMaterialTokens::default(),
            effects: HdrEffectTokens::default(),
            policy: HdrPolicyTokens::default(),
        }
    }

    pub fn from_default_theme(theme: DefaultTheme) -> Self {
        Self::from_colors(theme.colors)
    }

    pub fn sync_semantic_defaults(&mut self, colors: ThemeColors) {
        self.color_roles.sync_sdr_from_colors(colors);
    }
}

impl Default for HdrThemeTokens {
    fn default() -> Self {
        Self::from_colors(ThemeColors::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLuminanceRole {
    Standard,
    Focused,
    SemanticAccent,
    EmissiveIndicator,
    AlertPulse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetMaterialRole {
    Flat,
    Raised,
    Glass,
    Glossy,
    Stylized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetEffectRole {
    FocusRing,
    Glow,
    Pulse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedMaterialStyle {
    pub opacity: f32,
    pub blur_radius: f32,
    pub specular_strength: f32,
    pub rim_light_strength: f32,
}

impl ResolvedMaterialStyle {
    fn from_token(token: MaterialToken) -> Self {
        Self {
            opacity: token.opacity,
            blur_radius: token.blur_radius,
            specular_strength: token.specular_strength,
            rim_light_strength: token.rim_light_strength,
        }
    }
}

impl Default for ResolvedMaterialStyle {
    fn default() -> Self {
        Self::from_token(MaterialToken::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedEffectStyle {
    pub intensity: f32,
    pub speed: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedHdrStyle {
    pub color: Color,
    pub peak_lift: f32,
    pub material: ResolvedMaterialStyle,
    pub effect: Option<ResolvedEffectStyle>,
}

pub fn resolve_semantic_color(token: SemanticColorToken, mode: HdrThemeMode) -> Color {
    token.resolve(mode)
}

pub fn resolve_luminance_role(tokens: &HdrThemeTokens, role: WidgetLuminanceRole) -> f32 {
    let requested = match role {
        WidgetLuminanceRole::Standard => tokens.luminance.reference_white,
        WidgetLuminanceRole::Focused => tokens.luminance.focused,
        WidgetLuminanceRole::SemanticAccent => tokens.luminance.semantic_accent,
        WidgetLuminanceRole::EmissiveIndicator => tokens.luminance.emissive_indicator,
        WidgetLuminanceRole::AlertPulse => tokens.luminance.alert_pulse,
    }
    .max(tokens.luminance.reference_white);

    match tokens.mode {
        HdrThemeMode::Disabled => tokens.luminance.reference_white,
        HdrThemeMode::WideGamutOnly => requested.min(tokens.luminance.reference_white),
        HdrThemeMode::ConstrainedHdr => match role {
            WidgetLuminanceRole::EmissiveIndicator | WidgetLuminanceRole::AlertPulse => {
                requested.min(tokens.policy.max_constrained_lift)
            }
            WidgetLuminanceRole::Standard
            | WidgetLuminanceRole::Focused
            | WidgetLuminanceRole::SemanticAccent => {
                requested.min(tokens.policy.max_large_area_lift)
            }
        },
        HdrThemeMode::FullHdr => match role {
            WidgetLuminanceRole::EmissiveIndicator | WidgetLuminanceRole::AlertPulse => {
                requested.min(tokens.policy.max_emissive_lift)
            }
            WidgetLuminanceRole::Standard
            | WidgetLuminanceRole::Focused
            | WidgetLuminanceRole::SemanticAccent => {
                requested.min(tokens.policy.max_large_area_lift)
            }
        },
    }
}

pub fn resolve_material_role(
    tokens: &HdrThemeTokens,
    role: WidgetMaterialRole,
) -> ResolvedMaterialStyle {
    let token = match role {
        WidgetMaterialRole::Flat => tokens.materials.flat,
        WidgetMaterialRole::Raised => tokens.materials.raised,
        WidgetMaterialRole::Glass => tokens.materials.glass,
        WidgetMaterialRole::Glossy => tokens.materials.glossy,
        WidgetMaterialRole::Stylized => tokens.materials.stylized,
    };

    if matches!(tokens.mode, HdrThemeMode::Disabled) {
        ResolvedMaterialStyle::from_token(tokens.materials.flat)
    } else {
        ResolvedMaterialStyle::from_token(token)
    }
}

pub fn resolve_effect_role(
    tokens: &HdrThemeTokens,
    role: WidgetEffectRole,
    fallback_color: Color,
) -> Option<ResolvedEffectStyle> {
    if matches!(tokens.mode, HdrThemeMode::Disabled) {
        return None;
    }

    let token = match role {
        WidgetEffectRole::FocusRing => tokens.effects.focus_ring,
        WidgetEffectRole::Glow => tokens.effects.glow,
        WidgetEffectRole::Pulse => tokens.effects.pulse,
    };

    (token.intensity > 0.0).then_some(ResolvedEffectStyle {
        intensity: token.intensity,
        speed: token.speed,
        color: token.color.unwrap_or(fallback_color),
    })
}

pub fn resolve_widget_hdr_style(
    tokens: &HdrThemeTokens,
    color_role: WidgetColorRole,
    luminance_role: WidgetLuminanceRole,
    material_role: WidgetMaterialRole,
    effect_role: Option<WidgetEffectRole>,
) -> ResolvedHdrStyle {
    let color = resolve_semantic_color(tokens.color_roles.for_widget_role(color_role), tokens.mode);

    ResolvedHdrStyle {
        color,
        peak_lift: resolve_luminance_role(tokens, luminance_role),
        material: resolve_material_role(tokens, material_role),
        effect: effect_role.and_then(|role| resolve_effect_role(tokens, role, color)),
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::{
        resolve_luminance_role, resolve_material_role, resolve_semantic_color,
        resolve_widget_hdr_style, HdrColorRoles, HdrEffectTokens, HdrLuminanceTokens,
        HdrMaterialTokens, HdrPolicyTokens, HdrThemeMode, HdrThemeTokens, MaterialToken,
        ResolvedHdrStyle, SemanticColorToken, WidgetColorRole, WidgetEffectRole,
        WidgetLuminanceRole, WidgetMaterialRole,
    };
    use crate::theme::DefaultTheme;
    use sui_core::Color;

    fn assert_copy_debug_eq<T: Copy + Debug + PartialEq>() {}

    #[test]
    fn hdr_theme_tokens_default_to_disabled_mode() {
        let tokens = HdrThemeTokens::default();

        assert_eq!(tokens.mode, HdrThemeMode::Disabled);
    }

    #[test]
    fn hdr_luminance_constrained_defaults_stay_at_or_above_reference_white() {
        let tokens = HdrLuminanceTokens::constrained_defaults();

        assert!(tokens.focused >= tokens.reference_white);
        assert!(tokens.semantic_accent >= tokens.reference_white);
        assert!(tokens.emissive_indicator >= tokens.reference_white);
        assert!(tokens.alert_pulse >= tokens.reference_white);
    }

    #[test]
    fn hdr_color_roles_derive_from_default_theme_semantics() {
        let theme = DefaultTheme::default();
        let roles = HdrColorRoles::from_default_theme(theme);

        assert_eq!(roles.surface.sdr, theme.colors.base_100);
        assert_eq!(roles.surface_elevated.sdr, theme.colors.base_200);
        assert_eq!(roles.text.sdr, theme.colors.base_content);
        assert_eq!(roles.accent.sdr, theme.colors.primary);
        assert_eq!(roles.accent_text.sdr, theme.colors.primary_content);
        assert_eq!(roles.success.sdr, theme.colors.success);
        assert_eq!(roles.warning.sdr, theme.colors.warning);
        assert_eq!(roles.danger.sdr, theme.colors.error);
    }

    #[test]
    fn hdr_tokens_are_copy_debug_and_partial_eq_where_practical() {
        assert_copy_debug_eq::<SemanticColorToken>();
        assert_copy_debug_eq::<HdrColorRoles>();
        assert_copy_debug_eq::<HdrLuminanceTokens>();
        assert_copy_debug_eq::<MaterialToken>();
        assert_copy_debug_eq::<HdrMaterialTokens>();
        assert_copy_debug_eq::<HdrEffectTokens>();
        assert_copy_debug_eq::<HdrPolicyTokens>();
        assert_copy_debug_eq::<HdrThemeTokens>();
        assert_copy_debug_eq::<ResolvedHdrStyle>();
    }

    #[test]
    fn disabled_mode_material_role_resolves_flat_style() {
        let mut tokens = HdrThemeTokens::default();
        tokens.mode = HdrThemeMode::Disabled;
        tokens.materials.flat = MaterialToken::new(1.0, 0.0, 0.0, 0.0);
        tokens.materials.raised = MaterialToken::new(0.98, 0.0, 0.08, 0.04);
        tokens.materials.glass = MaterialToken::new(0.42, 8.0, 0.35, 0.28);
        tokens.materials.glossy = MaterialToken::new(0.77, 2.0, 0.5, 0.4);
        tokens.materials.stylized = MaterialToken::new(0.61, 5.0, 0.65, 0.55);

        let expected = resolve_material_role(&tokens, WidgetMaterialRole::Flat);

        assert_eq!(expected.opacity, 1.0);
        assert_eq!(expected.blur_radius, 0.0);
        assert_eq!(expected.specular_strength, 0.0);
        assert_eq!(expected.rim_light_strength, 0.0);
        assert_eq!(
            resolve_material_role(&tokens, WidgetMaterialRole::Raised),
            expected
        );
        assert_eq!(
            resolve_material_role(&tokens, WidgetMaterialRole::Glass),
            expected
        );
        assert_eq!(
            resolve_material_role(&tokens, WidgetMaterialRole::Glossy),
            expected
        );
        assert_eq!(
            resolve_material_role(&tokens, WidgetMaterialRole::Stylized),
            expected
        );
    }

    #[test]
    fn disabled_mode_resolves_to_sdr_semantics() {
        let token = SemanticColorToken::from_sdr(Color::rgba(0.2, 0.3, 0.4, 1.0))
            .with_wide_gamut(Color::display_p3(0.8, 0.2, 0.1, 1.0))
            .with_hdr(Color::linear_display_p3(1.4, 0.3, 0.2, 1.0));
        let mut tokens = HdrThemeTokens::default();
        tokens.mode = HdrThemeMode::Disabled;
        tokens.color_roles.accent = token;
        tokens.luminance.semantic_accent = 1.8;

        let resolved = resolve_widget_hdr_style(
            &tokens,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::SemanticAccent,
            WidgetMaterialRole::Glass,
            Some(WidgetEffectRole::Glow),
        );

        assert_eq!(resolve_semantic_color(token, tokens.mode), token.sdr);
        assert_eq!(resolved.color, token.sdr);
        assert_eq!(resolved.peak_lift, tokens.luminance.reference_white);
        assert_eq!(
            resolved.material,
            resolve_material_role(&tokens, WidgetMaterialRole::Flat)
        );
        assert!(resolved.effect.is_none());
    }

    #[test]
    fn wide_gamut_only_prefers_wide_gamut_variants_but_clamps_luminance_to_reference_white() {
        let mut tokens = HdrThemeTokens::default();
        tokens.mode = HdrThemeMode::WideGamutOnly;
        tokens.color_roles.accent = SemanticColorToken::from_sdr(Color::rgba(0.2, 0.3, 0.4, 1.0))
            .with_wide_gamut(Color::display_p3(0.8, 0.2, 0.1, 1.0))
            .with_hdr(Color::linear_display_p3(1.4, 0.3, 0.2, 1.0));
        tokens.luminance.semantic_accent = 1.8;

        let resolved = resolve_widget_hdr_style(
            &tokens,
            WidgetColorRole::Accent,
            WidgetLuminanceRole::SemanticAccent,
            WidgetMaterialRole::Raised,
            None,
        );

        assert_eq!(resolved.color, Color::display_p3(0.8, 0.2, 0.1, 1.0));
        assert_eq!(resolved.peak_lift, tokens.luminance.reference_white);
    }

    #[test]
    fn constrained_hdr_caps_emissive_roles_below_full_hdr() {
        let mut constrained = HdrThemeTokens::default();
        constrained.mode = HdrThemeMode::ConstrainedHdr;
        constrained.luminance.emissive_indicator = 2.5;
        constrained.policy.max_constrained_lift = 1.3;
        constrained.policy.max_emissive_lift = 2.2;

        let mut full = constrained;
        full.mode = HdrThemeMode::FullHdr;

        let constrained_peak =
            resolve_luminance_role(&constrained, WidgetLuminanceRole::EmissiveIndicator);
        let full_peak = resolve_luminance_role(&full, WidgetLuminanceRole::EmissiveIndicator);

        assert_eq!(constrained_peak, 1.3);
        assert_eq!(full_peak, 2.2);
        assert!(constrained_peak < full_peak);
    }

    #[test]
    fn full_hdr_respects_max_large_area_lift_policy() {
        let mut tokens = HdrThemeTokens::default();
        tokens.mode = HdrThemeMode::FullHdr;
        tokens.luminance.semantic_accent = 1.8;
        tokens.policy.max_large_area_lift = 1.25;

        assert_eq!(
            resolve_luminance_role(&tokens, WidgetLuminanceRole::SemanticAccent),
            1.25
        );
    }
}
