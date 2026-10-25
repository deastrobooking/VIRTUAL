# Interface direction

Notes from studying Resolume Arena and Ableton Live, the two tools VIRTUAL's
operators most often know, and what VIRTUAL takes from each.

## What the references do well

**Resolume Arena** (media server / VJ):

- The clip grid is the whole show: layers run as horizontal rows, each with
  a thumbnail per clip. The left of each layer holds **X** (eject), **B**
  (bypass), **S** (solo) and a **V** opacity fader, so a layer can be killed
  or isolated without leaving the grid.
- **Column triggers** along the top fire a whole column of clips at once.
  **Decks** are sets of columns, switched with tabs below the grid.
- Composition, Layer and Clip property panels sit to the right. The browser
  for Files, Sources, Effects and Compositions sits to the left. Composition
  and Preview monitors sit at the top right: clicking a clip's title sends it
  to preview, clicking its thumbnail sends it live.
- Panels can be hidden, docked, undocked to other screens and saved as
  layout presets. Clips, columns, layers and decks can all be colour-coded.
- The look is neutral charcoal with thin outlines and small type, so
  thumbnails carry the colour. A single cool accent marks what is live.

**Ableton Live** (Session view):

- Tracks are vertical **columns** that each play one clip at a time. Scenes
  are rows, launched from the rightmost Main column.
- Every clip has a triangular launch button. Square **stop** buttons sit in
  empty slots and in the track status row, and the Main track's status field
  has **Stop All Clips**.
- Track title bars are coloured. Clips take colours from a palette, and the
  track status shows a loop pie or a one-shot progress bar.
- Tracks can be narrowed to just their launch buttons, which makes a dense
  grid on a laptop screen.

## What VIRTUAL adopts

Built so far:

- **Two clip views** over the same slots:
  - **Session** has condensed horizontal lanes in the style of
    Premiere/Final Cut tracks. Lane headers carry Resolume's layer controls
    (badge, X/B/S, level, what is playing). Clips are filmstrips with name
    and length labels, a deck-colour bar and a live playhead.
  - **Launcher** is Ableton's clip launcher: deck columns, scene launchers
    on the right, per-deck stop and Stop All.
  - Both have S/M/L sizes, saved with the project's appearance.
- **Layers** view: Resolume's grid, kept as a third option.
- **Clip colours**: 16 colours per slot from the context menu, saved in the
  clip's playback settings so they follow moves.
- **Docked properties panel** on wide windows, with a live **PROGRAM**
  monitor (the program texture shown through egui-wgpu) above the selected
  deck's controls.
- **Compact X/S/B buttons** in deck strips, matching the grid.
- **Browser panel** on the left (Files, Sources, Effects), with every item
  draggable onto clip slots.
- **PREVIEW monitor** beside PROGRAM: an isolated, muted moving source cue
  with play/pause and scrubbing. Clip selection and editor opening do not launch.
- **Browser posters**: bounded background decoding and a 64-poster cache.
- **XS size**: name-only clips, Ableton-style narrow tracks.
- **Layouts**: Perform, Prepare, Minimal and Full fold the window in one
  click.
- **Clip preview on hover**: a 256×144 frame above the clip details.
- **Templates** as starting points for colour, outlines, rounding, text
  size and spacing:
  - **Arena** is the new default: charcoal, a teal accent, thin lines and
    compact spacing.
  - **Session** has warm greys, green play and orange selection.
  - **Graphite** is monochrome, with colour only marking state.
  - The earlier themes remain.
  - Templates show colour chips in the Appearance window.
- **Foldable section bars** for Setup, Clips, Audio, the selected deck,
  Mix & Master and Master Effects: a slim tinted bar with a chevron, an icon
  and a small upper-case title, plus a status line where useful (decks
  playing, master level, blackout).
- **Condensed toolbar**: a one-line logo, status dots and compact A–D mute
  chips.

The workspace now includes a function-key section rail, a movable selected-deck
UNIT, compact source/automation tabs in the clip editor, cross-view clip and
scene clipboards, and a saved sequential editor for video cuts and recorded take
segments. Source cuts render to MP4; trimmed takes assemble into a new recovery
journal. See [timeline editing](TIMELINE_EDITING.md) for the workflow and current
rendering boundaries.

The next batch implements separate cue/launch interaction, gesture-based
undo in the timeline and popup clip editor, pinned/following deck UNIT windows,
saved Perform/Edit/Build layouts, moving cue playback and background browser
posters. Initial finished-program capture records native FX into a silent raw
MOV with bounded staging and writing. Native-window layout inspection and
show-machine load/failure certification remain outstanding.

Next work is isolated sequence playback and deterministic native-FX offline
rendering, followed by reusable ordered UNIT racks. See the
[effects-suite feature plan](FX_SUITE_PLAN.md) for the dependencies and completion
checks, and [workspace and recording](WORKSPACE_RECORDING.md) for the current UI.

## Sources

- Resolume, [Interface Layouts](https://www.resolume.com/support/en/layouts)
- Resolume, [Quickstart Tutorial](https://resolume.com/support/en/quickstart-tutorial)
- VJ Academy, [Resolume Arena for Beginners](https://vjacademy.info/resolume-arena-guide)
- Ableton, [Live 12 manual: Session View](https://www.ableton.com/en/live-manual/12/session-view/)
- Ableton, [Live 12 manual: Launching Clips](https://www.ableton.com/en/live-manual/12/launching-clips)
