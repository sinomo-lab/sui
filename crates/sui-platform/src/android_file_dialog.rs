use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::fd::FromRawFd,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicI64, Ordering},
    },
    task::{Poll, Waker},
};

use jni::{
    Env, EnvUnowned, NativeMethod, Outcome,
    objects::{JClass, JObject, JObjectArray, JString, JValue},
    refs::Global,
};
use sui_core::{Error, Result};
use winit::platform::android::activity::AndroidApp;

use crate::file_dialog::{
    FileDialogFuture, FileDialogMode, FileDialogRequest, FileDialogSelection, FolderEntry,
    PlatformFile, PlatformFolder,
};

/// `net.sinomo.ui.platform.FilePicker`, compiled by `cargo xtask android dex`.
const FILE_PICKER_DEX: &[u8] = include_bytes!("../java/classes.dex");

// Matches the mode numbers in FilePicker.java.
const OPEN_FILE: i32 = 0;
const OPEN_FILES: i32 = 1;
const SAVE_FILE: i32 = 2;
const OPEN_FOLDER: i32 = 3;

/// The MIME type that `DocumentsContract` gives folders.
const FOLDER_MIME_TYPE: &str = "vnd.android.document/directory";

/// What a picker answered: the chosen document URIs, `None` when cancelled.
type PickerAnswer = std::result::Result<Option<Vec<String>>, String>;

#[derive(Default)]
struct PendingPicker {
    answer: Option<PickerAnswer>,
    waker: Option<Waker>,
}

static ANDROID_APP: Mutex<Option<AndroidApp>> = Mutex::new(None);
static PENDING_PICKERS: Mutex<BTreeMap<i64, Arc<Mutex<PendingPicker>>>> =
    Mutex::new(BTreeMap::new());
static NEXT_TOKEN: AtomicI64 = AtomicI64::new(1);
static FILE_PICKER_CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();
static FILE_PICKER_CLASS_LOADING: Mutex<()> = Mutex::new(());

/// Lets file dialogs reach the activity the platform runs in.
pub(crate) fn set_android_app(app: AndroidApp) {
    *lock(&ANDROID_APP) = Some(app);
}

/// Shows the system document picker. Android has no dialog titles, parent
/// windows, or starting directories, and its pickers filter by MIME type, so
/// the request's extensions become the types Android knows for them. Its
/// folder picker chooses one folder, even for `OpenFolders`.
pub(crate) fn show_file_dialog(request: FileDialogRequest) -> FileDialogFuture {
    let mode = match request.mode {
        FileDialogMode::OpenFile => OPEN_FILE,
        FileDialogMode::OpenFiles => OPEN_FILES,
        FileDialogMode::SaveFile => SAVE_FILE,
        FileDialogMode::OpenFolder | FileDialogMode::OpenFolders => OPEN_FOLDER,
    };
    let app = match android_app() {
        Ok(app) => app,
        Err(error) => return Box::pin(async move { Err(error) }),
    };
    let extensions = request
        .filters
        .into_iter()
        .flat_map(|filter| filter.extensions)
        .collect::<Vec<_>>();
    let name = request.suggested_name;

    let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    let pending = Arc::new(Mutex::new(PendingPicker::default()));
    lock(&PENDING_PICKERS).insert(token, pending.clone());
    let main_thread_app = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let shown = with_activity(&main_thread_app, |env, activity| {
            let class = file_picker_class(env, activity)?;
            let extensions = string_array(env, &extensions)?;
            let name = match &name {
                Some(name) => env.new_string(name)?.into(),
                None => JObject::null(),
            };
            env.call_static_method(
                class,
                jni::jni_str!("show"),
                jni::jni_sig!("(Landroid/app/Activity;JI[Ljava/lang/String;Ljava/lang/String;)V"),
                &[
                    activity.into(),
                    JValue::Long(token),
                    JValue::Int(mode),
                    (&extensions).into(),
                    (&name).into(),
                ],
            )?;
            Ok(())
        });
        if let Err(error) = shown {
            answer(
                token,
                Err(format!("could not show the file dialog: {error}")),
            );
        }
    }));

    Box::pin(async move {
        let answer = std::future::poll_fn(|context| {
            let mut pending = lock(&pending);
            match pending.answer.take() {
                Some(answer) => Poll::Ready(answer),
                None => {
                    pending.waker = Some(context.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
        .map_err(Error::new)?;
        let Some(uris) = answer.filter(|uris| !uris.is_empty()) else {
            return Ok(None);
        };
        let mut selection = FileDialogSelection::default();
        for uri in uris {
            let name = display_name(&app, &uri);
            if mode == OPEN_FOLDER {
                selection.folders.push(AndroidFolder::at(uri, name));
            } else {
                selection.files.push(AndroidFile::at(uri, name));
            }
        }
        Ok(Some(selection))
    })
}

/// A document the user chose in an Android picker, or found in a chosen
/// folder, named by a content URI.
#[derive(Clone, Debug)]
pub(crate) struct AndroidFile {
    uri: String,
    name: String,
}

impl AndroidFile {
    fn at(uri: String, name: String) -> PlatformFile {
        PlatformFile::from_inner(Self { uri, name })
    }

    pub(crate) fn file_name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        None
    }

    pub(crate) async fn read(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.open("r")?
            .read_to_end(&mut bytes)
            .map_err(|error| Error::new(format!("file read failed: {error}")))?;
        Ok(bytes)
    }

    pub(crate) async fn write(&self, data: &[u8]) -> Result<()> {
        self.open("wt")?
            .write_all(data)
            .map_err(|error| Error::new(format!("file write failed: {error}")))
    }

    /// Opens the document through its provider in a `ContentResolver` mode.
    fn open(&self, mode: &str) -> Result<File> {
        let app = android_app()?;
        let descriptor = with_activity(&app, |env, activity| {
            let class = file_picker_class(env, activity)?;
            let uri = env.new_string(&self.uri)?;
            let mode = env.new_string(mode)?;
            env.call_static_method(
                class,
                jni::jni_str!("open"),
                jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)I"),
                &[activity.into(), (&uri).into(), (&mode).into()],
            )?
            .i()
        })
        .map_err(|error| Error::new(format!("could not open {}: {error}", self.name)))?;
        // SAFETY: `FilePicker.open` detached the descriptor from its Java
        // owner, so the file is its only owner.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

/// A folder in a document tree the user chose in Android's folder picker,
/// named by the content URI of its document within that tree. The tree's
/// permission covers everything inside it.
#[derive(Clone, Debug)]
pub(crate) struct AndroidFolder {
    uri: String,
    name: String,
}

/// A document directly inside an [`AndroidFolder`].
struct Child {
    uri: String,
    name: String,
    folder: bool,
}

impl Child {
    fn into_entry(self) -> FolderEntry {
        if self.folder {
            FolderEntry::Folder(AndroidFolder::at(self.uri, self.name))
        } else {
            FolderEntry::File(AndroidFile::at(self.uri, self.name))
        }
    }
}

impl AndroidFolder {
    fn at(uri: String, name: String) -> PlatformFolder {
        PlatformFolder::from_inner(Self { uri, name })
    }

    pub(crate) fn name(&self) -> String {
        self.name.clone()
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        None
    }

    pub(crate) async fn entries(&self) -> Result<Vec<FolderEntry>> {
        Ok(self
            .children()?
            .into_iter()
            .map(Child::into_entry)
            .collect())
    }

    pub(crate) async fn file(&self, name: &str) -> Result<Option<PlatformFile>> {
        Ok(self
            .child(name)?
            .filter(|child| !child.folder)
            .map(|child| AndroidFile::at(child.uri, child.name)))
    }

    pub(crate) async fn folder(&self, name: &str) -> Result<Option<PlatformFolder>> {
        Ok(self
            .child(name)?
            .filter(|child| child.folder)
            .map(|child| Self::at(child.uri, child.name)))
    }

    pub(crate) async fn create_file(&self, name: &str) -> Result<PlatformFile> {
        let child = self.child_or_create(name, false)?;
        Ok(AndroidFile::at(child.uri, child.name))
    }

    pub(crate) async fn create_folder(&self, name: &str) -> Result<PlatformFolder> {
        let child = self.child_or_create(name, true)?;
        Ok(Self::at(child.uri, child.name))
    }

    fn children(&self) -> Result<Vec<Child>> {
        let app = android_app()?;
        let listing = with_activity(&app, |env, activity| {
            let class = file_picker_class(env, activity)?;
            let uri = env.new_string(&self.uri)?;
            let listing = env
                .call_static_method(
                    class,
                    jni::jni_str!("children"),
                    jni::jni_sig!(
                        "(Landroid/content/Context;Ljava/lang/String;)[Ljava/lang/String;"
                    ),
                    &[activity.into(), (&uri).into()],
                )?
                .l()?;
            let listing = env.cast_local::<JObjectArray<JString>>(listing)?;
            let mut strings = Vec::new();
            for index in 0..listing.len(env)? {
                let string = listing.get_element(env, index)?;
                strings.push(if string.is_null() {
                    String::new()
                } else {
                    string.try_to_string(env)?
                });
            }
            Ok(strings)
        })
        .map_err(|error| Error::new(format!("could not list {}: {error}", self.name)))?;
        let (children, _) = listing.as_chunks::<3>();
        Ok(children
            .iter()
            .map(|[uri, name, mime_type]| Child {
                uri: uri.clone(),
                name: name.clone(),
                folder: mime_type == FOLDER_MIME_TYPE,
            })
            .collect())
    }

    /// The child called `name`. Document providers do not look documents up
    /// by name, so this lists the folder.
    fn child(&self, name: &str) -> Result<Option<Child>> {
        Ok(self
            .children()?
            .into_iter()
            .find(|child| child.name == name))
    }

    /// The child called `name` if it is the requested kind, else a new one.
    /// Providers give a new document a unique name rather than replace one,
    /// so this checks for an existing child first.
    fn child_or_create(&self, name: &str, folder: bool) -> Result<Child> {
        match self.child(name)? {
            Some(child) if child.folder == folder => return Ok(child),
            Some(_) => {
                return Err(Error::new(format!(
                    "could not create {name} in {}: a {} has that name",
                    self.name,
                    if folder { "file" } else { "folder" }
                )));
            }
            None => {}
        }
        let app = android_app()?;
        let uri = with_activity(&app, |env, activity| {
            let class = file_picker_class(env, activity)?;
            let folder_uri = env.new_string(&self.uri)?;
            let child_name = env.new_string(name)?;
            let uri = env
                .call_static_method(
                    class,
                    jni::jni_str!("create"),
                    jni::jni_sig!(
                        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Z)Ljava/lang/String;"
                    ),
                    &[
                        activity.into(),
                        (&folder_uri).into(),
                        (&child_name).into(),
                        JValue::Bool(folder),
                    ],
                )?
                .l()?;
            env.cast_local::<JString>(uri)?.try_to_string(env)
        })
        .map_err(|error| {
            Error::new(format!("could not create {name} in {}: {error}", self.name))
        })?;
        // The provider may adjust the name, so ask for the one it chose.
        let name = display_name(&app, &uri);
        Ok(Child { uri, name, folder })
    }
}

/// The name a document's provider shows for it, or its URI if that fails.
fn display_name(app: &AndroidApp, uri: &str) -> String {
    with_activity(app, |env, activity| {
        let class = file_picker_class(env, activity)?;
        let uri = env.new_string(uri)?;
        let name = env
            .call_static_method(
                class,
                jni::jni_str!("displayName"),
                jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;"),
                &[activity.into(), (&uri).into()],
            )?
            .l()?;
        env.cast_local::<JString>(name)?.try_to_string(env)
    })
    .unwrap_or_else(|_| uri.to_string())
}

fn android_app() -> Result<AndroidApp> {
    lock(&ANDROID_APP)
        .clone()
        .ok_or_else(|| Error::new("file dialogs need the Android platform to be running"))
}

fn answer(token: i64, answer: PickerAnswer) {
    let Some(pending) = lock(&PENDING_PICKERS).remove(&token) else {
        return;
    };
    let mut pending = lock(&pending);
    pending.answer = Some(answer);
    if let Some(waker) = pending.waker.take() {
        waker.wake();
    }
}

/// `FilePicker.onResult`, which the Java main thread calls once a picker
/// answers.
extern "system" fn on_picker_result<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    token: jni::sys::jlong,
    uris: JObjectArray<'local, JString<'local>>,
    error: JString<'local>,
) {
    let outcome = env
        .with_env(|env| -> jni::errors::Result<PickerAnswer> {
            if !error.is_null() {
                return Ok(Err(error.try_to_string(env)?));
            }
            if uris.is_null() {
                return Ok(Ok(None));
            }
            let mut chosen = Vec::new();
            for index in 0..uris.len(env)? {
                chosen.push(uris.get_element(env, index)?.try_to_string(env)?);
            }
            Ok(Ok(Some(chosen)))
        })
        .into_outcome();
    answer(
        token,
        match outcome {
            Outcome::Ok(picked) => picked,
            Outcome::Err(error) => Err(format!("could not read the file dialog's answer: {error}")),
            Outcome::Panic(_) => Err("reading the file dialog's answer panicked".to_string()),
        },
    );
}

/// The `FilePicker` class, loaded from the embedded DEX the first time it is
/// needed. The activity's class loader cannot see it, so callers must use this
/// class reference rather than look the class up by name.
fn file_picker_class<'a>(
    env: &mut Env<'_>,
    activity: &JObject<'_>,
) -> jni::errors::Result<&'a Global<JClass<'static>>> {
    if let Some(class) = FILE_PICKER_CLASS.get() {
        return Ok(class);
    }
    let _loading = lock(&FILE_PICKER_CLASS_LOADING);
    if let Some(class) = FILE_PICKER_CLASS.get() {
        return Ok(class);
    }
    let dex = env.byte_array_from_slice(FILE_PICKER_DEX)?;
    let buffer = env
        .call_static_method(
            jni::jni_str!("java/nio/ByteBuffer"),
            jni::jni_str!("wrap"),
            jni::jni_sig!("([B)Ljava/nio/ByteBuffer;"),
            &[(&dex).into()],
        )?
        .l()?;
    let parent = env
        .call_method(
            activity,
            jni::jni_str!("getClassLoader"),
            jni::jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let loader = env.new_object(
        jni::jni_str!("dalvik/system/InMemoryDexClassLoader"),
        jni::jni_sig!("(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V"),
        &[(&buffer).into(), (&parent).into()],
    )?;
    let class_name = env.new_string("net.sinomo.ui.platform.FilePicker")?;
    let class = env
        .call_method(
            &loader,
            jni::jni_str!("loadClass"),
            jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[(&class_name).into()],
        )?
        .l()?;
    let class = env.cast_local::<JClass>(class)?;
    let natives = [unsafe {
        NativeMethod::from_raw_parts(
            jni::jni_str!("onResult"),
            jni::jni_str!("(J[Ljava/lang/String;Ljava/lang/String;)V"),
            on_picker_result as *mut _,
        )
    }];
    unsafe { env.register_native_methods(&class, &natives)? };
    let class = env.new_global_ref(class)?;
    Ok(FILE_PICKER_CLASS.get_or_init(|| class))
}

fn string_array<'local>(
    env: &mut Env<'local>,
    strings: &[String],
) -> jni::errors::Result<JObjectArray<'local, JString<'local>>> {
    let array = JObjectArray::<JString>::new(env, strings.len(), JString::null())?;
    for (index, string) in strings.iter().enumerate() {
        let string = env.new_string(string)?;
        array.set_element(env, index, &string)?;
    }
    Ok(array)
}

/// Runs `call` with the activity on the current thread, attaching it to the
/// Java VM if needed, and reports a Java exception it throws as an error.
fn with_activity<T>(
    app: &AndroidApp,
    call: impl FnOnce(&mut Env<'_>, &JObject<'_>) -> jni::errors::Result<T>,
) -> std::result::Result<T, String> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw_activity = app.activity_as_ptr() as jni::sys::jobject;
    vm.attach_current_thread(|env| -> jni::errors::Result<_> {
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
        Ok(match call(env, &activity) {
            Ok(value) => Ok(value),
            Err(jni::errors::Error::JavaException) => Err(match env.exception_catch() {
                Err(jni::errors::Error::CaughtJavaException { name, msg, .. }) => {
                    format!("{name}: {msg}")
                }
                Err(error) => error.to_string(),
                Ok(()) => "a Java exception was thrown".to_string(),
            }),
            Err(error) => Err(error.to_string()),
        })
    })
    .unwrap_or_else(|error| Err(error.to_string()))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
