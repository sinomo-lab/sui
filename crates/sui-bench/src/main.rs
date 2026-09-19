#![forbid(unsafe_code)]
mod config;
#[cfg(feature = "desktop")]
mod desktop;
mod fixtures;
mod results;
mod runner;

use config::{Config, FIXTURES};
use results::Trial;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn main() {
    let entered = Instant::now();
    if let Err(error) = dispatch(entered) {
        eprintln!("sui-bench: {error}");
        std::process::exit(1);
    }
}

fn dispatch(entered: Instant) -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("list")=>{for fixture in FIXTURES{println!("{fixture}");}Ok(())}
        Some("run")=>{let(c,out)=config::parse(&args[1..])?;run(c,Path::new(&out))}
        Some("compare") if args.len()==3=>compare(Path::new(&args[1]),Path::new(&args[2])),
        Some("__sample") if args.len()==4=>{
            let c:Config=serde_json::from_str(&args[1]).map_err(|e|e.to_string())?;
            let trial=args[2].parse().map_err(|_|"invalid trial")?;
            let result=runner::run_at(&c,trial,entered);
            write_json(Path::new(&args[3]),&result)
        }
        _=>Err("usage: sui-bench list | run [--preset smoke|startup|updates|stress] [--fixture NAME|all] [--mode construct|runtime|offscreen|desktop] [--builder runtime|public] [--redraw natural|requested] [--size N] [--depth N] [--trials N] [--steps N] [--warmup N] [--cold-processes N] [--mutation default|local|distributed|all|resize|paint|reorder|scroll|rebuild|idle] [--change-fraction F] [--rate-hz HZ] [--width PX] [--height PX] [--dpr SCALE] [--seed N] [--diagnostics] [--allow-software] [--vsync on|off] [--timeout-secs N] [--output DIR] | compare BEFORE AFTER".into()),
    }
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn fingerprint(bytes: &[u8]) -> String {
    // Reproducibility identifier, not a security/integrity guarantee.
    let mut hash = 0xcbf29ce484222325u64;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn environment() -> Value {
    let cpu = fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("model name"))
            .map(str::to_owned)
    });
    let font_list = command_text("fc-list", &["--format", "%{file}\n"]);
    let fallback_fingerprint = font_list.map(|list| {
        let mut fonts: Vec<_> = list.lines().collect();
        fonts.sort_unstable();
        fonts.dedup();
        let mut bytes = Vec::new();
        for name in fonts {
            bytes.extend_from_slice(name.as_bytes());
            if let Ok(data) = fs::read(name) {
                bytes.extend_from_slice(fingerprint(&data).as_bytes());
            }
        }
        fingerprint(&bytes)
    });
    json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"cpu":cpu,
        "rustc":env!("SUI_BENCH_RUSTC"),"opt_level":env!("SUI_BENCH_OPT"),"target":env!("SUI_BENCH_TARGET"),"profile":if cfg!(debug_assertions){"debug"}else{"release"},
        "primary_font":fingerprint(sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT),"fallback_fonts":fallback_fingerprint,
        "features":{"diagnostics":cfg!(feature="diagnostics"),"gpu":cfg!(feature="gpu"),"desktop":cfg!(feature="desktop"),"public_api":cfg!(feature="public-api")},
        "display":std::env::var("DISPLAY").ok(),"wayland_display":std::env::var("WAYLAND_DISPLAY").ok(),
        "power_governor":fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok().map(|s|s.trim().to_owned())})
}

fn run(config: Config, out: &Path) -> Result<(), String> {
    if out.exists()
        && fs::read_dir(out)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(format!(
            "refusing to overwrite nonempty output directory {}",
            out.display()
        ));
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut manifest = json!({"schema_version":1,"fixture_version":2,"status":"running","config":config,
        "environment":environment(),"commit":env!("SUI_BENCH_COMMIT"),"source_fingerprint":env!("SUI_BENCH_SOURCE"),"built_dirty":env!("SUI_BENCH_DIRTY"),
        "invocation_commit":command_text("git",&["rev-parse","HEAD"]),"worktree":command_text("git",&["status","--porcelain"]),"binary":fingerprint(&fs::read(&exe).map_err(|e|e.to_string())?),
        "lockfile":fs::read("Cargo.lock").ok().map(|b|fingerprint(&b)),"startup_regime":"fresh child process, OS/driver caches uncontrolled",
        "allocation_metrics":null,"allocation_metrics_reason":"use an external allocator profiler; process RSS is reported separately"});
    write_json(&out.join("manifest.json"), &manifest)?;
    let fixtures: Vec<_> = if config.fixture == "all" {
        FIXTURES.iter().map(|s| s.to_string()).collect()
    } else {
        vec![config.fixture.clone()]
    };
    let repeats = if config.preset == "startup" || config.mode == "construct" {
        config.cold_processes
    } else {
        config.trials
    };
    let mut trials = Vec::new();
    let mut raw = fs::File::create(out.join("samples.jsonl")).map_err(|e| e.to_string())?;
    for fixture in fixtures {
        for index in 0..repeats {
            let mut c = config.clone();
            c.fixture = fixture.clone();
            let stem = format!("{fixture}-{index:03}");
            let result_path = out.join(format!("{stem}.json"));
            let log =
                fs::File::create(out.join(format!("{stem}.log"))).map_err(|e| e.to_string())?;
            let mut command = Command::new(&exe);
            command
                .args([
                    "__sample",
                    &serde_json::to_string(&c).map_err(|e| e.to_string())?,
                    &index.to_string(),
                ])
                .arg(&result_path)
                .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(log));
            // Never inherit profiling state accidentally; set it before child statics initialize.
            command
                .env_remove("SUI_PROFILE_WIDGET_TIMINGS")
                .env_remove("SUI_PROFILE_TEXT_TIMINGS");
            if c.diagnostics {
                command
                    .env("SUI_PROFILE_WIDGET_TIMINGS", "1")
                    .env("SUI_PROFILE_TEXT_TIMINGS", "1");
            }
            let started = Instant::now();
            let mut child = command.spawn().map_err(|e| e.to_string())?;
            let status = loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    break Some(status);
                }
                if started.elapsed() > Duration::from_secs(c.timeout_secs) {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            let parent_us = started.elapsed().as_secs_f64() * 1e6;
            let result = match status {
                None => Trial::failure(
                    "timeout",
                    format!("child exceeded {}s; see {stem}.log", c.timeout_secs),
                ),
                Some(status) if !status.success() => {
                    Trial::failure("error", format!("child exited {status}; see {stem}.log"))
                }
                Some(_) => match fs::read(&result_path)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Trial>(&b).ok())
                {
                    Some(result) => result,
                    None => Trial::failure("error", "missing or invalid child result"),
                },
            };
            let mut record = serde_json::to_value(&result).map_err(|e| e.to_string())?;
            record["fixture"] = json!(fixture);
            record["trial"] = json!(index);
            record["parent_spawn_to_exit_us"] = json!(parent_us);
            writeln!(
                raw,
                "{}",
                serde_json::to_string(&record).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            if result.status != "ok" {
                eprintln!(
                    "{stem}: {}: {}",
                    result.status,
                    result.error.as_deref().unwrap_or("unknown error")
                );
            }
            trials.push(result);
        }
    }
    let summary = results::summary(&trials);
    write_json(&out.join("summary.json"), &summary)?;
    manifest["status"] = json!(if trials.iter().all(|t| t.status == "ok") {
        "complete"
    } else {
        "incomplete"
    });
    manifest["backends"] = json!(trials.iter().map(|t| &t.backend).collect::<Vec<_>>());
    write_json(&out.join("manifest.json"), &manifest)?;
    results::print_summary(&summary);
    println!("Artifacts: {}", out.display());
    if trials.iter().any(|t| t.status != "ok") {
        return Err("one or more trials failed or were unsupported; evidence retained".into());
    }
    Ok(())
}

fn read_value(dir: &Path, name: &str) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(dir.join(name)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn compatible(a: &Value, b: &Value) -> Result<(), String> {
    for key in [
        "schema_version",
        "fixture_version",
        "config",
        "environment",
        "backends",
    ] {
        if a[key] != b[key] {
            return Err(format!(
                "incompatible {key}; compare matching fixtures, profiles, diagnostics, and hardware"
            ));
        }
    }
    if a["status"] != "complete" || b["status"] != "complete" {
        return Err("cannot compare incomplete runs".into());
    }
    Ok(())
}

fn compare(a: &Path, b: &Path) -> Result<(), String> {
    compatible(
        &read_value(a, "manifest.json")?,
        &read_value(b, "manifest.json")?,
    )?;
    let before = read_value(a, "summary.json")?;
    let after = read_value(b, "summary.json")?;
    println!(
        "{:<42} {:>12} {:>12} {:>12}",
        "workload", "before us", "after us", "change %"
    );
    for (name, v) in before["groups"]
        .as_object()
        .ok_or("missing summary groups")?
    {
        let old = v["median_us"].as_f64().ok_or("missing median")?;
        let new = after["groups"][name]["median_us"]
            .as_f64()
            .ok_or("missing matching group")?;
        println!(
            "{name:<42} {old:>12.3} {new:>12.3} {:>12.2}",
            100.0 * (new - old) / old.max(f64::EPSILON)
        );
    }
    println!(
        "Descriptive comparison only; inspect trial distributions before declaring a regression."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_rejects_different_modes_and_incomplete_runs() {
        let a = json!({"schema_version":1,"fixture_version":1,"config":{"mode":"runtime"},"environment":{},"backends":[],"status":"complete"});
        assert!(compatible(&a, &a).is_ok());
        let mut b = a.clone();
        b["config"]["mode"] = json!("offscreen");
        assert!(compatible(&a, &b).is_err());
        b = a.clone();
        b["status"] = json!("incomplete");
        assert!(compatible(&a, &b).is_err());
    }
}
