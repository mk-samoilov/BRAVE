use ash::vk;

use crate::context::VulkanContext;
use crate::mesh::Vertex;

const VERT_SPV:          &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mesh.vert.glsl.spv"));
const FRAG_SPV:          &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mesh.frag.glsl.spv"));
const SHADOW_VERT_SPV:   &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shadow.vert.glsl.spv"));
const SKYBOX_VERT_SPV:   &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skybox.vert.glsl.spv"));
const SKYBOX_FRAG_SPV:   &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skybox.frag.glsl.spv"));

pub const MAX_POINT_LIGHTS: usize = 8;
pub const MAX_SPOT_LIGHTS:  usize = 4;
pub const SHADOW_MAP_SIZE:  u32   = 2048;
pub const MSAA_SAMPLES: vk::SampleCountFlags = vk::SampleCountFlags::TYPE_8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameUbo {
    pub view:  [f32; 16],
    pub proj:  [f32; 16],
    pub dir_light_dir:   [f32; 4],
    pub dir_light_color: [f32; 4],
    pub ambient: [f32; 4],
    pub point_pos_range:       [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_count: i32,
    pub _pad0: [i32; 3],
    pub spot_pos_range:       [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_color_intensity: [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_dir_angle:       [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_count: i32,
    pub _pad1: [i32; 3],
    pub light_space_matrix: [f32; 16],
    pub shadows_enabled: i32,
    pub _pad2: [i32; 3],
    pub cam_pos: [f32; 4],
}

#[repr(C)]
pub struct PushConstants {
    pub model:      [f32; 16],
    pub base_color: [f32; 4],
    pub metallic:   f32,
    pub roughness:  f32,
    pub _pad:       [f32; 2],
}

#[repr(C)]
pub struct ShadowPush {
    pub model:        [f32; 16],
    pub light_space:  [f32; 16],
}

pub struct Pipeline {
    pub render_pass:         vk::RenderPass,
    pub layout:              vk::PipelineLayout,
    pub handle:              vk::Pipeline,
    pub desc_set_layout:     vk::DescriptorSetLayout,
    pub tex_desc_set_layout: vk::DescriptorSetLayout,
}

impl Pipeline {
    pub fn new(ctx: &VulkanContext, color_format: vk::Format, depth_format: vk::Format) -> Self {
        let vert_module = create_shader_module(&ctx.device, VERT_SPV);
        let frag_module = create_shader_module(&ctx.device, FRAG_SPV);
        let entry = std::ffi::CString::new("main").unwrap();

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(&entry),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(&entry),
        ];

        let binding    = Vertex::binding_description();
        let attributes = Vertex::attribute_descriptions();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attributes);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(MSAA_SAMPLES)
            .sample_shading_enable(true)
            .min_sample_shading(0.2);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);

        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state  = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states);

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let desc_set_layout = unsafe {
            ctx.device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .unwrap()
        };

        let push_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: std::mem::size_of::<PushConstants>() as u32,
        };

        // set = 1: per-object albedo texture
        let tex_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let tex_desc_set_layout = unsafe {
            ctx.device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(std::slice::from_ref(&tex_binding)),
                    None,
                )
                .unwrap()
        };

        let set_layouts = [desc_set_layout, tex_desc_set_layout, tex_desc_set_layout, tex_desc_set_layout, tex_desc_set_layout];
        let layout = unsafe {
            ctx.device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts)
                        .push_constant_ranges(std::slice::from_ref(&push_range)),
                    None,
                )
                .unwrap()
        };

        let render_pass = create_color_depth_render_pass(&ctx.device, color_format, depth_format);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
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

        let handle = unsafe {
            ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("Failed to create main pipeline")[0]
        };

        unsafe {
            ctx.device.destroy_shader_module(vert_module, None);
            ctx.device.destroy_shader_module(frag_module, None);
        }

        Self { render_pass, layout, handle, desc_set_layout, tex_desc_set_layout }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.handle, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_render_pass(self.render_pass, None);
            device.destroy_descriptor_set_layout(self.desc_set_layout, None);
            device.destroy_descriptor_set_layout(self.tex_desc_set_layout, None);
        }
    }
}

pub struct ShadowPipeline {
    pub render_pass: vk::RenderPass,
    pub layout:      vk::PipelineLayout,
    pub handle:      vk::Pipeline,
}

impl ShadowPipeline {
    pub fn new(ctx: &VulkanContext, depth_format: vk::Format) -> Self {
        let vert_module = create_shader_module(&ctx.device, SHADOW_VERT_SPV);
        let entry = std::ffi::CString::new("main").unwrap();

        let shader_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(&entry);

        let binding    = Vertex::binding_description();
        let attributes = Vertex::attribute_descriptions();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attributes);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(true)
            .depth_bias_constant_factor(1.25)
            .depth_bias_slope_factor(1.75);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state  = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states);

        let push_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX,
            offset: 0,
            size: std::mem::size_of::<ShadowPush>() as u32,
        };

        let layout = unsafe {
            ctx.device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .push_constant_ranges(std::slice::from_ref(&push_range)),
                    None,
                )
                .unwrap()
        };

        let render_pass = create_depth_only_render_pass(&ctx.device, depth_format);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(std::slice::from_ref(&shader_stage))
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .dynamic_state(&dynamic_state)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);

        let handle = unsafe {
            ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("Failed to create shadow pipeline")[0]
        };

        unsafe { ctx.device.destroy_shader_module(vert_module, None) };

        Self { render_pass, layout, handle }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.handle, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_render_pass(self.render_pass, None);
        }
    }
}

pub struct SkyboxPipeline {
    pub layout:              vk::PipelineLayout,
    pub handle:              vk::Pipeline,
    pub tex_desc_set_layout: vk::DescriptorSetLayout,
}

impl SkyboxPipeline {
    pub fn new(ctx: &VulkanContext, render_pass: vk::RenderPass) -> Self {
        let vert_module = create_shader_module(&ctx.device, SKYBOX_VERT_SPV);
        let frag_module = create_shader_module(&ctx.device, SKYBOX_FRAG_SPV);
        let entry = std::ffi::CString::new("main").unwrap();

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(&entry),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(&entry),
        ];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(MSAA_SAMPLES);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false)
            .depth_write_enable(false);

        let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&blend_attachment));

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state  = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states);

        let tex_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let tex_desc_set_layout = unsafe {
            ctx.device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(std::slice::from_ref(&tex_binding)),
                    None,
                )
                .unwrap()
        };

        let push_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 68,
        };

        let set_layouts = [tex_desc_set_layout];
        let layout = unsafe {
            ctx.device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts)
                        .push_constant_ranges(std::slice::from_ref(&push_range)),
                    None,
                )
                .unwrap()
        };

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
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

        let handle = unsafe {
            ctx.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("Failed to create skybox pipeline")[0]
        };

        unsafe {
            ctx.device.destroy_shader_module(vert_module, None);
            ctx.device.destroy_shader_module(frag_module, None);
        }

        Self { layout, handle, tex_desc_set_layout }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_pipeline(self.handle, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_set_layout(self.tex_desc_set_layout, None);
        }
    }
}

fn create_shader_module(device: &ash::Device, spv_bytes: &[u8]) -> vk::ShaderModule {
    assert_eq!(spv_bytes.len() % 4, 0, "SPIR-V size must be a multiple of 4");
    let spv: Vec<u32> = spv_bytes
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let create_info = vk::ShaderModuleCreateInfo::default().code(&spv);
    unsafe { device.create_shader_module(&create_info, None).unwrap() }
}

fn create_color_depth_render_pass(
    device: &ash::Device,
    color_format: vk::Format,
    depth_format: vk::Format,
) -> vk::RenderPass {
    let msaa_color_att = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(MSAA_SAMPLES)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

    let depth_att = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(MSAA_SAMPLES)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let resolve_att = vk::AttachmentDescription::default()
        .format(color_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

    let color_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL };
    let depth_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
    let resolve_ref = vk::AttachmentReference { attachment: 2, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL };

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_ref))
        .depth_stencil_attachment(&depth_ref)
        .resolve_attachments(std::slice::from_ref(&resolve_ref));

    let dependency = vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        src_access_mask: vk::AccessFlags::empty(),
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        dependency_flags: vk::DependencyFlags::empty(),
    };

    let attachments = [msaa_color_att, depth_att, resolve_att];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(std::slice::from_ref(&subpass))
                    .dependencies(std::slice::from_ref(&dependency)),
                None,
            )
            .unwrap()
    }
}

pub fn create_depth_only_render_pass(device: &ash::Device, depth_format: vk::Format) -> vk::RenderPass {
    let depth_att = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

    let depth_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_ref);

    let dep_write = vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
        src_access_mask: vk::AccessFlags::SHADER_READ,
        dst_stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        dst_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        dependency_flags: vk::DependencyFlags::BY_REGION,
    };
    let dep_read = vk::SubpassDependency {
        src_subpass: 0,
        dst_subpass: vk::SUBPASS_EXTERNAL,
        src_stage_mask: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
        src_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
        dst_access_mask: vk::AccessFlags::SHADER_READ,
        dependency_flags: vk::DependencyFlags::BY_REGION,
    };

    let deps = [dep_write, dep_read];
    unsafe {
        device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&depth_att))
                    .subpasses(std::slice::from_ref(&subpass))
                    .dependencies(&deps),
                None,
            )
            .unwrap()
    }
}
