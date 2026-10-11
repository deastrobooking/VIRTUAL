//! Estimates the leader's clock on a follower from ping/pong round trips.
//!
//! Each round trip gives an offset estimate with an uncertainty of half its
//! round-trip time. Wi-Fi queuing only ever adds delay, so the sample with the
//! smallest round trip among recent ones is the most accurate (the classic
//! NTP clock filter).

use std::collections::VecDeque;

const SAMPLES: usize = 16;

#[derive(Clone, Debug, Default)]
pub struct ClockOffset {
    samples: VecDeque<(u64, i64)>,
    /// Fallback from one-way clock packets before any round trip completes.
    rough: Option<i64>,
}

impl ClockOffset {
    /// Adds a round trip: ping sent at `sent_us` and its pong received at
    /// `received_us` (follower clock), stamped `leader_us` by the leader.
    pub fn add_round_trip(&mut self, sent_us: u64, leader_us: u64, received_us: u64) {
        if received_us < sent_us {
            return;
        }
        let rtt = received_us - sent_us;
        let offset = leader_us as i64 + (rtt / 2) as i64 - received_us as i64;
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back((rtt, offset));
    }

    /// Notes a one-way clock packet, assuming zero network delay.
    pub fn add_one_way(&mut self, leader_us: u64, received_us: u64) {
        self.rough = Some(leader_us as i64 - received_us as i64);
    }

    /// Leader time minus follower time, in microseconds.
    pub fn offset_us(&self) -> Option<i64> {
        self.samples
            .iter()
            .min_by_key(|(rtt, _)| *rtt)
            .map(|(_, offset)| *offset)
            .or(self.rough)
    }

    /// Smallest recent round-trip time: the estimate's uncertainty bound.
    pub fn best_rtt_us(&self) -> Option<u64> {
        self.samples.iter().map(|(rtt, _)| *rtt).min()
    }
}

/// The last clock message from a leader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeaderClock {
    pub leader_us: u64,
    pub beat: f64,
    pub bpm: f32,
    pub beats_per_bar: u8,
    pub playing: bool,
}

impl LeaderClock {
    /// Musical position at follower time `local_us`.
    pub fn beat_at(&self, local_us: u64, offset_us: i64) -> f64 {
        let leader_now = local_us as i64 + offset_us;
        self.beat + (leader_now - self.leader_us as i64) as f64 / 60.0e6 * f64::from(self.bpm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fastest_round_trip_wins() {
        let mut clock = ClockOffset::default();
        assert_eq!(clock.offset_us(), None);
        clock.add_one_way(1_000_000, 600);
        assert_eq!(clock.offset_us(), Some(999_400), "rough until a round trip");
        // True offset 1_000_000. A 40 ms queued round trip, then a clean 2 ms one.
        // The queued sample's stamp is skewed by asymmetric delay.
        clock.add_round_trip(0, 1_035_000, 40_000);
        // Clean sample: sent at 100 ms, back at 102 ms, stamped at the midpoint.
        clock.add_round_trip(100_000, 1_101_000, 102_000);
        assert_eq!(clock.offset_us(), Some(1_000_000));
        assert_eq!(clock.best_rtt_us(), Some(2_000));
    }

    #[test]
    fn beats_advance_with_tempo_from_the_leader_stamp() {
        let clock = LeaderClock {
            leader_us: 10_000_000,
            beat: 32.0,
            bpm: 120.0,
            beats_per_bar: 4,
            playing: true,
        };
        // Follower clock runs 1 s behind; half a second later is one beat on.
        assert!((clock.beat_at(9_500_000, 1_000_000) - 33.0).abs() < 1e-9);
    }
}
