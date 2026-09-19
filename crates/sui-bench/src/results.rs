use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use sui_runtime::LayoutWorkSnapshot;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sample {
    pub fixture: String,
    pub phase: String,
    pub trial: usize,
    pub operation: usize,
    pub elapsed_us: f64,
    pub mutation_us: f64,
    pub service_us: f64,
    pub queue_age_us: f64,
    pub changed_items: usize,
    pub offered_updates: usize,
    pub coalesced_updates: usize,
    pub frames: usize,
    pub mounted_widgets: usize,
    pub timings: BTreeMap<String, f64>,
    pub work: Value,
    pub text_cache: Value,
    pub renderer: Value,
    pub widget_details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub status: String,
    pub error: Option<String>,
    pub backend: Value,
    pub samples: Vec<Sample>,
    pub teardown_us: Option<f64>,
    pub final_rss_kib: Option<u64>,
    pub teardown_work: Value,
    pub warmup_work: Value,
}

impl Trial {
    pub fn failure(status: &str, message: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            error: Some(message.into()),
            backend: Value::Null,
            samples: Vec::new(),
            teardown_us: None,
            final_rss_kib: rss_kib(),
            teardown_work: Value::Null,
            warmup_work: Value::Null,
        }
    }
}

pub fn rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

pub fn work_json(work: Option<LayoutWorkSnapshot>) -> Value {
    let Some(w) = work else {
        return Value::Null;
    };
    json!({"constructed":w.constructed,"dropped":w.dropped,
        "measure_requests":w.measure_requests,"measure_executions":w.measure_executions,"size_measure_executions":w.size_measure_executions,
        "measure_cache_hits":w.measure_cache_hits,"forced_measures":w.forced_measures,
        "constraint_changes":w.constraint_changes,"first_measures":w.first_measures,"paint_executions":w.paint_executions,"semantics_executions":w.semantics_executions,"paint_cache_hits":w.paint_cache_hits,"semantics_cache_hits":w.semantics_cache_hits,
        "probe_requests":w.probe_requests,"probe_cache_hits":w.probe_cache_hits,"shared_probe_cache_hits":w.shared_probe_cache_hits,
        "intrinsic_executions":w.intrinsic_executions,"intrinsic_cache_hits":w.intrinsic_cache_hits,
        "intrinsic_horizontal":w.intrinsic_horizontal,"intrinsic_vertical":w.intrinsic_vertical,
        "arrange_requests":w.arrange_requests,"arrange_executions":w.arrange_executions,
        "arrange_cache_hits":w.arrange_cache_hits,"translations":w.translations,
        "layout_passes":w.layout_passes,"gutter_iterations":w.gutter_iterations,"measure_us":w.measure_us,"arrange_us":w.arrange_us,"graph_us":w.graph_us})
}

pub fn percentile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let index = ((q * values.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    Some(values[index])
}

pub fn summary(trials: &[Trial]) -> Value {
    let mut groups: BTreeMap<String, Vec<&Sample>> = BTreeMap::new();
    for trial in trials.iter().filter(|trial| trial.status == "ok") {
        for sample in &trial.samples {
            groups
                .entry(format!("{}/{}", sample.fixture, sample.phase))
                .or_default()
                .push(sample);
        }
    }
    let mut output = serde_json::Map::new();
    for (name, samples) in groups {
        let values: Vec<_> = samples.iter().map(|s| s.elapsed_us).collect();
        let startup = samples[0].phase != "update";
        let mut independent: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for sample in &samples {
            independent
                .entry(sample.trial)
                .or_default()
                .push(sample.elapsed_us);
        }
        let per_trial: Vec<_> = independent.iter().map(|(trial,v)| json!({"trial":trial,"samples":v.len(),"median_us":percentile(v,0.5),"p95_us":if v.len()>=100{percentile(v,0.95)}else{None}})).collect();
        let trial_medians: Vec<_> = independent
            .values()
            .filter_map(|v| percentile(v, 0.5))
            .collect();
        output.insert(name,json!({
            "samples":values.len(), "independent_trials":independent.len(),
            "median_us":percentile(&values,0.5), "p90_us":if values.len()>=20{percentile(&values,0.9)}else{None},
            "p95_us":if values.len()>=100{percentile(&values,0.95)}else{None},
            "p99_us":if !startup && values.len()>=5000{percentile(&values,0.99)}else{None},
            "max_us":percentile(&values,1.0), "trial_median_min_us":percentile(&trial_medians,0.0),
            "trial_median_max_us":percentile(&trial_medians,1.0), "bootstrap_trial_median_95ci_us":bootstrap_interval(&trial_medians), "trials":per_trial,
            "offered_updates":samples.iter().map(|s|s.offered_updates).sum::<usize>(),
            "coalesced_updates":samples.iter().map(|s|s.coalesced_updates).sum::<usize>(),
        }));
    }
    json!({"schema_version":1,"ok_trials":trials.iter().filter(|t|t.status=="ok").count(),
        "failed_trials":trials.iter().filter(|t|t.status!="ok").count(),"groups":output})
}

fn bootstrap_interval(values: &[f64]) -> Option<[f64; 2]> {
    if values.len() < 5 {
        return None;
    }
    let mut state = 0x41c64e6du64;
    let mut medians = Vec::with_capacity(2000);
    for _ in 0..2000 {
        let sample: Vec<_> = (0..values.len())
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                values[(state >> 32) as usize % values.len()]
            })
            .collect();
        medians.push(percentile(&sample, 0.5)?);
    }
    Some([percentile(&medians, 0.025)?, percentile(&medians, 0.975)?])
}

pub fn print_summary(summary: &Value) {
    println!(
        "{:<42} {:>9} {:>13} {:>13}",
        "workload", "samples", "median us", "p95 us"
    );
    if let Some(groups) = summary["groups"].as_object() {
        for (name, v) in groups {
            println!(
                "{name:<42} {:>9} {:>13.3} {:>13}",
                v["samples"],
                v["median_us"].as_f64().unwrap_or_default(),
                v["p95_us"]
                    .as_f64()
                    .map(|n| format!("{n:.3}"))
                    .unwrap_or_else(|| "n/a".into())
            );
        }
    }
    println!(
        "{} successful trials; {} failed/unsupported trials",
        summary["ok_trials"], summary["failed_trials"]
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_rank_percentiles_and_insufficient_tails() {
        assert_eq!(percentile(&[], 0.95), None);
        assert_eq!(percentile(&[4.0, 1.0, 3.0, 2.0], 0.5), Some(2.0));
        let trial = Trial {
            status: "ok".into(),
            error: None,
            backend: Value::Null,
            samples: vec![Sample {
                fixture: "a".into(),
                phase: "startup".into(),
                elapsed_us: 5.0,
                ..Default::default()
            }],
            teardown_us: None,
            final_rss_kib: None,
            teardown_work: Value::Null,
            warmup_work: Value::Null,
        };
        let result = summary(&[trial]);
        assert!(result["groups"]["a/startup"]["p95_us"].is_null());
        assert_eq!(result["groups"]["a/startup"]["median_us"], 5.0);
    }
}
