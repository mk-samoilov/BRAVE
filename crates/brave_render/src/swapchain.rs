use ash::{khr, vk};

use crate::context::VulkanContext;
use crate::pipeline::MSAA_SAMPLES;

pub struct Swapchain {
    pub loader: khr::swapchain::Device,
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub depth_image: vk::Image,
    pub depth_image_memory: vk::DeviceMemory,
    pub depth_image_view: vk::ImageView,
    pub depth_format: vk::Format,
    pub msaa_color_image: vk::Image,
    pub msaa_color_memory: vk::DeviceMemory,
    pub msaa_color_view: vk::ImageView,
}

impl Swapchain {
    pub fn new(ctx: &VulkanContext, width: u32, height: u32) -> Self {
        let loader = khr::swapchain::Device::new(&ctx.instance, &ctx.device);
        let (handle, format, extent, images) = Self::create_swapchain(ctx, &loader, width, height, vk::SwapchainKHR::null());
        let image_views = Self::create_image_views(&ctx.device, &images, format);

        let depth_format = Self::find_depth_format(ctx);
        let (depth_image, depth_image_memory, depth_image_view) =
            Self::create_depth_resources(ctx, extent, depth_format);
        let (msaa_color_image, msaa_color_memory, msaa_color_view) =
            Self::create_msaa_color_resources(ctx, extent, format);

        Self {
            loader,
            handle,
            images,
            image_views,
            format,
            extent,
            depth_image,
            depth_image_memory,
            depth_image_view,
            depth_format,
            msaa_color_image,
            msaa_color_memory,
            msaa_color_view,
        }
    }

    pub fn recreate(&mut self, ctx: &VulkanContext, width: u32, height: u32) {
        unsafe {
            ctx.device.device_wait_idle().unwrap();
            self.destroy_resources(&ctx.device);
        }

        let (handle, format, extent, images) =
            Self::create_swapchain(ctx, &self.loader, width, height, self.handle);

        unsafe { self.loader.destroy_swapchain(self.handle, None) };

        self.handle = handle;
        self.format = format;
        self.extent = extent;
        self.images = images;
        self.image_views = Self::create_image_views(&ctx.device, &self.images, format);

        let (depth_image, depth_image_memory, depth_image_view) =
            Self::create_depth_resources(ctx, extent, self.depth_format);
        self.depth_image = depth_image;
        self.depth_image_memory = depth_image_memory;
        self.depth_image_view = depth_image_view;

        let (msaa_color_image, msaa_color_memory, msaa_color_view) =
            Self::create_msaa_color_resources(ctx, extent, self.format);
        self.msaa_color_image = msaa_color_image;
        self.msaa_color_memory = msaa_color_memory;
        self.msaa_color_view = msaa_color_view;
    }

    fn create_swapchain(
        ctx: &VulkanContext,
        loader: &khr::swapchain::Device,
        width: u32,
        height: u32,
        old_swapchain: vk::SwapchainKHR,
    ) -> (vk::SwapchainKHR, vk::Format, vk::Extent2D, Vec<vk::Image>) {
        let capabilities = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_capabilities(ctx.physical_device, ctx.surface)
                .unwrap()
        };
        let formats = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_formats(ctx.physical_device, ctx.surface)
                .unwrap()
        };
        let present_modes = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_present_modes(ctx.physical_device, ctx.surface)
                .unwrap()
        };

        let format = Self::choose_format(&formats);
        let present_mode = Self::choose_present_mode(&present_modes);
        let extent = Self::choose_extent(&capabilities, width, height);

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
            image_count = capabilities.max_image_count;
        }

        let families = [
            ctx.queue_families.graphics,
            ctx.queue_families.present,
        ];
        let (sharing_mode, queue_family_indices) =
            if ctx.queue_families.graphics != ctx.queue_families.present {
                (vk::SharingMode::CONCURRENT, families.as_slice())
            } else {
                (vk::SharingMode::EXCLUSIVE, &[][..])
            };

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(ctx.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(sharing_mode)
            .queue_family_indices(queue_family_indices)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain);

        let handle =
            unsafe { loader.create_swapchain(&create_info, None).expect("Failed to create swapchain") };
        let images = unsafe { loader.get_swapchain_images(handle).unwrap() };

        (handle, format.format, extent, images)
    }

    fn choose_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        *formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&formats[0])
    }

    fn choose_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
        if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        }
    }

    fn choose_extent(capabilities: &vk::SurfaceCapabilitiesKHR, width: u32, height: u32) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        }
    }

    fn create_image_views(
        device: &ash::Device,
        images: &[vk::Image],
        format: vk::Format,
    ) -> Vec<vk::ImageView> {
        images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe { device.create_image_view(&create_info, None).unwrap() }
            })
            .collect()
    }

    fn find_depth_format(ctx: &VulkanContext) -> vk::Format {
        let candidates = [
            vk::Format::D32_SFLOAT,
            vk::Format::D32_SFLOAT_S8_UINT,
            vk::Format::D24_UNORM_S8_UINT,
        ];
        for &format in &candidates {
            let props = unsafe {
                ctx.instance
                    .get_physical_device_format_properties(ctx.physical_device, format)
            };
            if props
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
            {
                return format;
            }
        }
        panic!("Failed to find supported depth format");
    }

    fn create_msaa_color_resources(
        ctx: &VulkanContext,
        extent: vk::Extent2D,
        format: vk::Format,
    ) -> (vk::Image, vk::DeviceMemory, vk::ImageView) {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .samples(MSAA_SAMPLES)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let image = unsafe { ctx.device.create_image(&image_info, None).unwrap() };
        let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(image) };
        let mem_type = ctx.memory_type_index(mem_reqs.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL);
        let memory = unsafe {
            ctx.device.allocate_memory(
                &vk::MemoryAllocateInfo::default().allocation_size(mem_reqs.size).memory_type_index(mem_type),
                None,
            ).unwrap()
        };
        unsafe { ctx.device.bind_image_memory(image, memory, 0).unwrap() };

        let view = unsafe {
            ctx.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0, level_count: 1,
                        base_array_layer: 0, layer_count: 1,
                    }),
                None,
            ).unwrap()
        };
        (image, memory, view)
    }

    fn create_depth_resources(
        ctx: &VulkanContext,
        extent: vk::Extent2D,
        format: vk::Format,
    ) -> (vk::Image, vk::DeviceMemory, vk::ImageView) {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .samples(MSAA_SAMPLES)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let image = unsafe { ctx.device.create_image(&image_info, None).unwrap() };
        let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(image) };
        let mem_type = ctx.memory_type_index(
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);
        let memory = unsafe { ctx.device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { ctx.device.bind_image_memory(image, memory, 0).unwrap() };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe { ctx.device.create_image_view(&view_info, None).unwrap() };

        (image, memory, view)
    }

    pub unsafe fn destroy_resources(&self, device: &ash::Device) {
        unsafe {
            device.destroy_image_view(self.msaa_color_view, None);
            device.destroy_image(self.msaa_color_image, None);
            device.free_memory(self.msaa_color_memory, None);
            device.destroy_image_view(self.depth_image_view, None);
            device.destroy_image(self.depth_image, None);
            device.free_memory(self.depth_image_memory, None);
            for &view in &self.image_views {
                device.destroy_image_view(view, None);
            }
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {}
}
