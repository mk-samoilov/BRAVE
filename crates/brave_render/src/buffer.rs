use ash::vk;

use crate::context::VulkanContext;

pub struct Buffer {
    pub handle: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
}

impl Buffer {
    pub fn new(
        ctx: &VulkanContext,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        mem_props: vk::MemoryPropertyFlags,
    ) -> Self {
        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let handle = unsafe { ctx.device.create_buffer(&create_info, None).unwrap() };
        let mem_reqs = unsafe { ctx.device.get_buffer_memory_requirements(handle) };
        let mem_type = ctx.memory_type_index(mem_reqs.memory_type_bits, mem_props);

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);

        let memory = unsafe { ctx.device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { ctx.device.bind_buffer_memory(handle, memory, 0).unwrap() };

        Self { handle, memory, size }
    }

    pub fn upload<T: Copy>(&self, ctx: &VulkanContext, data: &[T]) {
        let size = std::mem::size_of_val(data) as vk::DeviceSize;
        unsafe {
            let ptr = ctx
                .device
                .map_memory(self.memory, 0, size, vk::MemoryMapFlags::empty())
                .unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut T, data.len());
            ctx.device.unmap_memory(self.memory);
        }
    }

    pub fn destroy(&self, ctx: &VulkanContext) {
        unsafe {
            ctx.device.destroy_buffer(self.handle, None);
            ctx.device.free_memory(self.memory, None);
        }
    }
}

pub fn upload_via_staging<T: Copy>(
    ctx: &VulkanContext,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    data: &[T],
    usage: vk::BufferUsageFlags,
) -> Buffer {
    let size = std::mem::size_of_val(data) as vk::DeviceSize;

    let staging = Buffer::new(
        ctx,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    );
    staging.upload(ctx, data);

    let dst = Buffer::new(
        ctx,
        size,
        usage | vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );

    let cmd = begin_single_time_commands(ctx, command_pool);
    let region = vk::BufferCopy { src_offset: 0, dst_offset: 0, size };
    unsafe { ctx.device.cmd_copy_buffer(cmd, staging.handle, dst.handle, &[region]) };
    end_single_time_commands(ctx, command_pool, queue, cmd);

    staging.destroy(ctx);
    dst
}

// ─── Batch upload ─────────────────────────────────────────────────────────────

/// Collects all GPU transfers (meshes + textures) into a single command buffer.
/// Call `flush` once to submit everything with a single `queue_wait_idle`.
/// This eliminates the N×`queue_wait_idle` penalty when loading a model with
/// multiple primitives and textures.
pub struct UploadBatch {
    pub cmd:  vk::CommandBuffer,
    pool:     vk::CommandPool,
    staging:  Vec<Buffer>,
}

impl UploadBatch {
    pub fn new(ctx: &VulkanContext, pool: vk::CommandPool) -> Self {
        let cmd = begin_single_time_commands(ctx, pool);
        Self { cmd, pool, staging: Vec::new() }
    }

    /// Record a DEVICE_LOCAL buffer upload. Staging buffer kept alive until `flush`.
    pub fn upload_buffer<T: Copy>(
        &mut self,
        ctx:   &VulkanContext,
        data:  &[T],
        usage: vk::BufferUsageFlags,
    ) -> Buffer {
        let size = std::mem::size_of_val(data) as vk::DeviceSize;

        let staging = Buffer::new(
            ctx, size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        staging.upload(ctx, data);

        let dst = Buffer::new(
            ctx, size,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        let region = vk::BufferCopy { src_offset: 0, dst_offset: 0, size };
        unsafe { ctx.device.cmd_copy_buffer(self.cmd, staging.handle, dst.handle, &[region]); }
        self.staging.push(staging);
        dst
    }

    /// Record an RGBA8 image upload + layout transitions + mipmap generation.
    /// Staging kept alive until `flush`.
    pub fn upload_image(
        &mut self,
        ctx:    &VulkanContext,
        width:  u32,
        height: u32,
        pixels: &[u8],
    ) -> (vk::Image, vk::DeviceMemory, u32) {
        let size = pixels.len() as vk::DeviceSize;

        let staging = Buffer::new(
            ctx, size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        staging.upload(ctx, pixels);

        let mip_levels = (width.max(height) as f32).log2().floor() as u32 + 1;

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe { ctx.device.create_image(&image_info, None).unwrap() };
        let mem_req  = unsafe { ctx.device.get_image_memory_requirements(image) };
        let mem_type = ctx.memory_type_index(mem_req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL);
        let alloc    = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_req.size)
            .memory_type_index(mem_type);
        let memory = unsafe { ctx.device.allocate_memory(&alloc, None).unwrap() };
        unsafe { ctx.device.bind_image_memory(image, memory, 0).unwrap() };

        // Transition level 0: UNDEFINED → TRANSFER_DST, then upload base level
        record_image_barrier_mip(&ctx.device, self.cmd, image,
            vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, 0, 1);

        let region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask:      vk::ImageAspectFlags::COLOR,
                mip_level:        0,
                base_array_layer: 0,
                layer_count:      1,
            })
            .image_extent(vk::Extent3D { width, height, depth: 1 });
        unsafe {
            ctx.device.cmd_copy_buffer_to_image(
                self.cmd, staging.handle, image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region],
            );
        }

        // Generate mipmaps via blitting
        // Transition level 0: TRANSFER_DST → TRANSFER_SRC
        record_image_barrier_mip(&ctx.device, self.cmd, image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, 0, 1);

        let mut mip_w = width;
        let mut mip_h = height;

        for i in 1..mip_levels {
            let dst_w = (mip_w / 2).max(1);
            let dst_h = (mip_h / 2).max(1);

            // Transition level i: UNDEFINED → TRANSFER_DST
            record_image_barrier_mip(&ctx.device, self.cmd, image,
                vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, i, 1);

            let blit = vk::ImageBlit {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i - 1, base_array_layer: 0, layer_count: 1,
                },
                src_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: mip_w as i32, y: mip_h as i32, z: 1 },
                ],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i, base_array_layer: 0, layer_count: 1,
                },
                dst_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D { x: dst_w as i32, y: dst_h as i32, z: 1 },
                ],
            };
            unsafe {
                ctx.device.cmd_blit_image(
                    self.cmd,
                    image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    image, vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit], vk::Filter::LINEAR,
                );
            }

            // Transition level i-1: TRANSFER_SRC → SHADER_READ_ONLY
            record_image_barrier_mip(&ctx.device, self.cmd, image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, i - 1, 1);

            // Transition level i: TRANSFER_DST → TRANSFER_SRC (for next iteration)
            if i < mip_levels - 1 {
                record_image_barrier_mip(&ctx.device, self.cmd, image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, i, 1);
            }

            mip_w = dst_w;
            mip_h = dst_h;
        }

        // Transition last mip level to SHADER_READ_ONLY
        if mip_levels > 1 {
            record_image_barrier_mip(&ctx.device, self.cmd, image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                mip_levels - 1, 1);
        } else {
            // Only 1 mip level — level 0 is in TRANSFER_SRC after the initial transition
            record_image_barrier_mip(&ctx.device, self.cmd, image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, 0, 1);
        }

        self.staging.push(staging);
        (image, memory, mip_levels)
    }

    /// Submit all recorded commands (ONE `queue_wait_idle`), then free staging buffers.
    pub fn flush(self, ctx: &VulkanContext, queue: vk::Queue) {
        end_single_time_commands(ctx, self.pool, queue, self.cmd);
        for s in self.staging { s.destroy(ctx); }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

pub fn begin_single_time_commands(ctx: &VulkanContext, pool: vk::CommandPool) -> vk::CommandBuffer {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_pool(pool)
        .command_buffer_count(1);

    let cmd = unsafe { ctx.device.allocate_command_buffers(&alloc_info).unwrap()[0] };
    let begin_info = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { ctx.device.begin_command_buffer(cmd, &begin_info).unwrap() };
    cmd
}

pub fn end_single_time_commands(
    ctx: &VulkanContext,
    pool: vk::CommandPool,
    queue: vk::Queue,
    cmd: vk::CommandBuffer,
) {
    unsafe {
        ctx.device.end_command_buffer(cmd).unwrap();
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        ctx.device.queue_submit(queue, &[submit_info], vk::Fence::null()).unwrap();
        ctx.device.queue_wait_idle(queue).unwrap();
        ctx.device.free_command_buffers(pool, &[cmd]);
    }
}

/// Record an image layout transition barrier into `cmd` (covers mip level 0, layer_count=1).
pub fn record_image_barrier(
    device:     &ash::Device,
    cmd:        vk::CommandBuffer,
    image:      vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    record_image_barrier_mip(device, cmd, image, old_layout, new_layout, 0, 1);
}

/// Record an image layout transition barrier for a specific mip range into `cmd`.
pub fn record_image_barrier_mip(
    device:      &ash::Device,
    cmd:         vk::CommandBuffer,
    image:       vk::Image,
    old_layout:  vk::ImageLayout,
    new_layout:  vk::ImageLayout,
    base_mip:    u32,
    level_count: u32,
) {
    let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
            vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::TRANSFER_READ,
            vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        _ => panic!("record_image_barrier_mip: unsupported {:?} → {:?}", old_layout, new_layout),
    };
    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout).new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: base_mip, level_count,
            base_array_layer: 0, layer_count: 1,
        })
        .src_access_mask(src_access).dst_access_mask(dst_access);
    unsafe {
        device.cmd_pipeline_barrier(cmd, src_stage, dst_stage,
            vk::DependencyFlags::empty(), &[], &[], &[barrier]);
    }
}
