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
| Color & light | hue, hue range, hue drift, saturation, lightness, brightness, line width, depth fade, trails, transparent background |
| Audio | audio amount: bass, mid and high bend the geometry (tree angle, length and roll; Koch bumps; web wobble and Z displacement; geodesic spikes) and shift hue, saturation and lightness |
| Output | 540p / 720p / 1080p / square 1080 / portrait 720 / portrait 1080, at 24, 30, 50 or 60 fps (hidden in Show Mode) |

**Neon**, **Fire**, **Ice** and **Mono** buttons apply color presets and stop
hue drift; shape, motion and output settings are preserved. Hue drift can be
enabled again after choosing a preset. Spirograph and Torus Knot use depth
for curve detail, with seeded randomness and continuous closed seams.

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

- MIDI learn, OSC and LFO routes do not reach generator parameters yet. The
  deck's own effects, transforms and LFO routes do apply on top of the
  generator.
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

Next useful additions are generator MIDI/OSC parameter routing, tempo-synced
motion, and GPU line rendering for heavy multi-generator shows. These remain
future work.
