# Roadmap

Each phase is **about one PR** (1–2 days of work). It must leave `main` green (CI passing) and deliver something runnable, visible, or measurable. The phases are ordered: the world is built and seen first, then RL is added, then the system scales.

Legend: ☐ not started · ◐ in progress · ☑ done

---

## Stage 0: Foundations

- ☑ **0.1 Workspace skeleton.** Cargo workspace with empty crates (`sim`, `env`, `agents`, `trainer`, `storage`, `protocol`, `server`, `web`, `cli`), `rust-toolchain.toml`, rustfmt/clippy config, README.
- ☑ **0.2 CI pipeline.** GitHub Actions: fmt, clippy `-D warnings`, nextest, cargo-deny, with caching.
- ☑ **0.3 Observability & errors baseline.** `tracing` setup in `cli`, error conventions (`thiserror`/`anyhow`), `CONTRIBUTING.md`, first ADR (why Rust + Burn + Leptos).

## Stage 1: Simulator (single intersection, no RL)

- ☑ **1.1 Road model.** Types for a single 4-way intersection: approaches, lanes, stop lines. TOML config loading and validation.
- ☑ **1.2 Signal model.** Phases, yellow and all-red clearance, min-green, **max-red**. Tests that conflicting movements are never green together.
- ☑ **1.3 Vehicles & movement.** Spawn, advance, queue at red, and cross on green in a fixed time step. Property tests: vehicles are conserved and never overlap.
- ☑ **1.4 Demand generation.** Seeded Poisson arrivals per approach. A determinism test (same seed produces an identical trajectory, checked with an `insta` snapshot).
- ☑ **1.5 Metrics.** Per-vehicle wait tracking, and mean / p95 / p99 / max wait, throughput, and queue length per episode.
- ☑ **1.6 Fixed-time controller + CLI.** `rl-semaphore simulate` runs N steps and prints metrics. This is the first baseline number.
- ☑ **1.7 Sim benchmark.** A `criterion` benchmark for steps/sec, recorded in the README.

## Stage 2: Visualization of the simulator

- ☑ **2.1 Protocol crate.** Versioned `Snapshot` and `Metrics` DTOs with serde round-trip tests.
- ☑ **2.2 Axum server.** Health endpoint and a WebSocket that streams snapshots from a running fixed-time sim.
- ☐ **2.3 Leptos app shell.** Build with `cargo-leptos`, served by Axum, with a connection status indicator.
- ☐ **2.4 Canvas renderer.** Draws the intersection, lanes, lights, and vehicles live. **This is the first visible milestone.**
- ☐ **2.5 Live metrics panel.** Current queue lengths and a wait chart (plotters). Vehicles are color-coded by wait time, and starved vehicles are highlighted.
- ☐ **2.6 Dockerize.** Multi-stage Dockerfile (cargo-chef), `docker compose up` serves the demo, and CI builds the image.

## Stage 3: RL environment

- ☐ **3.1 Env trait.** A Gymnasium-like `reset(seed)` / `step(action)` / `observation` / `action_mask` API over `sim`.
- ☐ **3.2 Observation & action spaces.** Per-lane queue, wait, current phase, and time in phase. Action: keep the phase or switch to phase *k*.
- ☐ **3.3 Safety layer.** Action masking that enforces min-green, yellow, and **max-red** (non-bypassable). Property test: no episode ever exceeds max-red.
- ☐ **3.4 Fair reward function.** Negative mean wait plus a super-linear individual-wait penalty, with configurable weights and unit tests on hand-crafted states.
- ☐ **3.5 Baselines in env.** Fixed-time and simple actuated (queue-threshold) controllers, plus an `eval` command that reports mean and tail metrics over K seeds.

## Stage 4: First agent (DQN, single intersection)

- ☐ **4.1 Burn setup & Q-network.** An MLP Q-network with the NdArray backend and a forward-pass test.
- ☐ **4.2 Replay buffer & ε-schedule.** Unit-tested.
- ☐ **4.3 DQN update.** Target network, Double DQN, Huber loss, masked argmax. Sanity test: it learns a trivial toy env.
- ☐ **4.4 Training loop + CLI.** `rl-semaphore train --config ...`, with seeded, deterministic runs.
- ☐ **4.5 Checkpointing.** Save and load with Burn's recorder, and resume training.
- ☐ **4.6 Beat the baseline.** Evaluation report: DQN vs fixed-time vs actuated on **mean and p95/p99/max** wait. Results are documented.

## Stage 5: Experiment tracking & training visualization

- ☐ **5.1 Storage crate.** SQLite schema and migrations for runs (config, seed, git SHA, status) and per-episode metrics.
- ☐ **5.2 Trainer → storage.** Metrics are persisted during training, and checkpoints are indexed.
- ☐ **5.3 Runs API.** REST endpoints to list runs, fetch metrics, and fetch configs.
- ☐ **5.4 Training dashboard.** Run list and training curves (reward, mean wait, p99 wait, ε/loss).
- ☐ **5.5 Watch a policy.** Load a checkpoint and stream its greedy rollout to the canvas.

## Stage 6: Scale to a grid city (independent agents)

- ☐ **6.1 Road network graph.** Generalize the sim to a graph of intersections joined by road segments.
- ☐ **6.2 Grid presets & routing.** 2×2, 3×3, and 5×5 TOML presets, with origin–destination demand and simple shortest-path routes.
- ☐ **6.3 Grid rendering.** The canvas renders the full city with pan and zoom.
- ☐ **6.4 Multi-agent env.** PettingZoo-style parallel API with per-intersection observations, actions, rewards, and masks.
- ☐ **6.5 Independent DQN.** One agent per intersection, with an optional shared-parameter mode. Evaluated against grid baselines (including green-wave fixed-time).
- ☐ **6.6 City-level fairness metrics.** Trip-level wait distribution, per-intersection tail metrics, and a starvation heatmap in the UI.

## Stage 7: PPO

- ☐ **7.1 Actor-critic network & rollout buffer.**
- ☐ **7.2 GAE & PPO clipped loss with action masking.** Toy-env sanity test.
- ☐ **7.3 PPO on single intersection.** Compared with DQN.
- ☐ **7.4 PPO on grid.** Independent or shared-parameter PPO.

## Stage 8: Live training control

- ☐ **8.1 Run manager.** The server spawns and supervises training jobs on worker threads, with a status state machine.
- ☐ **8.2 Start / pause / stop from UI.** Plus a config editor with validation.
- ☐ **8.3 Live training stream.** Watch the agent act *during* training, with a configurable frame rate so it doesn't slow training down.
- ☐ **8.4 Hyperparameter edits between runs.** Clone a run with modified config.

## Stage 9: Replay & comparison

- ☐ **9.1 Episode recording.** Compact on-disk format for snapshots.
- ☐ **9.2 Replay player.** Play, pause, scrub, and change speed.
- ☐ **9.3 Side-by-side comparison.** Two policies (or policy vs baseline) on the same seed, with synced playback and a metric diff.
- ☐ **9.4 Run comparison charts.** Overlay the curves of several runs.

## Stage 10: Traffic scenarios & robustness

- ☐ **10.1 Time-varying demand.** Rush-hour profiles.
- ☐ **10.2 Incidents.** Lane closures and a blocked segment.
- ☐ **10.3 Scenario suite eval.** Every policy is evaluated across all scenarios, and a report table is shown in the UI.
- ☐ **10.4 Coordination experiment.** Neighbor-aware observations, compared against independent agents.

## Stage 11: Hardening & public deployment

- ☐ **11.1 Performance pass.** Profile the sim and trainer, add the optional `wgpu` backend, and use a binary WS encoding if needed.
- ☐ **11.2 Security & limits.** Rate limiting, a read-only public mode, and input validation audit.
- ☐ **11.3 Release pipeline.** Tagged releases push the image to GHCR, with a changelog.
- ☐ **11.4 Public demo deployment.** For example Fly.io: a read-only demo with pre-trained policies.
- ☐ **11.5 Portfolio polish.** README with GIFs, architecture docs, and a results write-up.
