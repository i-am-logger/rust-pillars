# The QA process and the refuter checklist

Load this when running QA on a change: as the builder handing work over, as
the refuter, or when triaging a verdict.

Status: the process is specified here; the versioned driver
`.claude/workflows/lanes.js` in rust-template ships the process as a workflow. rust-pillars versions it in the repo and makes
the judge pluggable (an LLM subagent, a named judge, or a human).

## 1. Three roles

```
builder ──(claim + diff + gate list)──▶ refuter ──(verdict as data)──▶ triage
   ▲                                                                     │
   └────────────────────────── fixes / rebuttals ◀───────────────────────┘
```

**Builder** writes the change and a claim: what it does, which tests prove
it, which gates must be green, and every number it states.

**Refuter** gets a **closed brief**: the claim, the diff (base and head
revisions), the gate commands, and this checklist. Not the builder's
reasoning, transcript or explanations. Why: a refuter who reads the
argument tends to check the argument; one who reads only the claim checks
the code.

**Triage** (the owner, or the builder under the owner's rules) marks each
finding fixed, refuted (with a command and its output) or open (with the
observation that would close it), as rust-research's triage defines.
Nothing is dismissed without evidence.

## 2. Refuter rules

### R1. Red first, on the base
For every new or changed test that the claim says proves a fix or feature:
check it out on the **base** revision and run it. It must fail, for the
intended reason (see [gates-that-can-fail.md](gates-that-can-fail.md)). A test
green on the base proves nothing about the change.
- Stream/protocol code: red first at every read size (1..=64, 4096) and every
  two-read split the test exercises, not one convenient split.
- If the test does not exist on the base, port only the test (not the fix)
  onto the base and run it.

### R2. At least two perturbations per mechanism
For each mechanism the change adds (a check, a counter, a refusal, a cache,
a branch), perturb it at least two different ways and confirm a test goes
red each time: delete the check, invert the comparison, off-by-one the bound,
skip the counter increment, return early.
- A perturbation that does **not compile** is **unbuilt**: it is neither red
  nor green and does not count toward the two. Make one that compiles.
- A perturbation no test catches is a code finding: the mechanism is
  untested (cargo-mutants reports the same as "missed").

### R3. Gates on a pristine archive
Run every gate in the claim on a clean export of the head revision, never in
the builder's working tree:
```sh
dir=$(mktemp -d)
git archive HEAD | tar -x -C "$dir"
cd "$dir" && devenv test
```
Why: untracked fixtures, a stale `target/`, a gitignored regression file or a
local config can make a tree green that a clone is not. (rust-measure uses the
same `git archive` rule for benchmark arms.)

### R4. Counts are taken, not stated
Every number in the claim — tests passed, cases run, fixtures checked,
budgets, sizes, timings — is re-measured by the refuter from tool output.
"800 tests pass" with 799 in the output is a finding. Quote the command and the line.

### R5. Every sentence is a claim
Each sentence of the claim, the commit message, and changed docs is checked:
a named path exists, a named test exists and runs, a stated behaviour is
exercised by a test, a "measured" figure has a row in `MEASUREMENTS.md`, an
"inferred" figure is not called measured.

### R6. Block only on code
`CODE-NOT-READY` only for findings in code or tests: a wrong behaviour, an
untested mechanism, a gate that cannot fail, a test that does not run, a
number that does not reproduce from the code. Prose findings (a doc sentence
wrong, a comment stale) are reported in `prose[]` and fixed, but do not block.
Why: a refuter that blocks on wording trains builders to write less, not
better.

### R7. Check the gates themselves
For each gate the change adds or relies on: does it have a positive control?
Is there a plant that turns it red? Does it skip anything on some platform?
Is every floor exact? Is any test `#[ignore]`d, and is it counted?

### R8. Stay in the brief
Do not fix anything. Do not read the builder's notes. Do not accept "this is
covered elsewhere" without the test name and a run.

## 3. Verdict and triage: rust-research owns them
The closed brief, the verdict JSON schema (`{verdict, code[], prose[],
checked[]}`, one `code` entry per finding with its rule id R1-R8 and a
reproduce command) and the triage outcomes (fixed / refuted / open, then a
fresh refuter) live in rust-research `references/refuter-brief.md`. One
schema, so one workflow can parse every verdict. Disagreements that
evidence does not settle go to rust-research's judge protocol, then the
owner, who holds a veto.

## 4. Builder's pre-flight (before handing over)
- [ ] `devenv test` green on a `git archive` of HEAD.
- [ ] Every new test seen red on the base, for the intended reason.
- [ ] Every mechanism perturbed at least twice, compiling, each caught.
- [ ] No new `#[ignore]` without a reason and a count; no skip-and-return.
- [ ] Every number in the claim copied from tool output, with the command.
- [ ] `test:mutants-diff` green: no missed mutant on a changed line of
      domain or core (it runs in `devenv test`; shipped).
- [ ] Every label honest: measured / by-reading / inferred / unmeasured.
