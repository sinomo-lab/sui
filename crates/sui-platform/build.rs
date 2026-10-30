//! Compiles the Java helpers that Android builds load at runtime into DEX.

use std::{path::PathBuf, process::ExitStatus};

const JAVA_SOURCES: &[&str] = &["java/net/sinomo/ui/platform/FilePicker.java"];

fn main() {
    println!("cargo::rerun-if-changed=java");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        build_android_dex();
    }
}

fn build_android_dex() {
    let android_jar = android_build::android_jar(None).unwrap_or_else(|| {
        panic!("building for Android needs the Android SDK; set ANDROID_HOME to its directory")
    });
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let classes_dir = out_dir.join("java-classes");
    let dex_dir = out_dir.join("java-dex");
    let _ = std::fs::remove_dir_all(&classes_dir);
    std::fs::create_dir_all(&classes_dir).expect("create the Java classes directory");
    std::fs::create_dir_all(&dex_dir).expect("create the DEX directory");

    let mut javac = android_build::JavaBuild::new();
    javac
        .files(JAVA_SOURCES)
        .class_path(&android_jar)
        .classes_out_dir(&classes_dir)
        .java_source_version(8)
        .java_target_version(8)
        .debug_info(android_build::DebugInfo {
            line_numbers: !release,
            source_files: !release,
            variables: !release,
        });
    check("javac", javac.compile());

    let mut d8 = android_build::Dexer::new();
    d8.android_jar(&android_jar)
        .class_path(&classes_dir)
        .collect_classes(&classes_dir)
        .expect("collect the compiled Java classes")
        .release(release)
        .android_min_api(26)
        .out_dir(&dex_dir);
    check("d8", d8.run());

    let dex = dex_dir.join("classes.dex");
    assert!(dex.exists(), "d8 did not write {}", dex.display());
    println!("cargo::rustc-env=SUI_ANDROID_DEX={}", dex.display());
}

fn check(tool: &str, status: std::io::Result<ExitStatus>) {
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("{tool} failed with {status}"),
        Err(error) => panic!(
            "could not run {tool}: {error}; building for Android needs a JDK \
             (set JAVA_HOME) and the Android SDK build tools (set ANDROID_HOME)"
        ),
    }
}
