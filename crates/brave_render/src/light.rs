use brave_ecs::Component;
use brave_math::Color;

pub struct DirectionalLight {
    pub color:     Color,
    pub intensity: f32,
    pub shadows:   bool,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self { color: Color::WHITE, intensity: 1.0, shadows: false }
    }
}

impl Component for DirectionalLight {}

pub struct PointLight {
    pub color:     Color,
    pub intensity: f32,
    pub range:     f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self { color: Color::WHITE, intensity: 1.0, range: 10.0 }
    }
}

impl Component for PointLight {}

pub struct SpotLight {
    pub color:     Color,
    pub intensity: f32,
    pub range:     f32,
    pub angle:     f32, // угол конуса в радианах
}

impl Default for SpotLight {
    fn default() -> Self {
        Self { color: Color::WHITE, intensity: 1.0, range: 10.0, angle: 0.5 }
    }
}

impl Component for SpotLight {}

pub struct AmbientLight {
    pub color:     Color,
    pub intensity: f32,
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self { color: Color::WHITE, intensity: 0.1 }
    }
}

impl Component for AmbientLight {}
