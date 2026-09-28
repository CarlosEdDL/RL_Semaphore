//! Property test: arbitrary `ServerMessage`s round-trip through JSON to an equal value (R8.2).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rl_semaphore_protocol::{
    ApproachLayout, ApproachLights, ByApproach, Direction, Hello, LaneLayout, LaneQueueNow,
    LaneQueueStats, LaneRef, LaneState, Layout, Light, Metrics, Movement, MovementRef, PhaseLayout,
    QueueStats, ServerMessage, SignalLayout, SignalStateView, SignalView, Snapshot, Summary,
    VehicleCounts, VehicleView, WaitStats,
};

/// A finite `f64` over the whole finite range.
fn finite_f64() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("finite", |f| f.is_finite())
}

fn direction() -> impl Strategy<Value = Direction> {
    prop_oneof![
        Just(Direction::North),
        Just(Direction::East),
        Just(Direction::South),
        Just(Direction::West),
    ]
}

fn movement() -> impl Strategy<Value = Movement> {
    prop_oneof![
        Just(Movement::Left),
        Just(Movement::Through),
        Just(Movement::Right),
    ]
}

fn light() -> impl Strategy<Value = Light> {
    prop_oneof![Just(Light::Red), Just(Light::Yellow), Just(Light::Green)]
}

fn lane_ref() -> impl Strategy<Value = LaneRef> {
    (direction(), 0u32..4).prop_map(|(approach, index)| LaneRef { approach, index })
}

fn movement_ref() -> impl Strategy<Value = MovementRef> {
    (direction(), movement()).prop_map(|(approach, movement)| MovementRef { approach, movement })
}

fn by_approach<T: core::fmt::Debug>(
    inner: impl Strategy<Value = T>,
) -> impl Strategy<Value = ByApproach<T>> {
    prop::collection::vec(inner, 4).prop_map(|mut v| {
        let west = v.pop().unwrap();
        let south = v.pop().unwrap();
        let east = v.pop().unwrap();
        let north = v.pop().unwrap();
        ByApproach {
            north,
            east,
            south,
            west,
        }
    })
}

fn lane_layout() -> impl Strategy<Value = LaneLayout> {
    (0u32..4, 2u32..50, prop::collection::vec(movement(), 0..=3)).prop_map(
        |(index, len_cells, movements)| LaneLayout {
            index,
            len_cells,
            movements,
        },
    )
}

fn approach_layout() -> impl Strategy<Value = ApproachLayout> {
    (finite_f64(), prop::collection::vec(lane_layout(), 0..=4))
        .prop_map(|(length_m, lanes)| ApproachLayout { length_m, lanes })
}

fn phase_layout() -> impl Strategy<Value = PhaseLayout> {
    (
        any::<String>(),
        prop::collection::vec(movement_ref(), 0..=8),
    )
        .prop_map(|(name, green)| PhaseLayout { name, green })
}

fn signal_layout() -> impl Strategy<Value = SignalLayout> {
    (
        finite_f64(),
        finite_f64(),
        finite_f64(),
        finite_f64(),
        prop::collection::vec(phase_layout(), 0..=4),
    )
        .prop_map(
            |(yellow_s, all_red_s, min_green_s, max_red_s, phases)| SignalLayout {
                yellow_s,
                all_red_s,
                min_green_s,
                max_red_s,
                phases,
            },
        )
}

fn layout() -> impl Strategy<Value = Layout> {
    (
        finite_f64(),
        finite_f64(),
        by_approach(approach_layout()),
        signal_layout(),
    )
        .prop_map(|(step_s, cell_length_m, approaches, signal)| Layout {
            step_s,
            cell_length_m,
            approaches,
            signal,
        })
}

fn hello() -> impl Strategy<Value = Hello> {
    (any::<u32>(), layout()).prop_map(|(protocol_version, layout)| Hello {
        protocol_version,
        layout,
    })
}

fn signal_state_view() -> impl Strategy<Value = SignalStateView> {
    prop_oneof![
        (0u32..8, finite_f64())
            .prop_map(|(phase, elapsed_s)| SignalStateView::Green { phase, elapsed_s }),
        (0u32..8, 0u32..8, finite_f64()).prop_map(|(from, to, elapsed_s)| {
            SignalStateView::Yellow {
                from,
                to,
                elapsed_s,
            }
        }),
        (0u32..8, 0u32..8, finite_f64()).prop_map(|(from, to, elapsed_s)| {
            SignalStateView::AllRed {
                from,
                to,
                elapsed_s,
            }
        }),
    ]
}

fn approach_lights() -> impl Strategy<Value = ApproachLights> {
    (light(), light(), light()).prop_map(|(left, through, right)| ApproachLights {
        left,
        through,
        right,
    })
}

fn signal_view() -> impl Strategy<Value = SignalView> {
    (signal_state_view(), by_approach(approach_lights()))
        .prop_map(|(state, lights)| SignalView { state, lights })
}

fn vehicle_counts() -> impl Strategy<Value = VehicleCounts> {
    (any::<u64>(), any::<u64>(), any::<u64>(), any::<u64>()).prop_map(
        |(spawned, backlog, on_lane, departed)| VehicleCounts {
            spawned,
            backlog,
            on_lane,
            departed,
        },
    )
}

fn lane_state() -> impl Strategy<Value = LaneState> {
    (lane_ref(), any::<u64>()).prop_map(|(lane, backlog)| LaneState { lane, backlog })
}

fn vehicle_view() -> impl Strategy<Value = VehicleView> {
    (
        any::<u64>(),
        lane_ref(),
        any::<u32>(),
        movement(),
        finite_f64(),
        any::<bool>(),
    )
        .prop_map(|(id, lane, cell, movement, wait_s, stopped)| VehicleView {
            id,
            lane,
            cell,
            movement,
            wait_s,
            stopped,
        })
}

fn snapshot() -> impl Strategy<Value = Snapshot> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        finite_f64(),
        signal_view(),
        vehicle_counts(),
        prop::collection::vec(lane_state(), 0..=6),
        prop::collection::vec(vehicle_view(), 0..=6),
    )
        .prop_map(
            |(step, episode, seed, time_s, signal, counts, lanes, vehicles)| Snapshot {
                step,
                episode,
                seed,
                time_s,
                signal,
                counts,
                lanes,
                vehicles,
            },
        )
}

fn wait_stats() -> impl Strategy<Value = WaitStats> {
    (
        any::<u64>(),
        finite_f64(),
        finite_f64(),
        finite_f64(),
        finite_f64(),
        finite_f64(),
    )
        .prop_map(|(count, mean_s, p50_s, p95_s, p99_s, max_s)| WaitStats {
            count,
            mean_s,
            p50_s,
            p95_s,
            p99_s,
            max_s,
        })
}

fn opt_wait_stats() -> impl Strategy<Value = Option<WaitStats>> {
    prop_oneof![1 => Just(None), 3 => wait_stats().prop_map(Some)]
}

fn queue_stats() -> impl Strategy<Value = QueueStats> {
    (finite_f64(), any::<u64>()).prop_map(|(mean, max)| QueueStats { mean, max })
}

fn lane_queue_stats() -> impl Strategy<Value = LaneQueueStats> {
    (lane_ref(), finite_f64(), any::<u64>()).prop_map(|(lane, mean, max)| LaneQueueStats {
        lane,
        mean,
        max,
    })
}

fn lane_queue_now() -> impl Strategy<Value = LaneQueueNow> {
    (lane_ref(), any::<u64>()).prop_map(|(lane, stopped)| LaneQueueNow { lane, stopped })
}

fn summary() -> impl Strategy<Value = Summary> {
    (
        any::<u64>(),
        finite_f64(),
        any::<u64>(),
        any::<u64>(),
        finite_f64(),
        opt_wait_stats(),
        by_approach(opt_wait_stats()),
        prop::collection::vec(lane_queue_stats(), 0..=6),
        by_approach(queue_stats()),
        queue_stats(),
    )
        .prop_map(
            |(
                steps,
                duration_s,
                departed,
                in_system,
                throughput_veh_per_h,
                wait,
                wait_by_approach,
                queue_by_lane,
                queue_by_approach,
                queue_total,
            )| Summary {
                steps,
                duration_s,
                departed,
                in_system,
                throughput_veh_per_h,
                wait,
                wait_by_approach,
                queue_by_lane,
                queue_by_approach,
                queue_total,
            },
        )
}

fn metrics() -> impl Strategy<Value = Metrics> {
    (
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        finite_f64(),
        summary(),
        prop::collection::vec(lane_queue_now(), 0..=6),
    )
        .prop_map(
            |(step, episode, seed, time_s, summary, queue_now)| Metrics {
                step,
                episode,
                seed,
                time_s,
                summary,
                queue_now,
            },
        )
}

fn server_message() -> impl Strategy<Value = ServerMessage> {
    prop_oneof![
        hello().prop_map(ServerMessage::Hello),
        snapshot().prop_map(ServerMessage::Snapshot),
        metrics().prop_map(ServerMessage::Metrics),
    ]
}

proptest! {
    #[test]
    fn every_message_round_trips(msg in server_message()) {
        let text = msg.to_json().unwrap();
        let decoded = ServerMessage::from_json(&text).unwrap();
        prop_assert_eq!(decoded, msg);
    }
}
