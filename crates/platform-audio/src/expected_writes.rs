//! Value-based echo tracking for adapters without native callback context IDs.
use speaker_volume_bridge_domain::LocalAudioState;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const LIFETIME: Duration = Duration::from_millis(500);
const CAPACITY: usize = 64;

#[derive(Default)]
pub(crate) struct ExpectedWrites {
    writes: VecDeque<(LocalAudioState, Instant)>,
}

impl ExpectedWrites {
    pub(crate) fn record(&mut self, state: LocalAudioState, now: Instant) {
        self.prune(now);
        if self.writes.len() == CAPACITY {
            self.writes.pop_front();
        }
        self.writes.push_back((state, now + LIFETIME));
    }

    pub(crate) fn matches(&mut self, state: LocalAudioState, tolerance: u8, now: Instant) -> bool {
        self.prune(now);
        // Matching and unrelated callbacks must not consume later echoes.
        self.writes.iter().any(|(expected, _)| {
            expected.muted == state.muted
                && expected.volume.get().abs_diff(state.volume.get()) <= tolerance
        })
    }

    fn prune(&mut self, now: Instant) {
        self.writes.retain(|(_, expires)| now <= *expires);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaker_volume_bridge_domain::{MuteState, NormalizedVolume};
    fn state(volume: u8, muted: bool) -> LocalAudioState {
        LocalAudioState {
            volume: NormalizedVolume::new(volume).unwrap(),
            muted: MuteState(muted),
        }
    }
    #[test]
    fn repeated_and_overlapping_volume_and_mute_echoes_remain_suppressed() {
        let now = Instant::now();
        let mut writes = ExpectedWrites::default();
        for value in [state(20, false), state(40, false), state(40, true)] {
            writes.record(value, now);
        }
        for _ in 0..3 {
            for value in [state(40, true), state(20, false), state(40, false)] {
                assert!(writes.matches(value, 0, now + Duration::from_millis(250)));
            }
        }
    }
    #[test]
    fn unrelated_changes_do_not_consume_expectations_and_expiry_is_not_extended() {
        let now = Instant::now();
        let mut writes = ExpectedWrites::default();
        writes.record(state(40, false), now);
        assert!(!writes.matches(state(42, false), 1, now));
        assert!(!writes.matches(state(40, true), 1, now));
        assert!(writes.matches(state(41, false), 1, now + LIFETIME));
        assert!(!writes.matches(
            state(40, false),
            1,
            now + LIFETIME + Duration::from_millis(1)
        ));
    }
    #[test]
    fn each_write_expires_independently_and_history_is_bounded() {
        let now = Instant::now();
        let mut writes = ExpectedWrites::default();
        writes.record(state(10, false), now);
        writes.record(state(20, false), now + Duration::from_millis(250));
        let later = now + Duration::from_millis(501);
        assert!(!writes.matches(state(10, false), 0, later));
        assert!(writes.matches(state(20, false), 0, later));
        for _ in 0..1000 {
            writes.record(state(30, false), later);
        }
        assert_eq!(writes.writes.len(), CAPACITY);
    }
}
