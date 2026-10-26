---
name: rust-architecture
description: "Use when designing or reviewing Rust structure: crates and dependencies (layers.toml roles), a core state machine (events in, effects polled), who owns a buffer or how to avoid Box::leak, no_std, unsafe and quarantine crates, FFI and -sys crates, SIMD backends, cache-aware layout, CPU/GPU shared memory, or allocation outside the core."
license: CC-BY-NC-SA-4.0
---

# rust-architecture

Each rule gives its WHY and the rust-template gate that enforces it. "(review)" means no gate exists yet, so you are the gate.

Evidence labels:
- **measured**: someone measured it.
- **by-reading**: the code or notes say it.
- **inferred**: predicted, never measured.
- **measured here**: measured, by this skill's own compile checks (rustc 1.94.1, in a template copy).

These are rust-research's labels (measured / by-reading / inferred / unmeasured).

Never present an inferred claim as measured.

## Crate roles and direction ([layering.md](references/layering.md))

- Give every crate one role in `layers.toml`:
  - `domain`: types
  - `core`: the machine
  - `adapter`: I/O, platform, GPU
  - `bin`: wiring
  - `tool`: xtask
  - `harness`: benchmarks and difftests (use anything; nothing ships using them)
  - `quarantine`: `unsafe`, behind safe wrappers the core may call
- Point dependencies domain → core → adapter → bin: privacy is enforced only at crate edges. Gate: `cargo xtask layers`.
- The core gets no external crates (`external = []`) and the domain gets only `zerocopy`, because every dependency is unmeasured hot code. Gate: `cargo xtask layers`.
- Change an edge or an allowlist only in its own commit that says why (review).
- Do all I/O (clock, files, stdin, sockets, GPU) outside domain and core: in adapters, or in the bin when there is no adapter (as in the template). Domain and core then stay testable without a clock or a file. Gates: `#![no_std]` and `test:bare`.
- Keep features additive, because Cargo unifies features across the graph. Gate: `test:features`.
- Measurement arms are `cfg(arm = "...")`, never features: features unify across the graph, and arms are not additive, so as features they break `--all-features` under `-D warnings`. Declaring and building arms: rust-measure. (review, planned gate)
- Every crate except a quarantine sets `[lints] workspace = true`, so one table holds `forbid(unsafe_code)` for all of them. Gate: `cargo xtask arch`.

## Sans-IO core ([sans-io.md](references/sans-io.md))

- The core's interface is `handle(&mut self, Event)` → `effects() -> &[Effect]` → `clear_effects()`, so its output is data a test compares.
- Effects are polled, never called back: no `fn(..)`, no `impl`/`dyn Fn*`, no `&mut dyn Sink`, no generic method bound by a behaviour trait. A callback runs host code inside the core's borrow. Gate: `cargo xtask arch` (it flags every bound outside its data-trait list, such as `Iterator`, `AsRef`, `Clone`).
- Time, randomness and I/O enter as events (`Event::Tick(Millis)`, a seed, `Wire<'a>`), so the same input always gives the same output. Gates: the crate-local `clippy.toml` bans `Instant`/`SystemTime`/`Thread`; `#![no_std]`.
- Every buffer is bounded. On overflow, refuse and count (`Queued::Dropped` → `Refusals`); never grow, block or drop silently. Gate: `crates/<name>-core/tests/alloc.rs` plus unit tests that fill each bound.
- `const`-assert each message size against its buffer, and take capacities from the protocol's maximum. Gate: the build.
- State lives on the struct between events, so a token split across reads stays in fields.
- Every core ships a `chunk_invariance_*` property: any split equals the whole, and one-byte reads equal an **independent** model. A planted doubled advance passed split equality alone (measured, rust-pillars's own planted run). Gate: `cargo xtask arch` checks the marker; independence is review. Writing the property and the model: rust-qa.
- The core is not `async`, so any executor, a test or a bare-metal loop can drive it. Its error types: rust-idioms (errors).

## Zero-copy, own don't leak ([zero-copy.md](references/zero-copy.md))

- Borrow input and output (`&[u8]`, `&[Effect]`), and have the caller lend storage (`Lent<'b>`), as C-CALLER-CONTROL says (review).
- Stored and shared records are zerocopy POD (`repr(C)`, size-asserted, zero is valid), so no consumer needs `unsafe`.
- Nothing allocates per event. Gate: `crates/<name>-core/tests/alloc.rs` (dhat: `total_blocks` and `curr_bytes` unchanged). A hot-module `.to_vec()` ban is planned.
- Copy only where a measurement says so: every copy adds to write amplification, one of the two invariants ([caches.md](references/caches.md)).
- Own, don't leak, in this order:
  1. a plain owner by value
  2. `self_cell` (adapter or bin)
  3. an RAII guard per foreign handle
  4. a generic `S: Storage` for **cold** types only, with the bound on the type or impl: `cargo xtask arch` flags a core `pub fn` generic over a behaviour trait (measured here)
  5. one named process owner, last
- Gate: clippy `disallowed_methods` bans `Box`/`Vec`/`String::leak`, `mem::forget` and `ManuallyDrop::new`.
- `self_cell!` expands to a `mem::forget` of its build-panic drop guard, which the ban flags (measured here). Wrap it in a module with one counted `#[expect(clippy::disallowed_methods, reason)]`.

## Ownership invisible to the hot path

- Unpack owners into slices and locals once per batch, and write back once: a view built per write pays its setup on every write.
- Keep hot impls non-generic and owners out of hot structs: code placement moves hot-path speed, so a change that moves hot code is a risk to measure (rust-measure). DEPARTS from C-GENERIC.
- INFERRED: owned storage is neutral only while hot structs stay byte-identical. Pin their sizes with `const` asserts, and ship an A/B.

## Layout for the cache ([caches.md](references/caches.md))

- Split hot from cold and pin both sizes (`const _: () = assert!(size_of::<ColdState>() == N);`): a hot field in the cold half drags a cold line into the hot loop.
- Probe geometry in an adapter into a pure `CacheGeometry` that has an `UNKNOWN` (with a real line size, such as 64), so a failed probe still lays out correctly. Only the line size drives layout.
- The pitch is an odd number of lines, clear of the L1 set wrap, held in a `Pitch` that is not `Cols`. At a pitch of exactly one L1 way every row maps to the same sets. Line-align the base too. (review: an alignment test is planned)
- Build tables at compile time (`const fn` into a `static`, or a two-stage trie): they land in rodata with no startup initialiser. Assert each byte budget, and test finite domains exhaustively with `assert_eq!(checked, N)`, so a sweep that silently shrank fails.
- No prefetch without a winning A/B: an explicit prefetch is one more load competing with the demand loads, not free advice.
- Pad cross-thread hot state with `#[repr(align(128))]`: two threads writing one line serialise on it, and Apple's lines are 128 bytes (by-reading).

## no_std ([no-std.md](references/no-std.md))

- Domain and core are `#![no_std]`, with `alloc` behind an off-by-default feature and `default-features = false` on every dependency. Gates: `cargo xtask arch`, `test:bare` (`x86_64-unknown-none`, `thumbv7em-none-eabihf`).
- List every clippy ban under `alloc::`/`core::` as well as `std::`. In no_std a `std::` path names nothing, so the ban is silently void (measured).
- The nearest `clippy.toml` replaces the root one, so each crate-local copy is a superset of it. Gate: `cargo xtask arch`.
- no_std errors are `Copy` enums with no payload (no allocator to carry one); the rule and its DEPARTS from BeautifulRust are rust-idioms'.

## No unsafe ([no-unsafe.md](references/no-unsafe.md))

- `unsafe_code = "forbid"` everywhere, which no inner attribute can lower: a finite audit needs one crate. DEPARTS from BeautifulRust's three allowed reasons.
- Escape in this order: safe std, then a safe crate (zerocopy, dhat), then a quarantine.
  - Safe `#[target_feature]` kernels shrink a quarantine to a single entry. They do not replace it.
- A quarantine is declared in `layers.toml`, depends only on the domain and other quarantine crates, and exposes a safe API only.
  - Its own lints set `unsafe_code = "deny"`.
  - Each block carries `#[expect(unsafe_code, reason)]` plus a `// SAFETY:` comment.
  - Gates: `cargo xtask budgets` (exact `unsafe_code`/`expect_attr`) and clippy `undocumented_unsafe_blocks`. Miri and sanitizers are planned.
- A dependency whose macros expand `unsafe` into your crate is an `unsafe` decision, because `forbid` and `budgets` miss it (measured here, `self_cell!`).

## FFI inside the quarantine ([ffi.md](references/ffi.md))

- Put every `unsafe extern` block, `#[unsafe(no_mangle)]` export and raw foreign pointer in a quarantine crate: edition 2024 makes the first two unsafe code, which `forbid` refuses elsewhere (measured here), and the pointer is usable only through `unsafe`.
- Treat the module as the soundness boundary: private fields hold each invariant a `// SAFETY:` comment relies on, so no safe call sequence can break it (Rustonomicon, "Working with Unsafe").
- Split a `<name>-sys` crate (bindgen, `links`, `allowlist_function`) from a safe wrapper, both quarantine role: one native library per `-sys` crate (Cargo Book, "Build Scripts"). `budgets` misses `include!`d bindings (review).
- Mark every struct that crosses `#[repr(C)]`, size-asserted, with `core::ffi` integers: Rust's own layout is unspecified (Rustonomicon, "Alternative representations").
- Take a C integer and `TryFrom` it into a domain enum, never a Rust `enum`/`bool` from C: an out-of-range value is UB the moment it exists.
- Pass strings as `CStr`/`CString` and OS strings as `OsStr`/`Path`: C strings are NUL-terminated and neither is guaranteed UTF-8.
- Free memory with the allocator that made it, inside one RAII owner per foreign handle: Rust's global allocator need not be C's.
- Turn null into `None`/`Err` in the constructor (`NonNull::new`): a wrapper that exists then always holds a live handle.
- Wrap every `extern "C"` export in `catch_unwind` (`panic = "abort"` in `no_std`): a panic reaching the boundary aborts the host (measured here).
- Write `unsafe impl Send`/`Sync` only with a doc comment naming the C library's guarantee (Rustonomicon, "Send and Sync"): the raw pointer's `!Send` is the right default.
- Run Miri on the pure-Rust `unsafe` and sanitizers on the real C calls, since Miri cannot execute foreign code. Commands: rust-qa (planned).
- Keep FFI out of every crate but a declared quarantine. DEPARTS from BeautifulRust: it allows FFI in any module that keeps its invariant, and a finite audit needs one place to look.

## SIMD ([simd.md](references/simd.md))

- The scalar portable code is the specification. `core::simd` is nightly-only (E0658, measured here).
- Read the disassembly before claiming vectorisation: a scalar 16-byte count stayed scalar (measured here).
- Every backend has an exhaustive agreement test that asserts its count (`assert_eq!(checked, 65_536)`): the spec is the oracle, and a sweep that ran short is not a pass.
- Select backends at compile time (`cfg(all(target_arch = "aarch64", target_feature = "neon"))`), verified in the disassembly. DEPARTS from runtime dispatch, which needs `unsafe` at every call.
- Write kernels as safe `#[target_feature]` fns with zerocopy loads, and put the single `unsafe` entry in the quarantine. Entering such a fn needs `unsafe` even when the feature is enabled (E0133, measured here).
- Backend inputs and outputs are domain types: a raw `-> u32` is the bug class the newtypes prevent.

## Shared CPU/GPU memory ([gpu-shared-memory.md](references/gpu-shared-memory.md))

- Hand off by generations, with frames in flight + 1 slots. The producer never writes a slot the consumer holds.
- Completion is an event. When no slot is free, skip the frame and count it. GPU retains are RAII guards.
- Whether to copy per frame or use a ring is **open** and unmeasured: weigh each option against the two invariants and latency, with an A/B.

## Performance is a design requirement (domain and core)

- Design layout and ownership for speed from the start, then measure every change. DEPARTS from BeautifulRust's "usability first, optimize after measuring", which still holds in adapters and bins: a hot struct's layout is costly to change once code depends on it.
- Every hot change states its effect on write amplification and working set vs cache, with a measurement, not an argument (the two invariants: rust-measure).

## Allocation outside the core ([allocation.md](references/allocation.md))

Adapters, bins and setup only: the domain and core allocate nothing per event. Measure each tip (the rust-measure skill).

- Build with `collect()` or `with_capacity`, not a push loop: 1000 pushes made 9 heap blocks, `collect` 1 (measured here). (review: clippy `vec_init_then_push` catches only pushes straight after `Vec::new()`)
- Let the caller own the buffer in a hot API (`fn encode_into(.., out: &mut Vec<u8>)`): a reused buffer allocates nothing at steady state (C-CALLER-CONTROL).
- Store long-lived immutable data as `Box<[T]>`/`Box<str>`/`Arc<str>`, and `shrink_to_fit` once building ends: no spare capacity, one allocation. Gate: clippy `box_collection`; `Rc<String>` is review (`rc_buffer` is restriction, not enabled).
- Use a fast hasher (`foldhash`, `FxHash`) only for keys the program made: `RandomState` is what resists HashDoS. The core uses no hash map. DEPARTS from BeautifulRust: in a `no_std` core the map itself is the cost.
- Lock once per batch, not per item: each acquire is a contended atomic (review).
- Set a `#[global_allocator]` (mimalloc etc.) only in a bin, after a winning A/B: a library that sets one takes the choice from every binary. DEPARTS from BeautifulRust: it lets any application set one unmeasured.
- Derive `Copy` only for small types, and size-assert them: a large `Copy` is a hidden memcpy at every move. (review: pedantic `large_types_passed_by_value` is not enabled)

## Before handing back

- Run `devenv test`. It runs fmt, sources, clippy `-D warnings`, check, arch, layers, budgets, docs, tests, coverage, unit (qa profile, seed 1), doc, bare, features, deny, mutants-diff and controls. A snippet that passes clippy alone has passed one gate of these.
- Every new pub fn in domain or core needs a test that calls it (`cargo xtask coverage`), and every mutant of a changed line must be caught (`test:mutants-diff`).
- A budget is the count. Lower it when the count falls; raise it only in its own commit with a reason (`cargo xtask budgets --print`).

## References (load on demand)

| File | Load when |
|---|---|
| [sans-io.md](references/sans-io.md) | a core, events/effects, buffers, the chunk-invariance property |
| [layering.md](references/layering.md) | a new crate, dependency or feature; a `layers`/`arch` failure |
| [zero-copy.md](references/zero-copy.md) | ownership, lent storage, leaks, `'static`, self_cell, copies |
| [no-std.md](references/no-std.md) | no_std, alloc, clippy paths, porting std code |
| [no-unsafe.md](references/no-unsafe.md) | anything that seems to need `unsafe`; quarantine crates |
| [caches.md](references/caches.md) | struct splits, pitch, alignment, tables, prefetch, false sharing |
| [ffi.md](references/ffi.md) | extern "C", bindgen `-sys` crates, C strings, foreign handles, exports |
| [allocation.md](references/allocation.md) | allocation in adapters/bins, hashers, locks, global allocator, large `Copy` |
| [simd.md](references/simd.md) | vectorising, intrinsics, replacing an unsafe SIMD loop |
| [gpu-shared-memory.md](references/gpu-shared-memory.md) | CPU/GPU or cross-process shared buffers |
