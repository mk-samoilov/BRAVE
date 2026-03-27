use std::collections::HashMap;
use std::sync::Arc;

#[cfg(debug_assertions)]
use std::path::{Path, PathBuf};
#[cfg(not(debug_assertions))]
use std::path::PathBuf;

use ash::vk;
use brave_render::{GpuTexture, Mesh, VulkanContext};

#[cfg(debug_assertions)]
use crate::gltf_loader;
use crate::image_loader::TextureData;
#[cfg(debug_assertions)]
use crate::shader_loader;

// ─── Public types ─────────────────────────────────────────────────────────────

/// One GPU-ready primitive from a loaded GLTF model.
pub struct LoadedPrimitive {
    pub mesh:       Arc<Mesh>,
    pub base_color: [f32; 4],
    /// Albedo texture, if the material had one.
    pub texture:    Option<Arc<GpuTexture>>,
}

/// All primitives extracted from a single GLTF file.
pub struct LoadedModel {
    pub primitives: Vec<LoadedPrimitive>,
}

// ─── Legacy Asset enum (kept for backward compat) ─────────────────────────────

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
        match self { Asset::Model(m) => m, _ => panic!("Asset is not a Model") }
    }
    pub fn into_texture(self) -> TextureData {
        match self { Asset::Texture(t) => t, _ => panic!("Asset is not a Texture") }
    }
    pub fn into_shader(self) -> Vec<u32> {
        match self { Asset::Shader(s) => s, _ => panic!("Asset is not a Shader") }
    }
}

// ─── Internal GPU context ─────────────────────────────────────────────────────

struct GpuCtx {
    ctx:                 *const VulkanContext,
    command_pool:        vk::CommandPool,
    tex_descriptor_pool: vk::DescriptorPool,
    tex_desc_set_layout: vk::DescriptorSetLayout,
}

unsafe impl Send for GpuCtx {}
unsafe impl Sync for GpuCtx {}

// ─── AssetManager ─────────────────────────────────────────────────────────────

pub struct AssetManager {
    #[cfg(debug_assertions)]
    root: PathBuf,
    gpu: Option<GpuCtx>,
    /// Cache for full GLTF scenes.
    model_cache:   HashMap<String, Arc<LoadedModel>>,
    /// Cache for individual GPU textures.
    texture_cache: HashMap<String, Arc<GpuTexture>>,
}

impl AssetManager {
    pub fn new(assets_path: impl Into<PathBuf>) -> Self {
        let _root = assets_path.into();
        #[cfg(not(debug_assertions))]
        log::info!("AssetManager: release mode — loading from .ast archives");
        Self {
            #[cfg(debug_assertions)]
            root: _root,
            gpu:           None,
            model_cache:   HashMap::new(),
            texture_cache: HashMap::new(),
        }
    }

    /// Connect to the Vulkan renderer so GPU resources can be created.
    ///
    /// # Safety
    /// `ctx` must remain valid for the lifetime of this AssetManager.
    pub unsafe fn connect_renderer(
        &mut self,
        ctx:                 *const VulkanContext,
        command_pool:        vk::CommandPool,
        tex_descriptor_pool: vk::DescriptorPool,
        tex_desc_set_layout: vk::DescriptorSetLayout,
    ) {
        self.gpu = Some(GpuCtx { ctx, command_pool, tex_descriptor_pool, tex_desc_set_layout });
    }

    // ─── GLTF (primary API) ──────────────────────────────────────────────────

    /// Load a GLTF model by stem name (e.g. "dungeon").
    /// Returns cached result on repeated calls.
    /// Automatically loads embedded textures.
    pub fn load_gltf(&mut self, name: &str) -> Arc<LoadedModel> {
        if let Some(cached) = self.model_cache.get(name) {
            return Arc::clone(cached);
        }

        let model = self.load_gltf_uncached(name);
        let arc = Arc::new(model);
        self.model_cache.insert(name.to_string(), Arc::clone(&arc));
        arc
    }

    fn load_gltf_uncached(&mut self, name: &str) -> LoadedModel {
        #[cfg(debug_assertions)]
        {
            let path = self.find_model(name);
            let (primitives, embedded) = gltf_loader::load(&path, name);

            // Extract GPU context values before any mutable borrow of self.
            let (ctx_ptr, command_pool, tex_descriptor_pool, tex_desc_set_layout) = {
                let gpu = self.gpu.as_ref().expect("AssetManager not connected to renderer");
                (gpu.ctx, gpu.command_pool, gpu.tex_descriptor_pool, gpu.tex_desc_set_layout)
            };
            let ctx = unsafe { &*ctx_ptr };

            // ONE command buffer for all uploads — one queue_wait_idle for the whole model.
            let mut batch = brave_render::UploadBatch::new(ctx, command_pool);

            // Upload albedo textures into batch (skip if already cached).
            for tex in &embedded {
                if !self.texture_cache.contains_key(&tex.name) {
                    let gpu_tex = brave_render::GpuTexture::from_rgba8_batched(
                        ctx, &mut batch,
                        tex_descriptor_pool, tex_desc_set_layout,
                        tex.width, tex.height, &tex.pixels,
                    );
                    self.texture_cache.insert(tex.name.clone(), gpu_tex);
                }
            }

            // Upload all mesh primitives into batch.
            let gpu_primitives: Vec<LoadedPrimitive> = primitives.into_iter().map(|p| {
                let mesh = brave_render::Mesh::new_batched(ctx, &mut batch, &p.vertices, &p.indices);
                let texture = p.material.albedo_tex_index.map(|i| {
                    let tex_name = format!("{}_tex_{}", name, i);
                    self.texture_cache.get(&tex_name).cloned().unwrap_or_else(|| {
                        log::warn!("AssetManager: texture '{}' not in cache", tex_name);
                        Arc::clone(self.texture_cache.values().next().unwrap())
                    })
                });
                LoadedPrimitive { mesh, base_color: p.material.base_color_factor, texture }
            }).collect();

            // Single GPU submit + wait for the entire model.
            batch.flush(ctx, ctx.graphics_queue);

            return LoadedModel { primitives: gpu_primitives };
        }

        #[cfg(not(debug_assertions))]
        {
            let model_data = self.open_ast("models.ast").read_raw(name);
            let decoded    = decode_model_multi(&model_data);

            let gpu_primitives = decoded.into_iter().map(|(verts, inds, color, tex_name)| {
                let mesh    = self.upload_mesh(&verts, &inds);
                let texture = if tex_name.is_empty() {
                    None
                } else {
                    Some(self.load_gpu_texture(&tex_name))
                };
                LoadedPrimitive { mesh, base_color: color, texture }
            }).collect();

            LoadedModel { primitives: gpu_primitives }
        }
    }

    // ─── GPU Texture ─────────────────────────────────────────────────────────

    /// Load a texture by name and upload to GPU. Cached.
    pub fn load_gpu_texture(&mut self, name: &str) -> Arc<GpuTexture> {
        if let Some(cached) = self.texture_cache.get(name) {
            return Arc::clone(cached);
        }
        let data = self.load_texture_data(name);
        let tex  = self.upload_texture_data(data.width, data.height, &data.pixels);
        self.texture_cache.insert(name.to_string(), Arc::clone(&tex));
        tex
    }

    // ─── Legacy single-mesh API ──────────────────────────────────────────────

    pub fn load_asset(&mut self, name: &str, format: AssetFormat) -> Asset {
        match format {
            AssetFormat::Model   => Asset::Model(self.load_single_mesh(name)),
            AssetFormat::Texture => Asset::Texture(self.load_texture_data(name)),
            AssetFormat::Shader  => Asset::Shader(self.load_shader(name)),
        }
    }

    fn load_single_mesh(&mut self, name: &str) -> Arc<Mesh> {
        // Reuse the first primitive of load_gltf
        let model = self.load_gltf(name);
        Arc::clone(&model.primitives[0].mesh)
    }

    // ─── Internal loaders ────────────────────────────────────────────────────

    #[cfg(not(debug_assertions))]
    fn upload_mesh(&self, vertices: &[Vertex], indices: &[u32]) -> Arc<Mesh> {
        let gpu  = self.gpu.as_ref().expect("AssetManager not connected to renderer");
        let ctx  = unsafe { &*gpu.ctx };
        Mesh::new(ctx, gpu.command_pool, vertices, indices)
    }

    fn upload_texture_data(&self, width: u32, height: u32, pixels: &[u8]) -> Arc<GpuTexture> {
        let gpu = self.gpu.as_ref().expect("AssetManager not connected to renderer");
        let ctx = unsafe { &*gpu.ctx };
        GpuTexture::from_rgba8(
            ctx,
            gpu.command_pool,
            gpu.tex_descriptor_pool,
            gpu.tex_desc_set_layout,
            width,
            height,
            pixels,
        )
    }

    fn load_texture_data(&self, name: &str) -> TextureData {
        #[cfg(not(debug_assertions))]
        {
            let ast  = self.open_ast("textures.ast");
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
            let ast  = self.open_ast("shaders.ast");
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

    // ─── Debug file finders ──────────────────────────────────────────────────

    #[cfg(debug_assertions)]
    fn find_model(&self, name: &str) -> std::path::PathBuf {
        let base = self.root.join("models");
        self.find_file(&base, name, &["glb", "gltf"])
    }

    #[cfg(debug_assertions)]
    fn find_texture(&self, name: &str) -> std::path::PathBuf {
        let base = self.root.join("textures");
        self.find_file(&base, name, &["png", "jpg", "jpeg", "hdr"])
    }

    #[cfg(debug_assertions)]
    fn find_shader(&self, name: &str) -> std::path::PathBuf {
        let base   = self.root.join("shaders");
        let direct = base.join(name);
        if direct.exists() { return direct; }
        self.find_file(&base, name, &["vert.glsl", "frag.glsl", "comp.glsl", "glsl"])
    }

    #[cfg(debug_assertions)]
    fn find_file(&self, dir: &Path, name: &str, exts: &[&str]) -> std::path::PathBuf {
        let direct = dir.join(name);
        if direct.exists() { return direct; }
        for ext in exts {
            let p = dir.join(format!("{}.{}", name, ext));
            if p.exists() { return p; }
        }
        panic!("Asset '{}' not found in '{}'", name, dir.display())
    }

    // ─── Release .ast helpers ────────────────────────────────────────────────

    #[cfg(not(debug_assertions))]
    fn open_ast(&self, filename: &str) -> crate::ast::AstFile {
        let path = self.ast_dir().join(filename);
        crate::ast::AstFile::open(&path)
    }

    #[cfg(not(debug_assertions))]
    fn ast_dir(&self) -> PathBuf {
        std::env::current_exe().unwrap()
            .parent().unwrap()
            .to_path_buf()
    }
}

// ─── Release decoders ─────────────────────────────────────────────────────────

/// Decode "BRMM" multi-primitive model binary.
/// Returns Vec of (vertices, indices, base_color, tex_name).
#[cfg(not(debug_assertions))]
fn decode_model_multi(data: &[u8]) -> Vec<(Vec<Vertex>, Vec<u32>, [f32; 4], String)> {
    assert!(data.len() >= 8, "Model data too short");
    assert_eq!(&data[..4], b"BRMM", "Expected BRMM model format");

    let prim_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut pos    = 8usize;
    let mut result = Vec::with_capacity(prim_count);

    for _ in 0..prim_count {
        let vtx_count = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize; pos += 4;
        let idx_count = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize; pos += 4;
        let color: [f32; 4] = std::array::from_fn(|i| {
            f32::from_le_bytes(data[pos + i*4..pos + i*4 + 4].try_into().unwrap())
        });
        pos += 16;
        let tex_len  = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize; pos += 4;
        let tex_name = std::str::from_utf8(&data[pos..pos+tex_len]).unwrap().to_string(); pos += tex_len;

        let f = |p: usize| f32::from_le_bytes(data[p..p+4].try_into().unwrap());
        let mut vertices = Vec::with_capacity(vtx_count);
        for _ in 0..vtx_count {
            vertices.push(Vertex {
                position: [f(pos), f(pos+4), f(pos+8)],
                normal:   [f(pos+12), f(pos+16), f(pos+20)],
                uv:       [f(pos+24), f(pos+28)],
            });
            pos += 32;
        }

        let mut indices = Vec::with_capacity(idx_count);
        for _ in 0..idx_count {
            indices.push(u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()));
            pos += 4;
        }

        result.push((vertices, indices, color, tex_name));
    }

    result
}

#[cfg(not(debug_assertions))]
fn decode_texture(data: &[u8]) -> TextureData {
    assert!(data.len() >= 8, "Texture data too short");
    let width  = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let pixels = data[8..].to_vec();
    TextureData { width, height, pixels }
}
