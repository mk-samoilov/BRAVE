use std::sync::Arc;

use ash::vk;

use crate::buffer::{begin_single_time_commands, end_single_time_commands, Buffer};
use crate::context::VulkanContext;

pub struct GpuTexture {
    pub image:          vk::Image,
    pub memory:         vk::DeviceMemory,
    pub view:           vk::ImageView,
    pub sampler:        vk::Sampler,
    pub descriptor_set: vk::DescriptorSet,
    device:             *const ash::Device,
    descriptor_pool:    vk::DescriptorPool,
}

unsafe impl Send for GpuTexture {}
unsafe impl Sync for GpuTexture {}

impl GpuTexture {
    /// Upload RGBA8 pixels to a DEVICE_LOCAL VkImage and allocate a descriptor set.
    pub fn from_rgba8(
        ctx:             &VulkanContext,
        command_pool:    vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
        width:           u32,
        height:          u32,
        pixels:          &[u8],
    ) -> Arc<Self> {
        assert_eq!(pixels.len(), (width * height * 4) as usize, "Expected RGBA8 pixels");

        let size = pixels.len() as vk::DeviceSize;
        let staging = Buffer::new(
            ctx,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        staging.upload(ctx, pixels);

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
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

        let cmd = begin_single_time_commands(ctx, command_pool);
        transition_layout(&ctx.device, cmd, image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL);

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
                cmd, staging.handle, image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region],
            );
        }

        transition_layout(&ctx.device, cmd, image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        end_single_time_commands(ctx, command_pool, ctx.graphics_queue, cmd);
        staging.destroy(ctx);

        let view    = create_image_view(&ctx.device, image);
        let sampler = create_sampler(&ctx.device);
        let descriptor_set = alloc_descriptor_set(
            &ctx.device, descriptor_pool, set_layout, view, sampler,
        );

        Arc::new(Self {
            image,
            memory,
            view,
            sampler,
            descriptor_set,
            device:          &ctx.device as *const ash::Device,
            descriptor_pool,
        })
    }

    /// 1×1 white texture — used as fallback when no texture is assigned.
    pub fn white(
        ctx:             &VulkanContext,
        command_pool:    vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
    ) -> Arc<Self> {
        let pixels = [255u8, 255, 255, 255];
        Self::from_rgba8(ctx, command_pool, descriptor_pool, set_layout, 1, 1, &pixels)
    }
}

impl Drop for GpuTexture {
    fn drop(&mut self) {
        let device = unsafe { &*self.device };
        unsafe {
            // free_descriptor_sets is fallible; ignore result on shutdown
            let _ = device.free_descriptor_sets(self.descriptor_pool, &[self.descriptor_set]);
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.view, None);
            device.destroy_image(self.image, None);
            device.free_memory(self.memory, None);
        }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn transition_layout(
    device:     &ash::Device,
    cmd:        vk::CommandBuffer,
    image:      vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        _ => panic!("GpuTexture: unsupported layout transition"),
    };

    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask:      vk::ImageAspectFlags::COLOR,
            base_mip_level:   0,
            level_count:      1,
            base_array_layer: 0,
            layer_count:      1,
        })
        .src_access_mask(src_access)
        .dst_access_mask(dst_access);

    unsafe {
        device.cmd_pipeline_barrier(
            cmd, src_stage, dst_stage,
            vk::DependencyFlags::empty(),
            &[], &[], &[barrier],
        );
    }
}

fn create_image_view(device: &ash::Device, image: vk::Image) -> vk::ImageView {
    let info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_SRGB)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask:      vk::ImageAspectFlags::COLOR,
            base_mip_level:   0,
            level_count:      1,
            base_array_layer: 0,
            layer_count:      1,
        });
    unsafe { device.create_image_view(&info, None).unwrap() }
}

fn create_sampler(device: &ash::Device) -> vk::Sampler {
    let info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .max_lod(vk::LOD_CLAMP_NONE);
    unsafe { device.create_sampler(&info, None).unwrap() }
}

fn alloc_descriptor_set(
    device:          &ash::Device,
    descriptor_pool: vk::DescriptorPool,
    set_layout:      vk::DescriptorSetLayout,
    view:            vk::ImageView,
    sampler:         vk::Sampler,
) -> vk::DescriptorSet {
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(std::slice::from_ref(&set_layout));
    let set = unsafe { device.allocate_descriptor_sets(&alloc_info).unwrap()[0] };

    let image_info = vk::DescriptorImageInfo {
        sampler,
        image_view:   view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    let write = vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info));
    unsafe { device.update_descriptor_sets(&[write], &[]) };

    set
}
