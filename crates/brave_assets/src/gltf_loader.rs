use std::path::Path;

use brave_render::Vertex;

/// CPU data for one mesh primitive extracted from a GLTF scene.
pub struct PrimitiveData {
    pub name:     String,
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
    pub material: MaterialData,
}

pub struct MaterialData {
    /// Linear RGBA base color factor (default white).
    pub base_color_factor: [f32; 4],
    /// Index into the `EmbeddedTexture` list returned by `load()`, if any.
    pub albedo_tex_index: Option<usize>,
}

/// A texture extracted from a GLTF buffer (embedded or external).
pub struct EmbeddedTexture {
    /// `{model_name}_tex_{index}` — used as the key in textures.ast.
    pub name:   String,
    pub width:  u32,
    pub height: u32,
    /// Always RGBA8.
    pub pixels: Vec<u8>,
}

/// Load all meshes + materials + embedded images from a GLTF/GLB file.
///
/// Returns `(primitives, embedded_textures)`.
pub fn load(path: &Path, model_name: &str) -> (Vec<PrimitiveData>, Vec<EmbeddedTexture>) {
    let (doc, buffers, images) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to import GLTF '{}': {}", path.display(), e));

    // Convert all images to RGBA8
    let embedded: Vec<EmbeddedTexture> = images
        .iter()
        .enumerate()
        .map(|(i, img)| {
            let pixels = to_rgba8(img);
            EmbeddedTexture {
                name:   format!("{}_tex_{}", model_name, i),
                width:  img.width,
                height: img.height,
                pixels,
            }
        })
        .collect();

    let mut primitives = Vec::new();
    let mut prim_idx = 0usize;

    // Walk the node tree depth-first
    let scene = doc.default_scene()
        .or_else(|| doc.scenes().next())
        .expect("GLTF has no scenes");

    let mut stack: Vec<gltf::Node> = scene.nodes().collect();
    while let Some(node) = stack.pop() {
        for child in node.children() {
            stack.push(child);
        }

        let mesh = match node.mesh() {
            Some(m) => m,
            None    => continue,
        };

        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .unwrap_or_else(|| panic!(
                    "GLTF '{}' primitive {} has no positions", path.display(), prim_idx
                ))
                .collect();

            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let vertices: Vec<Vertex> = positions.iter()
                .zip(normals.iter())
                .zip(uvs.iter())
                .map(|((pos, norm), uv)| Vertex {
                    position: *pos,
                    normal:   *norm,
                    uv:       *uv,
                })
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .map(|ri| ri.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            // Material
            let mat       = prim.material();
            let pbr       = mat.pbr_metallic_roughness();
            let factor    = pbr.base_color_factor();
            let tex_index = pbr.base_color_texture()
                .map(|info| info.texture().source().index());

            let name = format!(
                "{}__{}__{}",
                model_name,
                mesh.name().unwrap_or("mesh"),
                prim_idx
            );

            primitives.push(PrimitiveData {
                name,
                vertices,
                indices,
                material: MaterialData {
                    base_color_factor: factor,
                    albedo_tex_index:  tex_index,
                },
            });

            prim_idx += 1;
        }
    }

    log::debug!(
        "GLTF '{}': {} primitives, {} embedded textures",
        path.display(), primitives.len(), embedded.len()
    );

    (primitives, embedded)
}

// ─── helpers ─────────────────────────────────────────────────────────────────

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
        Format::R8 => {
            let mut out = Vec::with_capacity(img.pixels.len() * 4);
            for &v in &img.pixels {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        Format::R8G8 => {
            let mut out = Vec::with_capacity(img.pixels.len() * 2);
            for chunk in img.pixels.chunks(2) {
                out.extend_from_slice(&[chunk[0], chunk[1], 0, 255]);
            }
            out
        }
        other => panic!("GltfLoader: unsupported image format {:?}", other),
    }
}
