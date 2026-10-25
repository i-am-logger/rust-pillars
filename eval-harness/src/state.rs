//! What each run of a `--keep-temp` suite left behind, kept small
//! (plugin-eval.sh `--keep-state`).
//!
//! The runner deletes every run's directory -- the working copy the graders
//! read and the full trace -- and has no command graders (docs: "There are
//! no custom-code graders"), so a regex grader's verdict keeps no evidence.
//! Under `--keep-temp` it keeps them all, about 1.4 GB per run (measured),
//! sealed: the run directory read-only and `sealed/` (where the home with
//! the working copy goes) mode 000 (measured).
//!
//! This keeps, per run listed in the result, the full trace and the seconds
//! from the run's start to its first trace event, which need no change to
//! the sealed directory. With `unseal` it also gives the owner back the
//! rights on that one directory (`u+rwX`, as `chmod -R` would), keeps the
//! working copy's changes against a fresh scaffold of its case (build output
//! left out), and removes the directory -- only a directory the result
//! names, under the run root, named `e-*`, holding `out/trace.jsonl`.
//! Without `unseal` the kept directories are listed in `kept-dirs.txt` and
//! left in place.
//!
//! Everything in the working copy was written by the plugin under test, so
//! none of it is followed or opened blindly: a symbolic link is kept as its
//! target's name (a link to `~/.ssh/<key>` must not bring the key into the
//! kept changes), and a FIFO, socket or device -- which can hang a reader --
//! or a second name of a file (a hard link) is never read, only listed in
//! `skipped.txt`. Both trees are first copied that way into plain mirrors,
//! and `diff` only ever sees those.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde_json::Value;

/// The run directory a trace path belongs to, if it is one this may touch.
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
        && plain_dir(dir, Path::new("out"))
        && fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir())
        && fs::symlink_metadata(trace).is_ok_and(|m| m.is_file())
        && (dir.join("home").exists() || dir.join("sealed").exists());
    ok.then(|| dir.to_path_buf())
}

/// The contents of `path` if it is a regular file with one name, read only
/// if the file opened is the one lstat saw: never through a link, never a
/// FIFO or device.
fn read_regular(path: &Path) -> Option<String> {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.nlink() > 1 {
        return None;
    }
    let mut file = fs::File::open(path).ok()?;
    let opened = file.metadata().ok()?;
    if !(opened.is_file() && opened.dev() == meta.dev() && opened.ino() == meta.ino()) {
        return None;
    }
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Whether `dir/rel` is a directory reached through directories only, no
/// component of it a symbolic link the session could have planted.
fn plain_dir(dir: &Path, rel: &Path) -> bool {
    let mut p = dir.to_path_buf();
    rel.components().all(|c| {
        p.push(c);
        fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir())
    })
}

/// The working copy inside a run directory whose rights are back: the live
/// layout's `home/cwd`, or the one under `sealed/`.
fn working_copy(dir: &Path) -> Option<PathBuf> {
    ["home/cwd", "sealed/home/cwd"]
        .into_iter()
        .find(|rel| plain_dir(dir, Path::new(rel)))
        .map(|rel| dir.join(rel))
        .or_else(|| {
            if !plain_dir(dir, Path::new("sealed")) {
                return None;
            }
            fs::read_dir(dir.join("sealed"))
                .ok()?
                .flatten()
                .find_map(|e| {
                    let rel = Path::new("sealed").join(e.file_name()).join("cwd");
                    plain_dir(dir, &rel).then(|| dir.join(rel))
                })
        })
}

/// Gives the owner back `u+rwX` on `path` and everything below it, as
/// `chmod -R u+rwX` would, never through a symbolic link.
fn open_up(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let meta = fs::symlink_metadata(path)?;
    let mode = meta.permissions().mode();
    if meta.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o700))?;
        for entry in fs::read_dir(path)? {
            open_up(&entry?.path())?;
        }
    } else if meta.is_file() {
        let x = if mode & 0o111 == 0 { 0 } else { 0o100 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o600 | x))?;
    }
    Ok(())
}

/// The kind of a file that is neither a directory, a regular file nor a
/// symbolic link.
fn special_kind(meta: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt as _;
    let t = meta.file_type();
    if t.is_fifo() {
        "fifo"
    } else if t.is_socket() {
        "socket"
    } else if t.is_char_device() {
        "character device"
    } else if t.is_block_device() {
        "block device"
    } else {
        "unknown file type"
    }
}

/// `src` copied into `dst` (which must not exist) the safe way: directories
/// and regular files as they are, each symbolic link as a file holding
/// `symlink -> <target>`, and nothing else -- FIFOs, sockets, devices and
/// hard-linked files are appended to `skipped` (relative to `top`) instead.
/// `target` and `.devenv.env` are left out, as the diff leaves them out.
fn mirror(src: &Path, dst: &Path, top: &Path, skipped: &mut String) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    fs::create_dir(dst)?;
    let mut entries: Vec<_> = fs::read_dir(src)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if name == "target" || name == ".devenv.env" {
            continue;
        }
        let (from, to) = (entry.path(), dst.join(&name));
        let rel = from
            .strip_prefix(top)
            .unwrap_or(&from)
            .display()
            .to_string();
        let meta = fs::symlink_metadata(&from)?;
        if meta.file_type().is_symlink() {
            fs::write(
                &to,
                format!("symlink -> {}\n", fs::read_link(&from)?.display()),
            )?;
        } else if meta.is_dir() {
            mirror(&from, &to, top, skipped)?;
        } else if !meta.is_file() {
            writeln!(skipped, "{rel}\t{}", special_kind(&meta))?;
        } else if meta.nlink() > 1 {
            writeln!(skipped, "{rel}\thard link ({} names)", meta.nlink())?;
        } else {
            // Opened only after lstat said regular file, and kept only if
            // the open file is that same file: one swapped for a link or a
            // FIFO in between is not read.
            let mut file = fs::File::open(&from)?;
            let opened = file.metadata()?;
            if opened.is_file() && opened.dev() == meta.dev() && opened.ino() == meta.ino() {
                std::io::copy(&mut file, &mut fs::File::create(&to)?)?;
            } else {
                writeln!(skipped, "{rel}\tchanged while read")?;
            }
        }
    }
    Ok(())
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

/// The changes of one run's working copy kept in `dest`, after giving the
/// owner back the rights on its directory, which is then removed -- only
/// once the changes are kept: a run whose working copy or case cannot be
/// found keeps its directory (false).
fn unseal_and_remove(dir: &Path, dest: &Path, export: &Path, case: &str) -> Result<bool> {
    open_up(dir).with_context(|| format!("giving back the rights on {}", dir.display()))?;
    let scratch = dest.join(".start");
    let (before, after) = (dest.join(".before"), dest.join(".after"));
    let mut kept = false;
    if let (Some(start), Some(cwd)) = (fresh_start(export, case, &scratch), working_copy(dir)) {
        let mut skipped = String::new();
        let mut ignored = String::new();
        mirror(&start, &before, &start, &mut ignored)?;
        mirror(&cwd, &after, &cwd, &mut skipped)?;
        if !skipped.is_empty() {
            fs::write(dest.join("skipped.txt"), &skipped)?;
        }
        let diff = Command::new("diff")
            .args(["-ruN", "-x", "target", "-x", ".devenv.env"])
            .arg(&before)
            .arg(&after)
            .stdin(Stdio::null())
            .output()?;
        // diff exits 0 (the same) or 1 (different); 2 is trouble.
        if diff.status.code().is_some_and(|c| c < 2) {
            fs::write(dest.join("changes.diff"), &diff.stdout)?;
            kept = true;
        }
    }
    for d in [&scratch, &before, &after] {
        if d.exists() {
            fs::remove_dir_all(d)?;
        }
    }
    if kept {
        fs::remove_dir_all(dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    Ok(kept)
}

/// `--harvest <result.json> <state dir> <export root>`, runs under
/// `run_root` (`$PE_RUN_ROOT`, default /private/tmp); `unseal` as above.
pub fn run_harvest(
    result: &Path,
    state: &Path,
    export: &Path,
    run_root: &Path,
    unseal: bool,
) -> Result<()> {
    let doc: Value = serde_json::from_str(
        &fs::read_to_string(result).with_context(|| format!("reading {}", result.display()))?,
    )?;
    fs::create_dir_all(state)?;
    let mut table = String::from("case\tarm\trun\tdurationSeconds\tstartDelaySeconds\terror\n");
    let mut left = String::new();
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
                        "harvest: {} is not a run directory of this result; left alone",
                        trace.display()
                    );
                    continue;
                };
                let dest = state.join(name).join(format!("{arm}-{i}"));
                fs::create_dir_all(&dest)?;
                let text = read_regular(&trace).unwrap_or_default();
                fs::write(dest.join("trace.jsonl"), &text)?;
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
                if unseal && unseal_and_remove(&dir, &dest, export, name)? {
                    removed += 1;
                } else {
                    writeln!(left, "{}", dir.display())?;
                }
            }
        }
    }
    fs::write(state.join("runs.tsv"), &table)?;
    if !left.is_empty() {
        fs::write(state.join("kept-dirs.txt"), &left)?;
    }
    println!(
        "harvest: kept {kept} run(s) in {}; removed {removed} run directory(ies){}",
        state.display(),
        if left.is_empty() {
            String::new()
        } else {
            format!(
                "; the rest are listed in {}",
                state.join("kept-dirs.txt").display()
            )
        }
    );
    print!("{table}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("harvest-test-{name}-{}", std::process::id()));
        if d.exists() {
            let _ = Command::new("chmod").args(["-R", "u+rwX"]).arg(&d).status();
            let _ = fs::remove_dir_all(&d);
        }
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_timestamp_difference_is_in_seconds() {
        let a = timestamp("2026-10-25T13:54:09.745Z").unwrap();
        let b = timestamp("2026-10-25T14:08:04.745Z").unwrap();
        assert!((b - a - 835.0).abs() < 1e-6);
        let day =
            timestamp("2026-03-01T00:00:00Z").unwrap() - timestamp("2026-02-28T00:00:00Z").unwrap();
        assert!((day - 86_400.0).abs() < 1e-6);
    }

    /// A run directory as the runner keeps it: read-only, the home under a
    /// mode-000 `sealed/`, after `plant` put what it likes in the working
    /// copy, as the session under test could.
    fn sealed_run_with(runs: &Path, name: &str, plant: impl Fn(&Path)) -> PathBuf {
        let run = runs.join(name);
        let cwd = run.join("sealed/home/cwd");
        fs::create_dir_all(run.join("out")).unwrap();
        fs::create_dir_all(cwd.join("target")).unwrap();
        fs::write(cwd.join("a.txt"), "after\n").unwrap();
        plant(&cwd);
        fs::write(cwd.join("target/big"), "build output").unwrap();
        let trace = run.join("out/trace.jsonl");
        fs::write(
            &trace,
            "{\"type\":\"system\"}\n{\"type\":\"assistant\",\"timestamp\":\"2026-10-25T10:00:07.000Z\"}\n",
        )
        .unwrap();
        fs::set_permissions(run.join("sealed"), fs::Permissions::from_mode(0o000)).unwrap();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o500)).unwrap();
        trace
    }

    fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
        setup_with(name, |_, _| {})
    }

    /// `setup`, with `plant(working copy, test root)` run before sealing.
    fn setup_with(
        name: &str,
        plant: impl Fn(&Path, &Path),
    ) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = tmp(name);
        let runs = root.join("runs");
        fs::create_dir_all(&runs).unwrap();
        let trace = sealed_run_with(&runs, "e-abc123", |cwd| plant(cwd, &root));
        // An export whose case c scaffolds a.txt as "before".
        let export = root.join("export");
        fs::create_dir_all(export.join("evals/c")).unwrap();
        fs::write(export.join("evals/c/scaffold.sh"), "echo before > a.txt\n").unwrap();
        // A path that is not a run directory: never touched.
        let other = root.join("elsewhere/out");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("trace.jsonl"), "").unwrap();
        let result = root.join("result.json");
        let doc = serde_json::json!({"cases": [{"name": "c", "arms": {
            "with": [{"tracePath": trace, "startedAt": "2026-10-25T10:00:00.000Z", "durationSeconds": 9, "error": null}],
            "without": [{"tracePath": other.join("trace.jsonl"), "startedAt": "2026-10-25T10:00:00.000Z", "durationSeconds": 1, "error": null}]
        }}]});
        fs::write(&result, doc.to_string()).unwrap();
        (root, runs, export, result, other)
    }

    #[test]
    fn unsealed_a_listed_run_keeps_its_changes_and_its_directory_goes() {
        let (root, runs, export, result, other) = setup("unseal");
        let state = root.join("state");
        run_harvest(&result, &state, &export, &runs, true).unwrap();
        assert!(
            !runs.join("e-abc123").exists(),
            "the listed run directory is removed"
        );
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

    /// What the session wrote is not trusted: a link to a secret outside
    /// the run, file or directory, must not bring the secret into the kept
    /// changes; the link is kept as its target's name.
    #[test]
    fn the_harvest_never_follows_a_link_the_session_planted() {
        let (root, _runs, export, result, _) = setup_with("links", |cwd, root| {
            fs::write(root.join("secret.txt"), "TOPSECRET-FILE\n").unwrap();
            fs::create_dir_all(root.join("secretdir")).unwrap();
            fs::write(root.join("secretdir/key"), "TOPSECRET-DIR\n").unwrap();
            std::os::unix::fs::symlink(root.join("secret.txt"), cwd.join("leak")).unwrap();
            std::os::unix::fs::symlink(root.join("secretdir"), cwd.join("leakdir")).unwrap();
        });
        let state = root.join("state");
        run_harvest(&result, &state, &export, &root.join("runs"), true).unwrap();
        let diff = fs::read_to_string(state.join("c/with-0/changes.diff")).unwrap();
        assert!(!diff.contains("TOPSECRET"), "a link was followed: {diff}");
        assert!(
            diff.contains(&format!("symlink -> {}", root.join("secret.txt").display())),
            "the link is kept as its target: {diff}"
        );
        assert!(root.join("secret.txt").is_file() && root.join("secretdir/key").is_file());
        fs::remove_dir_all(&root).unwrap();
    }

    /// The trace itself is a path in a directory the session may have
    /// written: a link there to a secret must not be read either.
    #[test]
    fn the_harvest_never_reads_a_trace_that_is_a_link() {
        use std::os::unix::fs::PermissionsExt as _;
        let (root, runs, export, result, _) = setup("tracelink");
        fs::write(root.join("secret.txt"), "TOPSECRET-TRACE\n").unwrap();
        let run = runs.join("e-abc123");
        fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(run.join("out/trace.jsonl")).unwrap();
        std::os::unix::fs::symlink(root.join("secret.txt"), run.join("out/trace.jsonl")).unwrap();
        let state = root.join("state");
        run_harvest(&result, &state, &export, &runs, false).unwrap();
        let kept = state.join("c/with-0/trace.jsonl");
        assert!(
            !fs::read_to_string(&kept)
                .unwrap_or_default()
                .contains("TOPSECRET"),
            "the linked trace was read"
        );
        let _ = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&root)
            .status();
        fs::remove_dir_all(&root).unwrap();
    }

    /// A FIFO (or a device, or a socket) is never opened: reading one can
    /// hang the harvest for good. It is listed instead.
    #[test]
    fn the_harvest_never_reads_a_fifo_and_lists_it() {
        let (root, runs, export, result, _) = setup_with("fifo", |cwd, _| {
            let ok = Command::new("mkfifo")
                .arg(cwd.join("pipe"))
                .status()
                .unwrap()
                .success();
            assert!(ok, "mkfifo");
        });
        let state = root.join("state");
        let (tx, rx) = std::sync::mpsc::channel();
        let (r, s, e, n) = (result.clone(), state.clone(), export.clone(), runs.clone());
        std::thread::spawn(move || {
            let _ = tx.send(run_harvest(&r, &s, &e, &n, true).map_err(|e| e.to_string()));
        });
        let Ok(done) = rx.recv_timeout(std::time::Duration::from_secs(30)) else {
            // Free the reader that hangs on it (the old diff), then fail.
            let pipe = runs.join("e-abc123/sealed/home/cwd/pipe");
            drop(fs::OpenOptions::new().write(true).open(pipe));
            panic!("the harvest hung on a FIFO");
        };
        done.unwrap();
        let skipped = fs::read_to_string(state.join("c/with-0/skipped.txt")).unwrap_or_default();
        assert!(
            skipped.contains("pipe") && skipped.contains("fifo"),
            "{skipped}"
        );
        assert!(state.join("c/with-0/changes.diff").is_file());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unsealed_a_run_whose_working_copy_cannot_be_found_is_kept() {
        let (root, runs, export, result, _) = setup("nocopy");
        let run = runs.join("e-abc123");
        let _ = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&run)
            .status();
        fs::rename(run.join("sealed/home"), run.join("sealed/elsewhere")).unwrap();
        fs::remove_dir_all(run.join("sealed/elsewhere/cwd")).unwrap();
        let state = root.join("state");
        run_harvest(&result, &state, &export, &runs, true).unwrap();
        assert!(run.exists(), "no changes kept, so the directory stays");
        let listed = fs::read_to_string(state.join("kept-dirs.txt")).unwrap();
        assert!(listed.contains("e-abc123"), "{listed}");
        let _ = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&root)
            .status();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn sealed_a_listed_run_keeps_its_trace_and_is_left_listed() {
        let (root, runs, export, result, _) = setup("sealed");
        let state = root.join("state");
        run_harvest(&result, &state, &export, &runs, false).unwrap();
        let run = runs.join("e-abc123");
        assert!(run.exists(), "without unseal nothing is removed");
        let mode = fs::metadata(run.join("sealed"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0, "nor unsealed");
        assert!(state.join("c/with-0/trace.jsonl").is_file());
        let listed = fs::read_to_string(state.join("kept-dirs.txt")).unwrap();
        assert!(listed.contains("e-abc123"), "{listed}");
        let _ = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&root)
            .status();
        fs::remove_dir_all(&root).unwrap();
    }
}
