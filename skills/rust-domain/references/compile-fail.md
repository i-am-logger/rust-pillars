# compile_fail: twins and pinned stderr

A type rule that is never seen to reject a program is a claim, not a rule. Each rule in pillar 1 is held by one illegal program that must fail to compile with a stated error, plus a legal twin that must compile.

## 1. Layout (template `crates/<name>-domain/tests/`)

```text
tests/typesystem.rs                 the trybuild driver
tests/ui/fail/<case>.rs             the illegal program
tests/ui/fail/<case>.stderr         rustc's error, pinned verbatim
tests/ui/pass/<case>.rs             the twin: the same program, the illegal line made legal
```

```rust
//! Pillar 1, held by the compiler: each illegal program in `ui/fail` must fail
//! to compile with the error pinned in its `.stderr`, and each has a twin in
//! `ui/pass` that differs only in the illegal line and must compile.

#[test]
fn illegal_programs_do_not_compile() {
    trybuild::TestCases::new().compile_fail("tests/ui/fail/*.rs");
}

#[test]
fn their_twins_do() {
    trybuild::TestCases::new().pass("tests/ui/pass/*.rs");
}
```

`trybuild` is a dev-dependency, so it never reaches the `no_std` build: `test:bare` builds without dev-dependencies, and `layers.toml` does not check dev edges.

## 2. What enforces it

| Check | Gate |
|---|---|
| Each `ui/fail/X.rs` has `ui/pass/X.rs` | `cargo xtask docs` ("has no passing twin ... it could be failing for an unrelated reason") |
| Each `ui/fail/X.rs` has `X.stderr` | `cargo xtask docs` ("has no pinned .stderr: the error it must fail with is not stated") |
| Each `ui/pass/X.rs` has its fail case | `cargo xtask docs` ("is a twin with no compile_fail case") |
| Each fail case fails with exactly the pinned stderr, and each twin compiles | trybuild, run by `test:unit` (nextest, `qa` profile) |
| The twin differs from its case only in the illegal line | Review. No gate checks it, so don't claim one does. |

## 3. Why twins and pinned stderr: the hollow test

- **The failure.** A `compile_fail` doctest that calls a function that does not exist, or a private constructor, fails to compile, so it passes, but for the wrong reason.
- **Where a typo hides it.** A rustdoc `compile_fail` block with no error code passes on any compile error: a typo, a missing import, a renamed item.
- **What it costs.** The rule it claims to hold can be deleted and the test stays green.
- **The twin proves the case's surroundings compile,** so the only thing that can fail is the illegal line.
- **The pinned stderr proves the failure is the rule's own error,** not a neighbouring one.
- **Together they turn "it didn't compile" into "it didn't compile because of this rule".**

Use trybuild in `tests/`, not rustdoc `compile_fail`, for pillar 1 rules. A doctest stays fine as an example of correct use.

## 4. The template's four cases

| Case | Illegal line | Pinned error | Rule it holds |
|---|---|---|---|
| `mixed_quantities` | `ByteLen::ZERO == WordCount::ZERO` | E0308 mismatched types | quantities are distinct types |
| `integer_into_quantity` | `let n: ByteLen = 5_u32.into();` | E0277 `ByteLen: From<u32>` not satisfied | no `From<int>` |
| `forge_an_interval` | `Elapsed(5)` | E0423 cannot initialize a tuple struct which contains private fields | constructor narrowing |
| `push_after_complete` | `line.push(..)` on `Line<Complete>` | E0599 no method named `push` | typestate |

Here is `forge_an_interval` and its twin:

```rust,compile_fail,E0423
// tests/ui/fail/forge_an_interval.rs
use my_project_domain::{Elapsed, Millis};

fn main() {
    let _: Elapsed = Elapsed(5);
    let _ = Millis::ZERO;
}
```

```rust
// tests/ui/pass/forge_an_interval.rs
use my_project_domain::{Elapsed, Millis};

fn main() {
    let _: Elapsed = Millis::from_host(5).since(Millis::ZERO);
    let _ = Millis::ZERO;
}
```

The pinned stderr names the rule: `pub struct Elapsed(pub(crate) u64);` with the note "private field". rustc even suggests the legal seam: "you might have meant to use the `threshold_ms` associated function".

## 5. Writing a case

1. **Write the twin first.** It is the shortest legal program that reaches the line you want to forbid, and it must compile (`cargo nextest run -p <domain> --test typesystem`).
2. **Copy it to `ui/fail/` under the same name,** and change exactly one line into the misuse.
3. **Generate the stderr** with `TRYBUILD=overwrite cargo test -p <domain> --test typesystem`, then **read it**:
   - The error code must be the rule's own, from the table below.
   - Reject E0425 (unresolved name), E0432/E0433 (unresolved import or path) and E0061 (wrong argument count). Each means the case is broken, not that the rule holds.
4. **Perturb the rule once, in a scratch copy.** Remove the private field, add the `From<u32>` impl, or add `push` to `Line<Complete>`. The case must turn red, with trybuild reporting that it compiled.
   - If the case still fails to compile after you delete its rule, trybuild stays green, and the case was never testing that rule.
   - If the perturbation does not compile, nothing was tested; that is not a green result (rust-qa).
5. **Commit the case, its twin and its stderr together.** `cargo xtask docs` rejects any one without the others.

| Rule | Typical misuse | Error to expect |
|---|---|---|
| Distinct quantities | comparing or passing one type as another | E0308 |
| No `From<int>` | `n.into()`, `ByteLen::from(5)` | E0277 |
| Private field / narrowing | `Elapsed(5)` from outside the crate | E0423 (tuple struct), E0451 (named field) |
| Typestate | a method of another state | E0599 |
| Sealed trait | `impl State for MyState` downstream | pin what rustc reports |
| Brand escape or mix | a `Slot<'id>` from one view used in another | a lifetime error: pin what rustc reports |
| No operator | `a + b` on quantities with no `Add` | E0369 |

Where the table says "pin what rustc reports", generate the stderr and read it. Don't write an expected code from memory.

## 6. Stderr drift

- The pinned stderr is rustc's own text, so it changes when the compiler changes.
- The template's toolchain is `channel = "stable"` (`nix/rust.nix`), and `devenv.lock` pins which stable it is.
- After a toolchain bump, re-run with `TRYBUILD=overwrite` and review the diff like code. A changed message is fine; a changed error code means the case now fails for a different reason, so investigate it.
- trybuild normalises paths: the stderr shows `--> src/quantity.rs` for the dependency's source, without a line number, so moving the item within its file does not churn the `.stderr`.

## 7. What deserves a case

- **Every rule a type exists to enforce:** one case per new quantity's main misuse, one per typestate transition boundary, one per narrowing constructor, and one per brand.
- **Not for behaviour.** Saturation and refusal are run-time behaviour and get ordinary unit tests at the extremes (`quantity.rs` tests).
- **Not for what clippy or the budgets gate already catch.** A `bool` parameter is caught by `bool_param_or_field`, a `pub` newtype field by `pub_newtype_field`. A `compile_fail` case is for what the type system itself rejects.
