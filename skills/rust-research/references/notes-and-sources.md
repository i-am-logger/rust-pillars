# Notes and sources

How a source becomes a pinned file, how a note turns pinned sources into an
argument, and how every claim in it is labelled. Back to
[SKILL.md](../SKILL.md).

## The manifest

`docs/research/sources/MANIFEST.toml` lists every source a note or a code
comment cites. `cargo xtask sources` (devenv task `test:sources`, second in
the chain, so it fails before clippy runs):

- re-hashes every `retrieved` file and fails on a mismatch;
- fails on any non-gated entry without a 64-hex-digit lowercase sha256;
- fails on any file in the sources directory that no entry names (except
  `MANIFEST.toml` and `README.md`);
- fails on any citation marker whose key is not listed, or is `gated`;
- runs a positive control first: an unknown citation and a short digest must
  be caught, or the gate fails before it reports anything.

An entry:

```toml
[sources.example-spec]
title = "Example Protocol Specification, 5th edition"
url = "<the URL you read it at>"
status = "retrieved"
file = "example-spec.pdf"
sha256 = "<64 lowercase hex digits of the bytes you read>"
```

| status | committed | sha256 | citable | use for |
|---|---|---|---|---|
| `retrieved` | yes, as `file` | re-hashed every run | yes | anything the licence lets you commit |
| `external` | no | of the bytes you read | yes | licence or size forbids committing; the hash still pins *which* bytes |
| `gated` | no | none | **no** | paywalled, withdrawn, never obtained -- recorded so nobody cites it by memory |

Rules:

- Hash the file you actually read (`shasum -a 256 <file>` on macOS,
  `sha256sum <file>` on Linux), not a file you downloaded again later: a
  re-download can be a different revision under the same URL.
- A key is lowercase letters, digits, `.`, `_`, `-`. The scanner ignores a
  marker whose key does not match, which is why examples write `<key>`.
- Pin your own observations as sources too -- a capture, a tool's output, a
  dump of a system setting -- under a `<key>-observed/` directory. Each file
  there is its own `retrieved` entry with `file = "<key>-observed/<name>"`:
  the gate walks the directory recursively and fails on any file no entry
  names. An observation you cannot re-hash cannot be re-checked.
- Pin other implementations by commit and path, not by branch: practice at an
  unpinned branch is a moving target.
- A licence that forbids derivatives (Effective Rust is CC BY-NC-ND) means:
  link and cite by section; never quote or digest it.

## Citing

In Markdown: `[[source: <key>]] section 8.3` -- the key plus a clause, page
or line range. A citation without a clause makes the reader re-search the
whole source, and the re-search is where misreadings come from.

In Rust, cite in a plain `//` comment, not a `///` doc comment: rustdoc
parses doc comments as Markdown, so a placeholder like `<key>` trips
`rustdoc::invalid_html_tags` under `test:doc`'s `-D warnings`, and a real citation would
leak into the public API docs. The comment says which way the code goes
against the text:

Excerpt -- the caller is the table builder (`build()` in the template's
domain crate), so `reference` is not dead code there:

```rust
/// What a byte means to a line counter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ByteClass {
    /// Ends a line.
    Newline,
    /// Separates words.
    Space,
    /// Part of a word.
    Other,
}

// The separator set is the governing text's, quoted with its clause in the
// research note: [[source: <key>]] clause N. CONFORMING.
const fn reference(b: u8) -> ByteClass {
    match b {
        b'\n' => ByteClass::Newline,
        b' ' | b'\t' | b'\r' | 0x0b | 0x0c => ByteClass::Space,
        _ => ByteClass::Other,
    }
}
```

(The template's domain crate builds its 256-entry class table from a
function of this shape and tests every input against it; the citation is what
says *why* `0x0b` is in the set.)

## Reading a source: positive control first

A miss means "absent" only after the same search, on the same extraction,
has hit something you know is there.

1. Pick a control term you know the source contains (a heading from its
   table of contents, a term on a page you have seen).
2. Search for it with the exact method you will use for the real term.
3. Control misses -> the extraction is broken (letter-spaced or ligature
   text, scanned pages, hyphenation). Change method -- render the pages and
   read them -- and repeat the control.
4. Record in "How it was checked": method, control term, where it hit.

Example: searches of a letter-spaced PDF all miss, and the misses read as
absence. One control search would have shown the extraction could not find
anything.

The same applies to code: a grep that returns nothing on a renamed function,
a macro-generated item, or a different crate path is not proof of absence.
A completeness claim ("every call site") needs a search whose hits are
listed and ruled in or out before the fix, not a suite that went green; the
procedure is in [records-and-retractions.md](records-and-retractions.md),
"Sweep for expired claims".

## The note

Copy `docs/research/notes/TEMPLATE.md` to
`docs/research/notes/<letter>-<slug>.md`. The letter is the handle
("note C") used in decisions and records.

| section | what goes in | failure it prevents |
|---|---|---|
| title | the question, as a sentence | a note that answers something nobody asked |
| status line | `research`, `decided` or `built`, a date, what is built in one line, and "every number is measured unless it says inferred" | an old note read as current |
| The answer, in short | 1-5 lines, labelled | the reader stops after the summary -- so the summary must carry labels |
| The governing text | verbatim quote, `[[source: <key>]]` + clause; how it was read; the positive control | arguing from memory of a spec |
| Practice | table of implementations at pinned commits/lines, each CONFORMING or DEPARTS against the quote | following popular practice that departs |
| Options | each option, CONFORMING/DEPARTS, its cost, its measurement or "unmeasured" | a faster departure chosen as a "win" |
| How it was checked | commands, controls, judge figures, refuter verdict | an unrepeatable conclusion |
| Not read | sources, sections, implementations skipped and why | a gap read as a finding |
| Open | what remains, each with the observation that would close it | an open question silently treated as closed |

Practice table:

| implementation | pinned at | where | does | vs the text |
|---|---|---|---|---|
| `<name>` | `<commit>` | `<path>:<line>` | what it does, one line | CONFORMING / DEPARTS (clause) |

Practice can tell you what breaks if you conform (programs that depend on a
departure), which is a **Cost** in the decision, never a reason to depart
silently.

Status line changes are edits to one line plus a record entry; the rest of
the note keeps its history through retractions (see
[records-and-retractions.md](records-and-retractions.md)).

## Claim labels

Every number and every statement of what code or a system does carries one:

| label | means | must name |
|---|---|---|
| measured | you ran it and read the result | commit, host, statistic, repetitions, A/A spread (rust-measure) |
| by-reading | you read the code or text | the pinned place: `[[source: <key>]]` clause, or commit + path + line |
| inferred | reasoned from other facts, not observed | the facts it rests on, and the measurement that would settle it |
| unmeasured | nobody has looked | nothing -- and nothing may be built on it |

- A label belongs to a claim, not to a document: one paragraph can hold all
  four.
- Never upgrade a label by restating: "X is neutral" after "we expect X to be
  neutral" is an inference promoted to fact.
- An unpinned source (a URL you read but did not hash, a memory of a spec)
  can at most support "inferred", and the note says "unpinned" next to it.
- When a label changes (an inference gets measured), strike the old claim
  through and write the new one; then sweep (records reference) for every
  other place that repeated it.

Example, each claim with its label:

- inferred: owned storage would be speed-neutral if hot structs stay
  byte-identical (a read-only analysis);
- measured: an A/B row in which bytes that nothing reads moved a hot
  function and changed its timing.

The inference names its condition; the measurement shows why the condition
is load-bearing. Neither may be quoted without its label.
