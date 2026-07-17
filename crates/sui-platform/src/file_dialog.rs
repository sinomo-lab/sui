use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use sui_core::{Error, Result};

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

pub type FileDialogFuture =
    Pin<Box<dyn Future<Output = Result<Option<FileDialogSelection>>> + 'static>>;

pub trait FileDialogService {
    fn show(&self, request: FileDialogRequest) -> FileDialogFuture;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFileDialogs;

impl FileDialogService for NativeFileDialogs {
    fn show(&self, request: FileDialogRequest) -> FileDialogFuture {
        Box::pin(show_file_dialog(request))
    }
}

pub async fn show_file_dialog(request: FileDialogRequest) -> Result<Option<FileDialogSelection>> {
    if !RFD_SUPPORTED {
        return Err(Error::new(
            "native file dialogs are not supported on this platform",
        ));
    }

    #[cfg(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    ))]
    {
        let mut dialog = rfd::AsyncFileDialog::new();
        if let Some(title) = request.title {
            dialog = dialog.set_title(title);
        }
        if let Some(name) = request.suggested_name {
            dialog = dialog.set_file_name(name);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(directory) = request.initial_directory {
            dialog = dialog.set_directory(directory);
        }
        dialog = dialog.set_can_create_directories(request.can_create_directories);
        for filter in request.filters {
            dialog = dialog.add_filter(filter.name, &filter.extensions);
        }

        let handles = match request.mode {
            FileDialogMode::OpenFile => dialog.pick_file().await.map(|file| vec![file]),
            FileDialogMode::OpenFiles => dialog.pick_files().await,
            FileDialogMode::SaveFile => dialog.save_file().await.map(|file| vec![file]),
            #[cfg(not(target_arch = "wasm32"))]
            FileDialogMode::OpenFolder => dialog.pick_folder().await.map(|file| vec![file]),
            #[cfg(not(target_arch = "wasm32"))]
            FileDialogMode::OpenFolders => dialog.pick_folders().await,
            #[cfg(target_arch = "wasm32")]
            FileDialogMode::OpenFolder | FileDialogMode::OpenFolders => {
                return Err(Error::new("folder dialogs are not available in web builds"));
            }
        };
        Ok(handles.map(|files| FileDialogSelection {
            files: files
                .into_iter()
                .map(|inner| PlatformFile { inner })
                .collect(),
        }))
    }

    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    {
        let _ = request;
        Err(Error::new(
            "native file dialogs are not supported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_dialog_request_preserves_portable_options() {
        let request = FileDialogRequest::new(FileDialogMode::OpenFiles)
            .title("Attach files")
            .filter(FileDialogFilter::new("Text", ["txt", "md"]))
            .suggested_name("notes.md")
            .can_create_directories(false);
        assert_eq!(request.mode, FileDialogMode::OpenFiles);
        assert_eq!(request.filters[0].extensions, ["txt", "md"]);
        assert_eq!(request.suggested_name.as_deref(), Some("notes.md"));
        assert!(!request.can_create_directories);
    }
}
