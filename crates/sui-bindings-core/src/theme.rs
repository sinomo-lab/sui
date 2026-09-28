use crate::application::normalized_option_name;
use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use std::sync::Arc;
use std::sync::Mutex;
use sui::Color;
use sui::ControlSize;
use sui::DefaultTheme;
use sui::ThemeColors;

/// Live, thread-safe handle to SUI's built-in theme tokens.
///
/// Binding widgets capture this handle rather than a theme snapshot, so preset,
/// accent, and control-size changes propagate without rebuilding the foreign tree.
#[derive(Debug, Clone)]
pub struct BindingTheme {
    pub(crate) inner: Arc<BindingThemeInner>,
}

#[derive(Debug)]
pub(crate) struct BindingThemeInner {
    pub(crate) value: Mutex<DefaultTheme>,
    pub(crate) ui_handle: Mutex<Option<BindingUiHandle>>,
}

impl BindingTheme {
    pub fn preset(name: &str) -> Result<Self, String> {
        Ok(Self {
            inner: Arc::new(BindingThemeInner {
                value: Mutex::new(binding_theme_preset(name)?),
                ui_handle: Mutex::new(None),
            }),
        })
    }

    pub fn snapshot(&self) -> DefaultTheme {
        *recover_lock(&self.inner.value)
    }

    pub fn set_preset(&self, name: &str) -> Result<(), String> {
        self.publish(binding_theme_preset(name)?);
        Ok(())
    }

    pub fn set_accent(&self, color: Color) {
        let mut theme = self.snapshot();
        theme.colors.primary = color;
        theme.sync_derived_fields();
        self.publish(theme);
    }

    pub fn accent(&self) -> Color {
        self.snapshot().palette.accent
    }

    pub fn set_control_size(&self, size: &str) -> Result<(), String> {
        let size = match normalized_option_name(size).as_str() {
            "small" | "compact" => ControlSize::Small,
            "medium" | "standard" => ControlSize::Medium,
            "large" | "touch" => ControlSize::Large,
            _ => {
                return Err(format!(
                    "control size must be 'small', 'medium', or 'large', got '{size}'"
                ));
            }
        };
        self.publish(self.snapshot().with_size(size));
        Ok(())
    }

    pub fn color(&self, name: &str) -> Result<Color, String> {
        let mut colors = self.snapshot().colors;
        theme_color_slot(&mut colors, name).map(|slot| *slot)
    }

    pub fn set_color(&self, name: &str, color: Color) -> Result<(), String> {
        let mut theme = self.snapshot();
        *theme_color_slot(&mut theme.colors, name)? = color;
        theme.sync_derived_fields();
        self.publish(theme);
        Ok(())
    }

    pub fn number(&self, name: &str) -> Result<f32, String> {
        let theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "spacing" => Ok(theme.spacing),
            "radiusxs" => Ok(theme.radius.xs),
            "radiussm" => Ok(theme.radius.sm),
            "radiusmd" => Ok(theme.radius.md),
            "radiuslg" => Ok(theme.radius.lg),
            "radiusxl" => Ok(theme.radius.xl),
            "breakpointsm" | "breakpointmedium" => Ok(theme.breakpoints.sm),
            "breakpointlg" | "breakpointexpanded" => Ok(theme.breakpoints.lg),
            "motionfast" => Ok(theme.motion.duration_fast),
            "motionnormal" => Ok(theme.motion.duration_normal),
            "motionslow" => Ok(theme.motion.duration_slow),
            "motionslower" => Ok(theme.motion.duration_slower),
            _ => Err(format!("unknown theme number token '{name}'")),
        }
    }

    pub fn set_number(&self, name: &str, value: f32) -> Result<(), String> {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "theme number token '{name}' must be finite and non-negative"
            ));
        }
        let mut theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "spacing" => theme.spacing = value,
            "radiusxs" => theme.radius.xs = value,
            "radiussm" => theme.radius.sm = value,
            "radiusmd" => theme.radius.md = value,
            "radiuslg" => theme.radius.lg = value,
            "radiusxl" => theme.radius.xl = value,
            "breakpointsm" | "breakpointmedium" => theme.breakpoints.sm = value,
            "breakpointlg" | "breakpointexpanded" => theme.breakpoints.lg = value,
            "motionfast" => theme.motion.duration_fast = value,
            "motionnormal" => theme.motion.duration_normal = value,
            "motionslow" => theme.motion.duration_slow = value,
            "motionslower" => theme.motion.duration_slower = value,
            _ => return Err(format!("unknown theme number token '{name}'")),
        }
        if matches!(
            normalized_option_name(name).as_str(),
            "spacing" | "radiusxs" | "radiussm" | "radiusmd" | "radiuslg" | "radiusxl"
        ) {
            theme.sync_derived_fields();
        }
        self.publish(theme);
        Ok(())
    }

    pub fn bind_ui_handle(&self, handle: BindingUiHandle) {
        *recover_lock(&self.inner.ui_handle) = Some(handle);
    }

    pub(crate) fn publish(&self, value: DefaultTheme) {
        if let Some(handle) = recover_lock(&self.inner.ui_handle).clone()
            && !handle.is_draining()
        {
            let theme = self.clone();
            handle.post(move || theme.publish_immediate(value));
        } else {
            self.publish_immediate(value);
        }
    }

    pub(crate) fn publish_immediate(&self, value: DefaultTheme) {
        *recover_lock(&self.inner.value) = value;
    }
}

/// Resolve a source color token name (case- and separator-insensitive).
fn theme_color_slot<'a>(colors: &'a mut ThemeColors, name: &str) -> Result<&'a mut Color, String> {
    let neutrals = &mut colors.neutrals;
    let decorative = &mut colors.decorative;
    let slot = match normalized_option_name(name).as_str() {
        "window" | "background" => &mut neutrals.window,
        "subtle" => &mut neutrals.subtle,
        "panel" | "surface" => &mut neutrals.panel,
        "overlay" => &mut neutrals.overlay,
        "control" => &mut neutrals.control,
        "controlhover" => &mut neutrals.control_hover,
        "controlactive" => &mut neutrals.control_active,
        "button" => &mut neutrals.button,
        "buttonhover" => &mut neutrals.button_hover,
        "buttonactive" => &mut neutrals.button_active,
        "field" => &mut neutrals.field,
        "bordersubtle" => &mut neutrals.border_subtle,
        "border" => &mut neutrals.border,
        "borderstrong" => &mut neutrals.border_strong,
        "bordercontrol" => &mut neutrals.border_control,
        "text" | "foreground" => &mut neutrals.text,
        "textsecondary" => &mut neutrals.text_secondary,
        "texttertiary" => &mut neutrals.text_tertiary,
        "textdisabled" => &mut neutrals.text_disabled,
        "primary" | "accent" => &mut colors.primary,
        "onprimary" | "onaccent" => &mut colors.on_primary,
        "secondary" => &mut colors.secondary,
        "onsecondary" => &mut colors.on_secondary,
        "info" => &mut colors.info,
        "oninfo" => &mut colors.on_info,
        "success" => &mut colors.success,
        "onsuccess" => &mut colors.on_success,
        "warning" => &mut colors.warning,
        "onwarning" => &mut colors.on_warning,
        "danger" | "error" => &mut colors.danger,
        "ondanger" | "onerror" => &mut colors.on_danger,
        "red" => &mut decorative.red,
        "orange" => &mut decorative.orange,
        "amber" => &mut decorative.amber,
        "green" => &mut decorative.green,
        "teal" => &mut decorative.teal,
        "cyan" => &mut decorative.cyan,
        "blue" => &mut decorative.blue,
        "violet" => &mut decorative.violet,
        "magenta" => &mut decorative.magenta,
        _ => return Err(format!("unknown theme color token '{name}'")),
    };
    Ok(slot)
}

pub(crate) fn binding_theme_preset(name: &str) -> Result<DefaultTheme, String> {
    match normalized_option_name(name).as_str() {
        "sui" | "light" | "default" => Ok(DefaultTheme::light()),
        "dark" => Ok(DefaultTheme::dark()),
        "neutral" | "neutrallight" => Ok(DefaultTheme::neutral()),
        "neutraldark" => Ok(DefaultTheme::neutral_dark()),
        "highcontrast" => Ok(DefaultTheme::high_contrast()),
        "oled" | "void" => Ok(DefaultTheme::void()),
        _ => Err(format!(
            "unknown theme preset '{name}'; expected light, dark, neutral, neutral-dark, high-contrast, or oled"
        )),
    }
}
