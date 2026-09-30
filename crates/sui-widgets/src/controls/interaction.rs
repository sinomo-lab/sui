use super::{set_hover_animation_target, set_press_animation_target};
use crate::{DefaultTheme, Progress};
use sui_core::{Event, KeyState, PointerButton, PointerEventKind};
use sui_runtime::EventCtx;
use sui_runtime::FrameClock;

/// Pins the interaction visuals of a control.
///
/// Widget galleries, documentation, and screenshot tests use a preview to show
/// hover, press, and focus chrome side by side without synthesizing input.
/// The preview only affects paint: the control stays interactive, keeps its
/// real semantics state, and live interaction can still add to the pinned
/// visuals. Disabled controls ignore previews, like they ignore input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InteractionPreview {
    /// Show only live interaction. This is the default.
    #[default]
    None,
    /// Paint as if a pointer rests over the control.
    Hovered,
    /// Paint as if the control is held down; implies hover.
    Pressed,
    /// Paint the keyboard focus chrome.
    Focused,
}

impl InteractionPreview {
    /// Hover progress pinned by this preview.
    pub(crate) fn hover(self) -> f32 {
        matches!(self, Self::Hovered | Self::Pressed) as u8 as f32
    }

    /// Press progress pinned by this preview.
    pub(crate) fn press(self) -> f32 {
        (self == Self::Pressed) as u8 as f32
    }

    /// Focus progress pinned by this preview.
    pub(crate) fn focus(self) -> f32 {
        (self == Self::Focused) as u8 as f32
    }

    /// Whether the preview shows the control as focused.
    pub(crate) fn is_focused(self) -> bool {
        self == Self::Focused
    }
}

/// Pointer/keyboard press state shared by ordinary and icon buttons.
pub(super) struct PressInteraction {
    pub(super) hovered: bool,
    pub(super) pressed: bool,
    pub(super) hover_animation: Progress,
    pub(super) press_animation: Progress,
    pub(super) preview: InteractionPreview,
    /// Whether a pointer press focuses the control, or leaves focus where it
    /// is.
    pub(super) focus_on_press: bool,
}

impl Default for PressInteraction {
    fn default() -> Self {
        Self {
            hovered: false,
            pressed: false,
            hover_animation: Progress::new(0.0),
            press_animation: Progress::new(0.0),
            preview: InteractionPreview::None,
            focus_on_press: true,
        }
    }
}

impl PressInteraction {
    /// Hover progress at `clock`'s frame, including any pinned preview.
    pub(super) fn hover_progress(&self, clock: &impl FrameClock) -> f32 {
        self.hover_animation.get(clock).max(self.preview.hover())
    }

    /// Press progress at `clock`'s frame, including any pinned preview.
    pub(super) fn press_progress(&self, clock: &impl FrameClock) -> f32 {
        self.press_animation.get(clock).max(self.preview.press())
    }

    pub(super) fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        enabled: bool,
        theme: impl Fn() -> DefaultTheme,
    ) -> bool {
        if !enabled {
            if self.hovered || self.pressed {
                let theme = theme();
                self.hovered = false;
                self.pressed = false;
                set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.request_paint();
                ctx.request_semantics();
            }
            return false;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx, &theme);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Enter) => {
                self.set_hovered(true, ctx, &theme);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(false, ctx, &theme);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let theme = theme();
                self.pressed = true;
                self.hovered = true;
                set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
                set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
                ctx.request_pointer_capture(pointer.pointer_id);
                if self.focus_on_press {
                    ctx.request_focus();
                } else {
                    ctx.keep_focus();
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && (pointer.button == Some(PointerButton::Primary) || self.pressed) =>
            {
                let theme = theme();
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                set_hover_animation_target(
                    &mut self.hover_animation,
                    hovered as u8 as f32,
                    &theme,
                    ctx,
                );
                set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    return true;
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    let theme = theme();
                    self.pressed = false;
                    self.hovered = false;
                    set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx);
                    set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                return true;
            }
            _ => {}
        }

        false
    }

    fn set_hovered(
        &mut self,
        hovered: bool,
        ctx: &mut EventCtx,
        theme: &impl Fn() -> DefaultTheme,
    ) {
        if self.hovered != hovered {
            let theme = theme();
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}
