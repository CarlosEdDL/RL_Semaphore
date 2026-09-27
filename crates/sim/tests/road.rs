//! Tests for the road model.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rl_semaphore_sim::{ConfigError, Direction, Intersection, IntersectionConfig, Movement};

const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// A valid config with a replaceable north approach.
fn config(north_lanes: &str, north_len: &str, cell: &str) -> String {
    format!(
        r#"
cell_length_m = {cell}
[approaches.north]
length_m = {north_len}
lanes = {north_lanes}
[approaches.east]
length_m = 100.0
lanes = [ {{ movements = ["through"] }} ]
[approaches.south]
length_m = 100.0
lanes = [ {{ movements = ["through"] }} ]
[approaches.west]
length_m = 100.0
lanes = [ {{ movements = ["through"] }} ]
"#
    )
}

fn err(toml: &str) -> ConfigError {
    Intersection::from_toml_str(toml).expect_err("config should be rejected")
}

#[test]
fn example_config_loads() {
    let x = Intersection::from_toml_str(EXAMPLE).unwrap();
    assert_eq!(x.approaches().count(), 4);
    let lanes = |d| x.approach(d).lanes();
    assert_eq!(lanes(Direction::North).len(), 2);
    assert_eq!(lanes(Direction::South).len(), 2);
    assert_eq!(lanes(Direction::East).len(), 1);
    assert_eq!(lanes(Direction::West).len(), 1);
    assert_eq!(lanes(Direction::North)[0].movements().len(), 1);
    assert!(
        lanes(Direction::North)[0]
            .movements()
            .contains(&Movement::Left)
    );
    assert_eq!(lanes(Direction::East)[0].movements().len(), 3);
    assert_eq!(lanes(Direction::North)[1].len_cells(), 20);
    assert_eq!(lanes(Direction::East)[0].len_cells(), 13);
    let order: Vec<_> = x.approaches().map(|a| a.direction()).collect();
    assert_eq!(order, Direction::ALL);
}

#[test]
fn stop_line_is_last_cell() {
    let x = Intersection::from_toml_str(EXAMPLE).unwrap();
    let lane = &x.approach(Direction::North).lanes()[0];
    assert_eq!(lane.stop_line_cell(), lane.len_cells() - 1);
    assert_eq!(lane.id().approach(), Direction::North);
    assert_eq!(lane.id().index(), 0);
}

#[test]
fn cell_count_drops_remainder() {
    let x =
        Intersection::from_toml_str(&config(r#"[ { movements = ["through"] } ]"#, "20.0", "7.5"))
            .unwrap();
    assert_eq!(x.approach(Direction::North).lanes()[0].len_cells(), 2);
}

#[test]
fn destination_all_pairs() {
    use Direction::{East, North, South, West};
    use Movement::{Left, Right, Through};
    let table = [
        (North, Left, East),
        (North, Through, South),
        (North, Right, West),
        (East, Left, South),
        (East, Through, West),
        (East, Right, North),
        (South, Left, West),
        (South, Through, North),
        (South, Right, East),
        (West, Left, North),
        (West, Through, East),
        (West, Right, South),
    ];
    for (from, m, to) in table {
        assert_eq!(from.destination(m), to, "{from} {m}");
    }
}

#[test]
fn round_trip() {
    let x = Intersection::from_toml_str(EXAMPLE).unwrap();
    let text = x.to_config().to_toml_string().unwrap();
    let again = Intersection::from_toml_str(&text).unwrap();
    assert_eq!(x, again);
    assert_eq!(
        x.to_config(),
        IntersectionConfig::from_toml_str(&text).unwrap()
    );
}

#[test]
fn rejects_bad_cell_length() {
    for bad in ["0.0", "-1.0", "nan", "inf"] {
        let e = err(&config(r#"[ { movements = ["through"] } ]"#, "100.0", bad));
        assert!(
            matches!(e, ConfigError::InvalidCellLength { .. }),
            "{bad}: {e}"
        );
        assert!(e.to_string().contains("cell_length_m"));
    }
}

#[test]
fn rejects_bad_length() {
    for bad in ["0.0", "-5.0", "nan", "inf"] {
        let e = err(&config(r#"[ { movements = ["through"] } ]"#, bad, "7.5"));
        assert!(
            matches!(&e, ConfigError::InvalidLength { path, .. } if path == "approaches.north.length_m"),
            "{bad}: {e}"
        );
    }
}

#[test]
fn rejects_too_short_or_too_many_cells() {
    let lane = r#"[ { movements = ["through"] } ]"#;
    let e = err(&config(lane, "14.0", "7.5"));
    assert!(
        matches!(&e, ConfigError::BadCellCount { path, cells } if path == "approaches.north.length_m" && *cells == 1.0),
        "{e}"
    );
    let e = err(&config(lane, "1e300", "1e-300"));
    assert!(matches!(e, ConfigError::BadCellCount { .. }), "{e}");
}

#[test]
fn rejects_bad_lane_count() {
    let e = err(&config("[]", "100.0", "7.5"));
    assert!(
        matches!(&e, ConfigError::LaneCount { path, count: 0 } if path == "approaches.north.lanes"),
        "{e}"
    );
    let five = r#"[ {movements=["through"]}, {movements=["through"]}, {movements=["through"]}, {movements=["through"]}, {movements=["through"]} ]"#;
    let e = err(&config(five, "100.0", "7.5"));
    assert!(matches!(e, ConfigError::LaneCount { count: 5, .. }), "{e}");
}

#[test]
fn rejects_empty_and_duplicate_movements() {
    let e = err(&config(
        r#"[ {movements=["through"]}, {movements=[]} ]"#,
        "100.0",
        "7.5",
    ));
    assert!(
        matches!(&e, ConfigError::EmptyMovements { path } if path == "approaches.north.lanes[1].movements"),
        "{e}"
    );
    assert!(e.to_string().contains("approaches.north.lanes[1]"));

    let e = err(&config(
        r#"[ {movements=["left", "left"]} ]"#,
        "100.0",
        "7.5",
    ));
    assert!(
        matches!(&e, ConfigError::DuplicateMovement { path, movement: Movement::Left } if path == "approaches.north.lanes[0].movements"),
        "{e}"
    );
}

#[test]
fn rejects_crossing_movements_but_allows_shared() {
    let e = err(&config(
        r#"[ {movements=["right"]}, {movements=["through"]} ]"#,
        "100.0",
        "7.5",
    ));
    assert!(
        matches!(&e, ConfigError::CrossingMovements { path, left_lane: 0, .. } if path == "approaches.north.lanes[1].movements"),
        "{e}"
    );
    Intersection::from_toml_str(&config(
        r#"[ {movements=["left", "through"]}, {movements=["through", "right"]} ]"#,
        "100.0",
        "7.5",
    ))
    .unwrap();
}

#[test]
fn first_error_follows_fixed_order() {
    // Both the cell length and the north lane count are wrong: the cell length wins.
    let e = err(&config("[]", "100.0", "0.0"));
    assert!(matches!(e, ConfigError::InvalidCellLength { .. }));
}

#[test]
fn parse_errors() {
    // Missing approach.
    let missing_approach = r#"
cell_length_m = 7.5
[approaches.north]
length_m = 100.0
lanes = [ { movements = ["through"] } ]
"#;
    assert!(matches!(err(missing_approach), ConfigError::Parse(_)));

    // Missing field.
    let missing_field = config(r#"[ { movements = ["through"] } ]"#, "100.0", "7.5")
        .replace("cell_length_m = 7.5", "");
    assert!(matches!(err(&missing_field), ConfigError::Parse(_)));

    // Unknown field.
    let unknown = format!(
        "{}\nextra = 1\n",
        EXAMPLE.replace("[approaches.north]", "[approaches.north]\nfoo = 1")
    );
    assert!(matches!(err(&unknown), ConfigError::Parse(_)));
    let unknown_top = format!("extra = 1\n{EXAMPLE}");
    assert!(matches!(err(&unknown_top), ConfigError::Parse(_)));
}
