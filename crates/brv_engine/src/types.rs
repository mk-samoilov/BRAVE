use std::sync::Arc;
use brv_colors::Color;

pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct TextureData {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct Material {
    pub albedo: Color,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Color,
    pub albedo_texture: Option<Arc<TextureData>>,
    pub normal_texture: Option<Arc<TextureData>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            albedo: Color::WHITE,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
            albedo_texture: None,
            normal_texture: None,
        }
    }
}

pub struct MeshComponent {
    pub data: Arc<MeshData>,
    pub material: Material,
}

impl MeshComponent {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self {
            data: Arc::new(MeshData { vertices, indices }),
            material: Material::default(),
        }
    }
}

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
