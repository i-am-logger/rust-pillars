# Ownership: borrow, don't clone

Load this when the borrow checker refuses code, before you reach for `.clone()`, `Rc` or a
`'static`. A clone added to quiet the checker hides a design question and costs a copy on every
call. Core has no allocator to clone into (`alloc` is off by default; `test:bare` builds without it).

## 1. Defaults

- Unique ownership is the default. Every value has one owner, and everything else borrows.
- Parameters borrow (`&str`, `&[T]`, `Wire<'_>`). Take `T` by value only to store or consume it.
- Return owned values you created, and borrows of values you hold (`Machine::effects() -> &[Effect]`:
  the host reads effects where they sit).
- Lifetimes: elide when there is one (clippy `needless_lifetimes`). With two or more, name
  them by what they borrow (`'src`, `'buf`), so an error message says which borrow is wrong.

## 2. The ladder: try these in order before cloning

| # | Rung | When it works | Code kind |
|---|---|---|---|
| 1 | Shorten the borrow: name a temporary, end its use before mutating | the shared borrow is only needed for a value | all |
| 2 | Split the expression: compute first, then mutate | `self.f(self.g())` style conflicts | all |
| 3 | `mem::take`, `mem::replace`, `Option::take` | moving a field out of `&mut self` | all |
| 4 | Borrow disjoint fields: destructure `self` | a loop over one field calls a method needing another | all |
| 5 | Split the struct, with methods on the parts | the conflict recurs (rung 4 everywhere) | all; hot/cold split in core |
| 6 | Copy hot fields into locals, write back once | hot loops over `&mut self` | core |
| 7 | Indices or handles instead of references | graphs, back-pointers, self-reference | all |
| 8 | A deliberate small clone, with a comment saying why | `Copy`-cheap or cold, measured or obviously negligible | adapter/bin |
| 9 | `Rc<RefCell<T>>` with `Weak` back-pointers; `Arc<Mutex<T>>` across threads | genuinely shared, mutable, dynamic lifetime | adapter/bin only |

Rungs 8 and 9 do not exist in domain or core: there is no allocator, and shared mutability
defeats the one-owner state machine. If core seems to need them, the state is in the wrong
place. Move it onto the machine and feed it as events (rust-architecture).

### Rung 3: move out with `mem::replace`

From `crates/<name>-core/src/machine.rs`. The finished line is moved out and a new one
moved in, in one expression, with no clone and no `Option` wrapper (excerpt):

```rust,ignore
/// Ends the current line and starts the next at the current position.
fn finish_line(&mut self) {
    let done = mem::replace(&mut self.line, Line::starting_at(self.pos)).complete();
    self.emit(Effect::Line(done.record()));
}
```

With a `Default` type, use `mem::take` (adapter code):

```rust
use std::mem;

/// Collects lines into batches.
#[derive(Debug, Default)]
pub struct Collector {
    current: Vec<String>,
    done: Vec<Vec<String>>,
}

impl Collector {
    /// Closes the current batch and starts an empty one, moving the lines
    /// rather than cloning them.
    pub fn flush(&mut self) {
        let full = mem::take(&mut self.current);
        self.done.push(full);
    }
}
```

### Rung 4: borrow disjoint fields

```rust,ignore
// Does not compile (E0502): `self.bump` borrows all of `self` mutably
// while `&self.words` is borrowed for the loop.
for w in &self.words {
    self.bump(*w);
}
```

Destructure, so the compiler sees two separate borrows (adapter code; in domain and core the
`u32` would be a domain type):

```rust
/// Counts.
#[derive(Debug, Default)]
pub struct Counts {
    seen: u32,
}

impl Counts {
    /// Adds `w` to what has been seen.
    pub fn bump(&mut self, w: u32) {
        self.seen = self.seen.saturating_add(w);
    }
}

/// Words and their running count.
#[derive(Debug)]
pub struct Tallier {
    words: [u32; 4],
    counts: Counts,
}

impl Tallier {
    /// Folds every word into the count.
    pub fn tally(&mut self) {
        let Self { words, counts } = self;
        for &w in words.iter() {
            counts.bump(w);
        }
    }
}
```

The method moved to the part it touches (`Counts::bump`). That is rung 5 in miniature, and
usually the better design anyway.

### Rung 6: one borrow per batch

In core hot loops, read hot fields into locals once, loop on the locals, write back once:

```rust
/// One read's bytes, borrowed. The entry seam is `of`.
#[derive(Clone, Copy, Debug)]
pub struct Wire<'a>(&'a [u8]);

impl<'a> Wire<'a> {
    /// A host's bytes.
    #[must_use]
    pub const fn of(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

/// A saturating count. The exit seam is `get`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tally(u64);

impl Tally {
    /// Nothing counted.
    pub const ZERO: Self = Self(0);

    const fn saturating_incr(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// The raw count, for interop.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Counts bytes and lines in a stream.
#[derive(Debug)]
pub struct Counter {
    bytes: Tally,
    lines: Tally,
}

impl Counter {
    /// Nothing counted yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: Tally::ZERO,
            lines: Tally::ZERO,
        }
    }

    /// Touches `self` once on the way in and once on the way out.
    pub fn feed(&mut self, wire: Wire<'_>) {
        let (mut bytes, mut lines) = (self.bytes, self.lines);
        for &b in wire.0 {
            bytes = bytes.saturating_incr();
            if b == b'\n' {
                lines = lines.saturating_incr();
            }
        }
        (self.bytes, self.lines) = (bytes, lines);
    }

    /// Lines seen so far.
    #[must_use]
    pub const fn lines(&self) -> Tally {
        self.lines
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}
```

Why: locals can live in registers, while stores through `&mut self` may be repeated inside
the loop; and the borrow of `self` no longer spans the loop body.

Whether it pays is a measurement (rust-measure): a view built per write pays its setup on
every write, and one built per batch pays once.

### Rung 7: handles instead of references

```rust
/// A handle to a node in a [`Graph`]. Only the graph mints them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeId(usize);

/// Nodes that refer to each other through handles, not references.
#[derive(Debug, Default)]
pub struct Graph {
    edges: Vec<Vec<NodeId>>,
}

impl Graph {
    /// Adds a node and returns its handle.
    pub fn add(&mut self) -> NodeId {
        self.edges.push(Vec::new());
        NodeId(self.edges.len() - 1)
    }

    /// Points `from` at `to`. Returns `None` if `from` is not in this graph.
    pub fn link(&mut self, from: NodeId, to: NodeId) -> Option<()> {
        self.edges.get_mut(from.0)?.push(to);
        Some(())
    }

    /// The nodes `id` points at.
    #[must_use]
    pub fn edges(&self, id: NodeId) -> &[NodeId] {
        self.edges.get(id.0).map_or(&[], Vec::as_slice)
    }
}
```

The handle is a newtype, so it cannot be confused with a count (C-NEWTYPE). If it must not
outlive a snapshot or cross containers, brand it with an invariant lifetime (rust-domain).

### Rung 9: shared ownership, adapters only

A tree whose children point back at their parent: `children: RefCell<Vec<Rc<Node>>>`,
`parent: RefCell<Weak<Node>>`; the back-pointer is `Weak`, so there is no reference cycle. Keep
`Rc`/`RefCell` inside the type and expose plain borrows or owned values (api-design.md §2).

## 3. Closures and threads

- Rebind before a `move` closure, so the closure captures exactly what it needs:

```rust
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// A line total that worker threads add to. The `Arc` stays inside.
#[derive(Debug, Default)]
pub struct SharedTotal {
    total: Arc<Mutex<u64>>,
}

impl SharedTotal {
    /// Counts `lines` on a worker thread and sends the new total back. A
    /// worker that finds the lock poisoned panics, and the receiver then
    /// sees a disconnect, not a total.
    ///
    /// # Panics
    ///
    /// If the OS cannot start a thread (`thread::spawn`).
    pub fn spawn_counter(&self, lines: Vec<String>) -> mpsc::Receiver<u64> {
        let (tx, rx) = mpsc::channel();
        let total = Arc::clone(&self.total);
        thread::spawn(move || {
            let n = u64::try_from(lines.len()).unwrap_or(u64::MAX);
            let now = {
                let mut guard = total.lock().expect("total lock poisoned");
                *guard = guard.saturating_add(n);
                *guard
            };
            let _sent = tx.send(now);
        });
        rx
    }
}
```

- `Arc::clone(&x)`, not `x.clone()`: a pointer copy should not read like a deep copy.
- The lock is held in a block that ends before `send`: short lock scopes, no returned guard.
- `let _sent = tx.send(now);` drops the result on purpose; the receiver may have gone away.
  `let _ = m.lock()` would drop the guard at once. Gate: rustc `let_underscore_lock`, deny by
  default.

## 4. No leaks: own instead

`Box::leak`, `Vec::leak`, `String::leak`, `mem::forget` and `ManuallyDrop::new` are banned under
`std::`, `alloc::` and `core::` paths (clippy `disallowed_methods`, `clippy.toml`). Wanting a
`'static` borrow is a sign the owner is missing. The owner order, the `self_cell` caveat and the
evidence are rust-architecture's (`references/zero-copy.md`, "Own, don't leak").
