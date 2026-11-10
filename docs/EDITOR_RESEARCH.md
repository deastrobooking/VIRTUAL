# Editing and performance workspace research

Research date: 2026-11-10. This is a workflow comparison, not a claim of feature parity.

## References and decisions

| Workflow | Reference behavior | VIRTUAL decision |
|---|---|---|
| Source vs program | Premiere's Source Monitor prepares individual clips; Program Monitor shows the assembled sequence | Separate A/B source preparation from the actual GPU Program monitor; label source posters honestly until independent preview decoders exist |
| Timeline tools | Premiere exposes selection, ripple/rolling trim, razor, slip, slide and pen tools | First ship frame-based selection, non-ripple move/trim, split and undo; add ripple and roll after edit invariants are established |
| Live mixing | Resolume composites layers into A/B buses before crossfading | Retain the existing A/B engine and controls in the Performance workspace; do not reinterpret a bus as an independent clip |
| Recording | Resolume records composition/layer/group/crossfader or advanced output without stopping mixing | Treat live recording as a separate capture job from offline sequence rendering |
| Export | Premiere exposes codec-dependent resolution, frame-rate and other video settings | Keep source-edit interchange separate from movie rendering; never imply an edit export includes WGSL effects |
| Planned show | Standard nonlinear editors use sequence time, source In, duration, tracks and markers | Store integer frames and an explicit rate in project v7; insert source ranges, reject same-track overlap and preserve source continuity at a split |

Primary sources:

- [Adobe: Source and Program Monitor](https://helpx.adobe.com/premiere/desktop/get-started/source-and-program-monitor-adjustments/about-source-monitor-and-program-monitor.html)
- [Adobe: editing tools](https://helpx.adobe.com/premiere/desktop/get-started/tour-the-workspace/tools-panel-and-options-panel.html)
- [Adobe: basic video export settings](https://helpx.adobe.com/premiere/desktop/render-and-export/export-files/basic-video-settings.html)
- [Resolume: composition and crossfader](https://resolume.com/support/en/composition)
- [Resolume: recording](https://resolume.com/support/en/recording)
- [OpenTimelineIO: file format](https://opentimelineio.readthedocs.io/en/v0.15/tutorials/otio-file-format-specification.html)
- [OpenTimelineIO: serialized schemas](https://opentimelineio.readthedocs.io/en/v0.18.0/tutorials/otio-serialized-schema.html)

## Implemented workspace pass

The application now fills its operator window rather than placing its editor
in a floating window over the output. Performance retains the clip-launch grid
and deck inspectors. Session / Edit presents A/B source preparation, a live
Program monitor, frame timecode, four video lanes, a ruler/playhead, source bin,
clip inspector, zoom, whole-second move snapping, markers, split/delete and
32-step arrangement undo/redo. The source bin selects without launching.

A/B preparation cards show the selected clip poster for a layer on each bus,
source In/Out, explicit launch and layer transport, and Cut to A/B. The Program
monitor samples the finished composition on the GPU with an unorm view of its
sRGB storage, preserving egui's gamma convention. Output mapping and calibration
cards are presentation-only and do not appear in this composition monitor.

## Planned-show operation

1. Import file-backed clips in Performance; switch to Session / Edit.
2. Choose an empty sequence's frame rate (24, 25, 30, 50 or 60). The rate is fixed
   after insertion so an edit cannot silently be retimed.
3. Select a clip in the Session source bin. Set its source In/Out in the A/B
   card, set the ruler playhead, and insert. Its deck determines V1–V4.
4. Click to select blocks, drag horizontally to move, or edit start, duration
   and source In in the inspector. Clips cannot overlap on the same track.
   Split at playhead preserves the source offset of the right-hand piece.
5. Add cue markers. Save with the normal project controls. Version 7 stores
   the arrangement; v1–v6 load with an empty arrangement. Older app versions
   should reject v7 rather than silently discard the edit.
6. Play runs the planning clock. **Drive live Program** explicitly arms deck
   control; arming immediately auditions the current frame, even while paused.
   Keep external output disabled when editing away from the stage.
7. Armed playback uses existing decoder workers and seeks, drives sources at
   1×, bypasses track gaps, pauses at the end and honors the current mixer,
   effects, bus assignments and master controls. Missing/live/too-short sources
   are bypassed with a status message. Release timeline restores the previous
   bypass flags and pauses the layers; the last launched sources remain loaded.

Session shortcuts: Space play/pause, Home start, Left/Right one frame,
Delete/Backspace remove the selected timeline block, Cmd/Ctrl+Z undo and
Cmd/Ctrl+Shift+Z redo. Text fields retain keyboard ownership. Existing emergency
blackout remains available. Timeline editing is disabled while armed or playing.
The pinned Release timeline control stays visible when changing workspaces or
entering Show Mode.

## Export contract

**Export timeline…** writes an atomic `.otio` file on a worker. It carries four
video tracks, source media URLs, trims, gaps, clip names and cue markers. Every
clip must reference a resolved file source. URLs escape spaces, Unicode and
reserved characters. External applications need an OTIO-compatible importer or
adapter; direct native Premiere/Final Cut project import is not claimed.

It does not bake VIRTUAL color, transforms, WGSL packages, bus fades or live
modulation. It does not contain audio. A timeline file is not a rendered movie.
Source edits are independent of a particular live mix's output image.

Sequence blocks reference deck/slot locations in the saved clip bank. Replacing
or moving slot media changes the referenced source; the arrangement is not yet a
standalone media pool with stable asset IDs. Playback is real-time and decoder
availability-dependent; it is not certified frame-accurate or a master-clock
replacement for SMPTE/MIDI synchronization. Structural timeline edits use project
save/autosave, rather than journal undo/replay.

## Next stages toward a full editor

1. Stable asset IDs and a proper media pool; isolated source audition and
   independent live A/B bus monitors; dockable/resizable panel boundaries.
2. Track locks/mutes, vertical moves, ripple/roll/slip tools, editable markers,
   range selection and multi-select; fractional frame rates and drop-frame TC.
3. Audio tracks, waveform proxies, sound output and audio/video linked edits.
4. Per-instance effects and keyframes, crossfader/parameter automation tracks,
   transitions, nested sequences and explicit Blender cue tracks.
5. Offline renderer with a fixed frame clock, deterministic decoder readiness,
   temporal-effect warm-up, audio mux, progress/cancel and H.264/ProRes/HAP
   presets. Reuse the exact composition engine rather than approximating WGSL
   output in an external FFmpeg filter chain.
6. Program recording as its own bounded real-time worker. Test recorded and
   offline exports against the live Program at known frames before declaring
   export parity or show-machine certification.

## Validation and command-line export

The export can also be produced without the GUI:

```sh
cargo run -p virtual-io --example export_sequence -- show.virtual show.otio
```

Validation for this pass includes the workspace test suite (including headless
GPU readback), strict Clippy, formatting, sequence-boundary and edit-history
checks, v1–v7 project fixtures, and parsing an exported three-clip/four-track
arrangement with the official OpenTimelineIO 0.18.1 Python library. The parser
check verifies source trims, gaps, duration and markers.

The native UI inspection tool timed out on the running review build; drawn
layout and mouse interaction therefore remain unverified. Automated checks do
not replace a show-machine rehearsal or certify exact cue timing.
