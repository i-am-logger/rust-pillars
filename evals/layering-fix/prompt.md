---
name: layering-fix
description: The core crate picked up a `log` dependency and now reports dropped effects only as debug logging; restore the layering without losing what the binary can tell about what the core dropped.
runs: 3
max_turns: 40
timeout_seconds: 900
---
`cargo xtask layers` started failing after someone added logging to `rust-code-core`. I need the gates green again, but I don't want to lose what the binary can tell me about what the core refused or dropped.

Before that change, a run that overflowed the queue said so at the end. For example:

```
yes ab | head -n 3000 > target/in
RUST_LOG=warn cargo run -q -- < target/in > /dev/null
```

used to warn on stderr that 1976 effects were dropped, along with the other refusals. Now it prints nothing unless I turn on debug logging, and then it's one line per dropped effect. I want that warning with the count back, and nothing on stderr when nothing was dropped.
