use sui_core::{CustomEvent, Event};

/// Custom-event kind carrying a [`TextCommand`] (see
/// [`TextCommand::into_event`]).
pub const TEXT_COMMAND_EVENT_KIND: &str = "sui.text-command";

/// Standard clipboard/selection command understood by the text editing
/// widgets (`TextInput`, `TextArea`, `TextSurface`).
///
/// Applications deliver these to a widget as a custom event — typically via
/// `EventCtx::post_event` from a context-menu activation — so menus and other
/// chrome can drive copy/paste without reaching into widget internals:
///
/// ```ignore
/// ctx.post_event(editor_id, TextCommand::Paste.into_event());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCommand {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl TextCommand {
    const CUT: &'static str = "cut";
    const COPY: &'static str = "copy";
    const PASTE: &'static str = "paste";
    const SELECT_ALL: &'static str = "select-all";

    pub fn into_custom_event(self) -> CustomEvent {
        let payload = match self {
            Self::Cut => Self::CUT,
            Self::Copy => Self::COPY,
            Self::Paste => Self::PASTE,
            Self::SelectAll => Self::SELECT_ALL,
        };
        CustomEvent {
            kind: TEXT_COMMAND_EVENT_KIND.to_string(),
            payload: Some(payload.to_string()),
        }
    }

    pub fn into_event(self) -> Event {
        Event::Custom(self.into_custom_event())
    }

    pub fn from_custom_event(event: &CustomEvent) -> Option<Self> {
        if event.kind != TEXT_COMMAND_EVENT_KIND {
            return None;
        }
        match event.payload.as_deref() {
            Some(Self::CUT) => Some(Self::Cut),
            Some(Self::COPY) => Some(Self::Copy),
            Some(Self::PASTE) => Some(Self::Paste),
            Some(Self::SELECT_ALL) => Some(Self::SelectAll),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_round_trip_through_custom_events() {
        for command in [
            TextCommand::Cut,
            TextCommand::Copy,
            TextCommand::Paste,
            TextCommand::SelectAll,
        ] {
            let event = command.into_custom_event();
            assert_eq!(TextCommand::from_custom_event(&event), Some(command));
        }
    }

    #[test]
    fn unrelated_custom_events_are_ignored() {
        let event = CustomEvent::new("sui.external.wake");
        assert_eq!(TextCommand::from_custom_event(&event), None);
    }
}
