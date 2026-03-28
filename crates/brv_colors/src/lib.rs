#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);
    pub const WARM: Self = Self::new(1.0, 0.87, 0.6, 1.0);
    pub const COOL: Self = Self::new(0.6, 0.7, 1.0, 1.0);
    pub const DAYLIGHT: Self = Self::new(1.0, 0.97, 0.9, 1.0);
    pub const SUNSET: Self = Self::new(1.0, 0.5, 0.2, 1.0);
}
