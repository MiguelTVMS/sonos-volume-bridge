//! Async orchestration ports and a bounded synchronization coordinator.
//!
//! This crate knows no native platform API, HTTP implementation, or Tauri type.

use async_trait::async_trait;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, SonosVolume};
use sonos_volume_bridge_synchronization::{Effect, SonosObservationSource, SyncEvent, Synchronizer};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::{sync::mpsc, time::sleep};

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("Sonos port failed: {0}")]
    Sonos(String),
    #[error("local audio port failed: {0}")]
    Audio(String),
    #[error("synchronizer rejected an event: {0}")]
    State(String),
}

#[async_trait]
pub trait SonosPort: Send + Sync {
    async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError>;
    async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError>;
    async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError>;
}

#[async_trait]
pub trait LocalAudioPort: Send + Sync {
    async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Health { Healthy, SubscriptionDegraded, PollingFallback }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimingMetrics { pub last_command_to_confirmation: Option<Duration> }

#[derive(Clone, Copy, Debug)]
pub struct SynchronizationPolicy { pub volume_debounce: Duration, pub unhealthy_poll_interval: Duration, pub healthy_poll_interval: Duration }
impl Default for SynchronizationPolicy { fn default() -> Self { Self { volume_debounce: Duration::from_millis(45), unhealthy_poll_interval: Duration::from_secs(1), healthy_poll_interval: Duration::from_secs(30) } } }

/// Coalesces only volume writes. Mute writes bypass debounce and are dispatched immediately.
pub struct Coordinator<S, A> { synchronizer: Synchronizer, sonos: S, audio: A, policy: SynchronizationPolicy, health: Health, local_tx: mpsc::Sender<SyncEvent>, local_rx: mpsc::Receiver<SyncEvent>, last_command: Option<Instant>, metrics: TimingMetrics }

impl<S: SonosPort, A: LocalAudioPort> Coordinator<S, A> {
    pub fn new(synchronizer: Synchronizer, sonos: S, audio: A, policy: SynchronizationPolicy) -> Self {
        let (local_tx, local_rx) = mpsc::channel(1);
        Self { synchronizer, sonos, audio, policy, health: Health::SubscriptionDegraded, local_tx, local_rx, last_command: None, metrics: TimingMetrics::default() }
    }
    pub fn local_event_sender(&self) -> mpsc::Sender<SyncEvent> { self.local_tx.clone() }
    pub const fn health(&self) -> Health { self.health }
    pub const fn timing_metrics(&self) -> TimingMetrics { self.metrics }
    pub async fn reconcile_startup(&mut self) -> Result<(), IntegrationError> {
        let (volume, muted) = self.sonos.current_state().await?;
        self.handle(SyncEvent::SonosConfirmed { volume, muted, source: SonosObservationSource::ExplicitRead, at_ms: now_ms() }).await
    }
    pub async fn on_sonos_event(&mut self, volume: SonosVolume, muted: MuteState) -> Result<(), IntegrationError> {
        self.health = Health::Healthy;
        self.handle(SyncEvent::SonosConfirmed { volume, muted, source: SonosObservationSource::Event, at_ms: now_ms() }).await
    }
    pub async fn on_subscription_lost(&mut self) { self.health = Health::PollingFallback; }
    pub async fn poll_once(&mut self) -> Result<(), IntegrationError> {
        let (volume, muted) = self.sonos.current_state().await?;
        self.handle(SyncEvent::SonosConfirmed { volume, muted, source: SonosObservationSource::Poll, at_ms: now_ms() }).await
    }
    pub fn next_poll_interval(&self) -> Duration { if self.health == Health::Healthy { self.policy.healthy_poll_interval } else { self.policy.unhealthy_poll_interval } }
    pub async fn run_local_once(&mut self) -> Result<(), IntegrationError> {
        let Some(first) = self.local_rx.recv().await else { return Ok(()); };
        match first {
            SyncEvent::LocalChanged { state, origin, at_ms } if origin != LocalOrigin::Application => {
                sleep(self.policy.volume_debounce).await;
                let mut newest = SyncEvent::LocalChanged { state, origin, at_ms };
                while let Ok(event) = self.local_rx.try_recv() { newest = event; }
                self.handle(newest).await
            },
            event => self.handle(event).await,
        }
    }
    pub async fn on_local_event(&mut self, state: LocalAudioState, origin: LocalOrigin, mute_changed: bool) -> Result<(), IntegrationError> {
        let event = SyncEvent::LocalChanged { state, origin, at_ms: now_ms() };
        if mute_changed { self.handle(event).await } else { self.local_tx.try_send(event).map_err(|_| IntegrationError::Audio("local volume queue is full".to_owned())) }
    }
    async fn handle(&mut self, event: SyncEvent) -> Result<(), IntegrationError> {
        let effects = self.synchronizer.handle(event).map_err(|error| IntegrationError::State(error.to_string()))?;
        for effect in effects { self.apply(effect).await?; }
        Ok(())
    }
    async fn apply(&mut self, effect: Effect) -> Result<(), IntegrationError> {
        match effect {
            Effect::RequestSonosVolume(volume) => { self.last_command = Some(Instant::now()); self.sonos.set_volume(volume).await },
            Effect::RequestSonosMute(muted) => { self.last_command = Some(Instant::now()); self.sonos.set_mute(muted).await },
            Effect::ApplyLocal(state) => { self.metrics.last_command_to_confirmation = self.last_command.map(|start| start.elapsed()); self.audio.apply(state).await },
            Effect::SuppressedLocalCallback => Ok(()),
        }
    }
}

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().try_into().unwrap_or(u64::MAX) }

#[cfg(test)]
mod tests {
    use super::*;
    use sonos_volume_bridge_domain::{NormalizedVolume, VolumeMapping};
    use std::sync::{Arc, Mutex};
    struct Sonos(Arc<Mutex<(SonosVolume, MuteState)>>);
    #[async_trait] impl SonosPort for Sonos { async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError> { Ok(*self.0.lock().unwrap()) } async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError> { self.0.lock().unwrap().0 = volume; Ok(()) } async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError> { self.0.lock().unwrap().1 = muted; Ok(()) } }
    struct Audio(Arc<Mutex<Vec<LocalAudioState>>>);
    #[async_trait] impl LocalAudioPort for Audio { async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError> { self.0.lock().unwrap().push(state); Ok(()) } }
    fn volume(value: u8) -> SonosVolume { SonosVolume::new(value).unwrap() }
    #[tokio::test]
    async fn startup_reconciles_from_sonos_without_writing_back() {
        let state = Arc::new(Mutex::new((volume(24), MuteState(false)))); let applied = Arc::new(Mutex::new(Vec::new()));
        let machine = Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true).unwrap();
        let mut coordinator = Coordinator::new(machine, Sonos(Arc::clone(&state)), Audio(Arc::clone(&applied)), SynchronizationPolicy::default());
        coordinator.reconcile_startup().await.unwrap();
        assert_eq!(applied.lock().unwrap()[0].volume, NormalizedVolume::new(24).unwrap());
        assert_eq!(*state.lock().unwrap(), (volume(24), MuteState(false)));
    }
    #[tokio::test]
    async fn later_sonos_confirmation_wins_over_local_intent() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false)))); let applied = Arc::new(Mutex::new(Vec::new()));
        let machine = Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true).unwrap();
        let mut coordinator = Coordinator::new(machine, Sonos(Arc::clone(&state)), Audio(Arc::clone(&applied)), SynchronizationPolicy::default());
        coordinator.on_local_event(LocalAudioState { volume: NormalizedVolume::new(30).unwrap(), muted: MuteState(false) }, LocalOrigin::User, true).await.unwrap();
        coordinator.on_sonos_event(volume(35), MuteState(false)).await.unwrap();
        assert_eq!(applied.lock().unwrap().last().unwrap().volume, NormalizedVolume::new(35).unwrap());
    }
    #[tokio::test]
    async fn lost_subscription_uses_fast_polling_until_event_recovers() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false)))); let applied = Arc::new(Mutex::new(Vec::new()));
        let machine = Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true).unwrap();
        let mut coordinator = Coordinator::new(machine, Sonos(state), Audio(applied), SynchronizationPolicy::default());
        coordinator.on_subscription_lost().await;
        assert_eq!(coordinator.health(), Health::PollingFallback);
        assert_eq!(coordinator.next_poll_interval(), Duration::from_secs(1));
        coordinator.on_sonos_event(volume(20), MuteState(false)).await.unwrap();
        assert_eq!(coordinator.health(), Health::Healthy);
    }
}
