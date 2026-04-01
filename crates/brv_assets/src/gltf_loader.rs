use crate::{MeshData, Vertex};
use brv_math::{Vec3, Mat4};

pub fn load_gltf(full_path: &str) -> MeshData {
    load_gltf_path(&format!("{}/scene.gltf", full_path))
}

pub fn load_glb(full_path: &str) -> MeshData {
    load_gltf_path(full_path)
}

fn load_gltf_path(path: &str) -> MeshData {
    let (doc, buffers, _) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF {}: {}", path, e));

    let mut vertices = Vec::new();
    let mut indices  = Vec::new();

    let scene = doc.default_scene()
        .or_else(|| doc.scenes().next())
        .unwrap_or_else(|| panic!("GLTF has no scene: {}", path));

    for node in scene.nodes() {
        collect_node(&node, Mat4::IDENTITY, &buffers, &mut vertices, &mut indices);
    }

    MeshData { vertices, indices }
}

fn collect_node(
    node:            &gltf::Node,
    parent_transform: Mat4,
    buffers:         &[gltf::buffer::Data],
    vertices:        &mut Vec<Vertex>,
    indices:         &mut Vec<u32>,
) {
    let local      = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global     = parent_transform * local;
    let normal_mat = global.inverse().transpose();

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

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
        collect_node(&child, global, buffers, vertices, indices);
    }
}
