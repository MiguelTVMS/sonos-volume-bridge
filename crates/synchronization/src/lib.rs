//! Adapter-agnostic synchronization state machine.

use sonos_volume_bridge_domain::{
    ConfirmedSonosState, LocalAudioState, LocalOrigin, MappingError, MuteState, NormalizedVolume,
    PendingIntent, SonosVolume, SuppressionDecision, SyncState, VolumeMapping,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SonosObservationSource { Event, ExplicitRead, Poll }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncEvent {
    LocalChanged { state: LocalAudioState, origin: LocalOrigin, at_ms: u64 },
    SonosConfirmed { volume: SonosVolume, muted: MuteState, source: SonosObservationSource, at_ms: u64 },
    ConnectionLost,
    ConnectionRestored,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    RequestSonosVolume(SonosVolume),
    RequestSonosMute(MuteState),
    ApplyLocal(LocalAudioState),
    SuppressedLocalCallback,
}

#[derive(Clone, Debug)]
pub struct Synchronizer {
    mapping: VolumeMapping,
    maximum: SonosVolume,
    synchronize_mute: bool,
    confirmed: Option<ConfirmedSonosState>,
    pending_volume: Option<PendingIntent>,
    pending_mute: Option<PendingIntent>,
    state: SyncState,
}

impl Synchronizer {
    pub fn new(mapping: VolumeMapping, maximum: SonosVolume, synchronize_mute: bool) -> Result<Self, MappingError> {
        mapping.validate()?;
        Ok(Self { mapping, maximum, synchronize_mute, confirmed: None, pending_volume: None, pending_mute: None, state: SyncState::Connecting })
    }
    pub const fn state(&self) -> SyncState { self.state }
    pub const fn confirmed(&self) -> Option<ConfirmedSonosState> { self.confirmed }
    pub const fn pending_volume(&self) -> Option<PendingIntent> { self.pending_volume }

    pub fn handle(&mut self, event: SyncEvent) -> Result<Vec<Effect>, MappingError> {
        match event {
            SyncEvent::ConnectionLost => { self.state = SyncState::Degraded; Ok(vec![]) }
            SyncEvent::ConnectionRestored => { self.state = SyncState::Connecting; Ok(vec![]) }
            SyncEvent::LocalChanged { state, origin: LocalOrigin::Application, .. } => Ok(vec![Effect::SuppressedLocalCallback]),
            SyncEvent::LocalChanged { state, origin, at_ms } => self.handle_local(state, origin, at_ms),
            SyncEvent::SonosConfirmed { volume, muted, at_ms, .. } => self.handle_confirmed(volume, muted, at_ms),
        }
    }

    fn handle_local(&mut self, local: LocalAudioState, _origin: LocalOrigin, at_ms: u64) -> Result<Vec<Effect>, MappingError> {
        let desired = self.mapping.to_sonos(local.volume, self.maximum)?;
        self.pending_volume = Some(PendingIntent::SetVolume { desired, created_at_ms: at_ms });
        self.state = SyncState::WaitingForSonosConfirmation;
        let mut effects = vec![Effect::RequestSonosVolume(desired)];
        if self.synchronize_mute { self.pending_mute = Some(PendingIntent::SetMute { desired: local.muted, created_at_ms: at_ms }); effects.push(Effect::RequestSonosMute(local.muted)); }
        Ok(effects)
    }

    fn handle_confirmed(&mut self, volume: SonosVolume, muted: MuteState, at_ms: u64) -> Result<Vec<Effect>, MappingError> {
        let revision = self.confirmed.map_or(1, |last| last.revision + 1);
        self.confirmed = Some(ConfirmedSonosState { volume, muted, revision, observed_at_ms: at_ms });
        self.pending_volume = None;
        self.pending_mute = None;
        self.state = SyncState::Synchronized;
        Ok(vec![Effect::ApplyLocal(LocalAudioState { volume: self.mapping.to_local(volume)?, muted })])
    }

    pub fn classify_expected_local_callback(&self, expected: sonos_volume_bridge_domain::ExpectedLocalWrite, observed: LocalAudioState, now_ms: u64) -> SuppressionDecision { expected.classify(observed, now_ms) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(value: u8) -> NormalizedVolume { NormalizedVolume::new(value).unwrap() }
    fn s(value: u8) -> SonosVolume { SonosVolume::new(value).unwrap() }
    fn local(volume: u8) -> LocalAudioState { LocalAudioState { volume: v(volume), muted: MuteState(false) } }
    fn sync() -> Synchronizer { Synchronizer::new(VolumeMapping::Linear, s(55), true).unwrap() }
    #[test] fn startup_confirmation_only_applies_sonos_state() { let mut machine = sync(); let effects = machine.handle(SyncEvent::SonosConfirmed { volume: s(24), muted: MuteState(false), source: SonosObservationSource::ExplicitRead, at_ms: 1 }).unwrap(); assert_eq!(effects, vec![Effect::ApplyLocal(local(24))]); assert_eq!(machine.state(), SyncState::Synchronized); }
    #[test] fn local_user_write_is_pending_and_capped() { let mut machine = sync(); let effects = machine.handle(SyncEvent::LocalChanged { state: local(80), origin: LocalOrigin::User, at_ms: 10 }).unwrap(); assert_eq!(effects, vec![Effect::RequestSonosVolume(s(55)), Effect::RequestSonosMute(MuteState(false))]); assert_eq!(machine.pending_volume(), Some(PendingIntent::SetVolume { desired: s(55), created_at_ms: 10 })); }
    #[test] fn latest_local_intent_replaces_older_one() { let mut machine = sync(); machine.handle(SyncEvent::LocalChanged { state: local(10), origin: LocalOrigin::User, at_ms: 1 }).unwrap(); machine.handle(SyncEvent::LocalChanged { state: local(20), origin: LocalOrigin::User, at_ms: 2 }).unwrap(); assert_eq!(machine.pending_volume(), Some(PendingIntent::SetVolume { desired: s(20), created_at_ms: 2 })); }
    #[test] fn external_sonos_confirmation_wins() { let mut machine = sync(); machine.handle(SyncEvent::LocalChanged { state: local(30), origin: LocalOrigin::User, at_ms: 1 }).unwrap(); let effects = machine.handle(SyncEvent::SonosConfirmed { volume: s(35), muted: MuteState(false), source: SonosObservationSource::Event, at_ms: 2 }).unwrap(); assert_eq!(effects, vec![Effect::ApplyLocal(local(35))]); assert_eq!(machine.pending_volume(), None); }
    #[test] fn application_callback_never_writes_sonos() { let mut machine = sync(); let effects = machine.handle(SyncEvent::LocalChanged { state: local(30), origin: LocalOrigin::Application, at_ms: 1 }).unwrap(); assert_eq!(effects, vec![Effect::SuppressedLocalCallback]); }
}

