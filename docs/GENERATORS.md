# Recursive geometry generators

A generator is a procedural deck source. In place of a clip or a camera, the
deck draws recursive 2D or 3D line geometry. It goes through the same path as
a camera, so deck effects, blend modes, transforms, modulation, Freeze and
**● Record clip** all work on it unchanged.

## Using it

1. Select a deck. Under the clip grid, **Generator · recursive geometry**
   offers a pattern list grouped into 2D and 3D. Choose one, then press
   **Load to Deck X**. This replaces the deck's clip or video input, and it
   is allowed in Show Mode.
2. Generators are a deck source, not an effect, so their controls are in a
   separate **◆ GENERATOR · DECK X** window. It opens when a generator
   becomes a deck's source (from the loader or a project load) and closes
   when the deck switches to a clip, camera or Eject. Each generator deck
   has its own window.
3. The window holds a pattern gallery (2D and 3D tiles) and the Shape,
   Motion & camera, Color & light, Audio and Output sections. Pattern and
   controls can be changed live; rotation, growth and trails carry on
   without a restart.
4. The deck strip shows only a summary: the pattern, a stats line (2D/3D,
   segment count, depth, resolution, CPU time per frame) and a
   **Generator controls…** button that reopens the window. When the depth
   is shown in amber, it was lowered to stay inside the segment budget.
5. Generator settings are saved in the project and restored when it is
   opened.

## Patterns

| Pattern | Dim | Colour / reveal order | Spread · Twist |
|---|---|---|---|
| Fractal Tree | 3D | trunk → tips | branch angle · branch roll |
| Recursive Spiral | 3D | trunk → tips | turn rate · roll |
| Mandala | 3D | trunk → tips | arm count and branch angle · roll |
| Koch Snowflake | 2D | along the path | bump angle (60° at default) |
| Crystal Growth | 3D | trunk → tips | side-branch angle · arm roll |
| Recursive Web | 2D | centre → rim | spoke count · ring swirl |
| Sierpinski Triangle | 2D | centre → edge | cell gap · cell rotation |
| Dragon Curve | 2D | along the path | fold angle (90° at default) · fold skew |
| Hilbert Curve | 2D | along the path | corner rounding |
| H-Tree | 2D | trunk → tips | child length ratio · branch rotation |
| Pythagoras Tree | 2D | trunk → tips | lean angle · alternating lean |
| Sierpinski Tetrahedron | 3D | centre → edge | cell gap · cell rotation |
| Menger Sponge | 3D | centre → edge | cube gap · cube rotation |
| Geodesic Sphere | 3D | centre → edge | fractal spikes · spike phase |
| Spirograph | 2D | along the path | lobe count · loop size |
| Torus Knot | 3D | along the path | winding count · tube radius |
| Chebyshev Curve | 3D | along the path | y phase · z phase (shape comes from degree x/y/z and polynomial mix) |
| Polynomial Contours | 3D | along the path | unused (shape comes from surface order, cross, contours and slice axis) |
| Supershape | 3D | along the path | unused (shape comes from symmetry and exponent) |

The first six patterns follow the formulas in the earlier generator. Three
deliberate changes were made:

- **Recursive Spiral** draws 24 steps per arm, and each arm sprouts three
  smaller spirals.
- **Koch** bump angle and **Recursive Web** swirl now respond to spread and
  twist. Both are neutral at the default values.
- **Randomness** uses a seeded generator, so a seed always gives the same
  shape.

## Controls

| Group | Controls |
|---|---|
| Shape | depth (3–11 levels; each pattern maps this to its own useful range), scale, spread, twist, randomness, seed / New seed, **Flatten to 2D** |
| Motion & camera | rotate (turntable), tilt, spin (about the view axis), zoom, perspective, reveal, grow loop, Stop motion |
| Polynomial & surface | degree x/y/z (1–12), polynomial mix, surface order (even, 2–12), surface cross, contours (4–64), slice axis, symmetry, exponent |
| Recursive object echoes | echo copies (1–24), echo scale, echo x/y/z rotation, echo offset, echo fade |
| Line tracing | trace heads (1–8), trace length, trace speed, trace spread |
| Color & light | hue, hue range, hue drift, saturation, lightness, brightness, line width, depth fade, trails, transparent background |
| Audio | audio amount: bass, mid and high bend the geometry (tree angle, length and roll; Koch bumps; web wobble and Z displacement; geodesic spikes) and shift hue, saturation and lightness |
| Output | 540p / 720p / 1080p / square 1080 / portrait 720 / portrait 1080, at 24, 30, 50 or 60 fps (hidden in Show Mode) |

**Neon**, **Fire**, **Ice** and **Mono** buttons apply color presets and stop
hue drift; shape, motion and output settings are preserved. Hue drift can be
enabled again after choosing a preset. Spirograph and Torus Knot use depth
for curve detail, with seeded randomness and continuous closed seams.

**Chebyshev Curve** evaluates Tₙ(cos t) on each axis with its own degree;
polynomial mix blends toward a plain Lissajous. **Polynomial Contours**
slices the closed surface xⁿ + yⁿ + zⁿ + c·(x²y² + y²z² + z²x²) = 1 into
contour rings, finding each ring's radius by bisection; slice axis picks
which axis the rings stack along. **Supershape** draws latitude and
longitude lines of a superformula solid.

**Echoes** repeat the whole object. Each copy is the previous copy rotated
by echo x/y/z (±180°), scaled by echo scale, offset along Z and dimmed by
echo fade. The segment budget is shared by all copies, so high copy counts
lower the effective depth.

**Tracing** draws only moving windows of each continuous path. Every path
carries its arc length, so heads travel at the same speed regardless of
segment density, partial segments are clipped exactly, and tails wrap around
closed curves. Trace spread staggers heads between separate paths. A trace
length of 1 (the default) draws the full wireframe.

**Traced sculpture** sets up 8 echoes with three moving trace heads and
trails; **Full wireframe** turns tracing back off. Defaults for every new
control (1 echo copy, trace length 1) leave existing projects looking exactly
as before.

## MIDI and OSC

Every slider and toggle in the generator window is a mappable target. Turn on
**MIDI Map** in the main window, click a generator control, then move a
controller; right-click a control to clear its mapping. Mappings are saved
with the project as `GeneratorParameter { deck, parameter }`.

Parameters have stable, append-only numeric IDs (listed in
`crates/virtual-generate/src/parameters.rs`). New controls are only ever
added at the end, so saved mappings keep pointing at the same control.
Values use the normalized 0–1 range and are scaled to each control's range.
Toggles switch at 0.5, and **pattern** (ID 45) steps through all patterns in
gallery order, which allows MIDI pattern switching.

| ID | Control | ID | Control | ID | Control |
|---|---|---|---|---|---|
| 0 | depth | 16 | lightness | 32 | echo copies |
| 1 | scale | 17 | brightness | 33 | echo scale |
| 2 | spread | 18 | line width | 34 | echo x |
| 3 | twist | 19 | depth fade | 35 | echo y |
| 4 | randomness | 20 | trails | 36 | echo z |
| 5 | rotate speed | 21 | audio amount | 37 | echo offset |
| 6 | tilt | 22 | degree x | 38 | echo fade |
| 7 | spin speed | 23 | degree y | 39 | trace heads |
| 8 | zoom | 24 | degree z | 40 | trace length |
| 9 | perspective | 25 | polynomial mix | 41 | trace speed |
| 10 | reveal | 26 | surface order | 42 | trace spread |
| 11 | grow speed | 27 | surface cross | 43 | flatten |
| 12 | hue | 28 | contours | 44 | transparent |
| 13 | hue range | 29 | slice axis | 45 | pattern |
| 14 | color speed | 30 | symmetry | | |
| 15 | saturation | 31 | exponent | | |

OSC uses the same IDs: `/virtual/deck/{1-4}/generator/{id}` with a float 0–1.
Mapped generator controls are included in MIDI feedback and OSC feedback.

Framing fits the shorter output dimension, including portrait formats.
Trail decay and incoming brightness use elapsed time, keeping their 60 fps
appearance consistent at other frame rates.

With **Transparent background** on, alpha follows line brightness, so the
lines sit over lower decks under Normal blend. With it off, lines are drawn
on black.

## How it works

```text
deck UI edits DeckState::Generator in place
      │  (playback compares with the last-sent copy)
      ▼
DeckDecoder::update_generator ──► deck worker thread
                                    │ regenerate geometry only when a
                                    │ geometry input changes (quantised)
                                    │ project → additive AA lines → tone map
                                    ▼
                    RGBA8 frame (pooled lease) ──► scheduler ──► GPU upload
```

- The `virtual-generate` crate contains no GPU, FFmpeg or UI code. `generate()`
  returns renderer-neutral segments, and `Geometry::to_buffers()` packs them
  as 6 position floats and 6 colour floats per segment. `Generator` holds one
  deck's animation state and its CPU rasterizer.
- **Budget.** No pattern ever produces more than 200,000 segments. Each
  pattern estimates its segment count for a given depth and lowers the depth
  until the estimate fits. For example, Koch grows as 3·4ⁿ and stops at
  depth 8, Menger grows as 12·20ⁿ and stops at 3, and Mandala multiplies a
  full tree by its 8–32 arms. The recursion also stops as soon as the budget
  is used up.
- **Pacing.** The worker renders at the chosen frame rate and waits on its
  command channel between frames, so setting changes apply straight away.
  Like a camera, it drops frames when the render loop falls behind. Live
  audio and per-frame statistics pass through atomics, so the render thread
  never waits.
- **Cost.** In a release build at 720p, frames take about 1–6 ms on a single
  worker core. A geometry rebuild at maximum depth adds up to about 12 ms
  (Mandala). Run
  `cargo run --release -p virtual-generate --example contact_sheet -- sheet.bmp`
  to render every pattern and print timings for the current machine.

## Not yet

- LFO and modulation routes do not reach generator parameters yet (MIDI and
  OSC do). The deck's own effects, transforms and LFO routes do apply on top
  of the generator.
- Seed, resolution and frame rate are not mappable.
- Generator decks are not included in session-journal crash recovery
  (camera decks aren't either). Project autosave does include them.

## October 2026 review

- **Orbit / Spin / Grow** motion presets change animation speeds without
  replacing the shape, colors or output settings. **Reset camera controls**
  restores tilt, zoom and perspective while retaining the current animation pose.
- The stats line shows the revealed segment count and flags frames exceeding
  the selected frame-rate budget. Lower depth, line width, resolution or fps
  when this persists; a geometry rebuild may cause a single transient warning.
- Hue drift now integrates its phase. Changing or stopping its speed keeps
  the current color; selecting a different base hue resets the drift offset.
- The renderer uses a fixed-size palette table instead of allocating it on
  every frame.

Local release measurements at 720p (10 frames per pattern, no live audio):
about 1–2.6 ms/frame at default depth, and 1–9.7 ms at maximum depth. Maximum
Mandala geometry generation took 13.2 ms separately. These are single-generator
CPU timings, not guarantees for a four-deck show with effects and video.

Next useful additions are LFO routing to generator parameters, tempo-synced
motion, and GPU line rendering for heavy multi-generator shows. These remain
future work.
