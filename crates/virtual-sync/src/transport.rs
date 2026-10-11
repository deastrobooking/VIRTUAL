//! UDP transport for leaders and followers. Non-blocking: the application
//! polls once per frame, so no threads or locks are involved.
//!
//! The leader binds an ephemeral port and broadcasts to [`DEFAULT_PORT`] (or
//! the configured port); followers listen on that port and learn the leader's
//! address from its announcements. Followers that say hello also get every
//! stream they subscribe to by unicast, which is more reliable than broadcast
//! on busy Wi-Fi. Listen-only devices just use the broadcasts.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use crate::clock::{ClockOffset, LeaderClock};
use crate::mac::Key;
use crate::protocol::{
    ControlValue, Datagram, MAX_DATAGRAM, Message, NodeKind, ReplayWindow, decode, encode, streams,
};

const ANNOUNCE_EVERY: Duration = Duration::from_secs(1);
/// How often the leader re-reads interface broadcast addresses (Wi-Fi changes).
const INTERFACES_EVERY: Duration = Duration::from_secs(5);
const HELLO_EVERY: Duration = Duration::from_secs(1);
/// A follower or leader silent for this long is dropped.
const PEER_TIMEOUT: Duration = Duration::from_secs(5);

fn random_session() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    (hasher.finish() as u32).max(1)
}

fn micros_since(epoch: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(epoch).as_micros() as u64
}

/// A follower seen by a leader.
#[derive(Clone, Debug, PartialEq)]
pub struct FollowerInfo {
    pub address: SocketAddr,
    pub name: String,
    pub kind: NodeKind,
    pub streams: u8,
    pub last_seen: Instant,
}

/// Publishes streams to followers.
pub struct Leader {
    socket: UdpSocket,
    session: u32,
    sequence: u32,
    key: Option<Key>,
    name: String,
    port: u16,
    epoch: Instant,
    broadcast: bool,
    targets: Vec<SocketAddr>,
    followers: HashMap<SocketAddr, FollowerInfo>,
    windows: HashMap<SocketAddr, ReplayWindow>,
    last_announce: Option<Instant>,
    broadcasts: Vec<SocketAddr>,
    broadcasts_read: Option<Instant>,
    offered: u8,
    zones: u8,
    sent: u64,
    rejected: u64,
}

impl Leader {
    /// `port` is where followers listen. `broadcast` also sends every stream
    /// to the subnet broadcast address for listen-only devices; `targets` are
    /// extra fixed addresses (lights with static IPs on networks that block
    /// broadcast).
    pub fn bind(
        name: &str,
        port: u16,
        key: Option<Key>,
        broadcast: bool,
        targets: Vec<SocketAddr>,
        epoch: Instant,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            session: random_session(),
            sequence: 0,
            key,
            name: name.to_owned(),
            port,
            epoch,
            broadcast,
            targets,
            followers: HashMap::new(),
            windows: HashMap::new(),
            last_announce: None,
            broadcasts: Vec::new(),
            broadcasts_read: None,
            offered: streams::ALL,
            zones: 0,
            sent: 0,
            rejected: 0,
        })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().map_or(0, |address| address.port())
    }

    /// Streams and zone count advertised in announcements.
    pub fn set_offer(&mut self, offered: u8, zones: u8) {
        self.offered = offered;
        self.zones = zones;
    }

    fn send_to(&mut self, message: &Message, address: SocketAddr) {
        self.sequence = self.sequence.wrapping_add(1);
        if let Some(bytes) = encode(message, self.session, self.sequence, self.key)
            && self.socket.send_to(&bytes, address).is_ok()
        {
            self.sent += 1;
        }
    }

    /// Every interface's subnet broadcast, plus loopback so a follower on
    /// this machine hears the leader too.
    fn refresh_broadcasts(&mut self, now: Instant) {
        if self
            .broadcasts_read
            .is_some_and(|at| now.saturating_duration_since(at) < INTERFACES_EVERY)
        {
            return;
        }
        self.broadcasts_read = Some(now);
        self.broadcasts = crate::interfaces::broadcast_addresses()
            .into_iter()
            .chain([Ipv4Addr::LOCALHOST])
            .map(|ip| SocketAddr::V4(SocketAddrV4::new(ip, self.port)))
            .collect();
    }

    /// Handles hellos and pings, announces, and expires silent followers.
    pub fn poll(&mut self, now: Instant) {
        let mut buffer = [0u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, from)) => {
                    let Ok(Datagram {
                        session,
                        sequence,
                        message,
                    }) = decode(&buffer[..len], self.key)
                    else {
                        self.rejected += 1;
                        continue;
                    };
                    if !self
                        .windows
                        .entry(from)
                        .or_default()
                        .accept(session, sequence)
                    {
                        continue;
                    }
                    match message {
                        Message::Hello {
                            name,
                            kind,
                            streams,
                        } => {
                            self.followers.insert(
                                from,
                                FollowerInfo {
                                    address: from,
                                    name,
                                    kind,
                                    streams,
                                    last_seen: now,
                                },
                            );
                        }
                        Message::Ping { follower_us } => {
                            let leader_us = micros_since(self.epoch, now);
                            self.send_to(
                                &Message::Pong {
                                    follower_us,
                                    leader_us,
                                },
                                from,
                            );
                        }
                        _ => {}
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        self.followers
            .retain(|_, follower| now.saturating_duration_since(follower.last_seen) < PEER_TIMEOUT);
        self.windows
            .retain(|address, _| self.followers.contains_key(address));
        self.refresh_broadcasts(now);
        if self
            .last_announce
            .is_none_or(|at| now.saturating_duration_since(at) >= ANNOUNCE_EVERY)
        {
            self.last_announce = Some(now);
            let announce = Message::Announce {
                name: self.name.clone(),
                streams: self.offered,
                zones: self.zones,
                port: self.local_port(),
            };
            let mut addresses = self.broadcasts.clone();
            addresses.extend(self.targets.iter().copied());
            for address in addresses {
                self.send_to(&announce, address);
            }
        }
    }

    /// Whether anyone wants `stream` (subscribed followers, broadcast or
    /// fixed targets), so callers can skip expensive work like readback.
    pub fn wants(&self, stream: u8) -> bool {
        self.broadcast
            || !self.targets.is_empty()
            || self
                .followers
                .values()
                .any(|follower| follower.streams & stream != 0)
    }

    /// Sends one datagram of `stream` to everyone who wants it. A follower
    /// that also hears the broadcast discards the duplicate by sequence.
    pub fn publish(&mut self, message: &Message, stream: u8) {
        self.sequence = self.sequence.wrapping_add(1);
        let Some(bytes) = encode(message, self.session, self.sequence, self.key) else {
            return;
        };
        let mut addresses: Vec<SocketAddr> = self
            .followers
            .values()
            .filter(|follower| follower.streams & stream != 0)
            .map(|follower| follower.address)
            .collect();
        addresses.extend(self.targets.iter().copied());
        if self.broadcast {
            addresses.extend(self.broadcasts.iter().copied());
        }
        addresses.sort();
        addresses.dedup();
        for address in addresses {
            if self.socket.send_to(&bytes, address).is_ok() {
                self.sent += 1;
            }
        }
    }

    /// The current time on this leader's clock, as sent in clock messages.
    pub fn now_us(&self, now: Instant) -> u64 {
        micros_since(self.epoch, now)
    }

    pub fn followers(&self) -> Vec<FollowerInfo> {
        let mut followers: Vec<_> = self.followers.values().cloned().collect();
        followers.sort_by(|a, b| a.name.cmp(&b.name));
        followers
    }

    /// Datagrams sent, and datagrams rejected (bad signature or malformed).
    pub fn stats(&self) -> (u64, u64) {
        (self.sent, self.rejected)
    }
}

/// A leader seen by a follower.
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderInfo {
    pub address: SocketAddr,
    pub name: String,
    pub streams: u8,
    pub zones: u8,
    pub last_seen: Instant,
}

/// What a follower received this poll.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Received {
    pub controls: Vec<ControlValue>,
    pub audio: Option<Message>,
    pub zones: Option<Vec<[u8; 3]>>,
}

/// Receives a leader's streams.
pub struct Follower {
    socket: UdpSocket,
    session: u32,
    sequence: u32,
    key: Option<Key>,
    name: String,
    kind: NodeKind,
    streams: u8,
    epoch: Instant,
    leaders: HashMap<SocketAddr, LeaderInfo>,
    windows: HashMap<SocketAddr, ReplayWindow>,
    /// Leader to follow by name; empty follows the first one heard.
    wanted: String,
    following: Option<SocketAddr>,
    clock: Option<LeaderClock>,
    offset: ClockOffset,
    last_hello: Option<Instant>,
    received: u64,
    rejected: u64,
}

impl Follower {
    pub fn bind(
        name: &str,
        port: u16,
        key: Option<Key>,
        streams: u8,
        epoch: Instant,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            session: random_session(),
            sequence: 0,
            key,
            name: name.to_owned(),
            kind: NodeKind::Follower,
            streams,
            epoch,
            leaders: HashMap::new(),
            windows: HashMap::new(),
            wanted: String::new(),
            following: None,
            clock: None,
            offset: ClockOffset::default(),
            last_hello: None,
            received: 0,
            rejected: 0,
        })
    }

    /// Follows the leader with this name; empty follows the first heard.
    pub fn follow(&mut self, name: &str) {
        if self.wanted != name {
            self.wanted = name.to_owned();
            self.following = None;
            self.clock = None;
            self.offset = ClockOffset::default();
        }
    }

    fn send_to(&mut self, message: &Message, address: SocketAddr) {
        self.sequence = self.sequence.wrapping_add(1);
        if let Some(bytes) = encode(message, self.session, self.sequence, self.key) {
            let _ = self.socket.send_to(&bytes, address);
        }
    }

    /// Receives everything pending, keeps the subscription alive and measures
    /// the leader's clock.
    pub fn poll(&mut self, now: Instant) -> Received {
        let mut received = Received::default();
        let mut buffer = [0u8; MAX_DATAGRAM];
        loop {
            let (len, from) = match self.socket.recv_from(&mut buffer) {
                Ok(packet) => packet,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            };
            let Ok(datagram) = decode(&buffer[..len], self.key) else {
                self.rejected += 1;
                continue;
            };
            let local_us = micros_since(self.epoch, now);
            // Clock replies come from the leader's socket, like everything else.
            if let Message::Announce {
                name,
                streams,
                zones,
                ..
            } = &datagram.message
            {
                self.leaders.insert(
                    from,
                    LeaderInfo {
                        address: from,
                        name: name.clone(),
                        streams: *streams,
                        zones: *zones,
                        last_seen: now,
                    },
                );
                if self.following.is_none() && (self.wanted.is_empty() || self.wanted == *name) {
                    self.following = Some(from);
                }
            }
            if Some(from) != self.following {
                continue;
            }
            if !self
                .windows
                .entry(from)
                .or_default()
                .accept(datagram.session, datagram.sequence)
            {
                continue;
            }
            self.received += 1;
            if let Some(leader) = self.leaders.get_mut(&from) {
                leader.last_seen = now;
            }
            match datagram.message {
                Message::Clock {
                    leader_us,
                    beat,
                    bpm,
                    beats_per_bar,
                    playing,
                } => {
                    self.offset.add_one_way(leader_us, local_us);
                    self.clock = Some(LeaderClock {
                        leader_us,
                        beat,
                        bpm,
                        beats_per_bar,
                        playing,
                    });
                }
                Message::Pong {
                    follower_us,
                    leader_us,
                } => self.offset.add_round_trip(follower_us, leader_us, local_us),
                Message::Controls { values, .. } => received.controls.extend(values),
                message @ Message::Audio { .. } => received.audio = Some(message),
                Message::Zones { colors } => received.zones = Some(colors),
                _ => {}
            }
        }
        self.leaders
            .retain(|_, leader| now.saturating_duration_since(leader.last_seen) < PEER_TIMEOUT);
        if self
            .following
            .is_some_and(|address| !self.leaders.contains_key(&address))
        {
            self.following = None;
            self.clock = None;
        }
        if let Some(leader) = self.following
            && self
                .last_hello
                .is_none_or(|at| now.saturating_duration_since(at) >= HELLO_EVERY)
        {
            self.last_hello = Some(now);
            let hello = Message::Hello {
                name: self.name.clone(),
                kind: self.kind,
                streams: self.streams,
            };
            self.send_to(&hello, leader);
            let follower_us = micros_since(self.epoch, now);
            self.send_to(&Message::Ping { follower_us }, leader);
        }
        received
    }

    /// The followed leader's beat, tempo and bar length at `now`.
    pub fn beat_at(&self, now: Instant) -> Option<(f64, f32, u8, bool)> {
        let clock = self.clock?;
        let offset = self.offset.offset_us()?;
        let beat = clock.beat_at(micros_since(self.epoch, now), offset);
        Some((beat, clock.bpm, clock.beats_per_bar, clock.playing))
    }

    pub fn leaders(&self) -> Vec<LeaderInfo> {
        let mut leaders: Vec<_> = self.leaders.values().cloned().collect();
        leaders.sort_by(|a, b| a.name.cmp(&b.name));
        leaders
    }

    pub fn following(&self) -> Option<&LeaderInfo> {
        self.following
            .and_then(|address| self.leaders.get(&address))
    }

    /// Best recent round trip to the leader, the clock estimate's uncertainty.
    pub fn round_trip(&self) -> Option<Duration> {
        self.offset.best_rtt_us().map(Duration::from_micros)
    }

    /// Datagrams accepted from the leader, and datagrams rejected.
    pub fn stats(&self) -> (u64, u64) {
        (self.received, self.rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac::derive_key;
    use crate::protocol::control_id;

    /// Leader and follower on loopback: discovery by unicast target (the
    /// sandbox may not allow broadcast), hello, clock, controls and pings.
    #[test]
    fn a_follower_discovers_subscribes_and_tracks_the_leader() {
        let epoch = Instant::now();
        let key = derive_key("side room");
        let mut follower = Follower::bind("TV", 0, key, streams::ALL, epoch).unwrap();
        let follower_port = follower.socket.local_addr().unwrap().port();
        let target = SocketAddr::from(([127, 0, 0, 1], follower_port));
        let mut leader =
            Leader::bind("Main", follower_port, key, false, vec![target], epoch).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut controls = Vec::new();
        let mut step = 0u32;
        while Instant::now() < deadline {
            let now = Instant::now();
            leader.poll(now);
            leader.publish(
                &Message::Clock {
                    leader_us: leader.now_us(now),
                    beat: 64.0,
                    bpm: 120.0,
                    beats_per_bar: 4,
                    playing: true,
                },
                streams::CLOCK,
            );
            if step == 5 {
                leader.publish(
                    &Message::Controls {
                        snapshot: false,
                        values: vec![ControlValue {
                            id: control_id("deck.0.level"),
                            value: 0.25,
                        }],
                    },
                    streams::CONTROLS,
                );
            }
            controls.extend(follower.poll(Instant::now()).controls);
            step += 1;
            if !leader.followers().is_empty()
                && follower.round_trip().is_some()
                && !controls.is_empty()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(follower.following().map(|l| l.name.as_str()), Some("Main"));
        assert_eq!(leader.followers()[0].name, "TV");
        assert_eq!(
            controls,
            [ControlValue {
                id: control_id("deck.0.level"),
                value: 0.25
            }]
        );
        let (beat, bpm, bar, playing) = follower.beat_at(Instant::now()).unwrap();
        assert_eq!((bpm, bar, playing), (120.0, 4, true));
        assert!((64.0..66.0).contains(&beat), "beat {beat}");
        assert!(follower.round_trip().unwrap() < Duration::from_millis(500));
    }

    #[test]
    fn a_wrong_show_key_is_ignored_and_counted() {
        let epoch = Instant::now();
        let mut follower =
            Follower::bind("TV", 0, derive_key("right"), streams::ALL, epoch).unwrap();
        let port = follower.socket.local_addr().unwrap().port();
        let target = SocketAddr::from(([127, 0, 0, 1], port));
        let mut leader = Leader::bind(
            "Rogue",
            port,
            derive_key("wrong"),
            false,
            vec![target],
            epoch,
        )
        .unwrap();
        for _ in 0..20 {
            leader.poll(Instant::now());
            follower.poll(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(follower.leaders().is_empty());
        assert!(follower.stats().1 > 0);
    }
}
