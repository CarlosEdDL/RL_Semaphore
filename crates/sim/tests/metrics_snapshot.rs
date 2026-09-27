//! The summary of one full run of the example scenario is pinned by a snapshot.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fmt::Write;

use common::{EXAMPLE, run_with_metrics};
use rl_semaphore_sim::{
    Command, Demand, Direction, EpisodeMetrics, Scenario, Simulation, WaitStats,
};

fn wait_line(text: &mut String, name: &str, stats: Option<&WaitStats>) {
    match stats {
        None => writeln!(text, "wait {name}: none").unwrap(),
        Some(w) => writeln!(
            text,
            "wait {name}: count={} mean={:.3} p50={:.3} p95={:.3} p99={:.3} max={:.3}",
            w.count, w.mean_s, w.p50_s, w.p95_s, w.p99_s, w.max_s
        )
        .unwrap(),
    }
}

#[test]
fn example_summary_is_pinned() {
    let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut demand = Demand::new(&scenario, 2024);
    let mut sim = Simulation::new(scenario);
    let mut metrics = EpisodeMetrics::new(&sim);
    run_with_metrics(&mut sim, &mut demand, &mut metrics, 600, |_| Command::Hold);
    let s = metrics.summary(&sim);

    let mut text = String::new();
    writeln!(
        text,
        "steps={} duration_s={:.3} departed={} in_system={} throughput_veh_per_h={:.3}",
        s.steps, s.duration_s, s.departed, s.in_system, s.throughput_veh_per_h
    )
    .unwrap();
    wait_line(&mut text, "all", s.wait.as_ref());
    for d in Direction::ALL {
        wait_line(&mut text, d.name(), s.wait_by_approach[d.index()].as_ref());
    }
    for (id, q) in &s.queue_by_lane {
        writeln!(
            text,
            "queue {} lane {}: mean={:.3} max={}",
            id.approach(),
            id.index(),
            q.mean,
            q.max
        )
        .unwrap();
    }
    for d in Direction::ALL {
        let q = &s.queue_by_approach[d.index()];
        writeln!(text, "queue {d}: mean={:.3} max={}", q.mean, q.max).unwrap();
    }
    writeln!(
        text,
        "queue total: mean={:.3} max={}",
        s.queue_total.mean, s.queue_total.max
    )
    .unwrap();
    insta::assert_snapshot!(text);
}
