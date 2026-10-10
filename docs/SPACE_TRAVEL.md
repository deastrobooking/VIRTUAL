# Space Travel effect

Space Travel is a real-time WGSL image effect for generator or video decks and
the master chain. It draws a procedural starfield over the live source image,
with a polar launch point, animated depth, controllable streak length, star
density, tint, brightness, and source preservation. Its parameters use the
existing stable effect-parameter mapping system, so they can be automated and
assigned to MIDI/OSC controls just like other packaged effects.

The Space Travel package includes Hyperspace, Slow Drift, and Neon Launch
presets. Set the mix amount to zero to bypass it, or reduce Source Preservation
to let the starfield dominate.

Landscape Mapper remains a larger renderer feature. A true brightness-driven
wireframe requires sampling each deck's live frame into a low-resolution height
field and drawing a displaced line mesh. The current image-effect package ABI
only transforms pixels and cannot issue a geometry draw call, so describing a
fragment-shader approximation as that mapper would be misleading. Its next
implementation should add a dedicated deck renderer stage with bounded grid
resolutions, GPU displacement, and a shared frame-time budget.
