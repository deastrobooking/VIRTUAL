//! Procedural deck sources: recursive 2D and 3D line geometry.
//!
//! [`generate`] turns a pattern and its parameters into renderer-neutral
//! line segments. [`Generator`] owns one deck's animation state and
//! rasterizes those segments into RGBA8 frames for the deck pipeline.

mod geometry;
mod math;
mod patterns;
mod render;
mod settings;
mod turtle;

pub use geometry::{
    AudioBands, Geometry, GeometryBuffers, GeometryParams, Palette, Segment, generate,
};
pub use math::{Mat3, Rng, Vec3, hsl_to_rgb};
pub use render::{Generator, GeneratorStats};
pub use settings::{
    ColorMode, Dimension, FRAME_RATES, GeneratorSettings, RESOLUTIONS, RecursivePattern,
};
pub use turtle::{Frame, MAX_SEGMENTS, Turtle};
