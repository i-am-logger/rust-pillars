---
name: layering-fix
description: The core crate picked up a `log` dependency and now reports dropped effects only as debug logging; restore the layering without losing what a host can tell, through `refused()`, about what the core dropped.
tags: [smoke]
runs: 3
max_turns: 40
timeout_seconds: 900
---
`cargo xtask layers` started failing after someone added logging to `rust-code-core`. I need the gates green again, but I don't want to lose what a host can tell about what the core refused or dropped.

Before that change, every effect the machine dropped on a full queue was counted in what `Machine::refused()` reports, along with the other refusals, and the binary's end-of-run warning would print it with the rest. Our bin feeds the core in slices it can always hold, so it never hits this itself, but other hosts and tests drive `Machine` directly. A `Machine<1024>` handed 3000 one-word lines in a single `Event::Bytes` used to report 1976 dropped. Now `refused()` says nothing about it, and the only trace is one `log::debug!` line per dropped effect, which a host without a logger never sees.

I want that count back in `refused()`, so the bin's warning would carry it, and still nothing on stderr when nothing was refused.
