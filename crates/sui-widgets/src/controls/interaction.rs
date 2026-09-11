use super::{set_hover_animation_target, set_press_animation_target};
use crate::{DefaultTheme, MotionScalar};
use sui_core::{Event, KeyState, PointerButton, PointerEventKind};
use sui_runtime::EventCtx;

/// Pointer/keyboard press state shared by ordinary and icon buttons.
pub(super) struct PressInteraction {
    pub(super) hovered: bool,
    pub(super) pressed: bool,
    pub(super) hover_animation: MotionScalar,
    pub(super) press_animation: MotionScalar,
}

impl Default for PressInteraction {
    fn default() -> Self {
        Self {
            hovered: false,
            pressed: false,
            hover_animation: MotionScalar::new(0.0),
            press_animation: MotionScalar::new(0.0),
        }
    }
}

impl PressInteraction {
    pub(super) fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        enabled: bool,
        theme: impl Fn() -> DefaultTheme,
        focus_animation: &mut MotionScalar,
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
                ctx.request_focus();
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
            Event::Wake(sui_core::WakeEvent::AnimationFrame { time, .. }) => {
                if self.hover_animation.advance(*time)
                    | self.press_animation.advance(*time)
                    | focus_animation.advance(*time)
                {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
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
