# Zero-copy, own-don't-leak, ownership invisible to the hot path

Load this when you choose who owns a buffer, add storage to a core type, see a
`'static` lifetime demanded, hit the `Box::leak`/`mem::forget` clippy ban, or
add a copy.

Evidence labels: **measured** (someone measured it), **by-reading** (the code
or notes say it), **inferred** (predicted, never measured), **measured here**
(this skill's own checks: rustc 1.94.1, aarch64-apple-darwin, a `git archive`
copy of rust-template).

## Borrow, don't own

- Input is borrowed: `Wire<'a>(&'a [u8])` in the domain, `Event::Bytes(Wire<'a>)`.
  The domain never copies input.
- Output is borrowed where it sits: `Machine::effects(&self) -> &[Effect]`.
- Storage the core needs for its lifetime is *lent* by the caller (`Lent<'b>`
  below). Never allocate it inside the core.
- This CONFORMS to the Rust API Guidelines' C-CALLER-CONTROL (the caller decides
  where data is copied and placed). It also matches BeautifulRust's
  "caller-owned buffers in hot APIs".
- **Stored and shared layouts are zerocopy POD.** Wire, disk, shared-memory and
  GPU records take the derives
  `#[derive(FromBytes, IntoBytes, Immutable, KnownLayout)]` and `#[repr(C)]`,
  with the size `const`-asserted. The template's `LineRecord` is 16 bytes with
  no padding.
  - WHY: bulk zeroed allocation and byte views then need no `unsafe` in any
    consumer, and zero is a valid value, so a zeroed allocation is a valid empty state.
- **No allocation on the hot path.**
  - Gate: `crates/<name>-core/tests/alloc.rs`. Under `dhat` (a safe global
    allocator), 1000 rounds of events leave `total_blocks` and `curr_bytes`
    unchanged. That is zero allocations, and no growth.
  - Allocate setup before the profiler starts; make the input exercise every new path.
  - Planned, not in the template: a hot-module `.to_vec()`/`.to_owned()` ban and a long soak test.
- **Copy only where a measurement says so.** A copy is a write, and writes are
  one of the two invariants ([caches.md](caches.md#the-two-invariants)).
  - Write amplification is the bytes written per byte ingested.
  - Moving rows on scroll is a copy of data that did not change.
  - A ring with a rotation index moves bookkeeping, not data. On that path a
    conditional subtract can replace `%`; measure it (unmeasured here).

## Caller-lent storage, unpacked once per batch (core role)

The snippet below passed clippy `-D warnings` in a core-role crate, and
`cargo xtask budgets` added no count (measured here). `cargo xtask coverage`
then requires a test that calls `Log::feed`.

```rust
use my_project_domain::{ByteClass, BytePos, ReadLen, Tally, Wire};

/// Slots the host lends: a `Vec`'s slice from a host with an allocator, a
/// local array's from one without. The core never allocates, frees or grows them.
#[derive(Debug)]
pub struct Lent<'b> {
    starts: &'b mut [BytePos],
}

impl<'b> Lent<'b> {
    /// Borrows the host's slots for the life of the log.
    #[must_use]
    pub fn new(starts: &'b mut [BytePos]) -> Self {
        Self { starts }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Used(usize);

/// Touched for every byte: small, copied into locals per batch.
#[derive(Clone, Copy, Debug)]
struct HotState {
    pos: BytePos,
    used: Used,
}

/// Touched rarely: out of the per-byte path, size pinned.
#[derive(Clone, Copy, Debug)]
struct ColdState {
    refused: Tally,
    batches: Tally,
}

const _: () = assert!(core::mem::size_of::<HotState>() == 16);
const _: () = assert!(core::mem::size_of::<ColdState>() == 16);

/// Records where each line starts, into storage the host lent.
#[derive(Debug)]
pub struct Log<'b> {
    lent: Lent<'b>,
    hot: HotState,
    cold: ColdState,
}

impl<'b> Log<'b> {
    /// An empty log over the host's slots.
    #[must_use]
    pub fn new(lent: Lent<'b>) -> Self {
        let hot = HotState {
            pos: BytePos::ZERO,
            used: Used(0),
        };
        let cold = ColdState {
            refused: Tally::ZERO,
            batches: Tally::ZERO,
        };
        Self { lent, hot, cold }
    }

    /// Takes one read. Ownership is unpacked once, here: the per-byte loop
    /// sees a plain slice and locals, and state is written back once.
    pub fn feed(&mut self, wire: Wire<'_>) {
        let starts: &mut [BytePos] = self.lent.starts;
        let HotState { mut pos, mut used } = self.hot;
        let mut refused = Tally::ZERO;
        for class in wire.classes() {
            pos = pos.saturating_advance(ReadLen::ONE);
            if class == ByteClass::Newline {
                match starts.get_mut(used.0) {
                    Some(slot) => {
                        *slot = pos;
                        used.0 += 1;
                    }
                    None => refused = refused.saturating_incr(),
                }
            }
        }
        self.hot = HotState { pos, used };
        self.cold.refused = self.cold.refused.saturating_add(refused);
        self.cold.batches = self.cold.batches.saturating_incr();
    }
}
```

A host with no allocator lends a local array from `main`:
`let mut slots = [BytePos::ZERO; 1024]; let mut log = Log::new(Lent::new(&mut slots));`

Another no-allocator form is `&'static mut` from a linker section. That
needs `unsafe`, so it belongs in the quarantine
([no-unsafe.md](no-unsafe.md)).

## Own, don't leak

Gate: clippy `disallowed_methods` bans `Box::leak`, `Vec::leak`,
`String::leak`, `mem::forget` and `ManuallyDrop::new`, under `std::` and
`alloc::`/`core::` paths (root `clippy.toml`, copied into the crate-local ones).

Leaks are banned even once per process: a leak is invisible to leak checkers,
forces `'static` into every API downstream, and becomes a real leak the day the
process holds two instances.

1. **A plain owner, held by value.** The owner is a field or a local, and
   everything else borrows from it. The template boxes `Machine<N>` once in
   `main` and borrows it from there.
2. **`self_cell` for an owner plus a dependent that borrows from it**, when the
   two must travel together, as with a buffer and its parsed views.
   - Adapter or bin only. It needs `alloc`, and no pure role allowlists it.
   - **Gate interaction (measured here).** Once the dependent is built, the
     `self_cell!` expansion disarms its build-panic drop guard with
     `::core::mem::forget(drop_guard)`, so the template's `disallowed_methods`
     fires inside the macro. Wrap the invocation in a module with one counted,
     reasoned `#[expect]`, which `cargo xtask budgets` counts as `expect_attr`:

     ```rust,ignore
     pub use message::Message;

     #[expect(
         clippy::disallowed_methods,
         reason = "self_cell disarms its build-panic drop guard with mem::forget once the dependent is built: the cell owns the buffer, nothing leaks"
     )]
     mod message {
         use super::{Parsed, parse};

         self_cell::self_cell!(
             /// A buffer and the views into it, owned together.
             pub struct Message {
                 owner: Vec<u8>,
                 #[covariant]
                 dependent: Parsed,
             }
             impl {Debug}
         );

         impl Message {
             /// Takes ownership of `buf` and parses it once.
             #[must_use]
             pub fn parse(buf: Vec<u8>) -> Self {
                 Self::new(buf, |b| parse(b))
             }
         }
     }
     ```

     Here `Parsed<'a>` is a struct of `&'a [u8]` views, and `parse` is a plain
     `fn(&[u8]) -> Parsed<'_>`. Not compiled by the snippet gate: `self_cell` is an adapter dependency it does not link.
   - Budget delta: adapter `expect_attr` = 1, from `cargo xtask budgets --print`
     (measured here).
   - The expansion also contains `unsafe` blocks, which `forbid(unsafe_code)`
     does not report ([no-unsafe.md](no-unsafe.md#macro-expanded-unsafe)).
3. **An RAII guard for every foreign handle**: an OS handle, a GPU retain, a
   pool slot. Release happens in `Drop`, so it runs on every path out, early
   returns and unwinding included.
   - Destructors never fail (C-DTOR-FAIL) and never block (C-DTOR-BLOCK), so
     `Drop` flips a flag; it never allocates, locks or waits.
   - Bind the guard to a name. `let _ = guard()` drops it on the spot.
   - Adapter code, passed clippy `-D warnings` with a test (measured here):

   ```rust
   use core::cell::Cell;

   #[derive(Clone, Copy, PartialEq, Eq, Debug)]
   pub struct BufId(usize);

   #[derive(Clone, Copy, PartialEq, Eq, Debug)]
   enum Loan { Free, Out }

   /// `N` buffers; each is on loan to at most one `Lease`.
   #[derive(Debug)]
   pub struct Pool<const N: usize> { loans: [Cell<Loan>; N] }

   impl<const N: usize> Pool<N> {
       #[must_use]
       pub fn new() -> Self {
           Self { loans: [const { Cell::new(Loan::Free) }; N] }
       }

       /// A free buffer, or `None` when all are out (the caller counts it).
       #[must_use]
       pub fn lease(&self) -> Option<Lease<'_, N>> {
           let (i, loan) = self.loans.iter().enumerate().find(|(_, l)| l.get() == Loan::Free)?;
           loan.set(Loan::Out);
           Some(Lease { pool: self, id: BufId(i) })
       }
   }

   /// A buffer on loan, returned to its pool when the lease drops.
   #[derive(Debug)]
   pub struct Lease<'p, const N: usize> { pool: &'p Pool<N>, id: BufId }

   impl<const N: usize> Drop for Lease<'_, N> {
       fn drop(&mut self) {
           if let Some(loan) = self.pool.loans.get(self.id.0) {
               loan.set(Loan::Free);
           }
       }
   }
   ```
   (Add `impl Default for Pool<N>` calling `new`, for `new_without_default`.)

4. **A generic `S: Storage`, for cold types only.** It can be lent slices or an
   owner. Reserve it for types off the per-byte path, such as history,
   scrollback or configuration.
   - WHY: a generic hot impl is monomorphised once per storage type. That
     duplicates hot code and moves it.
   - Code placement moves hot-path speed (rust-measure), and a generic
     top-level state type makes every hot impl block on it generic.
   - Bound the type or its impl: a core `pub fn` generic over a behaviour trait
     is flagged by `cargo xtask arch` as a callback (measured here).
   - DEPARTS from the Rust API Guidelines' C-GENERIC (minimise assumptions with
     generics) for hot code: the cost of the generic is paid in code placement,
     where nobody sees it.
5. **At most one named process owner, last.** This is one value constructed in
   the bin's `main` that owns everything process-lifetime, borrowed downward.
   - If a foreign API truly demands `'static`, this owner is the single,
     named, documented place that supplies it. It is never an anonymous
     `Box::leak`.

## Ownership is invisible to the hot path

- **Unpack once per batch.** Turn owners and lent storage into plain slices and
  locals at the top of the batch, and write state back once at the end
  (`Log::feed` above).
  - A view built per write pays its setup on every write; built once per batch,
    it pays once.
- **Keep hot impls non-generic.** One concrete type per hot loop.
- **Keep owners outside hot structs.** An owner's pointer, length and capacity
  inside a hot struct change its size and its code layout.
- **Pin hot struct sizes** with `const _: () = assert!(size_of::<HotState>() == N);`.
  Green asserts prove the size kept, not the code: that is the `__TEXT` `cmp` below.
- **The prediction, labelled honestly.**
  - INFERRED: owned storage is speed-neutral *if* hot structs stay
    byte-identical. Nobody has measured this here.
  - The risk is placement: bytes that nothing reads, or one more indirection,
    can move hot code and its timing (unmeasured here).
  - So an ownership change on a hot path ships with an A/B (`rust-measure`
    arms hygiene). Compare `__TEXT` first: a byte-identical text section stands
    in for a timing. The inferred neutrality never replaces the A/B.

## Review checklist

- [ ] Input and output cross the core borrowed; no `Vec`/`String` in hot signatures.
- [ ] Stored and shared records: zerocopy derives, `repr(C)`, `const`-asserted size.
- [ ] `crates/<name>-core/tests/alloc.rs` still shows zero blocks and no growth, on input covering the change.
- [ ] Owners follow the order above; every `#[expect]` is counted and has a reason.
- [ ] Hot structs are byte-identical (asserts unchanged), or an A/B is attached.
