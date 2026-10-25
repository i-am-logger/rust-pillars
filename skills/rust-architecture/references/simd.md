# SIMD under no-unsafe: portable spec, checked backends, compile-time selection

Load this when you vectorise a loop, add an ISA backend, see `std::arch` or
`core::simd`, or are asked to replace an `unsafe` SIMD loop.

Evidence labels:
- **by-reading**: the documentation or code says it.
- **measured here**: this skill's own checks on rustc 1.94.1,
  aarch64-apple-darwin, release profile, in a `git archive` copy of
  rust-template.

## Rules

1. **The portable version is the specification.** Write it first, make it
   obviously right, and test it against the independent model
   ([sans-io.md](sans-io.md)). Every backend is an optimisation of the spec,
   and the spec is the oracle.
   - On stable Rust, `core::simd` is unstable: `use core::simd::u8x16` fails
     with E0658 `portable_simd` (measured here).
   - So, on the template's stable toolchain, the spec is scalar code. Use
     `core::simd` only in a project pinned to nightly, and record that as a
     decision.
2. **Do not assume the compiler vectorises.** Check the disassembly.
   - A scalar 16-byte newline count compiled to 16 scalar `ldrb`/`cmp`/`cinc`
     triples with no vector instruction (measured here). That held for both a
     `for` loop and a `wrapping_add` fold.
   - The hand-written NEON backend below compiled to `ldr q0`, `cmeq.16b` and
     `addv` (measured here).
3. **Check every backend against the spec with an exhaustive agreement test
   that asserts the sweep ran.**
   - End it with the count: `assert_eq!(checked, 65_536)` for 2^16 blocks.
   - A test that silently covers less is not a pass.
   - Choose an alphabet small enough to enumerate. For a newline count,
     {`\n`, `a`} at 16 positions is 2^16 blocks.
4. **Measure lane width; don't pick it.** A wider vector wins only when the
   runs it scans are long enough to fill it, which depends on the input, not
   the instruction set; a backend's gain can be small or none (unmeasured
   here: A/B each backend on your own workloads).
5. **A backend's inputs and outputs are domain types.**
   - A backend that returns a raw `u32` mask hands its caller the very bug
     class the newtypes exist to prevent.
   - In the core this is also a gate. A public `fn(&[u8; 16]) -> u8` in the
     core counted `primitive_api` +1 in `cargo xtask budgets` (measured here).
   - So the spec and its result type live in the domain, and the quarantine
     returns the domain type. The cost is one entry seam (`from_backend`) that
     can forge a count. Add its name to `[seams] names` in `budgets.toml` (a
     reviewed change), and make sure every caller of it has an agreement test.
6. **Select the backend at compile time, with `cfg`, and verify the selection
   in the disassembly.**
   - Use `#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]`, and
     the scalar spec under `not(...)`.
   - For x86 AVX2, build with `-C target-feature=+avx2` (or a `target-cpu`) so
     that `cfg(target_feature = "avx2")` holds.
   - DEPARTS from the common runtime-dispatch practice
     (`is_x86_feature_detected!` and then a call into a `#[target_feature]`
     fn). Under `forbid(unsafe_code)`, that call needs `unsafe` at every
     dispatch site. With compile-time selection, the binary's code is fixed per
     build, so an A/B compares exactly what ships.
   - Verify after building. Disassemble the release artefact (`otool -tV` on
     macOS, `objdump -d` elsewhere, or `--emit asm`), and grep for the
     backend's instructions.
   - An A/B can silently measure a cached binary against itself. `cmp` the
     two arms' binaries first: identical text means
     the arms are the same program. The `rust-measure` skill covers this.
7. **Write every SIMD backend with no `unsafe`, except one entry.**
   - Inside `#[target_feature(enable = "neon")]`, the arithmetic and compare
     intrinsics are safe (measured here).
   - Pointer loads such as `vld1q_u8` stay unsafe. Replace them with
     `zerocopy::transmute!(*block)` (zerocopy's `simd` feature), which needed
     no `unsafe` (measured here).
   - Calling the `#[target_feature]` fn from ordinary code needs `unsafe`, even
     when the build enables the feature. rustc's E0133 note says: "the neon
     target feature being enabled in the build configuration does not remove
     the requirement to list it in `#[target_feature]`" (measured here).
   - So the one entry lives in the quarantine
     ([no-unsafe.md](no-unsafe.md#the-quarantine-crate)).
   - Clippy also treats a *public* safe `#[target_feature]` fn as needing a
     `# Safety` doc section (`missing_safety_doc`, measured here). Keep such
     fns `pub(super)` behind the safe entry.
   - Toolchain: these rules need a compiler newer than the template's
     `rust-version = "1.85"`. They are believed to have arrived in 1.86 and
     1.87, which is UNVERIFIED here: they were checked on 1.94.1 only. Raise
     `rust-version` to what `cargo msrv find` measures.

## Where the backend lives

| Piece | Crate | Why |
|---|---|---|
| scalar spec (`Newlines::of`) and the result type | domain | pure, no_std; the core and the quarantine can both reach it |
| `#[target_feature]` kernels and the one `unsafe` entry | quarantine (`layers.toml` role `quarantine`) | the only crate allowed `unsafe`; it may depend on the domain |
| agreement test | the quarantine's `tests/`, using the domain, which is a normal dependency | spec and backend meet without any new edge |
| use of the backend from the core | a normal edge: `[roles.core].may_depend_on` includes `"quarantine"` | the core calls only the quarantine's safe wrappers |

With several architectures, put each ISA behind a trait, with one leaf crate
per architecture ([layering.md](layering.md), rule 7).

## Compiled example

This compiled with clippy `-D warnings`, and the agreement test passed
(measured here). The quarantine body is in
[no-unsafe.md](no-unsafe.md#the-quarantine-crate). It returns
`my_project_domain::Newlines` through `Newlines::from_backend(n)`.

Budget deltas, from `cargo xtask budgets --print`:
- domain: `seam` +3 (`of`, `from_backend` and `get`);
- quarantine: `unsafe_code` = 1 and `expect_attr` = 1;
- core: none.

The spec, in the domain:

```rust
/// How many newlines a 16-byte block holds: 0..=16.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Newlines(pub(crate) u8);

impl Newlines {
    /// The specification: obviously right, and the oracle for every backend.
    #[must_use]
    pub fn of(block: &[u8; 16]) -> Self {
        let mut n = 0_u8;
        for &b in block {
            n += u8::from(b == b'\n');
        }
        Self(n)
    }

    /// A backend's count. The price of domain-typed backend output: this
    /// seam can forge a count, so every backend that uses it has an
    /// agreement test against [`Newlines::of`].
    #[must_use]
    pub const fn from_backend(count: u8) -> Self {
        Self(count)
    }

    /// The count, for interop. The only way out.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}
```

The agreement test, in the quarantine's `crates/<name>-isa/tests/agreement.rs`
(`ignore`: it names a quarantine crate this project does not have, and its
file-level `cfg` removes the whole doctest off aarch64):

```rust,ignore
#![cfg(all(target_arch = "aarch64", target_feature = "neon"))]

#[test]
fn the_neon_backend_agrees_with_the_spec_on_every_block() {
    let mut checked = 0_u32;
    for bits in 0..=u16::MAX {
        let mut block = [b'a'; 16];
        for (i, b) in block.iter_mut().enumerate() {
            if (bits >> i) & 1 == 1 {
                *b = b'\n';
            }
        }
        let want = my_project_domain::Newlines::of(&block);
        assert_eq!(my_project_quarantine::newlines(&block), want, "{bits:#06x}");
        checked += 1;
    }
    assert_eq!(checked, 65_536, "the sweep did not cover every block");
}
```

- The `#![cfg]` means this test does not exist on x86 CI, and "a comparison
  that skips itself reads as a pass".
- So list each platform-specific test with the CI job that runs it. The
  `platform-tests.toml` gate is planned; see the `rust-qa` skill. Each backend
  needs a runner of its own architecture.

## Replacing an `unsafe` SIMD loop (the usual task)

1. Write the scalar spec and an exhaustive agreement test against the existing
   loop. Run it, and see it pass on the old code.
2. Check whether the spec alone is fast enough. Disassemble it, then A/B it
   against the old loop (`rust-measure`). If the result is neutral, delete the
   intrinsics. You are done, with zero `unsafe`.
3. Otherwise, port the loop into a quarantine:
   - make the kernels safe `#[target_feature]` fns;
   - use zerocopy transmutes for the loads;
   - put one `#[expect(unsafe_code, reason)]` with its `// SAFETY:` comment at
     the cfg-guarded entry.
4. Keep the agreement test, now spec against backend, and assert its count.
5. Verify the selection in the disassembly of the release build.
