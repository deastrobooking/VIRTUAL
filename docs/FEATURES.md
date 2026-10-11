# Feature status

This matrix reflects the current source tree, not the aspirational MVP notes.

## Working

| Area | Current implementation |
|---|---|
| Decks and clips | Four decks, eight persistent slots per deck and 32 independently assignable scene rows in A–D banks |
| Import | File/folder drag/drop, bounded recursive scanning, deterministic slot assignment, probing, thumbnails, first-frame launch previews, movie metadata and PNG/JPEG stills |
| Codecs | Direct HAP family path plus FFmpeg fallback for conventional codecs |
| Cameras | AVFoundation discovery/manual ID, requested size/FPS and any-deck assignment |
| Generators | Fourteen recursive 2D/3D line-geometry sources plus a project-saved Geometry Lab with typed node graphs, safe expression calculator, a GeoGebra-style 2D/3D graphing calculator (functions, polar, parametric, implicit, inequalities, sliders, surfaces, vector and slope fields, calculus overlays, keypad), parametric curves/surfaces, transforms, twists, repeats, noise and merge/output; live time/audio variables, bounded evaluation and per-layer project persistence — see [generators](GENERATORS.md) and [Geometry Lab](GEOMETRY_LAB.md) |
| Playback | Play, pause, restart, freeze, seek, loop/one-shot, 0.25–4× speed and per-slot In/Out ranges |
| Timing | Exact timestamps, bounded keyframe indexes, indexed conventional-codec reopen, bounded schedulers and generation-safe stale-frame rejection |
| Triggering | Immediate, next-beat and next-bar clip/scene launches, per-slot restart/resume and BPM-relative beat duration; scenes independently launch, stop or leave unchanged each deck, and can capture selected clip slots |
| Mixing | Independent A/B composites, 35 blend modes, Solo/Bypass, transforms, crop/source modes, linear/equal-power crossfade, master opacity and blackout |
| Output | Offscreen preset/custom program target, clean second window, display selection, aspect preservation, enable/fullscreen, calibration overlays, surface-health diagnostics, persisted single-quad projection warp with source crop, draggable/resizable blackout masks, per-edge feathering, gamma and black-floor calibration, optional NDI sender |
| Built-in deck effects | Fused Geometry UV prepass plus Color + Levels and Stylize + Key groups; only Color/Stylize change relative pixel order. Includes grading, mirror, fractal fold (plus spiral, Kali and Koch fold variants), neon, jitter, edges, bit reduction, black light, pixelate, luma key and bloom |
| Master effects and packages | Two reorderable slots with separable blur, persistent feedback/trails or registry-discovered one/two-pass WGSL packages; grouped controls, one-click looks, stable parameter identity, last-known-good reload and Recursive 2D, Fractal Volume 3D, Hyper Recursion 4D+ and Space Travel packages |
| Modulation | Three LFOs and eight bipolar routes per deck across 18 continuous effect destinations |
| OSC | Bounded OSC 1.0 UDP input/output, nested bundles, NTP-timetag scheduling, initial state snapshots, live health counters and origin-aware routes for mixer, decks, clips, scenes, tempo and output |
| Musical control | Manual BPM, Tap, half/double, beat/bar phase and synchronized LFO divisions |
| Audio modulation | Native input capture with interface channel selection, bounded queue, RMS/FFT bands, 8-band spectrum EQ with per-band gain and dB scale, transient, adaptive normalization, live meters, band-to-control mappings (continuous, trigger, gate) and thirteen audio plus beat/bar matrix sources |
| Persistence | Atomic background save with persistent pending/failure status, save-before-open, autosave, recovery, asynchronous restore, automatic v1–v9-to-v10 loading, saved scene assignments, show-folder-relative media paths, stable project/take identity, deterministic seeds, active graph metadata and missing-media relinking |
| Operator safety | Selected-deck primary editor, direct deck-row targeting, Show Mode performance lock enforced at UI and action level, fixed (non-scrolling) emergency toolbar, preflight rail covering devices, effects and saves, text-safe keyboard shortcuts and button/keyboard clip deletion |
| Diagnostics | FPS, decoder drop/repeat/late counters, RGBA allocation/reuse/live/discard telemetry, output surface state, presentation skips/recovery and display-topology changes |

## Partial foundations

| Area | Present | Still required |
|---|---|---|
| MIDI | Native device discovery/input, reconnect, learn UI, activity/drop diagnostics, absolute/momentary/toggle/relative modes, editable ranges, soft takeover, broad target routing, persistence, and beat-clock sync in and out with transport, Song Position and timing telemetry | Controller feedback output (LED/motorised state) and physical-controller soak validation |
| Audio analysis | Gain, noise floor, attack/release, normalization, band and transient analysis | Show-device disconnect/soak validation |
| Effects system | Three persisted built-in deck groups, two persisted package-capable master slots, one stateless package slot per deck, common bypass/dry-wet/reset, stable deck-package modulation/MIDI/OSC identities, visible branch-culling counters, per-deck GPU pass timing, five factory deck presets, bounded ping-pong blur, reset-safe feedback history, named persistence, generation-safe last-known-good hot reload, three master LFOs, eight custom-parameter routes, generated MIDI learn, an atomic one/two-pass master sequence and optional fixed per-slot custom history | Versioned shared WGSL modules, typed N-pass fragment graph, capability-gated HDR intermediates and budgeted compute/state resources |
| Output routing | Shared operator/output presentation, connected-display selection, persisted descriptor, topology polling and surface recovery diagnostics | Stronger identity across display topology changes and show-machine soak testing |
| Projection mapping | Persisted corner-pin homography, source crop, four interactive rectangular masks, per-edge feathering, gamma and black-floor calibration applied to the external program output | Physical projector calibration rehearsal, multi-projector overlap solving, polygon/mesh warps, and 3D model-based mapping |
| NDI output | Optional runtime-loaded sender with bounded frame handoff and live status | Receiver/network/platform certification; NDI runtime must be installed separately |
| Performance | Bounded workers, reusable CPU RGBA frame leases, 29.5 MB maximum first-frame cache, capped keyframe indexes, deterministic decoder faults and accelerated/extended soak coverage | Physical-media and show-machine soak certification |
| Typed graph runtime | Versioned typed node contracts, six rate domains, validation, explicit feedback rules, deterministic scheduling, immutable plans, resource lifetime reuse and GPU/memory budgets; the 11-node four-deck graph lowers to authoritative fused-composite, master-effect and output stages | Add executors beyond the compatibility graph, complete color/resolution inference and independently execute nodes that cannot be fused |
| Live transactions | Isolated shadow graphs, all-or-nothing preparation, last-known-good retention and frame/beat/bar/timecode commit scheduling | Graph editor/preview UI, prewarming and live operator commit controls |
| Performance replay | Serializable show commands, session state, checkpoints, deterministic replay and versioned JSONL journals; operators can start named takes, add labeled timeline markers, scrub full journal history into named branches, safely manage take metadata, export/archive unique bundle copies and edit scoped deterministic seeds | Add marker editing plus portable project/media manifests |
| Clip automation | Every clip slot owns a saved 16-beat automation loop by default; Draw/Select curve editor with linear, smooth, step and exponential segments; up to eight bounded lanes and 128 keyframes per lane; tempo-clock playback over video, camera, or generator sources; automation-only slots and scene triggering | Reverse automation, clip-specific target takeover/restore, more editing gestures and automation recording |
| Generator layers | Up to four algorithmic generator layers per deck, independent source settings and 2D placement/blend, full-stack project save/load, one shared 200k segment cap per deck | Layer-specific MIDI/OSC/automation targets, solo/reorder/duplicate, and GPU compositing to reach 1080p/60 |
| Low-light crowd FX | Camera/capture-card input, deck edge detection and grading, Thermal Contours, master feedback/repeater, plus Crowd Night Vision's four camera-focused looks | Real-footage tuning and bounded deck-local motion trails |

The current shader/package boundary and the phased per-deck upgrade are detailed
in [Shader system](SHADER_SYSTEM.md). Stateless algorithmic packages now run on
either a deck or master through the implemented precomposition seam and
target-specific `deck-v1` GPU contract.

Proposed Pure Data and TouchDesigner-inspired feature designs are in
[Feature designs](FEATURE_DESIGNS.md). They cover control patching, automation
clips, multi-generator deck layers, procedural texture sources and a
constrained visual render-graph editor.

## Not implemented

- Recent-project list
- OSC route expansion/discovery, Syphon/Spout
- Developer ID signing, notarization and a pre-macOS-26 FFmpeg build (`--portable` bundles Homebrew FFmpeg)
- Graph editor, Score view, Spatial view and compiled GPU node execution
