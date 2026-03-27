use brave_ecs::Component;
use brave_math::{Mat4, Vec3};

pub struct Camera {
    pub fov:  f32,
    pub near: f32,
    pub far:  f32,
}

impl Camera {
    pub fn new(fov: f32, near: f32, far: f32) -> Self {
        Self { fov, near, far }
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), aspect, self.near, self.far)
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self { fov: 60.0, near: 0.1, far: 1000.0 }
    }
}

impl Component for Camera {}

pub fn compute_view(position: Vec3, forward: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(position, position + forward, up)
}
