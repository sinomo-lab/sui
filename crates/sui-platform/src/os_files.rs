use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sui_core::{Error, Result};

use crate::file_dialog::{FolderEntry, PlatformFile, PlatformFolder};

/// A file on the local file system, chosen in a desktop file dialog or found
/// in a chosen folder.
#[derive(Clone, Debug)]
pub(crate) struct OsFile(rfd::FileHandle);

impl From<rfd::FileHandle> for OsFile {
    fn from(handle: rfd::FileHandle) -> Self {
        Self(handle)
    }
}

impl OsFile {
    fn at(path: PathBuf) -> PlatformFile {
        PlatformFile::from_inner(Self(rfd::FileHandle::from(path)))
    }

    pub(crate) fn file_name(&self) -> String {
        self.0.file_name()
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        Some(self.0.path())
    }

    pub(crate) async fn read(&self) -> Result<Vec<u8>> {
        Ok(self.0.read().await)
    }

    pub(crate) async fn write(&self, data: &[u8]) -> Result<()> {
        self.0
            .write(data)
            .await
            .map_err(|error| Error::new(format!("file write failed: {error}")))
    }
}

/// A folder on the local file system, chosen in a desktop folder dialog or
/// found in a chosen folder.
#[derive(Clone, Debug)]
pub(crate) struct OsFolder {
    path: PathBuf,
}

impl OsFolder {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn at(path: PathBuf) -> PlatformFolder {
        PlatformFolder::from_inner(Self::new(path))
    }

    pub(crate) fn name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        Some(&self.path)
    }

    pub(crate) async fn entries(&self) -> Result<Vec<FolderEntry>> {
        let failed = |error| folder_error("list", &self.path, error);
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.path).map_err(failed)? {
            let path = entry.map_err(failed)?.path();
            // Follow symbolic links, as opening the entry would. Broken links
            // and special files such as sockets are neither files nor folders.
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_dir() => {
                    entries.push(FolderEntry::Folder(Self::at(path)));
                }
                Ok(metadata) if metadata.is_file() => {
                    entries.push(FolderEntry::File(OsFile::at(path)));
                }
                _ => {}
            }
        }
        Ok(entries)
    }

    pub(crate) async fn file(&self, name: &str) -> Result<Option<PlatformFile>> {
        let path = self.path.join(name);
        Ok(path.is_file().then(|| OsFile::at(path)))
    }

    pub(crate) async fn folder(&self, name: &str) -> Result<Option<PlatformFolder>> {
        let path = self.path.join(name);
        Ok(path.is_dir().then(|| Self::at(path)))
    }

    pub(crate) async fn create_file(&self, name: &str) -> Result<PlatformFile> {
        let path = self.path.join(name);
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| folder_error("create", &path, error))?;
        Ok(OsFile::at(path))
    }

    pub(crate) async fn create_folder(&self, name: &str) -> Result<PlatformFolder> {
        let path = self.path.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {}
            Err(error) => return Err(folder_error("create", &path, error)),
        }
        Ok(Self::at(path))
    }
}

fn folder_error(action: &str, path: &Path, error: io::Error) -> Error {
    Error::new(format!("could not {action} {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    /// Desktop folder operations finish without waiting, apart from file reads
    /// and writes, which rfd runs on a thread.
    fn finish<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            std::thread::yield_now();
        }
    }

    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(test: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("sui-platform-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn folder(&self) -> PlatformFolder {
            OsFolder::at(self.0.clone())
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn folder_lists_its_files_and_folders() {
        let temp = TempFolder::new("lists");
        fs::write(temp.0.join("notes.txt"), "notes").unwrap();
        fs::create_dir(temp.0.join("drafts")).unwrap();
        let folder = temp.folder();

        let mut entries = finish(folder.entries())
            .unwrap()
            .iter()
            .map(|entry| (entry.name(), matches!(entry, FolderEntry::Folder(_))))
            .collect::<Vec<_>>();
        entries.sort();
        assert_eq!(
            entries,
            [
                ("drafts".to_string(), true),
                ("notes.txt".to_string(), false)
            ]
        );
        assert_eq!(folder.path(), Some(temp.0.as_path()));
    }

    #[test]
    fn folder_finds_entries_by_name_and_kind() {
        let temp = TempFolder::new("finds");
        fs::write(temp.0.join("notes.txt"), "notes").unwrap();
        fs::create_dir(temp.0.join("drafts")).unwrap();
        let folder = temp.folder();

        let notes = finish(folder.file("notes.txt")).unwrap().unwrap();
        assert_eq!(finish(notes.read()).unwrap(), b"notes");
        assert!(finish(folder.file("drafts")).unwrap().is_none());
        assert!(finish(folder.file("missing.txt")).unwrap().is_none());
        let drafts = finish(folder.folder("drafts")).unwrap().unwrap();
        assert_eq!(drafts.name(), "drafts");
        assert!(finish(folder.folder("notes.txt")).unwrap().is_none());
    }

    #[test]
    fn creating_an_existing_entry_returns_it_unchanged() {
        let temp = TempFolder::new("creates");
        let folder = temp.folder();

        let file = finish(folder.create_file("new.txt")).unwrap();
        assert_eq!(finish(file.read()).unwrap(), b"");
        finish(file.write(b"kept")).unwrap();
        let again = finish(folder.create_file("new.txt")).unwrap();
        assert_eq!(finish(again.read()).unwrap(), b"kept");

        let child = finish(folder.create_folder("child")).unwrap();
        finish(child.create_file("inner.txt")).unwrap();
        let child = finish(folder.create_folder("child")).unwrap();
        assert_eq!(finish(child.entries()).unwrap().len(), 1);
        assert!(temp.0.join("child/inner.txt").is_file());
    }

    #[test]
    fn creating_an_entry_over_the_other_kind_fails() {
        let temp = TempFolder::new("conflicts");
        fs::write(temp.0.join("file"), "").unwrap();
        fs::create_dir(temp.0.join("folder")).unwrap();
        let folder = temp.folder();

        assert!(finish(folder.create_file("folder")).is_err());
        assert!(finish(folder.create_folder("file")).is_err());
    }

    #[test]
    fn entry_names_cannot_leave_the_folder() {
        let temp = TempFolder::new("names");
        let folder = temp.folder();

        for name in ["", ".", "..", "../escape.txt", "a/b", "a\\b", "nul\0"] {
            assert!(finish(folder.create_file(name)).is_err(), "{name:?}");
            assert!(finish(folder.file(name)).is_err(), "{name:?}");
        }
        assert!(!temp.0.parent().unwrap().join("escape.txt").exists());
    }
}
