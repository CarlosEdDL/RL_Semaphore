# 1.1 Road model

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R7) · [plan.md](plan.md) (tasks).

## Goal

Give the simulator its first real types: the static geometry of one 4-way intersection. At the end of this phase the `sim` crate can parse a TOML description of an intersection (four approaches, each with one or more lanes, each lane with its allowed movements and a length), validate it, and turn it into an immutable, discretized `Intersection` model with lanes split into fixed-length cells and a stop line at the downstream end of every lane. A checked-in example config loads cleanly in a test, and every invalid config produces a typed error that names the offending field. The spatial model (discrete cells) is recorded in ADR-0002.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-dependencies) | Workspace dependencies (`serde`, `toml`, `thiserror` in `sim`) |
| [R2](requirements.md#r2-domain-types) | `Direction`, `Movement`, IDs, `Lane`, `Approach`, `Intersection`, stop lines |
| [R3](requirements.md#r3-toml-configuration) | TOML schema, parsing from a string, serialization |
| [R4](requirements.md#r4-validation) | Validation rules and the `ConfigError` enum |
| [R5](requirements.md#r5-example-config-and-tests) | `configs/single-intersection.toml` and the test suite |
| [R6](requirements.md#r6-adr-0002-discrete-cell-model) | ADR-0002: discrete cell model |
| [R7](requirements.md#r7-docs-and-roadmap) | README, tech-stack, roadmap update |

## Decisions taken

- **Spatial model: discrete cells.** Each lane is a sequence of fixed-length cells (one vehicle per cell), in the family of Nagel–Schreckenberg / cell-transmission models. It is fast, trivially collision-free, and fully deterministic with integer positions. The config is written in meters, and `sim` converts lengths to cell counts once, at load time. Recorded in ADR-0002.
- **Lanes and movements:** each approach has 1 to 4 lanes. Each lane lists its allowed movements (`left`, `through`, `right`), so both shared lanes (`["through", "right"]`) and dedicated turn lanes (`["left"]`) are expressible. Signal phases in 1.2 grant movements, not lanes.
- **Integration level:** library types, TOML parsing, and validation in `sim`, plus one example file in `configs/` that a test loads. `sim` parses from a `&str` and does no file I/O. The CLI reads files starting in 1.6.
- **Single intersection only.** Exactly one 4-way intersection with North/East/South/West approaches. Generalizing to a road graph is phase 6.1.
- **Right-hand traffic.** Movement destinations assume vehicles drive on the right.

## Out of scope

- Signals, phases, and the movement conflict matrix (phase 1.2).
- Vehicles, speeds, time step, and movement through cells (phase 1.3).
- Demand and arrival rates (phase 1.4).
- Exit (outbound) lanes as simulated space. A vehicle that crosses the stop line leaves the model in Stage 1; exits exist only as destination directions.
- T-junctions or approaches with zero lanes. All four approaches are required.
- Reading files, `--config` flags, or any CLI change.
- Grid presets and road segments between intersections (phase 6.x).

## Acceptance criteria

1. `configs/single-intersection.toml` exists, and a test parses and validates it into an `Intersection` with four approaches, the expected lane counts, and the expected cell counts. (R3, R5.1)
2. Every validation rule in R4 has at least one test with an invalid config that returns the matching `ConfigError` variant, and the error's `Display` names the field path (for example `approaches.north.lanes[1]`). (R4, R5.2)
3. Unknown TOML keys and missing required keys are rejected with a parse error, not ignored. (R3.3)
4. `Direction::destination` (or equivalent) maps every approach and movement to the correct exit direction for right-hand traffic, and is tested for all 12 combinations. (R2.3)
5. Serializing a validated config back to TOML and parsing it again gives an equal model. (R3.5)
6. `sim` has no `anyhow`, no I/O, no `HashMap`, and no `unwrap`/`expect` outside tests without an `// INVARIANT:` comment. Clippy pedantic passes. (R2.7, R4.6)
7. `specs/adr/0002-discrete-cell-model.md` exists with status `Accepted`, and the ADR index lists it. (R6)
8. `cargo deny check` passes with the new dependencies. CI is green. (R1.3)
9. Roadmap entry 1.1 is marked ☑. (R7.3)
