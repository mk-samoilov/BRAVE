use std::sync::Arc;
use std::f32::consts::PI;

use ash::vk;
use brave_render::{Mesh, Vertex, VulkanContext};

// ─── Cube ─────────────────────────────────────────────────────────────────────

/// Generate a unit cube (side length 1, centered at origin).
///
/// `subdivisions` controls how many quads each face is divided into:
/// - `1` → 12 triangles (same as the original simple cube)
/// - `4` → 192 triangles
/// - `16` → 3072 triangles
pub fn cube(
    ctx:          &VulkanContext,
    command_pool: vk::CommandPool,
    subdivisions: u32,
) -> Arc<Mesh> {
    let divs = subdivisions.max(1) as usize;

    // Each face defined by (normal, right, up) - all unit vectors.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([ 0.,  0.,  1.], [ 1.,  0.,  0.], [ 0.,  1.,  0.]), // +Z
        ([ 0.,  0., -1.], [-1.,  0.,  0.], [ 0.,  1.,  0.]), // -Z
        ([ 1.,  0.,  0.], [ 0.,  0., -1.], [ 0.,  1.,  0.]), // +X
        ([-1.,  0.,  0.], [ 0.,  0.,  1.], [ 0.,  1.,  0.]), // -X
        ([ 0.,  1.,  0.], [ 1.,  0.,  0.], [ 0.,  0., -1.]), // +Y
        ([ 0., -1.,  0.], [ 1.,  0.,  0.], [ 0.,  0.,  1.]), // -Y
    ];

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices:  Vec<u32>    = Vec::new();

    for (normal, right, up) in &faces {
        let base   = vertices.len() as u32;
        let stride = (divs + 1) as u32;

        for row in 0..=divs {
            for col in 0..=divs {
                let u = col as f32 / divs as f32; // 0..1
                let v = row as f32 / divs as f32; // 0..1

                let s = u - 0.5; // -0.5..0.5
                let t = v - 0.5;

                let px = normal[0] * 0.5 + right[0] * s + up[0] * t;
                let py = normal[1] * 0.5 + right[1] * s + up[1] * t;
                let pz = normal[2] * 0.5 + right[2] * s + up[2] * t;

                vertices.push(Vertex {
                    position: [px, py, pz],
                    normal:   *normal,
                    uv:       [u, 1.0 - v],
                });
            }
        }

        for row in 0..divs as u32 {
            for col in 0..divs as u32 {
                let a = base + row * stride + col;
                let b = base + row * stride + col + 1;
                let c = base + (row + 1) * stride + col;
                let d = base + (row + 1) * stride + col + 1;
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
    }

    Mesh::new(ctx, command_pool, &vertices, &indices)
}

// ─── Sphere ───────────────────────────────────────────────────────────────────

/// Generate a UV sphere (radius 0.5, centered at origin).
///
/// `segments` controls both latitude rings and longitude columns:
/// - `8`  → ~128 triangles
/// - `16` → ~512 triangles
/// - `32` → ~2048 triangles
pub fn sphere(
    ctx:          &VulkanContext,
    command_pool: vk::CommandPool,
    segments:     u32,
) -> Arc<Mesh> {
    let lat = segments.max(2) as usize; // latitude  rings   (top → bottom)
    let lon = segments.max(3) as usize; // longitude columns (around)

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices:  Vec<u32>    = Vec::new();

    for i in 0..=lat {
        let theta     = PI * i as f32 / lat as f32; // 0 (top) → PI (bottom)
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for j in 0..=lon {
            let phi     = 2.0 * PI * j as f32 / lon as f32;
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            vertices.push(Vertex {
                position: [x * 0.5, y * 0.5, z * 0.5],
                normal:   [x, y, z],
                uv:       [j as f32 / lon as f32, i as f32 / lat as f32],
            });
        }
    }

    let stride = (lon + 1) as u32;
    for i in 0..lat as u32 {
        for j in 0..lon as u32 {
            let a = i * stride + j;
            let b = i * stride + j + 1;
            let c = (i + 1) * stride + j;
            let d = (i + 1) * stride + j + 1;

            if i != 0 {
                indices.extend_from_slice(&[a, b, c]);
            }
            if i != lat as u32 - 1 {
                indices.extend_from_slice(&[b, d, c]);
            }
        }
    }

    Mesh::new(ctx, command_pool, &vertices, &indices)
}
