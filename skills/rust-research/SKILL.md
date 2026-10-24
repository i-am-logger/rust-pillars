---
name: rust-research
description: "Use before deciding what a spec, a measurement or the owner should settle: choosing between designs, implementing a protocol or format, citing a source or another implementation, writing a research note, decision record or retraction, or asking a judge to pick an option."
license: CC-BY-NC-SA-4.0
---

# rust-research: standard first, evidence labelled, decisions reversible

A research step turns sources into a decision that names what would undo it.
Notes, decisions and records are files in the repo, checked by gates where a
gate can check them. Measurement method is the rust-measure skill; the
test-side refuter checklist is the rust-qa skill. The gates named here ship
in rust-template projects; elsewhere, check by hand and say no gate held it.
Stands alone: native-api-first and long-task-discipline, if your setup has
them, go deeper on sections 13-14; nothing needs them.

## 1. Standard first

- Quote the governing text, verbatim, with its clause or page, before writing
  code or reading practice -- the quote is what every option is judged against.
- Mark every option **CONFORMING** or **DEPARTS** against that quote; a
  departure is a defect unless the judge protocol (section 9) or the owner
  accepts it, recorded as a D-n naming the text (section 6).
- A result that is faster or cleaner *because* it departs is a defect, not a
  win: the text sets the requirement, and speed must fit inside it.
- Write "DEPARTS from <source>: <why>" wherever a rule or design leaves a
  guideline (Rust API Guidelines item, Cargo reference, a protocol spec).

## 2. Pinned sources -- gate `cargo xtask sources` (`test:sources`)

- Pin every cited source in `docs/research/sources/MANIFEST.toml` with its
  sha256 -- a citation is to the bytes you read, not to a URL that can change.
- Status `retrieved` (committed, re-hashed every run), `external` (read and
  hashed, not committed: licence or size), `gated` (never obtained: **not
  citable**; the gate fails on any citation of it).
- Cite as `[[source: <key>]]` + clause in `.md`/`.rs`; the gate fails on an
  unknown, gated or malformed key, a bad hash, or an unlisted file.
- Pin your own observations too (captures, tool output): each file under
  `<key>-observed/` is its own `retrieved` entry, `file = "<key>-observed/<name>"`
  -- the gate checks every file, and an unhashed observation cannot be re-checked.
- In examples and prose about the marker, write `<key>` in angle brackets:
  the scanner skips it; any real-looking key must resolve.
- A source you have not pinned is "unpinned": name it, never let it carry a
  decision. Effective Rust is CC BY-NC-ND: link it, never quote or digest it.
- Detail: [notes-and-sources.md](references/notes-and-sources.md).

## 3. Practice is not the standard

- Read other implementations at pinned commits and lines, and record what they
  do as **practice** -- evidence of what people ship, not of what is correct.
- Where practice departs from the text, record the departure; never follow it
  for that reason. Agreement between two implementations is not a quote.

## 4. Positive control before believing a miss

- Before reading "no hits" as "absent", search for a term you know is there;
  a source you cannot search is not a source you have read.
- The same holds for tools: every `cargo xtask` gate checks a known-bad
  fixture first (gate-side rules: rust-qa).
- Example: every search of a letter-spaced PDF misses; a control term shows why.

## 5. Notes -- start from `docs/research/notes/TEMPLATE.md`

- One question per note, lettered, as a sentence; a dated status line
  (`research | decided | built`) saying what is built -- a note without one
  reads as current when it is not.
- Sections in order: the answer in short; the governing text; practice;
  options (CONFORMING/DEPARTS, cost); how it was checked; **Not read**; Open.
- "Not read" lists what you skipped, so a reader knows which absence is a
  gap and which is a finding.
- Label every claim (section 7). Judge figures go in the note.

## 6. Decisions -- `docs/decisions.md`

- Each decision is `D-n`, a claim as a sentence, then **Evidence** (measured or
  by-reading, said which), **Cost** (what it gives up), **Reverses if** (the
  observation that undoes it) -- a decision without a reversal condition
  cannot be revisited honestly.
- A decision that departs from a governing text names the text.
- Undecided by evidence -> judge protocol (section 9) -> owner.

## 7. Claim labels -- every number and every "X does Y"

- **measured**: you ran it; name commit, host, statistic, A/A spread
  (rust-measure). **by-reading**: you read the code or text, at a pinned
  place. **inferred**: reasoned, not observed. **unmeasured**: say so and stop.
- Never upgrade a label in prose: an inference stated as fact is a false claim.
- Example: "owned storage is speed-neutral if hot structs stay byte-identical"
  stays *inferred* until its own arm is measured.
- "Unmeasured -- do not assert" beats an estimate: an estimate gets quoted as
  a measurement two documents later.

## 8. Workflow: propose -> refute -> write

- Propose (note draft or diff) -> a refuter with a **closed brief** tries to
  break it -> triage each finding -> only then write the decision or record.
- Closed brief: the refuter gets the claim, the pinned texts and the gates --
  not your reasoning, so it cannot inherit your framing.
- Verdict comes back as data, not prose. Block on code for a code subject;
  for a note, on quotes, citations, labels, controls and CONFORMING/DEPARTS marks.
- Brief template, the one verdict schema and triage:
  [refuter-brief.md](references/refuter-brief.md); code-side rules R1-R8 are rust-qa's.
- Before handing back: if you changed anything a gate reads -- the manifest, a
  pinned file, `Cargo.toml`, code -- run the whole suite (`devenv test`; else
  clippy `-D warnings`, the tests and `cargo xtask all`), not `sources` alone,
  and report what it printed.

## 9. Judge protocol -- any judge: LLM subagent, external model, human

- Ask twice, uncached (fresh context each time), options **reordered** -- a
  judge that picks whichever comes second carries no signal.
- Attach the principles (quoted, with keys) and the evidence; attach renders
  for any visual question -- a judge cannot rate what it cannot see.
- Accept only the same choice both times at confidence **>= 0.9** both times.
- Below that: research what is missing and re-ask, never unchanged; the
  owner gets only what no fact settles, a pure preference directly.
- Framing that leaks the answer ("the faster one", "our fix") voids the
  support; record that the ask was void and re-ask clean.
- Detail, ask template, judges that return only probabilities: [judge-protocol.md](references/judge-protocol.md).

## 10. The owner

- The owner holds a veto over any judge, refuter or measurement verdict.
- Quote the owner's words verbatim with the date, never paraphrase -- a
  paraphrase drifts toward what the writer wanted to hear.

## 11. Records -- `docs/record.md`, append-only

- Dated entries; each heading states the finding as a sentence; cite by
  heading. Never rewrite history: git keeps it, the record shows it.
- A wrong claim is struck through in place with the correction, the date and
  the reason -- deleting it hides that the method failed once.
- Keep a "tried and lost" list with conditions; distrust it too -- re-measure
  before citing it under changed conditions.
- Sweep for expired claims whenever a measurement lands: an honest
  "unmeasured" becomes false the moment somebody measures it.
- Detail: [records-and-retractions.md](references/records-and-retractions.md).

## 12. Rules vs records -- gate `cargo xtask docs` (`test:docs`)

- `CLAUDE.md` holds standing rules only; it is loaded whole into every
  session, so `budgets.toml` `[docs] claude_md_bytes` pins its exact size and
  the gate fails over budget *and* under (lower the budget in that commit).
- Records go to `docs/record.md`, numbers to `MEASUREMENTS.md`, decisions to
  `docs/decisions.md`, reasoning to notes.
- Documents are claims about the tree: `cargo xtask docs` fails on a dead
  relative link or a backticked repo path that does not exist.

## 13. Platform APIs: the primary source owns the value

- Before fitting a number until output "matches", find the spec, header,
  `-sys` binding source or vendor reference that already sets it -- a fitted
  constant matches one case and hides the API misuse that caused the gap.
- Read the primary source whole, in authority order (header or source at the
  pinned version, vendor reference, design guide); list the rest in **Not read**.
- Cite in a non-obvious `const`'s doc what chose it: `[[source: <key>]]` + clause,
  or the D-n / `MEASUREMENTS.md` entry; neither is a finding (naming: rust-idioms).
- Measure the reference before matching it -- a feature it does not have
  cannot be matched, only imitated.
- An undocumented or private symbol is a DEPARTS the owner decides; the FFI
  goes in the quarantine crate (the rust-architecture skill).
- Detail: [primary-sources-and-long-work.md](references/primary-sources-and-long-work.md).

## 14. Long and looping work

- An owner's "do X first" or "wait for my approval" holds until the owner
  lifts it -- work past an owner hold is work the owner could not steer.
- Re-judge a standing instruction (loop, schedule, repeated task) every run:
  when it stops moving anything, say so and propose a change instead of running it again.
- Read commissioned output (a subagent, a workflow, a refuter) in full, its
  gaps section first, before acting -- a skimmed report gets re-derived badly.
- A gate that cannot run is accruing debt: say so in every artefact made while it is down.
- Detail: [primary-sources-and-long-work.md](references/primary-sources-and-long-work.md).

## Departures

- DEPARTS from BeautifulRust: no research method, and self-graded evals tuned
  on themselves -- here a judge counts only through section 9's protocol.
- The judge process is versioned in the repo, and any judge plugs in.
