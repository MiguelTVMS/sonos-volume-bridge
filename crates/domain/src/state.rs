use crate::{MuteState, NormalizedVolume, SonosVolume};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfirmedSonosState {
    pub volume: SonosVolume,
    pub muted: MuteState,
    pub revision: u64,
    pub observed_at_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalAudioState {
    pub volume: NormalizedVolume,
    pub muted: MuteState,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingIntent {
    SetVolume {
        desired: SonosVolume,
        created_at_ms: u64,
    },
    SetMute {
        desired: MuteState,
        created_at_ms: u64,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncState {
    ConfigurationRequired,
    Connecting,
    Synchronized,
    WaitingForSonosConfirmation,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedLocalWrite {
    pub state: LocalAudioState,
    pub expires_at_ms: u64,
    pub generation: u64,
    pub tolerance: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuppressionDecision {
    Suppress,
    Forward,
}

impl ExpectedLocalWrite {
    pub fn classify(self, observed: LocalAudioState, now_ms: u64) -> SuppressionDecision {
        if now_ms > self.expires_at_ms || observed.muted != self.state.muted {
            return SuppressionDecision::Forward;
        }
        let delta = observed.volume.get().abs_diff(self.state.volume.get());
        if delta <= self.tolerance {
            SuppressionDecision::Suppress
        } else {
            SuppressionDecision::Forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn local(volume: u8, muted: bool) -> LocalAudioState {
        LocalAudioState {
            volume: NormalizedVolume::new(volume).unwrap(),
            muted: MuteState(muted),
        }
    }
    #[test]
    fn expected_write_honours_tolerance_and_expiry() {
        let expected = ExpectedLocalWrite {
            state: local(50, false),
            expires_at_ms: 100,
            generation: 1,
            tolerance: 1,
        };
        assert_eq!(
            expected.classify(local(51, false), 99),
            SuppressionDecision::Suppress
        );
        assert_eq!(
            expected.classify(local(52, false), 99),
            SuppressionDecision::Forward
        );
        assert_eq!(
            expected.classify(local(50, false), 101),
            SuppressionDecision::Forward
        );
    }
}
