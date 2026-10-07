//! Turtle frame and the bounded segment builder the patterns draw into.

use crate::geometry::Segment;
use crate::math::Vec3;

/// Hard ceiling on emitted segments. Patterns lower their depth to fit it
/// (see `patterns::effective_depth`); the builder stops at it regardless.
pub const MAX_SEGMENTS: usize = 200_000;

pub(crate) struct Builder {
    pub segments: Vec<Segment>,
    budget: usize,
    pub truncated: bool,
}

impl Builder {
    pub fn new(budget: usize, estimate: u64) -> Self {
        let budget = budget.min(MAX_SEGMENTS);
        Self {
            segments: Vec::with_capacity((estimate as usize).min(budget)),
            budget,
            truncated: false,
        }
    }

    pub fn full(&self) -> bool {
        self.segments.len() >= self.budget
    }

    /// Emits one segment. `t` is the colour/reveal coordinate in `[0, 1]`.
    pub fn line(&mut self, start: Vec3, end: Vec3, t: f32) -> bool {
        if self.full() {
            self.truncated = true;
            return false;
        }
        self.segments.push(Segment {
            start,
            end,
            t,
            trace: [0.0, 1.0, 0.0],
            echo: 0,
        });
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub position: Vec3,
    pub direction: Vec3,
    pub up: Vec3,
    pub right: Vec3,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::Y,
            up: Vec3::Z,
            right: Vec3::X,
        }
    }
}

/// A 3D turtle. Rotations use the turtle's local axes: yaw turns about `up`
/// (positive = left / counter-clockwise in XY), pitch about `right`
/// (positive = toward `up`), roll about `direction`.
#[derive(Clone, Debug, Default)]
pub struct Turtle {
    frame: Frame,
    stack: Vec<Frame>,
}

impl Turtle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn reset(&mut self) {
        self.frame = Frame::default();
        self.stack.clear();
    }

    pub(crate) fn forward(&mut self, distance: f32, t: f32, out: &mut Builder) {
        let start = self.frame.position;
        self.frame.position += self.frame.direction * distance;
        out.line(start, self.frame.position, t);
    }

    pub fn move_forward(&mut self, distance: f32) {
        self.frame.position += self.frame.direction * distance;
    }

    pub fn yaw(&mut self, angle: f32) {
        let axis = self.frame.up;
        self.frame.direction = self.frame.direction.rotated(axis, angle).normalized();
        self.frame.right = self.frame.right.rotated(axis, angle).normalized();
    }

    pub fn pitch(&mut self, angle: f32) {
        let axis = self.frame.right;
        self.frame.direction = self.frame.direction.rotated(axis, angle).normalized();
        self.frame.up = self.frame.up.rotated(axis, angle).normalized();
    }

    pub fn roll(&mut self, angle: f32) {
        let axis = self.frame.direction;
        self.frame.up = self.frame.up.rotated(axis, angle).normalized();
        self.frame.right = self.frame.right.rotated(axis, angle).normalized();
    }

    pub fn push(&mut self) {
        self.stack.push(self.frame);
    }

    /// Restores the last pushed frame. Popping an empty stack leaves the
    /// turtle unchanged and returns `false`.
    pub fn pop(&mut self) -> bool {
        match self.stack.pop() {
            Some(frame) => {
                self.frame = frame;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn forward_emits_one_segment_and_advances_by_distance() {
        let mut turtle = Turtle::new();
        let mut out = Builder::new(10, 1);
        turtle.forward(2.5, 0.0, &mut out);
        assert_eq!(out.segments.len(), 1);
        assert!(close(out.segments[0].start, Vec3::ZERO));
        assert!(close(out.segments[0].end, Vec3::new(0.0, 2.5, 0.0)));
        assert!(close(turtle.frame().position, Vec3::new(0.0, 2.5, 0.0)));
    }

    #[test]
    fn push_pop_restores_position_and_all_axes() {
        let mut turtle = Turtle::new();
        turtle.push();
        turtle.yaw(0.3);
        turtle.pitch(0.7);
        turtle.roll(-1.1);
        turtle.move_forward(3.0);
        assert!(turtle.pop());
        assert_eq!(turtle.frame(), Frame::default());
        assert!(!turtle.pop(), "empty pop is a defined no-op");
        assert_eq!(turtle.frame(), Frame::default());
    }

    #[test]
    fn local_axis_conventions() {
        let mut turtle = Turtle::new();
        turtle.yaw(FRAC_PI_2);
        assert!(close(turtle.frame().direction, Vec3::new(-1.0, 0.0, 0.0)));
        assert!(close(turtle.frame().up, Vec3::Z));

        let mut turtle = Turtle::new();
        turtle.pitch(FRAC_PI_2);
        assert!(close(turtle.frame().direction, Vec3::Z));

        let mut turtle = Turtle::new();
        turtle.roll(FRAC_PI_2);
        assert!(close(turtle.frame().direction, Vec3::Y));
        // After rolling, yaw turns about the rolled `up` axis, not world Z.
        turtle.yaw(FRAC_PI_2);
        assert!(turtle.frame().direction.z.abs() > 0.99);
    }

    #[test]
    fn builder_refuses_segments_past_its_budget() {
        let mut out = Builder::new(2, 2);
        assert!(out.line(Vec3::ZERO, Vec3::X, 0.0));
        assert!(out.line(Vec3::ZERO, Vec3::Y, 0.0));
        assert!(!out.line(Vec3::ZERO, Vec3::Z, 0.0));
        assert!(out.truncated);
        assert_eq!(out.segments.len(), 2);
    }
}
