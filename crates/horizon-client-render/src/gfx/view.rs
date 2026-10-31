//! The views a frame is drawn from. A flat screen has one; a headset has
//! one per eye. Every view shares the frame's camera position, so the
//! camera-relative world (section origins, entity offsets) is built once and
//! only each view's rotation and lens differ.

use glam::Mat4;

/// The most views one frame draws.
const MAX_VIEWS: usize = 2;

/// One eye's view of the camera-relative world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Camera-relative world to eye: a rotation, plus the eye's offset from
    /// the camera on a headset.
    pub view: Mat4,
    /// Reversed-Z perspective: the near plane at depth 1, far at 0.
    pub projection: Mat4,
    /// Width over height of the image this view fills.
    pub aspect: f32,
    /// `tan(fov_y / 2)`, for shaders rebuilding view rays from NDC.
    pub tan_half_fov: f32,
}

impl View {
    /// The flat-screen view: vanilla's camera rotation and a symmetric
    /// vertical field of view.
    pub fn screen(yaw: f32, pitch: f32, fov_degrees: f32, aspect: f32, far: f32) -> Self {
        Self {
            view: crate::camera::view_matrix(yaw, pitch),
            projection: super::world_projection(fov_degrees, aspect, far),
            aspect,
            tan_half_fov: (fov_degrees.to_radians() / 2.0).tan(),
        }
    }

    /// A headset eye: `view` from the camera-relative world, and an
    /// off-center lens from the tangents of its four half-angles
    /// (left and down negative), reversed-Z like the screen's.
    pub fn eye(view: Mat4, tangents: [f32; 4], far: f32) -> Self {
        let [l, r, d, u] = tangents;
        let near = super::NEAR_PLANE;
        let projection = Mat4::from_cols(
            glam::Vec4::new(2.0 / (r - l), 0.0, 0.0, 0.0),
            glam::Vec4::new(0.0, 2.0 / (u - d), 0.0, 0.0),
            glam::Vec4::new(
                (r + l) / (r - l),
                (u + d) / (u - d),
                near / (far - near),
                -1.0,
            ),
            glam::Vec4::new(0.0, 0.0, near * far / (far - near), 0.0),
        );
        Self {
            view,
            projection,
            aspect: (r - l) / (u - d),
            tan_half_fov: (u - d) / 2.0,
        }
    }

    /// Clip space from the camera-relative world, with `pose` (view bobbing,
    /// nausea spin) between the lens and the rotation.
    pub fn view_proj(&self, pose: Mat4) -> Mat4 {
        self.projection * pose * self.view
    }
}

/// Every view of a frame, and the one geometry is culled with.
#[derive(Clone, Copy, Debug)]
pub struct ViewSet {
    views: [View; MAX_VIEWS],
    len: usize,
    cull: View,
}

impl ViewSet {
    /// A frame drawn from `view` alone, culled with it.
    pub fn single(view: View) -> Self {
        Self {
            views: [view; MAX_VIEWS],
            len: 1,
            cull: view,
        }
    }

    /// A frame drawn from `view`, culled with `cull` (a wider view, shared
    /// by every eye of the frame).
    pub fn with_cull(view: View, cull: View) -> Self {
        Self {
            views: [view; MAX_VIEWS],
            len: 1,
            cull,
        }
    }

    /// The view the HUD, hand and screen-space effects follow.
    pub fn primary(&self) -> &View {
        &self.views[0]
    }

    pub fn views(&self) -> &[View] {
        &self.views[..self.len]
    }

    /// The view whose frustum covers every other view's.
    pub fn cull(&self) -> &View {
        &self.cull
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formulas `Renderer` used before views existed, kept to pin the
    /// screen view to them bit for bit.
    fn legacy_view_proj(fov: f32, yaw: f32, pitch: f32, pose: Mat4, aspect: f32, far: f32) -> Mat4 {
        super::super::world_projection(fov, aspect, far)
            * pose
            * crate::camera::view_matrix(yaw, pitch)
    }

    fn poses() -> [Mat4; 3] {
        [
            Mat4::IDENTITY,
            Mat4::from_translation(glam::Vec3::new(0.013, -0.027, 0.0))
                * Mat4::from_rotation_z(0.021),
            Mat4::from_rotation_y(0.4) * Mat4::from_scale(glam::Vec3::new(1.1, 0.9, 1.0)),
        ]
    }

    #[test]
    fn the_screen_view_matches_the_legacy_projection_bit_for_bit() {
        for (fov, yaw, pitch, aspect, far) in [
            (70.0, 0.0, 0.0, 16.0 / 9.0, 512.0),
            (110.0, -137.25, 61.5, 21.0 / 9.0, 8192.0),
            (30.0, 359.9, -90.0, 4.0 / 3.0, 96.0),
            (82.333, 12.5, 7.75, 1.0, 1024.0),
        ] {
            let view = View::screen(yaw, pitch, fov, aspect, far);
            for pose in poses() {
                let old = legacy_view_proj(fov, yaw, pitch, pose, aspect, far);
                let new = view.view_proj(pose);
                assert_eq!(
                    old.to_cols_array().map(f32::to_bits),
                    new.to_cols_array().map(f32::to_bits),
                    "fov {fov} yaw {yaw} pitch {pitch}"
                );
            }
            assert_eq!(view.view, crate::camera::view_matrix(yaw, pitch));
            assert_eq!(
                view.tan_half_fov.to_bits(),
                (fov.to_radians() / 2.0).tan().to_bits()
            );
            assert_eq!(view.aspect.to_bits(), aspect.to_bits());
        }
    }

    #[test]
    fn a_single_view_set_draws_and_culls_with_its_view() {
        let view = View::screen(45.0, 10.0, 70.0, 16.0 / 9.0, 512.0);
        let set = ViewSet::single(view);
        assert_eq!(set.views(), &[view]);
        assert_eq!(*set.primary(), view);
        assert_eq!(*set.cull(), view);
    }
}
