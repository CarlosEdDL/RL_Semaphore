//! Formats the result of a `simulate` run as a text table or as one JSON document.

use std::fmt::Write;

use rl_semaphore_env::EpisodeReport;
use rl_semaphore_sim::{Direction, FixedTimePlan, QueueStats, Scenario, WaitStats};
use serde::Serialize;

/// Version of the JSON document. Bump it when a field changes meaning or goes away.
const SCHEMA_VERSION: u32 = 1;

/// Everything a printed result says about the run that produced it.
#[derive(Debug)]
pub struct RunInfo<'a> {
    /// The config path, as the user gave it.
    pub config: &'a str,
    pub seed: u64,
    pub steps: u64,
    pub controller: &'a str,
    pub scenario: &'a Scenario,
    pub plan: &'a FixedTimePlan,
}

/// `(phase name, green seconds)` per phase, and the cycle in seconds.
fn timing<'a>(info: &RunInfo<'a>) -> (Vec<(&'a str, f64)>, f64) {
    let step_s = info.scenario.step_s();
    let phases = info
        .scenario
        .signal_plan()
        .phases()
        .iter()
        .zip(info.plan.green_steps())
        .map(|(phase, &g)| (phase.name(), f64::from(g) * step_s))
        .collect();
    (phases, f64::from(info.plan.cycle_steps()) * step_s)
}

/// The fixed-layout text table.
pub fn text(info: &RunInfo<'_>, report: &EpisodeReport) -> String {
    let s = &report.summary;
    let (phases, cycle_s) = timing(info);
    let mut out = String::new();
    // Writing to a `String` cannot fail, so the results are ignored.
    let _ = writeln!(out, "config:      {}", info.config);
    let _ = writeln!(out, "seed:        {}", info.seed);
    let _ = writeln!(out, "steps:       {}", info.steps);
    let _ = writeln!(out, "duration_s:  {:.1}", s.duration_s);
    let _ = writeln!(out, "controller:  {}", info.controller);
    for (name, green_s) in &phases {
        let _ = writeln!(out, "  green {name:<12} {green_s:>7.1} s");
    }
    let _ = writeln!(out, "  cycle {:<12} {cycle_s:>7.1} s", "");
    let _ = writeln!(out, "departed:    {}", s.departed);
    let _ = writeln!(out, "in_system:   {}", s.in_system);
    let _ = writeln!(out, "throughput:  {:.1} veh/h", s.throughput_veh_per_h);

    let _ = writeln!(out, "\nwait (s)");
    let _ = writeln!(
        out,
        "{:<10} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "", "count", "mean", "p50", "p95", "p99", "max"
    );
    wait_row(&mut out, "all", s.wait.as_ref());
    for d in Direction::ALL {
        wait_row(&mut out, d.name(), s.wait_by_approach[d.index()].as_ref());
    }

    let _ = writeln!(out, "\nqueue (vehicles)");
    let _ = writeln!(out, "{:<10} {:>8} {:>8}", "", "mean", "max");
    for (id, q) in &s.queue_by_lane {
        queue_row(&mut out, &format!("{} {}", id.approach(), id.index()), q);
    }
    for d in Direction::ALL {
        queue_row(&mut out, d.name(), &s.queue_by_approach[d.index()]);
    }
    queue_row(&mut out, "total", &s.queue_total);

    let _ = writeln!(out, "\nsignal");
    let _ = writeln!(out, "switches_started:  {}", report.signal.switches_started);
    let _ = writeln!(out, "switches_forced:   {}", report.signal.switches_forced);
    let _ = writeln!(out, "commands_ignored:  {}", report.signal.commands_ignored);
    out
}

fn wait_row(out: &mut String, name: &str, stats: Option<&WaitStats>) {
    let _ = match stats {
        None => writeln!(
            out,
            "{name:<10} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "-", "-", "-", "-", "-", "-"
        ),
        Some(w) => writeln!(
            out,
            "{name:<10} {:>7} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            w.count, w.mean_s, w.p50_s, w.p95_s, w.p99_s, w.max_s
        ),
    };
}

fn queue_row(out: &mut String, name: &str, q: &QueueStats) {
    let _ = writeln!(out, "{name:<10} {:>8.2} {:>8}", q.mean, q.max);
}

#[derive(Serialize)]
struct Document<'a> {
    schema_version: u32,
    config: &'a str,
    seed: u64,
    steps: u64,
    step_s: f64,
    controller: Controller<'a>,
    summary: Summary,
    signal: Signal,
}

#[derive(Serialize)]
struct Controller<'a> {
    kind: &'a str,
    phases: Vec<PhaseGreen<'a>>,
    cycle_s: f64,
}

#[derive(Serialize)]
struct PhaseGreen<'a> {
    name: &'a str,
    green_s: f64,
}

#[derive(Serialize)]
struct Summary {
    steps: u64,
    duration_s: f64,
    departed: u64,
    in_system: u64,
    throughput_veh_per_h: f64,
    wait: Option<Wait>,
    wait_by_approach: Approaches<Option<Wait>>,
    queue_by_lane: Vec<LaneQueue>,
    queue_by_approach: Approaches<Queue>,
    queue_total: Queue,
}

#[derive(Serialize)]
struct Wait {
    count: u64,
    mean_s: f64,
    p50_s: f64,
    p95_s: f64,
    p99_s: f64,
    max_s: f64,
}

#[derive(Serialize)]
struct Queue {
    mean: f64,
    max: u64,
}

#[derive(Serialize)]
struct LaneQueue {
    approach: &'static str,
    lane: usize,
    mean: f64,
    max: u64,
}

/// A value per approach, keyed by approach name.
#[derive(Serialize)]
struct Approaches<T> {
    north: T,
    east: T,
    south: T,
    west: T,
}

#[derive(Serialize)]
struct Signal {
    switches_started: u64,
    switches_forced: u64,
    commands_ignored: u64,
}

impl From<&WaitStats> for Wait {
    fn from(w: &WaitStats) -> Self {
        Self {
            count: w.count,
            mean_s: w.mean_s,
            p50_s: w.p50_s,
            p95_s: w.p95_s,
            p99_s: w.p99_s,
            max_s: w.max_s,
        }
    }
}

impl From<&QueueStats> for Queue {
    fn from(q: &QueueStats) -> Self {
        Self {
            mean: q.mean,
            max: q.max,
        }
    }
}

/// The JSON document, followed by a newline.
pub fn json(info: &RunInfo<'_>, report: &EpisodeReport) -> Result<String, serde_json::Error> {
    let s = &report.summary;
    let (phases, cycle_s) = timing(info);
    let wait = |d: Direction| s.wait_by_approach[d.index()].as_ref().map(Wait::from);
    let queue = |d: Direction| Queue::from(&s.queue_by_approach[d.index()]);
    let document = Document {
        schema_version: SCHEMA_VERSION,
        config: info.config,
        seed: info.seed,
        steps: info.steps,
        step_s: info.scenario.step_s(),
        controller: Controller {
            kind: info.controller,
            phases: phases
                .into_iter()
                .map(|(name, green_s)| PhaseGreen { name, green_s })
                .collect(),
            cycle_s,
        },
        summary: Summary {
            steps: s.steps,
            duration_s: s.duration_s,
            departed: s.departed,
            in_system: s.in_system,
            throughput_veh_per_h: s.throughput_veh_per_h,
            wait: s.wait.as_ref().map(Wait::from),
            wait_by_approach: Approaches {
                north: wait(Direction::North),
                east: wait(Direction::East),
                south: wait(Direction::South),
                west: wait(Direction::West),
            },
            queue_by_lane: s
                .queue_by_lane
                .iter()
                .map(|(id, q)| LaneQueue {
                    approach: id.approach().name(),
                    lane: id.index(),
                    mean: q.mean,
                    max: q.max,
                })
                .collect(),
            queue_by_approach: Approaches {
                north: queue(Direction::North),
                east: queue(Direction::East),
                south: queue(Direction::South),
                west: queue(Direction::West),
            },
            queue_total: Queue::from(&s.queue_total),
        },
        signal: Signal {
            switches_started: report.signal.switches_started,
            switches_forced: report.signal.switches_forced,
            commands_ignored: report.signal.commands_ignored,
        },
    };
    let mut text = serde_json::to_string_pretty(&document)?;
    text.push('\n');
    Ok(text)
}
