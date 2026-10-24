# Records, decisions and retractions

Where each kind of written claim lives, how it is appended, how a wrong one
is withdrawn, and how stale ones are found. Back to [SKILL.md](../SKILL.md).

## Where things go

| kind | file | loaded | changes by |
|---|---|---|---|
| standing rules | `CLAUDE.md` | whole, every session | edit, under a byte budget |
| decisions | `docs/decisions.md` | on demand | append; reverse by a new D-n |
| findings, measured changes | `docs/record.md` | on demand | append; retract in place |
| current numbers | `MEASUREMENTS.md` | on demand | replace; history in git |
| reasoning from sources | `docs/research/notes/` | on demand | status line + retractions |
| the sources themselves | `docs/research/sources/MANIFEST.toml` | by the gate | append; re-pin on change |

Anything with a date, a number or a history is a record, not a rule. A rule
is imperative, standing and short; its reason may point at a record.

## The record: `docs/record.md`

Append-only and dated. Each entry:

```markdown
## <The finding, as a sentence that would be false if the finding were> (YYYY-MM-DD)

What changed (commit). What was measured: host, power state, statistic,
repetitions, A/A spread -- or "by-reading" with the pinned place. What result
was stated in advance as the one that would prove it wrong, and whether it
came.
```

- **Headings are findings.** "Investigating the counter" says nothing;
  "Splitting the cold totals out of the hot struct changed no hot-path
  instruction (byte-identical text)" is citable and checkable.
- **Cite by heading**: "record: '<heading>'". A heading that is a finding is
  stable; a section number is not.
- **Append, never rewrite.** New entries go at the end. An old entry is
  touched only to retract a claim in it.
- **State the refutation in advance.** Written after the result, it bends to
  fit the result (rust-measure has the A/B rules).

## Retraction: strike through in place

A wrong claim stays visible, struck through, with what replaced it and why:

```markdown
~~Moving the totals out is speed-neutral (measured).~~ **Retracted
YYYY-MM-DD:** the A/A spread overlapped the delta, so there was no result;
replaced by "<new claim, labelled>" in record: '<heading>'. Why it was
wrong: <the step in the method that failed>.
```

- Never delete: deletion hides that the method failed once, and the reader
  who quoted the old claim cannot find out it was withdrawn.
- Retract in every place the claim was repeated (the sweep below finds them).
- Name the failed step ("read a miss without a control", "promoted an
  inference"), not just the wrong value -- the step is what recurs.
- Repeated retractions are a method defect, not bad luck, and the two shapes
  need opposite responses. **Oscillation** (one conclusion withdrawn in both
  directions more than once): the method is broken -- return to a known-good
  reference and bisect forward instead of varying more things. **Drift** (the
  same class of claim wrong the same way, e.g. "dead", "absent", "never
  called" three times over): a bias that careful case-by-case reading will
  not find -- stop fixing instances and make one exhaustive pass over the
  whole class, with a verifier told to argue the opposite (find the caller).
- Before eliminating a hypothesis, confirm the test exercised it: a change
  that did not take effect, a build that failed, or a probe on the wrong
  region all look like clean negatives.

A retraction that recurs becomes a standing check: an identification of a
hardware-counter column from its magnitude alone, withdrawn more than once,
becomes the rule "identify every column with control programs".

## Decisions: `docs/decisions.md`

```markdown
### D7. <The claim, as a sentence>
**Evidence:** note C (by-reading, [[source: <key>]] clause N); record:
'<heading>' (measured).
**Cost:** <what it gives up, including any departure: "DEPARTS from <source>
clause N: <why>">
**Reverses if:** <an observation someone can make>.
```

- **Reverses if** names an observation, not a feeling. Bad: "if it turns out
  slower". Good: "if an A/B with an A/A arm on <workload> gives disjoint
  ranges with the alternative ahead by more than the row's floor" (rust-measure;
  for a whole-process timing, `cargo xtask bench --aa` prints that verdict).
- Numbers are never reused. A reversed decision is struck through and points
  forward: "~~D7 ...~~ Reversed by D12 (YYYY-MM-DD): <the observation that
  fired>". The new decision points back.
- An owner decision quotes the owner verbatim, dated, in **Evidence**
  ([judge-protocol.md](judge-protocol.md)).
- A decision still waiting on the judge or owner is not in the register; it
  is an Open item in its note.

## Tried and lost

Keep a heading in the record, "Things tried that lost", one line per
attempt: what, the measured result, the conditions (commit, host, workload,
toolchain), the date, and the record heading with the detail.

- It stops the same idea being re-tried blind.
- **Distrust it too.** A loss is a measurement under its conditions; when the
  conditions change (layout, toolchain, workload, hardware), re-measure
  before citing it. "We tried that" without conditions is not evidence.

Record a loss with its reason and its conditions, so the next reader knows
what would change it: which workloads a prefetch lost on, and why (each
prefetch is one more load, competing with the demand loads).

## Sweep for expired claims

A claim labelled unmeasured is out of date once something measures it.

Sweep whenever one of these lands: a measurement, a re-pinned source (new
sha256), a reversed decision, a toolchain or dependency bump.

1. Grep the docs and code comments for the subject (type, function,
   workload) and for the labels `unmeasured`, `inferred`, `Open`, `Reverses
   if`.
2. List every hit before changing any: the list is the work plan. A green
   suite finds only what it already covers, so it cannot back "that was all
   of them"; only a search listed before the fix can.
3. For each: still true, update with the new label, or strike through with
   the correction.
4. Record the sweep itself in the record entry that triggered it: which
   claims changed, which were checked and held.

## Rules vs records: the CLAUDE.md budget

`CLAUDE.md` is loaded into every session whole, so every byte is paid on
every task. `budgets.toml` holds `[docs] claude_md_bytes`, and
`cargo xtask docs` (`test:docs`) fails when the file is:

- **over** budget -- move records out to `docs/`, or raise the budget in a
  commit of its own that says why;
- **under** budget -- lower the budget in the same commit, so the ratchet
  only goes down and a later growth is visible.

What to cut first: anything with a date or a number (a record), any
reasoning longer than one clause (a note or a skill), any list of files (the
docs gate checks paths; `docs/README.md` indexes them).

## Documents are tested claims

- `cargo xtask docs` fails on a relative Markdown link that does not resolve
  and on a backticked repo path that does not exist -- so rename a file and
  the docs that name it fail the gate.
- A coverage or status row says `done` only when it names a test that would
  fail on regression (rust-qa holds the meta-tests).
- Prose with no gate drifts: a doc naming "the one module that needs
  unsafe" stays unchanged while other modules start to allow it.
