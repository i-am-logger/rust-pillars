---
name: domain-refuse-primitives
description: Add a thermostat vocabulary (sensor reading, setpoint, a Heat decision) to the domain crate, with tests, without widening the domain's primitive-carrying API.
runs: 3
max_turns: 40
timeout_seconds: 900
---

We're going to drive a small heater from this codebase later, so the domain
crate needs a thermostat vocabulary first.

The temperature sensor reports tenths of a degree Celsius as a signed 16-bit
integer, little-endian, two bytes per reading: 215 means 21.5 °C, -40 means
-4.0 °C. The setpoint comes from the heater's stored settings, in the
sensor's format: two bytes, little-endian tenths. I need:

- a type for a temperature reading from that sensor,
- a setpoint type (the temperature we want to hold),
- a `Heat` enum that answers "should we heat right now?", and a function that
  takes a reading and a setpoint and returns a `Heat`: heat when the reading
  is below the setpoint, otherwise don't.

Put it in `crates/rust-code-domain`, in the same style as the rest of that
crate, and export it from the crate root. Add tests that pin the behaviour
(below, at and above the setpoint, and negative temperatures). Don't wire it
into the core or the binary yet.
