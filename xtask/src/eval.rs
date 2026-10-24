//! `cargo xtask eval`: the plugin's evals, scored by running the code.
//!
//! `claude plugin eval` grades a transcript; it has no command graders, so it
//! cannot tell whether the code it produced builds. This layer can: each case
//! in `evals/<case>/` is run headless (`claude -p`) in a fresh copy of the
//! base workspace plus the case's fixture, once with this plugin
//! (`--plugin-dir`) and once without, and then every command in the case's
//! `checks.toml` is run against what was left on disk.
//!
//! Each arm runs `--runs` times (default 2), and the spread between an arm's
//! own runs is its A/A noise: a difference between the arms counts only when
//! it is larger than that spread. Anything else is reported as no result.
//!
//! `--control` runs no model: it checks every check against the untouched
//! base, where a task's own checks must fail -- a check the base already
//! passes would grade nothing. Every other run is a real model call, so this
//! is a manual task (`eval:run`), never part of `devenv test`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, Result, bail, ensure};
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

/// Whether to run the model or only the control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Control,
    Model,
}

#[derive(Debug)]
struct Opts {
    mode: Mode,
    case: Option<String>,
    runs: usize,
    model: Option<String>,
    max_turns: usize,
}

fn opts(args: &[String]) -> Result<Opts> {
    let mut o = Opts {
        mode: Mode::Model,
        case: None,
        runs: 2,
        model: None,
        max_turns: 40,
    };
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--control" => o.mode = Mode::Control,
            "--case" => o.case = Some(val()?.clone()),
            "--runs" => o.runs = val()?.parse()?,
            "--model" => o.model = Some(val()?.clone()),
            "--max-turns" => o.max_turns = val()?.parse()?,
            other => bail!("unknown eval option `{other}`"),
        }
    }
    ensure!(
        o.runs >= 2,
        "at least 2 runs per arm: one run has no spread, and the spread is the A/A control"
    );
    Ok(o)
}

/// The prompt: `prompt.md` with its YAML frontmatter removed.
fn prompt(case: &Path) -> Result<String> {
    let text = fs::read_to_string(case.join("prompt.md")).context("reading prompt.md")?;
    let body = match text.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").map_or(rest, |(_, b)| b),
        None => &text,
    };
    Ok(body.trim().to_string())
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

/// What a case starts from: this workspace's crates, gates, config and docs
/// skeleton, as a project made from the template has them -- without the
/// plugin itself, its evals, or its research (the evidence note and the pinned
/// sources, which include BeautifulRust's skill text): the arm without the
/// plugin must not be able to read what the plugin teaches.
fn base(root: &Path, work: &Path) -> Result<()> {
    const SKIP: &[&str] = &[
        "target",
        ".git",
        "skills",
        "evals",
        ".claude-plugin",
        ".claude",
        ".direnv",
        "README.md",
    ];
    fs::create_dir_all(work)?;
    for e in fs::read_dir(root)? {
        let e = e?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) || name.starts_with(".devenv") {
            continue;
        }
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &work.join(name.as_ref()))?;
        } else {
            fs::copy(e.path(), work.join(name.as_ref()))?;
        }
    }
    // The research skeleton stays; the plugin's own research goes.
    let research = work.join("docs/research");
    for dir in ["sources", "notes"] {
        let d = research.join(dir);
        if d.exists() {
            for e in fs::read_dir(&d)? {
                let p = e?.path();
                let keep = p.file_name().is_some_and(|n| n == "TEMPLATE.md");
                if !keep {
                    fs::remove_file(&p)?;
                }
            }
        }
    }
    fs::create_dir_all(research.join("sources"))?;
    fs::write(
        research.join("sources/MANIFEST.toml"),
        "# Pinned research sources.\n",
    )?;
    let index = work.join("docs/README.md");
    if index.exists() {
        let text = fs::read_to_string(&index)?;
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| !l.contains("a-rust-code-evidence"))
            .collect();
        fs::write(&index, kept.join("\n") + "\n")?;
    }
    Ok(())
}

/// A fresh working copy for one run: the base, then the case's fixture.
fn fresh(root: &Path, case: &Path, slot: &str) -> Result<PathBuf> {
    let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("case");
    let work = root.join("target/eval").join(name).join(slot);
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    base(root, &work)?;
    let fixture = case.join("fixture");
    if fixture.is_dir() {
        copy_dir(&fixture, &work).context("copying the fixture")?;
    }
    Ok(work)
}

fn run_check(work: &Path, c: &Check) -> Result<bool> {
    let status = Command::new("sh")
        .args(["-c", &c.run])
        .current_dir(work)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("running check `{}`", c.name))?;
    Ok(status.success())
}

/// Every check on the untouched base must do what its `on_base` says.
fn control(root: &Path, case: &Path, checks: &Checks) -> Result<Vec<String>> {
    let work = fresh(root, case, "control")?;
    let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("case");
    let mut wrong = Vec::new();
    for c in &checks.check {
        let passed = run_check(&work, c)?;
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

/// Runs the model once in a fresh working copy; returns the copy.
fn attempt(root: &Path, case: &Path, arm: Arm, run: usize, o: &Opts) -> Result<PathBuf> {
    let work = fresh(root, case, &format!("{}-{run}", arm.name()))?;
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(prompt(case)?)
        .args([
            "--output-format",
            "json",
            "--permission-mode",
            "bypassPermissions",
        ])
        .args(["--max-turns", &o.max_turns.to_string()])
        .current_dir(&work)
        .stdin(Stdio::null());
    if arm == Arm::With {
        cmd.arg("--plugin-dir").arg(root);
    }
    if let Some(m) = &o.model {
        cmd.args(["--model", m]);
    }
    let out = cmd
        .output()
        .context("running claude -p (is Claude Code installed and logged in?)")?;
    fs::write(work.join(".eval-transcript.json"), &out.stdout)?;
    ensure!(
        out.status.success(),
        "claude -p failed in {}: {}",
        work.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(work)
}

/// Runs each check in `work`; returns how many passed and the names that did not.
fn score(work: &Path, checks: &Checks) -> Result<(usize, Vec<String>)> {
    let mut passed = 0;
    let mut failed = Vec::new();
    for c in &checks.check {
        if run_check(work, c)? == (c.expect == Expect::Pass) {
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

pub(crate) fn run(args: &[String], root: &Path) -> Result<ExitCode> {
    let o = opts(args)?;
    let cases = cases(root, o.case.as_ref())?;
    if o.mode == Mode::Control {
        let mut wrong = Vec::new();
        for case in &cases {
            wrong.extend(control(root, case, &checks_of(case)?)?);
        }
        for w in &wrong {
            eprintln!("{w}");
        }
        eprintln!(
            "{} cases against the untouched base: {} wrong",
            cases.len(),
            wrong.len()
        );
        return Ok(if wrong.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
    let mut report =
        String::from("| case | checks | with | without | verdict |\n|---|---|---|---|---|\n");
    for case in &cases {
        let checks = checks_of(case)?;
        let mut scores = [Vec::new(), Vec::new()];
        for (i, arm) in [Arm::With, Arm::Without].into_iter().enumerate() {
            for run in 1..=o.runs {
                let work = attempt(root, case, arm, run, &o)?;
                let (passed, failed) = score(&work, &checks)?;
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
    fs::write(root.join("target/eval/report.md"), &report)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delta_inside_the_spread_is_no_result() {
        assert!(verdict(&[5, 7], &[5, 5]).starts_with("no result"));
        assert!(verdict(&[9, 9], &[5, 6]).starts_with("delta +7/2"));
    }
}
