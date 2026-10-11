//! VIRTUAL Sync wire format, version 1.
//!
//! Every datagram is a 20-byte header, a payload and, when signed, an 8-byte
//! SipHash-2-4 tag over header and payload. All integers and floats are
//! little-endian, which is native on ESP32, ARM and x86, so small clients read
//! fields in place.
//!
//! ```text
//! offset size field
//!      0    4 magic "VSYN"
//!      4    1 version (1)
//!      5    1 kind (see `Message`)
//!      6    1 flags: bit 0 = signed (8-byte tag follows the payload)
//!      7    1 reserved, 0
//!      8    4 session: random per sender run
//!     12    4 sequence: increments per datagram
//!     16    2 payload length
//!     18    2 reserved, 0
//! ```
//!
//! Payloads are documented on each [`Message`] variant; `docs/NETWORK_SYNC.md`
//! has the same tables for client authors.

use crate::mac::{Key, siphash24};

pub const MAGIC: [u8; 4] = *b"VSYN";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 20;
pub const TAG_LEN: usize = 8;
/// Largest datagram sent: fits one Wi-Fi frame without IP fragmentation.
pub const MAX_DATAGRAM: usize = 1400;
pub const DEFAULT_PORT: u16 = 41_641;
pub const FLAG_SIGNED: u8 = 1;
pub const MAX_NAME: usize = 63;
pub const SPECTRUM_BANDS: usize = 8;
pub const MAX_ZONES: usize = 128;
/// Control entries per datagram: 8 bytes each after a 4-byte prefix.
pub const CONTROLS_PER_DATAGRAM: usize = (MAX_DATAGRAM - HEADER_LEN - TAG_LEN - 4) / 8;

/// Streams a leader offers or a follower wants (bit flags).
pub mod streams {
    pub const CLOCK: u8 = 1;
    pub const CONTROLS: u8 = 2;
    pub const AUDIO: u8 = 4;
    pub const ZONES: u8 = 8;
    pub const ALL: u8 = CLOCK | CONTROLS | AUDIO | ZONES;
}

/// What kind of device a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Leader,
    Follower,
    Light,
    Custom,
}

impl NodeKind {
    fn code(self) -> u8 {
        match self {
            Self::Leader => 1,
            Self::Follower => 2,
            Self::Light => 3,
            Self::Custom => 4,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Leader,
            2 => Self::Follower,
            3 => Self::Light,
            _ => Self::Custom,
        }
    }
}

/// One parameter value. `id` is [`control_id`] of the parameter's path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlValue {
    pub id: u32,
    pub value: f32,
}

/// FNV-1a 32-bit hash of a parameter path such as `deck.0.level`. Stable
/// across builds; clients compare against published constants.
pub const fn control_id(path: &str) -> u32 {
    let bytes = path.as_bytes();
    let mut hash: u32 = 0x811c_9dc5;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }
    hash
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// Kind 1, leader → broadcast, about once a second.
    /// `name_len u8, name, node_kind u8, streams u8, zone_count u8, port u16`.
    Announce {
        name: String,
        streams: u8,
        zones: u8,
        port: u16,
    },
    /// Kind 2, follower → leader, about once a second; keeps it subscribed.
    /// `name_len u8, name, node_kind u8, streams u8`.
    Hello {
        name: String,
        kind: NodeKind,
        streams: u8,
    },
    /// Kind 3, leader, 20 per second. `leader_us u64, beat f64, bpm f32,
    /// beats_per_bar u8, playing u8, reserved u16`. `beat` is the musical
    /// position at `leader_us` on the leader's monotonic clock.
    Clock {
        leader_us: u64,
        beat: f64,
        bpm: f32,
        beats_per_bar: u8,
        playing: bool,
    },
    /// Kind 4, leader. `snapshot u8, reserved u8, count u16, count ×
    /// (id u32, value f32)`. Snapshot datagrams repeat every value about once
    /// a second so late joiners and lost packets converge; others are changes.
    Controls {
        snapshot: bool,
        values: Vec<ControlValue>,
    },
    /// Kind 5, leader, up to 60 per second. `level, bass, mid, high, hit,
    /// bands[8]`, all f32 in 0–1.
    Audio {
        level: f32,
        bass: f32,
        mid: f32,
        high: f32,
        hit: f32,
        bands: [f32; SPECTRUM_BANDS],
    },
    /// Kind 6, leader, 30 per second. `count u8, reserved [3], count × rgb`.
    /// The program image averaged into vertical strips, left to right, sRGB.
    Zones { colors: Vec<[u8; 3]> },
    /// Kind 7, follower → leader. `follower_us u64`.
    Ping { follower_us: u64 },
    /// Kind 8, leader → follower. `follower_us u64` (echoed), `leader_us u64`.
    Pong { follower_us: u64, leader_us: u64 },
}

impl Message {
    pub fn kind(&self) -> u8 {
        match self {
            Self::Announce { .. } => 1,
            Self::Hello { .. } => 2,
            Self::Clock { .. } => 3,
            Self::Controls { .. } => 4,
            Self::Audio { .. } => 5,
            Self::Zones { .. } => 6,
            Self::Ping { .. } => 7,
            Self::Pong { .. } => 8,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("not a VIRTUAL Sync datagram")]
    Magic,
    #[error("unsupported protocol version {0}")]
    Version(u8),
    #[error("truncated or oversized datagram")]
    Length,
    #[error("unknown message kind {0}")]
    Kind(u8),
    #[error("signature missing or wrong")]
    Signature,
    #[error("malformed payload")]
    Payload,
}

/// A decoded datagram with its sender identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Datagram {
    pub session: u32,
    pub sequence: u32,
    pub message: Message,
}

fn put_name(out: &mut Vec<u8>, name: &str) {
    let mut end = name.len().min(MAX_NAME);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    out.push(end as u8);
    out.extend_from_slice(&name.as_bytes()[..end]);
}

fn payload(message: &Message) -> Vec<u8> {
    let mut out = Vec::new();
    match message {
        Message::Announce {
            name,
            streams,
            zones,
            port,
        } => {
            put_name(&mut out, name);
            out.push(NodeKind::Leader.code());
            out.push(*streams);
            out.push(*zones);
            out.extend_from_slice(&port.to_le_bytes());
        }
        Message::Hello {
            name,
            kind,
            streams,
        } => {
            put_name(&mut out, name);
            out.push(kind.code());
            out.push(*streams);
        }
        Message::Clock {
            leader_us,
            beat,
            bpm,
            beats_per_bar,
            playing,
        } => {
            out.extend_from_slice(&leader_us.to_le_bytes());
            out.extend_from_slice(&beat.to_le_bytes());
            out.extend_from_slice(&bpm.to_le_bytes());
            out.push(*beats_per_bar);
            out.push(u8::from(*playing));
            out.extend_from_slice(&[0, 0]);
        }
        Message::Controls { snapshot, values } => {
            out.push(u8::from(*snapshot));
            out.push(0);
            out.extend_from_slice(&(values.len() as u16).to_le_bytes());
            for value in values {
                out.extend_from_slice(&value.id.to_le_bytes());
                out.extend_from_slice(&value.value.to_le_bytes());
            }
        }
        Message::Audio {
            level,
            bass,
            mid,
            high,
            hit,
            bands,
        } => {
            for value in [level, bass, mid, high, hit].into_iter().chain(bands) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        Message::Zones { colors } => {
            out.push(colors.len().min(MAX_ZONES) as u8);
            out.extend_from_slice(&[0, 0, 0]);
            for color in colors.iter().take(MAX_ZONES) {
                out.extend_from_slice(color);
            }
        }
        Message::Ping { follower_us } => out.extend_from_slice(&follower_us.to_le_bytes()),
        Message::Pong {
            follower_us,
            leader_us,
        } => {
            out.extend_from_slice(&follower_us.to_le_bytes());
            out.extend_from_slice(&leader_us.to_le_bytes());
        }
    }
    out
}

/// Encodes one datagram. Control messages longer than one datagram must be
/// split with [`control_datagrams`] first; this returns `None` if the payload
/// does not fit.
pub fn encode(message: &Message, session: u32, sequence: u32, key: Option<Key>) -> Option<Vec<u8>> {
    let body = payload(message);
    let total = HEADER_LEN + body.len() + if key.is_some() { TAG_LEN } else { 0 };
    if total > MAX_DATAGRAM {
        return None;
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(message.kind());
    out.push(if key.is_some() { FLAG_SIGNED } else { 0 });
    out.push(0);
    out.extend_from_slice(&session.to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&(body.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&body);
    if let Some(key) = key {
        let tag = siphash24(key, &out);
        out.extend_from_slice(&tag.to_le_bytes());
    }
    Some(out)
}

/// Splits control values into messages that each fit one datagram.
pub fn control_datagrams(values: &[ControlValue], snapshot: bool) -> Vec<Message> {
    values
        .chunks(CONTROLS_PER_DATAGRAM)
        .map(|chunk| Message::Controls {
            snapshot,
            values: chunk.to_vec(),
        })
        .collect()
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        let slice = self
            .data
            .get(self.at..self.at + n)
            .ok_or(DecodeError::Payload)?;
        self.at += n;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, DecodeError> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn name(&mut self) -> Result<String, DecodeError> {
        let len = usize::from(self.u8()?);
        if len > MAX_NAME {
            return Err(DecodeError::Payload);
        }
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| DecodeError::Payload)
    }
}

/// Decodes and authenticates one datagram. With a `key`, unsigned or wrongly
/// signed datagrams are rejected; without one, signed datagrams are rejected
/// too, so a mismatched configuration fails loudly instead of half working.
pub fn decode(data: &[u8], key: Option<Key>) -> Result<Datagram, DecodeError> {
    if data.len() < HEADER_LEN || data.len() > MAX_DATAGRAM {
        return Err(DecodeError::Length);
    }
    if data[..4] != MAGIC {
        return Err(DecodeError::Magic);
    }
    if data[4] != VERSION {
        return Err(DecodeError::Version(data[4]));
    }
    let kind = data[5];
    let signed = data[6] & FLAG_SIGNED != 0;
    let session = u32::from_le_bytes(data[8..12].try_into().unwrap());
    let sequence = u32::from_le_bytes(data[12..16].try_into().unwrap());
    let length = usize::from(u16::from_le_bytes(data[16..18].try_into().unwrap()));
    let body_end = HEADER_LEN + length;
    if data.len() != body_end + if signed { TAG_LEN } else { 0 } {
        return Err(DecodeError::Length);
    }
    match (key, signed) {
        (Some(key), true) => {
            let tag = u64::from_le_bytes(data[body_end..].try_into().unwrap());
            if siphash24(key, &data[..body_end]) != tag {
                return Err(DecodeError::Signature);
            }
        }
        (None, false) => {}
        _ => return Err(DecodeError::Signature),
    }
    let mut r = Reader {
        data: &data[HEADER_LEN..body_end],
        at: 0,
    };
    let message = match kind {
        1 => {
            let name = r.name()?;
            let _kind = r.u8()?;
            Message::Announce {
                name,
                streams: r.u8()?,
                zones: r.u8()?,
                port: r.u16()?,
            }
        }
        2 => Message::Hello {
            name: r.name()?,
            kind: NodeKind::from_code(r.u8()?),
            streams: r.u8()?,
        },
        3 => {
            let message = Message::Clock {
                leader_us: r.u64()?,
                beat: r.f64()?,
                bpm: r.f32()?,
                beats_per_bar: r.u8()?,
                playing: r.u8()? != 0,
            };
            r.u16()?;
            message
        }
        4 => {
            let snapshot = r.u8()? != 0;
            r.u8()?;
            let count = usize::from(r.u16()?);
            let values = (0..count)
                .map(|_| {
                    Ok(ControlValue {
                        id: r.u32()?,
                        value: r.f32()?,
                    })
                })
                .collect::<Result<_, DecodeError>>()?;
            Message::Controls { snapshot, values }
        }
        5 => {
            let mut values = [0.0f32; 5 + SPECTRUM_BANDS];
            for value in &mut values {
                *value = r.f32()?;
            }
            Message::Audio {
                level: values[0],
                bass: values[1],
                mid: values[2],
                high: values[3],
                hit: values[4],
                bands: values[5..].try_into().unwrap(),
            }
        }
        6 => {
            let count = usize::from(r.u8()?);
            r.take(3)?;
            if count > MAX_ZONES {
                return Err(DecodeError::Payload);
            }
            let colors = (0..count)
                .map(|_| r.take(3).map(|rgb| [rgb[0], rgb[1], rgb[2]]))
                .collect::<Result<_, _>>()?;
            Message::Zones { colors }
        }
        7 => Message::Ping {
            follower_us: r.u64()?,
        },
        8 => Message::Pong {
            follower_us: r.u64()?,
            leader_us: r.u64()?,
        },
        other => return Err(DecodeError::Kind(other)),
    };
    if r.at != r.data.len() {
        return Err(DecodeError::Payload);
    }
    Ok(Datagram {
        session,
        sequence,
        message,
    })
}

/// Rejects datagrams already seen or too old, per sender session. Wi-Fi can
/// reorder packets, so a 64-datagram window accepts late arrivals once.
#[derive(Clone, Debug, Default)]
pub struct ReplayWindow {
    session: Option<u32>,
    highest: u32,
    seen: u64,
}

impl ReplayWindow {
    /// True when the datagram is new and should be processed.
    pub fn accept(&mut self, session: u32, sequence: u32) -> bool {
        if self.session != Some(session) {
            // A restarted sender has a new random session.
            *self = Self {
                session: Some(session),
                highest: sequence,
                seen: 1,
            };
            return true;
        }
        if sequence > self.highest {
            let shift = sequence - self.highest;
            self.seen = if shift >= 64 { 0 } else { self.seen << shift };
            self.seen |= 1;
            self.highest = sequence;
            return true;
        }
        let age = self.highest - sequence;
        if age >= 64 || self.seen & (1 << age) != 0 {
            return false;
        }
        self.seen |= 1 << age;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac::derive_key;

    fn round_trip(message: Message, key: Option<Key>) {
        let bytes = encode(&message, 0xdead_beef, 42, key).unwrap();
        assert!(bytes.len() <= MAX_DATAGRAM);
        let datagram = decode(&bytes, key).unwrap();
        assert_eq!(datagram.message, message);
        assert_eq!((datagram.session, datagram.sequence), (0xdead_beef, 42));
    }

    fn all_messages() -> Vec<Message> {
        vec![
            Message::Announce {
                name: "Main room".into(),
                streams: streams::ALL,
                zones: 8,
                port: DEFAULT_PORT,
            },
            Message::Hello {
                name: "ESP32 strip".into(),
                kind: NodeKind::Light,
                streams: streams::CLOCK | streams::ZONES,
            },
            Message::Clock {
                leader_us: 123_456_789,
                beat: 1234.5,
                bpm: 128.0,
                beats_per_bar: 4,
                playing: true,
            },
            Message::Controls {
                snapshot: true,
                values: vec![
                    ControlValue {
                        id: control_id("deck.0.level"),
                        value: 0.75,
                    },
                    ControlValue {
                        id: control_id("master.blackout"),
                        value: 1.0,
                    },
                ],
            },
            Message::Audio {
                level: 0.5,
                bass: 0.9,
                mid: 0.3,
                high: 0.1,
                hit: 1.0,
                bands: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            },
            Message::Zones {
                colors: vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]],
            },
            Message::Ping { follower_us: 7 },
            Message::Pong {
                follower_us: 7,
                leader_us: 99,
            },
        ]
    }

    #[test]
    fn every_message_round_trips_signed_and_unsigned() {
        let key = derive_key("dance floor");
        for message in all_messages() {
            round_trip(message.clone(), None);
            round_trip(message, key);
        }
    }

    #[test]
    fn signatures_reject_tampering_and_mismatched_keys() {
        let key = derive_key("dance floor");
        let message = &all_messages()[2];
        let mut bytes = encode(message, 1, 1, key).unwrap();
        assert!(decode(&bytes, derive_key("other")).is_err());
        assert_eq!(decode(&bytes, None), Err(DecodeError::Signature));
        let unsigned = encode(message, 1, 1, None).unwrap();
        assert_eq!(decode(&unsigned, key), Err(DecodeError::Signature));
        bytes[HEADER_LEN + 3] ^= 1;
        assert_eq!(decode(&bytes, key), Err(DecodeError::Signature));
    }

    #[test]
    fn malformed_datagrams_are_rejected() {
        let good = encode(&all_messages()[0], 1, 1, None).unwrap();
        assert_eq!(decode(&good[..10], None), Err(DecodeError::Length));
        let mut bad = good.clone();
        bad[0] = b'X';
        assert_eq!(decode(&bad, None), Err(DecodeError::Magic));
        let mut bad = good.clone();
        bad[4] = 9;
        assert_eq!(decode(&bad, None), Err(DecodeError::Version(9)));
        let mut bad = good.clone();
        bad[5] = 77;
        assert_eq!(decode(&bad, None), Err(DecodeError::Kind(77)));
        let mut bad = good;
        bad.push(0);
        assert_eq!(decode(&bad, None), Err(DecodeError::Length));
    }

    #[test]
    fn large_control_sets_split_into_datagrams_that_fit() {
        let values: Vec<ControlValue> = (0..400)
            .map(|i| ControlValue {
                id: i,
                value: i as f32,
            })
            .collect();
        let messages = control_datagrams(&values, true);
        assert_eq!(messages.len(), 400usize.div_ceil(CONTROLS_PER_DATAGRAM));
        let mut decoded = Vec::new();
        for message in &messages {
            let bytes = encode(message, 1, 1, derive_key("k")).unwrap();
            if let Message::Controls { values, .. } =
                decode(&bytes, derive_key("k")).unwrap().message
            {
                decoded.extend(values);
            }
        }
        assert_eq!(decoded, values);
    }

    #[test]
    fn replay_window_rejects_duplicates_and_stale_packets() {
        let mut window = ReplayWindow::default();
        assert!(window.accept(9, 100));
        assert!(!window.accept(9, 100), "duplicate");
        assert!(window.accept(9, 102));
        assert!(window.accept(9, 101), "late but new");
        assert!(!window.accept(9, 101));
        assert!(window.accept(9, 200));
        assert!(!window.accept(9, 120), "older than the window");
        assert!(window.accept(10, 1), "a restarted sender starts fresh");
    }

    #[test]
    fn control_ids_are_fnv1a() {
        assert_eq!(control_id(""), 0x811c_9dc5);
        assert_eq!(control_id("a"), 0xe40c_292c);
    }
}
