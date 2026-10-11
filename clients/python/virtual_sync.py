"""VIRTUAL Sync client for Python 3.8+ (Raspberry Pi, laptops, lighting rigs).

Standard library only. Receives a VIRTUAL show's tempo and beat, parameter
values, audio bands and light zones over UDP. See docs/NETWORK_SYNC.md.

    from virtual_sync import Listener

    listener = Listener(passphrase="dance floor", name="Pi lights")
    for message in listener:
        if message.kind == "zones":
            drive_leds(message.colors)          # [(r, g, b), ...]
        elif message.kind == "clock":
            print(listener.beat())              # live beat position

Run ``python3 virtual_sync.py [passphrase]`` to watch a show from the shell.
"""

from __future__ import annotations

import socket
import struct
import sys
import time
from dataclasses import dataclass, field
from typing import Iterator, List, Optional, Tuple

PORT = 41641
VERSION = 1
HEADER = struct.Struct("<4sBBBBIIHH")
MASK = 0xFFFFFFFFFFFFFFFF
STREAM_CLOCK, STREAM_CONTROLS, STREAM_AUDIO, STREAM_ZONES = 1, 2, 4, 8
NODE_FOLLOWER, NODE_LIGHT, NODE_CUSTOM = 2, 3, 4
KINDS = {1: "announce", 2: "hello", 3: "clock", 4: "controls", 5: "audio", 6: "zones", 7: "ping", 8: "pong"}


def _rotl(x: int, b: int) -> int:
    return ((x << b) | (x >> (64 - b))) & MASK


def siphash24(key: Tuple[int, int], data: bytes) -> int:
    """SipHash-2-4, the signature on every datagram of a keyed show."""
    k0, k1 = key
    v0, v1 = 0x736F6D6570736575 ^ k0, 0x646F72616E646F6D ^ k1
    v2, v3 = 0x6C7967656E657261 ^ k0, 0x7465646279746573 ^ k1

    def rounds(n: int) -> None:
        nonlocal v0, v1, v2, v3
        for _ in range(n):
            v0 = (v0 + v1) & MASK; v1 = _rotl(v1, 13) ^ v0; v0 = _rotl(v0, 32)
            v2 = (v2 + v3) & MASK; v3 = _rotl(v3, 16) ^ v2
            v0 = (v0 + v3) & MASK; v3 = _rotl(v3, 21) ^ v0
            v2 = (v2 + v1) & MASK; v1 = _rotl(v1, 17) ^ v2; v2 = _rotl(v2, 32)

    full = len(data) - len(data) % 8
    for i in range(0, full, 8):
        m = int.from_bytes(data[i:i + 8], "little")
        v3 ^= m; rounds(2); v0 ^= m
    last = bytearray(data[full:]) + bytes(8 - (len(data) - full))
    last[7] = len(data) & 0xFF
    m = int.from_bytes(last, "little")
    v3 ^= m; rounds(2); v0 ^= m
    v2 ^= 0xFF
    rounds(4)
    return v0 ^ v1 ^ v2 ^ v3


def derive_key(passphrase: str) -> Optional[Tuple[int, int]]:
    """Show key from the passphrase typed into VIRTUAL; None for unsigned."""
    text = passphrase.strip().encode()
    if not text:
        return None
    return (
        siphash24((0x5649525455414C31, 0x73796E636B657961), text),
        siphash24((0x5649525455414C32, 0x73796E636B657962), text),
    )


def control_id(path: str) -> int:
    """FNV-1a id of a parameter path, e.g. control_id("deck.0.level")."""
    h = 0x811C9DC5
    for byte in path.encode():
        h = ((h ^ byte) * 0x01000193) & 0xFFFFFFFF
    return h


class SyncError(ValueError):
    pass


@dataclass
class Message:
    kind: str
    session: int
    sequence: int
    fields: dict = field(default_factory=dict)

    def __getattr__(self, name):
        try:
            return self.fields[name]
        except KeyError:
            raise AttributeError(name) from None


def parse(data: bytes, key: Optional[Tuple[int, int]]) -> Message:
    """Validates and decodes one datagram. Raises SyncError when invalid."""
    if len(data) < HEADER.size or len(data) > 1400:
        raise SyncError("length")
    magic, version, kind, flags, _, session, sequence, length, _ = HEADER.unpack_from(data)
    if magic != b"VSYN":
        raise SyncError("magic")
    if version != VERSION:
        raise SyncError("version")
    signed = bool(flags & 1)
    end = HEADER.size + length
    if len(data) != end + (8 if signed else 0):
        raise SyncError("length")
    if (key is not None) != signed:
        raise SyncError("signature")
    if key is not None and siphash24(key, data[:end]) != int.from_bytes(data[end:], "little"):
        raise SyncError("signature")
    p = data[HEADER.size:end]
    name = KINDS.get(kind)
    f: dict = {}
    try:
        if name in ("announce", "hello"):
            n = p[0]
            f["name"] = p[1:1 + n].decode()
            f["node_kind"], f["streams"] = p[1 + n], p[2 + n]
            if name == "announce":
                f["zones"] = p[3 + n]
                (f["port"],) = struct.unpack_from("<H", p, 4 + n)
        elif name == "clock":
            f["leader_us"], f["beat"], f["bpm"], f["beats_per_bar"], playing = struct.unpack("<QdfBBxx", p)
            f["playing"] = bool(playing)
        elif name == "controls":
            snapshot, _, count = struct.unpack_from("<BBH", p)
            if len(p) != 4 + 8 * count:
                raise SyncError("payload")
            f["snapshot"] = bool(snapshot)
            f["values"] = [struct.unpack_from("<If", p, 4 + 8 * i) for i in range(count)]
        elif name == "audio":
            values = struct.unpack("<13f", p)
            f.update(zip(("level", "bass", "mid", "high", "hit"), values[:5]))
            f["bands"] = list(values[5:])
        elif name == "zones":
            count = p[0]
            if len(p) != 4 + 3 * count:
                raise SyncError("payload")
            f["colors"] = [tuple(p[4 + 3 * i:7 + 3 * i]) for i in range(count)]
        elif name == "ping":
            (f["follower_us"],) = struct.unpack("<Q", p)
        elif name == "pong":
            f["follower_us"], f["leader_us"] = struct.unpack("<QQ", p)
        else:
            raise SyncError("kind")
    except (struct.error, IndexError, UnicodeDecodeError) as error:
        raise SyncError("payload") from error
    return Message(name, session, sequence, f)


def build_hello(name: str, streams: int, session: int, sequence: int,
                key: Optional[Tuple[int, int]], node_kind: int = NODE_LIGHT) -> bytes:
    """A hello asks the leader to also unicast `streams` to this device."""
    raw = name.encode()[:63]
    body = bytes([len(raw)]) + raw + bytes([node_kind, streams])
    header = HEADER.pack(b"VSYN", VERSION, 2, 1 if key else 0, 0, session, sequence, len(body), 0)
    packet = header + body
    if key:
        packet += siphash24(key, packet).to_bytes(8, "little")
    return packet


class ReplayWindow:
    """Drops duplicate and stale datagrams per sender session."""

    def __init__(self) -> None:
        self.session: Optional[int] = None
        self.highest = 0
        self.seen = 0

    def accept(self, session: int, sequence: int) -> bool:
        if session != self.session:
            self.session, self.highest, self.seen = session, sequence, 1
            return True
        if sequence > self.highest:
            shift = sequence - self.highest
            self.seen = 0 if shift >= 64 else (self.seen << shift) & MASK
            self.seen |= 1
            self.highest = sequence
            return True
        age = self.highest - sequence
        if age >= 64 or self.seen & (1 << age):
            return False
        self.seen |= 1 << age
        return True


class Listener:
    """Listens on the sync port, follows the first (or named) leader, sends
    hellos so the leader also unicasts, and yields decoded messages."""

    def __init__(self, passphrase: str = "", name: str = "Python client",
                 streams: int = STREAM_CLOCK | STREAM_AUDIO | STREAM_ZONES | STREAM_CONTROLS,
                 leader: str = "", port: int = PORT) -> None:
        self.key = derive_key(passphrase)
        self.name, self.streams, self.wanted = name, streams, leader
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("", port))
        self.sock.settimeout(0.5)
        self.leader: Optional[Tuple[str, int]] = None
        self.window = ReplayWindow()
        self.session = int.from_bytes(struct.pack("<d", time.time())[:4], "little") or 1
        self.sequence = 0
        self.last_hello = 0.0
        self.clock: Optional[Message] = None
        self.clock_received = 0.0

    def beat(self) -> Optional[float]:
        """Live beat position, extrapolated from the last clock message."""
        if self.clock is None:
            return None
        return self.clock.beat + (time.monotonic() - self.clock_received) * self.clock.bpm / 60.0

    def __iter__(self) -> Iterator[Message]:
        while True:
            if self.leader and time.monotonic() - self.last_hello > 1.0:
                self.last_hello = time.monotonic()
                self.sequence += 1
                self.sock.sendto(build_hello(self.name, self.streams, self.session,
                                             self.sequence, self.key), self.leader)
            try:
                data, sender = self.sock.recvfrom(1500)
            except socket.timeout:
                continue
            try:
                message = parse(data, self.key)
            except SyncError:
                continue
            if message.kind == "announce" and self.leader is None and self.wanted in ("", message.name):
                self.leader = sender
            if sender != self.leader or not self.window.accept(message.session, message.sequence):
                continue
            if message.kind == "clock":
                self.clock, self.clock_received = message, time.monotonic()
            yield message


if __name__ == "__main__":
    listener = Listener(passphrase=sys.argv[1] if len(sys.argv) > 1 else "")
    print(f"Listening on UDP {PORT}…")
    for msg in listener:
        if msg.kind == "clock":
            print(f"beat {listener.beat():9.3f}  {msg.bpm:6.2f} BPM")
        elif msg.kind == "zones":
            print("zones", msg.colors)
        elif msg.kind == "audio":
            print(f"bass {msg.bass:.2f} mid {msg.mid:.2f} high {msg.high:.2f} hit {msg.hit:.2f}")
        elif msg.kind == "controls":
            print(f"{len(msg.values)} control value(s)")
