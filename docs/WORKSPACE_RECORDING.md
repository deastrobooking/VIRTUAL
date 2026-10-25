# Workspace, cue and program recording

## Select, cue and launch

Click a clip body to select and cue it. Click its triangle to launch it to PROGRAM.
Double-click the body to open the trim/automation editor without launching. Drag
past the pointer threshold to move the clip; an occupied target swaps slots.
Scene launch buttons and number-key scene launches keep their performance role.

PREVIEW is a muted source player with its own decoder and texture. Its play/pause
button and scrubber affect the cue only. The cue respects the selected trim,
loops the source at normal speed and displays stills without repeatedly reopening
them. It does not preview deck/master FX or recorded-take playback. Output frames
fit within 480×270, publishing at up to 30 fps. Cue and browser poster software
decoding are separate from GPU-native HAP performance playback.

The Files browser decodes posters in the background, requests visible media only,
limits pending work and caches at most 64 posters. Refresh clears poster failures
and cached images. Folder changes reject results from old requests.

## Undo and UNIT targets

The timeline and popup clip/automation editor share a bounded transaction model:
a drag creates one undo entry, and clicking Undo returns to its starting value.
Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z undo/redo in the foreground editor; text fields
retain their own keyboard editing. Each editor retains up to 64 transactions.
The popup history belongs to its current clip and media reference; switching to
a different source starts a new history. Undo history is not project persistence.

Use **Expand UNIT**, then **Pin target** to hold the deck editor on its current
deck. Selecting another deck changes the cue/import target while the pinned
window continues editing the named deck. Unpin to follow selection. Closing the
window returns the deck editor to the properties area.

## Save a workspace

Under **Layout**, choose **Perform**, **Edit**, or **Build**. These start with
performance, arrangement, and modular editing presets. Adjust the sections,
browser, density, clip view/size, panel widths, UNIT pin and timeline visibility,
then use **Save current workspace as…**. Save the project to keep that override.
Reopening the project restores its active workspace. Appearance presets and the
older quick layouts remain available.

## Record the finished program

Use **Recording…** in the toolbar to choose 720p or 1080p and a target capture rate
of 24, 30 or 60 fps. The initial format is **silent uncompressed RGBA MOV**. The
menu shows an estimated data rate and GB/minute; at 720p30 it is approximately
111 MB/s or 6.6 GB/minute. Compressed encoding and audio recording are future work.

An empty destination creates a unique file in the workspace's `recordings`
folder. Choose another destination before the show if needed. The actual extension
is `.mov`. **REC PROGRAM** starts; **■ PROGRAM** stops. Start/Stop remain
available in Show Mode without opening a system dialog; settings and destination
selection are locked there.

Capture includes the finished native generator/deck/master FX composition,
blackout and freeze. It excludes operator UI, presentation calibration overlays
and output projection warp. A dedicated GPU target fits the composition to the
recording size while preserving its aspect ratio, so resizing the composition
or moving the output monitor does not change the movie's capture size.

The GPU readback ring and writer queue are bounded. Slow storage drops capture
frames instead of waiting in the render loop. The last image holds through gaps,
and Stop preserves the elapsed capture endpoint. The toolbar reports elapsed
time, dropped frames, finalization and failures. Normal app shutdown flushes
outstanding capture and closes the movie. Power loss or forced termination can
still leave an unfinished MOV.

Existing destination files are preserved. A completed movie is published only
after successful finalization. If publishing fails, its error identifies the
completed staging movie so it can be recovered. A writer failure ends recording
and reports the error while the live mixer continues.

After completion, **Add last recording to clips** imports into a free media and
automation slot, outside Show Mode, without launching it or replacing a live deck. Source-camera recording remains a separate
control. The timeline's MP4 source-cut exporter also remains separate: recording
captures the live processed mix; offline native-FX sequence export is still a
later milestone.
