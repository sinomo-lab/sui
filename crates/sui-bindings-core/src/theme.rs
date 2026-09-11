use crate::application::normalized_option_name;
use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use std::sync::Arc;
use std::sync::Mutex;
use sui::Color;
use sui::ControlSize;
use sui::DefaultTheme;

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
        theme.colors.accent = color;
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
        let colors = self.snapshot().colors;
        match normalized_option_name(name).as_str() {
            "base100" | "background" => Ok(colors.base_100),
            "base200" | "surface" => Ok(colors.base_200),
            "base300" | "border" => Ok(colors.base_300),
            "basecontent" | "foreground" | "text" => Ok(colors.base_content),
            "primary" => Ok(colors.primary),
            "primarycontent" => Ok(colors.primary_content),
            "secondary" => Ok(colors.secondary),
            "secondarycontent" => Ok(colors.secondary_content),
            "accent" => Ok(colors.accent),
            "accentcontent" => Ok(colors.accent_content),
            "neutral" => Ok(colors.neutral),
            "neutralcontent" => Ok(colors.neutral_content),
            "info" => Ok(colors.info),
            "infocontent" => Ok(colors.info_content),
            "success" => Ok(colors.success),
            "successcontent" => Ok(colors.success_content),
            "warning" => Ok(colors.warning),
            "warningcontent" => Ok(colors.warning_content),
            "error" | "danger" => Ok(colors.error),
            "errorcontent" | "dangercontent" => Ok(colors.error_content),
            _ => Err(format!("unknown theme color token '{name}'")),
        }
    }

    pub fn set_color(&self, name: &str, color: Color) -> Result<(), String> {
        let mut theme = self.snapshot();
        match normalized_option_name(name).as_str() {
            "base100" | "background" => theme.colors.base_100 = color,
            "base200" | "surface" => theme.colors.base_200 = color,
            "base300" | "border" => theme.colors.base_300 = color,
            "basecontent" | "foreground" | "text" => theme.colors.base_content = color,
            "primary" => theme.colors.primary = color,
            "primarycontent" => theme.colors.primary_content = color,
            "secondary" => theme.colors.secondary = color,
            "secondarycontent" => theme.colors.secondary_content = color,
            "accent" => theme.colors.accent = color,
            "accentcontent" => theme.colors.accent_content = color,
            "neutral" => theme.colors.neutral = color,
            "neutralcontent" => theme.colors.neutral_content = color,
            "info" => theme.colors.info = color,
            "infocontent" => theme.colors.info_content = color,
            "success" => theme.colors.success = color,
            "successcontent" => theme.colors.success_content = color,
            "warning" => theme.colors.warning = color,
            "warningcontent" => theme.colors.warning_content = color,
            "error" | "danger" => theme.colors.error = color,
            "errorcontent" | "dangercontent" => theme.colors.error_content = color,
            _ => return Err(format!("unknown theme color token '{name}'")),
        }
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
