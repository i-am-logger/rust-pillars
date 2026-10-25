# The closed-brief refuter

Propose -> refute -> write. The refuter is a separate agent (a fresh
subagent, another model, or a human) whose only job is to break the claim,
and whose verdict comes back as data. Back to [SKILL.md](../SKILL.md).
The test-side checklist (perturbation kinds, property and oracle checks)
is in **rust-qa**; this file is the brief and the verdict format.

## Closed means

The refuter gets:

- **the claim**, verbatim: what the builder says is done or true;
- **the subject** at a pinned commit: the diff, or the note's path;
- **the governing texts**, as `[[source: <key>]]` keys plus clauses, pinned in
  `docs/research/sources/MANIFEST.toml`;
- **the gates** and how to run them (`devenv test`, single `cargo xtask`
  gates);
- **the rules** below.

The refuter does **not** get the builder's transcript, reasoning, account of
what was hard, its own test results, or anyone's preference. A refuter that
reads the builder's argument inherits its framing -- the same failure as
leaked framing in the judge protocol -- and a refuter that reads the
builder's counts checks the arithmetic, not the tree.

## Rules the refuter follows

For a code subject, the refuter rules are rust-qa's R1-R8
(`references/refuter-checklist.md`): red first on the base, every read size
and split, two compiling perturbations per mechanism, gates on a pristine
`git archive`, counts taken not stated, every sentence a claim, block only on
code, check the gates themselves, stay in the brief.

For a research note the claims are the deliverable, so these block: a quote
that does not match its pinned clause, a citation that does not resolve, a
label stronger than its evidence, a miss read as absence without a positive
control, an option not marked CONFORMING or DEPARTS. Wording does not.

Every finding carries the command or steps that show it, so triage can
rerun it.

## The verdict, as data

```json
{
  "verdict": "CODE-READY",
  "code": [
    {
      "where": "<path>:<line> or gate name",
      "rule": "<rust-qa R1-R8, or the note rule broken>",
      "claim": "<the sentence or behaviour checked>",
      "observed": "<what actually happened>",
      "reproduce": "<command or steps>"
    }
  ],
  "prose": [
    { "where": "<file or message>", "claim": "<sentence>", "observed": "<why wrong>" }
  ],
  "checked": [
    { "what": "<claim or mechanism>", "how": "<command, perturbation, split>", "result": "<count or outcome>" }
  ]
}
```

- `verdict` is `CODE-READY` or `CODE-NOT-READY` for a code subject.
  rust-code adds `NOTE-READY` / `NOTE-NOT-READY` for a research
  note, with blocking findings in `code` (rule 7 says which).
- `CODE-NOT-READY` / `NOTE-NOT-READY` iff `code` is non-empty.
- `checked` lists what was tried even when nothing failed: a verdict with an
  empty `checked` is no verdict.
- This is the one schema for every refuter run (rust-qa points here). A
  workflow parses the JSON; a verdict that does not parse is a failed run,
  not a pass.

## Brief template

```text
You are a refuter. Your job is to show the claim below is false or
unproven. You have not seen how it was built and must not ask.

Claim (verbatim):
<claim>

Subject: <commit> -- <diff range or note path>

Governing texts (pinned; read the clause, not your memory of it):
- [[source: <key>]] <clause>

Gates: run `devenv test` on a `git archive` export of <commit> in a fresh
directory. Single gates: `cargo xtask <gate>`.

Rules:
1. Reproduce red on <base commit> before accepting any fix.
2. Stream code: every read size 1..64 and 4096, and every two-read split.
3. At least two perturbations per mechanism; a perturbation that does not
   compile is unbuilt, not green.
4. Take every count yourself; never copy a number from the claim.
5. Check every sentence of the claim, or list it as not checked.
6. Block only on code (or, for a note: quotes, citations, labels, controls,
   CONFORMING/DEPARTS marks).
7. Give a reproduce command for every finding.

Return only this JSON:
{"verdict": "CODE-READY|CODE-NOT-READY|NOTE-READY|NOTE-NOT-READY",
 "code": [{"where","rule","claim","observed","reproduce"}],
 "prose": [{"where","claim","observed"}],
 "checked": [{"what","how","result"}]}
```

## Triage

The builder answers every `code` item with one of:

- **fixed**: the commit, and the test or gate that now goes red without the
  fix (red first applies to the builder too);
- **refuted**: evidence at the refuter's standard -- a reproduce command
  whose output contradicts the finding;
- **open**: moved to the note's Open section or an issue, with the
  observation that would close it.

A disagreement that evidence does not settle goes to the judge protocol
([judge-protocol.md](judge-protocol.md)), then the owner, who holds a veto.

Then a **new** closed brief goes to a fresh refuter with the fixed subject;
the first refuter's verdict is not in it. Only after a READY verdict is the
decision, record entry or commit message written (propose -> refute ->
write). `prose` items are fixed in the same pass or listed as not fixed.

## Why closed, and why data

The process is builder -> closed-brief refuter -> triage, with the rules above
and the `{verdict, code, prose, checked}` schema. The brief, the rules
and the schema are versioned in the repo, so a refuter run is repeatable on
any machine and its verdict can be parsed by a workflow script.
