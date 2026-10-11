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

## Graphing calculator

Open **GRAPHING CALCULATOR** in a generator window, or click **Open in its own
window** for a large resizable calculator per deck. It follows the active
theme: text, input fields, buttons, grid, axes and object colours all come from
the Appearance palette.

### Algebra list

Type into **Input** and press Enter (or **Add**). Each object gets a colour
swatch (click to show/hide, right-click for the next colour), an editable
expression, its detected type, and type-specific controls. Unicode and keypad
symbols (`θ π τ × ÷ − ² ³ ≤ ≥ √`) are accepted, and multiplication can be
implicit: `2x`, `3sin(x)`, `x(1+x)`, `2pi`.

| Input | Object |
|---|---|
| `sin(x)`, `y = x^2`, `f(x) = 2x` | function of x |
| `x = y^2`, `x = 3` | function of y (vertical lines too) |
| `r = 1 + cos(θ)` | polar curve over its θ range |
| `(cos(t), sin(t))` | parametric curve over its t range |
| `(2, 3)`, `A = (a, 1)` | point |
| `x^2 + y^2 = 9` | implicit curve (any `f(x,y) = g(x,y)`) |
| `y < sin(x)`, `x^2 + y^2 ≤ 4` | shaded inequality with its boundary |
| `a = 2` | slider; any other object can use `a` |
| `z = sin(x) cos(y)` | surface (3D view) |
| `(cos(t), sin(t), t/5)`, `(1, 2, 3)` | 3D curve, 3D point |
| `(-y, x)`, `F = (P, Q)` | vector field with RK4 streamlines |
| `dy/dx = y - x` | slope field with solution curves |

Every expression can use `time` (seconds); **▶ Animate time** runs it, so
physics such as a projectile `(10t cos(0.8), 10t sin(0.8) - 4.9t^2)`, damped
oscillation `e^(-0.2x) cos(3x - time)` or fields with moving terms animate.
**Examples** adds a set of these with one click.

Functions: `sin cos tan asin acos atan sec csc cot sinh cosh tanh asinh acosh
atanh sqrt cbrt abs exp ln log log10 log2 floor ceil fract round sign min max
pow atan2 mod hypot clamp lerp smoothstep`; constants `pi tau e`. Plotting uses
real-valued maths: `sqrt(-1)`, `ln(0)` and division by zero are undefined
points (gaps), not errors, and curves break at asymptotes such as `tan(x)`.

### Views and analysis

- **2D graph:** drag to pan, scroll to zoom at the cursor, double-click to
  reset. The readout shows the cursor position and the active function's
  value there.
- **3D graph:** drag to orbit, scroll to zoom. Surfaces are shaded by height;
  x and y use the 2D view's range and z fits the visible objects. 2D curves are
  drawn in the z = 0 plane.
- Per function: **f′** draws the derivative (dashed), **roots/extrema** marks
  roots, local extrema and intersections with other marked functions, and
  **∫ area** shades and evaluates the integral between a and b (Simpson's
  rule). **ANALYSIS** summarises the active function and **Table of values**
  lists x, f(x) and f′(x) at a chosen step.

### Keypad

A 0–9 number pad laid out like a keyboard's (7 8 9 / 4 5 6 / 1 2 3 / 0 . ⌫),
operators, variables (`x y z t θ time`), a function grid, constants and
templates (`y =`, `z =`, `r =`, `(x, y)`, `(x, y, z)`, `dy/dx =`, `≤`, `≥`).
Keys type at the cursor of the focused expression (or Input), replacing any
selection; function keys place the cursor inside their parentheses.

### Applying to geometry

**Apply to curve node** / **Apply to surface node** writes the active object
into the focused (or first) matching Geometry Lab node, with slider values
substituted: functions map the visible x range onto the node's t; parametric,
polar and 3D curves map their t or θ range; surfaces map the visible x and y
ranges onto u and v. Save a graph to the project's Geometry library to reuse it
from another layer.

Calculator objects are kept for the session; they are not yet saved in the
project.

Graphs in that library and graphs assigned to generator layers are stored in
the project file.

This authoring version evaluates on the CPU and stores reusable definitions in
the project. It does not yet provide standalone geometry files, unrestricted
code or custom functions, 3D mesh/solid topology, GPU graph evaluation,
parameter automation/MIDI mapping, or canvas grouping and undo.
