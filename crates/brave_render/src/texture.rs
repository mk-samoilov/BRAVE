use std::sync::Arc;

use ash::vk;

use crate::buffer::UploadBatch;
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
    /// Upload RGBA8 pixels to a DEVICE_LOCAL VkImage with mipmaps.
    /// Creates its own UploadBatch and flushes immediately.
    /// Use for small/single textures (e.g. white fallback). For models use `from_rgba8_batched`.
    pub fn from_rgba8(
        ctx:             &VulkanContext,
        command_pool:    vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
        width:           u32,
        height:          u32,
        pixels:          &[u8],
    ) -> Arc<Self> {
        let mut batch = UploadBatch::new(ctx, command_pool);
        let tex = Self::from_rgba8_batched(ctx, &mut batch, descriptor_pool, set_layout, width, height, pixels);
        batch.flush(ctx, ctx.graphics_queue);
        tex
    }

    /// Record an RGBA8 upload into `batch`. Returns a fully-initialized GpuTexture
    /// (view/sampler/descriptor are created immediately; image data is uploaded on `batch.flush()`).
    pub fn from_rgba8_batched(
        ctx:             &VulkanContext,
        batch:           &mut UploadBatch,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
        width:           u32,
        height:          u32,
        pixels:          &[u8],
    ) -> Arc<Self> {
        let format = vk::Format::R8G8B8A8_SRGB;
        let (image, memory, mip_levels) = batch.upload_image(ctx, width, height, pixels, format);
        let view           = create_image_view(&ctx.device, image, mip_levels, format);
        let sampler        = create_sampler(&ctx.device);
        let descriptor_set = alloc_descriptor_set(&ctx.device, descriptor_pool, set_layout, view, sampler);
        Arc::new(Self {
            image, memory, view, sampler, descriptor_set,
            device:          &ctx.device as *const ash::Device,
            descriptor_pool,
        })
    }

    /// Upload a linear (non-sRGB) RGBA8 texture - for normal maps.
    pub fn from_rgba8_unorm_batched(
        ctx:             &VulkanContext,
        batch:           &mut UploadBatch,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
        width:           u32,
        height:          u32,
        pixels:          &[u8],
    ) -> Arc<Self> {
        let format = vk::Format::R8G8B8A8_UNORM;
        let (image, memory, mip_levels) = batch.upload_image(ctx, width, height, pixels, format);
        let view           = create_image_view(&ctx.device, image, mip_levels, format);
        let sampler        = create_sampler(&ctx.device);
        let descriptor_set = alloc_descriptor_set(&ctx.device, descriptor_pool, set_layout, view, sampler);
        Arc::new(Self {
            image, memory, view, sampler, descriptor_set,
            device:          &ctx.device as *const ash::Device,
            descriptor_pool,
        })
    }

    /// 1×1 white texture - used as fallback when no albedo is assigned.
    pub fn white(
        ctx:             &VulkanContext,
        command_pool:    vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
    ) -> Arc<Self> {
        let pixels = [255u8, 255, 255, 255];
        Self::from_rgba8(ctx, command_pool, descriptor_pool, set_layout, 1, 1, &pixels)
    }

    /// 1×1 flat normal map (128, 128, 255) - used as fallback when no normal map is assigned.
    pub fn flat_normal(
        ctx:             &VulkanContext,
        command_pool:    vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        set_layout:      vk::DescriptorSetLayout,
    ) -> Arc<Self> {
        let pixels = [128u8, 128, 255, 255];
        let mut batch = UploadBatch::new(ctx, command_pool);
        let tex = Self::from_rgba8_unorm_batched(ctx, &mut batch, descriptor_pool, set_layout, 1, 1, &pixels);
        batch.flush(ctx, ctx.graphics_queue);
        tex
    }
}

impl Drop for GpuTexture {
    fn drop(&mut self) {
        let device = unsafe { &*self.device };
        unsafe {
            let _ = device.free_descriptor_sets(self.descriptor_pool, &[self.descriptor_set]);
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.view, None);
            device.destroy_image(self.image, None);
            device.free_memory(self.memory, None);
        }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn create_image_view(device: &ash::Device, image: vk::Image, mip_levels: u32, format: vk::Format) -> vk::ImageView {
    let info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask:      vk::ImageAspectFlags::COLOR,
            base_mip_level:   0,
            level_count:      mip_levels,
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
