use std::path::Path;

use brave_math::{Mat3, Mat4, Vec3};
use brave_render::Vertex;

/// CPU data for one mesh primitive extracted from a GLTF scene.
pub struct PrimitiveData {
    pub name:     String,
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
    pub material: MaterialData,
}

pub struct MaterialData {
    pub base_color_factor: [f32; 4],
    pub metallic_factor:   f32,
    pub roughness_factor:  f32,
    pub albedo_tex_index:  Option<usize>,
    pub normal_tex_index:  Option<usize>,
    pub orm_tex_index:     Option<usize>,
}

/// A texture extracted from a GLTF file.
pub struct EmbeddedTexture {
    pub name:   String,
    pub width:  u32,
    pub height: u32,
    /// Always RGBA8.
    pub pixels: Vec<u8>,
    /// True for linear-space textures (normal maps). False for sRGB (albedo).
    pub linear: bool,
}


/// Load all meshes + materials + only albedo images from a GLTF/GLB file.
/// Node transforms are baked into vertex positions and normals.
pub fn load(path: &Path, model_name: &str) -> (Vec<PrimitiveData>, Vec<EmbeddedTexture>) {
    let base = path.parent().unwrap_or(Path::new("."));

    let gltf::Gltf { document, blob } = gltf::Gltf::open(path)
        .unwrap_or_else(|e| panic!("Failed to open GLTF '{}': {}", path.display(), e));
    let buffers = gltf::import_buffers(&document, Some(base), blob)
        .unwrap_or_else(|e| panic!("Failed to import buffers '{}': {}", path.display(), e));

    // Collect image indices used as albedo and normal maps.
    let albedo_indices: std::collections::HashSet<usize> = document
        .materials()
        .filter_map(|m| m.pbr_metallic_roughness().base_color_texture())
        .map(|t| t.texture().source().index())
        .collect();
    let normal_indices: std::collections::HashSet<usize> = document
        .materials()
        .filter_map(|m| m.normal_texture())
        .map(|t| t.texture().source().index())
        .collect();
    let orm_indices: std::collections::HashSet<usize> = document
        .materials()
        .filter_map(|m| m.pbr_metallic_roughness().metallic_roughness_texture())
        .map(|t| t.texture().source().index())
        .collect();
    let needed_indices: std::collections::HashSet<usize> = albedo_indices
        .union(&normal_indices).copied()
        .chain(orm_indices.iter().copied())
        .collect();

    let embedded: Vec<EmbeddedTexture> = document
        .images()
        .enumerate()
        .filter(|(i, _)| needed_indices.contains(i))
        .map(|(i, img)| {
            let (width, height, pixels) = load_image(img.source(), base, &buffers);
            let linear = normal_indices.contains(&i) || orm_indices.contains(&i);
            EmbeddedTexture { name: format!("{}_tex_{}", model_name, i), width, height, pixels, linear }
        })
        .collect();

    // Walk the node tree depth-first, accumulating world transforms.
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .expect("GLTF has no scenes");

    let mut primitives = Vec::new();
    let mut prim_idx   = 0usize;

    // Stack holds (node, accumulated_world_transform)
    let mut stack: Vec<(gltf::Node, Mat4)> = scene
        .nodes()
        .map(|n| (n, Mat4::IDENTITY))
        .collect();

    while let Some((node, parent_world)) = stack.pop() {
        let local: Mat4 = Mat4::from_cols_array_2d(&node.transform().matrix());
        let world = parent_world * local;

        for child in node.children() {
            stack.push((child, world));
        }

        let mesh = match node.mesh() {
            Some(m) => m,
            None    => continue,
        };

        // Normal matrix = inverse transpose of upper-left 3×3 of world transform.
        let normal_mat = Mat3::from_mat4(world).inverse().transpose();

        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .unwrap_or_else(|| panic!(
                    "GLTF '{}' primitive {} has no positions", path.display(), prim_idx
                ))
                .collect();

            let normals_raw: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let vertices: Vec<Vertex> = positions.iter()
                .zip(normals_raw.iter())
                .zip(uvs.iter())
                .map(|((pos, norm), uv)| {
                    // Apply world transform to position (w=1)
                    let p = world.transform_point3(Vec3::from_array(*pos));
                    // Apply normal matrix (inverse transpose) to normal (w=0)
                    let n = (normal_mat * Vec3::from_array(*norm)).normalize_or_zero();
                    Vertex { position: p.to_array(), normal: n.to_array(), uv: *uv }
                })
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .map(|ri| ri.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mat       = prim.material();
            let pbr       = mat.pbr_metallic_roughness();
            let factor    = pbr.base_color_factor();
            let metallic  = pbr.metallic_factor();
            let roughness = pbr.roughness_factor();
            let tex_index = pbr.base_color_texture()
                .map(|info| info.texture().source().index());
            let normal_index = mat.normal_texture()
                .map(|info| info.texture().source().index());
            let orm_index = pbr.metallic_roughness_texture()
                .map(|info| info.texture().source().index());

            let name = format!("{}__{}_{}", model_name, mesh.name().unwrap_or("mesh"), prim_idx);
            primitives.push(PrimitiveData {
                name,
                vertices,
                indices,
                material: MaterialData {
                    base_color_factor: factor,
                    metallic_factor:   metallic,
                    roughness_factor:  roughness,
                    albedo_tex_index:  tex_index,
                    normal_tex_index:  normal_index,
                    orm_tex_index:     orm_index,
                },
            });

            prim_idx += 1;
        }
    }

    log::debug!(
        "GLTF '{}': {} primitives, {} textures ({} albedo, {} normal, {} orm, {} skipped)",
        path.display(),
        primitives.len(),
        embedded.len(),
        albedo_indices.len(),
        normal_indices.len(),
        orm_indices.len(),
        document.images().count().saturating_sub(needed_indices.len()),
    );

    (primitives, embedded)
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn load_image(
    source:  gltf::image::Source,
    base:    &Path,
    buffers: &[gltf::buffer::Data],
) -> (u32, u32, Vec<u8>) {
    let img = match source {
        gltf::image::Source::Uri { uri, .. } => {
            let img_path = base.join(uri);
            image::open(&img_path)
                .unwrap_or_else(|e| panic!("Failed to load image '{}': {}", img_path.display(), e))
        }
        gltf::image::Source::View { view, mime_type } => {
            let buf  = &buffers[view.buffer().index()];
            let data = &buf[view.offset()..view.offset() + view.length()];
            let fmt  = image::ImageFormat::from_mime_type(mime_type)
                .unwrap_or(image::ImageFormat::Png);
            image::load_from_memory_with_format(data, fmt)
                .unwrap_or_else(|e| panic!("Failed to decode embedded image: {}", e))
        }
    };

    let rgba = img.to_rgba8();
    (rgba.width(), rgba.height(), rgba.into_raw())
}
