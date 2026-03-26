use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAGIC: &[u8; 4] = b"BRAV";
const VERSION: u32 = 1;
const NAME_LEN: usize = 64;
const ENTRY_SIZE: usize = NAME_LEN + 4 + 8 + 8;
const HEADER_SIZE: usize = 4 + 4 + 4;

const ASSET_TYPE_MODEL:   u32 = 0;
const ASSET_TYPE_TEXTURE: u32 = 1;
const ASSET_TYPE_SHADER:  u32 = 2;

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        return;
    }

    let out_dir    = std::env::var("OUT_DIR").unwrap();
    let bin_dir    = PathBuf::from(&out_dir).join("../../..").canonicalize().unwrap();

    let manifest_dir   = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(&manifest_dir).join("../..").canonicalize().unwrap();
    let assets_dir     = workspace_root.join("assets");

    if !assets_dir.exists() {
        eprintln!("brave_assets build.rs: assets/ not found, skipping .ast packing");
        return;
    }

    println!("cargo:rerun-if-changed={}", assets_dir.display());

    pack_models(&assets_dir, &bin_dir);
    pack_textures(&assets_dir, &bin_dir);
    pack_shaders(&assets_dir, &bin_dir);
}

fn pack_models(assets_dir: &Path, out_dir: &Path) {
    let models_dir = assets_dir.join("models");
    let out_path   = out_dir.join("models.ast");

    let files = collect_files(&models_dir, &["glb", "gltf"]);
    if files.is_empty() {
        write_empty_ast(&out_path);
        return;
    }

    let mut entries_data: Vec<(String, Vec<u8>)> = Vec::new();

    for path in &files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let (vertices, indices) = load_gltf(path);

        let vertex_count = vertices.len() as u32;
        let index_count  = indices.len() as u32;

        let mut data = Vec::new();
        data.extend_from_slice(&vertex_count.to_le_bytes());
        data.extend_from_slice(&index_count.to_le_bytes());
        for v in &vertices {
            for &f in v { data.extend_from_slice(&f.to_le_bytes()); }
        }
        for &idx in &indices {
            data.extend_from_slice(&idx.to_le_bytes());
        }

        entries_data.push((name, data));
        println!("cargo:rerun-if-changed={}", path.display());
    }

    write_ast(&out_path, ASSET_TYPE_MODEL, &entries_data);
    eprintln!("brave_assets: packed {} models → models.ast", entries_data.len());
}

fn pack_textures(assets_dir: &Path, out_dir: &Path) {
    let tex_dir  = assets_dir.join("textures");
    let out_path = out_dir.join("textures.ast");

    let files = collect_files(&tex_dir, &["png", "jpg", "jpeg", "hdr"]);
    if files.is_empty() {
        write_empty_ast(&out_path);
        return;
    }

    let mut entries_data: Vec<(String, Vec<u8>)> = Vec::new();

    for path in &files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let img  = image::open(path)
            .unwrap_or_else(|e| panic!("Failed to load image '{}': {}", path.display(), e));
        let rgba  = img.to_rgba8();
        let (w, h) = rgba.dimensions();

        let mut data = Vec::new();
        data.extend_from_slice(&w.to_le_bytes());
        data.extend_from_slice(&h.to_le_bytes());
        data.extend_from_slice(rgba.as_raw());

        entries_data.push((name, data));
        println!("cargo:rerun-if-changed={}", path.display());
    }

    write_ast(&out_path, ASSET_TYPE_TEXTURE, &entries_data);
    eprintln!("brave_assets: packed {} textures → textures.ast", entries_data.len());
}

fn pack_shaders(assets_dir: &Path, out_dir: &Path) {
    let shaders_dir = assets_dir.join("shaders");
    let out_path    = out_dir.join("shaders.ast");

    let files = collect_files(&shaders_dir, &["glsl"]);
    if files.is_empty() {
        write_empty_ast(&out_path);
        return;
    }

    let mut entries_data: Vec<(String, Vec<u8>)> = Vec::new();

    for path in &files {
        let full_name = path.file_name().unwrap().to_string_lossy();
        let name = strip_last_ext(&full_name);

        let tmp = std::env::temp_dir().join(format!("brave_build_{}.spv", name.replace('.', "_")));
        let status = Command::new("glslangValidator")
            .args(["-V", path.to_str().unwrap(), "-o", tmp.to_str().unwrap()])
            .status()
            .expect("glslangValidator not found: sudo apt install glslang-tools");
        assert!(status.success(), "Shader compilation failed: {}", path.display());

        let spv = fs::read(&tmp)
            .unwrap_or_else(|e| panic!("Failed to read SPIR-V '{}': {}", tmp.display(), e));

        entries_data.push((name.to_string(), spv));
        println!("cargo:rerun-if-changed={}", path.display());
    }

    write_ast(&out_path, ASSET_TYPE_SHADER, &entries_data);
    eprintln!("brave_assets: packed {} shaders → shaders.ast", entries_data.len());
}

fn write_ast(out_path: &Path, asset_type: u32, entries: &[(String, Vec<u8>)]) {
    let count = entries.len();
    let data_start = (HEADER_SIZE + count * ENTRY_SIZE) as u64;

    let mut buf = Vec::new();

    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(count as u32).to_le_bytes());

    let mut offset = data_start;
    for (name, data) in entries {
        let mut name_buf = [0u8; NAME_LEN];
        let bytes = name.as_bytes();
        let len = bytes.len().min(NAME_LEN - 1);
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
        .unwrap_or_else(|e| panic!("Failed to write '{}': {}", out_path.display(), e));
}

fn write_empty_ast(out_path: &Path) {
    let mut buf = Vec::new();
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    fs::write(out_path, &buf).ok();
}

fn collect_files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
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

fn load_gltf(path: &Path) -> (Vec<[f32; 8]>, Vec<u32>) {
    let (doc, buffers, _) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF '{}': {}", path.display(), e));

    let mesh = doc.meshes().next()
        .unwrap_or_else(|| panic!("No meshes in '{}'", path.display()));
    let prim = mesh.primitives().next()
        .unwrap_or_else(|| panic!("No primitives in '{}'", path.display()));

    let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

    let positions: Vec<[f32; 3]> = reader.read_positions()
        .unwrap_or_else(|| panic!("No positions in '{}'", path.display()))
        .collect();

    let normals: Vec<[f32; 3]> = reader.read_normals()
        .map(|it| it.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

    let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
        .map(|tc| tc.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let vertices: Vec<[f32; 8]> = positions.iter().zip(normals.iter()).zip(uvs.iter())
        .map(|((p, n), uv)| [p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]])
        .collect();

    let indices: Vec<u32> = reader.read_indices()
        .map(|ri| ri.into_u32().collect())
        .unwrap_or_else(|| (0..positions.len() as u32).collect());

    (vertices, indices)
}
