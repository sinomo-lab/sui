use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
};

use sui_core::{Error, Result, WindowId};

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use winit::window::Window;

const RFD_SUPPORTED: bool = cfg!(any(
    target_arch = "wasm32",
    target_os = "windows",
    target_os = "macos",
    target_os = "linux"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialogMode {
    OpenFile,
    OpenFiles,
    SaveFile,
    OpenFolder,
    OpenFolders,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileDialogFilter {
    pub fn new(
        name: impl Into<String>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            extensions: extensions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDialogRequest {
    pub mode: FileDialogMode,
    pub title: Option<String>,
    pub filters: Vec<FileDialogFilter>,
    pub initial_directory: Option<PathBuf>,
    pub suggested_name: Option<String>,
    pub can_create_directories: bool,
    /// Logical SUI window that should own the native dialog.
    ///
    /// Native desktop hosts resolve this to a live window and retain it while
    /// the returned dialog future is active. Web builds ignore this field.
    pub parent_window: Option<WindowId>,
}

impl FileDialogRequest {
    pub fn new(mode: FileDialogMode) -> Self {
        Self {
            mode,
            title: None,
            filters: Vec::new(),
            initial_directory: None,
            suggested_name: None,
            can_create_directories: true,
            parent_window: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn filter(mut self, filter: FileDialogFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn initial_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.initial_directory = Some(directory.into());
        self
    }

    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }

    pub const fn can_create_directories(mut self, can_create: bool) -> Self {
        self.can_create_directories = can_create;
        self
    }

    pub const fn parent_window(mut self, window_id: WindowId) -> Self {
        self.parent_window = Some(window_id);
        self
    }

    pub const fn clear_parent_window(mut self) -> Self {
        self.parent_window = None;
        self
    }
}

#[derive(Clone, Debug)]
pub struct PlatformFile {
    #[cfg(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    ))]
    inner: rfd::FileHandle,
    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    name: String,
}

impl PlatformFile {
    pub fn file_name(&self) -> String {
        #[cfg(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            self.inner.file_name()
        }
        #[cfg(not(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            self.name.clone()
        }
    }

    pub fn path(&self) -> Option<&Path> {
        #[cfg(all(
            not(target_arch = "wasm32"),
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        {
            Some(self.inner.path())
        }
        #[cfg(not(all(
            not(target_arch = "wasm32"),
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        )))]
        {
            None
        }
    }

    pub async fn read(&self) -> Result<Vec<u8>> {
        #[cfg(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            Ok(self.inner.read().await)
        }
        #[cfg(not(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            Err(Error::new(
                "file handles are not supported on this platform",
            ))
        }
    }

    pub async fn write(&self, data: &[u8]) -> Result<()> {
        #[cfg(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ))]
        {
            self.inner
                .write(data)
                .await
                .map_err(|error| Error::new(format!("file write failed: {error}")))
        }
        #[cfg(not(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            let _ = data;
            Err(Error::new(
                "file handles are not supported on this platform",
            ))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileDialogSelection {
    pub files: Vec<PlatformFile>,
}

impl FileDialogSelection {
    pub fn first(&self) -> Option<&PlatformFile> {
        self.files.first()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type FileDialogFuture =
    Pin<Box<dyn Future<Output = Result<Option<FileDialogSelection>>> + Send + 'static>>;
#[cfg(target_arch = "wasm32")]
pub type FileDialogFuture =
    Pin<Box<dyn Future<Output = Result<Option<FileDialogSelection>>> + 'static>>;

pub trait FileDialogService {
    /// Prepare and start a dialog operation.
    ///
    /// Call this method from the UI callback that initiated the request. The
    /// native implementation constructs the rfd operation synchronously (as
    /// required by macOS), then returns a `Send` future that may be polled by a
    /// background executor. Web callers must create and begin polling the
    /// returned local future during browser user activation.
    fn show(&self, request: FileDialogRequest) -> FileDialogFuture;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFileDialogs;

impl FileDialogService for NativeFileDialogs {
    fn show(&self, request: FileDialogRequest) -> FileDialogFuture {
        prepare_file_dialog(request)
    }
}

/// Show a dialog from an async context.
///
/// Unlike [`FileDialogService::show`], construction is deferred until this
/// future is first polled. UI callbacks that need macOS main-thread setup or
/// browser user activation should call the service method instead.
pub async fn show_file_dialog(request: FileDialogRequest) -> Result<Option<FileDialogSelection>> {
    prepare_file_dialog(request).await
}

fn prepare_file_dialog(request: FileDialogRequest) -> FileDialogFuture {
    if !RFD_SUPPORTED {
        return Box::pin(async {
            Err(Error::new(
                "native file dialogs are not supported on this platform",
            ))
        });
    }

    #[cfg(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    ))]
    {
        let FileDialogRequest {
            mode,
            title,
            filters,
            initial_directory,
            suggested_name,
            can_create_directories,
            parent_window,
        } = request;
        let mut dialog = rfd::AsyncFileDialog::new();
        if let Some(title) = title {
            dialog = dialog.set_title(title);
        }
        if let Some(name) = suggested_name {
            dialog = dialog.set_file_name(name);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(directory) = initial_directory {
            dialog = dialog.set_directory(directory);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = initial_directory;
        dialog = dialog.set_can_create_directories(can_create_directories);
        for filter in filters {
            dialog = dialog.add_filter(filter.name, &filter.extensions);
        }

        #[cfg(all(
            not(target_arch = "wasm32"),
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        let parent_guard = if let Some(parent_window) = parent_window {
            let Some(parent) = resolve_native_file_dialog_parent(parent_window) else {
                return Box::pin(async move {
                    Err(Error::new(format!(
                        "file dialog parent window {} is no longer available",
                        parent_window.get()
                    )))
                });
            };
            dialog = dialog.set_parent(parent.as_ref());
            Some(parent)
        } else {
            None
        };
        #[cfg(target_arch = "wasm32")]
        let parent_guard = {
            let _ = parent_window;
        };

        match mode {
            FileDialogMode::OpenFile => {
                let operation = dialog.pick_file();
                Box::pin(async move {
                    let result = await_dialog_with_parent(parent_guard, operation).await;
                    Ok(result.map(|file| selection(vec![file])))
                })
            }
            FileDialogMode::OpenFiles => {
                let operation = dialog.pick_files();
                Box::pin(async move {
                    let result = await_dialog_with_parent(parent_guard, operation).await;
                    Ok(result.map(selection))
                })
            }
            FileDialogMode::SaveFile => {
                let operation = dialog.save_file();
                Box::pin(async move {
                    let result = await_dialog_with_parent(parent_guard, operation).await;
                    Ok(result.map(|file| selection(vec![file])))
                })
            }
            #[cfg(not(target_arch = "wasm32"))]
            FileDialogMode::OpenFolder => {
                let operation = dialog.pick_folder();
                Box::pin(async move {
                    let result = await_dialog_with_parent(parent_guard, operation).await;
                    Ok(result.map(|file| selection(vec![file])))
                })
            }
            #[cfg(not(target_arch = "wasm32"))]
            FileDialogMode::OpenFolders => {
                let operation = dialog.pick_folders();
                Box::pin(async move {
                    let result = await_dialog_with_parent(parent_guard, operation).await;
                    Ok(result.map(selection))
                })
            }
            #[cfg(target_arch = "wasm32")]
            FileDialogMode::OpenFolder | FileDialogMode::OpenFolders => Box::pin(async {
                Err(Error::new("folder dialogs are not available in web builds"))
            }),
        }
    }

    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    {
        let _ = request;
        Box::pin(async {
            Err(Error::new(
                "native file dialogs are not supported on this platform",
            ))
        })
    }
}

async fn await_dialog_with_parent<Parent, Operation>(
    _parent: Parent,
    operation: Operation,
) -> Operation::Output
where
    Operation: Future,
{
    operation.await
}

#[cfg(any(
    target_arch = "wasm32",
    target_os = "windows",
    target_os = "macos",
    target_os = "linux"
))]
fn selection(files: Vec<rfd::FileHandle>) -> FileDialogSelection {
    FileDialogSelection {
        files: files
            .into_iter()
            .map(|inner| PlatformFile { inner })
            .collect(),
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
type NativeFileDialogParents = HashMap<WindowId, Weak<Window>>;

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn native_file_dialog_parents() -> &'static Mutex<NativeFileDialogParents> {
    static PARENTS: OnceLock<Mutex<NativeFileDialogParents>> = OnceLock::new();
    PARENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub(crate) fn register_native_file_dialog_parent(window_id: WindowId, window: &Arc<Window>) {
    native_file_dialog_parents()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(window_id, Arc::downgrade(window));
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub(crate) fn unregister_native_file_dialog_parent(window_id: WindowId) {
    native_file_dialog_parents()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&window_id);
}

#[cfg(all(
    not(target_arch = "wasm32"),
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn resolve_native_file_dialog_parent(window_id: WindowId) -> Option<Arc<Window>> {
    let mut parents = native_file_dialog_parents()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let parent = parents.get(&window_id).and_then(Weak::upgrade);
    if parent.is_none() {
        parents.remove(&window_id);
    }
    parent
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn file_dialog_request_preserves_portable_options() {
        let parent = WindowId::new(73);
        let request = FileDialogRequest::new(FileDialogMode::OpenFiles)
            .title("Attach files")
            .filter(FileDialogFilter::new("Text", ["txt", "md"]))
            .suggested_name("notes.md")
            .can_create_directories(false)
            .parent_window(parent);
        assert_eq!(request.mode, FileDialogMode::OpenFiles);
        assert_eq!(request.filters[0].extensions, ["txt", "md"]);
        assert_eq!(request.suggested_name.as_deref(), Some("notes.md"));
        assert!(!request.can_create_directories);
        assert_eq!(request.parent_window, Some(parent));
        assert_eq!(request.clone().clear_parent_window().parent_window, None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_file_dialog_future_can_move_to_a_background_executor() {
        fn assert_send<T: Send>() {}
        assert_send::<FileDialogFuture>();
    }

    #[test]
    fn dialog_parent_guard_lives_until_operation_finishes() {
        let parent = Arc::new(());
        let weak_parent = Arc::downgrade(&parent);
        let operation = std::future::poll_fn({
            let weak_parent = weak_parent.clone();
            move |_| {
                assert!(weak_parent.upgrade().is_some());
                Poll::Ready(())
            }
        });
        let future = await_dialog_with_parent(parent, operation);
        assert!(weak_parent.upgrade().is_some());

        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
        assert!(weak_parent.upgrade().is_none());
    }
}
