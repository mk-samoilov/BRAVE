#[cfg(debug_assertions)]
mod shader_loader;
#[cfg(debug_assertions)]
mod gltf_loader;
#[cfg(debug_assertions)]
mod image_loader;

use std::collections::HashMap;
use std::sync::Arc;
use brv_colors::Color;

pub struct Camera {
    pub fov:  f32,
    pub near: f32,
    pub far:  f32,
}

#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub uv:       [f32; 2],
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
}

pub struct TextureData {
    pub pixels: Vec<u8>,
    pub width:  u32,
    pub height: u32,
}

#[derive(Clone)]
pub struct Material {
    pub albedo:                     Color,
    pub metallic:                   f32,
    pub roughness:                  f32,
    pub emissive:                   Color,
    pub albedo_texture:             Option<Arc<TextureData>>,
    pub metallic_roughness_texture: Option<Arc<TextureData>>,
    pub normal_texture:             Option<Arc<TextureData>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            albedo:                     Color::WHITE,
            metallic:                   0.0,
            roughness:                  0.5,
            emissive:                   Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
            albedo_texture:             None,
            metallic_roughness_texture: None,
            normal_texture:             None,
        }
    }
}

pub struct MeshComponent {
    pub data:     Arc<MeshData>,
    pub material: Material,
}

impl MeshComponent {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self {
            data:     Arc::new(MeshData { vertices, indices }),
            material: Material::default(),
        }
    }
}

pub struct DirectionalLight {
    pub color:     Color,
    pub intensity: f32,
}

pub struct PointLight {
    pub color:     Color,
    pub intensity: f32,
    pub range:     f32,
}

pub struct SpotLight {
    pub color:     Color,
    pub intensity: f32,
    pub range:     f32,
    pub angle:     f32,
}

pub struct AmbientLight {
    pub color:     Color,
    pub intensity: f32,
}

pub enum Light {
    Directional(DirectionalLight),
    Point(PointLight),
    Spot(SpotLight),
    Ambient(AmbientLight),
}

impl From<DirectionalLight> for Light {
    fn from(l: DirectionalLight) -> Light { Light::Directional(l) }
}
impl From<PointLight> for Light {
    fn from(l: PointLight) -> Light { Light::Point(l) }
}
impl From<SpotLight> for Light {
    fn from(l: SpotLight) -> Light { Light::Spot(l) }
}
impl From<AmbientLight> for Light {
    fn from(l: AmbientLight) -> Light { Light::Ambient(l) }
}

pub enum AssetType {
    GLTFModel,
    GLBModel,
    Texture,
    Shader,
}

pub enum AssetData {
    Mesh(MeshComponent),
    Texture(Arc<TextureData>),
    Shader(Arc<Vec<u32>>),
}

impl From<AssetData> for MeshComponent {
    fn from(d: AssetData) -> Self {
        match d {
            AssetData::Mesh(m) => m,
            _ => panic!("expected Mesh asset"),
        }
    }
}

impl From<AssetData> for Arc<TextureData> {
    fn from(d: AssetData) -> Self {
        match d {
            AssetData::Texture(t) => t,
            _ => panic!("expected Texture asset"),
        }
    }
}

enum CachedData {
    Mesh(Arc<MeshData>, Material),
    Texture(Arc<TextureData>),
    Shader(Arc<Vec<u32>>),
}

struct CachedEntry {
    data:       CachedData,
    size_bytes: u64,
    generation: u64,
}

pub struct Assets {
    pub root:          String,
    cache:             HashMap<String, CachedEntry>,
    cache_limit_bytes: u64,
    generation:        u64,
    #[cfg(not(debug_assertions))]
    ast_index:         HashMap<String, AstEntry>,
}

#[cfg(not(debug_assertions))]
#[derive(Clone)]
struct AstEntry {
    file:  String,
    begin: u64,
    end:   u64,
}

impl Assets {
    pub fn new(root: &str) -> Self {
        #[cfg(debug_assertions)]
        {
            log::info!("Assets manager initialized: Debug, root=\"{}\"", root);
            Self {
                root:              root.to_string(),
                cache:             HashMap::new(),
                cache_limit_bytes: 512 * 1024 * 1024,
                generation:        0,
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let lock_path = exe_dir.join("astdb.lock");
            log::info!("Assets manager initialized: Release, astdb=\"{}\"", lock_path.display());
            let compiled_key = env!("BRAVE_COMPILED_ASSET_KEY");
            let index = Self::load_ast_index(&lock_path, compiled_key);
            log::info!("Asset index loaded: {} entries", index.len());
            Self {
                root:              root.to_string(),
                cache:             HashMap::new(),
                cache_limit_bytes: 512 * 1024 * 1024,
                generation:        0,
                ast_index:         index,
            }
        }
    }

    #[cfg(not(debug_assertions))]
    fn load_ast_index(lock_path: &std::path::Path, xor_key: &str) -> HashMap<String, AstEntry> {
        let encrypted = std::fs::read(lock_path)
            .unwrap_or_else(|e| panic!("Failed to read astdb.lock: {}", e));
        let decrypted = xor_bytes(&encrypted, xor_key.as_bytes());
        let toml_str = String::from_utf8(decrypted)
            .expect("astdb.lock is not valid UTF-8 after decryption");
        let doc: toml::Value = toml::from_str(&toml_str)
            .expect("astdb.lock is not valid TOML");
        let mut index = HashMap::new();
        if let toml::Value::Table(table) = doc {
            for (key, val) in table {
                if let toml::Value::Table(entry) = val {
                    let file  = entry.get("file").and_then(|v| v.as_str()).unwrap_or("a").to_string();
                    let begin = entry.get("begin").and_then(|v| v.as_str()).unwrap_or("0").parse::<u64>().unwrap_or(0);
                    let end   = entry.get("end").and_then(|v| v.as_str()).unwrap_or("0").parse::<u64>().unwrap_or(0);
                    index.insert(key, AstEntry { file, begin, end });
                }
            }
        }
        index
    }

    #[cfg(not(debug_assertions))]
    fn read_ast_entry(&self, path: &str) -> Vec<u8> {
        let entry = self.ast_index.get(path)
            .unwrap_or_else(|| panic!("Assets: \"{}\" not found in astdb.lock", path));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let ast_path = exe_dir.join(format!("x64_{}.ast", entry.file));
        let mut f = std::fs::File::open(&ast_path)
            .unwrap_or_else(|e| panic!("Failed to open {}: {}", ast_path.display(), e));
        use std::io::Read;
        std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(entry.begin)).unwrap();
        let size = (entry.end - entry.begin) as usize;
        let mut buf = vec![0u8; size];
        f.read_exact(&mut buf).unwrap();
        buf
    }

    pub fn set_cache_limit(&mut self, mb: u64) {
        self.cache_limit_bytes = mb * 1024 * 1024;
    }

    pub fn get_cache_limit(&self) -> u64 {
        self.cache_limit_bytes / 1024 / 1024
    }

    pub fn load(&mut self, path: &str, asset_type: AssetType) -> AssetData {
        self.generation += 1;
        let stamp = self.generation;

        if let Some(entry) = self.cache.get_mut(path) {
            entry.generation = stamp;
            log::debug!("Cache hit: \"{}\"", path);
            return Self::make_asset_data(&entry.data);
        }

        #[cfg(not(debug_assertions))]
        let (cached, size) = {
            let ast_key = match asset_type {
                AssetType::GLTFModel => format!("{}/scene.gltf", path),
                AssetType::GLBModel  => path.to_string(),
                _                    => path.to_string(),
            };
            let raw = self.read_ast_entry(&ast_key);
            match asset_type {
                AssetType::Shader => {
                    let spv: Vec<u32> = raw.chunks_exact(4)
                        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();
                    let size = raw.len() as u64;
                    log::info!("Shader ready: \"{}\" ({} bytes spv)", path, size);
                    (CachedData::Shader(Arc::new(spv)), size)
                }
                AssetType::Texture => {
                    let w = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                    let h = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
                    let pixels = raw[8..].to_vec();
                    let size = pixels.len() as u64;
                    log::info!("Texture ready: \"{}\" ({}x{} {} bytes)", path, w, h, size);
                    (CachedData::Texture(Arc::new(TextureData { pixels, width: w, height: h })), size)
                }
                AssetType::GLTFModel | AssetType::GLBModel => {
                    let vc = u64::from_le_bytes(raw[0..8].try_into().unwrap()) as usize;
                    let ic = u64::from_le_bytes(raw[8..16].try_into().unwrap()) as usize;
                    let mut vertices = Vec::with_capacity(vc);
                    let stride = 32;
                    for i in 0..vc {
                        let base = 16 + i * stride;
                        let mut f = [0f32; 8];
                        for j in 0..8 {
                            let b = &raw[base + j*4..base + j*4 + 4];
                            f[j] = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        }
                        vertices.push(Vertex {
                            position: [f[0], f[1], f[2]],
                            normal:   [f[3], f[4], f[5]],
                            uv:       [f[6], f[7]],
                        });
                    }
                    let idx_start = 16 + vc * stride;
                    let mut indices = Vec::with_capacity(ic);
                    for i in 0..ic {
                        let b = &raw[idx_start + i*4..idx_start + i*4 + 4];
                        indices.push(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                    }
                    let md = Arc::new(MeshData { vertices, indices });
                    let size = Self::mesh_size_bytes(&md);
                    log::info!("Mesh ready: \"{}\" ({} verts, {} idx, {} bytes)", path, vc, ic, size);
                    (CachedData::Mesh(md, Material::default()), size)
                }
            }
        };

        #[cfg(debug_assertions)]
        let full_path = format!("{}{}", self.root, path);

        #[cfg(debug_assertions)]
        let (cached, size) = match asset_type {
            AssetType::Shader => {
                log::debug!("Loading shader: \"{}\"", full_path);
                let spv = Arc::new(shader_loader::load(&full_path));
                let size = spv.len() as u64 * 4;
                log::info!("Shader ready: \"{}\" ({} bytes spv)", path, size);
                (CachedData::Shader(spv), size)
            }
            AssetType::Texture => {
                log::debug!("Loading texture: \"{}\"", full_path);
                let td = Arc::new(image_loader::load(&full_path));
                let size = td.pixels.len() as u64;
                log::info!("Texture ready: \"{}\" ({}x{} {} bytes)", path, td.width, td.height, size);
                (CachedData::Texture(td), size)
            }
            AssetType::GLTFModel => {
                log::debug!("Loading gltf: \"{}\"", full_path);
                let (mesh_data, mat) = gltf_loader::load_gltf(&full_path);
                let md = Arc::new(mesh_data);
                let size = Self::mesh_size_bytes(&md);
                log::info!("Mesh ready: \"{}\" ({} verts, {} idx, {} bytes)", path, md.vertices.len(), md.indices.len(), size);
                (CachedData::Mesh(md, mat), size)
            }
            AssetType::GLBModel => {
                log::debug!("Loading glb: \"{}\"", full_path);
                let (mesh_data, mat) = gltf_loader::load_glb(&full_path);
                let md = Arc::new(mesh_data);
                let size = Self::mesh_size_bytes(&md);
                log::info!("Mesh ready: \"{}\" ({} verts, {} idx, {} bytes)", path, md.vertices.len(), md.indices.len(), size);
                (CachedData::Mesh(md, mat), size)
            }
        };

        self.evict_if_needed(size);

        let asset_data = Self::make_asset_data(&cached);
        self.cache.insert(
            path.to_string(),
            CachedEntry { data: cached, size_bytes: size, generation: stamp },
        );
        asset_data
    }

    pub fn load_shader_spv(&mut self, path: &str) -> Arc<Vec<u32>> {
        match self.load(path, AssetType::Shader) {
            AssetData::Shader(spv) => spv,
            _ => unreachable!(),
        }
    }

    fn make_asset_data(cached: &CachedData) -> AssetData {
        match cached {
            CachedData::Mesh(md, mat) => AssetData::Mesh(MeshComponent {
                data:     Arc::clone(md),
                material: mat.clone(),
            }),
            CachedData::Texture(td) => AssetData::Texture(Arc::clone(td)),
            CachedData::Shader(spv) => AssetData::Shader(Arc::clone(spv)),
        }
    }

    fn mesh_size_bytes(md: &MeshData) -> u64 {
        (md.vertices.len() * std::mem::size_of::<Vertex>() + md.indices.len() * 4) as u64
    }

    fn evict_if_needed(&mut self, incoming: u64) {
        loop {
            let total: u64 = self.cache.values().map(|e| e.size_bytes).sum();
            if total + incoming <= self.cache_limit_bytes || self.cache.is_empty() {
                break;
            }
            let oldest = self
                .cache
                .iter()
                .min_by_key(|(_, e)| e.generation)
                .map(|(k, _)| k.clone())
                .unwrap();
            log::info!("Evicting from cache: \"{}\"", oldest);
            self.cache.remove(&oldest);
        }
    }
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}
