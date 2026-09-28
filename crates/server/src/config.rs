//! [`ServerConfig`], the server's configuration, and [`Pace`], the sim thread's pacing.

use std::net::SocketAddr;
use std::num::{NonZeroU64, NonZeroUsize};

use rl_semaphore_sim::Scenario;

use crate::error::ServerError;

/// Number of ticks a slow client may fall behind the broadcast before it starts skipping them.
const DEFAULT_BROADCAST_CAPACITY: usize = 64;

/// How the sim thread paces its ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pace {
    /// One tick every `step_s / speed` seconds of wall-clock time.
    RealTime {
        /// Must be finite and greater than 0; validated by [`ServerConfig::validate`], not by
        /// this constructor.
        speed: f64,
    },
    /// No pacing: the sim thread runs ticks back to back, as fast as it can.
    Unthrottled,
}

impl Pace {
    /// Rejects a [`Pace::RealTime`] speed that is not finite or not greater than 0.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::InvalidSpeed`] if it is.
    pub(crate) fn validate(&self) -> Result<(), ServerError> {
        if let Self::RealTime { speed } = *self
            && (!speed.is_finite() || speed <= 0.0)
        {
            return Err(ServerError::InvalidSpeed { speed });
        }
        Ok(())
    }
}

/// The server's configuration: the scenario to run, the episode looping, the pacing, and where
/// to bind.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The scenario to run. Must have a `[fixed_time]` table (validated by
    /// [`ServerConfig::validate`]).
    pub scenario: Scenario,
    /// The demand seed of episode 0. Episode `e` uses `seed.wrapping_add(e)`.
    pub seed: u64,
    /// Steps per episode, after which the run starts a fresh episode.
    pub steps_per_episode: NonZeroU64,
    /// How often, in steps, a tick includes a [`rl_semaphore_protocol::Metrics`] message (also
    /// sent at step 0 and at the last step of every episode).
    pub metrics_every: NonZeroU64,
    /// How the sim thread paces its ticks.
    pub pace: Pace,
    /// The address to bind the HTTP/WebSocket listener to.
    pub bind: SocketAddr,
    /// Number of ticks a slow client may fall behind the broadcast channel before it starts
    /// skipping them. Defaults to 64 ([`ServerConfig::new`]).
    pub broadcast_capacity: NonZeroUsize,
}

impl ServerConfig {
    /// Builds a config for `scenario`, bound to `bind`, with `seed` 0, `steps_per_episode`
    /// 3,600, `metrics_every` 5, `pace` real-time at speed 1, and `broadcast_capacity` 64.
    #[must_use]
    pub const fn new(scenario: Scenario, bind: SocketAddr) -> Self {
        Self {
            scenario,
            seed: 0,
            steps_per_episode: nonzero_u64(3600),
            metrics_every: nonzero_u64(5),
            pace: Pace::RealTime { speed: 1.0 },
            bind,
            broadcast_capacity: nonzero_usize(DEFAULT_BROADCAST_CAPACITY),
        }
    }

    /// Rejects a scenario with no `[fixed_time]` table, or an invalid [`Pace::RealTime`] speed.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::MissingFixedTimePlan`] or [`ServerError::InvalidSpeed`].
    pub(crate) fn validate(&self) -> Result<(), ServerError> {
        if self.scenario.fixed_time().is_none() {
            return Err(ServerError::MissingFixedTimePlan);
        }
        self.pace.validate()
    }
}

/// A nonzero `u64` literal, checked at compile time.
const fn nonzero_u64(n: u64) -> NonZeroU64 {
    match NonZeroU64::new(n) {
        Some(n) => n,
        None => panic!("literal must be nonzero"),
    }
}

/// A nonzero `usize` literal, checked at compile time.
const fn nonzero_usize(n: usize) -> NonZeroUsize {
    match NonZeroUsize::new(n) {
        Some(n) => n,
        None => panic!("literal must be nonzero"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::net::SocketAddr;

    use rl_semaphore_sim::Scenario;

    use super::{Pace, ServerConfig};
    use crate::error::ServerError;

    const EXAMPLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../configs/single-intersection.toml"
    ));

    const NO_FIXED_TIME: &str = r#"
        step_s = 1.0
        [intersection]
        cell_length_m = 7.5
        [intersection.approaches.north]
        length_m = 30.0
        lanes = [{ movements = ["through"] }]
        [intersection.approaches.east]
        length_m = 30.0
        lanes = [{ movements = ["through"] }]
        [intersection.approaches.south]
        length_m = 30.0
        lanes = [{ movements = ["through"] }]
        [intersection.approaches.west]
        length_m = 30.0
        lanes = [{ movements = ["through"] }]
        [signal]
        yellow_s = 3.0
        all_red_s = 2.0
        min_green_s = 5.0
        max_red_s = 90.0
        [[signal.phases]]
        name = "ns"
        green = { north = ["through"], south = ["through"] }
        [[signal.phases]]
        name = "ew"
        green = { east = ["through"], west = ["through"] }
    "#;

    fn bind() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    #[test]
    fn valid_config_passes() {
        let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
        let config = ServerConfig::new(scenario, bind());
        config.validate().unwrap();
    }

    #[test]
    fn missing_fixed_time_plan_is_rejected() {
        let scenario = Scenario::from_toml_str(NO_FIXED_TIME).unwrap();
        let config = ServerConfig::new(scenario, bind());
        assert!(matches!(
            config.validate(),
            Err(ServerError::MissingFixedTimePlan)
        ));
    }

    #[test]
    fn invalid_speeds_are_rejected() {
        let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
        for speed in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut config = ServerConfig::new(scenario.clone(), bind());
            config.pace = Pace::RealTime { speed };
            assert!(
                matches!(config.validate(), Err(ServerError::InvalidSpeed { .. })),
                "speed={speed}"
            );
        }
    }

    #[test]
    fn positive_finite_speeds_are_accepted() {
        let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
        for speed in [0.001, 1.0, 1000.0] {
            let mut config = ServerConfig::new(scenario.clone(), bind());
            config.pace = Pace::RealTime { speed };
            config.validate().unwrap();
        }
    }

    #[test]
    fn unthrottled_pace_is_always_valid() {
        let scenario = Scenario::from_toml_str(EXAMPLE).unwrap();
        let mut config = ServerConfig::new(scenario, bind());
        config.pace = Pace::Unthrottled;
        config.validate().unwrap();
    }
}
