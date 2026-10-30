//! Browser file and folder handles. Folders come from the File System Access
//! API's `showDirectoryPicker`, which only Chromium browsers provide; `web-sys`
//! marks it unstable, so it is called through `Reflect`.

use std::path::Path;

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use sui_core::{Error, Result};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::file_dialog::{
    FileDialogFuture, FileDialogSelection, FolderEntry, PlatformFile, PlatformFolder,
};

/// Shows the browser's folder picker for reading and writing. Browsers only
/// show it while handling the click or key press that asked for it, so call
/// this from that handler rather than when the returned future is polled.
pub(crate) fn show_directory_picker() -> FileDialogFuture {
    let picking = start_directory_picker();
    Box::pin(async move {
        match JsFuture::from(picking?).await {
            Ok(handle) => Ok(Some(FileDialogSelection {
                files: Vec::new(),
                folders: vec![WebFolder::at(handle)],
            })),
            Err(error) if error_name(&error) == "AbortError" => Ok(None),
            Err(error) => Err(js_error("could not show the folder dialog", &error)),
        }
    })
}

fn start_directory_picker() -> Result<Promise> {
    let window = web_sys::window().ok_or_else(|| Error::new("folder dialogs need a window"))?;
    let picker = Reflect::get(&window, &JsValue::from_str("showDirectoryPicker"))
        .ok()
        .and_then(|picker| picker.dyn_into::<Function>().ok())
        .ok_or_else(|| Error::new("this browser cannot show folder dialogs"))?;
    let options = Object::new();
    Reflect::set(&options, &"mode".into(), &"readwrite".into())
        .map_err(|error| js_error("could not show the folder dialog", &error))?;
    picker
        .call1(&window, &options)
        .and_then(|picking| picking.dyn_into::<Promise>())
        .map_err(|error| js_error("could not show the folder dialog", &error))
}

/// A file from a browser file dialog, or a file handle from a chosen folder.
#[derive(Clone, Debug)]
pub(crate) enum WebFile {
    Picked(rfd::FileHandle),
    Handle(JsValue),
}

impl From<rfd::FileHandle> for WebFile {
    fn from(file: rfd::FileHandle) -> Self {
        Self::Picked(file)
    }
}

impl WebFile {
    fn at(handle: JsValue) -> PlatformFile {
        PlatformFile::from_inner(Self::Handle(handle))
    }

    pub(crate) fn file_name(&self) -> String {
        match self {
            Self::Picked(file) => file.file_name(),
            Self::Handle(handle) => handle_name(handle),
        }
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        None
    }

    pub(crate) async fn read(&self) -> Result<Vec<u8>> {
        match self {
            Self::Picked(file) => Ok(file.read().await),
            Self::Handle(handle) => {
                let failed =
                    |error| js_error(&format!("could not read {}", handle_name(handle)), &error);
                let file = call(handle, "getFile", &[]).await.map_err(failed)?;
                let bytes = call(&file, "arrayBuffer", &[]).await.map_err(failed)?;
                Ok(Uint8Array::new(&bytes).to_vec())
            }
        }
    }

    pub(crate) async fn write(&self, data: &[u8]) -> Result<()> {
        match self {
            Self::Picked(file) => file
                .write(data)
                .await
                .map_err(|error| Error::new(format!("file write failed: {error}"))),
            Self::Handle(handle) => {
                let failed =
                    |error| js_error(&format!("could not write {}", handle_name(handle)), &error);
                // The stream starts empty and replaces the file when it closes.
                let stream = call(handle, "createWritable", &[]).await.map_err(failed)?;
                if let Err(error) = call(&stream, "write", &[Uint8Array::from(data).into()]).await {
                    let _ = call(&stream, "abort", &[]).await;
                    return Err(failed(error));
                }
                call(&stream, "close", &[]).await.map_err(failed)?;
                Ok(())
            }
        }
    }
}

/// A `FileSystemDirectoryHandle`.
#[derive(Clone, Debug)]
pub(crate) struct WebFolder {
    handle: JsValue,
}

impl WebFolder {
    fn at(handle: JsValue) -> PlatformFolder {
        PlatformFolder::from_inner(Self { handle })
    }

    pub(crate) fn name(&self) -> String {
        handle_name(&self.handle)
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        None
    }

    pub(crate) async fn entries(&self) -> Result<Vec<FolderEntry>> {
        let failed = |error| js_error(&format!("could not list {}", self.name()), &error);
        let entries = Reflect::get(&self.handle, &"values".into())
            .and_then(|values| values.dyn_into::<Function>())
            .and_then(|values| values.call0(&self.handle))
            .map_err(failed)?;
        let mut found = Vec::new();
        loop {
            let step = call(&entries, "next", &[]).await.map_err(failed)?;
            if Reflect::get(&step, &"done".into())
                .map_err(failed)?
                .is_truthy()
            {
                return Ok(found);
            }
            let handle = Reflect::get(&step, &"value".into()).map_err(failed)?;
            let kind = Reflect::get(&handle, &"kind".into()).map_err(failed)?;
            found.push(if kind.as_string().as_deref() == Some("directory") {
                FolderEntry::Folder(Self::at(handle))
            } else {
                FolderEntry::File(WebFile::at(handle))
            });
        }
    }

    pub(crate) async fn file(&self, name: &str) -> Result<Option<PlatformFile>> {
        self.child("getFileHandle", name, false)
            .await
            .map(|handle| handle.map(WebFile::at))
    }

    pub(crate) async fn folder(&self, name: &str) -> Result<Option<PlatformFolder>> {
        self.child("getDirectoryHandle", name, false)
            .await
            .map(|handle| handle.map(Self::at))
    }

    pub(crate) async fn create_file(&self, name: &str) -> Result<PlatformFile> {
        let handle = self.child("getFileHandle", name, true).await?;
        Ok(WebFile::at(handle.unwrap_or_default()))
    }

    pub(crate) async fn create_folder(&self, name: &str) -> Result<PlatformFolder> {
        let handle = self.child("getDirectoryHandle", name, true).await?;
        Ok(Self::at(handle.unwrap_or_default()))
    }

    /// Calls `getFileHandle` or `getDirectoryHandle`, which reject with
    /// `NotFoundError` when nothing has the name and `TypeMismatchError` when
    /// the entry is the other kind.
    async fn child(&self, getter: &str, name: &str, create: bool) -> Result<Option<JsValue>> {
        let options = Object::new();
        Reflect::set(&options, &"create".into(), &create.into())
            .map_err(|error| js_error("could not open a folder entry", &error))?;
        match call(&self.handle, getter, &[name.into(), options.into()]).await {
            Ok(handle) => Ok(Some(handle)),
            Err(error)
                if !create
                    && matches!(
                        error_name(&error).as_str(),
                        "NotFoundError" | "TypeMismatchError"
                    ) =>
            {
                Ok(None)
            }
            Err(error) => Err(js_error(
                &format!(
                    "could not {} {name} in {}",
                    if create { "create" } else { "open" },
                    self.name()
                ),
                &error,
            )),
        }
    }
}

/// Calls `object.method(...arguments)` and waits for the promise it returns.
async fn call(
    object: &JsValue,
    method: &str,
    arguments: &[JsValue],
) -> std::result::Result<JsValue, JsValue> {
    let function = Reflect::get(object, &JsValue::from_str(method))?.dyn_into::<Function>()?;
    let result = function.apply(object, &arguments.iter().collect::<Array>())?;
    JsFuture::from(Promise::resolve(&result)).await
}

fn handle_name(handle: &JsValue) -> String {
    Reflect::get(handle, &"name".into())
        .ok()
        .and_then(|name| name.as_string())
        .unwrap_or_default()
}

fn error_name(error: &JsValue) -> String {
    Reflect::get(error, &"name".into())
        .ok()
        .and_then(|name| name.as_string())
        .unwrap_or_default()
}

fn js_error(context: &str, error: &JsValue) -> Error {
    let message = Reflect::get(error, &"message".into())
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    match error_name(error) {
        name if name.is_empty() => Error::new(format!("{context}: {message}")),
        name => Error::new(format!("{context}: {name}: {message}")),
    }
}
