use std::sync::Arc;

use ash::vk;

use brave_ecs::Component;

use crate::buffer::{upload_via_staging, Buffer, UploadBatch};
use crate::context::VulkanContext;
use crate::texture::GpuTexture;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub uv:       [f32; 2],
}

impl Vertex {
    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    pub fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12,
            },
            vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 24,
            },
        ]
    }
}

pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub local_bounds: ([f32; 3], [f32; 3]),
    device: Arc<ash::Device>,
}

impl Mesh {
    pub fn new(
        ctx: &VulkanContext,
        command_pool: vk::CommandPool,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Arc<Self> {
        let mut bmin = [f32::MAX; 3];
        let mut bmax = [f32::MIN; 3];
        for v in vertices {
            for i in 0..3 {
                bmin[i] = bmin[i].min(v.position[i]);
                bmax[i] = bmax[i].max(v.position[i]);
            }
        }

        let vertex_buffer = upload_via_staging(
            ctx,
            command_pool,
            ctx.graphics_queue,
            vertices,
            vk::BufferUsageFlags::VERTEX_BUFFER,
        );
        let index_buffer = upload_via_staging(
            ctx,
            command_pool,
            ctx.graphics_queue,
            indices,
            vk::BufferUsageFlags::INDEX_BUFFER,
        );

        Arc::new(Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            local_bounds: (bmin, bmax),
            device: Arc::new(unsafe { std::ptr::read(&ctx.device) }),
        })
    }

    /// Upload mesh into an `UploadBatch` — no GPU wait until `batch.flush()`.
    pub fn new_batched(
        ctx:     &VulkanContext,
        batch:   &mut UploadBatch,
        vertices: &[Vertex],
        indices:  &[u32],
    ) -> Arc<Self> {
        let mut bmin = [f32::MAX; 3];
        let mut bmax = [f32::MIN; 3];
        for v in vertices {
            for i in 0..3 {
                bmin[i] = bmin[i].min(v.position[i]);
                bmax[i] = bmax[i].max(v.position[i]);
            }
        }
        let vertex_buffer = batch.upload_buffer(ctx, vertices, vk::BufferUsageFlags::VERTEX_BUFFER);
        let index_buffer  = batch.upload_buffer(ctx, indices,  vk::BufferUsageFlags::INDEX_BUFFER);
        Arc::new(Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            local_bounds: (bmin, bmax),
            device: Arc::new(unsafe { std::ptr::read(&ctx.device) }),
        })
    }

    pub fn cube(ctx: &VulkanContext, command_pool: vk::CommandPool) -> Arc<Self> {
        let v = |p: [f32; 3], n: [f32; 3], uv: [f32; 2]| Vertex { position: p, normal: n, uv };
        #[rustfmt::skip]
        let vertices = vec![
            v([-0.5,-0.5, 0.5],[0.,0.,1.],[0.,1.]), v([0.5,-0.5, 0.5],[0.,0.,1.],[1.,1.]),
            v([ 0.5, 0.5, 0.5],[0.,0.,1.],[1.,0.]), v([-0.5, 0.5, 0.5],[0.,0.,1.],[0.,0.]),
            v([ 0.5,-0.5,-0.5],[0.,0.,-1.],[0.,1.]), v([-0.5,-0.5,-0.5],[0.,0.,-1.],[1.,1.]),
            v([-0.5, 0.5,-0.5],[0.,0.,-1.],[1.,0.]), v([ 0.5, 0.5,-0.5],[0.,0.,-1.],[0.,0.]),
            v([0.5,-0.5, 0.5],[1.,0.,0.],[0.,1.]), v([0.5,-0.5,-0.5],[1.,0.,0.],[1.,1.]),
            v([0.5, 0.5,-0.5],[1.,0.,0.],[1.,0.]), v([0.5, 0.5, 0.5],[1.,0.,0.],[0.,0.]),
            v([-0.5,-0.5,-0.5],[-1.,0.,0.],[0.,1.]), v([-0.5,-0.5, 0.5],[-1.,0.,0.],[1.,1.]),
            v([-0.5, 0.5, 0.5],[-1.,0.,0.],[1.,0.]), v([-0.5, 0.5,-0.5],[-1.,0.,0.],[0.,0.]),
            v([-0.5, 0.5, 0.5],[0.,1.,0.],[0.,0.]), v([ 0.5, 0.5, 0.5],[0.,1.,0.],[1.,0.]),
            v([ 0.5, 0.5,-0.5],[0.,1.,0.],[1.,1.]), v([-0.5, 0.5,-0.5],[0.,1.,0.],[0.,1.]),
            v([-0.5,-0.5,-0.5],[0.,-1.,0.],[0.,1.]), v([ 0.5,-0.5,-0.5],[0.,-1.,0.],[1.,1.]),
            v([ 0.5,-0.5, 0.5],[0.,-1.,0.],[1.,0.]), v([-0.5,-0.5, 0.5],[0.,-1.,0.],[0.,0.]),
        ];
        #[rustfmt::skip]
        let indices: Vec<u32> = (0u32..6).flat_map(|f| {
            let b = f * 4;
            [b, b+1, b+2, b+2, b+3, b]
        }).collect();
        Self::new(ctx, command_pool, &vertices, &indices)
    }

    pub fn plane(ctx: &VulkanContext, command_pool: vk::CommandPool, size: f32) -> Arc<Self> {
        let h = size * 0.5;
        let vertices = vec![
            Vertex { position: [-h, 0.0, -h], normal: [0., 1., 0.], uv: [0., 0.] },
            Vertex { position: [ h, 0.0, -h], normal: [0., 1., 0.], uv: [1., 0.] },
            Vertex { position: [ h, 0.0,  h], normal: [0., 1., 0.], uv: [1., 1.] },
            Vertex { position: [-h, 0.0,  h], normal: [0., 1., 0.], uv: [0., 1.] },
        ];
        let indices = vec![0u32, 1, 2, 2, 3, 0];
        Self::new(ctx, command_pool, &vertices, &indices)
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.vertex_buffer.handle, None);
            self.device.free_memory(self.vertex_buffer.memory, None);
            self.device.destroy_buffer(self.index_buffer.handle, None);
            self.device.free_memory(self.index_buffer.memory, None);
        }
    }
}

pub struct MeshRenderer {
    pub mesh:       Arc<Mesh>,
    pub texture:    Option<Arc<GpuTexture>>,
    pub base_color: [f32; 4],
}

impl MeshRenderer {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        Self { mesh, texture: None, base_color: [1.0, 1.0, 1.0, 1.0] }
    }

    pub fn with_texture(mesh: Arc<Mesh>, texture: Arc<GpuTexture>) -> Self {
        Self { mesh, texture: Some(texture), base_color: [1.0, 1.0, 1.0, 1.0] }
    }

    pub fn with_color(mesh: Arc<Mesh>, r: f32, g: f32, b: f32) -> Self {
        Self { mesh, texture: None, base_color: [r, g, b, 1.0] }
    }
}

impl Component for MeshRenderer {}
