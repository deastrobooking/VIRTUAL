# AudVis bridge and operator workspaces

The operator editor has three session-only views: **Performance**, **Audio**,
and **Setup / Connections**. Performance opens by default. Show Mode always
returns to Performance. The master opacity, blackout, freeze and A/B crossfader
stay above the scrolling editor in all views. Existing controller targets and
project formats are unchanged.

Audio analysis and mapping live in Audio; project, device, output and OSC setup
live in Setup / Connections. This is the first workspace pass, not a dockable
panel layout or an independent preview bus.

## Connect Blender AudVis

Use the AudVis fork's **VIRTUAL Bridge** sidebar panel:

1. In Blender, click Connect / Reconnect (input 9000, feedback 9001).
2. In VIRTUAL's Setup / Connections view, open OSC input, listen on
   `127.0.0.1:9000`, set feedback to `127.0.0.1:9001`, then choose Send feedback.
   Toggle feedback off/on if it was already enabled to send a fresh snapshot.
3. Blender should report Receiving VIRTUAL feedback. Use its Send buttons for
   BPM, crossfader and master opacity, explicit On/Off buttons for blackout and
   freeze, or buttons 1–8 for scene launch requests.
4. Assign AudVis parameter snapshots to those eight cues in Blender. Scene
   launch-request feedback recalls the corresponding snapshot on the connected
   Blender scene. Empty assignments do nothing.

Use Immediate quantization for this initial bridge. Existing scene feedback
acknowledges the request; it does not identify the eventual quantized playback
boundary. BPM control alone does not synchronize Blender's beat phase. UDP
feedback is best-effort, with no exactly-once guarantee or heartbeat.

The bridge is localhost-only and session-only. Port settings and snapshot
assignments persist in the `.blend`; loading a file requires reconnecting.

The OSC bridge carries control messages only. VIRTUAL's NDI implementation
sends the finished program and is separate from Blender video input. The direct
video-source prototype and its AudVis Output Image integration plan are covered
in [AudVis video input for VIRTUAL](AUDVIS_VIDEO_INPUT.md). Execution-time cue
messages and shared macro identities remain later coordination work.

For the direct video-source proposal and AudVis fork study, see
[AudVis video input for VIRTUAL](AUDVIS_VIDEO_INPUT.md). The proposed route
streams the image selected by AudVis's existing **Output Image** selector to a
VIRTUAL deck over a localhost-only connection, without OBS.
