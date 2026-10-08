//! Application services that host languages reach outside of widget
//! contexts: the clipboard and native file dialogs.

use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use std::path::PathBuf;
use std::sync::Mutex;
use sui::ClipboardBackend;

/// One clipboard for the whole process, so text copied by host-language code
/// and by widgets in any app agrees.
static CLIPBOARD: Mutex<Option<Box<dyn ClipboardBackend + Send>>> = Mutex::new(None);

fn with_clipboard<T>(use_clipboard: impl FnOnce(&mut dyn ClipboardBackend) -> T) -> T {
    let mut clipboard = recover_lock(&CLIPBOARD);
    let backend = clipboard.get_or_insert_with(new_clipboard_backend);
    use_clipboard(backend.as_mut())
}

#[cfg(all(
    feature = "desktop",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn new_clipboard_backend() -> Box<dyn ClipboardBackend + Send> {
    // Falls back to process memory when no display server is available.
    Box::new(sui::OsClipboardBackend::new())
}

#[cfg(not(all(
    feature = "desktop",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
)))]
fn new_clipboard_backend() -> Box<dyn ClipboardBackend + Send> {
    Box::new(sui::LocalClipboardBackend::new())
}

/// Text on the clipboard: the system clipboard on desktop hosts with a
/// display, otherwise process memory.
pub fn binding_clipboard_text() -> Option<String> {
    with_clipboard(|clipboard| clipboard.text())
}

pub fn binding_set_clipboard_text(text: &str) {
    with_clipboard(|clipboard| clipboard.set_text(text));
}

/// A runtime clipboard backend that forwards to the process clipboard.
/// Host-driven runtimes install it so widget copy and paste match
/// [`binding_clipboard_text`].
pub(crate) struct SharedClipboardBackend;

impl ClipboardBackend for SharedClipboardBackend {
    fn text(&mut self) -> Option<String> {
        binding_clipboard_text()
    }

    fn set_text(&mut self, text: &str) {
        binding_set_clipboard_text(text);
    }
}

/// What a native file dialog chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingFileDialogMode {
    OpenFile,
    OpenFiles,
    SaveFile,
    OpenFolder,
    OpenFolders,
}

pub fn binding_file_dialog_mode_from_name(value: &str) -> Option<BindingFileDialogMode> {
    match crate::values::normalize_binding_name(value).as_str() {
        "open" | "openfile" => Some(BindingFileDialogMode::OpenFile),
        "openmultiple" | "openfiles" => Some(BindingFileDialogMode::OpenFiles),
        "save" | "savefile" => Some(BindingFileDialogMode::SaveFile),
        "folder" | "openfolder" => Some(BindingFileDialogMode::OpenFolder),
        "folders" | "openfolders" => Some(BindingFileDialogMode::OpenFolders),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct BindingFileDialogRequest {
    pub mode: BindingFileDialogMode,
    pub title: Option<String>,
    /// Named groups of extensions without dots, such as `("Images", ["png"])`.
    pub filters: Vec<(String, Vec<String>)>,
    pub directory: Option<PathBuf>,
    /// Initial file name for save dialogs.
    pub suggested_name: Option<String>,
}

impl BindingFileDialogRequest {
    pub fn new(mode: BindingFileDialogMode) -> Self {
        Self {
            mode,
            title: None,
            filters: Vec::new(),
            directory: None,
            suggested_name: None,
        }
    }
}

/// Paths a dialog returned, `None` when the user cancelled it.
pub type BindingFileDialogResult = Result<Option<Vec<PathBuf>>, String>;

impl BindingUiHandle {
    /// Show a native file dialog. The dialog starts on the UI thread, as
    /// some platforms require, and `on_result` runs on the UI thread once the
    /// user closes it.
    pub fn show_file_dialog(
        &self,
        request: BindingFileDialogRequest,
        on_result: impl FnOnce(BindingFileDialogResult) + Send + 'static,
    ) {
        let handle = self.clone();
        self.post(move || show_file_dialog_on_ui_thread(handle, request, Box::new(on_result)));
    }
}

#[cfg(feature = "desktop")]
fn show_file_dialog_on_ui_thread(
    handle: BindingUiHandle,
    request: BindingFileDialogRequest,
    on_result: Box<dyn FnOnce(BindingFileDialogResult) + Send>,
) {
    use sui::FileDialogService;

    let mut native = sui::FileDialogRequest::new(match request.mode {
        BindingFileDialogMode::OpenFile => sui::FileDialogMode::OpenFile,
        BindingFileDialogMode::OpenFiles => sui::FileDialogMode::OpenFiles,
        BindingFileDialogMode::SaveFile => sui::FileDialogMode::SaveFile,
        BindingFileDialogMode::OpenFolder => sui::FileDialogMode::OpenFolder,
        BindingFileDialogMode::OpenFolders => sui::FileDialogMode::OpenFolders,
    });
    if let Some(title) = request.title {
        native = native.title(title);
    }
    for (name, extensions) in request.filters {
        native = native.filter(sui::FileDialogFilter::new(name, extensions));
    }
    if let Some(directory) = request.directory {
        native = native.initial_directory(directory);
    }
    if let Some(name) = request.suggested_name {
        native = native.suggested_name(name);
    }
    let dialog = sui::NativeFileDialogs.show(native);
    // The dialog future may block until the user answers; wait for it off
    // the UI thread and deliver the answer back to it.
    std::thread::spawn(move || {
        let result = pollster::block_on(dialog)
            .map(|selection| {
                selection.map(|selection| {
                    selection
                        .files
                        .iter()
                        .filter_map(|file| file.path().map(PathBuf::from))
                        .collect()
                })
            })
            .map_err(|error| error.to_string());
        handle.post(move || on_result(result));
    });
}

#[cfg(not(feature = "desktop"))]
fn show_file_dialog_on_ui_thread(
    _handle: BindingUiHandle,
    _request: BindingFileDialogRequest,
    on_result: Box<dyn FnOnce(BindingFileDialogResult) + Send>,
) {
    on_result(Err(
        "native file dialogs require the `desktop` feature".to_string()
    ));
}
