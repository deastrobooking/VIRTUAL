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
inputs, edit expressions and node controls, and set the graph's output. Use the
calculator fields to try expressions against a representative input sample.
Save a graph to the project's Geometry library to reuse it from another layer.
Graphs in that library and graphs assigned to generator layers are stored in
the project file.

This is the first authoring version: it evaluates on the CPU, provides a compact
node canvas and math calculator, and stores reusable definitions in the
project. It does not yet provide standalone geometry files, unrestricted code
or custom functions, 3D mesh/solid topology, GPU graph evaluation, parameter
automation/MIDI mapping, or a full-featured canvas with grouping and undo.
