# 0001. Rust, Burn and Leptos for the whole system

**Status:** Accepted
**Date:** 2026-09-26

## Context

RL_Semaphore is a learning and portfolio project built to production standards (see [mission](../mission.md)). It needs a simulator, RL agents, a training pipeline, a server and a web UI. We had to pick the language and main libraries for each part.

Alternatives considered:

- **Python with PyTorch or Stable-Baselines3 for RL.** The most mature RL ecosystem, with tested DQN and PPO. But the simulator would then need to be fast in another language or be slow, the web parts would need a second language, and the project's goal is to learn the algorithms, not to call a library.
- **`tch-rs` or `candle` for ML in Rust.** `tch-rs` binds libtorch, which is a large native dependency that complicates CI and Docker. `candle` is lighter but has less tooling for training loops and backends than Burn.
- **A TypeScript frontend, or Yew/Dioxus for the UI.** TypeScript is the safest choice for a UI but means a second language and hand-written copies of the API types. Yew and Dioxus are also Rust-to-WASM, but Leptos has fine-grained reactivity and an active community, and we prefer it.

## Decision

Write everything in Rust: the simulator, the agents on [Burn](https://burn.dev) (`NdArray` on CPU by default, `Wgpu` optional), the Axum server, and a Leptos frontend compiled to WASM. Implement DQN and PPO in-house.

## Consequences

### Positive

- One language and one toolchain. The `protocol` crate shares type-checked DTOs between server and browser.
- A fast, deterministic simulator without FFI, which makes training throughput realistic.
- Writing DQN and PPO ourselves serves the learning goal.
- One build, one CI, and simple Docker images.

### Negative

- The RL ecosystem in Rust is much smaller. There is no ready-made gym, and few reference implementations to compare against.
- DQN and PPO are our own code, so bugs in them are ours to find. We need tests and baselines to trust the results.
- Burn is younger than PyTorch and its API may change between versions.
- The WASM build adds complexity: an extra target, `cargo-leptos`, larger bundles and slower builds.
- Fewer people can review or contribute to the project than with Python or TypeScript.
