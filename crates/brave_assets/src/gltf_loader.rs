use std::path::Path;

use brave_render::Vertex;

/// Загружает первый меш из .glb/.gltf файла.
/// Возвращает (вершины, индексы).
pub fn load(path: &Path) -> (Vec<Vertex>, Vec<u32>) {
    let (doc, buffers, _) = gltf::import(path)
        .unwrap_or_else(|e| panic!("Failed to load GLTF '{}': {}", path.display(), e));

    let mesh = doc
        .meshes()
        .next()
        .unwrap_or_else(|| panic!("No meshes in GLTF '{}'", path.display()));

    let primitive = mesh
        .primitives()
        .next()
        .unwrap_or_else(|| panic!("No primitives in GLTF '{}'", path.display()));

    let reader = primitive.reader(|buf| Some(&buffers[buf.index()]));

    // Позиции (обязательны)
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .unwrap_or_else(|| panic!("GLTF '{}': no positions", path.display()))
        .collect();

    // Нормали (или дефолтные вверх)
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

    // UV (или нули)
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|tc| tc.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let vertices: Vec<Vertex> = positions
        .iter()
        .zip(normals.iter())
        .zip(uvs.iter())
        .map(|((pos, norm), uv)| Vertex {
            position: *pos,
            normal: *norm,
            uv: *uv,
        })
        .collect();

    // Индексы
    let indices: Vec<u32> = reader
        .read_indices()
        .map(|ri| ri.into_u32().collect())
        .unwrap_or_else(|| (0..positions.len() as u32).collect());

    log::debug!("Loaded GLTF '{}': {} verts, {} indices", path.display(), vertices.len(), indices.len());

    (vertices, indices)
}
