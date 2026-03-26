use std::path::{Path, PathBuf};
use std::sync::Arc;

use ash::vk;
use brave_render::{Mesh, VulkanContext};

use crate::gltf_loader;
use crate::image_loader::TextureData;
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
    /// Извлечь GPU-меш. Паникует если не Model.
    pub fn into_model(self) -> Arc<Mesh> {
        match self {
            Asset::Model(m) => m,
            _ => panic!("Asset is not a Model"),
        }
    }

    /// Извлечь текстуру. Паникует если не Texture.
    pub fn into_texture(self) -> TextureData {
        match self {
            Asset::Texture(t) => t,
            _ => panic!("Asset is not a Texture"),
        }
    }

    /// Извлечь шейдер (SPIR-V). Паникует если не Shader.
    pub fn into_shader(self) -> Vec<u32> {
        match self {
            Asset::Shader(s) => s,
            _ => panic!("Asset is not a Shader"),
        }
    }
}

// GPU-контекст для загрузки ресурсов — устанавливается после инициализации рендера.
struct GpuCtx {
    /// Жив пока жив Renderer в Engine.
    ctx: *const VulkanContext,
    command_pool: vk::CommandPool,
}

// SAFETY: Engine — single-threaded (winit), поэтому Send/Sync не нужны.
// Указатель жив всё время работы Engine.
unsafe impl Send for GpuCtx {}
unsafe impl Sync for GpuCtx {}

pub struct AssetManager {
    root: PathBuf,
    gpu: Option<GpuCtx>,
}

impl AssetManager {
    pub fn new(assets_path: impl Into<PathBuf>) -> Self {
        Self { root: assets_path.into(), gpu: None }
    }

    /// Вызывается из brave_core после создания Renderer.
    /// SAFETY: ctx и command_pool должны жить не меньше чем AssetManager.
    pub unsafe fn connect_renderer(
        &mut self,
        ctx: *const VulkanContext,
        command_pool: vk::CommandPool,
    ) {
        self.gpu = Some(GpuCtx { ctx, command_pool });
    }

    /// Загрузить ассет с диска.
    /// - Model   → Arc<Mesh>       (GPU буферы)
    /// - Texture → TextureData     (CPU пиксели; GPU-загрузка на шаге 9)
    /// - Shader  → Vec<u32>        (SPIR-V байткод)
    pub fn load_asset(&self, name: &str, format: AssetFormat) -> Asset {
        match format {
            AssetFormat::Model   => Asset::Model(self.load_model(name)),
            AssetFormat::Texture => Asset::Texture(self.load_texture(name)),
            AssetFormat::Shader  => Asset::Shader(self.load_shader(name)),
        }
    }

    fn load_model(&self, name: &str) -> Arc<Mesh> {
        let gpu = self.gpu.as_ref().expect(
            "AssetManager not connected to renderer. Make sure RenderPlugin is added before calling load_asset."
        );
        let ctx = unsafe { &*gpu.ctx };

        // Поиск файла: .glb приоритет, потом .gltf
        let path = self.find_model(name);
        let (vertices, indices) = gltf_loader::load(&path);
        Mesh::new(ctx, gpu.command_pool, &vertices, &indices)
    }

    fn load_texture(&self, name: &str) -> TextureData {
        let path = self.find_texture(name);
        crate::image_loader::load(&path)
    }

    fn load_shader(&self, name: &str) -> Vec<u32> {
        let path = self.find_shader(name);
        shader_loader::load(&path)
    }

    // ─── Поиск файлов ────────────────────────────────────────────────────────

    fn find_model(&self, name: &str) -> PathBuf {
        let base = self.root.join("models");
        self.find_file(&base, name, &["glb", "gltf"])
    }

    fn find_texture(&self, name: &str) -> PathBuf {
        let base = self.root.join("textures");
        self.find_file(&base, name, &["png", "jpg", "jpeg", "hdr"])
    }

    fn find_shader(&self, name: &str) -> PathBuf {
        let base = self.root.join("shaders");
        // Если имя уже содержит расширение — ищем напрямую
        let direct = base.join(name);
        if direct.exists() {
            return direct;
        }
        self.find_file(&base, name, &["vert.glsl", "frag.glsl", "comp.glsl", "glsl"])
    }

    fn find_file(&self, dir: &Path, name: &str, exts: &[&str]) -> PathBuf {
        // Сначала пробуем имя как есть
        let direct = dir.join(name);
        if direct.exists() {
            return direct;
        }
        // Пробуем с расширениями
        for ext in exts {
            let p = dir.join(format!("{}.{}", name, ext));
            if p.exists() {
                return p;
            }
        }
        panic!("Asset '{}' not found in '{}'", name, dir.display())
    }
}
