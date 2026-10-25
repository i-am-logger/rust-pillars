//! What each run of a `--keep-temp` suite left behind, kept small and the
//! run's directory removed after (plugin-eval.sh `--keep-state`).
//!
//! The runner deletes every run's directory -- the working copy the graders
//! read and the full trace -- and has no command graders (docs: "There are
//! no custom-code graders"), so a regex grader's verdict keeps no evidence.
//! Under `--keep-temp` it keeps them all, about 1.4 GB per run (measured),
//! until something removes them. This keeps, per run listed in the result:
//! the full trace, the working copy's changes against a fresh scaffold of
//! its case (build output left out), and the seconds from the run's start to
//! its first trace event; then it removes that run's directory -- only a
//! directory the result names, under the run root, named `e-*`, holding
//! `out/trace.jsonl` and `home/cwd`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde_json::Value;

/// The run directory a trace path belongs to, if it is one this may remove.
fn run_dir(trace: &Path, run_root: &Path) -> Option<PathBuf> {
    if trace.file_name()? != "trace.jsonl" || trace.parent()?.file_name()? != "out" {
        return None;
    }
    let dir = trace.parent()?.parent()?;
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?.canonicalize().ok()?;
    let ok = name.starts_with("e-")
        && name.len() > 2
        && parent == run_root.canonicalize().ok()?
        && dir.join("home/cwd").is_dir();
    ok.then(|| dir.to_path_buf())
}

/// Seconds from `startedAt` to the first timestamped event of the trace.
fn start_delay(started: &str, trace: &str) -> Option<f64> {
    let t0 = timestamp(started)?;
    trace.lines().find_map(|l| {
        let v: Value = serde_json::from_str(l).ok()?;
        Some(timestamp(v["timestamp"].as_str()?)? - t0)
    })
}

/// An RFC 3339 UTC timestamp (`2026-10-25T13:54:09.745Z`) as seconds since
/// the epoch: enough for differences within one run.
fn timestamp(s: &str) -> Option<f64> {
    let (date, time) = s.trim_end_matches('Z').split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':');
    let (h, mi, sec) = (
        t.next()?.parse::<f64>().ok()?,
        t.next()?.parse::<f64>().ok()?,
        t.next()?.parse::<f64>().ok()?,
    );
    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days as f64 * 86_400.0 + h * 3600.0 + mi * 60.0 + sec)
}

/// A fresh working copy of `case` from the export, for the diff.
fn fresh_start(export: &Path, case: &str, scratch: &Path) -> Option<PathBuf> {
    let script = export.join("evals").join(case).join("scaffold.sh");
    if !script.is_file() {
        return None;
    }
    let cwd = scratch.join("home/cwd");
    fs::create_dir_all(&cwd).ok()?;
    Command::new("sh")
        .arg(&script)
        .current_dir(&cwd)
        .env("HOME", scratch.join("home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .ok()?
        .success()
        .then_some(cwd)
}

/// `--harvest <result.json> <state dir> <export root>`, runs under
/// `run_root` (`$PE_RUN_ROOT`, default /private/tmp).
pub fn run_harvest(result: &Path, state: &Path, export: &Path, run_root: &Path) -> Result<()> {
    let doc: Value = serde_json::from_str(
        &fs::read_to_string(result).with_context(|| format!("reading {}", result.display()))?,
    )?;
    fs::create_dir_all(state)?;
    let mut table = String::from("case\tarm\trun\tdurationSeconds\tstartDelaySeconds\terror\n");
    let (mut kept, mut removed) = (0, 0);
    for case in doc["cases"].as_array().into_iter().flatten() {
        let name = case["name"].as_str().unwrap_or("case");
        for (arm, runs) in case["arms"].as_object().into_iter().flatten() {
            for (i, run) in runs.as_array().into_iter().flatten().enumerate() {
                let Some(trace) = run["tracePath"].as_str().map(PathBuf::from) else {
                    continue;
                };
                let Some(dir) = run_dir(&trace, run_root) else {
                    eprintln!(
                        "harvest: {} is not a run directory to remove; left alone",
                        trace.display()
                    );
                    continue;
                };
                let dest = state.join(name).join(format!("{arm}-{i}"));
                fs::create_dir_all(&dest)?;
                let text = fs::read_to_string(&trace).unwrap_or_default();
                fs::write(dest.join("trace.jsonl"), &text)?;
                let scratch = dest.join(".start");
                if let Some(start) = fresh_start(export, name, &scratch) {
                    let diff = Command::new("diff")
                        .args(["-ruN", "-x", "target", "-x", ".devenv.env"])
                        .arg(&start)
                        .arg(dir.join("home/cwd"))
                        .output()?;
                    fs::write(dest.join("changes.diff"), &diff.stdout)?;
                }
                if scratch.exists() {
                    fs::remove_dir_all(&scratch)?;
                }
                let delay = run["startedAt"]
                    .as_str()
                    .and_then(|s| start_delay(s, &text))
                    .map_or("-".to_string(), |d| format!("{d:.0}"));
                writeln!(
                    table,
                    "{name}\t{arm}\t{i}\t{}\t{delay}\t{}",
                    run["durationSeconds"],
                    run["error"].as_str().unwrap_or("")
                )?;
                kept += 1;
                fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
                removed += 1;
            }
        }
    }
    fs::write(state.join("runs.tsv"), &table)?;
    println!(
        "harvest: kept {kept} run(s) in {}, removed {removed} run directory(ies)",
        state.display()
    );
    print!("{table}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("harvest-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_timestamp_difference_is_in_seconds() {
        let a = timestamp("2026-10-25T13:54:09.745Z").unwrap();
        let b = timestamp("2026-10-25T14:08:04.745Z").unwrap();
        assert!((b - a - 835.0).abs() < 1e-6);
        assert!(
            timestamp("2026-03-01T00:00:00Z").unwrap() - timestamp("2026-02-28T00:00:00Z").unwrap()
                == 86_400.0
        );
    }

    #[test]
    fn a_listed_run_is_kept_small_and_its_directory_removed() {
        let root = tmp("root");
        let runs = root.join("runs");
        let run = runs.join("e-abc123");
        fs::create_dir_all(run.join("out")).unwrap();
        fs::create_dir_all(run.join("home/cwd/target")).unwrap();
        fs::write(run.join("home/cwd/a.txt"), "after\n").unwrap();
        fs::write(run.join("home/cwd/target/big"), "build output").unwrap();
        let trace = run.join("out/trace.jsonl");
        fs::write(
            &trace,
            "{\"type\":\"system\"}\n{\"type\":\"assistant\",\"timestamp\":\"2026-10-25T10:00:07.000Z\"}\n",
        )
        .unwrap();
        // An export whose case c scaffolds a.txt as "before".
        let export = root.join("export");
        fs::create_dir_all(export.join("evals/c")).unwrap();
        fs::write(export.join("evals/c/scaffold.sh"), "echo before > a.txt\n").unwrap();
        // A path that is not a run directory: never removed.
        let other = root.join("elsewhere/out");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("trace.jsonl"), "").unwrap();
        let result = root.join("result.json");
        let doc = serde_json::json!({"cases": [{"name": "c", "arms": {
            "with": [{"tracePath": trace, "startedAt": "2026-10-25T10:00:00.000Z", "durationSeconds": 9, "error": null}],
            "without": [{"tracePath": other.join("trace.jsonl"), "startedAt": "2026-10-25T10:00:00.000Z", "durationSeconds": 1, "error": null}]
        }}]});
        fs::write(&result, doc.to_string()).unwrap();
        let state = root.join("state");
        run_harvest(&result, &state, &export, &runs).unwrap();
        assert!(!run.exists(), "the listed run directory is removed");
        assert!(other.exists(), "a directory that is no run directory stays");
        let diff = fs::read_to_string(state.join("c/with-0/changes.diff")).unwrap();
        assert!(
            diff.contains("-before") && diff.contains("+after"),
            "{diff}"
        );
        assert!(
            !diff.contains("build output"),
            "target/ is left out: {diff}"
        );
        assert!(state.join("c/with-0/trace.jsonl").is_file());
        let tsv = fs::read_to_string(state.join("runs.tsv")).unwrap();
        assert!(tsv.contains("c\twith\t0\t9\t7\t"), "{tsv}");
        fs::remove_dir_all(&root).unwrap();
    }
}
