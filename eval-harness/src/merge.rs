//! A with-arm-only `claude plugin eval` run (`--ablation none`) beside a
//! stored baseline's without arm, scored the way the runner scores a two-arm
//! run.
//!
//! The without arm never loads the plugin, so a commit that changes only the
//! plugin cannot change it: its runs can be reused, and only the with arm paid
//! for again. Reuse is allowed per case only when the case's content key --
//! written by `plugin-eval.sh` beside each result as `<result>.keys` -- is the
//! same in both runs: the key covers everything the without arm depends on
//! (what the scaffold copies, the case's prompt, graders, fixture and
//! solution, the captured toolchain, the pinned model and judge, the claude
//! version, the job caps).
//!
//! Scoring follows the docs ("Score against the no-plugin baseline"): in a
//! two-arm run every `tool_used` grader on `Skill` and every grader marked
//! `arm: with-only` is left out of the score in both arms, unless it is
//! marked `arm: both`, or unless every grader of the case is left out. Under
//! `--ablation none` nothing is left out, so a with-only run's own scores are
//! recomputed before they are set beside the baseline's.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

const EPS: f64 = 1e-9;

/// Whether a case's grader is left out of a two-arm run's score, before the
/// "every grader left out" rule.
fn with_only(def: &Value) -> Result<bool> {
    let config = &def["config"];
    for key in ["target", "focus"] {
        if config[key] == "mock_calls" {
            bail!(
                "grader `{}` reads mock_calls, whose scoring depends on which servers the plugin declares; not supported here",
                def["name"]
            );
        }
    }
    Ok(match config["arm"].as_str() {
        Some("both") => false,
        Some("with-only") => true,
        _ => def["type"] == "tool_used" && config["tool"] == "Skill",
    })
}

/// The graders of one case that count in a two-arm run, by name.
fn scored(case: &Value) -> Result<BTreeMap<String, bool>> {
    let defs = case["graders"]
        .as_array()
        .context("a case without graders")?;
    let mut out = BTreeMap::new();
    for d in defs {
        let name = d["name"].as_str().context("a grader without a name")?;
        out.insert(name.to_string(), !with_only(d)?);
    }
    if out.values().all(|s| !s) {
        out.values_mut().for_each(|s| *s = true);
    }
    Ok(out)
}

/// One run's score: the weighted share of its counted graders that passed.
/// A run with no grader results (it ended on an API error) keeps its own.
fn run_score(run: &Value, counts: &BTreeMap<String, bool>) -> Result<f64> {
    let graders = run["graders"].as_array().context("a run without graders")?;
    if graders.is_empty() {
        return run["score"].as_f64().context("a run without a score");
    }
    let (mut got, mut of) = (0.0, 0.0);
    for g in graders {
        let name = g["name"]
            .as_str()
            .context("a grader result without a name")?;
        if !counts.get(name).copied().unwrap_or(true) {
            continue;
        }
        let w = g["weight"].as_f64().unwrap_or(1.0);
        of += w;
        if g["passed"] == true {
            got += w;
        }
    }
    Ok(if of > 0.0 { got / of } else { 0.0 })
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Rescores `doc` in place as a two-arm run: each run's score, `passed` and
/// grader `scored` flags, each case's aggregates, the suite's aggregates.
pub fn rescore(doc: &mut Value) -> Result<()> {
    let threshold = doc["suite"]["threshold"].as_f64().unwrap_or(1.0);
    let mut case_scores = Vec::new();
    let mut pass_rates = Vec::new();
    let mut deltas = Vec::new();
    let mut passed_cases = 0;
    let cases = doc["cases"].as_array_mut().context("no cases")?;
    for case in cases.iter_mut() {
        let counts = scored(case)?;
        let mut arm_stats: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let arms = case["arms"]
            .as_object_mut()
            .context("a case without arms")?;
        for (arm, runs) in arms.iter_mut() {
            let runs = runs.as_array_mut().context("an arm without runs")?;
            let mut scores = Vec::new();
            for run in runs.iter_mut() {
                let s = run_score(run, &counts)?;
                run["score"] = json!(s);
                run["passed"] = json!(s >= threshold - EPS);
                if let Some(gs) = run["graders"].as_array_mut() {
                    for g in gs {
                        let name = g["name"].as_str().unwrap_or_default().to_string();
                        let c = counts.get(&name).copied().unwrap_or(true);
                        g["scored"] = json!(c);
                        g["withOnly"] = json!(!c);
                    }
                }
                scores.push(s);
            }
            let passes: Vec<f64> = scores
                .iter()
                .map(|s| f64::from(u8::from(*s >= threshold - EPS)))
                .collect();
            if let (Some(s), Some(p)) = (mean(&scores), mean(&passes)) {
                arm_stats.insert(arm.clone(), (s, p));
            }
        }
        let (score, pass_rate) = *arm_stats.get("with").context("a case without a with arm")?;
        let agg = &mut case["aggregates"];
        agg["score"] = json!(score);
        agg["passRate"] = json!(pass_rate);
        if let Some(&(sw, pw)) = arm_stats.get("without") {
            agg["scoreWithout"] = json!(sw);
            agg["passRateWithout"] = json!(pw);
            agg["delta"] = json!(score - sw);
            deltas.push(score - sw);
        }
        case_scores.push(score);
        pass_rates.push(pass_rate);
        if score >= threshold - EPS {
            passed_cases += 1;
        }
    }
    let agg = &mut doc["aggregates"];
    agg["casesTotal"] = json!(case_scores.len());
    agg["casesPassed"] = json!(passed_cases);
    agg["overallScore"] = json!(mean(&case_scores).unwrap_or(0.0));
    agg["overallPassRate"] = json!(mean(&pass_rates).unwrap_or(0.0));
    if let Some(d) = mean(&deltas) {
        agg["meanDelta"] = json!(d);
    }
    Ok(())
}

/// Every score `rescore` would change in a two-arm result: empty when the
/// rules above reproduce the runner's own numbers.
pub fn differences(doc: &Value) -> Result<Vec<String>> {
    let mut again = doc.clone();
    rescore(&mut again)?;
    let mut out = Vec::new();
    let num = |v: &Value| v.as_f64();
    let mut cmp = |what: String, a: &Value, b: &Value| match (num(a), num(b)) {
        (Some(x), Some(y)) if (x - y).abs() < EPS => {}
        (None, None) => {}
        _ => out.push(format!("{what}: recorded {a}, recomputed {b}")),
    };
    for (c, (old, new)) in doc["cases"]
        .as_array()
        .context("no cases")?
        .iter()
        .zip(again["cases"].as_array().context("no cases")?)
        .enumerate()
    {
        let name = old["name"].as_str().unwrap_or("?");
        for arm in ["with", "without"] {
            let (Some(o), Some(n)) = (old["arms"][arm].as_array(), new["arms"][arm].as_array())
            else {
                continue;
            };
            for (i, (ro, rn)) in o.iter().zip(n).enumerate() {
                cmp(
                    format!("{name} {arm}/{i} score"),
                    &ro["score"],
                    &rn["score"],
                );
            }
        }
        for k in [
            "score",
            "passRate",
            "scoreWithout",
            "passRateWithout",
            "delta",
        ] {
            cmp(
                format!("case {c} {name} {k}"),
                &old["aggregates"][k],
                &new["aggregates"][k],
            );
        }
    }
    for k in [
        "casesTotal",
        "casesPassed",
        "overallScore",
        "overallPassRate",
        "meanDelta",
    ] {
        cmp(
            format!("suite {k}"),
            &doc["aggregates"][k],
            &again["aggregates"][k],
        );
    }
    Ok(out)
}

/// `<case> <key>` lines, as `plugin-eval.sh` writes them.
pub fn parse_keys(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(c, k)| (c.trim().to_string(), k.trim().to_string()))
        .collect()
}

/// The with-only run `with` beside `baseline`'s without arm, as one two-arm
/// result. Fails, naming the cases, when a case has no baseline or its key
/// differs: then the without arm must be run again.
pub fn merge(
    with: &Value,
    with_keys: &BTreeMap<String, String>,
    baseline: &Value,
    base_keys: &BTreeMap<String, String>,
) -> Result<Value> {
    ensure!(
        with["suite"]["ablation"] == "none",
        "the run to merge must be a with-arm-only run (--ablation none), not {}",
        with["suite"]["ablation"]
    );
    ensure!(
        baseline["suite"]["ablation"] == "with-without",
        "the baseline must be a two-arm run (--ablation with-without)"
    );
    ensure!(
        baseline["partial"] != true,
        "the baseline is a partial run; it cannot stand in for a without arm"
    );
    let off = differences(baseline)?;
    ensure!(
        off.is_empty(),
        "the baseline's own scores are not reproduced by these rules:\n  {}",
        off.join("\n  ")
    );
    let mut out = with.clone();
    let mut stale = Vec::new();
    let base_cases = baseline["cases"].as_array().context("no baseline cases")?;
    for case in out["cases"].as_array_mut().context("no cases")? {
        let name = case["name"]
            .as_str()
            .context("a case without a name")?
            .to_string();
        let key = with_keys.get(&name);
        let base = base_cases.iter().find(|c| c["name"] == name.as_str());
        match (key, base_keys.get(&name), base) {
            (Some(k), Some(b), Some(base)) if k == b => {
                case["arms"]["without"] = base["arms"]["without"].clone();
            }
            (None, _, _) => stale.push(format!("{name}: the run has no key")),
            (_, None, _) | (_, _, None) => stale.push(format!("{name}: not in the baseline")),
            _ => stale.push(format!("{name}: its key differs from the baseline's")),
        }
    }
    ensure!(
        stale.is_empty(),
        "no reusable without arm for:\n  {}\nrun these cases with --ablation with-without",
        stale.join("\n  ")
    );
    out["suite"]["ablation"] = json!("with-without");
    out["merged"] = json!({
        "withoutArmFrom": {
            "startedAt": baseline["startedAt"],
            "claudeVersion": baseline["claudeVersion"],
            "suiteRoot": baseline["suite"]["root"],
        },
        "note": "the without arm of every case is a stored baseline's, reused because its content key is unchanged",
    });
    rescore(&mut out)?;
    Ok(out)
}

/// `--rescore <result.json>`: prints every score the rules here do not
/// reproduce; fails if there is one.
pub fn run_rescore(path: &Path) -> Result<bool> {
    let doc: Value = serde_json::from_str(&fs::read_to_string(path)?)
        .with_context(|| format!("reading {}", path.display()))?;
    let off = differences(&doc)?;
    for d in &off {
        println!("{d}");
    }
    println!(
        "{}: {} difference(s) from the runner's scores",
        path.display(),
        off.len()
    );
    Ok(off.is_empty())
}

/// `--merge <with.json> <baseline.json> <out.json>`.
pub fn run_merge(with: &Path, baseline: &Path, out: &Path) -> Result<()> {
    let read = |p: &Path| -> Result<Value> {
        serde_json::from_str(
            &fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?,
        )
        .with_context(|| format!("parsing {}", p.display()))
    };
    let keys = |p: &Path| -> Result<BTreeMap<String, String>> {
        let k = p.with_extension("keys");
        Ok(parse_keys(
            &fs::read_to_string(&k).with_context(|| format!("reading {}", k.display()))?,
        ))
    };
    let merged = merge(
        &read(with)?,
        &keys(with)?,
        &read(baseline)?,
        &keys(baseline)?,
    )?;
    fs::write(out, serde_json::to_string_pretty(&merged)? + "\n")?;
    println!("{}", table(&merged));
    Ok(())
}

/// `--evidence <result.json> <grader>`: each run's time, error and the
/// evidence one grader kept (a file grader keeps the whole file), as
/// plugin-eval.sh's `--probe` prints it.
pub fn run_evidence(path: &Path, grader: &str) -> Result<()> {
    let doc: Value = serde_json::from_str(&fs::read_to_string(path)?)
        .with_context(|| format!("reading {}", path.display()))?;
    println!(
        "{}: {} s wall, ${:.4}",
        path.display(),
        doc["durationSeconds"],
        doc["costUsd"].as_f64().unwrap_or(0.0)
    );
    for case in doc["cases"].as_array().into_iter().flatten() {
        for (arm, runs) in case["arms"].as_object().into_iter().flatten() {
            for (i, run) in runs.as_array().into_iter().flatten().enumerate() {
                println!(
                    "== {} {arm}/{i}: {} s, {} turns, error {}, trace {}",
                    case["name"].as_str().unwrap_or("?"),
                    run["durationSeconds"],
                    run["turns"],
                    run["error"],
                    run["tracePath"].as_str().unwrap_or("-")
                );
                let g = run["graders"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|g| g["name"] == grader);
                match g.map(|g| &g["evidence"]) {
                    Some(Value::String(s)) => println!("{s}"),
                    Some(v) if !v.is_null() => println!("{v}"),
                    _ => println!("(no evidence from grader `{grader}`)"),
                }
            }
        }
    }
    Ok(())
}

/// The runner's summary table, from a two-arm result.
pub fn table(doc: &Value) -> String {
    let mut s = String::from("| case | with | w/out | delta |\n|---|---|---|---|\n");
    let f = |v: &Value| v.as_f64().map_or("-".to_string(), |x| format!("{x:.2}"));
    for c in doc["cases"].as_array().into_iter().flatten() {
        let a = &c["aggregates"];
        s.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            c["name"].as_str().unwrap_or("?"),
            f(&a["score"]),
            f(&a["scoreWithout"]),
            f(&a["delta"])
        ));
    }
    s.push_str(&format!(
        "\nmean delta {}, overall score {}",
        f(&doc["aggregates"]["meanDelta"]),
        f(&doc["aggregates"]["overallScore"])
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run4() -> Value {
        serde_json::from_str(include_str!("../fixtures/run-28c0e7a.json")).unwrap()
    }

    #[test]
    fn the_rules_reproduce_every_score_of_a_real_two_arm_run() {
        let off = differences(&run4()).unwrap();
        assert!(off.is_empty(), "{off:#?}");
    }

    /// A with-only run as `--ablation none` reports it: one arm, every
    /// grader counted.
    fn as_with_only(doc: &Value) -> Value {
        let mut w = doc.clone();
        w["suite"]["ablation"] = json!("none");
        w["aggregates"].as_object_mut().unwrap().remove("meanDelta");
        for c in w["cases"].as_array_mut().unwrap() {
            c["arms"].as_object_mut().unwrap().remove("without");
            for r in c["arms"]["with"].as_array_mut().unwrap() {
                let gs = r["graders"].as_array_mut().unwrap();
                for g in gs.iter_mut() {
                    g["scored"] = json!(true);
                    g["withOnly"] = json!(false);
                }
                let n = gs.len() as f64;
                let p = gs.iter().filter(|g| g["passed"] == true).count() as f64;
                r["score"] = json!(if n > 0.0 { p / n } else { 0.0 });
            }
        }
        w
    }

    fn keys_of(doc: &Value, key: &str) -> BTreeMap<String, String> {
        doc["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["name"].as_str().unwrap().to_string(), key.to_string()))
            .collect()
    }

    #[test]
    fn a_with_only_run_beside_its_own_baseline_scores_as_the_two_arm_run_did() {
        let base = run4();
        let with = as_with_only(&base);
        let merged = merge(&with, &keys_of(&with, "k"), &base, &keys_of(&base, "k")).unwrap();
        for (m, b) in merged["cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(base["cases"].as_array().unwrap())
        {
            for k in ["score", "scoreWithout", "delta", "passRate"] {
                let (x, y) = (
                    m["aggregates"][k].as_f64().unwrap(),
                    b["aggregates"][k].as_f64().unwrap(),
                );
                assert!((x - y).abs() < EPS, "{} {k}: {x} vs {y}", b["name"]);
            }
        }
        let (x, y) = (
            merged["aggregates"]["meanDelta"].as_f64().unwrap(),
            base["aggregates"]["meanDelta"].as_f64().unwrap(),
        );
        assert!((x - y).abs() < EPS);
    }

    #[test]
    fn a_changed_key_refuses_the_reuse_and_names_the_case() {
        let base = run4();
        let with = as_with_only(&base);
        let mut wk = keys_of(&with, "k");
        wk.insert("own-dont-leak".into(), "other".into());
        let e = merge(&with, &wk, &base, &keys_of(&base, "k")).unwrap_err();
        let msg = format!("{e:#}");
        assert!(msg.contains("own-dont-leak: its key differs"), "{msg}");
        assert!(!msg.contains("layering-fix"), "{msg}");
    }

    #[test]
    fn a_two_arm_run_is_not_taken_as_the_with_only_side() {
        let base = run4();
        let e = merge(&base, &keys_of(&base, "k"), &base, &keys_of(&base, "k")).unwrap_err();
        assert!(format!("{e:#}").contains("--ablation none"));
    }

    fn one_case(graders: Value, results: Value) -> Value {
        json!({
            "suite": {"threshold": 1, "ablation": "with-without"},
            "aggregates": {},
            "cases": [{"name": "c", "graders": graders, "aggregates": {},
                       "arms": {"with": [{"score": 0, "graders": results}]}}]
        })
    }

    #[test]
    fn when_every_grader_is_with_only_all_of_them_count() {
        let mut d = one_case(
            json!([{"name": "s", "type": "tool_used", "config": {"tool": "Skill"}}]),
            json!([{"name": "s", "passed": true, "weight": 1}]),
        );
        rescore(&mut d).unwrap();
        assert_eq!(d["cases"][0]["arms"]["with"][0]["score"], json!(1.0));
    }

    #[test]
    fn arm_both_keeps_a_skill_grader_in_the_score_and_weights_count() {
        let mut d = one_case(
            json!([
                {"name": "s", "type": "tool_used", "config": {"tool": "Skill", "arm": "both"}},
                {"name": "f", "type": "regex", "config": {}}
            ]),
            json!([
                {"name": "s", "passed": false, "weight": 3},
                {"name": "f", "passed": true, "weight": 1}
            ]),
        );
        rescore(&mut d).unwrap();
        assert_eq!(d["cases"][0]["arms"]["with"][0]["score"], json!(0.25));
    }

    #[test]
    fn a_run_that_ended_on_an_api_error_keeps_its_zero() {
        let mut d = one_case(
            json!([{"name": "f", "type": "regex", "config": {}}]),
            json!([]),
        );
        rescore(&mut d).unwrap();
        assert_eq!(
            d["cases"][0]["arms"]["with"][0]["score"].as_f64(),
            Some(0.0)
        );
    }
}
