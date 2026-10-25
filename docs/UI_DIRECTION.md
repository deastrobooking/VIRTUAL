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

Next steps, in order of value:

1. **Docked side panels**: a browser (files, generators, effects) on the
   left and Deck/Clip properties on the right of the grid, instead of the
   long vertical scroll. Use egui `SidePanel`s inside the main window, with
   widths remembered.
2. **Preview monitor**: clicking a clip's name strip cues it to a preview
   thumbnail, as in Resolume, while clicking the image launches it.
3. **Clip colours**: per-slot colour from a 16-colour palette, chosen from
   the context menu and saved in the project. Tiles use it for the top bar
   and the empty-slot fill.
4. **Narrow tracks**: let Session columns collapse to launch buttons only,
   as Ableton does, for laptop screens.
5. **Icon set**: replace the remaining text buttons (Eject, Bypass, Solo,
   Record) in deck strips with the same compact glyph buttons used in the
   grid, keeping the text in tooltips.
6. **Layout presets**: save which sections are folded and the clip view as
   named layouts, alongside appearance presets.

## Sources

- Resolume, [Interface Layouts](https://www.resolume.com/support/en/layouts)
- Resolume, [Quickstart Tutorial](https://resolume.com/support/en/quickstart-tutorial)
- VJ Academy, [Resolume Arena for Beginners](https://vjacademy.info/resolume-arena-guide)
- Ableton, [Live 12 manual: Session View](https://www.ableton.com/en/live-manual/12/session-view/)
- Ableton, [Live 12 manual: Launching Clips](https://www.ableton.com/en/live-manual/12/launching-clips)
