use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sui-bench-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sui-bench"))
}

#[test]
fn subprocess_artifacts_compare_and_overwrite_protection() {
    let temp = Temp::new();
    let output = temp.0.join("run");
    let result = binary()
        .args([
            "run",
            "--fixture",
            "controls-grid",
            "--size",
            "4",
            "--steps",
            "2",
            "--warmup",
            "0",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "complete");
    assert!(
        manifest["source_fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("fnv1a64:")
    );
    let record: Value = serde_json::from_str(
        fs::read_to_string(output.join("samples.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record["samples"].as_array().unwrap().len(), 3);
    assert!(record["parent_spawn_to_exit_us"].as_f64().unwrap() > 0.0);
    assert!(
        binary()
            .arg("compare")
            .arg(&output)
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    let before = fs::read(output.join("manifest.json")).unwrap();
    assert!(
        !binary()
            .args(["run", "--output"])
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(before, fs::read(output.join("manifest.json")).unwrap());
}

#[test]
#[cfg(target_os = "linux")]
fn desktop_without_display_is_an_explicit_incomplete_run() {
    let temp = Temp::new();
    let output = temp.0.join("desktop");
    let result = binary()
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .args([
            "run",
            "--preset",
            "startup",
            "--mode",
            "desktop",
            "--fixture",
            "controls-grid",
            "--size",
            "2",
            "--cold-processes",
            "1",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let manifest: Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "incomplete");
    let record: Value = serde_json::from_str(
        fs::read_to_string(output.join("samples.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record["status"], "unsupported");
    assert!(record["samples"].as_array().unwrap().is_empty());
}

#[test]
#[cfg(feature = "diagnostics")]
fn diagnostic_child_collects_widget_hotspots_before_the_first_frame() {
    let temp = Temp::new();
    let output = temp.0.join("diagnostics");
    let result = binary()
        .args([
            "run",
            "--fixture",
            "controls-grid",
            "--size",
            "4",
            "--steps",
            "1",
            "--warmup",
            "0",
            "--diagnostics",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let record: Value = serde_json::from_str(
        fs::read_to_string(output.join("samples.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert!(
        !record["samples"][0]["widget_details"][0]["hotspots"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        record["samples"][0]["work"]["constructed"]
            .as_u64()
            .unwrap()
            > 0
    );
}
