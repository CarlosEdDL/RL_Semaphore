# 1.6 Fixed-time controller + CLI: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: `n` is the number of phases. `G`, `Y`, `A` and `M` are min-green, yellow, all-red and max-red in steps (as in ADR-0003). `g[i]` is the fixed-time green of phase `i` in steps. The cycle is `C = Σ (g[i] + Y + A)` over all phases. An *entry* is a signal state, as defined in the `signal` module docs.

## R1. Crate layout and compatibility

- **R1.1** The timing plan (`FixedTimeConfig`, `FixedTimePlan`) MUST live in `sim`, next to the scenario, and be re-exported from the crate root. `sim` MUST NOT gain any new dependency.
- **R1.2** `Controller`, `FixedTime`, `run_episode`, its result type and `EnvError` MUST live in `env` and be re-exported from its crate root. `env` MUST depend on `sim` and `thiserror` only, and MUST enable clippy pedantic.
- **R1.3** The `rl-semaphore` binary MAY depend on `sim`, `env`, `serde` and `serde_json` (moved from dev-dependency to dependency and added to `[workspace.dependencies]`). No other new dependency, except `insta` as a dev-dependency of `cli`.
- **R1.4** The step rule, the signal, demand generation and the metric definitions MUST NOT change. The snapshots `demand_snapshot__example_trajectory_is_pinned.snap` and `metrics_snapshot__example_summary_is_pinned.snap` MUST stay byte-identical, even though the example config gains a `[fixed_time]` table.
- **R1.5** Existing 1.1–1.5 tests MUST keep passing, and no existing assertion may be weakened or removed. The CLI tests that use `simulate` as the "not implemented" command MUST switch to another unimplemented command (for example `train`), with the same checks.

## R2. Fixed-time plan in the scenario

- **R2.1** `ScenarioConfig` MUST gain `fixed_time: Option<FixedTimeConfig>`, read from an optional `[fixed_time]` table and omitted from serialized TOML when `None`. `FixedTimeConfig` MUST have one field, `green_s: Vec<f64>`, and reject unknown keys.
- **R2.2** `green_s[i]` is the green duration of phase `i`, in the order of `[[signal.phases]]`. Each value MUST be converted to steps with the rules of `min_green_s` (snap within 1e-9, otherwise rounded up).
- **R2.3** Validation MUST return a `ConfigError` (new variants, each with rustdoc and a message naming the field path) when:
  - the table is present and `green_s.len() != n`;
  - a value is not finite, is not greater than 0, or does not fit in `u32` steps;
  - a value converts to fewer than `G` steps;
  - for `n ≥ 2`, some phase `j` would stay red for more than `M`: with the red age defined in the `signal` module, phase `j` is not green for `C − g[j]` consecutive entries in every cycle, and that red age MUST NOT exceed the bound the signal enforces. The implementation MUST derive the exact inequality from the signal's red-age definition and pin the boundary with a test (R8.1).
  Fixed-time checks run after signal validation, and the order of the checks MUST be added to the rustdoc on `ConfigError`.
- **R2.4** With `n = 1` the length and min-green checks apply, and the cycle check does not (the only phase is never red).
- **R2.5** `Scenario::fixed_time(&self) -> Option<&FixedTimePlan>` MUST return the validated plan. `FixedTimePlan` MUST expose `green_steps(&self) -> &[u32]` (one per phase, in phase order) and `cycle_steps(&self) -> u32` (`C`, computed with checked arithmetic at validation time), and derive `Debug`, `Clone` and `PartialEq`.
- **R2.6** `Scenario::to_config` MUST return the configured `green_s` seconds unchanged, so the round trip `Scenario → ScenarioConfig → TOML → Scenario` gives an equal scenario, with and without the table.
- **R2.7** The schema comment at the top of `configs/single-intersection.toml` MUST document `[fixed_time]`, and the file MUST contain a plan that passes validation (R9.1).

## R3. The Controller trait and FixedTime

- **R3.1** `env` MUST define `pub trait Controller { fn command(&mut self, sim: &Simulation) -> Command; }`, with rustdoc stating when it is called in the loop (R4.2). It MAY have a `name(&self) -> &str` method for logs and output.
- **R3.2** `FixedTime::new(plan: &FixedTimePlan) -> FixedTime` MUST build the controller from a validated plan. `FixedTime` MUST be `Debug` and `Clone`.
- **R3.3** `FixedTime::command` MUST depend only on `sim.signal().state()` and the plan:
  - `Green { phase: p, elapsed }` with `n ≥ 2` and `elapsed ≥ g[p]`: `SwitchTo((p + 1) mod n)`;
  - any other `Green` state, and every `Yellow` or `AllRed` state: `Hold`.
- **R3.4** Resulting timing, for a plan that passed R2.3: phase `p` is green for exactly `g[p]` consecutive entries each time it is served, phases are served in list order starting with phase 0 at entry 0, and each full cycle lasts exactly `C` steps. Every `StepOutcome` is `Held` or `SwitchStarted`; never `SwitchForced` or `Ignored`.
- **R3.5** The rustdoc on `FixedTime` MUST state the timing of R3.4 and why the controller keeps no clock of its own.

## R4. The episode runner

- **R4.1** `env::run_episode(scenario: &Scenario, controller: &mut impl Controller, seed: u64, steps: u64) -> Result<EpisodeReport, EnvError>` MUST run one episode on a fresh `Simulation` of `scenario`, with `Demand::new(scenario, seed)` and an `EpisodeMetrics` created before the first step.
- **R4.2** Each of the `steps` iterations MUST, in order: draw the arrivals and spawn each one; call `controller.command(&sim)`; call `sim.step` with that command; call `metrics.observe` with the returned report. The rustdoc MUST state this order.
- **R4.3** `EpisodeReport` MUST contain the `EpisodeSummary` (taken after the last step) and a `SignalCounts` with `switches_started`, `switches_forced` and `commands_ignored` (all `u64`), counted from the `StepOutcome` of every step. Both types MUST derive `Debug`, `Clone` and `PartialEq`, with rustdoc on every public field.
- **R4.4** `EnvError` MUST derive `thiserror::Error`, be `#[non_exhaustive]`, and have rustdoc on every variant. A `SpawnError` or `MetricsError` MUST be returned as an error, never unwrapped, even though neither can happen for a validated scenario and this loop.
- **R4.5** `steps = 0` MUST return a report with a 0-step summary and zero counters.
- **R4.6** `run_episode` MUST NOT log, read the clock, or do I/O. Timing and logging belong to the caller.

## R5. The simulate command

- **R5.1** `rl-semaphore simulate` MUST accept:
  - `--config <PATH>`: required, the scenario TOML;
  - `--steps <N>`: default `3600`, an integer ≥ 1 (clap `value_parser` range);
  - `--seed <S>`: default `0`, a `u64`;
  - `--output <text|json>`: default `text`.
  Each MUST have help text. The global logging flags keep working before and after the subcommand.
- **R5.2** The command MUST read the file, parse and validate it with `Scenario::from_toml_str`, build `FixedTime` from `scenario.fixed_time()`, call `run_episode`, and print the result to stdout.
- **R5.3** Errors MUST exit with code 1 through the existing `run` → `tracing::error!` path, with context naming the file: unreadable file, invalid scenario (the `ConfigError` message), and a scenario without `[fixed_time]` (the message MUST say the table is required by `simulate`). On error nothing MUST be printed to stdout. Usage errors keep clap's exit code 2.
- **R5.4** Logs MUST go to stderr only. The command SHOULD log at `info` the scenario path, seed and steps when it starts, and when it ends the wall-clock time and steps per second. The wall-clock values MUST NOT appear on stdout.
- **R5.5** The simulation code MUST NOT be duplicated in the CLI: all stepping goes through `run_episode`.

## R6. Output formats

- **R6.1** Text output MUST be a fixed layout, one fact per line or table row, containing: the config path as given, the seed, the steps and the duration in seconds; the controller name and its green times per phase (phase name and seconds) and cycle length; departed, in system and throughput in veh/h; a wait table (count, mean, p50, p95, p99, max, in seconds) with a row for all vehicles and one per approach in `Direction::ALL` order, `-` for an empty population; a queue table (mean, max) with a row per lane, per approach and the total; and the three signal counters.
- **R6.2** Floats in text output MUST use a fixed number of decimals (one for seconds and veh/h, two for queue means is a good default) so the snapshot cannot change with float formatting details.
- **R6.3** `--output json` MUST print exactly one JSON document followed by a newline, built from serde structs private to the CLI. It MUST contain `schema_version` (the integer `1`), `config` (path as given), `seed`, `steps`, `step_s`, a `controller` object (`kind: "fixed_time"`, per-phase `name` and `green_s`, `cycle_s`), a `summary` object with every `EpisodeSummary` field (wait stats as objects, or `null` when `None`; per-approach values keyed by approach name; lanes as an array of `{approach, lane, mean, max}`), and a `signal` object with the three counters.
- **R6.4** Text and JSON output of the same run MUST report the same numbers (up to the text rounding).
- **R6.5** The output formatting SHOULD live in its own module of the CLI (for example `simulate.rs` with `report.rs`), not in `main.rs`.

## R7. Determinism and robustness

- **R7.1** Same config, seed and steps MUST give byte-identical stdout, for both output formats.
- **R7.2** Nothing in `sim` or `env` may panic for a validated scenario, any seed and any `steps`. Casts follow the 1.1 rule: range-check first, then a local `#[allow(clippy::cast_*)]` with an `// INVARIANT:` comment.
- **R7.3** No `unwrap()`/`expect()` outside tests. Clippy pedantic MUST pass for `sim` and `env`, `-D warnings` for the workspace, and the WASM build of `web` MUST still pass.

## R8. Tests

- **R8.1** Plan validation tests (in `crates/sim/tests/`) MUST cover: a valid plan and its step values; a missing table (loads, `fixed_time()` is `None`); a wrong length; a zero, negative, NaN and infinite value; a green below min-green, and one that is exactly min-green after rounding; a cycle exactly at the max-red limit (accepted) and one step over it (rejected); a single-phase plan; and the TOML round trip with and without the table.
- **R8.2** `FixedTime` tests (in `crates/env/tests/`) MUST cover, on hand-built scenarios with known timings: the command for every signal state kind; that each phase is green for exactly `g[p]` consecutive entries over at least three cycles; that the cycle length is `C`; and a single-phase plan that always holds.
- **R8.3** A property test MUST generate valid fixed-time plans (random phase greens from `G` up to the max-red limit, on a scenario with 2–4 phases) and run them for at least 3 cycles, checking that no step returns `SwitchForced` or `Ignored`.
- **R8.4** Runner tests MUST cover: the example with a fixed seed returns `summary.steps == steps`, `switches_forced == 0` and `commands_ignored == 0`; `switches_started` equals the number of phase changes the plan implies for that many steps; two runs with the same seed are equal and two seeds differ; `steps = 0`.
- **R8.5** CLI tests (in `crates/cli/tests/cli.rs`, running the built binary) MUST cover: success with exit code 0 and an empty-of-logs stdout in both formats; that the JSON parses and has `schema_version == 1`; `--steps 0` and a non-numeric seed exit 2; a missing file, an invalid scenario, and a scenario without `[fixed_time]` exit 1 with an error on stderr and nothing on stdout; `--log-format json` keeps stdout clean and stderr JSON; byte-identical stdout for two runs with the same seed.
- **R8.6** `insta` snapshots MUST pin the text and the JSON stdout of `simulate --config configs/single-intersection.toml --seed 0 --steps 3600`, run from the workspace root so the printed path is stable.
- **R8.7** Every test MUST be deterministic (fixed seeds, no clock in assertions).

## R9. Docs, ADR and roadmap

- **R9.1** `configs/single-intersection.toml` MUST gain a `[fixed_time]` plan that serves the example demand without over-saturation and satisfies max-red (for instance 10 s for each left or side-road phase and 30 s for north–south through, giving an 80 s cycle). The chosen values and why MUST be stated in a comment above the table.
- **R9.2** The README MUST show the `simulate` command, and a short "Baseline" section with the example's results (seed 0, 3600 steps): mean, p95, p99 and max wait, throughput, and the timing plan. "Project status" MUST be updated.
- **R9.3** `specs/adr/0007-fixed-time-controller.md` MUST follow the template and record: the timing plan in the scenario and the controller in `env`; validation of the cycle against max-red instead of letting the signal force switches; a stateless controller that reads the signal state; the `Controller` trait and the runner loop order; stdout/stderr separation and the versioned CLI JSON. Its Context MUST name the alternatives considered (controller in `sim` or only in the CLI, a uniform `--green-s` flag, Webster timing from demand, a separate controller config file, a controller with its own timer, accepting plans the signal must correct, serde on `sim` types). Its Consequences MUST include the costs (a controller concern in the `sim` schema, the trait may change in 3.1, a CLI-local JSON schema to keep in sync until 2.1) and the benefits.
- **R9.4** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.
- **R9.5** Roadmap entry 1.6 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
