use jni::{
    objects::{JObject, JValue},
    refs::Global,
};
use sui_core::UrlOpenerBackend;
use winit::platform::android::activity::AndroidApp;

/// Opens URLs on Android by starting an `ACTION_VIEW` activity, which hands
/// the URL to the browser or the app registered for its scheme.
///
/// Android framework objects must be used on the Java main thread, so each
/// URL is posted there without waiting for the activity to start.
pub(crate) struct AndroidUrlOpenerBackend {
    app: AndroidApp,
}

impl AndroidUrlOpenerBackend {
    pub(crate) fn new(app: AndroidApp) -> Self {
        Self { app }
    }
}

impl UrlOpenerBackend for AndroidUrlOpenerBackend {
    fn open(&mut self, url: &str) -> bool {
        let app = self.app.clone();
        let url = url.to_string();
        self.app.run_on_java_main_thread(Box::new(move || {
            let _ = start_view_activity(&app, &url);
        }));
        true
    }
}

fn start_view_activity(app: &AndroidApp, url: &str) -> jni::errors::Result<()> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let raw_activity = app.activity_as_ptr() as jni::sys::jobject;
    vm.attach_current_thread(|env| {
        let result = view_url(env, &raw_activity, url);
        if result.is_err() {
            // No app handles the URL (ActivityNotFoundException); leave no
            // Java exception pending on the main thread.
            env.exception_clear();
        }
        result
    })
}

fn view_url(
    env: &mut jni::Env<'_>,
    raw_activity: &jni::sys::jobject,
    url: &str,
) -> jni::errors::Result<()> {
    let activity = unsafe { env.as_cast_raw::<Global<JObject>>(raw_activity)? };
    let url: JObject = env.new_string(url)?.into();
    let uri = env
        .call_static_method(
            jni::jni_str!("android/net/Uri"),
            jni::jni_str!("parse"),
            jni::jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
            &[JValue::Object(&url)],
        )?
        .l()?;
    let action: JObject = env.new_string("android.intent.action.VIEW")?.into();
    let intent = env.new_object(
        jni::jni_str!("android/content/Intent"),
        jni::jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
        &[JValue::Object(&action), JValue::Object(&uri)],
    )?;
    env.call_method(
        &activity,
        jni::jni_str!("startActivity"),
        jni::jni_sig!("(Landroid/content/Intent;)V"),
        &[JValue::Object(&intent)],
    )?;
    Ok(())
}
