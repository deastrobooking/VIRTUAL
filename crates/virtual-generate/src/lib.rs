//! Procedural deck sources: recursive 2D and 3D line geometry.
//!
//! [`generate`] turns a pattern and its parameters into renderer-neutral
//! line segments. [`Generator`] owns one deck's animation state and
//! rasterizes those segments into RGBA8 frames for the deck pipeline.

mod geometry;
mod geometry_graph;
pub mod graphing;
pub mod graphing_library;
mod math;
mod patterns;
mod render;
mod settings;
mod text;
mod texture;
mod turtle;

pub use geometry::{
    AudioBands, Geometry, GeometryBuffers, GeometryParams, Palette, Segment, generate,
};
pub use geometry_graph::{
    BoundExpression, CalculatorError, CalculatorExpression, CalculatorValue, GeometryGraph,
    GeometryGraphError, GeometryNode, GeometryNodeKind, GeometryParameter, GraphAxis, GraphNodeId,
    MAX_GEOMETRY_NODES, evaluate_geometry_graph, validate_geometry_graph,
};
pub use math::{Mat3, Rng, Vec3, hsl_to_rgb};
pub use render::{Generator, GeneratorStackRenderer, GeneratorStats};
pub use settings::{
    ColorMode, Dimension, FRAME_RATES, GeneratorBlendMode, GeneratorLayer, GeneratorSettings,
    GeneratorStack, RESOLUTIONS, RecursivePattern, TEXT_MAX_CHARS,
};
pub use turtle::{Frame, MAX_SEGMENTS, Turtle};

mod parameters;
pub use parameters::{GENERATOR_PARAMETERS, GeneratorParameter};
