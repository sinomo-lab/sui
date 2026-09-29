//! What the editor is editing: a preset plus source color edits, shape and
//! type scales, and role overrides. [`ThemeRecipe::build`] turns it into a
//! theme; the Rust export spells out the same steps.

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Signal, SimpleColorPickerMode};

use super::tokens::{RoleToken, SourceToken, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Preset {
    SuiLight,
    NeutralLight,
    SuiDark,
    NeutralDark,
    SuiTrueBlack,
}

impl Preset {
    pub(super) const ALL: [Self; 5] = [
        Self::SuiLight,
        Self::NeutralLight,
        Self::SuiDark,
        Self::NeutralDark,
        Self::SuiTrueBlack,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::SuiLight => "SUI light",
            Self::NeutralLight => "Neutral light",
            Self::SuiDark => "SUI dark",
            Self::NeutralDark => "Neutral dark",
            Self::SuiTrueBlack => "SUI true black",
        }
    }

    pub(super) fn theme(self) -> DefaultTheme {
        match self {
            Self::SuiLight => DefaultTheme::sui(),
            Self::NeutralLight => DefaultTheme::neutral(),
            Self::SuiDark => DefaultTheme::dark(),
            Self::NeutralDark => DefaultTheme::neutral_dark(),
            Self::SuiTrueBlack => DefaultTheme::high_contrast(),
        }
    }

    pub(super) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0)
    }

    pub(super) fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::SuiLight)
    }
}

pub(super) const SPACING_RANGE: (f64, f64) = (2.0, 12.0);
pub(super) const RADIUS_RANGE: (f64, f64) = (0.0, 2.0);
pub(super) const TEXT_RANGE: (f64, f64) = (0.75, 1.5);
pub(super) const MOTION_RANGE: (f64, f64) = (0.0, 2.0);

/// A preset plus every edit made to it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ThemeRecipe {
    pub(super) preset: Preset,
    pub(super) colors: ThemeColors,
    pub(super) control_size: ControlSize,
    pub(super) spacing: f32,
    pub(super) radius_scale: f32,
    pub(super) text_scale: f32,
    pub(super) motion_scale: f32,
    /// Role overrides in the order they were added.
    pub(super) overrides: Vec<(RoleToken, Color)>,
}

impl ThemeRecipe {
    pub(super) fn from_preset(preset: Preset) -> Self {
        let theme = preset.theme();
        Self {
            preset,
            colors: theme.colors,
            control_size: ControlSize::Medium,
            spacing: theme.spacing,
            radius_scale: 1.0,
            text_scale: 1.0,
            motion_scale: 1.0,
            overrides: Vec::new(),
        }
    }

    pub(super) fn build(&self) -> DefaultTheme {
        let mut theme = DefaultTheme::from_colors(self.colors);
        if self.shape_changed() {
            theme.spacing = self.spacing;
            theme.radius = scaled_radii(self.radius_scale);
            theme.text = scaled_text_scale(self.text_scale);
            theme.motion = scaled_motion(self.motion_scale);
            theme.sync_derived_fields();
        }
        let mut theme = theme.with_size(self.control_size);
        for (role, color) in &self.overrides {
            role.set(&mut theme.palette, *color);
        }
        theme
    }

    /// Whether spacing, radius, type, or motion differ from the defaults.
    pub(super) fn shape_changed(&self) -> bool {
        let defaults = Self::from_preset(self.preset);
        self.spacing != defaults.spacing
            || self.radius_scale != 1.0
            || self.text_scale != 1.0
            || self.motion_scale != 1.0
    }

    pub(super) fn changed_sources(&self) -> impl Iterator<Item = SourceToken> + '_ {
        let preset = self.preset.theme().colors;
        SourceToken::ALL
            .iter()
            .copied()
            .filter(move |token| token.get(&self.colors) != token.get(&preset))
    }

    pub(super) fn override_for(&self, role: RoleToken) -> Option<Color> {
        self.overrides
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .map(|(_, color)| *color)
    }
}

pub(super) fn scaled_radii(scale: f32) -> ThemeRadii {
    let base = ThemeRadii::default();
    ThemeRadii {
        xs: base.xs * scale,
        sm: base.sm * scale,
        md: base.md * scale,
        lg: base.lg * scale,
        xl: base.xl * scale,
        _2xl: base._2xl * scale,
        _3xl: base._3xl * scale,
        _4xl: base._4xl,
    }
}

fn scaled_text_token(token: ThemeTextToken, scale: f32) -> ThemeTextToken {
    ThemeTextToken {
        size: token.size * scale,
        line_height: token.line_height * scale,
    }
}

pub(super) fn scaled_text_scale(scale: f32) -> ThemeTextScale {
    let base = ThemeTextScale::default();
    ThemeTextScale {
        xs: scaled_text_token(base.xs, scale),
        sm: scaled_text_token(base.sm, scale),
        base: scaled_text_token(base.base, scale),
        lg: scaled_text_token(base.lg, scale),
        xl: scaled_text_token(base.xl, scale),
        _2xl: scaled_text_token(base._2xl, scale),
        _3xl: scaled_text_token(base._3xl, scale),
        _4xl: scaled_text_token(base._4xl, scale),
        _5xl: scaled_text_token(base._5xl, scale),
        _6xl: scaled_text_token(base._6xl, scale),
        _7xl: scaled_text_token(base._7xl, scale),
        _8xl: scaled_text_token(base._8xl, scale),
        _9xl: scaled_text_token(base._9xl, scale),
    }
}

pub(super) fn scaled_motion(scale: f32) -> ThemeMotion {
    let mut motion = ThemeMotion::standard();
    motion.duration_fast *= scale;
    motion.duration_normal *= scale;
    motion.duration_slow *= scale;
    motion.duration_slower *= scale;
    motion
}

/// The channel sliders the color editor shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PickerMode {
    Oklch,
    Rgb,
    Hsl,
}

impl PickerMode {
    pub(super) const ALL: [Self; 3] = [Self::Oklch, Self::Rgb, Self::Hsl];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Oklch => "OKLCH",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
        }
    }

    pub(super) const fn picker_mode(self) -> SimpleColorPickerMode {
        match self {
            Self::Oklch => SimpleColorPickerMode::Oklch,
            Self::Rgb => SimpleColorPickerMode::Rgb,
            Self::Hsl => SimpleColorPickerMode::Hsl,
        }
    }

    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0)
    }
}

/// Rebuild key for the open color picker: its mode, and a revision bumped
/// when the color changes from somewhere other than the picker itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PickerKey {
    pub(super) mode: PickerMode,
    pub(super) revision: u64,
}

/// Which control made a color edit, so the other one can resync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditOrigin {
    Picker,
    Hex,
}

/// Shared editor state. Widgets observe the signals; the recipe is the
/// source of truth and `theme` always holds its built theme.
#[derive(Clone)]
pub(super) struct ThemeEditorState {
    recipe: Rc<RefCell<ThemeRecipe>>,
    status: Rc<RefCell<Option<String>>>,
    theme: Signal<DefaultTheme>,
    selected: Signal<Option<Token>>,
    picker: Signal<PickerKey>,
    hex_revision: Signal<u64>,
    structure: Signal<u64>,
    summary: Signal<String>,
}

impl ThemeEditorState {
    pub(super) fn new() -> Self {
        let recipe = ThemeRecipe::from_preset(Preset::SuiLight);
        let theme = recipe.build();
        let state = Self {
            recipe: Rc::new(RefCell::new(recipe)),
            status: Rc::new(RefCell::new(None)),
            theme: Signal::new(theme),
            selected: Signal::new(None),
            picker: Signal::new(PickerKey {
                mode: PickerMode::Oklch,
                revision: 0,
            }),
            hex_revision: Signal::new(0),
            structure: Signal::new(0),
            summary: Signal::new(String::new()),
        };
        state.refresh_summary();
        state
    }

    pub(super) fn theme(&self) -> DefaultTheme {
        self.theme.get()
    }

    pub(super) fn theme_signal(&self) -> Signal<DefaultTheme> {
        self.theme.clone()
    }

    pub(super) fn selected_signal(&self) -> Signal<Option<Token>> {
        self.selected.clone()
    }

    pub(super) fn picker_signal(&self) -> Signal<PickerKey> {
        self.picker.clone()
    }

    pub(super) fn hex_signal(&self) -> Signal<u64> {
        self.hex_revision.clone()
    }

    pub(super) fn structure_signal(&self) -> Signal<u64> {
        self.structure.clone()
    }

    pub(super) fn summary_signal(&self) -> Signal<String> {
        self.summary.clone()
    }

    pub(super) fn recipe(&self) -> ThemeRecipe {
        self.recipe.borrow().clone()
    }

    pub(super) fn preset(&self) -> Preset {
        self.recipe.borrow().preset
    }

    /// Opens `token`'s editor, or closes it when it is already open.
    pub(super) fn toggle(&self, token: Token) {
        let next = (self.selected.get() != Some(token)).then_some(token);
        self.selected.set(next);
    }

    pub(super) fn color(&self, token: Token) -> Color {
        match token {
            Token::Source(source) => source.get(&self.recipe.borrow().colors),
            Token::Role(role) => role.get(&self.theme.get().palette),
        }
    }

    pub(super) fn is_modified(&self, token: Token) -> bool {
        let recipe = self.recipe.borrow();
        match token {
            Token::Source(source) => {
                source.get(&recipe.colors) != source.get(&recipe.preset.theme().colors)
            }
            Token::Role(role) => recipe.override_for(role).is_some(),
        }
    }

    pub(super) fn set_color(&self, token: Token, color: Color, origin: EditOrigin) {
        let color = color.clamped();
        {
            let mut recipe = self.recipe.borrow_mut();
            match token {
                Token::Source(source) => source.set(&mut recipe.colors, color),
                Token::Role(role) => {
                    match recipe
                        .overrides
                        .iter_mut()
                        .find(|(candidate, _)| *candidate == role)
                    {
                        Some((_, existing)) => *existing = color,
                        None => recipe.overrides.push((role, color)),
                    }
                }
            }
        }
        match origin {
            EditOrigin::Picker => self.bump_hex(),
            EditOrigin::Hex => self.bump_picker(),
        }
        self.commit();
    }

    /// Returns a source color to its preset value, or removes a role
    /// override.
    pub(super) fn reset(&self, token: Token) {
        {
            let mut recipe = self.recipe.borrow_mut();
            match token {
                Token::Source(source) => {
                    let preset = source.get(&recipe.preset.theme().colors);
                    source.set(&mut recipe.colors, preset);
                }
                Token::Role(role) => recipe.overrides.retain(|(candidate, _)| *candidate != role),
            }
        }
        if matches!(token, Token::Role(_)) {
            self.selected.set(None);
            self.bump_structure();
        }
        self.bump_hex();
        self.bump_picker();
        self.commit();
    }

    /// Starts over from `preset`, dropping every edit.
    pub(super) fn set_preset(&self, preset: Preset) {
        *self.recipe.borrow_mut() = ThemeRecipe::from_preset(preset);
        self.selected.set(None);
        self.bump_structure();
        self.bump_hex();
        self.bump_picker();
        self.commit();
    }

    pub(super) fn reset_all(&self) {
        self.set_preset(self.preset());
    }

    pub(super) fn overrides(&self) -> Vec<RoleToken> {
        self.recipe
            .borrow()
            .overrides
            .iter()
            .map(|(role, _)| *role)
            .collect()
    }

    pub(super) fn available_roles(&self) -> Vec<RoleToken> {
        let overridden = self.overrides();
        RoleToken::ALL
            .iter()
            .copied()
            .filter(|role| !overridden.contains(role))
            .collect()
    }

    /// Overrides `role` with its current derived color and opens it.
    pub(super) fn add_override(&self, role: RoleToken) {
        let color = role.get(&self.theme.get().palette);
        self.recipe.borrow_mut().overrides.push((role, color));
        self.selected.set(Some(Token::Role(role)));
        self.bump_structure();
        self.commit();
    }

    pub(super) fn set_picker_mode(&self, mode: PickerMode) {
        let key = self.picker.get();
        self.picker.set(PickerKey { mode, ..key });
    }

    pub(super) fn control_size(&self) -> ControlSize {
        self.recipe.borrow().control_size
    }

    pub(super) fn set_control_size(&self, size: ControlSize) {
        self.recipe.borrow_mut().control_size = size;
        self.commit();
    }

    pub(super) fn spacing(&self) -> f32 {
        self.recipe.borrow().spacing
    }

    pub(super) fn set_spacing(&self, spacing: f32) {
        self.recipe.borrow_mut().spacing =
            spacing.clamp(SPACING_RANGE.0 as f32, SPACING_RANGE.1 as f32);
        self.commit();
    }

    pub(super) fn radius_scale(&self) -> f32 {
        self.recipe.borrow().radius_scale
    }

    pub(super) fn set_radius_scale(&self, scale: f32) {
        self.recipe.borrow_mut().radius_scale =
            scale.clamp(RADIUS_RANGE.0 as f32, RADIUS_RANGE.1 as f32);
        self.commit();
    }

    pub(super) fn text_scale(&self) -> f32 {
        self.recipe.borrow().text_scale
    }

    pub(super) fn set_text_scale(&self, scale: f32) {
        self.recipe.borrow_mut().text_scale = scale.clamp(TEXT_RANGE.0 as f32, TEXT_RANGE.1 as f32);
        self.commit();
    }

    pub(super) fn motion_scale(&self) -> f32 {
        self.recipe.borrow().motion_scale
    }

    pub(super) fn set_motion_scale(&self, scale: f32) {
        self.recipe.borrow_mut().motion_scale =
            scale.clamp(MOTION_RANGE.0 as f32, MOTION_RANGE.1 as f32);
        self.commit();
    }

    /// Shows `message` in the summary line until the next edit.
    pub(super) fn set_status(&self, message: impl Into<String>) {
        *self.status.borrow_mut() = Some(message.into());
        self.refresh_summary();
    }

    /// One line describing how far the theme is from its preset.
    pub(super) fn edit_summary(&self) -> String {
        let recipe = self.recipe.borrow();
        let colors = recipe.changed_sources().count();
        let roles = recipe.overrides.len();
        let mut parts = Vec::new();
        if colors > 0 {
            parts.push(format!(
                "{colors} {} changed",
                if colors == 1 { "color" } else { "colors" }
            ));
        }
        if roles > 0 {
            parts.push(format!(
                "{roles} role {}",
                if roles == 1 { "override" } else { "overrides" }
            ));
        }
        if recipe.control_size != ControlSize::Medium || recipe.shape_changed() {
            parts.push("shape and type changed".to_string());
        }
        if parts.is_empty() {
            format!("{} with no changes", recipe.preset.label())
        } else {
            format!("{}: {}", recipe.preset.label(), parts.join(", "))
        }
    }

    fn commit(&self) {
        let theme = self.recipe.borrow().build();
        self.theme.set(theme);
        self.status.borrow_mut().take();
        self.refresh_summary();
    }

    fn refresh_summary(&self) {
        let summary = match self.status.borrow().as_ref() {
            Some(status) => format!("{} · {status}", self.edit_summary()),
            None => self.edit_summary(),
        };
        self.summary.set(summary);
    }

    fn bump_structure(&self) {
        self.structure.update(|revision| *revision += 1);
    }

    fn bump_hex(&self) {
        self.hex_revision.update(|revision| *revision += 1);
    }

    fn bump_picker(&self) {
        self.picker.update(|key| key.revision += 1);
    }
}
