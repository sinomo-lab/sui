use sui_runtime::{Command, CommandKey};

/// Standard clipboard/selection command understood by text editing widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCommand {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// Typed command key used for target-only text command delivery.
pub const TEXT_COMMAND: CommandKey<TextCommand> = CommandKey::new("sui.text-command");

impl TextCommand {
    pub fn from_command(command: &Command<'_>) -> Option<Self> {
        command.get(TEXT_COMMAND).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_command_key_has_a_stable_diagnostic_name() {
        assert_eq!(TEXT_COMMAND.name(), "sui.text-command");
    }
}
