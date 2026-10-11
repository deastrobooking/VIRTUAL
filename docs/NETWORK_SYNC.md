# VIRTUAL Sync

VIRTUAL Sync keeps other machines and devices in step with a VIRTUAL show over
the local network, Wi-Fi or Ethernet. One VIRTUAL machine is the **leader**.
**Followers** can be:

- another VIRTUAL machine (a side room, a TV wall, a backup), following the
  leader's tempo and any of its parameter groups;
- a lighting controller on an ESP32, an Arduino with Wi-Fi or a Raspberry Pi,
  using the beat, audio bands, parameter values and light zones;
- anything else that can receive UDP, using the C or Python client or this
  document.

Video itself is not carried: send the program picture with
[NDI](OPERATOR_GUIDE.md#ndi-output) and use Sync for timing and control.

## Setting it up

In **Setup → Network sync**:

1. On the main machine choose **Leader**, give it a name and type a **show
   key** (any phrase). Tick what to send: tempo and beat, parameter changes,
   audio bands and light zones (with the number of zones).
2. On each other VIRTUAL machine choose **Follower**, type the same show key
   and tick what to follow: tempo and beat, mixer and master, transport, clips
   and scenes, effects and generators, and (off by default) button presses.
   Leave **Follow leader** empty to follow the first leader heard, or type a
   leader's name when several shows share a network.
3. Flash the [ESP32 example](../clients/arduino/VirtualSyncLeds/VirtualSyncLeds.ino)
   or run [`virtual_sync.py`](../clients/python/virtual_sync.py) on a Pi with
   the same show key.

The Setup panel lists connected followers (leader) or leaders (follower) with
the measured round trip, and the toolbar **SYNC** dot is green while
connected. Settings are saved with the project. **Leader**, **Follower** and
**Off** can be mapped to MIDI buttons.

A follower applies the leader's changes through the same command path as its
own controls, so they are journaled, appear in OSC feedback, and take over any
clip automation driving the same parameter. Snapshots that repeat unchanged
values are not re-applied. Following tempo replaces the local clock source
while connected, the same way Ableton Link does. Clip and scene launches refer
to slot numbers, so a follower with different media plays its own clip in the
same slot.

## Wi-Fi notes

- The leader **broadcasts** on every active interface's subnet (for example
  `192.168.1.255`) so listen-only devices receive everything without setup.
  Followers that send a hello also receive their streams by **unicast**, which
  is more reliable than broadcast on busy Wi-Fi. Turn broadcast off when every
  device says hello (the C and Python clients do).
- Some access points isolate wireless clients or block broadcast. List devices
  with static IPs under **Fixed receivers** (`ip` or `ip:port`), or use a
  dedicated router for the show.
- UDP port 41641 must be open on every machine's firewall. The leader sends
  from a random port; followers learn it from announcements.
- Beat timing does not depend on network delay: every clock message carries
  the leader's beat at a timestamp, and followers predict the beat locally.
  VIRTUAL followers also measure round trips (the "round trip" shown in Setup)
  and keep the least-delayed sample, so phase is accurate to about half the
  best round trip, typically 1–3 ms on a good network.
- Light zones and audio run at about 30 messages per second. Zones are read
  back from the GPU only while some device wants them.

## Security

With a show key, every datagram carries a 64-bit SipHash-2-4 tag over its
header and payload, keyed by a 128-bit key derived from the phrase. Receivers
drop datagrams with a missing or wrong tag, and a sliding sequence window drops
replayed and duplicate datagrams, so another machine on the network cannot
inject or replay control of your show. Traffic is not encrypted: anyone on the
network can read the tempo and parameter values. Without a show key, traffic is
unsigned and any device on the network can send to followers; use a key on
shared or public networks.

The key is derived as `k0 = SipHash(salt_a, phrase)`,
`k1 = SipHash(salt_b, phrase)` with leading and trailing whitespace trimmed,
where `salt_a = (0x5649525455414c31, 0x73796e636b657961)` and
`salt_b = (0x5649525455414c32, 0x73796e636b657962)`.

## Wire format (version 1)

UDP datagrams, at most 1400 bytes (one Wi-Fi frame, no IP fragmentation). All
integers and floats are little-endian. A datagram is a 20-byte header, the
payload, and, when signed, the 8-byte tag.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `VSYN` |
| 4 | 1 | version, `1` |
| 5 | 1 | kind |
| 6 | 1 | flags: bit 0 = signed |
| 7 | 1 | reserved, 0 |
| 8 | 4 | session: random per sender run |
| 12 | 4 | sequence: +1 per datagram |
| 16 | 2 | payload length |
| 18 | 2 | reserved, 0 |
| 20 | n | payload |
| 20+n | 8 | tag (signed only): SipHash-2-4 of bytes 0..20+n |

Receivers reject wrong magic, version or lengths, unknown kinds, payloads that
do not exactly match their kind, and signature mismatches in both directions
(a signed datagram without a configured key is rejected too).

| Kind | Name | Direction | Payload |
|---|---|---|---|
| 1 | announce | leader → broadcast, 1/s | `name_len u8, name UTF-8 (≤63), node_kind u8 (1), streams u8, zones u8, port u16` |
| 2 | hello | follower → leader, 1/s | `name_len u8, name, node_kind u8, streams u8` |
| 3 | clock | leader, 20/s | `leader_us u64, beat f64, bpm f32, beats_per_bar u8, playing u8, reserved u16` |
| 4 | controls | leader | `snapshot u8, reserved u8, count u16, count × (id u32, value f32)` |
| 5 | audio | leader, ~30/s | `level, bass, mid, high, hit, bands[8]`, f32 in 0–1 |
| 6 | zones | leader, ~30/s | `count u8, reserved [3], count × (r, g, b) u8` |
| 7 | ping | follower → leader | `follower_us u64` |
| 8 | pong | leader → follower | `follower_us u64` (echoed), `leader_us u64` |

- **streams** bits: 1 clock, 2 controls, 4 audio, 8 zones. A hello asks the
  leader to unicast those streams to the sender; it expires after 5 seconds
  without a hello.
- **node_kind**: 1 leader, 2 VIRTUAL follower, 3 light, 4 other.
- **clock**: `beat` was the musical position at `leader_us` on the leader's
  monotonic clock. The beat at leader time `t` is
  `beat + (t − leader_us) / 60e6 × bpm`. Without ping/pong, estimate
  `t ≈ leader_us + (now − time the clock arrived)`.
- **controls**: snapshot datagrams repeat every value about once a second
  (late joiners and lost packets converge); others carry changes as they
  happen. Triggers (clip and scene launches, restarts) appear only as
  changes, as 1 then 0.
- **zones**: the program image averaged into equal-width vertical strips, left
  to right, sRGB.
- **ping/pong**: clock offset = `leader_us + rtt/2 − received`, where
  `rtt = received − follower_us`; keep the sample with the smallest round trip.

### Control ids

A control's `id` is the 32-bit FNV-1a hash (offset `0x811c9dc5`, prime
`0x01000193`) of its path. `vs_control_id()` and `control_id()` compute it.
Values use the same units as VIRTUAL's MIDI and OSC targets: levels and most
parameters 0–1, speed 0.25–4, toggles 0 or 1.

| Path | Id |
|---|---|
| `mixer.crossfader` | `0x7bd6cba0` |
| `master.opacity` | `0x3d0aa814` |
| `master.blackout` | `0x714f9ce2` |
| `master.freeze` | `0xe48f4558` |
| `deck.0.level` | `0xadded40e` |
| `deck.0.play` | `0x2f07c4fa` |
| `deck.0.speed` | `0x5329e4d3` |
| `deck.0.mute` | `0xf8e5ca51` |
| `deck.0.clip.0.launch` | `0x893bc92d` |
| `scene.0.launch` | `0x1da401ea` |

Other paths (decks and slots are zero-based): `deck.N.freeze`,
`deck.N.select`, `deck.N.restart`, `deck.N.automation`, `deck.N.layer.pin`,
`deck.N.layer.top|up|down`, `mixer.layer.reset`, `deck.N.clip.S.launch`,
`scene.S.launch`, `deck.N.effect.E.parameter.0`,
`deck.N.lfo.L.parameter.P`, `deck.N.mod_route.R.parameter.P`,
`deck.N.generator.parameter.P`, `deck.N.package.parameter.<key hex16>`,
`master.effect.S.parameter.<key hex16>`, `ui.button.<key hex16>`.

## Clients

See [`clients/`](../clients/README.md): a single-header C99 library for
microcontrollers and any C/C++ program, a standard-library Python module, and
an ESP32 + WS2812 example. Both libraries are tested against golden datagrams
produced by the Rust encoder.

## Limits

- Local networks only, IPv4. There is no relay across the internet.
- Followers receive; they do not send control back to the leader.
- One leader per show key and network: followers follow one leader at a time.
- The leader's light zones are vertical strips of the whole program picture;
  pixel-mapped fixtures and DMX (Art-Net or sACN) output are not built in yet.
