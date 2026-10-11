//! VIRTUAL Sync in the application: a leader publishes tempo, parameter
//! changes, audio and light zones; a follower applies the leader's tempo and
//! the control groups it follows. See `docs/NETWORK_SYNC.md`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use virtual_core::{ControlTarget, ControlUpdate};
use virtual_io::{SyncProject, SyncRoleProject};
use virtual_render::{ProgramReadback, ProgramTarget};
use virtual_session::{CommandOperation, CommandOrigin, control_parameter_path};
use virtual_sync::{
    ControlValue, Follower, Leader, Message, NodeKind, control_id, derive_key,
    protocol::control_datagrams, streams, zones::average_zones,
};

const CLOCK_EVERY: Duration = Duration::from_millis(50);
const AUDIO_EVERY: Duration = Duration::from_millis(33);
const ZONES_EVERY: Duration = Duration::from_millis(33);
const SNAPSHOT_EVERY: Duration = Duration::from_secs(1);
const INDEX_EVERY: Duration = Duration::from_secs(2);

/// The network id of a control target.
pub(crate) fn target_id(target: ControlTarget) -> u32 {
    control_id(&control_parameter_path(target))
}

/// Which follow option covers a control. `None` is never followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FollowGroup {
    Mixer,
    Transport,
    Effects,
    Buttons,
}

pub(crate) fn follow_group(target: ControlTarget) -> Option<FollowGroup> {
    use ControlTarget as T;
    Some(match target {
        T::Crossfader
        | T::MasterOpacity
        | T::MasterBlackout
        | T::MasterFreeze
        | T::DeckLevel(_)
        | T::DeckMute(_)
        | T::DeckPin(_)
        | T::DeckLayerTop(_)
        | T::DeckLayerUp(_)
        | T::DeckLayerDown(_)
        | T::LayerReset
        | T::DeckSelect(_) => FollowGroup::Mixer,
        T::DeckPlay(_)
        | T::DeckFreeze(_)
        | T::DeckSpeed(_)
        | T::DeckRestart(_)
        | T::ClipLaunch { .. }
        | T::SceneLaunch(_)
        | T::DeckAutomation(_) => FollowGroup::Transport,
        T::EffectParameter { .. }
        | T::LfoParameter { .. }
        | T::ModRouteParameter { .. }
        | T::GeneratorParameter { .. }
        | T::DeckEffectParameter { .. }
        | T::MasterEffectParameter { .. } => FollowGroup::Effects,
        T::UiButton(_) => FollowGroup::Buttons,
        // Tempo follows through the clock stream instead.
        T::TapTempo => return None,
    })
}

fn follows(settings: &SyncProject, group: FollowGroup) -> bool {
    match group {
        FollowGroup::Mixer => settings.follow_mixer,
        FollowGroup::Transport => settings.follow_transport,
        FollowGroup::Effects => settings.follow_effects,
        FollowGroup::Buttons => settings.follow_buttons,
    }
}

/// Parses `ip` or `ip:port` targets; a bare IP uses the sync port.
fn parse_targets(targets: &[String], port: u16) -> Vec<SocketAddr> {
    targets
        .iter()
        .filter_map(|target| {
            target.parse::<SocketAddr>().ok().or_else(|| {
                target
                    .parse::<std::net::IpAddr>()
                    .ok()
                    .map(|ip| SocketAddr::new(ip, port))
            })
        })
        .collect()
}

/// What the sockets were built for; any change rebinds.
#[derive(Clone, Debug, PartialEq)]
struct Binding {
    role: SyncRoleProject,
    name: String,
    passphrase: String,
    port: u16,
    broadcast: bool,
    targets: Vec<String>,
    follower_streams: u8,
}

impl Binding {
    fn from(settings: &SyncProject) -> Self {
        Self {
            role: settings.role,
            name: settings.name.clone(),
            passphrase: settings.passphrase.clone(),
            port: settings.port,
            broadcast: settings.broadcast,
            targets: settings.targets.clone(),
            follower_streams: follower_streams(settings),
        }
    }
}

fn leader_streams(settings: &SyncProject) -> u8 {
    [
        (settings.send_clock, streams::CLOCK),
        (settings.send_controls, streams::CONTROLS),
        (settings.send_audio, streams::AUDIO),
        (settings.send_zones, streams::ZONES),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(0, |bits, (_, bit)| bits | bit)
}

fn follower_streams(settings: &SyncProject) -> u8 {
    let mut bits = 0;
    if settings.follow_tempo {
        bits |= streams::CLOCK;
    }
    if settings.follow_mixer
        || settings.follow_transport
        || settings.follow_effects
        || settings.follow_buttons
    {
        bits |= streams::CONTROLS;
    }
    bits
}

/// A peer, for the Setup panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SyncPeer {
    pub name: String,
    pub detail: String,
}

/// Snapshot for the UI.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SyncStatus {
    pub role: SyncRoleProject,
    pub active: bool,
    pub error: Option<String>,
    pub peers: Vec<SyncPeer>,
    pub following: Option<String>,
    pub round_trip_ms: Option<f32>,
    pub sent_or_received: u64,
    pub rejected: u64,
    pub applied: u64,
}

#[derive(Default)]
pub(crate) struct NetworkSync {
    leader: Option<Leader>,
    follower: Option<Follower>,
    binding: Option<Binding>,
    error: Option<String>,
    pending: Vec<ControlValue>,
    last_clock: Option<Instant>,
    last_audio: Option<Instant>,
    last_snapshot: Option<Instant>,
    last_zones: Option<Instant>,
    readback: Option<ProgramReadback>,
    zone_count: u8,
    index: HashMap<u32, ControlTarget>,
    index_built: Option<Instant>,
    applied: u64,
}

impl NetworkSync {
    /// Binds, rebinds or closes sockets to match `settings`.
    fn configure(&mut self, settings: &SyncProject, epoch: Instant) {
        let binding = Binding::from(settings);
        if self.binding.as_ref() == Some(&binding) {
            return;
        }
        self.leader = None;
        self.follower = None;
        self.readback = None;
        self.pending.clear();
        self.error = None;
        self.binding = Some(binding);
        let key = derive_key(&settings.passphrase);
        let name = if settings.name.trim().is_empty() {
            "VIRTUAL"
        } else {
            settings.name.trim()
        };
        let result = match settings.role {
            SyncRoleProject::Off => Ok(()),
            SyncRoleProject::Leader => Leader::bind(
                name,
                settings.port,
                key,
                settings.broadcast,
                parse_targets(&settings.targets, settings.port),
                epoch,
            )
            .map(|leader| self.leader = Some(leader)),
            SyncRoleProject::Follower => {
                Follower::bind(name, settings.port, key, follower_streams(settings), epoch)
                    .map(|follower| self.follower = Some(follower))
            }
        };
        if let Err(error) = result {
            self.error = Some(format!(
                "Could not open UDP port {}: {error}",
                settings.port
            ));
        }
    }

    /// Queues a control change for followers (leader only).
    pub(crate) fn note(&mut self, target: ControlTarget, value: f32) {
        if self.leader.is_some() && value.is_finite() {
            let id = target_id(target);
            if let Some(existing) = self.pending.iter_mut().find(|pending| pending.id == id) {
                existing.value = value;
            } else {
                self.pending.push(ControlValue { id, value });
            }
        }
    }

    /// Records a program-frame copy for light zones when someone wants them.
    pub(crate) fn capture(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        program: &ProgramTarget,
        settings: &SyncProject,
        now: Instant,
    ) {
        let Some(leader) = &self.leader else {
            return;
        };
        if !settings.send_zones
            || !leader.wants(streams::ZONES)
            || self
                .last_zones
                .is_some_and(|at| now.saturating_duration_since(at) < ZONES_EVERY)
        {
            return;
        }
        self.last_zones = Some(now);
        self.zone_count = settings.zones.clamp(1, 128);
        if self
            .readback
            .as_ref()
            .is_none_or(|readback| readback.extent() != program.extent())
        {
            self.readback = Some(ProgramReadback::new(device, program.extent()));
        }
        if let Some(readback) = &mut self.readback {
            readback.capture(encoder, program.texture());
        }
    }

    /// After the frame's submission: averages finished copies into zones.
    pub(crate) fn after_submit(&mut self, device: &wgpu::Device) {
        let (Some(readback), Some(leader)) = (self.readback.as_mut(), self.leader.as_mut()) else {
            return;
        };
        let zones = self.zone_count;
        readback.poll(device, |frame| {
            let colors = average_zones(frame.data, frame.extent, frame.stride, usize::from(zones));
            leader.publish(&Message::Zones { colors }, streams::ZONES);
        });
    }

    pub(crate) fn status(&self) -> SyncStatus {
        let role = self
            .binding
            .as_ref()
            .map_or(SyncRoleProject::Off, |binding| binding.role);
        let mut status = SyncStatus {
            role,
            error: self.error.clone(),
            applied: self.applied,
            ..SyncStatus::default()
        };
        let now = Instant::now();
        if let Some(leader) = &self.leader {
            status.active = true;
            (status.sent_or_received, status.rejected) = leader.stats();
            status.peers = leader
                .followers()
                .into_iter()
                .map(|follower| SyncPeer {
                    detail: format!(
                        "{} · {} · {:.1} s ago",
                        match follower.kind {
                            NodeKind::Follower => "VIRTUAL",
                            NodeKind::Light => "light",
                            _ => "device",
                        },
                        follower.address,
                        now.saturating_duration_since(follower.last_seen)
                            .as_secs_f32()
                    ),
                    name: follower.name,
                })
                .collect();
        }
        if let Some(follower) = &self.follower {
            status.active = true;
            (status.sent_or_received, status.rejected) = follower.stats();
            status.following = follower.following().map(|leader| leader.name.clone());
            status.round_trip_ms = follower.round_trip().map(|rtt| rtt.as_secs_f32() * 1000.0);
            status.peers = follower
                .leaders()
                .into_iter()
                .map(|leader| SyncPeer {
                    detail: format!("leader · {}", leader.address),
                    name: leader.name,
                })
                .collect();
        }
        status
    }
}

impl super::State {
    /// Once per frame, before input handling settles: leader publishing or
    /// follower receiving and applying.
    pub(crate) fn update_network_sync(&mut self, now: Instant) {
        let settings = self.ui.sync.clone();
        self.network_sync
            .configure(&settings, self.performance_started);
        if self.network_sync.leader.is_some() {
            self.publish_sync(&settings, now);
        } else if self.network_sync.follower.is_some() {
            self.follow_sync(&settings, now);
        }
    }

    fn publish_sync(&mut self, settings: &SyncProject, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.performance_started)
            .as_secs_f64();
        let beat = self.tempo.beat_at(elapsed);
        let bpm = self.tempo.bpm() as f32;
        let beats_per_bar = self.tempo.beats_per_bar().min(255) as u8;
        let snapshot_due = settings.send_controls
            && self
                .network_sync
                .last_snapshot
                .is_none_or(|at| now.saturating_duration_since(at) >= SNAPSHOT_EVERY);
        let snapshot: Vec<ControlValue> = if snapshot_due {
            super::performance_control_snapshot(&self.ui, &self.mixer, &self.transports)
                .into_iter()
                .map(|(target, value)| ControlValue {
                    id: target_id(target),
                    value,
                })
                .collect()
        } else {
            Vec::new()
        };
        let audio = self.audio_snapshot.analysis;
        let sync = &mut self.network_sync;
        let Some(leader) = sync.leader.as_mut() else {
            return;
        };
        leader.set_offer(leader_streams(settings), settings.zones);
        leader.poll(now);
        let pending = std::mem::take(&mut sync.pending);
        if settings.send_controls && !pending.is_empty() {
            for message in control_datagrams(&pending, false) {
                leader.publish(&message, streams::CONTROLS);
            }
        }
        if snapshot_due {
            sync.last_snapshot = Some(now);
            for message in control_datagrams(&snapshot, true) {
                leader.publish(&message, streams::CONTROLS);
            }
        }
        if settings.send_clock
            && sync
                .last_clock
                .is_none_or(|at| now.saturating_duration_since(at) >= CLOCK_EVERY)
        {
            sync.last_clock = Some(now);
            leader.publish(
                &Message::Clock {
                    leader_us: leader.now_us(now),
                    beat,
                    bpm,
                    beats_per_bar,
                    playing: true,
                },
                streams::CLOCK,
            );
        }
        if settings.send_audio
            && leader.wants(streams::AUDIO)
            && sync
                .last_audio
                .is_none_or(|at| now.saturating_duration_since(at) >= AUDIO_EVERY)
        {
            sync.last_audio = Some(now);
            leader.publish(
                &Message::Audio {
                    level: audio.rms,
                    bass: audio.bass,
                    mid: audio.mid,
                    high: audio.high,
                    hit: audio.transient,
                    bands: audio.bands,
                },
                streams::AUDIO,
            );
        }
    }

    /// Maps network ids back to this machine's control targets.
    fn rebuild_sync_index(&mut self, now: Instant) {
        let mut targets: Vec<ControlTarget> =
            super::performance_control_snapshot(&self.ui, &self.mixer, &self.transports)
                .into_keys()
                .collect();
        targets.extend(crate::ui::known_control_targets());
        targets.extend(crate::ui::buttons::known_targets());
        for (deck, package) in self.ui.deck_packages.iter().enumerate() {
            for parameter in &package.parameters {
                targets.push(ControlTarget::DeckEffectParameter {
                    deck: deck as u8,
                    parameter_key: virtual_core::effect_parameter_key(
                        &package.package_id,
                        &parameter.id,
                    ),
                });
            }
        }
        for (slot, effect) in self.ui.master_effects.slots.iter().enumerate() {
            for parameter in &effect.parameters {
                targets.push(ControlTarget::MasterEffectParameter {
                    slot: slot as u8,
                    parameter_key: virtual_core::effect_parameter_key(
                        &effect.package_id,
                        &parameter.id,
                    ),
                });
            }
        }
        self.network_sync.index = targets
            .into_iter()
            .map(|target| (target_id(target), target))
            .collect();
        self.network_sync.index_built = Some(now);
    }

    fn follow_sync(&mut self, settings: &SyncProject, now: Instant) {
        let Some(follower) = self.network_sync.follower.as_mut() else {
            return;
        };
        follower.follow(settings.leader.trim());
        let received = follower.poll(now);
        let clock = follower.beat_at(now);
        if settings.follow_tempo
            && let Some((beat, bpm, beats_per_bar, _)) = clock
        {
            self.follow_leader_clock(beat, f64::from(bpm), beats_per_bar, now);
        }
        if received.controls.is_empty() {
            return;
        }
        if self
            .network_sync
            .index_built
            .is_none_or(|at| now.saturating_duration_since(at) >= INDEX_EVERY)
            || received
                .controls
                .iter()
                .any(|value| !self.network_sync.index.contains_key(&value.id))
                && self.network_sync.index_built.is_none_or(|at| {
                    now.saturating_duration_since(at) >= Duration::from_millis(250)
                })
        {
            self.rebuild_sync_index(now);
        }
        for value in received.controls {
            let Some(&target) = self.network_sync.index.get(&value.id) else {
                continue;
            };
            if !follow_group(target).is_some_and(|group| follows(settings, group)) {
                continue;
            }
            let current =
                super::current_control_value(&self.ui, &self.mixer, &self.transports, target);
            // Snapshots repeat every value; only changes become commands.
            if (current - value.value).abs() < 1e-5 {
                continue;
            }
            self.network_sync.applied += 1;
            self.dispatch_control_update(
                ControlUpdate {
                    target,
                    value: value.value,
                },
                CommandOrigin::Remote("virtual-sync".to_owned()),
                now,
            );
        }
    }

    /// Adopts the leader's tempo and beat phase, as Ableton Link does.
    fn follow_leader_clock(&mut self, beat: f64, bpm: f64, beats_per_bar: u8, now: Instant) {
        if !beat.is_finite() || !(20.0..=400.0).contains(&bpm) {
            return;
        }
        let elapsed = now
            .saturating_duration_since(self.performance_started)
            .as_secs_f64();
        if (self.ui.bpm - bpm).abs() > 1e-4 {
            self.record_show_operation(
                CommandOrigin::Remote("virtual-sync".to_owned()),
                now,
                CommandOperation::SetTempo { bpm },
            );
            self.ui.bpm = bpm;
            if let Some(sender) = &self.midi_clock_sender {
                sender.set_bpm(bpm);
            }
            self.publish_osc_value(&crate::osc::osc_address("/tempo"), bpm as f32);
        }
        if beats_per_bar > 0 {
            self.tempo.set_beats_per_bar(u32::from(beats_per_bar));
        }
        self.tempo.set_bpm(bpm, elapsed);
        self.launches.anchor_clock(&mut self.tempo, beat, elapsed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_target_has_a_unique_network_id() {
        let mut targets = crate::ui::known_control_targets();
        targets.sort();
        targets.dedup();
        let mut ids: Vec<u32> = targets.iter().map(|target| target_id(*target)).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert!(before > 300, "the MIDI target list covers the show");
        assert_eq!(ids.len(), before, "no two control paths share an id");
        assert_eq!(
            target_id(ControlTarget::DeckLevel(0)),
            control_id("deck.0.level")
        );
    }

    #[test]
    fn follow_groups_and_targets_parse() {
        assert_eq!(
            follow_group(ControlTarget::Crossfader),
            Some(FollowGroup::Mixer)
        );
        assert_eq!(
            follow_group(ControlTarget::ClipLaunch { deck: 0, slot: 3 }),
            Some(FollowGroup::Transport)
        );
        assert_eq!(
            follow_group(ControlTarget::UiButton(9)),
            Some(FollowGroup::Buttons)
        );
        assert_eq!(follow_group(ControlTarget::TapTempo), None);
        let defaults = SyncProject::default();
        assert!(
            !follows(&defaults, FollowGroup::Buttons),
            "remote button presses are opt-in"
        );
        assert_eq!(
            parse_targets(
                &["10.0.0.5".into(), "10.0.0.6:5000".into(), "nope".into()],
                41_641
            ),
            [
                "10.0.0.5:41641".parse::<SocketAddr>().unwrap(),
                "10.0.0.6:5000".parse().unwrap()
            ]
        );
        assert_eq!(leader_streams(&defaults), streams::ALL);
        assert_eq!(
            follower_streams(&defaults),
            streams::CLOCK | streams::CONTROLS
        );
    }
}
