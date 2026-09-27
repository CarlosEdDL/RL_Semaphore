# 1.6 Fixed-time controller + CLI: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/fixed-time-cli`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Fixed-time plan in `sim`
- [ ] Add `FixedTimeConfig { green_s: Vec<f64> }` (`deny_unknown_fields`) and `fixed_time: Option<FixedTimeConfig>` on `ScenarioConfig`, with `#[serde(default, skip_serializing_if = "Option::is_none")]` ([R2.1](requirements.md#r2-fixed-time-plan-in-the-scenario)).
- [ ] Add `FixedTimePlan { green: Vec<u32>, cycle: u32 }` with `green_steps()` and `cycle_steps()`, stored on `Scenario` and returned by `Scenario::fixed_time()` (R2.5). Re-export both types from `lib.rs` ([R1.1](requirements.md#r1-crate-layout-and-compatibility)).
- [ ] `validate_fixed_time(config, step_s, &SignalPlan) -> Result<Option<FixedTimePlan>, ConfigError>`, called after the signal and demand checks in `ScenarioConfig::validate`. Reuse `to_steps(.., Rounding::Up)` (R2.2–R2.4).
- [ ] Derive the exact max-red inequality from the red-age definition in the `signal` module docs (a phase is not green for `C − g[j]` consecutive entries). Write it in the rustdoc of `validate_fixed_time`, then confirm it with the boundary test of task 5 before trusting it.
- [ ] New `ConfigError` variants (length mismatch, invalid green, green below min-green, cycle exceeds max-red), with rustdoc, path-naming messages, and the check order added to the `ConfigError` rustdoc (R2.3).
- [ ] Check `to_config` keeps `fixed_time` as configured (it clones `self.config`, so it should already), and that the round trip holds (R2.6).

### 2. `Controller` and `FixedTime` in `env`
- [ ] `crates/env/Cargo.toml`: depend on `rl-semaphore-sim` (path) and `thiserror`; keep `#![warn(clippy::pedantic)]` ([R1.2](requirements.md#r1-crate-layout-and-compatibility)). Replace the placeholder test and update the crate doc.
- [ ] `controller` module: the `Controller` trait (R3.1) and a `fixed_time` module with `FixedTime` (R3.2–R3.5). `command` is a `match` on `sim.signal().state()`; no fields beyond the green steps.

### 3. Episode runner in `env`
- [ ] `runner` module: `run_episode`, `EpisodeReport`, `SignalCounts` ([R4](requirements.md#r4-the-episode-runner)), with the loop of R4.2 written once and documented.
- [ ] `EnvError` with `Spawn(#[from] SpawnError)` and `Metrics(#[from] MetricsError)` (R4.4).
- [ ] Rustdoc example on `run_episode` running the example scenario for a few hundred steps (a doctest, like the 1.5 one).

### 4. `simulate` command
- [ ] Workspace: add `serde_json = "1"` to `[workspace.dependencies]`. `cli`: depend on `rl-semaphore-sim`, `rl-semaphore-env`, `serde`, `serde_json`; dev-depend on `insta` ([R1.3](requirements.md#r1-crate-layout-and-compatibility)).
- [ ] Turn `Command::Simulate` into a variant with an `Args` struct (`--config`, `--steps`, `--seed`, `--output`) ([R5.1](requirements.md#r5-the-simulate-command)). Update `Command::name` and the unit tests (`parses_each_subcommand` needs `--config`; `unimplemented_command_fails_with_its_name` already uses `train`).
- [ ] New `simulate.rs`: read → validate → `FixedTime::new` → `run_episode` → print, with `anyhow` context on each failure and the start/end `info` events with wall-clock time and steps/s on stderr (R5.2–R5.5).
- [ ] New `report.rs`: the text layout and the private serde structs for JSON ([R6](requirements.md#r6-output-formats)). Write to a `String` and print once, so an error can never leave half a table on stdout.

### 5. Tests
- [ ] `crates/sim/tests/fixed_time.rs`: the validation cases of [R8.1](requirements.md#r8-tests), built on `common::inline` scenarios with round timings so the limit can be computed by hand.
- [ ] `crates/env/tests/fixed_time.rs`: the controller cases of R8.2 and the runner cases of R8.4. For `switches_started`, count phase changes from the plan by hand for a short run (for example two and a half cycles).
- [ ] `crates/env/tests/fixed_time_props.rs`: the property test of R8.3. Add `proptest` as a dev-dependency of `env`. Generate the phase greens from the max-red limit so every case is valid, and add one hand-picked invalid case to the validation tests instead.
- [ ] `crates/cli/tests/cli.rs`: move the "not implemented" tests from `simulate` to `train` ([R1.5](requirements.md#r1-crate-layout-and-compatibility)), then add the cases of R8.5. Run the binary with `current_dir` set to the workspace root (`concat!(env!("CARGO_MANIFEST_DIR"), "/../..")`) and a relative `--config`.
- [ ] `crates/cli/tests/simulate_snapshot.rs`: text and JSON snapshots of R8.6. Run `cargo insta test` once, review the `.snap` files by hand (no forced switches, throughput close to the example demand of 1,100 veh/h minus what is still in the system, max wait no higher than about one cycle plus the queue ahead), and commit them.

### 6. Example config, README, ADR-0007
- [ ] Add the `[fixed_time]` table and its schema comment to `configs/single-intersection.toml` ([R2.7](requirements.md#r2-fixed-time-plan-in-the-scenario), [R9.1](requirements.md#r9-docs-adr-and-roadmap)). Check that the 1.4 and 1.5 snapshots do not move (R1.4).
- [ ] README: the `simulate` command under "Getting started", a "Baseline" section with the numbers from the snapshot, and the updated "Project status" (R9.2).
- [ ] Copy `specs/adr/0000-template.md` to `specs/adr/0007-fixed-time-controller.md`, fill it in, and add it to the index in `specs/adr/README.md` (R9.3–R9.4).

### 7. Roadmap
- [ ] Mark 1.6 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R9.5).

### 8. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit

# spot checks
cargo run -p rl-semaphore -- simulate --config configs/single-intersection.toml
cargo run -p rl-semaphore -- simulate --config configs/single-intersection.toml --output json | jq .schema_version
cargo run -p rl-semaphore -- simulate --config configs/single-intersection.toml --steps 0; echo $?   # 2
CI=true cargo nextest run --workspace --locked   # insta must not write snapshots in CI
git diff --exit-code crates/sim/tests/snapshots/
cargo doc -p rl-semaphore-sim -p rl-semaphore-env --no-deps
```

## Notes and risks

- **Off-by-one in the max-red check.** The red age counts entries including the current one, and the signal's schedule check is stated on the last entry before a phase turns green. Getting the inequality wrong by one either rejects a valid plan or accepts one the signal then forces. Do not guess: derive it, then pin it with the "exactly at the limit / one over" pair in R8.1 *and* run the at-limit plan through `FixedTime` for several cycles to show it is never forced (R8.3 covers random plans, but the boundary deserves its own case).
- **Why round-robin is never forced.** The signal forces a switch only when holding would make the *minimum* schedule (others served longest-red first, each for `G`) miss max-red. In a round-robin cycle the next phase is always the longest-red one, and the real schedule serves every phase no later than its `C − g[j]` bound, so a plan that passes R2.3 never trips it. If the property test finds a forced switch, the validation inequality is wrong, not the signal.
- **Adding `[fixed_time]` to the example.** The 1.4 and 1.5 tests run the example with `Command::Hold`, so the new table must not change their trajectories. It changes the parsed `ScenarioConfig`, so any test comparing a hand-written config against the example (or a round-tripped TOML string) may need the new field; that is an update, not a weakened assertion.
- **`serde_json` and cargo-deny.** It is already a dev-dependency, so its license set (including `ryu`) has passed `cargo deny`. Moving it to a normal dependency should not change that, but run `cargo deny` before pushing.
- **The CLI JSON is not the protocol.** Keep its structs private to the CLI and versioned with `schema_version`. When 2.1 adds `protocol` DTOs, the CLI can switch to them and bump the version; do not add serde derives to `sim` types here.
- **Snapshots and paths.** The printed config path comes from the argument, so the snapshot test must run from a fixed directory with a relative path. Never print an absolute or canonicalized path.
- **Scope creep toward 3.1 and 3.5.** Do not add observations, action masks, an actuated controller, multiple seeds, or an `eval` command. The `Controller` trait takes `&Simulation` and returns a `Command`, nothing more.
