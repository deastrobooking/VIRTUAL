//! Localhost RGBA8 stream receiver used by the AudVis viewport source.
use std::io::{self, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use virtual_core::MediaTime;

use crate::{FrameBufferPool, FrameData, RgbaFrame};

const HEADER_BYTES: usize = 36;
const MAX_WIDTH: u32 = 3840;
const MAX_HEIGHT: u32 = 2160;
const MAX_PAYLOAD: usize = MAX_WIDTH as usize * MAX_HEIGHT as usize * 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudvisStreamStats {
    pub connected: bool,
    pub width: u32,
    pub height: u32,
    pub received: u64,
    pub sequence_gaps: u64,
    pub dropped_frames: u64,
    pub last_frame: Option<Instant>,
}

pub struct AudvisFrame {
    pub pixels: RgbaFrame,
    pub pts: MediaTime,
    pub sequence: u64,
}

pub struct AudvisStreamReceiver {
    listener: TcpListener,
    stream: Option<TcpStream>,
    header: [u8; HEADER_BYTES],
    header_read: usize,
    payload: Option<FrameData>,
    payload_read: usize,
    frame: Option<(u32, u32, u64, u64, usize)>,
    first_timestamp: Option<u64>,
    last_sequence: Option<u64>,
    stats: Arc<Mutex<AudvisStreamStats>>,
}

impl AudvisStreamReceiver {
    pub fn bind(port: u16, stats: Arc<Mutex<AudvisStreamStats>>) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            stream: None,
            header: [0; HEADER_BYTES],
            header_read: 0,
            payload: None,
            payload_read: 0,
            frame: None,
            first_timestamp: None,
            last_sequence: None,
            stats,
        })
    }

    pub fn local_port(&self) -> io::Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }

    pub fn poll(&mut self, pool: &FrameBufferPool) -> io::Result<Option<AudvisFrame>> {
        if self.stream.is_none() {
            match self.listener.accept() {
                Ok((stream, address)) if address.ip().is_loopback() => {
                    stream.set_nonblocking(true)?;
                    self.stream = Some(stream);
                    if let Ok(mut stats) = self.stats.lock() { stats.connected = true; }
                }
                Ok((_stream, _)) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        if self.stream.is_none() {
            return Ok(None);
        }
        loop {
            if self.frame.is_none() {
                match self.read_header() {
                    Ok(false) => return Ok(None),
                    Err(error) if is_disconnect(&error) => {
                        self.disconnected();
                        return Ok(None);
                    }
                    Err(error) => return Err(error),
                    Ok(true) => self.frame = Some(validate_header(&self.header)?),
                }
                let (_, _, _, _, length) = self.frame.expect("validated frame header");
                self.payload = Some(pool.acquire(length));
                self.payload_read = 0;
            }
            match self.read_payload() {
                Ok(false) => return Ok(None),
                Err(error) if is_disconnect(&error) => {
                    self.disconnected();
                    return Ok(None);
                }
                Err(error) => return Err(error),
                Ok(true) => {
                    let (width, height, sequence, timestamp, _) =
                        self.frame.take().expect("validated frame header");
                    self.header_read = 0;
                    let first = *self.first_timestamp.get_or_insert(timestamp);
                    let elapsed = timestamp.saturating_sub(first).min(i64::MAX as u64) as i64;
                    let pts = MediaTime::new(elapsed, 1_000_000_000)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    if let Some(previous) = self.last_sequence {
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.sequence_gaps = stats.sequence_gaps.saturating_add(
                                sequence.saturating_sub(previous).saturating_sub(1),
                            );
                        }
                    }
                    self.last_sequence = Some(sequence);
                    if let Ok(mut stats) = self.stats.lock() {
                        stats.received = stats.received.saturating_add(1);
                        stats.width = width;
                        stats.height = height;
                        stats.last_frame = Some(Instant::now());
                    }
                    self.payload_read = 0;
                    return Ok(Some(AudvisFrame {
                        pixels: RgbaFrame {
                            extent: [width, height],
                            data: self.payload.take().expect("completed frame buffer"),
                        },
                        pts,
                        sequence,
                    }));
                }
            }
        }
    }

    fn read_header(&mut self) -> io::Result<bool> {
        let Some(stream) = self.stream.as_mut() else { return Ok(false); };
        read_part(stream, &mut self.header, &mut self.header_read)
    }

    fn read_payload(&mut self) -> io::Result<bool> {
        let Some(stream) = self.stream.as_mut() else { return Ok(false); };
        let Some(payload) = self.payload.as_mut() else { return Ok(false); };
        read_part(stream, payload, &mut self.payload_read)
    }

    fn disconnected(&mut self) {
        self.stream = None;
        if let Ok(mut stats) = self.stats.lock() { stats.connected = false; }
        self.header_read = 0;
        self.payload = None;
        self.payload_read = 0;
        self.frame = None;
        self.first_timestamp = None;
        self.last_sequence = None;
    }
}

fn read_part(stream: &mut TcpStream, target: &mut [u8], filled: &mut usize) -> io::Result<bool> {
    while *filled < target.len() {
        match stream.read(&mut target[*filled..]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stream closed")),
            Ok(count) => *filled += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn is_disconnect(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe)
}

fn validate_header(header: &[u8; HEADER_BYTES]) -> io::Result<(u32, u32, u64, u64, usize)> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    if &header[0..4] != b"VFRM" {
        return Err(invalid("invalid AudVis frame magic"));
    }
    let u16_at = |offset| u16::from_be_bytes([header[offset], header[offset + 1]]);
    let u32_at = |offset| u32::from_be_bytes(header[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_be_bytes(header[offset..offset + 8].try_into().unwrap());
    if u16_at(4) != 1 || u16_at(6) != 1 {
        return Err(invalid("unsupported AudVis frame version or pixel format"));
    }
    let width = u32_at(8);
    let height = u32_at(12);
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err(invalid("AudVis frame dimensions exceed limits"));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|length| *length <= MAX_PAYLOAD)
        .ok_or_else(|| invalid("AudVis frame payload overflows"))?;
    let length = u32_at(32) as usize;
    if length != expected {
        return Err(invalid("AudVis payload length does not match dimensions"));
    }
    Ok((width, height, u64_at(16), u64_at(24), length))
}
