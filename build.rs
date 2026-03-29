use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn read_asset_key(manifest_dir: &str) -> String {
    let cargo_toml = fs::read_to_string(format!("{}/Cargo.toml", manifest_dir))
        .expect("Failed to read Cargo.toml");
    let doc: toml::Value = toml::from_str(&cargo_toml).expect("Failed to parse Cargo.toml");
    doc.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("brave"))
        .and_then(|b| b.get("asset_key"))
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string()
}

const AST_SIZE_LIMIT: u64 = 2_500_000_000;

fn main() {
    println!("cargo:rerun-if-changed=assets/");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets_dir = PathBuf::from(&manifest_dir).join("assets");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);
    // OUT_DIR = target/release/build/<pkg>-<hash>/out  →  go up 3 = target/release/
    let deploy_dir = out_path
        .ancestors()
        .nth(3)
        .unwrap_or(&out_path)
        .to_path_buf();

    let xor_key = read_asset_key(&manifest_dir);

    println!("cargo:warning=Compiling assets/ → {}", deploy_dir.display());

    let mut index: Vec<(String, String, u64, u64)> = Vec::new(); // (path, file_letter, begin, end)
    let mut current_letter = b'a';
    let mut current_offset: u64 = 0;
    let mut current_file: Option<fs::File> = None;

    let ensure_file = |letter: &mut u8, offset: &mut u64, file: &mut Option<fs::File>| {
        if file.is_none() {
            let p = deploy_dir.join(format!("x64_{}.ast", *letter as char));
            *file = Some(fs::File::create(&p).expect("Failed to create .ast file"));
            *offset = 0;
        }
    };

    let mut all_files: Vec<PathBuf> = Vec::new();
    collect_files(&assets_dir, &mut all_files);
    all_files.sort();

    for abs_path in &all_files {
        let rel = abs_path.strip_prefix(&assets_dir).unwrap();
        let path_str = rel.to_string_lossy().replace('\\', "/");

        let data = compile_asset(abs_path);
        if data.is_empty() {
            continue;
        }

        let size = data.len() as u64;

        if current_offset + size > AST_SIZE_LIMIT && current_offset > 0 {
            current_letter += 1;
            current_offset = 0;
            current_file = None;
        }

        ensure_file(&mut current_letter, &mut current_offset, &mut current_file);

        let f = current_file.as_mut().unwrap();
        let begin = current_offset;
        f.write_all(&data).expect("Failed to write asset");
        current_offset += size;

        let letter = (current_letter as char).to_string();
        index.push((path_str.clone(), letter, begin, current_offset));

        println!("cargo:warning=Packed \"{}\" ({} bytes)", path_str, size);
    }

    let toml_str = build_toml(&index);
    let encrypted = xor_bytes(toml_str.as_bytes(), xor_key.as_bytes());
    let lock_path = deploy_dir.join("astdb.lock");
    fs::write(&lock_path, &encrypted).expect("Failed to write astdb.lock");

    println!("cargo:warning=Assets done: {} files → {}", index.len(), lock_path.display());
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }
}

fn compile_asset(path: &Path) -> Vec<u8> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let name = path.to_string_lossy();

    match ext {
        "glsl" => compile_shader(path),
        "png" | "jpg" | "jpeg" | "hdr" => compile_image(path),
        "gltf" | "glb" => compile_mesh(path),
        _ => {
            println!("cargo:warning=[assets]   skip  \"{}\"", name);
            Vec::new()
        }
    }
}

fn compile_shader(path: &Path) -> Vec<u8> {
    let src = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read shader {:?}: {}", path, e));

    let name = path.to_string_lossy();
    let stage = if name.contains(".vert") {
        naga::ShaderStage::Vertex
    } else if name.contains(".frag") {
        naga::ShaderStage::Fragment
    } else {
        panic!("Unknown shader stage: {}", name)
    };

    let mut frontend = naga::front::glsl::Frontend::default();
    let module = frontend
        .parse(&naga::front::glsl::Options::from(stage), &src)
        .unwrap_or_else(|e| panic!("GLSL parse error in {:?}: {:?}", path, e));

    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::empty(),
        naga::valid::Capabilities::PUSH_CONSTANT,
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("Shader validation error in {:?}: {:?}", path, e));

    let spv = naga::back::spv::write_vec(
        &module,
        &info,
        &naga::back::spv::Options { lang_version: (1, 0), ..Default::default() },
        None,
    )
    .unwrap_or_else(|e| panic!("SPIR-V error in {:?}: {:?}", path, e));

    spv.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn compile_image(path: &Path) -> Vec<u8> {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("Failed to load image {:?}: {}", path, e))
        .into_rgba8();

    let w = img.width();
    let h = img.height();
    let pixels = img.into_raw();

    let mut out = Vec::with_capacity(8 + pixels.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&pixels);
    out
}

fn compile_mesh(path: &Path) -> Vec<u8> {
    let (doc, buffers, _) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF {:?}: {}", path, e));

    let mut positions_all: Vec<[f32; 3]> = Vec::new();
    let mut normals_all: Vec<[f32; 3]> = Vec::new();
    let mut uvs_all: Vec<[f32; 2]> = Vec::new();
    let mut indices_all: Vec<u32> = Vec::new();

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));
            let positions: Vec<[f32; 3]> = reader.read_positions()
                .unwrap_or_else(|| panic!("GLTF missing positions: {:?}", path))
                .collect();
            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let base = positions_all.len() as u32;
            positions_all.extend_from_slice(&positions);
            normals_all.extend_from_slice(&normals);
            uvs_all.extend_from_slice(&uvs);

            if let Some(iter) = reader.read_indices() {
                for idx in iter.into_u32() {
                    indices_all.push(base + idx);
                }
            } else {
                for i in 0..positions.len() as u32 {
                    indices_all.push(base + i);
                }
            }
        }
    }

    let vc = positions_all.len() as u64;
    let ic = indices_all.len() as u64;

    let mut out = Vec::new();
    out.extend_from_slice(&vc.to_le_bytes());
    out.extend_from_slice(&ic.to_le_bytes());
    for i in 0..positions_all.len() {
        for f in positions_all[i] { out.extend_from_slice(&f.to_le_bytes()); }
        for f in normals_all[i]   { out.extend_from_slice(&f.to_le_bytes()); }
        for f in uvs_all[i]       { out.extend_from_slice(&f.to_le_bytes()); }
    }
    for idx in &indices_all {
        out.extend_from_slice(&idx.to_le_bytes());
    }
    out
}

fn build_toml(index: &[(String, String, u64, u64)]) -> String {
    let mut s = String::new();
    for (path, file, begin, end) in index {
        s.push_str(&format!(
            "[\"{}\"]\nfile = \"{}\"\nbegin = \"{}\"\nend = \"{}\"\n\n",
            path, file, begin, end
        ));
    }
    s
}

fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}
