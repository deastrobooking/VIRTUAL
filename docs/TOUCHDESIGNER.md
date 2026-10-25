# TouchDesigner and Pure Data features

VIRTUAL borrows ideas from TouchDesigner (operators, TOP generators, Movie
File Out, network inspection) and Pure Data (patch cords and control
signals), and keeps them inside a live show tool: everything is bounded,
mappable to MIDI and OSC, saved in the project, and safe in Show Mode.
This document records what is built, how it works, and the agreed plan for
what is not built yet. The original designs are in
[FEATURE_DESIGNS.md](FEATURE_DESIGNS.md).

| Feature | Inspiration | Status |
|---|---|---|
| Generator layer stack | TOP compositing | Built |
| Clip automation lanes | CHOP channels on a timeline | Built |
| Procedural sources | Ramp, Noise, Constant, Text TOPs | Built (October 2026) |
| Graphing calculator and Math Library | Script CHOP/SOP expressions | Built (October 2026) |
| Program recording | Movie File Out TOP | Planned, researched |
| Render-graph inspector | TOP network view | Planned |
| Control patcher | Pd patch cords, CHOP networks | Planned |

## Built

### Procedural sources

Generator **Sources** need no media: Solid Color, Color Bars, Linear Ramp,
Radial Ramp, Noise, Checkerboard, Test Grid and Text. They appear in the
SOURCES row of the generator gallery and behave like any generator layer:
blend modes, layer transforms, deck effects, MIDI/OSC `GeneratorParameter`
targets, clip automation and project saving all apply. User-facing controls
are in [GENERATORS.md](GENERATORS.md#sources).

Implementation (`crates/virtual-generate`):

- `RecursivePattern` gained eight variants. `is_raster()` marks the fill
  sources and `is_source()` the gallery row. Pattern ids are persisted, so
  only append.
- `texture.rs` renders fills per pixel into straight-alpha RGBA8, the frame
  format every deck already consumes. A `Fill` struct carries everything the
  image depends on, already integrated over time: the palette LUT, ramp
  scroll, rotation and noise flow. `Fill::key()` lets `Generator::render_fill`
  reuse the previous frame when nothing changed, so static sources cost a
  memcpy.
- Noise is seeded fractal value noise (`mix64` lattice hash, smoothstep
  interpolation). It is computed on a grid four pixels apart and
  bilinearly interpolated, because it is smooth.
- Colours come from the existing palette: hue, hue range, saturation and
  lightness. Ramps sweep the palette, and **depth fade** darkens toward the
  end colour, which gives colour-to-black ramps.
- Text is not a raster source. `text.rs` lays out a 16-segment display font
  (ASCII, `|` for new lines) as line geometry in `patterns::build`, so text
  gets trails, echoes, tracing, reveal and audio colour like any line
  pattern. `GeometryParams.text` is a fixed `[u8; 48]` so the params stay
  `Copy`, and `GeometryKey` includes it so editing the text regenerates.
- Cost: about 10 ms per animated 1280×720 frame on one core (release, M3
  Pro). Generators render on deck worker threads, not the render thread.

Deviation from the design: the design asked for GPU shaders. The CPU path was
chosen so sources share layers, budgets and persistence with existing
generators immediately. Moving fills to WGSL is the obvious optimisation if
several 4K fills run at once.

### Graphing calculator engine and Math Library

Not a TouchDesigner operator, but it fills the same need as expression
CHOPs: maths that drives visuals. See [GEOMETRY_LAB.md](GEOMETRY_LAB.md).

- `sum(k, lo, hi, body)` and `prod(…)` bind a local index (`Bound::Local`)
  and are capped at 10,000 terms. There is also postfix `n!`, plus `gamma`,
  `binom`, `erf`, `gcd`, `lcm` and `isprime`.
- `(X, Y, Z)` in `u, v` draws parametric surfaces; `(a(n), b(n))` draws
  sequences.
- Unary minus now binds looser than `^` (`-x^2 = -(x^2)`).
- `graphing_library.rs` holds 292 examples in 24 folders, each with an
  explanation. A test checks that every example parses and draws in its
  window, so the library cannot rot as the engine changes.

## Planned: program recording (Movie File Out)

**Outcome:** press ⏺ REC (or a mapped MIDI button) and the program output is
written to a movie file until stopped, without disturbing the show.

**What exists to build on:**

- `ProgramReadback` (`virtual-render/src/readback.rs`) copies the program
  texture into a ring of mapped staging buffers and delivers frames without
  blocking. When every slot is busy it skips a capture and counts it.
  `NdiOutput` (`virtual-app/src/ndi.rs`) shows the full pattern: `capture()`
  before submit, `after_submit()` to poll and forward.
- The program texture is `Rgba8UnormSrgb` and is captured before projection
  warp and window presentation, so recordings are the clean composition the
  design requires.
- `CameraRecorder` (`virtual-media/src/record.rs`) is a bounded,
  drop-instead-of-stall muxer thread. It writes raw RGBA MOV, keeps capture
  timestamps across dropped frames, and reports results through a channel.
- FFmpeg 8 (Homebrew) on the development Mac has `h264_videotoolbox`,
  `libx264`, `hevc_videotoolbox`, `prores`/`prores_aw` and `mpeg4`.
  Windows (vcpkg) and Linux builds must probe for encoders at runtime.

**Plan:**

1. Generalise `CameraRecorder` into `FrameRecorder` with a codec choice:
   - ProRes 422 `.mov` as the default (always available, edit-friendly);
   - H.264 `.mp4` using `h264_videotoolbox`, then `libx264`, then `mpeg4`;
   - raw for camera clips as today.

   Convert RGBA to the encoder's pixel format with `software-scaling` on
   the muxer thread, never on the render thread.
2. Add `virtual-app/src/program_record.rs`. It owns a `ProgramReadback`
   created only while recording, plus start/stop, elapsed time and dropped
   frames. Name files `VIRTUAL-YYYYMMDD-HHMMSS` in a folder chosen in
   Setup → Recording, saved with the project. Share the readback with NDI
   when both run.
3. Add a `ControlTarget::ProgramRecord` toggle and plumb it through the
   standard target checklist:
   - session path `program.record`;
   - io `ControlTargetProject` and its validation;
   - `project.rs` conversions;
   - OSC route and feedback;
   - MIDI labels and target list;
   - `current_control_value` and the snapshot;
   - devices dispatch.
4. Add a toolbar ⏺ button with time and drop count, and an "Add to clip
   library" choice when recording stops.
5. Test by writing ten synthetic frames per codec to a temp file and probing
   it with `probe_movie`.
6. Out of scope at first: audio muxing, a disk-space estimate (show MB/min
   from resolution and codec instead), and HAP.

## Planned: render-graph inspector

**Outcome:** a window that shows the live processing path and what it
costs, so an operator can see where GPU time goes and reorder master
effects safely.

**What exists:** `LoweredRenderPlan` (`virtual-render/src/graph_plan.rs`)
holds the executed stages:

- `FourDeckComposite`, which fuses four deck source→effects branches and
  the mixer, with each logical node still addressable;
- `MasterEffects`;
- `ProgramOutput`.

The typed graph compiler (`virtual-graph`) stays authoritative.

**Plan:**

1. Add a **Render Graph** window, drawn left to right: four deck sources →
   deck effects → four-deck mixer → each master effect as its own node →
   program → outputs (projection windows, NDI, recording, sync zones).
   Each node shows live state: source and clip name, level, mute and
   bypass, effect counts. Clicking a node opens its existing panel.
2. Measure GPU cost per stage with wgpu timestamp queries when the adapter
   supports `TIMESTAMP_QUERY`, using a small resolve ring in
   `virtual-render`. Show milliseconds coloured against the frame budget,
   and "n/a" when timestamps are not supported.
3. Allow reordering master effects only, through the existing journaled
   reorder command, so it can be undone and it records in takes. Show Mode
   disables it. Unsupported topology edits are shown as unavailable, never
   approximated.

## Planned: control patcher

**Outcome:** small signal chains such as *audio bass → smooth → scale → deck
2 level*, built by wiring nodes rather than stacking matrix routes.

**Plan:**

1. Put the model in `virtual-core/src/patcher.rs`, pure and tested.
   `Patch { nodes, wires }` uses stable node ids. Node kinds:
   - **sources:** audio band (level, bass, mid, high, hit, 8 bands), LFO
     (sine, triangle, saw, square, random; beat-synced), beat/bar phase,
     constant, and the current value of any control;
   - **processing:** smooth (attack/release), scale/range, curve, math
     (+ − × min max), gate/threshold → trigger, sample & hold,
     toggle/counter, and a one-tick delay (the only way to make feedback);
   - **sinks:** out → `ControlTarget`.

   Nodes are evaluated once per frame in topological order. Wiring a cycle
   without a delay is rejected, and an invalid edit keeps the last valid
   patch running.
2. Each frame, send outputs through `apply_control_update_unrecorded`, the
   same path as sync and automation, so manual takeover and the automation
   switch behave identically. Send only changes beyond an epsilon.
3. Add a **Patcher** window: a node canvas with drag-to-wire ports, a
   right-click node menu and a live value meter on every port. Pick output
   targets with the existing "click a control" learn flow.
4. Save the patch in the project (`ProjectPatch` in virtual-io, validated on
   load). Patcher on/off is a mappable `ControlTarget::PatcherEnabled`.
5. Tests cover:
   - topological evaluation;
   - cycle rejection and delay feedback;
   - smoothing maths;
   - the gate's trigger edge;
   - LFO beat sync;
   - the round-trip through project files.
