use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn files(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("rs" | "wgsl" | "ttf" | "toml")
        ) {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut inputs = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    for entry in fs::read_dir(root.join("crates")).unwrap().flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        inputs.push(dir.join("Cargo.toml"));
        for child in ["src", "assets", "benches"] {
            files(&dir.join(child), &mut inputs);
        }
    }
    inputs.sort();
    let mut hash = 0xcbf29ce484222325u64;
    for path in inputs {
        println!("cargo:rerun-if-changed={}", path.display());
        if let Ok(bytes) = fs::read(&path) {
            for byte in path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .bytes()
                .chain(bytes)
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    for git_file in [".git/HEAD", ".git/index", ".git/refs/heads/main"] {
        println!("cargo:rerun-if-changed={}", root.join(git_file).display());
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into())
    };
    println!(
        "cargo:rustc-env=SUI_BENCH_COMMIT={}",
        git(&["rev-parse", "HEAD"])
    );
    println!(
        "cargo:rustc-env=SUI_BENCH_DIRTY={}",
        !git(&["status", "--porcelain"]).is_empty()
    );
    println!("cargo:rustc-env=SUI_BENCH_SOURCE=fnv1a64:{hash:016x}");
    let rustc = Command::new(env::var_os("RUSTC").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    println!(
        "cargo:rustc-env=SUI_BENCH_RUSTC={}",
        String::from_utf8_lossy(&rustc.stdout).trim()
    );
    println!(
        "cargo:rustc-env=SUI_BENCH_OPT={}",
        env::var("OPT_LEVEL").unwrap()
    );
    println!(
        "cargo:rustc-env=SUI_BENCH_TARGET={}",
        env::var("TARGET").unwrap()
    );
}
