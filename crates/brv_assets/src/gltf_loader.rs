use std::sync::Arc;
use crate::{MeshData, Vertex, Material, TextureData};
use brv_colors::Color;
use brv_math::{Vec3, Mat4};

pub fn load_gltf(full_path: &str) -> (MeshData, Material) {
    load_gltf_path(&format!("{}/scene.gltf", full_path))
}

pub fn load_glb(full_path: &str) -> (MeshData, Material) {
    load_gltf_path(full_path)
}

fn load_gltf_path(path: &str) -> (MeshData, Material) {
    let (doc, buffers, images) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF {}: {}", path, e));

    let mut vertices = Vec::new();
    let mut indices  = Vec::new();
    let mut material = Material::default();

    let scene = doc.default_scene()
        .or_else(|| doc.scenes().next())
        .unwrap_or_else(|| panic!("GLTF has no scene: {}", path));

    for node in scene.nodes() {
        collect_node(&node, Mat4::IDENTITY, &buffers, &images, &mut vertices, &mut indices, &mut material);
    }

    (MeshData { vertices, indices }, material)
}

fn to_rgba8(data: &gltf::image::Data) -> TextureData {
    use gltf::image::Format;
    let pixels = match data.format {
        Format::R8G8B8A8 => data.pixels.clone(),
        Format::R8G8B8   => data.pixels.chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255u8])
            .collect(),
        _ => vec![255u8; (data.width * data.height * 4) as usize],
    };
    TextureData { pixels, width: data.width, height: data.height }
}

fn collect_node(
    node:             &gltf::Node,
    parent_transform: Mat4,
    buffers:          &[gltf::buffer::Data],
    images:           &[gltf::image::Data],
    vertices:         &mut Vec<Vertex>,
    indices:          &mut Vec<u32>,
    material:         &mut Material,
) {
    let local      = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global     = parent_transform * local;
    let normal_mat = global.inverse().transpose();

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let pbr = prim.material().pbr_metallic_roughness();
            let [r, g, b, a] = pbr.base_color_factor();

            let albedo_tex = pbr.base_color_texture()
                .map(|t| Arc::new(to_rgba8(&images[t.texture().source().index()])));
            let mr_tex = pbr.metallic_roughness_texture()
                .map(|t| Arc::new(to_rgba8(&images[t.texture().source().index()])));
            let normal_tex = prim.material().normal_texture()
                .map(|t| Arc::new(to_rgba8(&images[t.texture().source().index()])));

            *material = Material {
                albedo:                     Color { r, g, b, a },
                metallic:                   pbr.metallic_factor(),
                roughness:                  pbr.roughness_factor(),
                emissive:                   { let [er, eg, eb] = prim.material().emissive_factor(); Color { r: er, g: eg, b: eb, a: 1.0 } },
                albedo_texture:             albedo_tex,
                metallic_roughness_texture: mr_tex,
                normal_texture:             normal_tex,
            };

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .unwrap_or_else(|| panic!("GLTF mesh has no positions"))
                .collect();

            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let base = vertices.len() as u32;

            for i in 0..positions.len() {
                let p = global.transform_point3(Vec3::from(positions[i]));
                let n = normal_mat.transform_vector3(Vec3::from(normals[i])).normalize();
                vertices.push(Vertex {
                    position: p.into(),
                    normal:   n.into(),
                    uv:       uvs[i],
                });
            }

            if let Some(iter) = reader.read_indices() {
                for idx in iter.into_u32() {
                    indices.push(base + idx);
                }
            } else {
                for i in 0..positions.len() as u32 {
                    indices.push(base + i);
                }
            }
        }
    }

    for child in node.children() {
        collect_node(&child, global, buffers, images, vertices, indices, material);
    }
}
