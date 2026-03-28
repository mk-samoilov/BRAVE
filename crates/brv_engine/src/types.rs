use brv_colors::Color;

pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

pub struct MeshComponent;

pub struct DirectionalLight {
    pub color: Color,
    pub intensity: f32,
}

pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
}

pub struct SpotLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub angle: f32,
}

pub struct AmbientLight {
    pub color: Color,
    pub intensity: f32,
}

pub enum Light {
    Directional(DirectionalLight),
    Point(PointLight),
    Spot(SpotLight),
    Ambient(AmbientLight),
}

impl From<DirectionalLight> for Light {
    fn from(l: DirectionalLight) -> Light {
        Light::Directional(l)
    }
}

impl From<PointLight> for Light {
    fn from(l: PointLight) -> Light {
        Light::Point(l)
    }
}

impl From<SpotLight> for Light {
    fn from(l: SpotLight) -> Light {
        Light::Spot(l)
    }
}

impl From<AmbientLight> for Light {
    fn from(l: AmbientLight) -> Light {
        Light::Ambient(l)
    }
}
