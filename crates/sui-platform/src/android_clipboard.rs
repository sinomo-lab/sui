use jni::{
    objects::{JObject, JString, JValue},
    refs::Global,
};
use sui_core::ClipboardBackend;
use winit::platform::android::activity::AndroidApp;

const CLIPBOARD_MAIN_THREAD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// Android system-clipboard bridge for the NativeActivity platform.
///
/// Android framework objects must be used on the Java main thread. Clipboard
/// calls originate on Winit's native event-loop thread, so each operation is
/// posted to the Java thread and synchronously returns its small result.
pub(crate) struct AndroidClipboardBackend {
    app: AndroidApp,
    fallback: Option<String>,
}

impl AndroidClipboardBackend {
    pub(crate) fn new(app: AndroidApp) -> Self {
        Self {
            app,
            fallback: None,
        }
    }

    fn system_text(&self) -> Option<jni::errors::Result<Option<String>>> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let app = self.app.clone();
        self.app.run_on_java_main_thread(Box::new(move || {
            let _ = send.send(read_system_text(&app));
        }));
        receive.recv_timeout(CLIPBOARD_MAIN_THREAD_TIMEOUT).ok()
    }

    fn set_system_text(&self, text: String) -> Option<jni::errors::Result<()>> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let app = self.app.clone();
        self.app.run_on_java_main_thread(Box::new(move || {
            let _ = send.send(write_system_text(&app, &text));
        }));
        receive.recv_timeout(CLIPBOARD_MAIN_THREAD_TIMEOUT).ok()
    }
}

impl ClipboardBackend for AndroidClipboardBackend {
    fn text(&mut self) -> Option<String> {
        match self.system_text() {
            Some(Ok(text)) => text,
            Some(Err(_)) | None => self.fallback.clone(),
        }
    }

    fn set_text(&mut self, text: &str) {
        self.fallback = Some(text.to_owned());
        let _ = self.set_system_text(text.to_owned());
    }
}

fn read_system_text(app: &AndroidApp) -> jni::errors::Result<Option<String>> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw_activity = app.activity_as_ptr() as jni::sys::jobject;
    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
        let service_name: JObject = env.new_string("clipboard")?.into();
        let manager = env
            .call_method(
                &activity,
                jni::jni_str!("getSystemService"),
                jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&service_name)],
            )?
            .l()?;
        let has_clip = env
            .call_method(
                &manager,
                jni::jni_str!("hasPrimaryClip"),
                jni::jni_sig!("()Z"),
                &[],
            )?
            .z()?;
        if !has_clip {
            return Ok(None);
        }

        let clip = env
            .call_method(
                &manager,
                jni::jni_str!("getPrimaryClip"),
                jni::jni_sig!("()Landroid/content/ClipData;"),
                &[],
            )?
            .l()?;
        if clip.is_null() {
            return Ok(None);
        }
        let count = env
            .call_method(
                &clip,
                jni::jni_str!("getItemCount"),
                jni::jni_sig!("()I"),
                &[],
            )?
            .i()?;
        if count == 0 {
            return Ok(None);
        }

        let item = env
            .call_method(
                &clip,
                jni::jni_str!("getItemAt"),
                jni::jni_sig!("(I)Landroid/content/ClipData$Item;"),
                &[JValue::Int(0)],
            )?
            .l()?;
        let text = env
            .call_method(
                &item,
                jni::jni_str!("coerceToText"),
                jni::jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                &[JValue::Object(activity.as_ref())],
            )?
            .l()?;
        if text.is_null() {
            return Ok(None);
        }
        let string = env
            .call_method(
                &text,
                jni::jni_str!("toString"),
                jni::jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        let string = env.as_cast::<JString>(&string)?;
        Ok(Some(string.try_to_string(env)?))
    })
}

fn write_system_text(app: &AndroidApp, text: &str) -> jni::errors::Result<()> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw_activity = app.activity_as_ptr() as jni::sys::jobject;
    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw_activity)? };
        let service_name: JObject = env.new_string("clipboard")?.into();
        let manager = env
            .call_method(
                &activity,
                jni::jni_str!("getSystemService"),
                jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&service_name)],
            )?
            .l()?;
        let label: JObject = env.new_string("Sinomo")?.into();
        let text: JObject = env.new_string(text)?.into();
        let clip = env
            .call_static_method(
                jni::jni_str!("android/content/ClipData"),
                jni::jni_str!("newPlainText"),
                jni::jni_sig!(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                ),
                &[JValue::Object(&label), JValue::Object(&text)],
            )?
            .l()?;
        env.call_method(
            &manager,
            jni::jni_str!("setPrimaryClip"),
            jni::jni_sig!("(Landroid/content/ClipData;)V"),
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}
