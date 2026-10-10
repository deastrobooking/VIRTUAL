# Feature designs inspired by Pure Data and TouchDesigner

These designs adapt useful ideas from Pure Data's control-signal patches and
TouchDesigner's real-time operators to VIRTUAL's four-deck performance
workflow. They extend existing systems; they do not make VIRTUAL a general
purpose node IDE. Four decks remain the performance model. Multiple
simultaneous program outputs are outside this work.

## What VIRTUAL already has

The original feature notes describe several capabilities that are now present:

- Four-deck compositing, clip playback, transforms, crop, blend modes and
  output presentation.
- Live camera input, NDI sending, audio analysis, MIDI/OSC control and
  projection mapping.
- Three LFOs and per-deck modulation routes, including audio bands, transient,
  beat and bar signals.
- Recursive geometry generators, master feedback/trails, and a typed graph
  compiler with validation, scheduling, resource budgets and shadow
  transactions.

The strongest remaining opportunities are easier visual control authoring,
more useful non-file visual sources, and a visual way to build and understand
processing graphs. A final-program recorder is also a practical output gap, but
should be designed as a separate media/reliability slice.

## 1. Control patcher

**Inspiration:** Pure Data's patch cords and TouchDesigner's CHOP control
channels.

**User outcome:** Build a small signal flow such as audio bass → smooth → scale
→ deck effect amount, without hand-building a long set of matrix routes.

**First version:** Provide a separate Control Patch view with typed nodes for
audio bands, transient, beat/bar phase, LFO, envelope follower, math/range,
clamp, slew/smoothing, threshold/gate, quantize/sample-and-hold and supported
performance targets. Include reusable named patches, live value meters,
enable/bypass, and per-output range/curve controls. Targeting should reuse the
existing stable control-target gateway so MIDI, OSC, UI and patch signals share
validation and recording behavior.

**Runtime rules:** Evaluate control and event nodes on a bounded control tick;
never run audio analysis or UI work in the audio callback. Reject unbounded
cycles; permit feedback only through an explicit one-tick delay node. Keep
video textures and GPU work out of this first patcher. Validate target types,
units, ranges and rate adapters before activation, then keep the last valid
patch running if an edit fails.

**Persistence and migration:** Store a versioned patch graph in the project.
Existing projects load with an empty patch set and preserve all current matrix
routes. Give nodes stable IDs so renaming or moving them does not break saved
connections or session journals.

**Acceptance:** A user can connect an audio band to an effect target, see live
values, disable the patch without changing its saved wiring, reload the show,
and reproduce the same bounded behavior. Removing an audio device resolves its
source safely to zero. Invalid edits never replace the active patch.

**Recommendation:** Keep this as the broader follow-on to clip automation.
First define target ownership and override behavior for clip lanes, then reuse
those same rules in freeform control patches. Most runtime pieces already
exist; the main work is the patch UI, stable schema and safe command-gateway
integration.

## 2. Clip rows and automation clips

**Inspiration:** Ableton-style scene rows and clip envelopes, plus Pure Data's
time-varying control signals.

**Existing behavior:** The eight scene controls already launch the same-numbered
slot on all four decks, quantized immediately, to the next beat or to the next
bar. Treat that as the first row-launch implementation. The row design extends
it with explicit cell types, row stop/hold behavior and automation-only cells.

**User outcome:** Launch a complete row across the four decks. A cell can be a
video clip with its own automation, an automation-only clip that controls an
existing camera/generator/live source on that deck, an empty/no-op cell, or an
explicit stop cell. Empty cells leave their deck untouched. This lets a row
change clips on some decks while starting automation on a live-input or
generator deck without replacing that visual source.

### Clip and lane model

- Every video clip can own zero or more automation lanes. Launching the video
  starts its media and its attached envelopes from the same clip position.
- An automation-only slot has no media path. It has a label, musical loop
  length and automation lanes; launching it leaves that deck's current visual
  source and media transport alone.
- Automation may target supported continuous features on its own deck or the
  shared mixer/master, including opacity, crossfader, transforms, effect
  parameters, LFO amounts and generator controls. Targets use stable IDs and
  the same range validation and command/session gateway as MIDI, OSC and the
  control patcher.
- A loop length is 1–64 bars, saved as bars/beats rather than seconds. The loop
  follows the show clock, so a tempo change changes its wall-clock duration
  without changing its musical length. MIDI clock Start resets phase,
  Continue resumes, Stop holds the current value, and Song Position Pointer
  jumps the automation playhead using the existing musical clock position.
- Automation clips loop by default. Provide a one-shot/hold option for cues
  that should reach the end and remain there. Row launch uses the existing
  Immediate / Next beat / Next bar quantization; add a per-row launch quantize
  override only if the operator workflow proves it necessary.

### Automation editor

Each lane gets a time ruler in bars and beats, a value ruler in the target's
meaningful range, and **Select** and **Draw** tools. Select supports point
selection, box selection, move, duplicate and delete. Draw supports freehand
curves (simplified to a bounded point count), straight ramps, eased curves and
square/step patterns. Grid snap can be set in beat subdivisions or disabled;
editing must support undo and preserve endpoints when a loop length changes.
Show live playhead/value and allow audition without altering the saved source
or external-input assignment.

Use the existing linear, smooth, step and exponential segment evaluator where
it matches the shape. Add a bounded cubic curve representation only where
smooth drawn curves cannot be represented well by those segment types. Raise
the current 128-keyframe cap only after profiling the editor and runtime; a
freehand gesture must be simplified and capped before persistence.

### Ownership and conflict rules

Automation on one deck can run alongside automation on other decks. If two
active clips write the same target, show the conflict and resolve it
deterministically: the most recently launched enabled lane owns that target;
when it stops, the prior active lane resumes. Manual input temporarily
overrides automation for that target until the operator releases the control or
relaunches the controlling clip. Bypass disables writes without deleting the
clip. These rules need a visible target-ownership indicator so the performer
can tell why a knob is moving.

### Reverse playback

Both video clips and automation clips need a persisted forward/reverse
direction. Reversing an automation clip starts at its loop end and runs toward
its start, still locked to the beat clock; reversing a video clip plays frames
backward through the selected In/Out range. Direction changes while playing
should take effect at the next frame without a stop/relaunch.

Video reverse needs a bounded decoder strategy: seek to preceding keyframes,
decode a bounded GOP/window on the worker, and present cached frames in reverse
timestamp order. Reuse the existing keyframe index and HAP path where possible,
but enforce a memory cap and report codecs/sources that cannot reverse. Do not
reverse cameras or silently fall back to forward playback. The automation
playhead can reverse independently of the deck's video transport when the
active cell is automation-only.

### Persistence and acceptance

Store cell kind, automation loop length, direction, lanes, curve data, launch
quantization and row stop/hold intent in a versioned project schema. Existing
projects migrate with current video clips unchanged and no automation lanes.
Automation-only cells must survive save/reopen without a placeholder media
file. Clip moves and swaps carry their automation and direction with the cell.

Acceptance includes: row launch with mixed video, empty, stop and automation-only
cells; 1-bar and 64-bar loops at internal and MIDI-clock tempo; MIDI Start,
Continue, Stop and Song Position jumps; reverse automation phase; supported
video reverse with bounded memory; target conflict/override behavior; and
save/reload/replay determinism. A missing or unsupported reverse decoder must
be surfaced before launch, leaving the current output intact.

## 3. Procedural texture sources

**Inspiration:** TouchDesigner TOP generators such as Ramp, Noise and Text.

**User outcome:** Create useful visuals without importing a movie or opening a
shader editor, then mix them on any of the four existing decks.

**First version:** Add a compact set of GPU-backed sources: solid/color bars,
linear and radial ramps, deterministic 2D noise, and text/title. Give each
source a small parameter set (colors, scale, seed, speed, font/size, alignment)
and reuse the existing deck transform, effects, opacity, blend, modulation,
persistence and MIDI/OSC targets. Noise with a saved seed should reproduce
after project reload. Text should handle Unicode and missing-font fallback.

**Runtime rules:** Render into the deck source at the composition's working
extent, with bounded shader cost and no CPU per-pixel generation. Time-varying
sources use the existing show clock and deterministic seed state. Text may use
a bounded glyph atlas/cache; cache growth and eviction need explicit limits.

**Acceptance:** Each source can be loaded, mixed, modulated, saved and restored
as a normal deck source. A static source does not allocate or redraw every
frame when its parameters are unchanged. Noise remains deterministic when
paused and while replaying a recorded take.

**Boundary:** Keep the first release to 2D source generators. Particle systems,
3D scenes, external textures and arbitrary user code remain separate designs.

## Generator layer stack on one deck

**Inspiration:** TouchDesigner compositing several procedural TOPs inside one
network, while retaining VIRTUAL's generator gallery and deck workflow.

**User outcome:** Put several algorithmic generators on one track, place and
size each independently, edit each generator's own shape and motion controls,
then feed their combined image through the existing deck FX, A/B mixer and
program output as one source.

### Layer model and controls

Allow up to four generator layers per deck in the first version. Each layer
has a stable ID, its own pattern and complete `GeneratorSettings`, enabled and
solo state, normalized X/Y position, uniform scale, rotation, opacity and an
internal blend mode. Provide Add, Duplicate, Delete, Reorder, Enable and Solo
controls in the generator window. Selecting a layer shows only that layer's
existing pattern, geometry, camera, color, audio and output controls. Position,
scale and rotation remain in a small always-visible layer transform section.

Start with Normal/Over, Add and Screen internal compositing. These blends apply
between generator layers only; existing deck blend mode and transforms still
apply once to the completed generator image. Existing deck effects also run
after the stack. The generator remains one deck source and does not consume
extra mixer tracks.

### Runtime and budget

Keep a single bounded generator worker per deck rather than starting one thread
per layer. The worker owns each layer's geometry and animation state and
composites one final RGBA frame for the existing frame scheduler. Share the
current 200,000-segment cap across the whole deck stack; allocate the cap
fairly across enabled layers. Use one shared output resolution and frame rate
for the stack. The current CPU prototype reuses one RGBA scratch frame and
shares the float accumulation buffer for layers without trails. Steady-state
1080p release measurements (`stack_bench` example, low geometry depth, M3 Pro)
are about 2.1, 6.1, 8.8 and 12 ms per frame for one through four layers, inside
the 16.7 ms 60 fps target. An earlier first pass measured 14.5–47.8 ms; that
was dominated by a per-pixel trigonometric CPU compositor and timed only cold
first frames. Profile at maximum depth, with opaque layers and live audio on
the target machine; GPU composition remains the next step if four opaque
layers at maximum depth miss the budget.

The 200k segment budget alone does not prove the 60 fps target: rebuilding
several geometries and transforming/compositing them can increase worker time.
Benchmark at 720p and 1080p with maximum-depth edits and live audio before
raising the cap or enabling every layer by default. Keep stale-frame dropping bounded and show worker time,
effective depth, total/visible segments and dropped frames in the deck status.

### Persistence and control mapping

Store a versioned generator-stack structure in each deck project. Migrate an
existing single `GeneratorProject` into one layer with the same pattern,
settings and output resolution; older projects must render identically after
loading. Stable layer IDs must survive reorder and duplication. Keep existing
deck-level generator MIDI/OSC mappings pointed at the first/primary layer so
old controller setups remain useful; add layer-scoped target IDs for the other
layers. Automation clips and the future control patcher can target a specific
layer's stable ID and parameter.

### Acceptance

- One deck can show four distinct generators with different position, scale,
  color and motion; moving or changing one does not change another.
- Internal blend order, enable, solo, duplicate and reorder behave predictably;
  the final stack still responds to the deck's existing transform, effects and
  A/B bus assignment.
- A saved legacy single-generator deck migrates to one stack layer; a new
  multi-layer stack saves, reloads and reproduces its settings and seeds.
- MIDI/OSC and clip automation address an individual layer without changing
  mappings or automation for another layer.
- The aggregate segment and scratch-memory caps are enforced. Four-layer
  1080p output meets the measured 60 fps target on the show machine without
  render-thread waits or unbounded queue growth.

## 5. Visual render-graph editor

**Inspiration:** TouchDesigner's TOP network and operator inspection.

**User outcome:** Understand and safely customize the processing path instead
of treating the graph as an internal architecture detail.

**First version:** Start with an inspectable graph for the existing supported
topology, with visible source, deck effects, four-deck composite, master chain
and program output. Allow editing only nodes the renderer can execute. Provide
node search, typed ports, parameter inspection, validation errors, measured
GPU cost, and a shadow-preview/commit workflow. Reordering supported master
effects and inserting bounded supported effect nodes are the first meaningful
edits; unsupported topology must be visibly unavailable rather than silently
approximated.

**Runtime rules:** The existing typed graph compiler remains authoritative.
The editor works on a shadow graph, reports port/type/rate/color/resolution and
resource-budget errors, and only commits a fully prepared plan at the chosen
frame/beat/bar boundary. The proven fused four-deck renderer remains the
compatibility executor until an independent GPU node has a real executor and
benchmark.

**Acceptance:** A failed edit leaves the running show unchanged; a valid edit
can be previewed and committed at a scheduled boundary; undo/revert returns to
the last-known-good graph; project save/reload preserves stable node identity
and connections. Show Mode prevents graph edits while keeping performance
controls available.

**Boundary:** This is a constrained live-effects graph, not a general visual
programming language. Scripting, arbitrary GPU code, unbounded feedback loops,
compute nodes and multi-output routing are not part of the first editor.

## Separate design: final-program recording

The feature notes list a Movie File Out operator. VIRTUAL currently records
camera capture to a clip, while final-program recording is a different path.
Design it separately around a bounded asynchronous encoder queue, explicit
recording resolution/codec, disk-space estimate, drop/failure status, clean
finalization and a choice to add the result to the clip library. Capture the
clean composition before window presentation and projection warp so the file
does not include operator UI or depend on a physical display's geometry. Audio
capture/muxing, compressed formats and cross-platform codec availability need
decisions before implementation.

## 6. Low-light crowd and party FX

**Inspiration:** TouchDesigner-style camera-to-texture processing and
Pure-Data-style modulation, aimed at making club crowd footage readable and
visually expressive in a live VJ mix.

**User outcome:** Connect a camera or capture card, see the crowd on a deck,
apply a one-click look that brings out people and movement in dark footage,
and optionally make that look pulse with the music. Keep the image recognizable
while allowing a stronger abstract party treatment.

### Start from a camera-safe signal path

Keep camera capture and visual treatment separate. The camera feed enters a
normal deck, so its effects can be bypassed, blended, recorded as a source clip
and changed later. Camera-to-clip recording should keep capturing the source
signal by default; this preserves a clean original for later grading. Recording
the fully processed composition is a separate program-recording feature.

Offer a small live **Crowd FX** preset group on a camera deck, with intensity,
wet/dry and a clear bypass. Use current exposure/black-level/color controls
before edge extraction, and do not imply software can restore detail clipped by
the sensor or lost below its noise floor. Show a short camera-input readiness
hint for low light: lock focus/exposure if the capture device supports it, avoid
automatic exposure shifts during a take, and keep gain/noise tradeoffs visible.

### First preset set

1. **Silhouette Edge:** Lift usable shadows, suppress small noisy gradients,
   extract a luminance edge map, then use threshold/softness and edge width to
   separate people from the background. Offer white, cyan and user color; keep
   the original image blendable underneath the outline.
2. **Neon Crowd:** Reuse the silhouette edge map with a two-color palette and
   restrained glow. A solid-color outline mode keeps the crowd legible when the
   source image itself is nearly black.
3. **Night Vision:** Monochrome luminance with black point, gamma, highlight
   compression and green or amber phosphor palette. Add optional low-level
   grain and vignette as stylistic controls, defaulting off so noise does not
   dominate the people.
4. **High-Contrast Mono:** Black/white or two-tone threshold mapping with edge
   detail. Expose threshold and softness so a performer can choose between
   clean silhouettes and visible clothing/movement texture.
5. **Crowd Trails:** A deck-local motion trail that retains recent camera
   frames, decays them, and combines them with the current frame or extracted
   edges. Provide persistence, threshold, direction/echo count and blend mode;
   clear history on source change, disable, seek/reset, project load and
   resolution change.

The first four looks can be stateless one-pass deck packages using existing
camera, built-in color controls, edge detection, package presets and
modulation. Crowd Trails requires per-deck temporal history. The current master
Video Repeater has history but affects the composed output, while deck packages
are stateless; do not present either as an isolated camera trail. Add bounded
deck-local history as a separate renderer capability before shipping Crowd
Trails.

### Music and performance controls

All parameters should use stable deck-effect targets, so current audio
analysis, beat/bar sources, LFOs, MIDI and OSC can drive them. Provide optional
factory maps such as bass → trail persistence and transient → edge brightness,
with conservative depth and smoothing. Keep audio reaction disabled by default
and make mappings editable; a dark camera can already be noisy, so bass should
not force the edge threshold into an unusable range.

### Limits and validation

- The edge look should distinguish actual silhouettes from sensor noise across
  representative camera gain, exposure and lighting conditions. Include dark
  clothing, backlight, haze and moving spotlights in the fixture/rehearsal set.
- Stateless edge/night-vision looks should stay within the deck package's
  bounded one-pass contract and meet the 1080p60 target on the intended show
  machine with four active sources.
- Per-deck trail history must have a fixed memory budget, avoid render-thread
  waits, reset deterministically and expose its GPU cost. Test one and multiple
  camera decks before enabling it by default.
- Presets must work on transparent and opaque sources, retain alpha correctly,
  save/reopen and respond to MIDI/OSC. A bypass must return the camera image
  without stale history or color changes.
- Measure whether exposure lift before edge extraction needs an explicit
  temporal or spatial denoise. Avoid adding denoise by default if it smears
  dancers or materially increases latency.

**Recommended sequence:** Prototype Silhouette Edge and Night Vision as
stateless deck looks first, tune them against real club-camera footage, then
specify deck-local history and Crowd Trails. Keep all effects non-destructive;
the clean capture remains available if a look needs to change after recording.

## Suggested sequence

1. Wire the existing clip-envelope evaluator to video playback, then prototype
   row cells, automation-only launch and the Draw/Select editor. Reuse the
   existing row quantization, audio/clock and command-gateway code.
2. Design the broader control patcher after clip-lane ownership and target
   arbitration are settled, so the two automation systems share one rule set.
3. Prototype a per-deck generator layer stack with a shared segment budget,
   stable layer IDs and migration from existing single-generator projects.
4. Add deterministic 2D procedural sources as ordinary deck inputs.
5. Build a constrained visual editor over graph operations with existing GPU
   executors and last-known-good activation.
6. Prototype low-light crowd looks on real capture-card footage, then add
   deck-local history only after the stateless looks meet the show-machine
   performance target.
7. Specify final-program recording once codec, audio and storage behavior are
   selected.

Projection-map physical calibration remains an independent validation task.
It does not require simultaneous external outputs or changes to the four-deck
architecture.
