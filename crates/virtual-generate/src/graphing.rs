//! Graphing-calculator engine: classifies GeoGebra-style input lines and
//! samples them into drawable primitives. Pure and UI-free, so every object
//! type is unit tested.
//!
//! Recognised input, after normalising Unicode (`θ π τ × ÷ − ² ³ ≤ ≥ √`):
//!
//! | Input | Object |
//! |---|---|
//! | `sin(x)`, `y = x^2`, `f(x) = 2x` | function of x |
//! | `x = y^2` | function of y |
//! | `r = 1 + cos(theta)` | polar curve |
//! | `(cos(t), sin(t))` | parametric curve, `t` over the object's range |
//! | `(2, 3)`, `A = (a, 1)` | point |
//! | `(cos(t), sin(t), t/4)` | 3D parametric curve |
//! | `(1, 2, 3)` | 3D point |
//! | `x^2 + y^2 = 9` | implicit curve |
//! | `y < sin(x)`, `x^2 + y^2 <= 4` | shaded inequality |
//! | `a = 2` | slider other objects can use |
//! | `z = sin(x)*cos(y)` | surface |
//! | `(y, -x)`, `F = (P, Q)` with x/y and no t | vector field with streamlines |
//! | `dy/dx = x - y` | slope field |
//!
//! Every expression may also use `time` (seconds) to animate and any slider.

use crate::geometry_graph::{BoundExpression, CalculatorError, CalculatorExpression};

/// Variables every object can read, in slot order; sliders follow.
pub const BASE_VARIABLES: [&str; 6] = ["x", "y", "z", "t", "theta", "time"];
const X: usize = 0;
const Y: usize = 1;
const T: usize = 3;
const THETA: usize = 4;
const TIME: usize = 5;

/// Names reserved for variables, so they cannot become sliders.
const RESERVED: [&str; 8] = ["x", "y", "z", "t", "theta", "time", "r", "e"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Comparison {
    fn holds(self, value: f32) -> bool {
        match self {
            Self::Less => value < 0.0,
            Self::LessEqual => value <= 0.0,
            Self::Greater => value > 0.0,
            Self::GreaterEqual => value >= 0.0,
        }
    }
}

/// What an input line describes. Expressions are kept as normalised source.
#[derive(Clone, Debug, PartialEq)]
pub enum PlotKind {
    /// `y = f(x)`.
    Function(String),
    /// `x = f(y)`.
    FunctionOfY(String),
    /// `r = f(theta)`.
    Polar(String),
    Parametric(String, String),
    Point(String, String),
    Parametric3(String, String, String),
    Point3(String, String, String),
    /// `f(x, y) = 0`.
    Implicit(String),
    /// `f(x, y) <op> 0`.
    Inequality(String, Comparison),
    Slider {
        name: String,
        value: f32,
    },
    /// `z = f(x, y)`.
    Surface(String),
    VectorField(String, String),
    /// `dy/dx = f(x, y)`.
    SlopeField(String),
}

impl PlotKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Function(_) => "function",
            Self::FunctionOfY(_) => "function of y",
            Self::Polar(_) => "polar",
            Self::Parametric(..) => "parametric",
            Self::Point(..) => "point",
            Self::Parametric3(..) => "3D curve",
            Self::Point3(..) => "3D point",
            Self::Implicit(_) => "implicit",
            Self::Inequality(..) => "inequality",
            Self::Slider { .. } => "slider",
            Self::Surface(_) => "surface",
            Self::VectorField(..) => "vector field",
            Self::SlopeField(_) => "slope field",
        }
    }

    /// Whether the object belongs in the 3D view.
    pub fn is_3d(&self) -> bool {
        matches!(
            self,
            Self::Surface(_) | Self::Parametric3(..) | Self::Point3(..)
        )
    }

    /// Whether `t` is this object's sampling parameter.
    pub fn uses_parameter(&self) -> bool {
        matches!(self, Self::Parametric(..) | Self::Parametric3(..))
    }
}

/// Replaces display symbols with the ASCII the evaluator parses.
pub fn normalize(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for c in source.trim().chars() {
        match c {
            'θ' => out.push_str("theta"),
            'π' => out.push_str("pi"),
            'τ' => out.push_str("tau"),
            '×' | '·' | '⋅' => out.push('*'),
            '÷' => out.push('/'),
            '−' | '–' => out.push('-'),
            '²' => out.push_str("^2"),
            '³' => out.push_str("^3"),
            '≤' => out.push_str("<="),
            '≥' => out.push_str(">="),
            '√' => out.push_str("sqrt"),
            _ => out.push(c),
        }
    }
    out
}

/// Splits at commas outside parentheses.
fn split_top_level(source: &str) -> Vec<&str> {
    let (mut depth, mut start, mut parts) = (0i32, 0, Vec::new());
    for (index, c) in source.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(source[start..].trim());
    parts
}

/// The inside of `( … )` when the outer parentheses enclose everything.
fn tuple_body(source: &str) -> Option<&str> {
    let inner = source.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0i32;
    for c in inner.chars() {
        depth += match c {
            '(' => 1,
            ')' => -1,
            _ => 0,
        };
        if depth < 0 {
            return None;
        }
    }
    (depth == 0).then_some(inner)
}

/// Finds the comparison operator outside parentheses.
fn split_relation(source: &str) -> Option<(&str, &str, &str)> {
    let bytes = source.as_bytes();
    let mut depth = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'<' | b'>' | b'=' if depth == 0 => {
                let width = if bytes.get(index + 1) == Some(&b'=') {
                    2
                } else {
                    1
                };
                return Some((
                    source[..index].trim(),
                    &source[index..index + width],
                    source[index + width..].trim(),
                ));
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn compile(source: &str) -> Result<CalculatorExpression, String> {
    CalculatorExpression::compile(source).map_err(|error| error.to_string())
}

fn uses_any(source: &str, names: &[&str]) -> bool {
    compile(source).is_ok_and(|expr| names.iter().any(|name| expr.uses_variable(name)))
}

fn is_identifier(source: &str) -> bool {
    let mut chars = source.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `name` or `name(args)`: the label part of `f(x) = …` or `A = (…)`.
fn object_label(source: &str) -> Option<&str> {
    let name = source.split('(').next()?.trim();
    let rest = source[name.len()..].trim();
    (is_identifier(name) && (rest.is_empty() || (rest.starts_with('(') && rest.ends_with(')'))))
        .then_some(name)
}

fn classify_tuple(body: &str) -> Result<PlotKind, String> {
    let parts = split_top_level(body);
    for part in &parts {
        compile(part)?;
    }
    let has = |names: &[&str]| parts.iter().any(|part| uses_any(part, names));
    match parts.as_slice() {
        [px, py] if has(&["t"]) => Ok(PlotKind::Parametric(px.to_string(), py.to_string())),
        [px, py] if has(&["x", "y"]) => Ok(PlotKind::VectorField(px.to_string(), py.to_string())),
        [px, py] => Ok(PlotKind::Point(px.to_string(), py.to_string())),
        [px, py, pz] if has(&["t"]) => Ok(PlotKind::Parametric3(
            px.to_string(),
            py.to_string(),
            pz.to_string(),
        )),
        [px, py, pz] => Ok(PlotKind::Point3(
            px.to_string(),
            py.to_string(),
            pz.to_string(),
        )),
        _ => Err("use 2 or 3 coordinates, e.g. (cos(t), sin(t))".into()),
    }
}

/// Recognises an input line. `sliders` are names defined by other lines.
pub fn classify(source: &str) -> Result<PlotKind, String> {
    let source = normalize(source);
    if source.is_empty() {
        return Err("empty".into());
    }
    let Some((lhs, op, rhs)) = split_relation(&source) else {
        if let Some(body) = tuple_body(&source) {
            return classify_tuple(body);
        }
        compile(&source)?;
        return Ok(if uses_any(&source, &["y"]) {
            PlotKind::Implicit(source)
        } else {
            PlotKind::Function(source)
        });
    };
    if rhs.is_empty() || lhs.is_empty() {
        return Err("both sides of the relation need an expression".into());
    }
    if op != "=" {
        let comparison = match op {
            "<" => Comparison::Less,
            "<=" => Comparison::LessEqual,
            ">" => Comparison::Greater,
            _ => Comparison::GreaterEqual,
        };
        let difference = format!("({lhs})-({rhs})");
        compile(&difference)?;
        return Ok(PlotKind::Inequality(difference, comparison));
    }
    let rhs = rhs.to_string();
    match lhs.replace(' ', "").as_str() {
        "y" if !uses_any(&rhs, &["y"]) => {
            compile(&rhs)?;
            return Ok(PlotKind::Function(rhs));
        }
        "x" if !uses_any(&rhs, &["x"]) => {
            compile(&rhs)?;
            return Ok(PlotKind::FunctionOfY(rhs));
        }
        "r" => {
            compile(&rhs)?;
            return Ok(PlotKind::Polar(rhs));
        }
        "z" => {
            compile(&rhs)?;
            return Ok(PlotKind::Surface(rhs));
        }
        "dy/dx" | "y'" => {
            compile(&rhs)?;
            return Ok(PlotKind::SlopeField(rhs));
        }
        _ => {}
    }
    if let Some(name) = object_label(lhs)
        && !RESERVED.contains(&name)
    {
        if let Some(body) = tuple_body(&rhs) {
            return classify_tuple(body);
        }
        let expr = compile(&rhs)?;
        if lhs == name && expr.variables().is_empty() {
            let value = expr
                .evaluate(&Default::default())
                .map_err(|error| error.to_string())?;
            return Ok(PlotKind::Slider {
                name: name.to_owned(),
                value,
            });
        }
        return Ok(if expr.uses_variable("y") {
            PlotKind::Implicit(format!("({rhs})-y"))
        } else {
            PlotKind::Function(rhs)
        });
    }
    let difference = format!("({lhs})-({rhs})");
    compile(&difference)?;
    Ok(PlotKind::Implicit(difference))
}

/// Visible region in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: [f32; 2],
    pub y: [f32; 2],
}

/// Drawable output of one object, in world coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primitives {
    pub polylines: Vec<Vec<[f32; 2]>>,
    pub segments: Vec<[[f32; 2]; 2]>,
    /// Arrows from base to tip.
    pub arrows: Vec<[[f32; 2]; 2]>,
    pub points: Vec<[f32; 2]>,
    /// Shaded axis-aligned cells: min and max corners.
    pub cells: Vec<[[f32; 2]; 2]>,
}

/// An object ready for repeated sampling.
pub struct Plot {
    pub kind: PlotKind,
    bound: Vec<BoundExpression>,
    slots: usize,
}

/// Slider names and values, in slot order after [`BASE_VARIABLES`].
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub names: Vec<String>,
    pub values: Vec<f32>,
    pub time: f32,
}

impl Scope {
    fn slot_names(&self) -> Vec<&str> {
        BASE_VARIABLES
            .iter()
            .copied()
            .chain(self.names.iter().map(String::as_str))
            .collect()
    }

    fn frame(&self) -> Vec<f32> {
        let mut values = vec![0.0; BASE_VARIABLES.len()];
        values[TIME] = self.time;
        values.extend_from_slice(&self.values);
        values
    }
}

impl Plot {
    pub fn new(kind: PlotKind, scope: &Scope) -> Result<Self, CalculatorError> {
        let slots = scope.slot_names();
        let sources: Vec<&String> = match &kind {
            PlotKind::Function(a)
            | PlotKind::FunctionOfY(a)
            | PlotKind::Polar(a)
            | PlotKind::Implicit(a)
            | PlotKind::Inequality(a, _)
            | PlotKind::Surface(a)
            | PlotKind::SlopeField(a) => vec![a],
            PlotKind::Parametric(a, b) | PlotKind::Point(a, b) | PlotKind::VectorField(a, b) => {
                vec![a, b]
            }
            PlotKind::Parametric3(a, b, c) | PlotKind::Point3(a, b, c) => vec![a, b, c],
            PlotKind::Slider { .. } => vec![],
        };
        let bound = sources
            .into_iter()
            .map(|source| CalculatorExpression::compile(source)?.bind(&slots))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            kind,
            bound,
            slots: slots.len(),
        })
    }

    fn values(&self, scope: &Scope) -> Vec<f32> {
        let mut values = scope.frame();
        values.resize(self.slots, 0.0);
        values
    }

    /// Value of a function of x (or y, or theta) at `input`.
    pub fn evaluate_function(&self, scope: &Scope, input: f32) -> f32 {
        let mut values = self.values(scope);
        let slot = match self.kind {
            PlotKind::FunctionOfY(_) => Y,
            PlotKind::Polar(_) => THETA,
            _ => X,
        };
        values[slot] = input;
        self.bound
            .first()
            .map_or(f32::NAN, |expr| expr.eval(&values))
    }

    /// Samples the object over `view`. `resolution` is the plot width in
    /// pixels; `parameter` is the t range for parametric curves.
    pub fn sample(
        &self,
        scope: &Scope,
        view: View,
        resolution: usize,
        parameter: [f32; 2],
    ) -> Primitives {
        let mut values = self.values(scope);
        let mut out = Primitives::default();
        let [x0, x1] = view.x;
        let [y0, y1] = view.y;
        let span_y = (y1 - y0).abs().max(f32::EPSILON);
        let span_x = (x1 - x0).abs().max(f32::EPSILON);
        let samples = resolution.clamp(64, 2400);
        match &self.kind {
            PlotKind::Function(_) | PlotKind::FunctionOfY(_) => {
                let of_y = matches!(self.kind, PlotKind::FunctionOfY(_));
                let (input_slot, [a, b], span) = if of_y {
                    (Y, view.y, span_x)
                } else {
                    (X, view.x, span_y)
                };
                let mut line = Vec::new();
                let mut previous: Option<f32> = None;
                for i in 0..=samples {
                    let input = a + (b - a) * i as f32 / samples as f32;
                    values[input_slot] = input;
                    let output = self.bound[0].eval(&values);
                    // Break at undefined points and at asymptotes, where the
                    // value leaps across the view between samples.
                    let jump = previous.is_some_and(|p| (output - p).abs() > span * 2.0);
                    if !output.is_finite() || jump {
                        if line.len() > 1 {
                            out.polylines.push(std::mem::take(&mut line));
                        }
                        line.clear();
                    }
                    if output.is_finite() {
                        line.push(if of_y {
                            [output, input]
                        } else {
                            [input, output]
                        });
                        previous = Some(output);
                    } else {
                        previous = None;
                    }
                }
                if line.len() > 1 {
                    out.polylines.push(line);
                }
            }
            PlotKind::Polar(_) => {
                let polar_samples = samples * 2;
                let mut line = Vec::new();
                for i in 0..=polar_samples {
                    let theta = parameter[0]
                        + (parameter[1] - parameter[0]) * i as f32 / polar_samples as f32;
                    values[THETA] = theta;
                    values[T] = theta;
                    let r = self.bound[0].eval(&values);
                    if r.is_finite() {
                        line.push([r * theta.cos(), r * theta.sin()]);
                    } else if line.len() > 1 {
                        out.polylines.push(std::mem::take(&mut line));
                    } else {
                        line.clear();
                    }
                }
                if line.len() > 1 {
                    out.polylines.push(line);
                }
            }
            PlotKind::Parametric(..) => {
                let mut line = Vec::new();
                for i in 0..=samples {
                    values[T] =
                        parameter[0] + (parameter[1] - parameter[0]) * i as f32 / samples as f32;
                    let point = [self.bound[0].eval(&values), self.bound[1].eval(&values)];
                    if point.iter().all(|v| v.is_finite()) {
                        line.push(point);
                    } else if line.len() > 1 {
                        out.polylines.push(std::mem::take(&mut line));
                    } else {
                        line.clear();
                    }
                }
                if line.len() > 1 {
                    out.polylines.push(line);
                }
            }
            PlotKind::Point(..) => {
                let point = [self.bound[0].eval(&values), self.bound[1].eval(&values)];
                if point.iter().all(|v| v.is_finite()) {
                    out.points.push(point);
                }
            }
            PlotKind::Implicit(_) => {
                out.segments = marching_squares(
                    &self.bound[0],
                    &mut values,
                    view,
                    (samples / 5).clamp(48, 240),
                );
            }
            PlotKind::Inequality(_, comparison) => {
                let columns = (samples / 6).clamp(40, 200);
                let rows = (columns as f32 * span_y / span_x)
                    .round()
                    .clamp(24.0, 200.0) as usize;
                let (cw, ch) = (span_x / columns as f32, span_y / rows as f32);
                for row in 0..rows {
                    for column in 0..columns {
                        values[X] = x0 + (column as f32 + 0.5) * cw;
                        values[Y] = y0 + (row as f32 + 0.5) * ch;
                        if comparison.holds(self.bound[0].eval(&values)) {
                            let min = [x0 + column as f32 * cw, y0 + row as f32 * ch];
                            out.cells.push([min, [min[0] + cw, min[1] + ch]]);
                        }
                    }
                }
                // Draw the boundary too, as GeoGebra does.
                out.segments = marching_squares(
                    &self.bound[0],
                    &mut values,
                    view,
                    (samples / 5).clamp(48, 240),
                );
            }
            PlotKind::VectorField(..) => {
                let (columns, rows) = field_grid(view);
                let cell = (span_x / columns as f32).min(span_y / rows as f32);
                let mut vectors = Vec::with_capacity(columns * rows);
                for row in 0..rows {
                    for column in 0..columns {
                        let x = x0 + (column as f32 + 0.5) * span_x / columns as f32;
                        let y = y0 + (row as f32 + 0.5) * span_y / rows as f32;
                        values[X] = x;
                        values[Y] = y;
                        let v = [self.bound[0].eval(&values), self.bound[1].eval(&values)];
                        if v.iter().all(|c| c.is_finite()) {
                            vectors.push(([x, y], v));
                        }
                    }
                }
                let longest = vectors
                    .iter()
                    .map(|(_, v)| v[0].hypot(v[1]))
                    .fold(0.0_f32, f32::max)
                    .max(f32::EPSILON);
                for ([x, y], v) in vectors {
                    let scale = 0.85 * cell / longest;
                    out.arrows
                        .push([[x, y], [x + v[0] * scale, y + v[1] * scale]]);
                }
                out.polylines = self.streamlines(&mut values, view);
            }
            PlotKind::SlopeField(_) => {
                let (columns, rows) = field_grid(view);
                let half = 0.4 * (span_x / columns as f32).min(span_y / rows as f32);
                for row in 0..rows {
                    for column in 0..columns {
                        let x = x0 + (column as f32 + 0.5) * span_x / columns as f32;
                        let y = y0 + (row as f32 + 0.5) * span_y / rows as f32;
                        values[X] = x;
                        values[Y] = y;
                        let slope = self.bound[0].eval(&values);
                        if slope.is_finite() {
                            let angle = slope.atan();
                            let (dx, dy) = (angle.cos() * half, angle.sin() * half);
                            out.segments.push([[x - dx, y - dy], [x + dx, y + dy]]);
                        }
                    }
                }
                out.polylines = self.streamlines(&mut values, view);
            }
            PlotKind::Slider { .. }
            | PlotKind::Surface(_)
            | PlotKind::Parametric3(..)
            | PlotKind::Point3(..) => {}
        }
        out
    }

    /// Velocity at a point for fields: (P, Q) or (1, dy/dx).
    fn velocity(&self, values: &mut [f32], point: [f32; 2]) -> Option<[f32; 2]> {
        values[X] = point[0];
        values[Y] = point[1];
        let v = match self.kind {
            PlotKind::VectorField(..) => [self.bound[0].eval(values), self.bound[1].eval(values)],
            _ => [1.0, self.bound[0].eval(values)],
        };
        v.iter().all(|c| c.is_finite()).then_some(v)
    }

    /// Fourth-order Runge–Kutta trajectories from a grid of seeds, traced
    /// forwards and backwards in arc-length steps until they leave the view.
    fn streamlines(&self, values: &mut [f32], view: View) -> Vec<Vec<[f32; 2]>> {
        let span = (view.x[1] - view.x[0]).max(view.y[1] - view.y[0]);
        let h = span / 240.0;
        let inside = |p: [f32; 2]| {
            p[0] >= view.x[0] && p[0] <= view.x[1] && p[1] >= view.y[0] && p[1] <= view.y[1]
        };
        let mut lines = Vec::new();
        for sy in 0..4 {
            for sx in 0..5 {
                let seed = [
                    view.x[0] + (sx as f32 + 0.5) / 5.0 * (view.x[1] - view.x[0]),
                    view.y[0] + (sy as f32 + 0.5) / 4.0 * (view.y[1] - view.y[0]),
                ];
                for direction in [1.0, -1.0] {
                    let mut line = vec![seed];
                    let mut p = seed;
                    for _ in 0..600 {
                        // Unit-speed direction field, so steps are uniform.
                        let mut unit = |q: [f32; 2]| {
                            self.velocity(values, q).and_then(|v| {
                                let length = v[0].hypot(v[1]);
                                (length > 1e-6)
                                    .then(|| [direction * v[0] / length, direction * v[1] / length])
                            })
                        };
                        let Some(k1) = unit(p) else { break };
                        let Some(k2) = unit([p[0] + 0.5 * h * k1[0], p[1] + 0.5 * h * k1[1]])
                        else {
                            break;
                        };
                        let Some(k3) = unit([p[0] + 0.5 * h * k2[0], p[1] + 0.5 * h * k2[1]])
                        else {
                            break;
                        };
                        let Some(k4) = unit([p[0] + h * k3[0], p[1] + h * k3[1]]) else {
                            break;
                        };
                        p = [
                            p[0] + h / 6.0 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]),
                            p[1] + h / 6.0 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]),
                        ];
                        if !inside(p) {
                            break;
                        }
                        line.push(p);
                    }
                    if line.len() > 2 {
                        lines.push(line);
                    }
                }
            }
        }
        lines
    }

    /// Samples a 3D object: surface grid rows and columns, a 3D polyline, or
    /// a single point. `domain` is the x/y range used for surfaces.
    pub fn sample_3d(
        &self,
        scope: &Scope,
        domain: View,
        steps: usize,
        parameter: [f32; 2],
    ) -> Sampled3 {
        let mut values = self.values(scope);
        let mut out = Sampled3::default();
        match &self.kind {
            PlotKind::Surface(_) => {
                let steps = steps.clamp(4, 96);
                out.grid_steps = steps;
                for row in 0..=steps {
                    for column in 0..=steps {
                        let x = domain.x[0]
                            + (domain.x[1] - domain.x[0]) * column as f32 / steps as f32;
                        let y =
                            domain.y[0] + (domain.y[1] - domain.y[0]) * row as f32 / steps as f32;
                        values[X] = x;
                        values[Y] = y;
                        out.grid.push([x, y, self.bound[0].eval(&values)]);
                    }
                }
            }
            PlotKind::Parametric3(..) => {
                let samples = steps.clamp(16, 2000) * 8;
                for i in 0..=samples {
                    values[T] =
                        parameter[0] + (parameter[1] - parameter[0]) * i as f32 / samples as f32;
                    out.curve.push([
                        self.bound[0].eval(&values),
                        self.bound[1].eval(&values),
                        self.bound[2].eval(&values),
                    ]);
                }
            }
            PlotKind::Point3(..) => out.curve.push([
                self.bound[0].eval(&values),
                self.bound[1].eval(&values),
                self.bound[2].eval(&values),
            ]),
            _ => {}
        }
        out
    }
}

/// 3D samples; non-finite coordinates mark undefined points.
#[derive(Clone, Debug, Default)]
pub struct Sampled3 {
    /// Surface heights, `(grid_steps + 1)^2` points in row-major order.
    pub grid: Vec<[f32; 3]>,
    pub grid_steps: usize,
    pub curve: Vec<[f32; 3]>,
}

fn field_grid(view: View) -> (usize, usize) {
    let aspect =
        ((view.y[1] - view.y[0]) / (view.x[1] - view.x[0]).max(f32::EPSILON)).clamp(0.2, 5.0);
    let columns = 22;
    (
        columns,
        ((columns as f32 * aspect).round() as usize).clamp(6, 40),
    )
}

/// Zero contour of `f(x, y)` as line segments, with linear interpolation
/// along each crossed cell edge.
fn marching_squares(
    f: &BoundExpression,
    values: &mut [f32],
    view: View,
    columns: usize,
) -> Vec<[[f32; 2]; 2]> {
    let span_x = view.x[1] - view.x[0];
    let span_y = view.y[1] - view.y[0];
    let rows =
        ((columns as f32 * span_y / span_x.max(f32::EPSILON)).round() as usize).clamp(16, 240);
    let (cw, ch) = (span_x / columns as f32, span_y / rows as f32);
    let mut grid = vec![f32::NAN; (columns + 1) * (rows + 1)];
    for row in 0..=rows {
        for column in 0..=columns {
            values[X] = view.x[0] + column as f32 * cw;
            values[Y] = view.y[0] + row as f32 * ch;
            grid[row * (columns + 1) + column] = f.eval(values);
        }
    }
    let at = |column: usize, row: usize| grid[row * (columns + 1) + column];
    let mut segments = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            // Corners counter-clockwise from bottom-left.
            let corners = [
                (column, row),
                (column + 1, row),
                (column + 1, row + 1),
                (column, row + 1),
            ];
            let v = corners.map(|(c, r)| at(c, r));
            if v.iter().any(|value| !value.is_finite()) {
                continue;
            }
            // Ignore sign flips across poles (e.g. 1/x), which are not zeros.
            let limit = v.iter().fold(0.0_f32, |m, value| m.max(value.abs()));
            let position =
                |(c, r): (usize, usize)| [view.x[0] + c as f32 * cw, view.y[0] + r as f32 * ch];
            let mut crossings = Vec::with_capacity(4);
            for edge in 0..4 {
                let (a, b) = (edge, (edge + 1) % 4);
                if (v[a] < 0.0) != (v[b] < 0.0) {
                    let t = v[a] / (v[a] - v[b]);
                    let (pa, pb) = (position(corners[a]), position(corners[b]));
                    crossings.push([pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t]);
                }
            }
            let pole = limit > 1e4 * (span_x + span_y);
            match crossings.as_slice() {
                [a, b] if !pole => segments.push([*a, *b]),
                // Saddle cell: pair crossings by the cell-centre sign.
                [a, b, c, d] if !pole => {
                    let centre = v.iter().sum::<f32>() / 4.0;
                    if (centre < 0.0) == (v[0] < 0.0) {
                        segments.push([*a, *d]);
                        segments.push([*b, *c]);
                    } else {
                        segments.push([*a, *b]);
                        segments.push([*c, *d]);
                    }
                }
                _ => {}
            }
        }
    }
    segments
}

/// Roots of a function of x inside `[a, b]`, by sign change and bisection.
pub fn roots(f: impl Fn(f32) -> f32, [a, b]: [f32; 2], samples: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let step = (b - a) / samples as f32;
    let mut x0 = a;
    let mut y0 = f(x0);
    for i in 1..=samples {
        let x1 = a + step * i as f32;
        let y1 = f(x1);
        if y0 == 0.0 {
            out.push(x0);
        } else if y0.is_finite() && y1.is_finite() && (y0 < 0.0) != (y1 < 0.0) {
            let (mut lo, mut hi, mut flo) = (x0, x1, y0);
            for _ in 0..48 {
                let mid = 0.5 * (lo + hi);
                let fm = f(mid);
                if (fm < 0.0) == (flo < 0.0) {
                    lo = mid;
                    flo = fm;
                } else {
                    hi = mid;
                }
            }
            let root = 0.5 * (lo + hi);
            // A sign change across a pole is not a root.
            if f(root).abs() < (y0.abs() + y1.abs()) * 0.5 {
                out.push(root);
            }
        }
        x0 = x1;
        y0 = y1;
    }
    out.dedup_by(|a, b| (*a - *b).abs() < step * 0.5);
    out
}

/// Central-difference derivative with a step scaled to `x`.
pub fn derivative(f: impl Fn(f32) -> f32, x: f32) -> f32 {
    let h = 1e-3 * x.abs().max(1.0);
    (f(x + h) - f(x - h)) / (2.0 * h)
}

/// Local minima and maxima: roots of the derivative, with their values.
pub fn extrema(f: impl Fn(f32) -> f32 + Copy, range: [f32; 2], samples: usize) -> Vec<[f32; 2]> {
    roots(|x| derivative(f, x), range, samples)
        .into_iter()
        .map(|x| [x, f(x)])
        .filter(|point| point[1].is_finite())
        .collect()
}

/// Composite Simpson's rule; NaN where the function is undefined.
pub fn integral(f: impl Fn(f32) -> f32, [a, b]: [f32; 2], intervals: usize) -> f32 {
    let n = intervals.max(2) & !1;
    let h = (b - a) / n as f32;
    let mut sum = f(a) + f(b);
    for i in 1..n {
        sum += f(a + h * i as f32) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    sum * h / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(source: &str) -> PlotKind {
        classify(source).unwrap_or_else(|error| panic!("{source}: {error}"))
    }

    #[test]
    fn classifies_every_object_type() {
        assert_eq!(kind("sin(x)"), PlotKind::Function("sin(x)".into()));
        assert_eq!(kind("y = x²"), PlotKind::Function("x^2".into()));
        assert_eq!(kind("f(x) = 2x"), PlotKind::Function("2x".into()));
        assert_eq!(kind("x = y^2"), PlotKind::FunctionOfY("y^2".into()));
        assert_eq!(
            kind("r = 1 + cos(θ)"),
            PlotKind::Polar("1 + cos(theta)".into())
        );
        assert!(matches!(kind("(cos(t), sin(t))"), PlotKind::Parametric(..)));
        assert!(matches!(kind("(2, 3)"), PlotKind::Point(..)));
        assert!(matches!(kind("A = (a, 1)"), PlotKind::Point(..)));
        assert!(matches!(
            kind("(cos(t), sin(t), t/4)"),
            PlotKind::Parametric3(..)
        ));
        assert!(matches!(kind("(1, 2, 3)"), PlotKind::Point3(..)));
        assert!(matches!(kind("x^2 + y^2 = 9"), PlotKind::Implicit(_)));
        assert!(matches!(
            kind("y < sin(x)"),
            PlotKind::Inequality(_, Comparison::Less)
        ));
        assert!(matches!(
            kind("x^2 + y^2 ≤ 4"),
            PlotKind::Inequality(_, Comparison::LessEqual)
        ));
        assert_eq!(
            kind("a = 2"),
            PlotKind::Slider {
                name: "a".into(),
                value: 2.0
            }
        );
        assert_eq!(
            kind("z = sin(x)*cos(y)"),
            PlotKind::Surface("sin(x)*cos(y)".into())
        );
        assert!(matches!(kind("(y, -x)"), PlotKind::VectorField(..)));
        assert!(matches!(kind("F = (-y, x)"), PlotKind::VectorField(..)));
        assert_eq!(kind("dy/dx = x - y"), PlotKind::SlopeField("x - y".into()));
        assert!(classify("sin(").is_err());
        assert!(classify("(1, 2, 3, 4)").is_err());
        assert!(classify("y =").is_err());
    }

    fn scope(sliders: &[(&str, f32)]) -> Scope {
        Scope {
            names: sliders.iter().map(|(name, _)| name.to_string()).collect(),
            values: sliders.iter().map(|(_, value)| *value).collect(),
            time: 0.0,
        }
    }

    const VIEW: View = View {
        x: [-5.0, 5.0],
        y: [-5.0, 5.0],
    };

    #[test]
    fn functions_use_sliders_and_break_at_asymptotes() {
        let plot = Plot::new(kind("a x"), &scope(&[("a", 2.0)])).unwrap();
        assert_eq!(plot.evaluate_function(&scope(&[("a", 2.0)]), 3.0), 6.0);
        let tan = Plot::new(kind("tan(x)"), &Scope::default()).unwrap();
        let sampled = tan.sample(&Scope::default(), VIEW, 400, [0.0, 1.0]);
        assert!(sampled.polylines.len() >= 3, "tan splits at each asymptote");
        let sqrt = Plot::new(kind("sqrt(x)"), &Scope::default()).unwrap();
        let sampled = sqrt.sample(&Scope::default(), VIEW, 400, [0.0, 1.0]);
        assert_eq!(sampled.polylines.len(), 1);
        assert!(sampled.polylines[0].iter().all(|p| p[0] >= -0.03));
        assert!(
            Plot::new(kind("b x"), &Scope::default()).is_err(),
            "unknown slider"
        );
    }

    #[test]
    fn implicit_circle_lies_on_its_radius() {
        let plot = Plot::new(kind("x^2 + y^2 = 9"), &Scope::default()).unwrap();
        let sampled = plot.sample(&Scope::default(), VIEW, 600, [0.0, 1.0]);
        assert!(sampled.segments.len() > 40);
        for [a, b] in sampled.segments {
            for p in [a, b] {
                assert!((p[0].hypot(p[1]) - 3.0).abs() < 0.08, "{p:?}");
            }
        }
    }

    #[test]
    fn inequalities_shade_the_right_side() {
        let plot = Plot::new(kind("y > x"), &Scope::default()).unwrap();
        let sampled = plot.sample(&Scope::default(), VIEW, 600, [0.0, 1.0]);
        assert!(!sampled.cells.is_empty());
        assert!(sampled.cells.iter().all(|[min, max]| {
            let centre = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
            centre[1] > centre[0]
        }));
    }

    #[test]
    fn parametric_polar_points_and_fields() {
        let circle = Plot::new(kind("(cos(t), sin(t))"), &Scope::default()).unwrap();
        let sampled = circle.sample(&Scope::default(), VIEW, 400, [0.0, std::f32::consts::TAU]);
        assert!(
            sampled.polylines[0]
                .iter()
                .all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-4)
        );
        let polar = Plot::new(kind("r = 2"), &Scope::default()).unwrap();
        let sampled = polar.sample(&Scope::default(), VIEW, 400, [0.0, std::f32::consts::TAU]);
        assert!(
            sampled.polylines[0]
                .iter()
                .all(|p| (p[0].hypot(p[1]) - 2.0).abs() < 1e-4)
        );
        let point = Plot::new(kind("(a, 1)"), &scope(&[("a", 4.0)])).unwrap();
        assert_eq!(
            point
                .sample(&scope(&[("a", 4.0)]), VIEW, 400, [0.0, 1.0])
                .points,
            [[4.0, 1.0]]
        );
        // A rotational field's streamlines stay on circles around the origin.
        let field = Plot::new(kind("(-y, x)"), &Scope::default()).unwrap();
        let sampled = field.sample(&Scope::default(), VIEW, 400, [0.0, 1.0]);
        assert!(!sampled.arrows.is_empty());
        for line in &sampled.polylines {
            let r0 = line[0][0].hypot(line[0][1]);
            assert!(
                line.iter()
                    .all(|p| (p[0].hypot(p[1]) - r0).abs() < 0.02 * r0.max(1.0))
            );
        }
        let slope = Plot::new(kind("dy/dx = 1"), &Scope::default()).unwrap();
        let sampled = slope.sample(&Scope::default(), VIEW, 400, [0.0, 1.0]);
        assert!(
            sampled
                .segments
                .iter()
                .all(|[a, b]| ((b[1] - a[1]) - (b[0] - a[0])).abs() < 1e-4)
        );
    }

    #[test]
    fn surfaces_and_3d_curves_sample_their_domains() {
        let surface = Plot::new(kind("z = x + y"), &Scope::default()).unwrap();
        let sampled = surface.sample_3d(&Scope::default(), VIEW, 10, [0.0, 1.0]);
        assert_eq!(sampled.grid.len(), 121);
        assert!(
            sampled
                .grid
                .iter()
                .all(|[x, y, z]| (x + y - z).abs() < 1e-5)
        );
        let helix = Plot::new(kind("(cos(t), sin(t), t)"), &Scope::default()).unwrap();
        let sampled = helix.sample_3d(&Scope::default(), VIEW, 20, [0.0, 1.0]);
        assert_eq!(sampled.curve.first(), Some(&[1.0, 0.0, 0.0]));
    }

    #[test]
    fn calculus_finds_roots_extrema_and_areas() {
        let roots_found = roots(|x| x * x - 2.0, [-3.0, 3.0], 300);
        assert_eq!(roots_found.len(), 2);
        assert!((roots_found[1] - 2.0_f32.sqrt()).abs() < 1e-4);
        assert!(
            roots(|x| 1.0 / x, [-1.0, 1.0], 301).is_empty(),
            "pole is not a root"
        );
        let peaks = extrema(|x| x.sin(), [0.0, 6.0], 300);
        assert_eq!(peaks.len(), 2);
        assert!((peaks[0][0] - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        assert!((integral(|x| x * x, [0.0, 3.0], 64) - 9.0).abs() < 1e-3);
        assert!((derivative(|x| x * x * x, 2.0) - 12.0).abs() < 1e-2);
    }
}
