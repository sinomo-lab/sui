use std::{cell::RefCell, fmt, rc::Rc};

/// Storage driver behind a [`Clipboard`] handle.
///
/// The default [`LocalClipboardBackend`] keeps text in process memory so
/// copy/paste works out of the box (and deterministically in tests).
/// Platforms install an OS-backed implementation at startup to bridge the
/// system clipboard.
pub trait ClipboardBackend {
    fn text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
}

/// In-process clipboard storage used when no platform backend is installed.
#[derive(Debug, Default)]
pub struct LocalClipboardBackend {
    text: Option<String>,
}

impl LocalClipboardBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClipboardBackend for LocalClipboardBackend {
    fn text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.text = Some(text.to_string());
    }
}

/// Shared handle to the clipboard service.
///
/// Cloning is cheap and every clone refers to the same storage, so a backend
/// installed through one handle (see [`Clipboard::set_backend`]) is visible to
/// all of them.
#[derive(Clone)]
pub struct Clipboard {
    inner: Rc<RefCell<Box<dyn ClipboardBackend>>>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::with_backend(LocalClipboardBackend::new())
    }

    pub fn with_backend(backend: impl ClipboardBackend + 'static) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Box::new(backend))),
        }
    }

    /// Replace the storage driver while keeping every existing handle valid.
    pub fn set_backend(&self, backend: impl ClipboardBackend + 'static) {
        *self.inner.borrow_mut() = Box::new(backend);
    }

    pub fn text(&self) -> Option<String> {
        self.inner.borrow_mut().text()
    }

    pub fn set_text(&self, text: impl AsRef<str>) {
        self.inner.borrow_mut().set_text(text.as_ref());
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Clipboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Clipboard").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_backend_round_trips_text() {
        let clipboard = Clipboard::new();
        assert_eq!(clipboard.text(), None);

        clipboard.set_text("hello");
        assert_eq!(clipboard.text().as_deref(), Some("hello"));
    }

    #[test]
    fn clones_share_storage_and_backend_swaps_apply_to_all_handles() {
        let clipboard = Clipboard::new();
        let clone = clipboard.clone();
        clipboard.set_text("shared");
        assert_eq!(clone.text().as_deref(), Some("shared"));

        clone.set_backend(LocalClipboardBackend::new());
        assert_eq!(clipboard.text(), None);
    }
}
