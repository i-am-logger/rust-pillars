# Refactoring someone else's code

Load this when the request is to clean up, modernise or review code that you did not write.
The job is to make the code better without changing its contract, so that its callers notice
only the improvements they asked for.

## 1. The order of work

1. **Pin the current behaviour with tests before you edit anything.** Feed the code real inputs,
   including its edge cases and its failures. Assert the exact outputs: the returned values, the
   error variants and the output bytes. Why: without these tests, "behaviour kept" is a claim and
   not a fact. A pinning test must fail when you plant a change in the behaviour (rust-qa, "gates
   that can fail").
2. **List the smells that the request is about** (section 2). Fix all of them, the costly ones
   too. A smell that the request does not cover goes in the hand-back as a suggestion. Why:
   an unasked-for rewrite is a review burden, and it hides the change that was asked for.
3. **Keep the public surface** (section 3) unless the request asks for the change.
4. **Keep quirks that you cannot explain, and flag them** (section 4).
5. **Run the pinning tests and `devenv test`.** The pinning tests stay in the tree.
6. **Hand back a change list.** It shows each signature change before and after, with its reason
   (section 5).

## 2. Smells and their fixes

| Smell | Fix | Gate |
|---|---|---|
| `&String`, `&Vec<T>` or `&Box<T>` parameter | `&str`, `&[T]` or `&T` (C-CALLER-CONTROL) | clippy `ptr_arg` (style, denied) |
| string built with `+` or a chain of `push_str(&format!(..))` | `write!` into one `String`, `format!` once, or `join` | review: `string_add` is a restriction lint and is off |
| check, then `unwrap` (`if x.is_some() { x.unwrap() }`) | `if let`, `let ... else` or `match` | clippy `unnecessary_unwrap` (complexity); `unwrap_used` |
| `unwrap` on a fallible path | return the error, or `expect("why this holds")` for a real invariant | clippy `unwrap_used` |
| output whose order comes from iterating a `HashMap` or `HashSet` | `BTreeMap`, or collect and sort before writing | review: `iter_over_hash_type` is a restriction lint and is off; a pinning test that runs the output twice catches it |
| `bool` parameter | a two-variant enum named for the choice | domain and core: `bool_param_or_field` budget; elsewhere review (`fn_params_excessive_bools` is pedantic) |
| `new` returns `Rc<RefCell<Self>>` or `Arc<Mutex<Self>>` | return `Self`, and let the caller choose how to share it | review (the "no `Rc`/`Arc` in a public signature" rule) |
| error type without `Display` and `Error` | implement both: `thiserror` in adapters, by hand in core and in libraries for others (C-GOOD-ERR) | review |
| `#![deny(warnings)]` in source | delete it; lints live in `[workspace.lints]` | review; `cargo xtask arch` checks `[lints] workspace = true` |
| clone that exists only to satisfy the borrow checker | the ladder in ownership.md §2 | review; clippy `redundant_clone` is nursery and off |
| `get_` getter | drop the prefix (C-GETTER) | review |

## 3. Keep the public surface

Unless the request is to change it, every public item keeps its name, its path, its signature
and the traits it implements. A change listed below is a semver break for a library for others.
Inside one workspace, it is still a change that every caller has to absorb.
- **Do not drop an error variant.** A caller may match on it. Merge variants only when asked to.
- **Do not remove a derive or a trait impl.** Someone may use `Clone`, `Hash`, `Serialize` or
  `Default` in code that you cannot see. Adding one is fine where its meaning holds (rust-domain:
  "a derive is a promise").
- **Do not change what a constructor takes.** Changing `new(path: &str)` to
  `new(path: PathBuf)`, or adding a parameter, breaks every call site. Add `with_*` or a
  builder instead (api-design.md §5).
- **Do not narrow behaviour.** The same inputs must succeed and the same inputs must fail. The
  one welcome exception is a panic on bad input that becomes an `Err`, because callers want that
  change. Show it in the change list all the same.
- For a library for others, run `cargo semver-checks` before handing back. Gate: review, because
  it is not in the template.

## 4. Quirks

A quirk is behaviour that looks wrong and has no visible reason. Examples are an off-by-one in
an output format, a silently skipped empty line, or a sort that is not quite alphabetical.
- Keep it. Pin it with a test whose name says what it is, such as
  `quirk_trailing_blank_line_is_dropped`. Why: a quirk is often a contract with a caller that
  you cannot see, such as a parser downstream or a golden file somewhere else.
- Flag it in the hand-back. Say what it is, where it is, and what the fix would be if it turns
  out to be a bug.
- Fix it only when the owner says that it is a bug. The fix then goes in its own commit, and the
  pinning test changes in that same commit.

## 5. Show signature changes before and after

The change list shows every changed signature as a pair, with its reason:

```text
- pub fn render(names: &Vec<String>, verbose: bool) -> String
+ pub fn render(names: &[String], detail: Detail) -> String
  why: ptr_arg (every caller can still pass &vec); a bool parameter becomes a named enum
```

## 6. Worked example

The function before the change. It has four smells: a `&Vec<String>` parameter, a `bool`
parameter, a check followed by `unwrap`, and string building with `+`.

```rust
pub fn render_before(names: &Vec<String>, verbose: bool, title: Option<&str>) -> String {
    let mut out = String::new();
    if title.is_some() {
        out = out + title.unwrap() + "\n";
    }
    for (i, n) in names.iter().enumerate() {
        if verbose {
            out = out + &(i + 1).to_string() + ". " + n + "\n";
        } else {
            out = out + n + "\n";
        }
    }
    out
}

// The pinning test, written first and run against the old code.
let names = vec!["ada".to_owned(), "grace".to_owned()];
assert_eq!(render_before(&names, true, Some("crew")), "crew\n1. ada\n2. grace\n");
assert_eq!(render_before(&names, false, None), "ada\ngrace\n");
```

The same function after the change. The output bytes are the same, and the pinning test now
runs against it unchanged except for the new signature.

```rust
use std::fmt::Write as _;

/// How much each line of [`render`] shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    /// The name only.
    Plain,
    /// A 1-based number, then the name.
    Numbered,
}

/// Renders one name per line, under an optional title line.
pub fn render(names: &[String], detail: Detail, title: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(t) = title {
        out.push_str(t);
        out.push('\n');
    }
    for (i, n) in names.iter().enumerate() {
        match detail {
            // Writing into a String cannot fail.
            Detail::Numbered => writeln!(out, "{}. {n}", i + 1).expect("String write"),
            Detail::Plain => writeln!(out, "{n}").expect("String write"),
        }
    }
    out
}

let names = vec!["ada".to_owned(), "grace".to_owned()];
assert_eq!(render(&names, Detail::Numbered, Some("crew")), "crew\n1. ada\n2. grace\n");
assert_eq!(render(&names, Detail::Plain, None), "ada\ngrace\n");
```

The output whose order comes from a hash map gets its own pinning test, which renders twice
and also asserts the order:

```rust
use std::collections::BTreeMap;
use std::fmt::Write as _;

fn tally(words: &[&str]) -> String {
    // BTreeMap: iteration is in key order, so the output is the same on every run.
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for w in words {
        *counts.entry(w).or_default() += 1;
    }
    let mut out = String::new();
    for (w, c) in &counts {
        writeln!(out, "{w} {c}").expect("String write");
    }
    out
}

let words = ["b", "a", "b"];
assert_eq!(tally(&words), "a 1\nb 2\n");
assert_eq!(tally(&words), tally(&words));
```

Whether the old `HashMap` order counts as a quirk to keep is a judgement call. No caller could
have relied on it, because it changes from run to run, so making it sorted is a fix. Flag it
anyway.
