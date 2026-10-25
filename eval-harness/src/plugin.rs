//! `--plugin`: the plugin holds together, on every machine and on claude.ai.
//!
//! * every `skills/<name>/SKILL.md` names itself `<name>`, uses only the
//!   frontmatter keys claude.ai accepts for an uploaded skill, and has a
//!   description that is a quoted string of 1..=1024 characters -- an unquoted
//!   value is cut at the first ` #` by YAML, which once lost most of a
//!   description silently;
//! * SKILL.md stays under 180 lines and each reference under 300, so a skill
//!   loads in parts;
//! * every SKILL.md states its `license:`, so a skill uploaded alone still
//!   carries its terms;
//! * no command or skill pre-approves a bare `Bash`, `Edit` or `Write` in
//!   its `allowed-tools`: a grant is scoped (`Bash(cargo test:*)`), and an
//!   unscoped one lets text the model reads in a project steer any command
//!   or write while it runs;
//! * every relative link resolves inside its own skill's folder, so a skill
//!   uploaded alone still works;
//! * the manifest and the marketplace entry agree on the plugin's name.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Frontmatter keys an uploaded claude.ai skill may carry.
const KEYS: &[&str] = &[
    "name",
    "description",
    "license",
    "compatibility",
    "metadata",
    "allowed-tools",
];
/// Tools that must never be pre-approved without a scope.
const UNSCOPED: &[&str] = &["Bash", "Edit", "Write"];
const SKILL_LINES: usize = 180;
const REFERENCE_LINES: usize = 300;
const DESCRIPTION_CHARS: usize = 1024;

/// The `---` frontmatter of a Markdown file, if it has one.
fn front(text: &str) -> Option<&str> {
    text.strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
        .map(|(f, _)| f)
}

/// The entries of `allowed-tools` in a frontmatter: an inline value split on
/// the commas outside parentheses (`Bash(a, b:*)` is one entry), a flow list
/// (`[a, b]`), or a block list (`- a` lines below the key); quotes trimmed.
fn allowed_tools(front: &str) -> Vec<String> {
    let mut lines = front.lines();
    let mut out = Vec::new();
    while let Some(line) = lines.next() {
        let Some(value) = line.strip_prefix("allowed-tools:") else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_prefix('[')
            .and_then(|v| v.strip_suffix(']'))
            .unwrap_or(value);
        if value.is_empty() {
            for item in lines.by_ref() {
                let Some(entry) = item.trim_start().strip_prefix("- ") else {
                    break;
                };
                out.push(entry.to_string());
            }
        } else {
            let mut depth = 0usize;
            let mut start = 0;
            for (i, c) in value.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth = depth.saturating_sub(1),
                    ',' if depth == 0 => {
                        out.push(value[start..i].to_string());
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            out.push(value[start..].to_string());
        }
    }
    out.iter()
        .map(|e| e.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|e| !e.is_empty())
        .collect()
}

/// Problems in a frontmatter's `allowed-tools`: each unscoped grant of a tool
/// that runs commands or writes files.
fn grants(front: &str) -> Vec<String> {
    allowed_tools(front)
        .into_iter()
        .filter(|t| UNSCOPED.contains(&t.as_str()))
        .map(|t| {
            format!(
                "allowed-tools pre-approves a bare `{t}`; scope it (as in `Bash(cargo test:*)` or `Edit(docs/**)`) or leave it out, so it prompts"
            )
        })
        .collect()
}

/// Problems in one command file.
fn command(path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    Ok(match front(&text) {
        None => vec![],
        Some(f) => grants(f)
            .into_iter()
            .map(|p| format!("commands/{name}: {p}"))
            .collect(),
    })
}

/// Problems in one SKILL.md's frontmatter.
fn frontmatter(dir_name: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(front) = front(text) else {
        return vec!["has no `---` frontmatter".into()];
    };
    for line in front
        .lines()
        .filter(|l| !l.starts_with(' ') && l.contains(':'))
    {
        let (key, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.trim();
        if !KEYS.contains(&key) {
            out.push(format!(
                "frontmatter key `{key}` is not one claude.ai accepts ({KEYS:?})"
            ));
        }
        if key == "name" && value != dir_name {
            out.push(format!(
                "name `{value}` differs from its folder `{dir_name}`"
            ));
        }
        if key == "description" {
            match serde_json::from_str::<String>(value) {
                Err(_) => out.push(
                    "description must be one double-quoted string (unquoted, YAML cuts it at ` #`)"
                        .into(),
                ),
                Ok(d) if d.is_empty() || d.chars().count() > DESCRIPTION_CHARS => {
                    out.push(format!(
                        "description is {} characters; it must be 1..={DESCRIPTION_CHARS}",
                        d.chars().count()
                    ));
                }
                Ok(_) => {}
            }
        }
    }
    if !front.lines().any(|l| l.starts_with("description:")) {
        out.push("has no description".into());
    }
    if !front.lines().any(|l| {
        l.strip_prefix("license:")
            .is_some_and(|v| !v.trim().is_empty())
    }) {
        out.push("has no `license:`; a skill uploaded alone must state its terms".into());
    }
    out.extend(grants(front));
    out
}

/// Relative link targets in Markdown text, anchors stripped.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        rest = &rest[i + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = rest[..end].split_whitespace().next().unwrap_or("");
        let path = target.split('#').next().unwrap_or("");
        if !target.contains("://")
            && !target.starts_with("mailto:")
            && !target.starts_with('#')
            && !path.is_empty()
        {
            out.push(path.to_string());
        }
    }
    out
}

/// Problems in one skill folder.
fn skill(dir: &Path) -> Result<Vec<String>> {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string();
    let mut out = Vec::new();
    let skill_md = dir.join("SKILL.md");
    let text =
        fs::read_to_string(&skill_md).with_context(|| format!("reading {}", skill_md.display()))?;
    out.extend(
        frontmatter(&name, &text)
            .into_iter()
            .map(|p| format!("{name}/SKILL.md: {p}")),
    );
    let base = dir.canonicalize()?;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let rel = p.strip_prefix(dir).unwrap_or(&p).display().to_string();
            let text = fs::read_to_string(&p)?;
            let limit = if rel == "SKILL.md" {
                SKILL_LINES
            } else {
                REFERENCE_LINES
            };
            let lines = text.lines().count();
            if lines > limit {
                out.push(format!("{name}/{rel}: {lines} lines, over its {limit}"));
            }
            for target in links(&text) {
                let resolved = p.parent().unwrap_or(dir).join(&target);
                match resolved.canonicalize() {
                    Ok(r) if r.starts_with(&base) => {}
                    Ok(_) => out.push(format!("{name}/{rel}: links to `{target}`, outside the skill (breaks when the skill is uploaded alone)")),
                    Err(_) => out.push(format!("{name}/{rel}: links to `{target}`, which does not exist")),
                }
            }
        }
    }
    Ok(out)
}

fn manifest(root: &Path) -> Result<Vec<String>> {
    let plugin: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join(".claude-plugin/plugin.json"),
    )?)?;
    let market: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root.join(".claude-plugin/marketplace.json"),
    )?)?;
    let name = plugin["name"].as_str().unwrap_or("");
    let listed = market["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| p["name"] == name);
    Ok(if listed {
        vec![]
    } else {
        vec![format!("marketplace.json lists no plugin named `{name}`")]
    })
}

/// The positive control: each check must fire on a fixture built to fail it.
fn control() -> Result<()> {
    let bad = "---\nname: other\ndescription: marks a test #[ignore]\nuser-invocable: true\n---\n";
    // Wrong name, unquoted description, unknown key, no licence.
    let found = frontmatter("mine", bad);
    anyhow::ensure!(
        found.len() == 4,
        "positive control failed: the frontmatter check found {found:?}"
    );
    let good = "---\nname: mine\ndescription: \"x\"\nlicense: CC-BY-NC-SA-4.0\n---\n";
    anyhow::ensure!(
        frontmatter("mine", good).is_empty(),
        "positive control failed: a good frontmatter has findings"
    );
    // One finding per bare grant, in each form; scoped grants pass.
    for (front, bare) in [
        ("allowed-tools: Bash, Read, Bash(cargo test:*)", 1),
        ("allowed-tools: Bash(a, b:*), Edit, \"Write\"", 2),
        ("allowed-tools: [Read, Write]", 1),
        (
            "allowed-tools:\n  - Edit\n  - Bash(cargo xtask:*)\nother: x",
            1,
        ),
        ("allowed-tools: Bash(devenv test:*), Read, Grep, Glob", 0),
    ] {
        let found = grants(front);
        anyhow::ensure!(
            found.len() == bare,
            "positive control failed: allowed-tools `{front}` gave {found:?}"
        );
    }
    anyhow::ensure!(
        links("see [x](../other/SKILL.md) and [y](https://a.b)") == ["../other/SKILL.md"],
        "positive control failed: link scanner"
    );
    Ok(())
}

pub(crate) fn run(root: &Path) -> Result<usize> {
    control()?;
    let mut problems = manifest(root)?;
    let mut skills = 0;
    let mut dirs: Vec<_> = fs::read_dir(root.join("skills"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for d in &dirs {
        skills += 1;
        problems.extend(skill(d)?);
    }
    let mut commands = 0;
    let dir = root.join("commands");
    if dir.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        files.sort();
        for f in &files {
            commands += 1;
            problems.extend(command(f)?);
        }
    }
    for p in &problems {
        eprintln!("[plugin] {p}");
    }
    eprintln!(
        "[plugin] checked {skills} skills, {commands} commands: {}",
        if problems.is_empty() {
            "ok".to_string()
        } else {
            format!("{} finding(s)", problems.len())
        }
    );
    Ok(problems.len())
}
