# Compact workspace and timeline editing

The section rail and its function keys share one navigation map. Selecting a
section opens it and scrolls it into view; the browser floats on narrow windows.
The selected deck remains the target for generator and effect controls.

| Key | Section |
|---|---|
| F1 | Media files |
| F2 | Generators / sources |
| F3 | Deck effects |
| F4 | Clips / Session |
| F5 | Selected deck UNIT |
| F6 | Video mixer and master level |
| F7 | Master effects |
| F8 | Audio |
| F9 | Setup (outside Show Mode) |
| F10 | Video timeline and recorded takes |

The rail's **Expand UNIT** control opens the selected deck's existing transport,
transform, effect, package and modulation controls in a movable window. Close
that window to return them to the properties panel. Generator windows retain
their independent editors.

## Clips and scenes

Session, Launcher and Layers are arrangements of the same 32 slots. New layouts
start with small clips. Session cells shrink to the available width while
retaining a minimum clickable size; XS/S/M/L remain available.

Select a clip and use **Copy**, switch views or decks, select a free destination,
and use **Paste**. Cmd/Ctrl+C and Cmd/Ctrl+V work in the foreground workspace
when a text editor does not own the keyboard. The clipboard contains a snapshot
of the media reference, trim, launch policy, color and automation. Clearing or
editing the original does not change that snapshot. Paste requires a free slot;
occupied slots remain intact. Deck-local automation follows a copy or move to
its destination deck. Routes deliberately targeting another deck or the master
keep their targets.

Double-click a clip, press Enter, or use the pencil button to open its popup
editor. **Source / Trim** and **Automation** share the window. **Copy clip**
copies its current settings; **Paste settings** applies the clipboard's trim,
launch policy, color and automation to the current clip while keeping its media.
Trim values are clamped to the destination source. Show Mode locks these edits.

A scene's context menu supports copying, pasting and clearing scene assignments,
as well as capturing the selected clips and assigning each deck individually.
Scene copying copies assignments; it does not duplicate media.

## Video cuts

Use **+ Timeline** below the clip grid, in the popup editor, or in the clip's
context menu to append its current trimmed source range. F10 opens the editor.
The **Video cuts** track supports:

- In/Out trimming, a cursor, and splitting the selected cut at the cursor.
- Dragging cuts into a new order, or moving them with the arrow buttons.
- Copy/paste, duplicate, remove, and 64 levels of undo/redo.
- Project save, autosave and reopen, including portable paths inside the show folder.

The track is a sequential source edit with up to 128 cuts. It does not yet have
stacked compositing tracks, transitions, audio editing, or timeline playback.
The existing program and preview monitors remain the performance monitors.

**Render video…** writes a silent, 30 fps H.264 MP4 at the composition size
(odd dimensions round down to an even size). Sources are fitted and letterboxed.
Each cut is normalized in sequence to bound decoder count, then joined. The
export runs on a worker and supports cancellation. An existing destination is
never overwritten. Failed or canceled jobs remove their temporary files.

This render exports source cuts and their In/Out trims. It does **not** bake the
live deck transforms, FX, generator state, playback speed or recorded mixer
performance into the movie. Rendering the complete modular composition is the
next renderer integration, separate from source-cut export.

Video rendering requires the **FFmpeg executable**, in addition to the FFmpeg
libraries used by playback. VIRTUAL uses `VIRTUAL_FFMPEG` when set, then the
standard Homebrew locations, then `ffmpeg` on PATH. The current packaging script
bundles playback libraries in portable builds; it does not bundle that executable.

## Recorded performance takes

In **Performance takes**, use **Scan recorded takes**, then **Add take segment**.
The catalog uses the existing project/unlinked recovery journals. Add multiple
segments, trim their ranges, reorder, split, duplicate, or remove them using the
same editor controls. A take's In point cannot precede its first baseline.

**Export assembled take…** creates a new JSONL journal with fresh command
identities and rebased cut times. Each cut starts from the source's state at its
In point. Intermediate checkpoints and timeline markers are retained. The
original journals remain intact. All segments must share a project identity
(or all be unlinked). The export contains inline checkpoints and can be recovered
without a separate checkpoint file.

Save the assembled journal in the default `.virtual/session` directory, then
scan to find it in recovery. **Restore take at cursor as branch** restores the
selected source segment at that point using the existing recovery workflow;
it changes the live mixer and starts a new branch. Show Mode blocks restoration.
A performance take stores control history and recovery state, not rendered video.

The next suite milestones are deterministic playback of an assembled performance,
full-composition offline rendering with the native effect graph, and overlapping
video tracks with transitions. These are not exposed as completed features.
