//! The plugin's evals, scored by running the code.
//!
//! `claude plugin eval` grades a transcript; it has no command graders, so it
//! cannot tell whether the code it produced builds. This harness can: each
//! case in `evals/<case>/` is run headless (`claude -p`) in a fresh copy of
//! the base workspace plus the case's fixture, once with the plugin and once
//! without, and then every command in the case's `checks.toml` is run against
//! what was left on disk.
//!
//! Each copy is built by the case's `scaffold.sh` -- the script `claude plugin
//! eval --scaffold` runs too -- and lives outside this repository (under the
//! system temp dir), so a
//! run cannot reach the cases' checks with a relative path, and the arm with
//! the plugin gets a copy of the plugin alone (its manifests and skills), not
//! the repository. Checks reach the repository through `$EVAL_ROOT` and their
//! case through `$EVAL_CASE`, which no model run sees.
//!
//! Each arm runs `--runs` times (default 2), and the spread between an arm's
//! own runs is its A/A noise: a difference between the arms counts only when
//! it is larger than that spread. Anything else is reported as no result.
//!
//! Two modes run no model, and between them prove each case can grade:
//! `--control` runs every check against the untouched base, where a task's
//! own checks must fail (a check the base already passes grades nothing), and
//! `--solution` runs them against the base plus the case's reference
//! `solution/`, where every check must pass (a check nothing can pass grades
//! nothing either); a solution may carry `.solve.sh`, run in the copy first,
//! to regenerate what a tool would leave instead of committing it. Every
//! other run is a real model call.
//!
//! Run it from the devenv shell: `cargo run --manifest-path
//! eval-harness/Cargo.toml -- [--control] [--case NAME] [--runs N]
//! [--model M]` (the `eval:run` task).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};

mod merge;
mod plugin;
mod state;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Checks {
    check: Vec<Check>,
}

/// One executable grader.
#[derive(Debug, Deserialize)]
struct Check {
    name: String,
    /// A shell command run in the case's working copy.
    run: String,
    /// `pass` (exit 0) or `fail` (any other exit), after the model's run.
    expect: Expect,
    /// What it must do on the untouched base, before any model ran.
    on_base: Expect,
    /// Seconds before the check is killed and counted as failed.
    #[serde(default = "default_check_timeout")]
    timeout_s: u64,
}

fn default_check_timeout() -> u64 {
    1800
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Expect {
    Pass,
    Fail,
}

/// Which side of the comparison a run is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Arm {
    With,
    Without,
}

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Self::With => "with",
            Self::Without => "without",
        }
    }
}

/// What to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Every check against the untouched base: task checks must fail.
    Control,
    /// Every check against the base plus the case's reference `solution/`:
    /// all must pass, or the case could never be passed at all.
    Solution,
    /// The model, with and without the plugin.
    Model,
    /// The plugin's own structure (`plugin.rs`); no case, no model.
    Plugin,
    /// A `claude plugin eval` result rescored by `merge.rs`'s rules: every
    /// score they do not reproduce. No model.
    Rescore,
    /// A with-arm-only result beside a baseline's without arm (`merge.rs`).
    Merge,
    /// Each run's time and one grader's evidence (`merge.rs`).
    Evidence,
    /// What each run of a `--keep-temp` suite left, kept; its directory
    /// removed (`state.rs`).
    Harvest,
}

#[derive(Debug)]
struct Opts {
    mode: Mode,
    case: Option<String>,
    runs: usize,
    model: Option<String>,
    /// The result files of `--rescore` and `--merge`.
    files: Vec<PathBuf>,
}

fn opts() -> Result<Opts> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Opts {
        mode: Mode::Model,
        case: None,
        runs: 2,
        model: None,
        files: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--control" => o.mode = Mode::Control,
            "--solution" => o.mode = Mode::Solution,
            "--plugin" => o.mode = Mode::Plugin,
            "--case" => o.case = Some(val()?.clone()),
            "--runs" => o.runs = val()?.parse()?,
            "--model" => o.model = Some(val()?.clone()),
            "--rescore" => {
                o.mode = Mode::Rescore;
                o.files = vec![PathBuf::from(val()?)];
            }
            "--merge" => {
                o.mode = Mode::Merge;
                for _ in 0..3 {
                    o.files.push(PathBuf::from(val()?));
                }
            }
            "--evidence" => {
                o.mode = Mode::Evidence;
                for _ in 0..2 {
                    o.files.push(PathBuf::from(val()?));
                }
            }
            "--harvest" => {
                o.mode = Mode::Harvest;
                for _ in 0..3 {
                    o.files.push(PathBuf::from(val()?));
                }
            }
            other => bail!("unknown option `{other}`"),
        }
    }
    ensure!(
        o.runs >= 2,
        "at least 2 runs per arm: one run has no spread, and the spread is the A/A control"
    );
    Ok(o)
}

/// What `prompt.md`'s frontmatter sets for a run.
#[derive(Debug)]
struct Prompt {
    body: String,
    max_turns: u64,
    timeout: Duration,
}

fn prompt(case: &Path) -> Result<Prompt> {
    let text = fs::read_to_string(case.join("prompt.md")).context("reading prompt.md")?;
    let (front, body) = match text.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").unwrap_or(("", rest)),
        None => ("", text.as_str()),
    };
    let field = |key: &str, default: u64| -> u64 {
        front
            .lines()
            .find_map(|l| {
                l.trim()
                    .strip_prefix(key)?
                    .strip_prefix(':')?
                    .trim()
                    .parse()
                    .ok()
            })
            .unwrap_or(default)
    };
    Ok(Prompt {
        body: body.trim().to_string(),
        max_turns: field("max_turns", 40),
        timeout: Duration::from_secs(field("timeout_seconds", 900)),
    })
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Where working copies and the plugin copy go: outside the repository.
fn scratch() -> PathBuf {
    std::env::temp_dir().join("rust-code-eval")
}

/// A copy of the plugin alone, for the arm with the plugin.
fn plugin_copy(root: &Path) -> Result<PathBuf> {
    let dir = scratch().join("plugin");
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    for part in [".claude-plugin", "skills", "commands", "agents", "hooks"] {
        if root.join(part).is_dir() {
            copy_dir(&root.join(part), &dir.join(part))?;
        }
    }
    for file in ["LICENSE", "NOTICE.md"] {
        if root.join(file).is_file() {
            fs::copy(root.join(file), dir.join(file))?;
        }
    }
    Ok(dir)
}

/// A fresh working copy for one run, built by the case's own `scaffold.sh`
/// (the same script `claude plugin eval --scaffold` runs): the base
/// workspace, then the case's fixture.
fn fresh(case: &Path, slot: &str) -> Result<PathBuf> {
    let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("case");
    let work = scratch().join(name).join(slot);
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(&work)?;
    let status = Command::new("sh")
        .arg(case.join("scaffold.sh"))
        .current_dir(&work)
        .stdin(Stdio::null())
        .status()
        .context("running the case's scaffold.sh")?;
    ensure!(status.success(), "{name}: scaffold.sh failed");
    Ok(work)
}

/// Waits for a child up to `limit`; `None` if it was killed for running over.
fn wait(mut child: Child, limit: Duration) -> Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() > limit {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn run_check(root: &Path, case: &Path, work: &Path, c: &Check) -> Result<bool> {
    let child = Command::new("sh")
        .args(["-c", &c.run])
        .current_dir(work)
        .env("EVAL_ROOT", root)
        .env("EVAL_CASE", case)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("running check `{}`", c.name))?;
    Ok(wait(child, Duration::from_secs(c.timeout_s))?.is_some_and(|s| s.success()))
}

/// Every check on the untouched base must do what its `on_base` says.
fn control(root: &Path, case: &Path, checks: &Checks) -> Result<Vec<String>> {
    let work = fresh(case, "control")?;
    let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("case");
    let mut wrong = Vec::new();
    for c in &checks.check {
        let passed = run_check(root, case, &work, c)?;
        if passed != (c.on_base == Expect::Pass) {
            let did = if passed { "passes" } else { "fails" };
            wrong.push(format!(
                "{name}: `{}` {did} on the untouched base, expected {:?}",
                c.name, c.on_base
            ));
        }
    }
    Ok(wrong)
}

/// The base plus the case's reference solution must pass every check.
fn solution(root: &Path, case: &Path, checks: &Checks) -> Result<Vec<String>> {
    let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("case");
    let reference = case.join("solution");
    if !reference.is_dir() {
        return Ok(vec![format!(
            "{name}: no solution/ -- nothing shows its checks can be passed"
        )]);
    }
    let work = fresh(case, "solution")?;
    copy_dir(&reference, &work)?;
    // A solution whose honest answer is a tool's output (a measurement, a
    // generated file) regenerates it here rather than committing build
    // products: `.solve.sh` runs in the copy, and is gone before the checks.
    let script = work.join(".solve.sh");
    if script.exists() {
        let solve = Check {
            name: ".solve.sh".into(),
            run: "sh .solve.sh".into(),
            expect: Expect::Pass,
            on_base: Expect::Pass,
            timeout_s: default_check_timeout(),
        };
        let solved = run_check(root, case, &work, &solve)?;
        fs::remove_file(&script)?;
        if !solved {
            return Ok(vec![format!("{name}: solution/.solve.sh failed")]);
        }
    }
    let mut wrong = Vec::new();
    for c in &checks.check {
        if run_check(root, case, &work, c)? != (c.expect == Expect::Pass) {
            wrong.push(format!(
                "{name}: `{}` is not {:?} with the reference solution",
                c.name, c.expect
            ));
        }
    }
    Ok(wrong)
}

/// Runs the model once in a fresh working copy; returns the copy.
fn attempt(case: &Path, plugin: Option<&Path>, slot: &str, o: &Opts) -> Result<PathBuf> {
    let p = prompt(case)?;
    let work = fresh(case, slot)?;
    let transcript = fs::File::create(work.join(".eval-transcript.json"))?;
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(&p.body)
        .args([
            "--output-format",
            "json",
            "--permission-mode",
            "bypassPermissions",
        ])
        .args(["--max-turns", &p.max_turns.to_string()])
        .current_dir(&work)
        .stdin(Stdio::null())
        .stdout(transcript)
        .stderr(Stdio::null());
    if let Some(dir) = plugin {
        cmd.arg("--plugin-dir").arg(dir);
    }
    if let Some(m) = &o.model {
        cmd.args(["--model", m]);
    }
    let child = cmd
        .spawn()
        .context("running claude -p (is Claude Code installed and logged in?)")?;
    match wait(child, p.timeout)? {
        Some(s) if s.success() => {}
        Some(s) => eprintln!("  claude -p exited {s} in {}", work.display()),
        None => eprintln!(
            "  claude -p ran over {}s in {}: scored as it stands",
            p.timeout.as_secs(),
            work.display()
        ),
    }
    Ok(work)
}

/// Runs each check in `work`; returns how many passed and the names that did not.
fn score(root: &Path, case: &Path, work: &Path, checks: &Checks) -> Result<(usize, Vec<String>)> {
    let mut passed = 0;
    let mut failed = Vec::new();
    for c in &checks.check {
        if run_check(root, case, work, c)? == (c.expect == Expect::Pass) {
            passed += 1;
        } else {
            failed.push(c.name.clone());
        }
    }
    Ok((passed, failed))
}

fn spread(v: &[usize]) -> usize {
    v.iter().max().copied().unwrap_or(0) - v.iter().min().copied().unwrap_or(0)
}

/// Compares the arms' totals in whole checks: with equal runs per arm, a
/// difference in means is a difference in totals over the run count.
fn verdict(with: &[usize], without: &[usize]) -> String {
    let (a, b) = (with.iter().sum::<usize>(), without.iter().sum::<usize>());
    let runs = with.len().max(1);
    let noise = spread(with).max(spread(without));
    let sign = if a >= b { "+" } else { "-" };
    let diff = a.abs_diff(b);
    if diff <= noise * runs {
        format!(
            "no result: delta {sign}{diff}/{runs} checks per run is within the A/A spread of {noise}"
        )
    } else {
        format!("delta {sign}{diff}/{runs} checks per run, outside the A/A spread of {noise}")
    }
}

fn cases(root: &Path, only: Option<&String>) -> Result<Vec<PathBuf>> {
    let mut cases: Vec<PathBuf> = fs::read_dir(root.join("evals"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("checks.toml").exists())
        .filter(|p| only.is_none_or(|c| p.ends_with(c)))
        .collect();
    cases.sort();
    ensure!(!cases.is_empty(), "no eval case with a checks.toml matched");
    Ok(cases)
}

fn checks_of(case: &Path) -> Result<Checks> {
    let path = case.join("checks.toml");
    toml::from_str(&fs::read_to_string(&path)?)
        .with_context(|| format!("parsing {}", path.display()))
}

fn run_control(root: &Path, cases: &[PathBuf], mode: Mode) -> Result<ExitCode> {
    let mut wrong = Vec::new();
    for case in cases {
        let checks = checks_of(case)?;
        wrong.extend(if mode == Mode::Solution {
            solution(root, case, &checks)?
        } else {
            control(root, case, &checks)?
        });
    }
    for w in &wrong {
        eprintln!("{w}");
    }
    let against = if mode == Mode::Solution {
        "the reference solution"
    } else {
        "the untouched base"
    };
    eprintln!(
        "{} cases against {against}: {} wrong",
        cases.len(),
        wrong.len()
    );
    Ok(if wrong.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn run_models(root: &Path, cases: &[PathBuf], o: &Opts) -> Result<ExitCode> {
    let plugin = plugin_copy(root)?;
    let mut report =
        String::from("| case | checks | with | without | verdict |\n|---|---|---|---|---|\n");
    for case in cases {
        let checks = checks_of(case)?;
        let mut scores = [Vec::new(), Vec::new()];
        for (i, arm) in [Arm::With, Arm::Without].into_iter().enumerate() {
            for run in 1..=o.runs {
                let dir = (arm == Arm::With).then_some(plugin.as_path());
                let work = attempt(case, dir, &format!("{}-{run}", arm.name()), o)?;
                let (passed, failed) = score(root, case, &work, &checks)?;
                let missed = if failed.is_empty() {
                    String::new()
                } else {
                    format!(" (failed: {})", failed.join(", "))
                };
                eprintln!(
                    "{} {} run {run}: {passed}/{}{missed}",
                    case.display(),
                    arm.name(),
                    checks.check.len()
                );
                scores[i].push(passed);
            }
        }
        let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let [with, without] = &scores;
        writeln!(
            report,
            "| {name} | {} | {with:?} | {without:?} | {} |",
            checks.check.len(),
            verdict(with, without)
        )?;
    }
    println!("{report}");
    fs::write(scratch().join("report.md"), &report)?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let result = opts().and_then(|o| {
        let root = root.canonicalize().context("finding the repository root")?;
        let ok = |pass: bool| {
            if pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        };
        match (o.mode, o.files.as_slice()) {
            (Mode::Plugin, _) => Ok(ok(plugin::run(&root)? == 0)),
            (Mode::Rescore, [file]) => Ok(ok(merge::run_rescore(file)?)),
            (Mode::Merge, [with, baseline, out]) => {
                merge::run_merge(with, baseline, out)?;
                Ok(ExitCode::SUCCESS)
            }
            (Mode::Evidence, [file, grader]) => {
                merge::run_evidence(file, &grader.to_string_lossy())?;
                Ok(ExitCode::SUCCESS)
            }
            (Mode::Harvest, [result, state_dir, export]) => {
                let run_root = std::env::var_os("PE_RUN_ROOT")
                    .map_or_else(|| PathBuf::from("/private/tmp"), PathBuf::from);
                let unseal = std::env::var_os("PE_UNSEAL").is_some_and(|v| v == "1");
                state::run_harvest(result, state_dir, export, &run_root, unseal)?;
                Ok(ExitCode::SUCCESS)
            }
            (Mode::Rescore | Mode::Merge | Mode::Evidence | Mode::Harvest, _) => {
                bail!(
                    "--rescore takes one file, --merge three, --evidence a file and a grader, --harvest three"
                )
            }
            (Mode::Control | Mode::Solution, _) => {
                run_control(&root, &cases(&root, o.case.as_ref())?, o.mode)
            }
            (Mode::Model, _) => run_models(&root, &cases(&root, o.case.as_ref())?, &o),
        }
    });
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("eval: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shell half of the harness (lib.sh, plugin-eval.sh) checks itself
    /// in test.sh; this runs it, so `cargo test` covers both halves.
    #[test]
    fn the_shell_scripts_pass_their_own_checks() {
        let out = Command::new("sh")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("test.sh"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn a_delta_inside_the_spread_is_no_result() {
        assert!(verdict(&[5, 7], &[5, 5]).starts_with("no result"));
        assert!(verdict(&[9, 9], &[5, 6]).starts_with("delta +7/2"));
    }
}
