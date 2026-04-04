use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use glam::Mat4;

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

    println!("cargo:warning=Compiling assets/ -> {}", deploy_dir.display());

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

    println!("cargo:warning=Done pack assets (total: {}): \"{}\"", index.len(), lock_path.display());
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
            println!("cargo:warning=Skip asset: \"{}\"", name);
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

struct MaterialRaw {
    albedo:    [f32; 4],
    metallic:  f32,
    roughness: f32,
    emissive:  [f32; 3],
    albedo_tex:  Option<(u32, u32, Vec<u8>)>,
    mr_tex:      Option<(u32, u32, Vec<u8>)>,
    normal_tex:  Option<(u32, u32, Vec<u8>)>,
}

impl Default for MaterialRaw {
    fn default() -> Self {
        Self {
            albedo:    [1.0, 1.0, 1.0, 1.0],
            metallic:  0.0,
            roughness: 0.5,
            emissive:  [0.0, 0.0, 0.0],
            albedo_tex:  None,
            mr_tex:      None,
            normal_tex:  None,
        }
    }
}

fn to_rgba8(data: &gltf::image::Data) -> (u32, u32, Vec<u8>) {
    use gltf::image::Format;
    let pixels = match data.format {
        Format::R8G8B8A8 => data.pixels.clone(),
        Format::R8G8B8   => data.pixels.chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255u8])
            .collect(),
        _ => vec![255u8; (data.width * data.height * 4) as usize],
    };
    (data.width, data.height, pixels)
}

fn write_optional_tex(out: &mut Vec<u8>, tex: &Option<(u32, u32, Vec<u8>)>) {
    match tex {
        Some((w, h, pixels)) => {
            out.push(1u8);
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            out.extend_from_slice(pixels);
        }
        None => out.push(0u8),
    }
}

fn compile_mesh(path: &Path) -> Vec<u8> {
    let (doc, buffers, images) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF {:?}: {}", path, e));

    let mut positions_all: Vec<[f32; 3]> = Vec::new();
    let mut normals_all: Vec<[f32; 3]> = Vec::new();
    let mut uvs_all: Vec<[f32; 2]> = Vec::new();
    let mut indices_all: Vec<u32> = Vec::new();
    let mut material = MaterialRaw::default();

    let scene = doc.default_scene().or_else(|| doc.scenes().next())
        .unwrap_or_else(|| panic!("GLTF has no scene: {:?}", path));

    for node in scene.nodes() {
        collect_node_mesh(
            &node, glam::Mat4::IDENTITY, &buffers, &images,
            &mut positions_all, &mut normals_all, &mut uvs_all, &mut indices_all,
            &mut material,
        );
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

    for f in material.albedo    { out.extend_from_slice(&f.to_le_bytes()); }
    out.extend_from_slice(&material.metallic.to_le_bytes());
    out.extend_from_slice(&material.roughness.to_le_bytes());
    for f in material.emissive  { out.extend_from_slice(&f.to_le_bytes()); }
    write_optional_tex(&mut out, &material.albedo_tex);
    write_optional_tex(&mut out, &material.mr_tex);
    write_optional_tex(&mut out, &material.normal_tex);

    out
}

fn collect_node_mesh(
    node:             &gltf::Node,
    parent_transform: Mat4,
    buffers:          &[gltf::buffer::Data],
    images:           &[gltf::image::Data],
    positions_all:    &mut Vec<[f32; 3]>,
    normals_all:      &mut Vec<[f32; 3]>,
    uvs_all:          &mut Vec<[f32; 2]>,
    indices_all:      &mut Vec<u32>,
    material:         &mut MaterialRaw,
) {
    let local      = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global     = parent_transform * local;
    let normal_mat = global.inverse().transpose();

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let pbr = prim.material().pbr_metallic_roughness();
            let [r, g, b, a] = pbr.base_color_factor();
            let [er, eg, eb] = prim.material().emissive_factor();
            *material = MaterialRaw {
                albedo:    [r, g, b, a],
                metallic:  pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                emissive:  [er, eg, eb],
                albedo_tex: pbr.base_color_texture()
                    .map(|t| to_rgba8(&images[t.texture().source().index()])),
                mr_tex: pbr.metallic_roughness_texture()
                    .map(|t| to_rgba8(&images[t.texture().source().index()])),
                normal_tex: prim.material().normal_texture()
                    .map(|t| to_rgba8(&images[t.texture().source().index()])),
            };

            let positions: Vec<[f32; 3]> = reader.read_positions()
                .unwrap_or_else(|| panic!("GLTF missing positions"))
                .collect();
            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let base = positions_all.len() as u32;

            for i in 0..positions.len() {
                let p = global.transform_point3(glam::Vec3::from(positions[i]));
                let n = normal_mat.transform_vector3(glam::Vec3::from(normals[i])).normalize();
                positions_all.push(p.into());
                normals_all.push(n.into());
                uvs_all.push(uvs[i]);
            }

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

    for child in node.children() {
        collect_node_mesh(&child, global, buffers, images, positions_all, normals_all, uvs_all, indices_all, material);
    }
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
