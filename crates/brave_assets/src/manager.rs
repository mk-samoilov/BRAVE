use std::sync::Arc;

#[cfg(debug_assertions)]
use std::path::{Path, PathBuf};
#[cfg(not(debug_assertions))]
use std::path::PathBuf;

use ash::vk;
use brave_render::{Mesh, VulkanContext};
#[cfg(not(debug_assertions))]
use brave_render::Vertex;

#[cfg(not(debug_assertions))]
use crate::ast::AstFile;
#[cfg(debug_assertions)]
use crate::gltf_loader;
use crate::image_loader::TextureData;
#[cfg(debug_assertions)]
use crate::shader_loader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetFormat {
    Model,
    Texture,
    Shader,
}

pub enum Asset {
    Model(Arc<Mesh>),
    Texture(TextureData),
    Shader(Vec<u32>),
}

impl Asset {
    pub fn into_model(self) -> Arc<Mesh> {
        match self {
            Asset::Model(m) => m,
            _ => panic!("Asset is not a Model"),
        }
    }

    pub fn into_texture(self) -> TextureData {
        match self {
            Asset::Texture(t) => t,
            _ => panic!("Asset is not a Texture"),
        }
    }

    pub fn into_shader(self) -> Vec<u32> {
        match self {
            Asset::Shader(s) => s,
            _ => panic!("Asset is not a Shader"),
        }
    }
}

struct GpuCtx {
    ctx:          *const VulkanContext,
    command_pool: vk::CommandPool,
}

unsafe impl Send for GpuCtx {}
unsafe impl Sync for GpuCtx {}

pub struct AssetManager {
    #[cfg(debug_assertions)]
    root: PathBuf,
    gpu:  Option<GpuCtx>,
}

impl AssetManager {
    pub fn new(assets_path: impl Into<PathBuf>) -> Self {
        let _root = assets_path.into();
        #[cfg(not(debug_assertions))]
        log::info!("AssetManager: release mode — assets loaded from .ast archives next to binary");
        Self {
            #[cfg(debug_assertions)]
            root: _root,
            gpu: None,
        }
    }

    pub unsafe fn connect_renderer(
        &mut self,
        ctx: *const VulkanContext,
        command_pool: vk::CommandPool,
    ) {
        self.gpu = Some(GpuCtx { ctx, command_pool });
    }

    pub fn load_asset(&self, name: &str, format: AssetFormat) -> Asset {
        match format {
            AssetFormat::Model   => Asset::Model(self.load_model(name)),
            AssetFormat::Texture => Asset::Texture(self.load_texture(name)),
            AssetFormat::Shader  => Asset::Shader(self.load_shader(name)),
        }
    }

    fn load_model(&self, name: &str) -> Arc<Mesh> {
        let gpu = self.gpu.as_ref().expect(
            "AssetManager not connected to renderer. Add RenderPlugin before load_asset."
        );
        let ctx = unsafe { &*gpu.ctx };

        #[cfg(not(debug_assertions))]
        {
            let ast = self.open_ast("models.ast");
            let data = ast.read_raw(name);
            let (vertices, indices) = decode_model(&data);
            return Mesh::new(ctx, gpu.command_pool, &vertices, &indices);
        }

        #[cfg(debug_assertions)]
        {
            let path = self.find_model(name);
            let (vertices, indices) = gltf_loader::load(&path);
            Mesh::new(ctx, gpu.command_pool, &vertices, &indices)
        }
    }

    fn load_texture(&self, name: &str) -> TextureData {
        #[cfg(not(debug_assertions))]
        {
            let ast = self.open_ast("textures.ast");
            let data = ast.read_raw(name);
            return decode_texture(&data);
        }

        #[cfg(debug_assertions)]
        {
            let path = self.find_texture(name);
            crate::image_loader::load(&path)
        }
    }

    fn load_shader(&self, name: &str) -> Vec<u32> {
        #[cfg(not(debug_assertions))]
        {
            let ast = self.open_ast("shaders.ast");
            let data = ast.read_raw(name);
            assert_eq!(data.len() % 4, 0, "SPIR-V size must be a multiple of 4");
            return data.chunks(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
        }

        #[cfg(debug_assertions)]
        {
            let path = self.find_shader(name);
            shader_loader::load(&path)
        }
    }

    #[cfg(not(debug_assertions))]
    fn open_ast(&self, filename: &str) -> AstFile {
        let path = self.ast_dir().join(filename);
        log::debug!("Reading from {}", path.display());
        AstFile::open(&path)
    }

    #[cfg(not(debug_assertions))]
    fn ast_dir(&self) -> PathBuf {
        std::env::current_exe()
            .expect("Failed to get executable path")
            .parent()
            .expect("Executable has no parent directory")
            .to_path_buf()
    }

    #[cfg(debug_assertions)]
    fn find_model(&self, name: &str) -> PathBuf {
        let base = self.root.join("models");
        self.find_file(&base, name, &["glb", "gltf"])
    }

    #[cfg(debug_assertions)]
    fn find_texture(&self, name: &str) -> PathBuf {
        let base = self.root.join("textures");
        self.find_file(&base, name, &["png", "jpg", "jpeg", "hdr"])
    }

    #[cfg(debug_assertions)]
    fn find_shader(&self, name: &str) -> PathBuf {
        let base = self.root.join("shaders");
        let direct = base.join(name);
        if direct.exists() {
            return direct;
        }
        self.find_file(&base, name, &["vert.glsl", "frag.glsl", "comp.glsl", "glsl"])
    }

    #[cfg(debug_assertions)]
    fn find_file(&self, dir: &Path, name: &str, exts: &[&str]) -> PathBuf {
        let direct = dir.join(name);
        if direct.exists() {
            return direct;
        }
        for ext in exts {
            let p = dir.join(format!("{}.{}", name, ext));
            if p.exists() {
                return p;
            }
        }
        panic!("Asset '{}' not found in '{}'", name, dir.display())
    }
}

#[cfg(not(debug_assertions))]
fn decode_model(data: &[u8]) -> (Vec<Vertex>, Vec<u32>) {
    assert!(data.len() >= 8, "Model data too short");
    let vertex_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let index_count  = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    let vertices_bytes = vertex_count * 32;
    let indices_bytes  = index_count * 4;
    assert!(data.len() >= 8 + vertices_bytes + indices_bytes, "Model data truncated");

    let mut vertices = Vec::with_capacity(vertex_count);
    let mut pos = 8usize;
    for _ in 0..vertex_count {
        let f = |p: usize| f32::from_le_bytes([data[p], data[p+1], data[p+2], data[p+3]]);
        vertices.push(Vertex {
            position: [f(pos), f(pos+4), f(pos+8)],
            normal:   [f(pos+12), f(pos+16), f(pos+20)],
            uv:       [f(pos+24), f(pos+28)],
        });
        pos += 32;
    }

    let mut indices = Vec::with_capacity(index_count);
    for i in 0..index_count {
        let o = pos + i * 4;
        indices.push(u32::from_le_bytes([data[o], data[o+1], data[o+2], data[o+3]]));
    }

    (vertices, indices)
}

#[cfg(not(debug_assertions))]
fn decode_texture(data: &[u8]) -> TextureData {
    assert!(data.len() >= 8, "Texture data too short");
    let width  = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let pixels = data[8..].to_vec();
    TextureData { width, height, pixels }
}
