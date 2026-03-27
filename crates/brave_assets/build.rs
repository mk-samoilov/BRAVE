//! Build-time asset pipeline (runs only in release builds).
//!
//! Produces three archives next to the binary:
//!   models.ast   — all GLTF primitives (multi-prim binary format "BRMM")
//!   textures.ast — filesystem textures + embedded GLTF textures
//!   shaders.ast  — compiled SPIR-V
//!
//! This mirrors Unity's build step: source assets in assets/, baked output
//! packed alongside the executable.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── .ast shared constants ────────────────────────────────────────────────────

const MAGIC: &[u8; 4] = b"BRAV";
const VERSION: u32 = 1;
const NAME_LEN: usize = 64;
const ENTRY_SIZE: usize = NAME_LEN + 4 + 8 + 8;
const HEADER_SIZE: usize = 4 + 4 + 4;

const ASSET_TYPE_MODEL: u32 = 0;
const ASSET_TYPE_TEXTURE: u32 = 1;
const ASSET_TYPE_SHADER: u32 = 2;

/// Multi-primitive model magic ("BRMM" = BRAVE Multi-Mesh).
const MODEL_MAGIC: &[u8; 4] = b"BRMM";

// ─── Vertex layout (must match brave_render::Vertex) ─────────────────────────

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    normal:   [f32; 3],
    uv:       [f32; 2],
}

// ─── entry point ─────────────────────────────────────────────────────────────

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        return;
    }

    let out_dir        = std::env::var("OUT_DIR").unwrap();
    let bin_dir        = PathBuf::from(&out_dir).join("../../..").canonicalize().unwrap();
    let manifest_dir   = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(&manifest_dir).join("../..").canonicalize().unwrap();
    let assets_dir     = workspace_root.join("assets");

    if !assets_dir.exists() {
        eprintln!("brave_assets build.rs: assets/ not found, skipping packing");
        return;
    }

    println!("cargo:rerun-if-changed={}", assets_dir.display());

    // Process GLTF files first to collect embedded textures
    let (model_entries, gltf_textures) = process_models(&assets_dir);
    write_ast(&bin_dir.join("models.ast"), ASSET_TYPE_MODEL, &model_entries);
    eprintln!("brave_assets: packed {} models → models.ast", model_entries.len());

    // Merge filesystem textures + GLTF embedded textures (dedup by name)
    let texture_entries = process_textures(&assets_dir, gltf_textures);
    write_ast(&bin_dir.join("textures.ast"), ASSET_TYPE_TEXTURE, &texture_entries);
    eprintln!("brave_assets: packed {} textures → textures.ast", texture_entries.len());

    pack_shaders(&assets_dir, &bin_dir);
}

// ─── Models ──────────────────────────────────────────────────────────────────

struct EmbeddedTexture {
    name:   String,
    width:  u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Process every .glb/.gltf file.
/// Returns `(model_entries, embedded_textures_from_all_models)`.
fn process_models(assets_dir: &Path) -> (Vec<(String, Vec<u8>)>, Vec<EmbeddedTexture>) {
    let models_dir  = assets_dir.join("models");
    let model_files = collect_files(&models_dir, &["glb", "gltf"]);

    let mut model_entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut all_textures: Vec<EmbeddedTexture>    = Vec::new();

    for path in &model_files {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        println!("cargo:rerun-if-changed={}", path.display());

        let (primitives, embedded) = load_gltf(path, &stem);
        all_textures.extend(embedded);

        let data = encode_model(&primitives);
        model_entries.push((stem, data));
    }

    (model_entries, all_textures)
}

struct PrimitiveBuild {
    vertices:          Vec<Vertex>,
    indices:           Vec<u32>,
    base_color_factor: [f32; 4],
    /// Name of the albedo texture in textures.ast (empty = none).
    albedo_tex_name:   String,
}

fn load_gltf(path: &Path, model_name: &str) -> (Vec<PrimitiveBuild>, Vec<EmbeddedTexture>) {
    let (doc, buffers, images) = gltf::import(path)
        .unwrap_or_else(|e| panic!("build.rs: failed to import '{}': {}", path.display(), e));

    // Convert embedded images to RGBA8
    let embedded: Vec<EmbeddedTexture> = images.iter().enumerate().map(|(i, img)| {
        EmbeddedTexture {
            name:   format!("{}_tex_{}", model_name, i),
            width:  img.width,
            height: img.height,
            pixels: to_rgba8(img),
        }
    }).collect();

    let mut primitives = Vec::new();

    let scene = doc.default_scene().or_else(|| doc.scenes().next())
        .expect("GLTF has no scenes");

    let mut stack: Vec<gltf::Node> = scene.nodes().collect();
    while let Some(node) = stack.pop() {
        for child in node.children() { stack.push(child); }
        let mesh = match node.mesh() { Some(m) => m, None => continue };

        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let positions: Vec<[f32; 3]> = reader.read_positions()
                .unwrap_or_else(|| panic!("build.rs: no positions in '{}'", path.display()))
                .collect();

            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let vertices: Vec<Vertex> = positions.iter()
                .zip(normals.iter())
                .zip(uvs.iter())
                .map(|((p, n), uv)| Vertex { position: *p, normal: *n, uv: *uv })
                .collect();

            let indices: Vec<u32> = reader.read_indices()
                .map(|ri| ri.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mat    = prim.material();
            let pbr    = mat.pbr_metallic_roughness();
            let factor = pbr.base_color_factor();
            let tex_name = pbr.base_color_texture()
                .map(|info| format!("{}_tex_{}", model_name, info.texture().source().index()))
                .unwrap_or_default();

            primitives.push(PrimitiveBuild {
                vertices,
                indices,
                base_color_factor: factor,
                albedo_tex_name:   tex_name,
            });

        }
    }

    (primitives, embedded)
}

/// Encode all primitives as "BRMM" binary:
/// [MAGIC 4B][prim_count u32]
/// for each prim:
///   [vtx_count u32][idx_count u32][color f32×4]
///   [tex_name_len u32][tex_name UTF-8 bytes]
///   [vertices vtx_count×32B][indices idx_count×4B]
fn encode_model(primitives: &[PrimitiveBuild]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(MODEL_MAGIC);
    buf.extend_from_slice(&(primitives.len() as u32).to_le_bytes());

    for p in primitives {
        buf.extend_from_slice(&(p.vertices.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(p.indices.len() as u32).to_le_bytes());
        for &f in &p.base_color_factor {
            buf.extend_from_slice(&f.to_le_bytes());
        }
        let name_bytes = p.albedo_tex_name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        for v in &p.vertices {
            for &f in &v.position { buf.extend_from_slice(&f.to_le_bytes()); }
            for &f in &v.normal   { buf.extend_from_slice(&f.to_le_bytes()); }
            for &f in &v.uv       { buf.extend_from_slice(&f.to_le_bytes()); }
        }
        for &idx in &p.indices {
            buf.extend_from_slice(&idx.to_le_bytes());
        }
    }

    buf
}

// ─── Textures ────────────────────────────────────────────────────────────────

/// Encode texture as [width u32][height u32][RGBA8 pixels].
fn encode_texture(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + pixels.len());
    data.extend_from_slice(&width.to_le_bytes());
    data.extend_from_slice(&height.to_le_bytes());
    data.extend_from_slice(pixels);
    data
}

fn process_textures(
    assets_dir:    &Path,
    gltf_textures: Vec<EmbeddedTexture>,
) -> Vec<(String, Vec<u8>)> {
    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();

    // 1. Filesystem textures
    let tex_dir = assets_dir.join("textures");
    for path in collect_files(&tex_dir, &["png", "jpg", "jpeg", "hdr"]) {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        println!("cargo:rerun-if-changed={}", path.display());
        let img  = image::open(&path)
            .unwrap_or_else(|e| panic!("build.rs: failed to open '{}': {}", path.display(), e));
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        entries.insert(name, encode_texture(w, h, rgba.as_raw()));
    }

    // 2. GLTF embedded textures (added after, so filesystem textures win on name collision)
    for tex in gltf_textures {
        entries.entry(tex.name).or_insert_with(|| encode_texture(tex.width, tex.height, &tex.pixels));
    }

    entries.into_iter().collect()
}

// ─── Shaders ─────────────────────────────────────────────────────────────────

fn pack_shaders(assets_dir: &Path, bin_dir: &Path) {
    let shaders_dir = assets_dir.join("shaders");
    let out_path    = bin_dir.join("shaders.ast");

    let files = collect_files(&shaders_dir, &["glsl"]);
    if files.is_empty() {
        write_empty_ast(&out_path);
        return;
    }

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for path in &files {
        let full_name = path.file_name().unwrap().to_string_lossy();
        let name      = strip_last_ext(&full_name).to_string();
        println!("cargo:rerun-if-changed={}", path.display());

        let tmp = std::env::temp_dir()
            .join(format!("brave_build_{}.spv", name.replace('.', "_")));
        let status = Command::new("glslangValidator")
            .args(["-V", path.to_str().unwrap(), "-o", tmp.to_str().unwrap()])
            .status()
            .expect("glslangValidator not found: sudo apt install glslang-tools");
        assert!(status.success(), "Shader compilation failed: {}", path.display());

        let spv = fs::read(&tmp)
            .unwrap_or_else(|e| panic!("build.rs: failed to read SPIR-V '{}': {}", tmp.display(), e));
        entries.push((name, spv));
    }

    write_ast(&out_path, ASSET_TYPE_SHADER, &entries);
    eprintln!("brave_assets: packed {} shaders → shaders.ast", entries.len());
}

// ─── .ast format ─────────────────────────────────────────────────────────────

fn write_ast(out_path: &Path, asset_type: u32, entries: &[(String, Vec<u8>)]) {
    let count      = entries.len();
    let data_start = (HEADER_SIZE + count * ENTRY_SIZE) as u64;

    let mut buf = Vec::new();
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(count as u32).to_le_bytes());

    let mut offset = data_start;
    for (name, data) in entries {
        let mut name_buf = [0u8; NAME_LEN];
        let bytes = name.as_bytes();
        let len   = bytes.len().min(NAME_LEN - 1);
        name_buf[..len].copy_from_slice(&bytes[..len]);
        buf.extend_from_slice(&name_buf);
        buf.extend_from_slice(&asset_type.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
        offset += data.len() as u64;
    }
    for (_, data) in entries {
        buf.extend_from_slice(data);
    }

    fs::write(out_path, &buf)
        .unwrap_or_else(|e| panic!("build.rs: failed to write '{}': {}", out_path.display(), e));
}

fn write_empty_ast(out_path: &Path) {
    let mut buf = Vec::new();
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    fs::write(out_path, &buf).ok();
}

// ─── gltf image helpers ───────────────────────────────────────────────────────

fn to_rgba8(img: &gltf::image::Data) -> Vec<u8> {
    use gltf::image::Format;
    match img.format {
        Format::R8G8B8A8 => img.pixels.clone(),
        Format::R8G8B8 => {
            let mut out = Vec::with_capacity(img.pixels.len() / 3 * 4);
            for chunk in img.pixels.chunks(3) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }
        Format::R8 => img.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        Format::R8G8 => img.pixels.chunks(2)
            .flat_map(|c| [c[0], c[1], 0u8, 255u8])
            .collect(),
        other => panic!("build.rs: unsupported image format {:?}", other),
    }
}

// ─── utilities ────────────────────────────────────────────────────────────────

fn collect_files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    if !dir.exists() { return Vec::new(); }
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if exts.contains(&ext) {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files
}

fn strip_last_ext(name: &str) -> &str {
    if let Some(pos) = name.rfind('.') { &name[..pos] } else { name }
}
