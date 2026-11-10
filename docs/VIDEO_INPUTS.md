# Cameras and video capture cards

VIRTUAL routes macOS video inputs to any of its four decks. This includes
cameras and capture cards whose drivers expose them through AVFoundation.
The input can use deck effects, blending, freeze and camera-to-clip recording.
Video inputs are live and cannot seek. Embedded capture-card audio is not
decoded by this path; select an audio-analysis input separately.

## AudVis viewport stream

Choose **AudVis stream** in the deck's video input controls, keep port `9010`,
and click **Listen**. In Blender, start **VIRTUAL Viewport Stream** from the
target 3D Viewport. The listener is loopback-only and starts only when selected;
the port and active source are not saved in the project. VIRTUAL displays
connection state, current frame dimensions, received frames and sequence gaps.

The provisional v1 TCP stream has a 36-byte network-order header (`VFRM`,
version 1, RGBA8, width, height, sequence, sender monotonic timestamp and byte
length) followed by packed top-to-bottom RGBA8 pixels. The receiver validates
the header before allocating and accepts images up to 3840 × 2160. It uses the
same live deck scheduler, effects, mixer, freeze, recording and output path as
camera input. A disconnect clears the live signal; a returning sender may
reconnect to the active listener.

This stream is a local video prototype. It carries no audio and does not use
NDI, OSC, a virtual camera or a project-persistent source assignment. Verify
frame age, sequence gaps and responsiveness at the chosen viewport size before
using it for a show.

1. Connect the capture card and its HDMI/SDI source. Install the manufacturer's
   macOS driver if required, and select the connector in its control utility.
2. Select a deck and open **Video input · camera / capture card**.
3. Click **Refresh**, then select the input by name.
4. Select a supported capture resolution and rate. 23.976, 29.97 and 59.94 use
   exact rational rates, not integer rounding. Match the source and card mode.
5. Start with **Automatic** pixel format, or choose NV12, UYVY 4:2:2,
   YUYV 4:2:2 or BGRA as appropriate for the device.
6. Click **Connect to Deck**. Check the live status and preview before enabling
   program output. Save the project to retain the deck's device and format.

**Manual input / custom size** accepts an AVFoundation name or index and custom
dimensions. A native device identity is stored for discovered inputs and
resolved at connection time. Names that could match multiple devices are
rejected; identical cards may require manual indices. Manual indices can change
after USB devices are reordered, so recheck them before a show.

Discovery uses native AVFoundation rather than FFmpeg's generic device-list
API. Opening and decoding use FFmpeg's
[AVFoundation input backend](https://ffmpeg.org/ffmpeg-devices.html#avfoundation).
Enumerating devices does not start video capture:

```sh
cargo run -p virtual-media --example list_video_inputs
```

If the card is missing, check power, cabling, driver installation and whether
macOS exposes it as a camera/video input. Grant camera permission to the app or
Terminal when running from Cargo. A card available only through a proprietary
SDK needs a separate adapter; a model-specific adapter is not included here.
Unsupported resolution/rate or missing signal is reported as an input error.
Reconnect using a supported mode; VIRTUAL does not automatically change the
card's HDMI/SDI connector or deinterlace an incoming signal.

Physical capture-card validation remains necessary: verify signal loss,
unplug/replug, fractional-rate pacing, source replacement, application exit and
saved-project restoration on the actual device. Existing synthetic capture
tests cannot establish hardware compatibility.
