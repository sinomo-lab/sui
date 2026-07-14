use sui_core::ClipboardBackend;

/// OS-clipboard bridge backed by `arboard`, installed into the runtime by the
/// desktop platform so widget copy/paste interoperates with other
/// applications.
///
/// The native connection is created lazily on first use. When it cannot be
/// established (for example headless CI sessions without a display server)
/// the backend degrades to process-local storage so in-app copy/paste keeps
/// working.
pub struct OsClipboardBackend {
    connection: Option<arboard::Clipboard>,
    connection_failed: bool,
    fallback: Option<String>,
}

impl OsClipboardBackend {
    pub fn new() -> Self {
        Self {
            connection: None,
            connection_failed: false,
            fallback: None,
        }
    }

    fn connection(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.connection.is_none() && !self.connection_failed {
            match arboard::Clipboard::new() {
                Ok(connection) => self.connection = Some(connection),
                Err(_) => self.connection_failed = true,
            }
        }
        self.connection.as_mut()
    }
}

impl Default for OsClipboardBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardBackend for OsClipboardBackend {
    fn text(&mut self) -> Option<String> {
        let Some(connection) = self.connection() else {
            return self.fallback.clone();
        };
        connection.get_text().ok()
    }

    fn set_text(&mut self, text: &str) {
        if let Some(connection) = self.connection()
            && connection.set_text(text.to_string()).is_ok()
        {
            return;
        }
        self.fallback = Some(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips text through the real OS clipboard (and clobbers whatever
    /// is on it), so it stays out of the default test run.
    #[test]
    #[ignore = "touches the real OS clipboard; run explicitly with --ignored"]
    fn os_clipboard_round_trips_text() {
        let mut backend = OsClipboardBackend::new();
        backend.set_text("sui os clipboard smoke");
        assert_eq!(
            backend.text().as_deref(),
            Some("sui os clipboard smoke"),
            "OS clipboard (or the in-process fallback) should round-trip text"
        );
    }
}
