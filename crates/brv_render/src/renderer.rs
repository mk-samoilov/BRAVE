use std::collections::HashMap;
use ash::{vk, Device, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use brv_window::Window;
use glam::{Mat4, Vec3};

const MAX_FRAMES: usize = 2;

struct GpuMesh {
    vertex_buffer: vk::Buffer,
    vertex_memory: vk::DeviceMemory,
    index_buffer: vk::Buffer,
    index_memory: vk::DeviceMemory,
    index_count: u32,
}

struct DrawCall {
    mesh_key: usize,
    model: [[f32; 4]; 4],
    albedo: [f32; 4],
    metallic_roughness: [f32; 4],
}

#[repr(C)]
struct PushData {
    model: [[f32; 4]; 4],
    albedo: [f32; 4],
    mr: [f32; 4],
}

#[repr(C)]
struct SceneUBO {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
}

pub struct Renderer {
    _entry: ash::Entry,
    instance: Instance,
    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    graphics_family: u32,
    present_family: u32,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_image_views: Vec<vk::ImageView>,
    depth_image: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
    render_pass: vk::RenderPass,
    descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available: Vec<vk::Semaphore>,
    in_flight: Vec<vk::Fence>,
    render_finished: Vec<vk::Semaphore>,
    images_in_flight: Vec<vk::Fence>,
    current_frame: usize,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
    uniform_buffers: Vec<vk::Buffer>,
    uniform_memories: Vec<vk::DeviceMemory>,
    uniform_mapped: Vec<*mut u8>,
    mesh_cache: HashMap<usize, GpuMesh>,
    #[cfg(debug_assertions)]
    debug_utils: ash::ext::debug_utils::Instance,
    #[cfg(debug_assertions)]
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Renderer {
    pub fn new(window: &Window, assets: &mut brv_assets::Assets) -> Self {
        let entry = unsafe { ash::Entry::load().expect("Failed to load Vulkan") };
        let instance = Self::create_instance(&entry, window);

        #[cfg(debug_assertions)]
        let (debug_utils, debug_messenger) = Self::setup_debug_messenger(&entry, &instance);

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .expect("Failed to create Vulkan surface")
        };

        let (physical_device, graphics_family, present_family) =
            Self::pick_physical_device(&instance, &surface_loader, surface);

        let device = Self::create_device(&instance, physical_device, graphics_family, present_family);
        let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family, 0) };

        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let (swapchain, swapchain_format, swapchain_extent, swapchain_image_views) =
            Self::create_swapchain(
                &device, physical_device, &surface_loader, surface,
                &swapchain_loader, window, graphics_family, present_family,
            );

        let (depth_image, depth_memory, depth_view) =
            Self::create_depth_resources(&instance, physical_device, &device, swapchain_extent);

        let render_pass = Self::create_render_pass(&device, swapchain_format);

        let descriptor_set_layout = Self::create_scene_descriptor_set_layout(&device);

        let vert_spv = assets.load_shader_spv("shaders/mesh.vert.glsl");
        let frag_spv = assets.load_shader_spv("shaders/mesh.frag.glsl");
        let (pipeline_layout, pipeline) = Self::create_pipeline(
            &device, render_pass,
            descriptor_set_layout,
            &vert_spv[..], &frag_spv[..],
        );

        let framebuffers = Self::create_framebuffers(
            &device, render_pass, &swapchain_image_views, depth_view, swapchain_extent,
        );

        let command_pool = Self::create_command_pool(&device, graphics_family);
        let command_buffers = Self::alloc_command_buffers(&device, command_pool);
        let (image_available, in_flight) = Self::create_sync_objects(&device);
        let image_count = swapchain_image_views.len();
        let render_finished = Self::create_semaphores(&device, image_count);
        let images_in_flight = vec![vk::Fence::null(); image_count];

        let (uniform_buffers, uniform_memories, uniform_mapped) =
            Self::create_uniform_buffers(&instance, physical_device, &device);

        let descriptor_pool = Self::create_scene_descriptor_pool(&device);
        let descriptor_sets = Self::create_scene_descriptor_sets(
            &device, descriptor_pool, descriptor_set_layout, &uniform_buffers,
        );

        log::info!(
            "Renderer initialized: {}x{}",
            swapchain_extent.width,
            swapchain_extent.height
        );

        Self {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            graphics_family,
            present_family,
            swapchain_loader,
            swapchain,
            swapchain_format,
            swapchain_extent,
            swapchain_image_views,
            depth_image,
            depth_memory,
            depth_view,
            render_pass,
            descriptor_set_layout,
            pipeline_layout,
            pipeline,
            framebuffers,
            command_pool,
            command_buffers,
            image_available,
            in_flight,
            render_finished,
            images_in_flight,
            current_frame: 0,
            descriptor_pool,
            descriptor_sets,
            uniform_buffers,
            uniform_memories,
            uniform_mapped,
            mesh_cache: HashMap::new(),
            #[cfg(debug_assertions)]
            debug_utils,
            #[cfg(debug_assertions)]
            debug_messenger,
        }
    }

    fn create_instance(entry: &ash::Entry, window: &Window) -> Instance {
        let surface_extensions = ash_window::enumerate_required_extensions(
            window.display_handle().unwrap().as_raw(),
        )
        .unwrap();

        #[cfg_attr(not(debug_assertions), allow(unused_mut))]
        let mut extensions: Vec<*const i8> = surface_extensions.to_vec();
        #[cfg(debug_assertions)]
        extensions.push(ash::ext::debug_utils::NAME.as_ptr());

        #[cfg(debug_assertions)]
        let layers = [c"VK_LAYER_KHRONOS_validation".as_ptr()];
        #[cfg(not(debug_assertions))]
        let layers: [*const i8; 0] = [];

        let app_info = vk::ApplicationInfo::default()
            .api_version(vk::API_VERSION_1_2);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);

        unsafe {
            entry
                .create_instance(&create_info, None)
                .expect("Failed to create Vulkan instance")
        }
    }

    #[cfg(debug_assertions)]
    fn setup_debug_messenger(
        entry: &ash::Entry,
        instance: &Instance,
    ) -> (ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT) {
        let debug_utils = ash::ext::debug_utils::Instance::new(entry, instance);
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));
        let messenger = unsafe {
            debug_utils
                .create_debug_utils_messenger(&create_info, None)
                .expect("Failed to create debug messenger")
        };
        (debug_utils, messenger)
    }

    fn pick_physical_device(
        instance: &Instance,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> (vk::PhysicalDevice, u32, u32) {
        let devices = unsafe { instance.enumerate_physical_devices().unwrap() };
        for device in devices {
            if let Some((gfx, prs)) =
                Self::check_device(instance, device, surface_loader, surface)
            {
                let props = unsafe { instance.get_physical_device_properties(device) };
                let name =
                    unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy();
                log::info!("Using GPU: {}", name);
                return (device, gfx, prs);
            }
        }
        panic!("No suitable Vulkan GPU found");
    }

    fn check_device(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Option<(u32, u32)> {
        let props = unsafe { instance.get_physical_device_queue_family_properties(device) };
        let mut gfx = None;
        let mut prs = None;
        for (i, prop) in props.iter().enumerate() {
            if prop.queue_flags.contains(vk::QueueFlags::GRAPHICS) && gfx.is_none() {
                gfx = Some(i as u32);
            }
            let present_ok = unsafe {
                surface_loader
                    .get_physical_device_surface_support(device, i as u32, surface)
                    .unwrap_or(false)
            };
            if present_ok && prs.is_none() {
                prs = Some(i as u32);
            }
        }
        let (gfx, prs) = (gfx?, prs?);

        let extensions = unsafe {
            instance
                .enumerate_device_extension_properties(device)
                .unwrap_or_default()
        };
        let has_swapchain = extensions.iter().any(|e| {
            let name = unsafe { std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) };
            name == ash::khr::swapchain::NAME
        });
        if !has_swapchain {
            return None;
        }

        let formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(device, surface)
                .unwrap_or_default()
        };
        if formats.is_empty() {
            return None;
        }

        Some((gfx, prs))
    }

    fn create_device(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        graphics_family: u32,
        present_family: u32,
    ) -> Device {
        let priority = [1.0f32];
        let mut families = vec![graphics_family];
        if present_family != graphics_family {
            families.push(present_family);
        }
        let queue_infos: Vec<_> = families
            .iter()
            .map(|&f| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(f)
                    .queue_priorities(&priority)
            })
            .collect();

        let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
        let features = vk::PhysicalDeviceFeatures::default();

        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&device_extensions)
            .enabled_features(&features);

        unsafe {
            instance
                .create_device(physical_device, &create_info, None)
                .expect("Failed to create logical device")
        }
    }

    fn create_swapchain(
        device: &Device,
        physical_device: vk::PhysicalDevice,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
        swapchain_loader: &ash::khr::swapchain::Device,
        window: &Window,
        graphics_family: u32,
        present_family: u32,
    ) -> (vk::SwapchainKHR, vk::Format, vk::Extent2D, Vec<vk::ImageView>) {
        let caps = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .unwrap()
        };
        let formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap()
        };
        let format = formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .cloned()
            .unwrap_or(formats[0]);

        let present_mode = vk::PresentModeKHR::FIFO;

        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: window.width().clamp(
                    caps.min_image_extent.width,
                    caps.max_image_extent.width,
                ),
                height: window.height().clamp(
                    caps.min_image_extent.height,
                    caps.max_image_extent.height,
                ),
            }
        };

        let image_count = {
            let desired = caps.min_image_count + 1;
            if caps.max_image_count > 0 { desired.min(caps.max_image_count) } else { desired }
        };

        let (sharing_mode, family_indices) = if graphics_family != present_family {
            (vk::SharingMode::CONCURRENT, vec![graphics_family, present_family])
        } else {
            (vk::SharingMode::EXCLUSIVE, vec![])
        };

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(sharing_mode)
            .queue_family_indices(&family_indices)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        let swapchain = unsafe {
            swapchain_loader.create_swapchain(&create_info, None).unwrap()
        };

        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() };
        let image_views = images
            .iter()
            .map(|&img| {
                let info = vk::ImageViewCreateInfo::default()
                    .image(img)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format.format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe { device.create_image_view(&info, None).unwrap() }
            })
            .collect();

        (swapchain, format.format, extent, image_views)
    }

    fn create_depth_resources(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        extent: vk::Extent2D,
    ) -> (vk::Image, vk::DeviceMemory, vk::ImageView) {
        let format = vk::Format::D32_SFLOAT;

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe { device.create_image(&image_info, None).unwrap() };
        let mem_reqs = unsafe { device.get_image_memory_requirements(image) };
        let mem_type = Self::find_memory_type(
            instance, physical_device, mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);
        let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { device.bind_image_memory(image, memory, 0).unwrap() };

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
        let view = unsafe { device.create_image_view(&view_info, None).unwrap() };

        (image, memory, view)
    }

    fn create_render_pass(device: &Device, color_format: vk::Format) -> vk::RenderPass {
        let color_att = vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let depth_att = vk::AttachmentDescription::default()
            .format(vk::Format::D32_SFLOAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let color_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let depth_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_ref))
            .depth_stencil_attachment(&depth_ref);

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            );

        let attachments = [color_att, depth_att];
        let info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        unsafe { device.create_render_pass(&info, None).unwrap() }
    }

    fn create_scene_descriptor_set_layout(device: &Device) -> vk::DescriptorSetLayout {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT);

        let info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(std::slice::from_ref(&binding));

        unsafe { device.create_descriptor_set_layout(&info, None).unwrap() }
    }

    fn create_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
        scene_layout: vk::DescriptorSetLayout,
        vert_spv: &[u32],
        frag_spv: &[u32],
    ) -> (vk::PipelineLayout, vk::Pipeline) {
        let vert_module = unsafe {
            device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo::default().code(vert_spv),
                    None,
                )
                .unwrap()
        };
        let frag_module = unsafe {
            device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo::default().code(frag_spv),
                    None,
                )
                .unwrap()
        };

        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(c"main"),
        ];

        let vertex_stride = std::mem::size_of::<brv_engine::Vertex>() as u32;
        let binding_desc = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(vertex_stride)
            .input_rate(vk::VertexInputRate::VERTEX);

        let attr_descs = [
            vk::VertexInputAttributeDescription::default()
                .location(0).binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .location(1).binding(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(12),
            vk::VertexInputAttributeDescription::default()
                .location(2).binding(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(24),
        ];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding_desc))
            .vertex_attribute_descriptions(&attr_descs);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        let push_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<PushData>() as u32);

        let set_layouts = [scene_layout];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(std::slice::from_ref(&push_range));

        let layout = unsafe {
            device.create_pipeline_layout(&layout_info, None).unwrap()
        };

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("Failed to create graphics pipeline")[0]
        };

        unsafe {
            device.destroy_shader_module(vert_module, None);
            device.destroy_shader_module(frag_module, None);
        }

        (layout, pipeline)
    }

    fn create_framebuffers(
        device: &Device,
        render_pass: vk::RenderPass,
        image_views: &[vk::ImageView],
        depth_view: vk::ImageView,
        extent: vk::Extent2D,
    ) -> Vec<vk::Framebuffer> {
        image_views
            .iter()
            .map(|&view| {
                let attachments = [view, depth_view];
                let info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                unsafe { device.create_framebuffer(&info, None).unwrap() }
            })
            .collect()
    }

    fn create_command_pool(device: &Device, graphics_family: u32) -> vk::CommandPool {
        let info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_family);
        unsafe { device.create_command_pool(&info, None).unwrap() }
    }

    fn alloc_command_buffers(device: &Device, pool: vk::CommandPool) -> Vec<vk::CommandBuffer> {
        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES as u32);
        unsafe { device.allocate_command_buffers(&info).unwrap() }
    }

    fn create_sync_objects(device: &Device) -> (Vec<vk::Semaphore>, Vec<vk::Fence>) {
        let sem_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let mut image_available = Vec::new();
        let mut in_flight = Vec::new();

        for _ in 0..MAX_FRAMES {
            unsafe {
                image_available.push(device.create_semaphore(&sem_info, None).unwrap());
                in_flight.push(device.create_fence(&fence_info, None).unwrap());
            }
        }

        (image_available, in_flight)
    }

    fn create_semaphores(device: &Device, count: usize) -> Vec<vk::Semaphore> {
        let info = vk::SemaphoreCreateInfo::default();
        (0..count)
            .map(|_| unsafe { device.create_semaphore(&info, None).unwrap() })
            .collect()
    }

    fn create_uniform_buffers(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
    ) -> (Vec<vk::Buffer>, Vec<vk::DeviceMemory>, Vec<*mut u8>) {
        let size = std::mem::size_of::<SceneUBO>() as u64;
        let mut buffers = Vec::new();
        let mut memories = Vec::new();
        let mut ptrs = Vec::new();

        for _ in 0..MAX_FRAMES {
            let (buf, mem) = Self::create_buffer(
                instance, physical_device, device, size,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let ptr = unsafe {
                device
                    .map_memory(mem, 0, size, vk::MemoryMapFlags::empty())
                    .unwrap() as *mut u8
            };
            buffers.push(buf);
            memories.push(mem);
            ptrs.push(ptr);
        }

        (buffers, memories, ptrs)
    }

    fn create_scene_descriptor_pool(device: &Device) -> vk::DescriptorPool {
        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(MAX_FRAMES as u32);

        let info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(std::slice::from_ref(&pool_size))
            .max_sets(MAX_FRAMES as u32);

        unsafe { device.create_descriptor_pool(&info, None).unwrap() }
    }

    fn create_scene_descriptor_sets(
        device: &Device,
        pool: vk::DescriptorPool,
        layout: vk::DescriptorSetLayout,
        uniform_buffers: &[vk::Buffer],
    ) -> Vec<vk::DescriptorSet> {
        let layouts = [layout; MAX_FRAMES];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(&layouts);

        let sets = unsafe { device.allocate_descriptor_sets(&alloc_info).unwrap() };

        for (i, &set) in sets.iter().enumerate() {
            let buffer_info = vk::DescriptorBufferInfo::default()
                .buffer(uniform_buffers[i])
                .offset(0)
                .range(std::mem::size_of::<SceneUBO>() as u64);

            let write = vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&buffer_info));

            unsafe { device.update_descriptor_sets(&[write], &[]) };
        }

        sets
    }

    fn find_memory_type(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        type_bits: u32,
        props: vk::MemoryPropertyFlags,
    ) -> u32 {
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        for i in 0..mem_props.memory_type_count {
            if type_bits & (1 << i) != 0
                && mem_props.memory_types[i as usize].property_flags.contains(props)
            {
                return i;
            }
        }
        panic!("No suitable memory type found");
    }

    fn create_buffer(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        props: vk::MemoryPropertyFlags,
    ) -> (vk::Buffer, vk::DeviceMemory) {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { device.create_buffer(&buffer_info, None).unwrap() };
        let mem_reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let mem_type = Self::find_memory_type(
            instance, physical_device, mem_reqs.memory_type_bits, props,
        );
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type);
        let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { device.bind_buffer_memory(buffer, memory, 0).unwrap() };
        (buffer, memory)
    }

    fn upload_mesh(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        device: &Device,
        mesh: &brv_engine::MeshComponent,
    ) -> GpuMesh {
        let vb_size = (mesh.data.vertices.len() * std::mem::size_of::<brv_engine::Vertex>())
            as vk::DeviceSize;
        let ib_size =
            (mesh.data.indices.len() * std::mem::size_of::<u32>()) as vk::DeviceSize;

        let (vertex_buffer, vertex_memory) = Self::create_buffer(
            instance, physical_device, device, vb_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        unsafe {
            let ptr = device
                .map_memory(vertex_memory, 0, vb_size, vk::MemoryMapFlags::empty())
                .unwrap();
            std::ptr::copy_nonoverlapping(
                mesh.data.vertices.as_ptr() as *const u8,
                ptr as *mut u8,
                vb_size as usize,
            );
            device.unmap_memory(vertex_memory);
        }

        let (index_buffer, index_memory) = Self::create_buffer(
            instance, physical_device, device, ib_size,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        unsafe {
            let ptr = device
                .map_memory(index_memory, 0, ib_size, vk::MemoryMapFlags::empty())
                .unwrap();
            std::ptr::copy_nonoverlapping(
                mesh.data.indices.as_ptr() as *const u8,
                ptr as *mut u8,
                ib_size as usize,
            );
            device.unmap_memory(index_memory);
        }

        GpuMesh {
            vertex_buffer,
            vertex_memory,
            index_buffer,
            index_memory,
            index_count: mesh.data.indices.len() as u32,
        }
    }

    fn compute_view_proj(
        world: &brv_engine::World,
        aspect: f32,
    ) -> ([[f32; 4]; 4], [f32; 4]) {
        let mut camera_count = 0;
        let mut result = None;

        for obj in world.objects() {
            if let Some(cam) = &obj.camera {
                camera_count += 1;
                if camera_count > 1 {
                    panic!("Multiple cameras found: only one camera allowed");
                }
                let pos = obj.transform.get();
                let quat = obj.rotate.quat();
                let forward = quat * Vec3::Z;
                let up = quat * Vec3::Y;
                let view = Mat4::look_to_rh(pos, forward, up);
                let mut proj = Mat4::perspective_rh(
                    cam.fov.to_radians(),
                    aspect,
                    cam.near,
                    cam.far,
                );
                proj.y_axis.y *= -1.0;
                result = Some(((proj * view).to_cols_array_2d(), [pos.x, pos.y, pos.z, 0.0]));
            }
        }

        result.unwrap_or_else(|| (Mat4::IDENTITY.to_cols_array_2d(), [0.0; 4]))
    }

    fn collect_draw_calls(&mut self, world: &brv_engine::World) -> Vec<DrawCall> {
        let mut calls = Vec::new();

        for obj in world.objects() {
            if !obj.visible.get() {
                continue;
            }
            if let Some(mesh) = &obj.mesh {
                let key = std::sync::Arc::as_ptr(&mesh.data) as usize;

                if !self.mesh_cache.contains_key(&key) {
                    let gpu_mesh = Self::upload_mesh(
                        &self.instance,
                        self.physical_device,
                        &self.device,
                        mesh,
                    );
                    self.mesh_cache.insert(key, gpu_mesh);
                }

                let pos = obj.transform.get();
                let rot = obj.rotate.quat();
                let scale = obj.transform.get_scale();
                let model = (Mat4::from_translation(pos)
                    * Mat4::from_quat(rot)
                    * Mat4::from_scale(scale))
                    .to_cols_array_2d();

                let mat = &mesh.material;
                calls.push(DrawCall {
                    mesh_key: key,
                    model,
                    albedo: [mat.albedo.r, mat.albedo.g, mat.albedo.b, mat.albedo.a],
                    metallic_roughness: [mat.metallic, mat.roughness, 0.0, 0.0],
                });
            }
        }

        calls
    }

    fn record_command_buffer(
        &self,
        cmd: vk::CommandBuffer,
        image_index: usize,
        draw_calls: &[DrawCall],
    ) {
        unsafe {
            self.device
                .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                .unwrap();

            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue { float32: [0.01, 0.01, 0.02, 1.0] },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
                },
            ];

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[image_index])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                })
                .clear_values(&clear_values);

            self.device.cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);
            self.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            self.device.cmd_set_viewport(cmd, 0, &[vk::Viewport {
                x: 0.0,
                y: self.swapchain_extent.height as f32,
                width: self.swapchain_extent.width as f32,
                height: -(self.swapchain_extent.height as f32),
                min_depth: 0.0,
                max_depth: 1.0,
            }]);
            self.device.cmd_set_scissor(cmd, 0, &[vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain_extent,
            }]);

            self.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_sets[self.current_frame]],
                &[],
            );

            for call in draw_calls {
                if let Some(gpu_mesh) = self.mesh_cache.get(&call.mesh_key) {
                    let push = PushData {
                        model: call.model,
                        albedo: call.albedo,
                        mr: call.metallic_roughness,
                    };
                    let push_bytes = std::slice::from_raw_parts(
                        &push as *const PushData as *const u8,
                        std::mem::size_of::<PushData>(),
                    );
                    self.device.cmd_push_constants(
                        cmd,
                        self.pipeline_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        push_bytes,
                    );

                    self.device.cmd_bind_vertex_buffers(
                        cmd, 0, &[gpu_mesh.vertex_buffer], &[0],
                    );
                    self.device.cmd_bind_index_buffer(
                        cmd, gpu_mesh.index_buffer, 0, vk::IndexType::UINT32,
                    );
                    self.device.cmd_draw_indexed(cmd, gpu_mesh.index_count, 1, 0, 0, 0);
                }
            }

            self.device.cmd_end_render_pass(cmd);
            self.device.end_command_buffer(cmd).unwrap();
        }
    }

    fn recreate_swapchain(&mut self, width: u32, height: u32) {
        unsafe { self.device.device_wait_idle().unwrap() };

        for &sem in &self.render_finished {
            unsafe { self.device.destroy_semaphore(sem, None) };
        }
        for &fb in &self.framebuffers {
            unsafe { self.device.destroy_framebuffer(fb, None) };
        }
        unsafe {
            self.device.destroy_image_view(self.depth_view, None);
            self.device.free_memory(self.depth_memory, None);
            self.device.destroy_image(self.depth_image, None);
        }
        for &view in &self.swapchain_image_views {
            unsafe { self.device.destroy_image_view(view, None) };
        }
        unsafe { self.swapchain_loader.destroy_swapchain(self.swapchain, None) };

        let caps = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .unwrap()
        };
        let formats = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.physical_device, self.surface)
                .unwrap()
        };
        let format = formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .cloned()
            .unwrap_or(formats[0]);

        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };

        let image_count = {
            let desired = caps.min_image_count + 1;
            if caps.max_image_count > 0 { desired.min(caps.max_image_count) } else { desired }
        };

        let (sharing_mode, family_indices) = if self.graphics_family != self.present_family {
            (vk::SharingMode::CONCURRENT, vec![self.graphics_family, self.present_family])
        } else {
            (vk::SharingMode::EXCLUSIVE, vec![])
        };

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(sharing_mode)
            .queue_family_indices(&family_indices)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);

        self.swapchain = unsafe {
            self.swapchain_loader.create_swapchain(&create_info, None).unwrap()
        };
        self.swapchain_format = format.format;
        self.swapchain_extent = extent;

        let images = unsafe {
            self.swapchain_loader.get_swapchain_images(self.swapchain).unwrap()
        };
        self.swapchain_image_views = images.iter().map(|&img| {
            let info = vk::ImageViewCreateInfo::default()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            unsafe { self.device.create_image_view(&info, None).unwrap() }
        }).collect();

        let (depth_image, depth_memory, depth_view) = Self::create_depth_resources(
            &self.instance, self.physical_device, &self.device, extent,
        );
        self.depth_image = depth_image;
        self.depth_memory = depth_memory;
        self.depth_view = depth_view;

        self.framebuffers = Self::create_framebuffers(
            &self.device,
            self.render_pass,
            &self.swapchain_image_views,
            self.depth_view,
            self.swapchain_extent,
        );

        let new_count = self.swapchain_image_views.len();
        self.render_finished = Self::create_semaphores(&self.device, new_count);
        self.images_in_flight = vec![vk::Fence::null(); new_count];
    }
}

impl brv_engine::RenderBackend for Renderer {
    fn draw_frame(&mut self, world: &brv_engine::World) {
        let draw_calls = self.collect_draw_calls(world);

        let aspect = self.swapchain_extent.width as f32 / self.swapchain_extent.height as f32;
        let (view_proj, camera_pos) = Self::compute_view_proj(world, aspect);

        let ubo = SceneUBO { view_proj, camera_pos };
        unsafe {
            std::ptr::copy_nonoverlapping(
                &ubo as *const SceneUBO as *const u8,
                self.uniform_mapped[self.current_frame],
                std::mem::size_of::<SceneUBO>(),
            );
        }

        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight[self.current_frame]], true, u64::MAX)
                .unwrap();

            let (image_index, _suboptimal) = match self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available[self.current_frame],
                vk::Fence::null(),
            ) {
                Ok(result) => result,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return,
                Err(e) => panic!("acquire_next_image failed: {:?}", e),
            };

            let ii = image_index as usize;
            if self.images_in_flight[ii] != vk::Fence::null() {
                self.device
                    .wait_for_fences(&[self.images_in_flight[ii]], true, u64::MAX)
                    .unwrap();
            }
            self.images_in_flight[ii] = self.in_flight[self.current_frame];

            self.device
                .reset_fences(&[self.in_flight[self.current_frame]])
                .unwrap();

            let cmd = self.command_buffers[self.current_frame];
            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(cmd, ii, &draw_calls);

            let wait_sems = [self.image_available[self.current_frame]];
            let signal_sems = [self.render_finished[ii]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmds = [cmd];

            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait_sems)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signal_sems);

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit],
                    self.in_flight[self.current_frame],
                )
                .unwrap();

            let swapchains = [self.swapchain];
            let indices = [image_index];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_sems)
                .swapchains(&swapchains)
                .image_indices(&indices);

            match self.swapchain_loader.queue_present(self.present_queue, &present) {
                Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {}
                Err(e) => panic!("queue_present failed: {:?}", e),
            }

            self.current_frame = (self.current_frame + 1) % MAX_FRAMES;
        }
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        self.recreate_swapchain(width, height);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            for (_, gpu_mesh) in self.mesh_cache.drain() {
                self.device.destroy_buffer(gpu_mesh.vertex_buffer, None);
                self.device.free_memory(gpu_mesh.vertex_memory, None);
                self.device.destroy_buffer(gpu_mesh.index_buffer, None);
                self.device.free_memory(gpu_mesh.index_memory, None);
            }

            self.device.destroy_descriptor_pool(self.descriptor_pool, None);

            for i in 0..MAX_FRAMES {
                self.device.unmap_memory(self.uniform_memories[i]);
                self.device.destroy_buffer(self.uniform_buffers[i], None);
                self.device.free_memory(self.uniform_memories[i], None);
            }

            for i in 0..MAX_FRAMES {
                self.device.destroy_semaphore(self.image_available[i], None);
                self.device.destroy_fence(self.in_flight[i], None);
            }
            for &sem in &self.render_finished {
                self.device.destroy_semaphore(sem, None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_image_view(self.depth_view, None);
            self.device.free_memory(self.depth_memory, None);
            self.device.destroy_image(self.depth_image, None);
            for &view in &self.swapchain_image_views {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);

            #[cfg(debug_assertions)]
            self.debug_utils.destroy_debug_utils_messenger(self.debug_messenger, None);

            self.instance.destroy_instance(None);
        }
    }
}

#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _type: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let msg = unsafe { std::ffi::CStr::from_ptr((*data).p_message) }.to_string_lossy();
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("[Vulkan] {}", msg);
    } else {
        log::warn!("[Vulkan] {}", msg);
    }
    vk::FALSE
}
