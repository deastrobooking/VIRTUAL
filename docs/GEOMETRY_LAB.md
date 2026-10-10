# Geometry Lab

Geometry Lab adds editable procedural geometry to generator layers. A graph can
create a 2D parametric curve or a 3D parametric surface, then pass its result
through transforms, twists, radial repetition, noise, and merge nodes before
the output node. Graph nodes have typed inputs and a bounded point budget, so
an invalid or unexpectedly expensive graph cannot grow without limit.

The calculator is a safe math expression evaluator rather than a general
programming language. It supports arithmetic, parentheses, constants such as
`pi` and `tau`, and common math functions including `sin`, `cos`, `sqrt`,
`min`, `max`, `clamp`, `lerp`, and `smoothstep`. Expressions can read graph
parameters and `x`, `y`, `z`, `u`, `v`, `t`, `time`, `bass`, `mid`, and `high`.
Time and audio-driven graphs refresh while playing. Non-finite results and
invalid expressions are rejected; the renderer keeps the last valid geometry.

In the selected generator layer's Geometry Lab panel, add nodes, choose their
inputs, edit expressions and node controls, and set the graph's output. Click a
node in the canvas to focus its inspector; rename or duplicate it there, set
its inputs, and safely delete it (connected nodes are rewired when possible).
The canvas shows connections and input/output pins, and nodes can be dragged to
arrange the graph.

The graphing calculator plots `y = f(x)` on a labeled Cartesian grid. Enter an
expression or build one with the on-screen keypad for variables, operators,
constants, common functions, and digits. Drag the plot to pan, scroll to zoom,
or set exact axis bounds. “Apply to focused curve” writes the plotted function
into a curve node, mapping the visible calculator x-domain across that curve's
parameter range. Invalid expressions are reported below the plot. Save a graph
to the project's Geometry library to reuse it from another layer.
Graphs in that library and graphs assigned to generator layers are stored in
the project file.

This authoring version evaluates on the CPU and stores reusable definitions in
the project. It does not yet provide standalone geometry files, unrestricted
code or custom functions, 3D mesh/solid topology, GPU graph evaluation,
parameter automation/MIDI mapping, or canvas grouping and undo.
