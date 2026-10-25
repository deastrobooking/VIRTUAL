# VIRTUAL effects suite feature plan

Planning review: October 25, 2026. Status: staged implementation in progress.

The first batch now implements selection-safe clip interaction, gesture undo in
the timeline and popup clip editor, pinned deck UNIT windows, saved workspaces,
moving source cue, bounded browser posters and initial processed-program capture.
Program capture writes silent raw RGBA MOV; compressed codecs and audio remain
future work. Automated click, history, workspace, cue and GPU/movie checks cover
these slices. Native desktop layout inspection and physical show-load/failure
certification remain pending. M3–M6 remain proposed. See
[workspace and recording](WORKSPACE_RECORDING.md).

The product direction is a compact live video instrument with the editing depth
of a video effects suite. Operators should be able to perform a mix, keep its
editable control history, assemble a better cut, and render the result through
the same native processing engine. Expandable modular UNIT editors remain a
first-class interface throughout that workflow.

The immediate priorities are recorded-performance editing and video arrangement
and rendering, alongside a denser, more predictable operator workspace.

## Starting review findings

These findings describe the starting point before this batch. The status above
records the changes delivered since that review; later milestones remain proposed.

| Area | Working at review start | Gap that determined the next work |
|---|---|---|
| Navigation | F1–F10 section rail, folding sections, floating browser on narrow windows | Remappable shortcuts, saved workspaces, and consistent selection context across docked and floating editors |
| Clips | Shared Session/Launcher/Layers slots, snapshot clipboard, popup trim/automation editor | Clicking a populated cell launches it; opening the editor by double-click can also launch. Selection and performance actions need separate hit targets |
| Video edits | Sequential cuts, trim/split/reorder/duplicate, project persistence | No timeline playback, isolated moving preview, overlapping tracks, or transitions |
| Undo | Bounded timeline snapshots | Snapshots are taken per changed UI frame; a continuous trim drag can consume many undo steps. Clip and scene edits need the same transaction model |
| Recorded takes | Control journals, checkpoint recovery, trimmed journal assembly | Restoring state at a cursor is not continuous playback. Media position, automation, modulation and temporal FX must be evaluated through time |
| Export | Silent 30 fps H.264 source-cut rendering | Source export bypasses the native compositor and FX. It is not a processed-performance render |
| Native FX | Built-in deck groups, deck package slot, master slots, modulation and generator editors | A common UNIT presentation can reuse those controls now; arbitrary ordered racks need additional renderer execution and resource budgeting |

Relevant implementation boundaries are `ui/clips.rs`, `ui/navigation.rs`,
`ui/timeline.rs`, `timeline_export.rs`, `recovery.rs`, and `playback.rs` in
`crates/virtual-app/src`; `virtual-session/src/edit.rs` and `journal.rs`;
and `virtual-render/src/readback.rs`.

## Product and interaction model

Use three saved workspaces over one project:

| Workspace | Primary surface | Main task |
|---|---|---|
| Perform | Dense clip grid, layer controls, program monitor, compact FX rack | Launch and manipulate the live composition |
| Edit | Sequence tracks, source/program monitors, contextual inspector | Arrange footage and recorded performances |
| Build | UNIT rack, expanded editors, generator and modulation controls | Construct reusable looks and processing chains |

The section rail remains available in each workspace. The inspector always names
its target: composition, layer, clip instance, UNIT, or take segment. Floating
editors can follow selection or pin to a named target; changing selection must
not silently retarget a pinned editor. Icons have tooltips and visible shortcut
hints, and panel widths and text density are saved per workspace.

In preparation and editing, clicking a clip body selects/cues it; the launch
triangle performs it. Double-click opens the editor. Perform can offer an
explicit launch-on-cell mode. Cue and editor actions must never reach PROGRAM
as a side effect. Copy/paste follows the active editing surface, and paste
special exposes media reference, trim, automation, or FX rack separately.

## Shared project model

Establish stable identities and ownership before expanding the timeline:

| Object | Owns |
|---|---|
| Media asset | Source identity, path/relink information and media metadata |
| Clip instance | Asset reference, trim, speed, loop/launch behavior, clip automation and optional clip FX |
| UNIT instance | Stable parameter IDs, effect/generator identity, settings, bypass/wet, preset and modulation references |
| Composition revision | Layer order, clip assignments, composition settings, UNIT state and dependency versions |
| Take | Recorded controls tied to a composition revision, tempo/input history and checkpoints |
| Sequence | Stable segment IDs, source references, placement, source-time mapping, tracks and transitions |
| Render job | Frozen composition/sequence revision, output settings, dependencies and job status |

A duplicate is an independent instance that can share the source asset. Referenced
edits update together only when explicitly linked. Keep source assets, clip
instances and rendered results distinct in the browser.

Use integer/rational media time and explicit frame rates in the sequence model;
convert to decimal seconds for display. Support rates such as 30000/1001 without
assuming every timeline second contains 30 frames. Keep beat positions tied to a
recorded tempo map. A segment has a source range and a destination range, with
explicit speed and half-open interval rules at cuts.

Follow `DEVELOPMENT.md` for format changes: additive fields get defaults;
changes to existing field meaning or shape require a project version bump and
explicit rejection of unsupported versions. This plan does not introduce a
migration policy.

## Milestones and completion checks

### M1 — Make preparation and performance predictable

Deliver separate selection/launch targets, compact inspector tabs, common UNIT
headers, pin/follow selection, workspace layouts and a searchable shortcut map.
Start with existing FX capabilities. Add gesture-based edit transactions shared
by timeline, clip, automation and scene edits, plus a small undo-history view.
Add live source cue playback and cached browser thumbnails with bounded workers.

Done when: opening or copying a clip leaves program playback unchanged; one drag
is one undo; a pinned editor stays on its target; the main controls remain usable
at 1280×800 and 1920×1080; layouts survive reopening; Show Mode keeps its locks.
Validate these behaviors in the native window as well as through automated checks.

### M2 — Record the finished live program

Add Program Record with codec/resolution, elapsed time, disk estimate, failure
status and an option to add the result to the media browser. Capture the clean
composition after master FX and before presentation/projection using the existing
program target/readback seam. Share an encoder interface with later offline jobs.
Keep source-camera recording and processed-program recording individually named.
Start with video-only capture; add synchronized audio after its recording clock
and device behavior are specified.

Done when: a recording includes visible deck/master FX, excludes operator UI,
finalizes cleanly, reports dropped frames, and preserves elapsed timing under
encoder backpressure. A full disk or encoder failure leaves the live show running.

This is the quickest path to a usable movie of the current modular FX performance.

### M3 — Play and scrub editable sequences safely

Introduce the stable sequence model and a transport that evaluates source time,
trim/speed/loops, recorded controls, tempo, generators and automation against one
explicit clock. Give Edit its own media players and render state so scrubbing
cannot seek live decks. Preload both sides of a cut. Start with the existing
four-layer renderer and supported effects; arbitrary tracks are a later expansion.

For take playback, bind history to its source composition revision and assets.
Capture the audio-derived modulation/input data required to reproduce a take.
Live cameras and other external inputs need recorded media or an explicit frozen
replacement before an offline replay can be complete.

Done when: playing, pausing, seeking and looping an edited sequence work without
changing PROGRAM; cuts select the right source frame; recorded tempo and control
changes occur at their scheduled times; repeated seeks produce the same state.
The first acceptance fixture combines two edited take segments and two video cuts.

### M4 — Render the actual composition

Drive the same compositor, native effects and generator evaluation from exact
output timestamps. Decode each required frame and apply controls before rendering.
Offline rendering can wait for decode/GPU/encoding; every requested frame must be
accounted for. Live capture retains its bounded, non-blocking behavior.

Specify temporal FX behavior before claiming matching renders: reset at a cut,
continue within a sequence, or warm up from an earlier state. Persist that choice.
Record random seeds and required modulation inputs. Compare image results within
an explicit tolerance on supported backends; do not promise identical pixels
across all GPUs.

Deliver export range, rational FPS, resolution, tested codecs, progress, cancel,
and output validation. Freeze dependencies when the job starts so live edits do
not change an in-progress export. Include the required encoder in the supported
installation, or present an actionable dependency check before starting a job.

Done when: a short fixture containing a generator, deck package, blend, master
feedback and recorded modulation exports with correct frame count/timing and
matches the Edit preview under the documented history policy. Export failure or
cancellation does not damage the project or an existing file.

### M5 — Make modular looks reusable

Expand the common UNIT interface into bounded ordered racks: drag/reorder,
bypass/wet, reset, compare, macros, rack presets, copy/paste and independent
windows. Define clip → layer → composition ownership and automation precedence.
An FX pasted to a clip must not silently edit its layer's FX.

Implement executable rack ordering and texture/history budgets before exposing
additional slots. Reuse the shader-system plan for package graphs and capability
checks. UNIT describes VIRTUAL's native modular interface; third-party VST hosting
would be a separate feature proposal.

Done when: a rack preset works in Perform and Edit and survives save/reopen;
MIDI/automation addresses survive reorder; illegal or over-budget racks keep the
last valid render plan; preview and export use the same rack evaluation.

### M6 — Add deeper editing and cross-session assembly

Add overlapping video/adjustment tracks, snapping, ripple/roll trim, crossfades,
track FX, groups, and nested compositions in tested increments. Start transitions
with a crossfade; expand transitions after overlap rendering is reliable. Add
waveforms, audio placement and sample-accurate audio export on a defined audio
clock. Then add cross-project import with a preview of assets, looks, take
references and naming conflicts, plus collection/relink of dependencies.

Done when: a merged project resolves its references independently of source slot
numbers, has predictable duration and transition behavior, and reopens/renders
with its imported looks intact. Imports and multi-selection edits are undoable.

## Recommended next implementation batch

The next batch should finish M1's interaction foundation and specify M2's encoder
contract. Continuous replay and arbitrary racks depend on larger model changes.

1. Separate clip selection/cue from launch and test double-click/editor behavior.
2. Make trim and automation gestures single undoable transactions.
3. Introduce stable selection targets; add pin/follow to the deck UNIT editor.
4. Save Perform/Edit/Build layouts and make the rail reflect the active inspector.
5. Add isolated moving cue preview, then background browser thumbnails.
6. Specify and implement bounded final-program recording, including timing and
   failure behavior, using the existing program readback.

The end-to-end release target after these batches is: perform → record editable
history → arrange takes and footage → preview through native UNITs → render the
processed result. M2 supplies a useful processed recording before that full loop
is finished. More effect packs, unlimited tracks and external plugin formats can
follow without delaying this core workflow.
