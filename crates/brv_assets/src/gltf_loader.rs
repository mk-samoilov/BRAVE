use crate::{MeshData, Vertex};

pub fn load_gltf(full_path: &str) -> MeshData {
    let scene_path = format!("{}/scene.gltf", full_path);
    load_gltf_path(&scene_path)
}

pub fn load_glb(full_path: &str) -> MeshData {
    load_gltf_path(full_path)
}

fn load_gltf_path(path: &str) -> MeshData {
    let (doc, buffers, _images) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF {}: {}", path, e));

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .unwrap_or_else(|| panic!("GLTF mesh has no positions: {}", path))
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
                vertices.push(Vertex {
                    position: positions[i],
                    normal:   normals[i],
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

    MeshData { vertices, indices }
}
