# Primary sources for platform APIs, and long or looping work

Depth for SKILL.md sections 13 and 14. Back to [SKILL.md](../SKILL.md).
Both are written so the skill stands alone; the personal skills
native-api-first and long-task-discipline, where a setup has them, cover the
same ground at more length, and nothing here requires them.

## The trigger: a number you are about to fit

You are about to pick an alpha, a timeout, a buffer size, a radius or a
retry count by trying values until the output "looks right" or "matches".
Stop and ask which text or API already decides it.

- A fitted constant reproduces the one case it was tuned on and fails on the
  next appearance, locale, terminal, kernel or hardware.
- Worse, it hides the defect: the gap it closes was usually an API used the
  wrong way (wrong property, wrong ordering, set too late), and once the
  number papers over it nobody looks again.
- Values a platform owns -- system colours, metrics, materials, limits,
  default timeouts, buffer sizes in a protocol -- are asked for, not fitted.

## Read in authority order

Read each source whole, not a summary of it; a premise-level mistake
survives exactly as long as nobody reads the page it contradicts.

1. **The header, or the source at the pinned version.** For a C platform API
   that is the SDK or system header; for a Rust binding, the `-sys` crate's
   generated source and the crate's docs at the version in `Cargo.lock`. They
   carry availability, defaults and ordering constraints that prose omits.
2. **The vendor's reference documentation.** Many vendor doc sites render
   with script and return an empty shell to a plain fetch; look for the
   data or JSON form the site itself loads, or render the page and read it.
   An empty fetch is a failed read, not an absent page (positive control,
   SKILL.md section 4).
3. **Talks, design notes and conference transcripts** from the vendor: they
   state the rules the headers imply -- ordering, what not to do.
4. **The design or interface guide**: where a thing belongs, which is often
   the constraint that matters most.

Pin what you read (SKILL.md section 2): a header excerpt or a saved doc page
is a `retrieved` source like any other; a vendor page you may not commit is
`external` with the hash of the bytes you read. List everything you skipped
under **Not read**. If the owner hands you links, read every one and say
which you did not.

Things to check against your platform's header (practice seen on one GUI
platform, unpinned -- a checklist, not a finding):

- A container API usually has a designated content slot; placing your
  element beside it instead is often documented as unsupported and fails
  without an error.
- Some properties take effect only when set before insertion or
  construction; setting them afterwards is a silent no-op.
- Two semantic constants can be identical in one mode (light theme, one
  locale) and different in another; check every mode before deciding "it
  does not matter which".
- Check that a property is writable before planning around writing it.

## The constant carries its source

The rust-idioms skill owns the rule that a magic number becomes a named
`const` with a doc comment saying where it came from. Research adds one
thing: the doc comment cites what chose the value -- the pinned source and
clause when a text sets it (`cargo xtask sources` then checks the citation),
or the D-n and `MEASUREMENTS.md` entry when a measurement chose it (a buffer
or cache size picked by an A/B with an A/A arm, the rust-measure skill). This goes
further than BeautifulRust's magic-number rule, which asks for the origin in
prose; it does not depart from it.

```rust
/// Largest parameter count one control sequence may carry.
///
/// Set by the governing text, [[source: <key>]] clause <n>, not fitted:
/// raising it changes which inputs are rejected, so it moves only with the text.
pub const MAX_PARAMS: usize = 16;

let params = [0_u16; 17];
assert!(params.len() > MAX_PARAMS, "17 parameters exceed the limit");
```

A constant with neither a text nor a measurement behind it is a finding:
either the governing text was not read, or the value was fitted. Record which.

## Measure the reference before matching it

Before spending effort making output "match" a reference (a native widget,
another implementation's output, a capture), establish what the reference is.
Profile it -- sample across the region or the trace and look for the step
you think is there. If there is no step, there is nothing to match, and every
iteration spent matching it is fitting noise. Pin the profile as an
observation (`<key>-observed/`), so the claim "the reference has X" is
**measured**, not remembered.

## Private and undocumented symbols

If the only route is a private selector, an undocumented syscall or an
unstable ABI, say so with the evidence and let the owner decide; record the
choice as a D-n that DEPARTS from the vendor's documented surface. Never ship
it silently and never present it as the supported path. Any FFI it needs
lives in the declared quarantine crate, behind a safe API (the
rust-architecture skill owns the quarantine; the Rustonomicon's "FFI"
chapter is the primary source for the boundary itself).

## Unresolved, once

When a source cannot settle a question, write "unresolved" with what was
tried, once. Asserting a conclusion, retracting it and asserting the
opposite is worse than one honest unknown: each assertion gets quoted.

## Owner holds are hard

- An owner's "do X first", "don't touch Y" or "wait for my approval" holds
  until the owner lifts it, not until proceeding looks reasonable. Work built
  past a gate is wasted if the owner disagrees, and it removed their chance to
  steer.
- Disagree in words and wait. Ask a blocking question at most twice; then say
  it is blocking and stop, rather than asking again while working on.
- "Blocked on your choice between A and B" is a better turn than another
  table of unchanged numbers.

## Standing instructions are re-judged

A recurring prompt -- a loop, a schedule, a task line that repeats -- is
executed by choice each time, so judge it each time. Say so and propose a
change when:

- the thing it re-measures has not moved for several runs;
- each run is spent looking for something to do rather than doing the next step;
- its wording encodes an approach the owner has since rejected, or conflicts
  with a newer instruction.

## Do the asked thing

State the literal ask in one sentence before acting. The long-task failure
is substitution: asked for X, delivering analysis of X, a record about X, or
an interesting finding met on the way. A finding met en route becomes an
issue in the same turn; then return to the ask. Notes, surveys and decision
records are deliverables only when asked for, or when this skill's workflow
(propose -> refute -> write) requires one.

## Read what you commissioned

- Read a subagent's, workflow's or refuter's output in full before acting,
  and before doing work it might already cover. Save it to a file if the
  preview truncates.
- Read its "unknown", "not read" and coverage-gap sections first: they say
  which absences are gaps, not findings.
- Check that every delegated lane delivered; partial delivery reads the same
  as complete delivery in a summary.
- Disagree with it against its cited evidence; never re-derive what it
  already answers.

## A gate that cannot run

A gate that cannot run is not passed and not neutral: drift accrues and
nothing reports it. Say so in every artefact produced while it is down
(note, commit message, verdict). When it returns, the first act is to
measure what it missed. Two unrelated tools failing at once is a hypothesis
about the shared environment; check the cheapest common dependency first.

## Close the step: review the conclusion, not just the evidence

A green gate ends the work, not the step. Before a commit message, record
entry or summary goes out, re-derive its load-bearing claims (reopen the
file, redo the count), and read its last paragraph on its own: that
paragraph, about why the change matters, is the one written from memory.
Attach a number to each sentence of significance or cut it; "what this does
not do" is usually the most accurate line in it. Completeness claims ("every
call site") need the search in
[records-and-retractions.md](records-and-retractions.md), "Sweep for
expired claims", not a green suite.
