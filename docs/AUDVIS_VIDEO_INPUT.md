# AudVis video input for VIRTUAL

This note is the implementation handoff for adding a direct Blender video
source to VIRTUAL and its companion sender in the AudVis fork. The target is a
Blender viewport image routed into one VIRTUAL deck. It should work on the same
computer without OBS, NDI, or a virtual camera driver. The existing OSC bridge
can remain available for controls, but it is not part of the frame path.

## Proposed first transport

Use a localhost TCP stream with tightly packed RGBA8 frames. This keeps the
first version self-contained in both projects: AudVis uses Python's standard
library socket and VIRTUAL owns the listener and frame decoder. Bind only to
`127.0.0.1`; use port `9010` by default, separate from OSC input `9000` and
feedback `9001`. The connection and selected source are session-only.

TCP provides framing order and reliable delivery, so the sender must keep a
single replaceable pending frame rather than build a queue. Pixel readback and
socket writes must not stall Blender's UI or VIRTUAL's render loop. If a sender
can't keep up, discard an unsent old frame and send the newest one. VIRTUAL
must keep bounded receive and decoder queues and report sequence gaps and frame
age. The receiver treats disconnect as loss of signal and reconnects when the
sender returns.

### Provisional frame wire format

Each frame starts with one fixed 36-byte network-byte-order header, followed by
exactly `payload_bytes` bytes. Version 1 uses tightly packed, top-to-bottom
RGBA8 pixels; therefore `stride_bytes = width * 4` and
`payload_bytes = width * height * 4`.

| Offset | Size | Field | Version 1 value |
| ---: | ---: | --- | --- |
| 0 | 4 | Magic | ASCII `VFRM` |
| 4 | 2 | Protocol version | `1` |
| 6 | 2 | Pixel format | `1` = RGBA8 |
| 8 | 4 | Width | pixels |
| 12 | 4 | Height | pixels |
| 16 | 8 | Frame sequence | increasing unsigned integer |
| 24 | 8 | Capture timestamp | nanoseconds, monotonic on sender |
| 32 | 4 | Payload length | bytes |

In Python, the provisional header format is `!4sHHIIQQI`. Treat all values as
unsigned. Reject unknown magic/version/format, zero or out-of-range dimensions,
overflow, a payload length that does not equal `width * height * 4`, and frames
larger than the configured cap (initially 3840 × 2160). Do not allocate from an
unvalidated length. A connection starts with frames immediately; no OSC-style
handshake is needed. Keep the version and header spec in sync before either
side ships.

## VIRTUAL receiver design

The receiver should enter the existing deck live-media path as a source
adapter. It must not upload textures directly from the socket thread. A
dedicated bounded worker reads and validates complete frames, converts them to
the existing `RgbaFrame` representation, timestamps them, and sends them
through the same bounded scheduling channel used by other live inputs. Reuse
the frame buffer pool, stale-frame drop behavior, generation cancellation,
deck effects, A/B mixer, recording, freeze, and program output. Keep the
listener loopback-only, stop it cleanly on disconnect or source replacement,
and surface permission-free connection status and dropped-frame counts in the
video input controls.

Add a distinct **AudVis stream** source choice to the deck video input. Keep it
out of the persistent project format for the first pass: host, port, and source
availability are local session state. Do not silently open a listener merely
because a project was loaded. The existing camera/capture-card route remains
unchanged.

## AudVis sender study

The existing fork is at `/Users/randolphchabot/Desktop/audvis`. Its current
`ui/virtual_bridge.py` handles OSC only. Study Blender's viewport draw-handler
and GPU framebuffer APIs for a render-context readback point. The sender needs
to capture the selected 3D Viewport image (not the full Blender window), handle
pixel row orientation and viewport resize, and avoid reading pixels when the
viewport is hidden or minimized.

Keep Blender API calls and framebuffer readback on Blender's main/render
context. Copy the resulting bytes into a one-frame replaceable handoff, then
let a worker thread perform blocking socket writes. Do not call `bpy` from that
worker. On stop, file load, unregister, or socket failure, remove the draw
handler, close the socket, stop the worker, and release queued frame memory.
Show connection state, dimensions, sent frame rate, and dropped frames in the
VIRTUAL Bridge panel. Keep transport controls and host/port session-only.

Things to verify in Blender before freezing the protocol: whether the draw
callback can copy the framebuffer without recursive redraw; color space and
alpha behavior; top-left versus bottom-left row order; HiDPI dimensions; and
reasonable readback cost at 720p/30 and 1080p/30. Blender must continue
rendering normally when VIRTUAL is disconnected. The viewport source should be
explicitly started/stopped, not active for every scene by default.

## Prototype implementation status

AudVis now has a session-only viewport sender and VIRTUAL has a loopback TCP
listener connected to the deck live-frame path. Both use the provisional v1
header above. The sender keeps a replaceable pending frame and moves blocking
socket writes to a worker; the framebuffer read remains in Blender's
`POST_PIXEL` draw callback. VIRTUAL validates the header and dimensions before
allocating, reads on its deck worker, reuses the existing frame pool, and reports
received dimensions/count, sender sequence gaps, local scheduler drops and time
since the last complete frame arrived.

The protocol is still provisional. Blender UI/GPU behavior, frame orientation,
HiDPI dimensions, color handling and 720p/1080p sustained readback cost have not
been verified in an interactive Blender/VIRTUAL session. The receiver currently
reports sender sequence gaps; application-side scheduler drops and frame age
need further instrumentation before this is a stable show transport.

## Delivery order

1. Prototype AudVis viewport readback and measure copy cost without networking.
2. Implement the VIRTUAL loopback listener and bounded frame source; first use a
   tiny standalone sender to validate framing, reconnect, resolution changes,
   malformed lengths, and backpressure.
3. Connect the source to the normal deck scheduler and controls.
4. Add the AudVis sender, status UI, and lifecycle cleanup; test Blender reload,
   viewport resize, source loss, VIRTUAL disconnect, and sustained operation.
5. Document macOS permissions and capture behavior, then decide whether a
   cross-platform transport or an optional compressed format is warranted.

This prototype sends video only. It does not carry audio, OSC controls, cue
execution times, or shared show identities. TCP on loopback is a local
best-effort live stream, not a recording or exactly-once protocol. Confirm the
header and target frame rates against measurements from the AudVis prototype
before treating them as a stable interface.
