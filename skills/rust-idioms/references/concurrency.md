# Concurrency and async

Load this before you write a thread, a lock, a channel, an `async fn` or a runtime call. Every
rule here applies to adapters and bins only. Domain and core have none of it (section 1).

The std examples below compile and run under the snippets gate. Examples that need tokio are
marked `ignore` because the gate links only the domain and core crates and their
dev-dependencies, and tokio is not among them.

## 1. Where concurrency lives: the sans-IO split

The core is a state machine. It takes an event and returns effects. It has no threads, no locks,
no futures, no clock and no callbacks, so it never waits and has nothing to race. Concurrency is
how an adapter decides *when* to call the core and *where* its effects go.

| Concern | Lives in | Gate |
|---|---|---|
| state transitions, refusals, counters | core | `chunk_invariance_` test (`cargo xtask arch`) |
| threads, tasks, spawning | adapter or bin | crate-local `clippy.toml` bans `std::thread::spawn` and `std::thread::Thread` in domain and core |
| the runtime (`#[tokio::main]`, `Runtime::new`) | bin only | review |
| callbacks into user code | adapter | `callback_param` budget in core (`cargo xtask budgets`) |
| async I/O crates (tokio, futures) | adapter or bin | `layers.toml` `[roles.core] external = []` (`cargo xtask layers`) |

Two consequences shape everything below.
- **Give each task its own core value instead of sharing one.** The core is plain data, so it is
  `Send` and can move into a task. One task per connection, each owning its own machine, needs
  no lock at all.
- **Send state that must be shared to one owner task over a channel.** That task owns the
  machine and applies events in arrival order. It is the sans-IO version of "prefer message
  passing to shared state".

DEPARTS from BeautifulRust: it advises a library to take `impl Read` or `impl AsyncRead` from
the caller. A rust-pillars core takes neither. It takes bytes as events, and only the adapter
reads, which keeps the core identical under every runtime and under none.

## 2. `async fn`, and when to write `-> impl Future` instead

Write `async fn` by default. Why: the signature reads like a normal function, and the borrows
it captures are obvious. Gate: clippy `manual_async_fn` (style) flags a `fn -> impl Future`
whose body is just an `async` block.

Write `-> impl Future` in exactly two cases:
- **In a public trait, when callers need `Send`.** An `async fn` in a trait cannot promise that
  its future is `Send`, so a multi-threaded caller cannot spawn it. Write
  `fn get(&self) -> impl Future<Output = T> + Send` instead. Gate: rustc `async_fn_in_trait`
  (warn by default, which the shell's `-D warnings` turns into an error).
- **When work must happen before the first poll.** Examples are validating an argument, or
  copying a borrowed argument so that the future is `'static` and can be spawned.

```rust
use std::future::Future;

/// Starts a lookup. The key is copied now, so the returned future borrows
/// nothing from the caller and can be moved to another task.
fn lookup(key: &str) -> impl Future<Output = usize> + Send + 'static {
    let key = key.to_owned(); // runs at the call, before any poll
    async move { key.len() }
}

fn assert_send<T: Send>(_: &T) {}

let fut = lookup("alpha");
assert_send(&fut);
```

## 3. `Send` futures, and what breaks them

A future is `Send` only if everything it holds across an `.await` is `Send`. An `Rc`, a
`RefCell` borrow or a `MutexGuard` that is alive at an `.await` makes the whole future `!Send`.
The error then appears where the caller spawns it, far from the cause.

```rust,compile_fail,E0277
use std::rc::Rc;

async fn tick() {}

async fn count() -> usize {
    let n = Rc::new(1);
    tick().await; // `n` is alive across this point
    *n
}

fn assert_send<T: Send>(_: T) {}
assert_send(count()); // error[E0277]: `Rc<i32>` cannot be sent between threads safely
```

The fix is to end the borrow, or drop the value, before the `.await`:

```rust
use std::rc::Rc;

async fn tick() {}

async fn count() -> usize {
    let n = {
        let rc = Rc::new(1);
        *rc
    }; // `rc` is gone before the await
    tick().await;
    n
}

fn assert_send<T: Send>(_: T) {}
assert_send(count());
```

Pin `Send` in a test for every public `async fn` (the SKILL.md rule), because nothing else
catches a regression until a caller spawns the future. Gates: clippy `await_holding_lock` and
`await_holding_refcell_ref` (both suspicious, so denied) catch the guard cases. clippy
`future_not_send` is nursery and off, so the general case is the test.

## 4. Locks

- **Never hold a lock across `.await`.** Why: the task can be suspended while it holds the
  lock, and every other task that wants the lock then waits on a task that may never be
  polled. Gate: clippy `await_holding_lock`. The std `Mutex` is the right choice in async code
  when the lock is never held across an `.await`. Use `tokio::sync::Mutex` only when the lock
  must be held across one, and it is slower.
- **Keep linked invariants under one lock.** If two fields must agree (a total and the entries
  it sums), put both in one struct behind one `Mutex`. Why: with two locks, another thread can
  observe the state between the two updates, and taking them in different orders deadlocks.
- **Never call user code while holding a lock.** Callbacks, closures, trait objects and
  `Drop` of user values all count. Why: the callee may take the same lock (a std `Mutex` is not
  re-entrant, so it deadlocks or panics), or block for as long as it likes. Copy what you need
  out, drop the guard, then call.
- **Hold a lock for as few statements as you can, and never return a guard.** The worked example is in
  ownership.md §3.

```rust
use std::sync::{Arc, Mutex};

/// Entries and their total change together, so they share one lock.
#[derive(Debug, Default)]
struct Ledger {
    entries: Vec<u64>,
    total: u64,
}

type Listener = Arc<dyn Fn(u64) + Send + Sync>;

#[derive(Default)]
struct Book {
    ledger: Mutex<Ledger>,
    listeners: Mutex<Vec<Listener>>,
}

impl Book {
    fn post(&self, amount: u64) {
        let total = {
            let mut l = self.ledger.lock().expect("ledger lock poisoned");
            l.entries.push(amount);
            l.total += amount; // both halves of the invariant, under one guard
            l.total
        };
        // Snapshot the listeners, release the lock, then call user code.
        let listeners: Vec<Listener> =
            self.listeners.lock().expect("listeners lock poisoned").clone();
        for f in &listeners {
            f(total);
        }
    }

    fn listen(&self, f: Listener) {
        self.listeners.lock().expect("listeners lock poisoned").push(f);
    }
}

let book = Arc::new(Book::default());
let seen = Arc::new(Mutex::new(Vec::new()));
let (b, s) = (Arc::clone(&book), Arc::clone(&seen));
book.listen(Arc::new(move |t| {
    s.lock().expect("seen lock poisoned").push(t);
    // Re-entering `book` here is safe: `post` holds no lock while calling.
    let _ = b.ledger.lock().expect("ledger lock poisoned").total;
}));
book.post(3);
book.post(4);
assert_eq!(*seen.lock().expect("seen lock poisoned"), [3, 7]);
```

## 5. Long CPU work in async code

An executor thread runs one task at a time. A task that computes for milliseconds without an
`.await` stalls every other task on that thread, including their timers. Alice Ryhl's "Async:
What is blocking?" (she maintains tokio) puts the budget between `.await` points at roughly 10
to 100 microseconds.
- Work that is long but can be split into chunks: call `tokio::task::yield_now().await` between
  chunks.
- Work that cannot be split, or blocking calls such as `std::fs` or a synchronous library: use
  `tokio::task::spawn_blocking`, or a dedicated thread that you feed through a channel.
- A sans-IO core makes this easy, because its unit of work is one event. The adapter decides
  how many events to feed between yields.

```rust,ignore
// ignore: tokio is not linked by the snippets gate.
async fn digest_all(blobs: Vec<Vec<u8>>) -> Result<Vec<u64>, tokio::task::JoinError> {
    // Hashing is pure CPU; keep it off the executor threads.
    tokio::task::spawn_blocking(move || blobs.iter().map(|b| checksum(b)).collect()).await
}
```

## 6. Bounded channels and backpressure

Make every channel bounded. Why: an unbounded queue turns a slow consumer into memory growth
that you never see until the process is killed. A bounded queue gives you a choice:
- **Wait (backpressure).** The producer waits when the queue is full, so the slowdown travels
  back to whoever is producing. Use `send` on `std::sync::mpsc::sync_channel(n)`, or
  `.send().await` on `tokio::sync::mpsc::channel(n)`.
- **Refuse and count.** Use `try_send`, and count the refusal. This is the core's own error
  model (errors.md §2), applied at the edge.

The bound is a named const with a doc comment that says where the number came from. Gate:
review, because nothing flags `mpsc::channel()` or `unbounded_channel()`.

```rust
use std::sync::mpsc::{self, TrySendError};
use std::thread;

/// Queue depth between reader and parser: one burst of input (illustrative).
const DEPTH: usize = 2;

let (tx, rx) = mpsc::sync_channel::<u32>(DEPTH);
tx.send(1).expect("receiver alive");
tx.send(2).expect("receiver alive");

// Full: refuse and count, instead of growing.
let mut refused = 0_u32;
if let Err(TrySendError::Full(_)) = tx.try_send(3) {
    refused += 1;
}
assert_eq!(refused, 1);

// Or wait: this producer blocks until the consumer makes room.
thread::scope(|s| {
    s.spawn(|| tx.send(4).expect("receiver alive"));
    assert_eq!(rx.recv().expect("sender alive"), 1);
});
assert_eq!(rx.try_iter().collect::<Vec<_>>(), [2, 4]);
```

## 7. Cancellation safety and `select!`

When one branch of `tokio::select!` completes, the futures of the other branches are dropped.
Dropping a future cancels it at its last `.await`. Any progress it made but had not yet
returned is lost: bytes already read, or a message already taken from a queue.
- Use only cancel-safe operations in a `select!` that runs in a loop. tokio documents this per
  method. `mpsc::Receiver::recv` and `AsyncReadExt::read` are cancel-safe. `read_exact`,
  `read_to_end` and `AsyncWriteExt::write_all` are not.
- To keep a future that is not cancel-safe, create it once outside the loop, pin it, and poll it
  by `&mut` so the loop never drops it.
- Document cancel safety in a `# Cancel safety` section on every public `async fn` that callers
  might put in a `select!`. Why: callers cannot see it in the signature.

```rust,ignore
// ignore: tokio is not linked by the snippets gate.
let deadline = tokio::time::sleep(LIMIT);
tokio::pin!(deadline); // created once; polled by &mut, never dropped by the loop
loop {
    tokio::select! {
        msg = rx.recv() => match msg { Some(m) => core.handle(m.into()), None => break },
        () = &mut deadline => break,
    }
}
```

## 8. Structured concurrency

Every thread or task you start has an owner that joins it. Why: a detached task's panic and
its error disappear, and it can outlive the state it uses.
- Threads that borrow local data: `std::thread::scope`. Every thread is joined before the scope
  returns, a panic in one comes back out of `scope`, and no `Arc` is needed. The example in
  section 6 uses one.
- Tasks: `tokio::task::JoinSet`, owned by the code that started them. Joining yields each
  result, and dropping the set aborts whatever is still running.
- Never drop a `JoinHandle` to detach. Gate: review. clippy `let_underscore_future` only catches
  `let _ = fut;`.

```rust,ignore
// ignore: tokio is not linked by the snippets gate.
let mut tasks = tokio::task::JoinSet::new();
for conn in listener_batch {
    tasks.spawn(serve(conn)); // each task owns its own core machine (section 1)
}
while let Some(done) = tasks.join_next().await {
    done??; // a panic (JoinError) or a serve error both surface here
}
```

## 9. Libraries never spawn or choose a runtime

Only the bin starts a runtime. An adapter may be written against one runtime's I/O types. It
must not call `#[tokio::main]`, build a `Runtime`, or spawn a detached task. It returns futures,
or it takes the `JoinSet` or spawner it should use. Why: starting a runtime, or calling
`block_on`, inside a runtime's context panics, and a hidden runtime competes with the bin's
for threads. Only the bin knows which runtime it is using. Gate: review. In domain and core, `std::thread::spawn`
is a `disallowed_methods` error through the crate-local `clippy.toml`.

This is BeautifulRust's rule. rust-pillars places it on the layering: the bin owns the runtime,
adapters lend it work, and the core has nothing to schedule.
