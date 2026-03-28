use ash::{vk, Device, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use brv_window::Window;

const MAX_FRAMES: usize = 2;

const VERT_SRC: &str = r#"
#version 450

layout(location = 0) out vec3 frag_color;

void main() {
    vec2 pos;
    vec3 col;
    if (gl_VertexIndex == 0) {
        pos = vec2(0.0, -0.5);
        col = vec3(1.0, 0.0, 0.0);
    } else if (gl_VertexIndex == 1) {
        pos = vec2(0.5, 0.5);
        col = vec3(0.0, 1.0, 0.0);
    } else {
        pos = vec2(-0.5, 0.5);
        col = vec3(0.0, 0.0, 1.0);
    }
    gl_Position = vec4(pos, 0.0, 1.0);
    frag_color = col;
}
"#;

const FRAG_SRC: &str = r#"
#version 450

layout(location = 0) in vec3 frag_color;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(frag_color, 1.0);
}
"#;

fn compile_glsl(src: &str, stage: naga::ShaderStage) -> Vec<u32> {
    let mut frontend = naga::front::glsl::Frontend::default();
    let module = frontend
        .parse(&naga::front::glsl::Options::from(stage), src)
        .expect("GLSL parse error");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::empty(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("Shader validation error");
    naga::back::spv::write_vec(
        &module,
        &info,
        &naga::back::spv::Options {
            lang_version: (1, 0),
            ..Default::default()
        },
        None,
    )
    .expect("SPIR-V generation error")
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
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available: Vec<vk::Semaphore>,
    render_finished: Vec<vk::Semaphore>,
    in_flight: Vec<vk::Fence>,
    current_frame: usize,
    #[cfg(debug_assertions)]
    debug_utils: ash::ext::debug_utils::Instance,
    #[cfg(debug_assertions)]
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
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
                &device,
                physical_device,
                &surface_loader,
                surface,
                &swapchain_loader,
                window,
                graphics_family,
                present_family,
            );

        let render_pass = Self::create_render_pass(&device, swapchain_format);

        let vert_spv = compile_glsl(VERT_SRC, naga::ShaderStage::Vertex);
        let frag_spv = compile_glsl(FRAG_SRC, naga::ShaderStage::Fragment);
        let (pipeline_layout, pipeline) =
            Self::create_pipeline(&device, render_pass, &vert_spv, &frag_spv);

        let framebuffers =
            Self::create_framebuffers(&device, render_pass, &swapchain_image_views, swapchain_extent);

        let command_pool = Self::create_command_pool(&device, graphics_family);
        let command_buffers = Self::alloc_command_buffers(&device, command_pool);
        let (image_available, render_finished, in_flight) = Self::create_sync_objects(&device);

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
            render_pass,
            pipeline_layout,
            pipeline,
            framebuffers,
            command_pool,
            command_buffers,
            image_available,
            render_finished,
            in_flight,
            current_frame: 0,
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
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
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

        let present_mode = present_modes
            .iter()
            .copied()
            .find(|&m| m == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);

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
            if caps.max_image_count > 0 {
                desired.min(caps.max_image_count)
            } else {
                desired
            }
        };

        let (sharing_mode, family_indices) = if graphics_family != present_family {
            (
                vk::SharingMode::CONCURRENT,
                vec![graphics_family, present_family],
            )
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
            swapchain_loader
                .create_swapchain(&create_info, None)
                .unwrap()
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

    fn create_render_pass(device: &Device, format: vk::Format) -> vk::RenderPass {
        let attachment = vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&attachment_ref));

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        let info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        unsafe { device.create_render_pass(&info, None).unwrap() }
    }

    fn create_pipeline(
        device: &Device,
        render_pass: vk::RenderPass,
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

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
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
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        let layout = unsafe {
            device
                .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)
                .unwrap()
        };

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
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
        extent: vk::Extent2D,
    ) -> Vec<vk::Framebuffer> {
        image_views
            .iter()
            .map(|&view| {
                let attachments = [view];
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

    fn create_sync_objects(
        device: &Device,
    ) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
        let sem_info = vk::SemaphoreCreateInfo::default();
        let fence_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let mut image_available = Vec::new();
        let mut render_finished = Vec::new();
        let mut in_flight = Vec::new();

        for _ in 0..MAX_FRAMES {
            unsafe {
                image_available.push(device.create_semaphore(&sem_info, None).unwrap());
                render_finished.push(device.create_semaphore(&sem_info, None).unwrap());
                in_flight.push(device.create_fence(&fence_info, None).unwrap());
            }
        }

        (image_available, render_finished, in_flight)
    }

    fn record_command_buffer(&self, cmd: vk::CommandBuffer, image_index: usize) {
        unsafe {
            self.device
                .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                .unwrap();

            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.01, 0.01, 0.02, 1.0],
                },
            };

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[image_index])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                })
                .clear_values(std::slice::from_ref(&clear));

            self.device
                .cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);
            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            self.device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.swapchain_extent.width as f32,
                    height: self.swapchain_extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                }],
            );

            self.device.cmd_draw(cmd, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(cmd);
            self.device.end_command_buffer(cmd).unwrap();
        }
    }

    fn recreate_swapchain(&mut self, width: u32, height: u32) {
        unsafe { self.device.device_wait_idle().unwrap() };

        for &fb in &self.framebuffers {
            unsafe { self.device.destroy_framebuffer(fb, None) };
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
        let present_modes = unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
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

        let present_mode = present_modes
            .iter()
            .copied()
            .find(|&m| m == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);

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
            .present_mode(present_mode)
            .clipped(true);

        self.swapchain = unsafe {
            self.swapchain_loader.create_swapchain(&create_info, None).unwrap()
        };
        self.swapchain_format = format.format;
        self.swapchain_extent = extent;

        let images = unsafe { self.swapchain_loader.get_swapchain_images(self.swapchain).unwrap() };
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

        self.framebuffers = Self::create_framebuffers(
            &self.device,
            self.render_pass,
            &self.swapchain_image_views,
            self.swapchain_extent,
        );
    }
}

impl brv_engine::RenderBackend for Renderer {
    fn draw_frame(&mut self) {
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

            self.device
                .reset_fences(&[self.in_flight[self.current_frame]])
                .unwrap();

            let cmd = self.command_buffers[self.current_frame];
            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(cmd, image_index as usize);

            let wait_sems = [self.image_available[self.current_frame]];
            let signal_sems = [self.render_finished[self.current_frame]];
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

            match self
                .swapchain_loader
                .queue_present(self.present_queue, &present)
            {
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

            for i in 0..MAX_FRAMES {
                self.device.destroy_semaphore(self.image_available[i], None);
                self.device.destroy_semaphore(self.render_finished[i], None);
                self.device.destroy_fence(self.in_flight[i], None);
            }
            self.device.destroy_command_pool(self.command_pool, None);
            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            for &view in &self.swapchain_image_views {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            #[cfg(debug_assertions)]
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _msg_type: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let msg = unsafe { std::ffi::CStr::from_ptr((*data).p_message) }.to_string_lossy();
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("[Vulkan] {}", msg);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        log::warn!("[Vulkan] {}", msg);
    } else {
        log::debug!("[Vulkan] {}", msg);
    }
    vk::FALSE
}
