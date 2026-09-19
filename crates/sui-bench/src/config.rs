use serde::{Deserialize, Serialize};

pub const FIXTURES: &[&str] = &[
    "controls-grid",
    "nested-flex",
    "scrollbar-thresholds",
    "keyed-collection",
    "virtual-collection",
    "streaming-document",
    "overlays-and-dialogs",
    "scene-properties",
    "application-shell",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub preset: String,
    pub mode: String,
    pub builder: String,
    pub redraw: String,
    pub fixture: String,
    pub size: usize,
    pub depth: usize,
    pub steps: usize,
    pub warmup: usize,
    pub trials: usize,
    pub cold_processes: usize,
    pub seed: u64,
    pub width: f32,
    pub height: f32,
    pub dpr: f64,
    pub fraction: f64,
    pub mutation: String,
    pub diagnostics: bool,
    pub allow_software: bool,
    pub vsync: bool,
    pub rate_hz: f64,
    pub timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            preset: "smoke".into(),
            mode: "runtime".into(),
            builder: "runtime".into(),
            redraw: "natural".into(),
            fixture: "all".into(),
            size: 64,
            depth: 4,
            steps: 12,
            warmup: 3,
            trials: 1,
            cold_processes: 2,
            seed: 1,
            width: 960.0,
            height: 640.0,
            dpr: 1.0,
            fraction: 0.01,
            mutation: "default".into(),
            diagnostics: false,
            allow_software: false,
            vsync: true,
            rate_hz: 0.0,
            timeout_secs: 60,
        }
    }
}

pub fn parse(args: &[String]) -> Result<(Config, String), String> {
    let mut c = Config::default();
    // Preset defaults are applied before explicit overrides, independent of order.
    for pair in args.windows(2) {
        if pair[0] == "--preset" {
            c.preset = pair[1].clone();
            match c.preset.as_str() {
                "smoke" => (),
                "startup" => {
                    c.size = 1_000;
                    c.cold_processes = 30;
                }
                "updates" => {
                    c.size = 1_000;
                    c.trials = 5;
                    c.steps = 200;
                    c.warmup = 32;
                }
                "stress" => {
                    c.size = 10_000;
                    c.trials = 5;
                    c.steps = 200;
                    c.warmup = 32;
                }
                other => return Err(format!("unknown preset {other}")),
            }
        }
    }
    let mut output = "target/widget-bench/run".to_string();
    let mut i = 0;
    while i < args.len() {
        let key = &args[i];
        match key.as_str() {
            "--diagnostics" => c.diagnostics = true,
            "--allow-software" => c.allow_software = true,
            _ => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| format!("missing value for {key}"))?;
                macro_rules! number {
                    ($field:ident) => {
                        c.$field = v.parse().map_err(|_| format!("invalid value for {key}"))?
                    };
                }
                match key.as_str() {
                    "--preset" => (),
                    "--mode" => c.mode = v.clone(),
                    "--builder" => c.builder = v.clone(),
                    "--redraw" => c.redraw = v.clone(),
                    "--fixture" => c.fixture = v.clone(),
                    "--output" => output = v.clone(),
                    "--mutation" => c.mutation = v.clone(),
                    "--size" => number!(size),
                    "--depth" => number!(depth),
                    "--steps" => number!(steps),
                    "--warmup" => number!(warmup),
                    "--trials" => number!(trials),
                    "--cold-processes" => number!(cold_processes),
                    "--seed" => number!(seed),
                    "--width" => number!(width),
                    "--height" => number!(height),
                    "--dpr" => number!(dpr),
                    "--change-fraction" => number!(fraction),
                    "--rate-hz" => number!(rate_hz),
                    "--timeout-secs" => number!(timeout_secs),
                    "--vsync" if v == "on" || v == "off" => c.vsync = v == "on",
                    _ => return Err(format!("unknown option or value: {key} {v}")),
                }
            }
        }
        i += 1;
    }
    c.validate()?;
    Ok((c, output))
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !["runtime", "public"].contains(&self.builder.as_str()) {
            return Err(format!("unknown builder {}", self.builder));
        }
        if !["natural", "requested"].contains(&self.redraw.as_str()) {
            return Err(format!("unknown redraw mode {}", self.redraw));
        }
        if !["construct", "runtime", "offscreen", "desktop"].contains(&self.mode.as_str()) {
            return Err(format!("unknown mode {}", self.mode));
        }
        if self.fixture != "all" && !FIXTURES.contains(&self.fixture.as_str()) {
            return Err(format!("unknown fixture {}", self.fixture));
        }
        if ![
            "default",
            "rebuild",
            "local",
            "distributed",
            "all",
            "resize",
            "paint",
            "reorder",
            "scroll",
            "idle",
        ]
        .contains(&self.mutation.as_str())
        {
            return Err(format!("unknown mutation {}", self.mutation));
        }
        if self.size == 0
            || self.size > 100_000
            || self.depth == 0
            || self.depth > 64
            || self.steps == 0
            || self.steps > 100_000
            || self.trials == 0
            || self.trials > 1_000
            || self.cold_processes == 0
            || self.cold_processes > 1_000
            || self.timeout_secs == 0
            || !self.width.is_finite()
            || !(1.0..=8192.0).contains(&self.width)
            || !self.height.is_finite()
            || !(1.0..=8192.0).contains(&self.height)
            || !self.dpr.is_finite()
            || !(0.5..=4.0).contains(&self.dpr)
            || !self.fraction.is_finite()
            || !(0.0..=1.0).contains(&self.fraction)
            || !self.rate_hz.is_finite()
            || !(0.0..=10_000.0).contains(&self.rate_hz)
        {
            return Err("configuration outside supported finite bounds".into());
        }
        if self.mode == "desktop" && self.preset != "startup" {
            return Err("desktop currently measures startup; select --preset startup".into());
        }
        if self.mutation == "rebuild" && self.fixture != "controls-grid" {
            return Err("rebuild requires the controls-grid fixture".into());
        }
        let collection = matches!(
            self.fixture.as_str(),
            "keyed-collection" | "virtual-collection"
        );
        if self.mutation == "reorder" && !collection {
            return Err("reorder requires a keyed-collection or virtual-collection fixture".into());
        }
        if self.mutation == "scroll"
            && !collection
            && !matches!(
                self.fixture.as_str(),
                "scrollbar-thresholds" | "streaming-document" | "application-shell"
            )
        {
            return Err("scroll requires a scrollable fixture".into());
        }
        if matches!(self.mutation.as_str(), "distributed" | "all")
            && matches!(
                self.fixture.as_str(),
                "streaming-document" | "overlays-and-dialogs" | "all"
            )
        {
            return Err("distributed/all mutations require an explicit row/control fixture".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_overrides_and_invalid_values() {
        let args = ["--size", "23", "--preset", "updates"].map(String::from);
        assert_eq!(parse(&args).unwrap().0.size, 23);
        for args in [
            vec!["--size", "0"],
            vec!["--dpr", "NaN"],
            vec!["--mode", "fallback"],
            vec!["--size"],
        ] {
            assert!(parse(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }
}
