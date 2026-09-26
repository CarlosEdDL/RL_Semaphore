# Mission

## Purpose

RL_Semaphore trains reinforcement-learning agents to control the traffic lights of a small simulated city, and lets you watch them learn.

It is a learning and portfolio project, **built to production standards**. Every part of it (simulator, learning algorithms, training pipeline, server, and web UI) should use the practices expected of a professional codebase: tests, typing, reproducibility, observability, CI/CD, containerization, and clear documentation.

## Audience

1. **The author.** It is a vehicle for learning RL, Rust, and systems design in depth.
2. **ML practitioners.** People who want to inspect, tweak, and compare agents, reward functions, and hyperparameters, and see the effect clearly.

Non-technical viewers are not the target audience. The UI should still be clear enough that a portfolio visitor understands what they are looking at.

## Objective: what "good" means

The primary objective is to **minimize average vehicle waiting time**, subject to a **fairness constraint**:

> No vehicle should "take the bullet for the team."

A policy can lower the average by letting a few vehicles (or a quiet side street) wait almost forever. That behavior is **unacceptable**, even if the average improves. RL_Semaphore treats it as a failure mode, not an optimization win.

This principle is applied at every layer:

| Layer | How fairness is enforced |
|-------|--------------------------|
| **Reward** | Combines average wait with a penalty that grows super-linearly with individual wait (for example, a squared or thresholded max-wait term). The average alone is never the reward. |
| **Safety layer** | A hard *max-red* limit: a phase that has been red longer than the configured limit is forced green through action masking. Min-green and yellow/all-red clearance times are also enforced. The agent cannot violate these limits. |
| **Metrics** | Every evaluation reports the mean, **p95, p99 and max** waiting time, plus per-approach wait. A policy "wins" only if it improves the mean **without** worsening the tail beyond an agreed tolerance. |
| **Visualization** | Starved vehicles and approaches are visibly highlighted, so tail behavior is easy to spot, not hidden behind averages. |

Secondary metrics tracked (not optimized directly): throughput, queue length, and number of stops.

## Scope

- **Control strategy:** start with **one intersection and one agent**, then scale to **one independent agent per intersection** on a grid, then explore **coordination** between neighboring agents.
- **World:** a custom, deterministic, seedable Rust traffic simulator for a synthetic **grid city from 3×3 to 5×5** intersections.
- **Baselines:** every RL result is compared against **fixed-time** and **simple actuated** controllers. An RL agent that does not beat these baselines on both mean and tail metrics is not a result.
- **Visualization:** a web app that shows the city animating live, training curves, and run comparisons.

## Non-goals

- Real-world deployment on physical traffic infrastructure.
- Photorealistic or 3D rendering.
- Importing real maps (OpenStreetMap) in the initial scope.
- Pedestrians, emergency-vehicle priority, and public transit in the initial scope. These may be revisited later.

## Guiding principles

1. **Correctness before cleverness.** The simulator is tested and deterministic before any agent trains on it.
2. **Reproducible by default.** Every run is fully determined by `config + seed + code version`, and all three are recorded.
3. **Measure the tail, not just the mean.** See the fairness constraint above.
4. **Small, reviewable increments.** Each roadmap phase is about one PR, and leaves `main` green and working.
5. **Visible progress.** Every phase should produce something you can run, see, or measure.
