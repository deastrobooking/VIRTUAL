//! Safe, bounded, serializable node graphs for custom procedural geometry.
//!
//! Expressions are compiled once per graph evaluation and run over normalized
//! curve/surface coordinates. The language deliberately has no I/O, loops,
//! reflection, or user code execution.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Dimension, Geometry, RecursivePattern, Segment, Vec3};

pub const GEOMETRY_GRAPH_VERSION: u32 = 1;
pub const MAX_GEOMETRY_NODES: usize = 64;
pub const MAX_EXPRESSION_BYTES: usize = 512;
const MAX_STEPS: u32 = 4096;

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct GraphNodeId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryGraph {
    pub version: u32,
    pub revision: u64,
    pub name: String,
    pub nodes: Vec<GeometryNode>,
    pub output: GraphNodeId,
    #[serde(default)]
    pub parameters: Vec<GeometryParameter>,
}

impl Default for GeometryGraph {
    fn default() -> Self {
        Self {
            version: GEOMETRY_GRAPH_VERSION,
            revision: 1,
            name: "Untitled Geometry".into(),
            nodes: vec![
                GeometryNode::new(
                    1,
                    "Circle",
                    GeometryNodeKind::Curve {
                        x: "cos(tau*t)".into(),
                        y: "sin(tau*t)".into(),
                        z: "0".into(),
                        steps: 256,
                        closed: true,
                    },
                ),
                GeometryNode::new(2, "Output", GeometryNodeKind::Output).with_input(1),
            ],
            output: GraphNodeId(2),
            parameters: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryParameter {
    pub name: String,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryNode {
    pub id: GraphNodeId,
    pub name: String,
    #[serde(default = "default_node_position")]
    pub position: [f32; 2],
    #[serde(default)]
    pub inputs: Vec<GraphNodeId>,
    pub kind: GeometryNodeKind,
}

fn default_node_position() -> [f32; 2] {
    [40.0, 40.0]
}

impl GeometryNode {
    pub fn new(id: u64, name: impl Into<String>, kind: GeometryNodeKind) -> Self {
        Self {
            id: GraphNodeId(id),
            name: name.into(),
            position: [
                40.0 + ((id.saturating_sub(1) % 4) as f32 * 190.0),
                40.0 + (((id.saturating_sub(1) / 4) % 4) as f32 * 100.0),
            ],
            inputs: Vec::new(),
            kind,
        }
    }
    pub fn with_input(mut self, id: u64) -> Self {
        self.inputs.push(GraphNodeId(id));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeometryNodeKind {
    /// Parametric curve. `t` runs from 0 to 1, inclusive.
    Curve {
        x: String,
        y: String,
        z: String,
        steps: u32,
        closed: bool,
    },
    /// Parametric surface wireframe. `u` and `v` run from 0 to 1.
    Surface {
        x: String,
        y: String,
        z: String,
        u_steps: u32,
        v_steps: u32,
    },
    Transform {
        translate: [String; 3],
        rotate: [String; 3],
        scale: [String; 3],
    },
    Twist {
        amount: String,
    },
    RadialRepeat {
        copies: u8,
        axis: GraphAxis,
    },
    Noise {
        amount: String,
        frequency: String,
        seed: u32,
    },
    Merge,
    Output,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAxis {
    X,
    #[default]
    Y,
    Z,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum GeometryGraphError {
    #[error("graph has more than {MAX_GEOMETRY_NODES} nodes")]
    TooManyNodes,
    #[error("node id {0} is duplicated")]
    DuplicateNode(u64),
    #[error("node {0} references missing input {1}")]
    MissingInput(u64, u64),
    #[error("graph contains a cycle at node {0}")]
    Cycle(u64),
    #[error("output node {0} does not exist")]
    MissingOutput(u64),
    #[error("expression error in node {node}: {message}")]
    Expression { node: u64, message: String },
    #[error(transparent)]
    Calculator(#[from] CalculatorError),
    #[error("geometry graph version {0} is unsupported")]
    UnsupportedVersion(u32),
    #[error("geometry graph has no output segments")]
    Empty,
    #[error("graph node {node} has an invalid number of inputs")]
    InputCount { node: u64 },
    #[error("graph parameter {0} is invalid")]
    InvalidParameter(String),
}

pub fn validate_geometry_graph(graph: &GeometryGraph) -> Result<(), GeometryGraphError> {
    if graph.version != GEOMETRY_GRAPH_VERSION {
        return Err(GeometryGraphError::UnsupportedVersion(graph.version));
    }
    if graph.nodes.len() > MAX_GEOMETRY_NODES {
        return Err(GeometryGraphError::TooManyNodes);
    }
    if graph.parameters.len() > 32 {
        return Err(GeometryGraphError::InvalidParameter(
            "too many parameters".into(),
        ));
    }
    for parameter in &graph.parameters {
        if parameter.name.is_empty()
            || parameter.name.len() > 32
            || !parameter
                .name
                .bytes()
                .enumerate()
                .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
            || !parameter.default.is_finite()
            || !parameter.min.is_finite()
            || !parameter.max.is_finite()
            || parameter.min > parameter.max
            || !(parameter.min..=parameter.max).contains(&parameter.default)
        {
            return Err(GeometryGraphError::InvalidParameter(parameter.name.clone()));
        }
    }
    let mut ids = HashSet::new();
    for node in &graph.nodes {
        if !ids.insert(node.id) {
            return Err(GeometryGraphError::DuplicateNode(node.id.0));
        }
    }
    if !ids.contains(&graph.output) {
        return Err(GeometryGraphError::MissingOutput(graph.output.0));
    }
    for node in &graph.nodes {
        let input_count = node.inputs.len();
        let invalid_count = match &node.kind {
            GeometryNodeKind::Curve { .. } | GeometryNodeKind::Surface { .. } => input_count != 0,
            GeometryNodeKind::Output
            | GeometryNodeKind::Transform { .. }
            | GeometryNodeKind::Twist { .. }
            | GeometryNodeKind::RadialRepeat { .. }
            | GeometryNodeKind::Noise { .. } => input_count != 1,
            GeometryNodeKind::Merge => input_count == 0,
        };
        if invalid_count {
            return Err(GeometryGraphError::InputCount { node: node.id.0 });
        }
        for input in &node.inputs {
            if !ids.contains(input) {
                return Err(GeometryGraphError::MissingInput(node.id.0, input.0));
            }
        }
        for expression in node_expressions(&node.kind) {
            let compiled = CalculatorExpression::compile(expression).map_err(|e| {
                GeometryGraphError::Expression {
                    node: node.id.0,
                    message: e.to_string(),
                }
            })?;
            for name in compiled.variables() {
                let built_in = [
                    "x", "y", "z", "t", "u", "v", "time", "bass", "mid", "high", "seed", "pi",
                    "tau", "e",
                ];
                if !built_in.contains(&name.as_str())
                    && !graph
                        .parameters
                        .iter()
                        .any(|parameter| parameter.name == name)
                {
                    return Err(GeometryGraphError::Expression {
                        node: node.id.0,
                        message: format!("unknown variable {name}"),
                    });
                }
            }
        }
    }
    let by_id: HashMap<_, _> = graph.nodes.iter().map(|n| (n.id, n)).collect();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for id in ids {
        visit(id, &by_id, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit(
    id: GraphNodeId,
    nodes: &HashMap<GraphNodeId, &GeometryNode>,
    visiting: &mut HashSet<GraphNodeId>,
    visited: &mut HashSet<GraphNodeId>,
) -> Result<(), GeometryGraphError> {
    if visited.contains(&id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(GeometryGraphError::Cycle(id.0));
    }
    let node = nodes[&id];
    for input in &node.inputs {
        visit(*input, nodes, visiting, visited)?;
    }
    visiting.remove(&id);
    visited.insert(id);
    Ok(())
}

fn node_expressions(kind: &GeometryNodeKind) -> Vec<&str> {
    match kind {
        GeometryNodeKind::Curve { x, y, z, .. } | GeometryNodeKind::Surface { x, y, z, .. } => {
            vec![x, y, z]
        }
        GeometryNodeKind::Transform {
            translate,
            rotate,
            scale,
        } => translate
            .iter()
            .chain(rotate)
            .chain(scale)
            .map(String::as_str)
            .collect(),
        GeometryNodeKind::Twist { amount } => vec![amount],
        GeometryNodeKind::Noise {
            amount, frequency, ..
        } => vec![amount, frequency],
        GeometryNodeKind::RadialRepeat { .. }
        | GeometryNodeKind::Merge
        | GeometryNodeKind::Output => Vec::new(),
    }
}

/// Evaluates the graph into line segments. Every node and expression has a
/// fixed bound; graph content cannot allocate beyond `budget` output segments.
pub fn evaluate_geometry_graph(
    graph: &GeometryGraph,
    time: f32,
    params: &HashMap<String, f32>,
    budget: usize,
) -> Result<Geometry, GeometryGraphError> {
    validate_geometry_graph(graph)?;
    let by_id: HashMap<_, _> = graph.nodes.iter().map(|n| (n.id, n)).collect();
    let mut cache = HashMap::<GraphNodeId, Vec<Segment>>::new();
    let mut active = HashSet::new();
    let segments = evaluate_node(
        graph.output,
        &by_id,
        &mut cache,
        &mut active,
        time,
        params,
        budget.min(crate::MAX_SEGMENTS),
    )?;
    if segments.is_empty() {
        return Err(GeometryGraphError::Empty);
    }
    let (center, radius) = graph_bounds(&segments);
    let planar = segments
        .iter()
        .all(|s| s.start.z.abs() < 1e-6 && s.end.z.abs() < 1e-6);
    Ok(Geometry {
        segments,
        pattern: RecursivePattern::FractalTree,
        dimension: Some(if planar {
            Dimension::Planar
        } else {
            Dimension::Spatial
        }),
        requested_depth: 1,
        depth: 1,
        truncated: false,
        center,
        radius,
    })
}

fn evaluate_node(
    id: GraphNodeId,
    nodes: &HashMap<GraphNodeId, &GeometryNode>,
    cache: &mut HashMap<GraphNodeId, Vec<Segment>>,
    active: &mut HashSet<GraphNodeId>,
    time: f32,
    params: &HashMap<String, f32>,
    budget: usize,
) -> Result<Vec<Segment>, GeometryGraphError> {
    if let Some(result) = cache.get(&id) {
        return Ok(result.clone());
    }
    if !active.insert(id) {
        return Err(GeometryGraphError::Cycle(id.0));
    }
    let node = nodes
        .get(&id)
        .ok_or(GeometryGraphError::MissingOutput(id.0))?;
    let inputs: Vec<Vec<Segment>> = node
        .inputs
        .iter()
        .map(|input| evaluate_node(*input, nodes, cache, active, time, params, budget))
        .collect::<Result<_, _>>()?;
    let expr = |text: &str| {
        CalculatorExpression::compile(text).map_err(|e| GeometryGraphError::Expression {
            node: id.0,
            message: e.to_string(),
        })
    };
    let result = match &node.kind {
        GeometryNodeKind::Curve {
            x,
            y,
            z,
            steps,
            closed,
        } => {
            let [x, y, z] = [expr(x)?, expr(y)?, expr(z)?];
            let count = (*steps)
                .clamp(2, MAX_STEPS)
                .min(budget.saturating_add(1) as u32);
            let mut points = Vec::with_capacity(count as usize);
            for i in 0..count {
                let t = i as f32 / (count - 1).max(1) as f32;
                let c = context(t, 0.0, 0.0, 0.0, 0.0, 0.0, time, 0.0, params);
                points.push(Vec3::new(x.evaluate(&c)?, y.evaluate(&c)?, z.evaluate(&c)?));
            }
            points_to_segments(&points, *closed, budget)
        }
        GeometryNodeKind::Surface {
            x,
            y,
            z,
            u_steps,
            v_steps,
        } => {
            let [x, y, z] = [expr(x)?, expr(y)?, expr(z)?];
            let u = (*u_steps).clamp(2, MAX_STEPS);
            let v = (*v_steps).clamp(2, MAX_STEPS);
            let point_count = (u as usize)
                .saturating_mul(v as usize)
                .min(budget.saturating_add(u as usize + v as usize));
            let v_count = (point_count / u as usize).max(2).min(v as usize);
            let u_count = (point_count / v_count).max(2).min(u as usize);
            let mut points = Vec::with_capacity(u_count * v_count);
            for ui in 0..u_count {
                let up = ui as f32 / (u_count - 1) as f32;
                for vi in 0..v_count {
                    let vp = vi as f32 / (v_count - 1) as f32;
                    let c = context(0.0, 0.0, 0.0, 0.0, up, vp, time, 0.0, params);
                    points.push(Vec3::new(x.evaluate(&c)?, y.evaluate(&c)?, z.evaluate(&c)?));
                }
            }
            let mut out = Vec::with_capacity(budget);
            for ui in 0..u_count {
                for vi in 0..v_count {
                    let index = ui * v_count + vi;
                    if vi + 1 < v_count && out.len() < budget {
                        out.push(segment(points[index], points[index + 1], out.len(), budget));
                    }
                    if ui + 1 < u_count && out.len() < budget {
                        out.push(segment(
                            points[index],
                            points[index + v_count],
                            out.len(),
                            budget,
                        ));
                    }
                }
            }
            out
        }
        GeometryNodeKind::Transform {
            translate,
            rotate,
            scale,
        } => {
            let input = inputs.first().cloned().unwrap_or_default();
            let t = compile3(translate, &expr)?;
            let r = compile3(rotate, &expr)?;
            let s = compile3(scale, &expr)?;
            input
                .into_iter()
                .take(budget)
                .map(|seg| {
                    let apply = |p: Vec3| -> Result<Vec3, GeometryGraphError> {
                        let c = context(seg.t, p.x, p.y, p.z, 0.0, 0.0, time, 0.0, params);
                        let tr = eval3(&t, &c)?;
                        let rot = eval3(&r, &c)?;
                        let sc = eval3(&s, &c)?;
                        Ok(rotate_point(Vec3::new(p.x * sc.x, p.y * sc.y, p.z * sc.z), rot) + tr)
                    };
                    Ok(Segment {
                        start: apply(seg.start)?,
                        end: apply(seg.end)?,
                        ..seg
                    })
                })
                .collect::<Result<Vec<_>, GeometryGraphError>>()?
        }
        GeometryNodeKind::Twist { amount } => {
            let amount = expr(amount)?;
            inputs
                .first()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(budget)
                .map(|mut seg| -> Result<Segment, GeometryGraphError> {
                    for p in [&mut seg.start, &mut seg.end] {
                        let a = amount.evaluate(&context(
                            seg.t, p.x, p.y, p.z, 0.0, 0.0, time, 0.0, params,
                        ))? * p.y;
                        let (s, c) = a.sin_cos();
                        let (x, y) = (p.x * c - p.z * s, p.x * s + p.z * c);
                        p.x = x;
                        p.z = y;
                    }
                    Ok(seg)
                })
                .collect::<Result<Vec<_>, GeometryGraphError>>()?
        }
        GeometryNodeKind::RadialRepeat { copies, axis } => {
            let source = inputs.first().cloned().unwrap_or_default();
            let copies = (*copies).clamp(1, 32) as usize;
            let mut out = Vec::with_capacity(budget.min(source.len().saturating_mul(copies)));
            for copy in 0..copies {
                let angle = std::f32::consts::TAU * copy as f32 / copies as f32;
                for seg in &source {
                    if out.len() >= budget {
                        break;
                    }
                    let rotation = match axis {
                        GraphAxis::X => Vec3::new(angle, 0.0, 0.0),
                        GraphAxis::Y => Vec3::new(0.0, angle, 0.0),
                        GraphAxis::Z => Vec3::new(0.0, 0.0, angle),
                    };
                    out.push(Segment {
                        start: rotate_point(seg.start, rotation),
                        end: rotate_point(seg.end, rotation),
                        echo: copy as u32,
                        ..*seg
                    });
                }
            }
            out
        }
        GeometryNodeKind::Noise {
            amount,
            frequency,
            seed,
        } => {
            let amount = expr(amount)?;
            let frequency = expr(frequency)?;
            inputs
                .first()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(budget)
                .map(|mut seg| -> Result<Segment, GeometryGraphError> {
                    for p in [&mut seg.start, &mut seg.end] {
                        let c = context(seg.t, p.x, p.y, p.z, 0.0, 0.0, time, *seed as f32, params);
                        let a = amount.evaluate(&c)?;
                        let f = frequency.evaluate(&c)?;
                        let h = ((*seed as f32 * 12.9898
                            + p.x * f * 78.233
                            + p.y * f * 37.719
                            + p.z * f * 11.17)
                            .sin()
                            * 43_758.547)
                            .fract();
                        let n = (h * 2.0 - 1.0) * a;
                        p.x += n;
                        p.y += n * 0.73;
                        p.z += n * 0.41;
                    }
                    Ok(seg)
                })
                .collect::<Result<Vec<_>, GeometryGraphError>>()?
        }
        GeometryNodeKind::Merge => {
            let mut out = Vec::with_capacity(budget);
            for input in inputs {
                for seg in input {
                    if out.len() >= budget {
                        break;
                    }
                    out.push(seg);
                }
            }
            out
        }
        GeometryNodeKind::Output => inputs
            .into_iter()
            .next()
            .unwrap_or_default()
            .into_iter()
            .take(budget)
            .collect(),
    };
    active.remove(&id);
    cache.insert(id, result.clone());
    Ok(result)
}

fn compile3(
    src: &[String; 3],
    compile: &impl Fn(&str) -> Result<CalculatorExpression, GeometryGraphError>,
) -> Result<[CalculatorExpression; 3], GeometryGraphError> {
    Ok([compile(&src[0])?, compile(&src[1])?, compile(&src[2])?])
}

fn eval3(
    values: &[CalculatorExpression; 3],
    context: &CalculatorValue,
) -> Result<Vec3, GeometryGraphError> {
    Ok(Vec3::new(
        values[0].evaluate(context)?,
        values[1].evaluate(context)?,
        values[2].evaluate(context)?,
    ))
}
fn points_to_segments(points: &[Vec3], closed: bool, budget: usize) -> Vec<Segment> {
    let count = points.len().saturating_sub(1) + usize::from(closed && points.len() > 2);
    (0..count.min(budget))
        .map(|i| {
            segment(
                points[i % points.len()],
                points[(i + 1) % points.len()],
                i,
                count,
            )
        })
        .collect()
}
fn segment(a: Vec3, b: Vec3, index: usize, count: usize) -> Segment {
    Segment {
        start: a,
        end: b,
        t: index as f32 / count.max(1) as f32,
        trace: [
            index as f32 / count.max(1) as f32,
            (index + 1) as f32 / count.max(1) as f32,
            0.0,
        ],
        echo: 0,
    }
}
fn graph_bounds(segments: &[Segment]) -> (Vec3, f32) {
    let mut lo = segments[0].start;
    let mut hi = lo;
    for s in segments {
        lo = lo.min(s.start).min(s.end);
        hi = hi.max(s.start).max(s.end);
    }
    let center = lo.lerp(hi, 0.5);
    let radius = segments
        .iter()
        .map(|s| (s.start - center).length().max((s.end - center).length()))
        .fold(0.0, f32::max)
        .max(1e-3);
    (center, radius)
}
fn rotate_point(p: Vec3, r: Vec3) -> Vec3 {
    let (sx, cx) = r.x.sin_cos();
    let (sy, cy) = r.y.sin_cos();
    let (sz, cz) = r.z.sin_cos();
    let p = Vec3::new(p.x, p.y * cx - p.z * sx, p.y * sx + p.z * cx);
    let p = Vec3::new(p.x * cy + p.z * sy, p.y, -p.x * sy + p.z * cy);
    Vec3::new(p.x * cz - p.y * sz, p.x * sz + p.y * cz, p.z)
}

#[allow(clippy::too_many_arguments)] // These are the named scalar variables exposed to expressions.
fn context(
    t: f32,
    x: f32,
    y: f32,
    z: f32,
    u: f32,
    v: f32,
    time: f32,
    seed: f32,
    params: &HashMap<String, f32>,
) -> CalculatorValue {
    let mut values = params.clone();
    values.extend([
        ("t".into(), t),
        ("x".into(), x),
        ("y".into(), y),
        ("z".into(), z),
        ("u".into(), u),
        ("v".into(), v),
        ("time".into(), time),
        ("pi".into(), std::f32::consts::PI),
        ("tau".into(), std::f32::consts::TAU),
        ("e".into(), std::f32::consts::E),
        ("seed".into(), seed),
    ]);
    CalculatorValue { values }
}

#[derive(Clone, Debug, Default)]
pub struct CalculatorValue {
    pub values: HashMap<String, f32>,
}

#[derive(Clone, Debug)]
pub struct CalculatorExpression {
    root: Expr,
}
#[derive(Clone, Debug)]
enum Expr {
    Number(f32),
    Variable(String),
    Unary(char, Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CalculatorError {
    #[error("expression is empty or exceeds {MAX_EXPRESSION_BYTES} bytes")]
    Length,
    #[error("unexpected token at byte {0}")]
    Token(usize),
    #[error("unknown function {0}")]
    Function(String),
    #[error("unknown variable {0}")]
    Variable(String),
    #[error("division by zero")]
    DivisionByZero,
    #[error("expression produced a non-finite value")]
    NonFinite,
    #[error("function {0} received the wrong number of arguments")]
    Arity(String),
}

impl CalculatorExpression {
    pub fn compile(source: &str) -> Result<Self, CalculatorError> {
        if source.is_empty() || source.len() > MAX_EXPRESSION_BYTES {
            return Err(CalculatorError::Length);
        }
        let mut parser = Parser {
            src: source.as_bytes(),
            at: 0,
        };
        let root = parser.expression(0)?;
        parser.space();
        if parser.at != parser.src.len() {
            return Err(CalculatorError::Token(parser.at));
        }
        validate_expr(&root)?;
        Ok(Self { root })
    }
    pub fn evaluate(&self, context: &CalculatorValue) -> Result<f32, CalculatorError> {
        let value = eval(&self.root, context)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(CalculatorError::NonFinite)
        }
    }

    pub fn uses_variable(&self, variable: &str) -> bool {
        fn visit(expr: &Expr, variable: &str) -> bool {
            match expr {
                Expr::Variable(name) => name == variable,
                Expr::Unary(_, value) => visit(value, variable),
                Expr::Binary(_, left, right) => visit(left, variable) || visit(right, variable),
                Expr::Call(_, args) => args.iter().any(|arg| visit(arg, variable)),
                Expr::Number(_) => false,
            }
        }
        visit(&self.root, variable)
    }

    pub fn variables(&self) -> Vec<String> {
        fn visit(expr: &Expr, names: &mut HashSet<String>) {
            match expr {
                Expr::Variable(name) => {
                    names.insert(name.clone());
                }
                Expr::Unary(_, value) => visit(value, names),
                Expr::Binary(_, left, right) => {
                    visit(left, names);
                    visit(right, names);
                }
                Expr::Call(_, args) => {
                    for arg in args {
                        visit(arg, names);
                    }
                }
                Expr::Number(_) => {}
            }
        }
        let mut names = HashSet::new();
        visit(&self.root, &mut names);
        names.into_iter().collect()
    }
}

/// An expression with its variables resolved to slot indices, for fast
/// repeated evaluation such as plotting. Real-valued maths: out-of-domain
/// inputs and division by zero produce NaN or infinity, which plotters treat
/// as undefined instead of failing the whole expression.
#[derive(Clone, Debug)]
pub struct BoundExpression {
    root: Bound,
}

#[derive(Clone, Debug)]
enum Bound {
    Number(f32),
    Slot(usize),
    Negate(Box<Bound>),
    Binary(char, Box<Bound>, Box<Bound>),
    One(fn(f32) -> f32, Box<Bound>),
    Two(fn(f32, f32) -> f32, Box<Bound>, Box<Bound>),
    Three(fn(f32, f32, f32) -> f32, Box<Bound>, Box<Bound>, Box<Bound>),
}

impl CalculatorExpression {
    /// Resolves variables to positions in `slots`. Unknown variables fail.
    pub fn bind(&self, slots: &[&str]) -> Result<BoundExpression, CalculatorError> {
        fn bind(expr: &Expr, slots: &[&str]) -> Result<Bound, CalculatorError> {
            Ok(match expr {
                Expr::Number(value) => Bound::Number(*value),
                Expr::Variable(name) => Bound::Slot(
                    slots
                        .iter()
                        .position(|slot| slot == name)
                        .ok_or_else(|| CalculatorError::Variable(name.clone()))?,
                ),
                Expr::Unary('-', value) => Bound::Negate(Box::new(bind(value, slots)?)),
                Expr::Unary(_, value) => bind(value, slots)?,
                Expr::Binary(op, left, right) => Bound::Binary(
                    *op,
                    Box::new(bind(left, slots)?),
                    Box::new(bind(right, slots)?),
                ),
                Expr::Call(name, args) => {
                    let mut args = args.iter().map(|arg| bind(arg, slots).map(Box::new));
                    let mut next = || {
                        args.next()
                            .ok_or_else(|| CalculatorError::Arity(name.clone()))?
                    };
                    match function(name).ok_or_else(|| CalculatorError::Function(name.clone()))? {
                        Function::One(f) => Bound::One(f, next()?),
                        Function::Two(f) => Bound::Two(f, next()?, next()?),
                        Function::Three(f) => Bound::Three(f, next()?, next()?, next()?),
                    }
                }
            })
        }
        Ok(BoundExpression {
            root: bind(&self.root, slots)?,
        })
    }
}

impl BoundExpression {
    /// Evaluates with `values[i]` for slot `i`. May return NaN or infinity.
    pub fn eval(&self, values: &[f32]) -> f32 {
        fn eval(node: &Bound, values: &[f32]) -> f32 {
            match node {
                Bound::Number(value) => *value,
                Bound::Slot(index) => values[*index],
                Bound::Negate(value) => -eval(value, values),
                Bound::Binary(op, left, right) => {
                    let (a, b) = (eval(left, values), eval(right, values));
                    match op {
                        '+' => a + b,
                        '-' => a - b,
                        '*' => a * b,
                        '/' => a / b,
                        _ => a.powf(b),
                    }
                }
                Bound::One(f, a) => f(eval(a, values)),
                Bound::Two(f, a, b) => f(eval(a, values), eval(b, values)),
                Bound::Three(f, a, b, c) => f(eval(a, values), eval(b, values), eval(c, values)),
            }
        }
        eval(&self.root, values)
    }
}

impl GeometryGraph {
    pub fn uses_variable(&self, variable: &str) -> bool {
        self.nodes
            .iter()
            .flat_map(|node| node_expressions(&node.kind))
            .any(|source| {
                CalculatorExpression::compile(source).is_ok_and(|expr| expr.uses_variable(variable))
            })
    }
}

/// A built-in function, with real-valued semantics: inputs outside a
/// function's domain produce NaN, which callers treat as undefined.
#[derive(Clone, Copy)]
enum Function {
    One(fn(f32) -> f32),
    Two(fn(f32, f32) -> f32),
    Three(fn(f32, f32, f32) -> f32),
}

impl Function {
    fn arity(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Two(_) => 2,
            Self::Three(_) => 3,
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).abs().max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Every function an expression may call.
fn function(name: &str) -> Option<Function> {
    use Function::{One, Three, Two};
    Some(match name {
        "sin" => One(f32::sin),
        "cos" => One(f32::cos),
        "tan" => One(f32::tan),
        "asin" => One(f32::asin),
        "acos" => One(f32::acos),
        "atan" => One(f32::atan),
        "sec" => One(|x| 1.0 / x.cos()),
        "csc" => One(|x| 1.0 / x.sin()),
        "cot" => One(|x| 1.0 / x.tan()),
        "sinh" => One(f32::sinh),
        "cosh" => One(f32::cosh),
        "tanh" => One(f32::tanh),
        "asinh" => One(f32::asinh),
        "acosh" => One(f32::acosh),
        "atanh" => One(f32::atanh),
        "abs" => One(f32::abs),
        "sqrt" => One(f32::sqrt),
        "cbrt" => One(f32::cbrt),
        "exp" => One(f32::exp),
        "log" | "ln" => One(f32::ln),
        "log10" => One(f32::log10),
        "log2" => One(f32::log2),
        "floor" => One(f32::floor),
        "ceil" => One(f32::ceil),
        "fract" => One(f32::fract),
        "round" => One(f32::round),
        "sign" | "sgn" => One(f32::signum),
        "min" => Two(f32::min),
        "max" => Two(f32::max),
        "pow" => Two(f32::powf),
        "atan2" => Two(f32::atan2),
        "mod" => Two(|a, b| a.rem_euclid(b.abs())),
        "hypot" => Two(f32::hypot),
        "clamp" => Three(|x, a, b| x.clamp(a.min(b), a.max(b))),
        "lerp" => Three(|a, b, t| a + (b - a) * t),
        "smoothstep" => Three(smoothstep),
        _ => return None,
    })
}

fn validate_expr(expr: &Expr) -> Result<(), CalculatorError> {
    match expr {
        Expr::Unary(_, a) => validate_expr(a),
        Expr::Binary(_, a, b) => {
            validate_expr(a)?;
            validate_expr(b)
        }
        Expr::Call(name, args) => {
            for arg in args {
                validate_expr(arg)?;
            }
            let arity = function(name)
                .ok_or_else(|| CalculatorError::Function(name.clone()))?
                .arity();
            if args.len() != arity {
                return Err(CalculatorError::Arity(name.clone()));
            }
            Ok(())
        }
        Expr::Number(_) | Expr::Variable(_) => Ok(()),
    }
}
fn eval(expr: &Expr, c: &CalculatorValue) -> Result<f32, CalculatorError> {
    let value = match expr {
        Expr::Number(n) => *n,
        Expr::Variable(name) => *c
            .values
            .get(name)
            .ok_or_else(|| CalculatorError::Variable(name.clone()))?,
        Expr::Unary(op, a) => {
            let x = eval(a, c)?;
            if *op == '-' { -x } else { x }
        }
        Expr::Binary(op, a, b) => {
            let x = eval(a, c)?;
            let y = eval(b, c)?;
            match op {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                '/' => {
                    if y.abs() < f32::EPSILON {
                        return Err(CalculatorError::DivisionByZero);
                    } else {
                        x / y
                    }
                }
                '^' => x.powf(y),
                _ => return Err(CalculatorError::Token(0)),
            }
        }
        Expr::Call(name, args) => {
            let a: Vec<f32> = args.iter().map(|x| eval(x, c)).collect::<Result<_, _>>()?;
            let one = || {
                a.first()
                    .copied()
                    .ok_or_else(|| CalculatorError::Arity(name.clone()))
            };
            match name.as_str() {
                "sin" => one()?.sin(),
                "cos" => one()?.cos(),
                "tan" => one()?.tan(),
                "asin" => one()?.asin(),
                "acos" => one()?.acos(),
                "atan" => one()?.atan(),
                "abs" => one()?.abs(),
                "sqrt" => one()?.max(0.0).sqrt(),
                "exp" => one()?.exp(),
                "log" => one()?.ln(),
                "floor" => one()?.floor(),
                "ceil" => one()?.ceil(),
                "fract" => one()?.fract(),
                "round" => one()?.round(),
                "sign" => one()?.signum(),
                "min" if a.len() == 2 => a[0].min(a[1]),
                "max" if a.len() == 2 => a[0].max(a[1]),
                "pow" if a.len() == 2 => a[0].powf(a[1]),
                "atan2" if a.len() == 2 => a[0].atan2(a[1]),
                "mod" if a.len() == 2 && a[1] != 0.0 => a[0].rem_euclid(a[1].abs()),
                "hypot" if a.len() == 2 => a[0].hypot(a[1]),
                "clamp" if a.len() == 3 => a[0].clamp(a[1].min(a[2]), a[1].max(a[2])),
                "lerp" if a.len() == 3 => a[0] + (a[1] - a[0]) * a[2],
                "smoothstep" if a.len() == 3 => smoothstep(a[0], a[1], a[2]),
                _ => match (function(name), a.as_slice()) {
                    (Some(Function::One(f)), [x]) => f(*x),
                    (Some(Function::Two(f)), [x, y]) => f(*x, *y),
                    (Some(Function::Three(f)), [x, y, z]) => f(*x, *y, *z),
                    _ => return Err(CalculatorError::Function(name.clone())),
                },
            }
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(CalculatorError::NonFinite)
    }
}

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self.at < self.src.len() && self.src[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }
    fn expression(&mut self, min: u8) -> Result<Expr, CalculatorError> {
        self.space();
        let mut lhs = if self.take(b'-') {
            Expr::Unary('-', Box::new(self.expression(4)?))
        } else if self.take(b'+') {
            Expr::Unary('+', Box::new(self.expression(4)?))
        } else if self.take(b'(') {
            let e = self.expression(0)?;
            self.space();
            if !self.take(b')') {
                return Err(CalculatorError::Token(self.at));
            }
            e
        } else if self.peek().is_some_and(|x| x.is_ascii_digit() || x == b'.') {
            Expr::Number(self.number()?)
        } else {
            let name = self.identifier()?;
            self.space();
            // `name(` is a call only for a built-in function; otherwise it is
            // a variable times a parenthesised factor, e.g. `x(1+x)`.
            if function(&name).is_some() && self.take(b'(') {
                let mut args = Vec::new();
                self.space();
                if !self.take(b')') {
                    loop {
                        args.push(self.expression(0)?);
                        self.space();
                        if self.take(b')') {
                            break;
                        }
                        if !self.take(b',') {
                            return Err(CalculatorError::Token(self.at));
                        }
                    }
                }
                Expr::Call(name, args)
            } else if name == "pi" {
                Expr::Number(std::f32::consts::PI)
            } else if name == "tau" {
                Expr::Number(std::f32::consts::TAU)
            } else if name == "e" {
                Expr::Number(std::f32::consts::E)
            } else {
                Expr::Variable(name)
            }
        };
        loop {
            self.space();
            let Some(op) = self.peek().map(char::from) else {
                break;
            };
            // A factor directly after another (`2x`, `3sin(x)`, `(a)(b)`)
            // multiplies, at the precedence of `*`.
            if op.is_ascii_alphanumeric() || op == '.' || op == '(' || op == '_' {
                if 2 < min {
                    break;
                }
                let rhs = self.expression(3)?;
                lhs = Expr::Binary('*', Box::new(lhs), Box::new(rhs));
                continue;
            }
            let prec = match op {
                '+' | '-' => 1,
                '*' | '/' => 2,
                '^' => 3,
                _ => break,
            };
            if prec < min {
                break;
            }
            self.at += 1;
            let rhs = self.expression(prec + u8::from(op != '^'))?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
    fn take(&mut self, b: u8) -> bool {
        self.space();
        if self.src.get(self.at) == Some(&b) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }
    fn number(&mut self) -> Result<f32, CalculatorError> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
            self.at += 1;
        }
        // Scientific notation only when digits follow: `2e3`, `1e-4`. A bare
        // `e` after a number is the constant, as in `2e`.
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let mut end = self.at + 1;
            if matches!(self.src.get(end), Some(b'+' | b'-')) {
                end += 1;
            }
            if self.src.get(end).is_some_and(u8::is_ascii_digit) {
                self.at = end;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.at += 1;
                }
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.at])
            .map_err(|_| CalculatorError::Token(start))?;
        let value: f32 = s.parse().map_err(|_| CalculatorError::Token(start))?;
        if !value.is_finite() {
            return Err(CalculatorError::NonFinite);
        }
        Ok(value)
    }
    fn identifier(&mut self) -> Result<String, CalculatorError> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.at += 1;
        }
        if self.at == start {
            return Err(CalculatorError::Token(self.at));
        }
        Ok(std::str::from_utf8(&self.src[start..self.at])
            .unwrap()
            .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculator_supports_safe_math_and_variables() {
        let e = CalculatorExpression::compile("sin(tau*t) + clamp(a, 0, 1)^2").unwrap();
        let c = CalculatorValue {
            values: HashMap::from([("t".into(), 0.25), ("a".into(), 2.0)]),
        };
        assert!((e.evaluate(&c).unwrap() - 2.0).abs() < 1e-5);
        assert!(
            CalculatorExpression::compile("1/0")
                .unwrap()
                .evaluate(&c)
                .is_err()
        );
        assert!(CalculatorExpression::compile("std::process::exit(0)").is_err());
    }
    #[test]
    fn implicit_multiplication_constants_and_scientific_notation() {
        let value = |source: &str, x: f32| {
            CalculatorExpression::compile(source)
                .unwrap_or_else(|error| panic!("{source}: {error}"))
                .evaluate(&CalculatorValue {
                    values: HashMap::from([("x".into(), x)]),
                })
                .unwrap()
        };
        assert_eq!(value("2x", 3.0), 6.0);
        assert_eq!(value("3sin(0)+2x^2", 2.0), 8.0);
        assert_eq!(value("x(1+x)", 2.0), 6.0);
        assert_eq!(value("(x+1)(x-1)", 3.0), 8.0);
        assert!((value("2pi", 0.0) - std::f32::consts::TAU).abs() < 1e-6);
        assert!((value("2e", 0.0) - 2.0 * std::f32::consts::E).abs() < 1e-6);
        assert_eq!(value("2e3", 0.0), 2000.0);
        assert_eq!(value("1e-2", 0.0), 0.01);
        assert_eq!(value("-2x", 3.0), -6.0);
        assert!((value("ln(e^2)+log10(100)", 0.0) - 4.0).abs() < 1e-5);
        assert!((value("cosh(0)+tanh(0)", 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bound_expressions_match_the_interpreter_and_flag_undefined_points() {
        let source = "sin(x)*y + clamp(x, 0, 1)^2 - hypot(x, y)/3";
        let compiled = CalculatorExpression::compile(source).unwrap();
        let bound = compiled.bind(&["x", "y"]).unwrap();
        for (x, y) in [(0.3, 1.2), (-2.0, 0.5), (4.0, -3.0)] {
            let expected = compiled
                .evaluate(&CalculatorValue {
                    values: HashMap::from([("x".into(), x), ("y".into(), y)]),
                })
                .unwrap();
            assert!((bound.eval(&[x, y]) - expected).abs() < 1e-5);
        }
        let sqrt = CalculatorExpression::compile("sqrt(x)").unwrap();
        assert!(sqrt.bind(&["x"]).unwrap().eval(&[-1.0]).is_nan());
        assert!(matches!(
            sqrt.bind(&["y"]),
            Err(CalculatorError::Variable(name)) if name == "x"
        ));
    }

    #[test]
    fn graph_validates_cycles_and_evaluates_a_surface() {
        let mut graph = GeometryGraph::default();
        graph.nodes.push(
            GeometryNode::new(
                3,
                "Twist",
                GeometryNodeKind::Twist {
                    amount: "sin(tau*t)*0.2".into(),
                },
            )
            .with_input(1),
        );
        graph.nodes.push(
            GeometryNode::new(
                4,
                "Twist 2",
                GeometryNodeKind::Twist {
                    amount: "0.2".into(),
                },
            )
            .with_input(3),
        );
        graph.nodes[1].inputs = vec![GraphNodeId(3)];
        graph.output = GraphNodeId(2);
        graph.nodes[2].inputs = vec![GraphNodeId(4)];
        assert!(matches!(
            validate_geometry_graph(&graph),
            Err(GeometryGraphError::Cycle(_))
        ));
        graph.nodes.truncate(2);
        graph.nodes[1].inputs = vec![GraphNodeId(1)];
        graph.output = GraphNodeId(2);
        let geometry = evaluate_geometry_graph(&graph, 0.0, &HashMap::new(), 1000).unwrap();
        assert_eq!(geometry.segments.len(), 256);
        assert!(geometry.is_planar());
    }
}
