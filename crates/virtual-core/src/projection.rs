//! Device-independent calibration for one projected program surface.
use serde::{Deserialize, Serialize};

pub const UNIT_QUAD: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectionMapping {
    pub enabled: bool,
    /// Clockwise in screen coordinates: top-left, top-right, bottom-right, bottom-left.
    pub corners: [[f32; 2]; 4],
    /// Source rectangle: left, top, right, bottom in normalized program coordinates.
    pub source: [f32; 4],
    /// Black rectangular masks in normalized projector coordinates; zero area disables a mask.
    pub masks: [[f32; 4]; 4],
}

impl Default for ProjectionMapping {
    fn default() -> Self {
        Self {
            enabled: false,
            corners: UNIT_QUAD,
            source: [0.0, 0.0, 1.0, 1.0],
            masks: [[0.0; 4]; 4],
        }
    }
}

impl ProjectionMapping {
    pub fn is_valid(&self) -> bool {
        fn rect(r: [f32; 4]) -> bool {
            r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && r[0] <= r[2]
                && r[1] <= r[3]
        }
        rect(self.source)
            && self.source[2] - self.source[0] >= 0.001
            && self.source[3] - self.source[1] >= 0.001
            && self.masks.iter().all(|r| rect(*r))
            && self.inverse_homography().is_some()
    }

    /// Rows of the destination-to-unit-square projective transform, padded for GPU uniforms.
    /// Reject folded, concave, nonfinite and near-degenerate quads rather than sending
    /// a singular transform to the GPU. Solve in f64 to keep calibration stable.
    pub fn inverse_homography(&self) -> Option<[[f32; 4]; 3]> {
        if !self
            .corners
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return None;
        }
        for i in 0..4 {
            let a = self.corners[i];
            let b = self.corners[(i + 1) % 4];
            let c = self.corners[(i + 2) % 4];
            if (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) < 0.0001 {
                return None;
            }
        }
        let mut rows = [[0.0_f64; 9]; 8];
        for (i, (dst, src)) in self.corners.iter().zip(UNIT_QUAD).enumerate() {
            let [x, y] = dst.map(f64::from);
            let [u, v] = src.map(f64::from);
            rows[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
            rows[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
        }
        for col in 0..8 {
            let pivot =
                (col..8).max_by(|&a, &b| rows[a][col].abs().total_cmp(&rows[b][col].abs()))?;
            if rows[pivot][col].abs() < 1e-10 {
                return None;
            }
            rows.swap(col, pivot);
            let scale = rows[col][col];
            for v in &mut rows[col][col..] {
                *v /= scale;
            }
            let pivot_row = rows[col];
            for (r, row) in rows.iter_mut().enumerate() {
                if r == col {
                    continue;
                }
                let factor = row[col];
                for (value, pivot) in row[col..].iter_mut().zip(&pivot_row[col..]) {
                    *value -= factor * pivot;
                }
            }
        }
        let h: [f32; 8] = std::array::from_fn(|i| rows[i][8] as f32);
        if !h.iter().all(|v| v.is_finite()) {
            return None;
        }
        Some([
            [h[0], h[1], h[2], 0.0],
            [h[3], h[4], h[5], 0.0],
            [h[6], h[7], 1.0, 0.0],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn perspective_quad_maps_all_four_corners_to_the_source() {
        let mapping = ProjectionMapping {
            corners: [[0.2, 0.1], [0.8, 0.2], [0.95, 0.9], [0.05, 0.8]],
            ..Default::default()
        };
        assert!(mapping.is_valid());
        let h = mapping.inverse_homography().unwrap();
        for ([x, y], [u, v]) in mapping.corners.into_iter().zip(UNIT_QUAD) {
            let w = h[2][0] * x + h[2][1] * y + h[2][2];
            assert!(((h[0][0] * x + h[0][1] * y + h[0][2]) / w - u).abs() < 1e-5);
            assert!(((h[1][0] * x + h[1][1] * y + h[1][2]) / w - v).abs() < 1e-5);
        }
    }
    #[test]
    fn rejects_folded_degenerate_and_nonfinite_calibration() {
        for corners in [
            [[0.0; 2]; 4],
            [[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]],
            [[f32::NAN, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        ] {
            assert!(
                !ProjectionMapping {
                    corners,
                    ..Default::default()
                }
                .is_valid()
            );
        }
        assert!(
            !ProjectionMapping {
                source: [0.5, 0.0, 0.4, 1.0],
                ..Default::default()
            }
            .is_valid()
        );
        assert!(
            !ProjectionMapping {
                masks: [[0.0, 0.0, 1.1, 1.0]; 4],
                ..Default::default()
            }
            .is_valid()
        );
        assert!(ProjectionMapping::default().is_valid());
    }
}
