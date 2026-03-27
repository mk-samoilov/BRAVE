use std::sync::Arc;
use std::collections::HashMap;

use ash::vk;
use brave_ecs::{Transform, World};
use brave_math::{Mat4, Vec3};

use crate::buffer::Buffer;
use crate::camera::Camera;
use crate::context::VulkanContext;
use crate::light::{AmbientLight, DirectionalLight, PointLight, SpotLight};
use crate::mesh::MeshRenderer;
use crate::pipeline::{
    AabbEntry, FrameUbo, Pipeline, PushConstants, ShadowPipeline, ShadowPush,
    SHADOW_MAP_SIZE, MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS, MAX_RT_OBJECTS,
};
use crate::swapchain::Swapchain;
use crate::texture::GpuTexture;

const FRAMES_IN_FLIGHT: usize = 2;

struct FrameData {
    image_available: vk::Semaphore,
    in_flight:       vk::Fence,
    command_buffer:  vk::CommandBuffer,
    ubo_buffer:      Buffer,
    ssbo_buffer:     Buffer,
    descriptor_set:  vk::DescriptorSet,
}

struct ShadowMap {
    image:       vk::Image,
    memory:      vk::DeviceMemory,
    view:        vk::ImageView,
    sampler:     vk::Sampler,
    framebuffer: vk::Framebuffer,
}

pub struct Renderer {
    ctx:                       VulkanContext,
    swapchain:                 Swapchain,
    pipeline:                  Pipeline,
    shadow_pipeline:           ShadowPipeline,
    shadow_map:                ShadowMap,
    framebuffers:              Vec<vk::Framebuffer>,
    command_pool:              vk::CommandPool,
    frames:                    Vec<FrameData>,
    render_finished_per_image: Vec<vk::Semaphore>,
    descriptor_pool:           vk::DescriptorPool,
    // default_texture must be declared BEFORE tex_descriptor_pool so it drops first
    default_texture:           Option<Arc<GpuTexture>>,
    tex_descriptor_pool:       vk::DescriptorPool,
    current_frame:             usize,
    _skybox:                   Option<()>,
}

impl Renderer {
    pub fn new(window: &brave_window::Window) -> Self {
        let raw = window.raw();
        let ctx = VulkanContext::new(raw);

        let swapchain = Swapchain::new(&ctx, window.width(), window.height());
        let pipeline = Pipeline::new(&ctx, swapchain.format, swapchain.depth_format);
        let shadow_pipeline = ShadowPipeline::new(&ctx, swapchain.depth_format);
        let framebuffers = create_framebuffers(&ctx, &swapchain, &pipeline);
        let command_pool = create_command_pool(&ctx);
        let shadow_map = create_shadow_map(&ctx, &shadow_pipeline, swapchain.depth_format);
        let descriptor_pool = create_descriptor_pool(&ctx, FRAMES_IN_FLIGHT as u32);

        let desc_set_layouts = vec![pipeline.desc_set_layout; FRAMES_IN_FLIGHT];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&desc_set_layouts);
        let descriptor_sets =
            unsafe { ctx.device.allocate_descriptor_sets(&alloc_info).unwrap() };

        let cmd_buffers = create_command_buffers(&ctx, command_pool, FRAMES_IN_FLIGHT);
        let ubo_size = std::mem::size_of::<FrameUbo>() as vk::DeviceSize;

        let ssbo_size = (std::mem::size_of::<AabbEntry>() * MAX_RT_OBJECTS) as vk::DeviceSize;

        let frames: Vec<FrameData> = (0..FRAMES_IN_FLIGHT)
            .map(|i| {
                let ubo_buffer = Buffer::new(
                    &ctx,
                    ubo_size,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                );

                let ssbo_buffer = Buffer::new(
                    &ctx,
                    ssbo_size,
                    vk::BufferUsageFlags::STORAGE_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                );

                let buffer_info = vk::DescriptorBufferInfo {
                    buffer: ubo_buffer.handle,
                    offset: 0,
                    range: ubo_size,
                };
                let write_ubo = vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_info));

                let image_info = vk::DescriptorImageInfo {
                    sampler:     shadow_map.sampler,
                    image_view:  shadow_map.view,
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                };
                let write_sampler = vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&image_info));

                let ssbo_info = vk::DescriptorBufferInfo {
                    buffer: ssbo_buffer.handle,
                    offset: 0,
                    range: ssbo_size,
                };
                let write_ssbo = vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(2)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(&ssbo_info));

                unsafe {
                    ctx.device.update_descriptor_sets(&[write_ubo, write_sampler, write_ssbo], &[]);
                }

                FrameData {
                    image_available: create_semaphore(&ctx),
                    in_flight: create_fence(&ctx, true),
                    command_buffer: cmd_buffers[i],
                    ubo_buffer,
                    ssbo_buffer,
                    descriptor_set: descriptor_sets[i],
                }
            })
            .collect();

        let render_finished_per_image = (0..swapchain.images.len())
            .map(|_| create_semaphore(&ctx))
            .collect();

        let tex_descriptor_pool = create_tex_descriptor_pool(&ctx, 1024);
        let default_texture = Some(GpuTexture::white(
            &ctx, command_pool, tex_descriptor_pool, pipeline.tex_desc_set_layout,
        ));

        Self {
            ctx,
            swapchain,
            pipeline,
            shadow_pipeline,
            shadow_map,
            framebuffers,
            command_pool,
            frames,
            render_finished_per_image,
            descriptor_pool,
            default_texture,
            tex_descriptor_pool,
            current_frame: 0,
            _skybox: None,
        }
    }

    pub fn render_frame(
        &mut self,
        world: &World,
        world_transforms: &HashMap<String, Mat4>,
        width: u32,
        height: u32,
    ) {
        let frame = &self.frames[self.current_frame];

        unsafe {
            self.ctx
                .device
                .wait_for_fences(&[frame.in_flight], true, u64::MAX)
                .unwrap();
        }

        let result = unsafe {
            self.swapchain.loader.acquire_next_image(
                self.swapchain.handle,
                u64::MAX,
                frame.image_available,
                vk::Fence::null(),
            )
        };

        let image_index = match result {
            Ok((index, _)) => index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(width, height);
                return;
            }
            Err(e) => panic!("Failed to acquire swapchain image: {:?}", e),
        };

        unsafe { self.ctx.device.reset_fences(&[frame.in_flight]).unwrap() };

        let rt_count = self.update_ssbo(world, world_transforms);
        self.update_ubo(world, width, height, rt_count);

        let frame = &self.frames[self.current_frame];
        unsafe {
            self.ctx
                .device
                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap()
        };
        self.record_commands(frame.command_buffer, image_index as usize, world, world_transforms);

        let frame = &self.frames[self.current_frame];
        let render_finished = self.render_finished_per_image[image_index as usize];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&frame.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&frame.command_buffer))
            .signal_semaphores(std::slice::from_ref(&render_finished));

        unsafe {
            self.ctx
                .device
                .queue_submit(self.ctx.graphics_queue, &[submit_info], frame.in_flight)
                .unwrap()
        };

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&render_finished))
            .swapchains(std::slice::from_ref(&self.swapchain.handle))
            .image_indices(std::slice::from_ref(&image_index));

        let present_result = unsafe {
            self.swapchain
                .loader
                .queue_present(self.ctx.present_queue, &present_info)
        };

        match present_result {
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(width, height)
            }
            Err(e) => panic!("Failed to present: {:?}", e),
            _ => {}
        }

        self.current_frame = (self.current_frame + 1) % FRAMES_IN_FLIGHT;
    }

    fn update_ssbo(&mut self, world: &World, world_transforms: &HashMap<String, Mat4>) -> i32 {
        let mut entries: Vec<AabbEntry> = Vec::new();

        for entity in world.entities() {
            if !entity.has::<MeshRenderer>() { continue; }
            if entries.len() >= MAX_RT_OBJECTS { break; }

            let mesh = &entity.get::<MeshRenderer>().mesh;
            let (lmin, lmax) = mesh.local_bounds;
            let model = world_transforms
                .get(&entity.name)
                .copied()
                .unwrap_or_else(|| {
                    entity.try_get::<Transform>()
                        .map(|t| t.matrix())
                        .unwrap_or(Mat4::IDENTITY)
                });

            let corners = [
                [lmin[0], lmin[1], lmin[2]],
                [lmax[0], lmin[1], lmin[2]],
                [lmin[0], lmax[1], lmin[2]],
                [lmax[0], lmax[1], lmin[2]],
                [lmin[0], lmin[1], lmax[2]],
                [lmax[0], lmin[1], lmax[2]],
                [lmin[0], lmax[1], lmax[2]],
                [lmax[0], lmax[1], lmax[2]],
            ];

            let mut wmin = [f32::MAX; 3];
            let mut wmax = [f32::MIN; 3];
            for c in &corners {
                let w = model.transform_point3(Vec3::from_array(*c));
                for i in 0..3 {
                    wmin[i] = wmin[i].min(w[i]);
                    wmax[i] = wmax[i].max(w[i]);
                }
            }

            entries.push(AabbEntry {
                min_pt: [wmin[0], wmin[1], wmin[2], 0.0],
                max_pt: [wmax[0], wmax[1], wmax[2], 0.0],
            });
        }

        let count = entries.len() as i32;
        if !entries.is_empty() {
            self.frames[self.current_frame].ssbo_buffer.upload(&self.ctx, &entries);
        }
        count
    }

    fn update_ubo(&mut self, world: &World, width: u32, height: u32, rt_aabb_count: i32) {
        let aspect = width as f32 / height.max(1) as f32;

        let (view, proj, cam_pos) = self.find_camera(world, aspect);
        let (light_dir, light_intensity, light_color, shadows_enabled, light_space_matrix) =
            self.find_directional_light(world);
        let (ambient_color, ambient_intensity) = self.find_ambient_light(world);

        let mut point_pos_range       = [[0.0f32; 4]; MAX_POINT_LIGHTS];
        let mut point_color_intensity = [[0.0f32; 4]; MAX_POINT_LIGHTS];
        let mut point_count = 0i32;

        for (pos, range, color, intensity) in self.find_point_lights(world) {
            if point_count as usize >= MAX_POINT_LIGHTS { break; }
            let i = point_count as usize;
            point_pos_range[i]       = [pos.x, pos.y, pos.z, range];
            point_color_intensity[i] = [color[0], color[1], color[2], intensity];
            point_count += 1;
        }

        let mut spot_pos_range       = [[0.0f32; 4]; MAX_SPOT_LIGHTS];
        let mut spot_color_intensity = [[0.0f32; 4]; MAX_SPOT_LIGHTS];
        let mut spot_dir_angle       = [[0.0f32; 4]; MAX_SPOT_LIGHTS];
        let mut spot_count = 0i32;

        for (pos, range, color, intensity, dir, cos_angle) in self.find_spot_lights(world) {
            if spot_count as usize >= MAX_SPOT_LIGHTS { break; }
            let i = spot_count as usize;
            spot_pos_range[i]       = [pos.x, pos.y, pos.z, range];
            spot_color_intensity[i] = [color[0], color[1], color[2], intensity];
            spot_dir_angle[i]       = [dir.x, dir.y, dir.z, cos_angle];
            spot_count += 1;
        }

        let ubo = FrameUbo {
            view: view.to_cols_array(),
            proj: proj.to_cols_array(),
            dir_light_dir:   [light_dir.x, light_dir.y, light_dir.z, light_intensity],
            dir_light_color: [light_color[0], light_color[1], light_color[2], 1.0],
            ambient:         [ambient_color[0], ambient_color[1], ambient_color[2], ambient_intensity],
            point_pos_range,
            point_color_intensity,
            point_count,
            _pad0: [0; 3],
            spot_pos_range,
            spot_color_intensity,
            spot_dir_angle,
            spot_count,
            _pad1: [0; 3],
            light_space_matrix: light_space_matrix.to_cols_array(),
            shadows_enabled: if shadows_enabled { 1 } else { 0 },
            _pad2: [0; 3],
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
            rt_aabb_count,
            _pad3: [0; 3],
        };

        self.frames[self.current_frame].ubo_buffer.upload(&self.ctx, std::slice::from_ref(&ubo));
    }

    fn find_camera(&self, world: &World, aspect: f32) -> (Mat4, Mat4, Vec3) {
        for entity in world.entities() {
            if entity.has::<Camera>() && entity.has::<Transform>() {
                let cam = entity.get::<Camera>();
                let tr  = entity.get::<Transform>();
                let forward = tr.rotation * (-Vec3::Z);
                let up      = tr.rotation * Vec3::Y;
                let view = Mat4::look_at_rh(tr.position, tr.position + forward, up);
                let mut proj = cam.projection_matrix(aspect);
                proj.y_axis.y *= -1.0;
                return (view, proj, tr.position);
            }
        }
        let pos = Vec3::new(0.0, 5.0, 10.0);
        let view = Mat4::look_at_rh(pos, Vec3::ZERO, Vec3::Y);
        let mut proj = Mat4::perspective_rh(60f32.to_radians(), aspect, 0.1, 1000.0);
        proj.y_axis.y *= -1.0;
        (view, proj, pos)
    }

    fn find_directional_light(
        &self,
        world: &World,
    ) -> (Vec3, f32, [f32; 3], bool, Mat4) {
        for entity in world.entities() {
            if entity.has::<DirectionalLight>() && entity.has::<Transform>() {
                let light = entity.get::<DirectionalLight>();
                let tr    = entity.get::<Transform>();
                let dir   = tr.position.normalize();
                let light_pos = tr.position;
                let light_view = Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
                let light_proj = Mat4::orthographic_rh(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);
                let light_space = light_proj * light_view;

                return (
                    dir,
                    light.intensity,
                    [light.color.r, light.color.g, light.color.b],
                    light.shadows,
                    light_space,
                );
            }
        }
        let dir = Vec3::new(0.5, 1.0, 0.5).normalize();
        (dir, 1.0, [1.0, 1.0, 1.0], false, Mat4::IDENTITY)
    }

    fn find_ambient_light(&self, world: &World) -> ([f32; 3], f32) {
        for entity in world.entities() {
            if entity.has::<AmbientLight>() {
                let light = entity.get::<AmbientLight>();
                return ([light.color.r, light.color.g, light.color.b], light.intensity);
            }
        }
        ([1.0, 1.0, 1.0], 0.1)
    }

    fn find_point_lights(&self, world: &World) -> Vec<(Vec3, f32, [f32; 3], f32)> {
        let mut result = Vec::new();
        for entity in world.entities() {
            if entity.has::<PointLight>() && entity.has::<Transform>() {
                let light = entity.get::<PointLight>();
                let tr    = entity.get::<Transform>();
                result.push((
                    tr.position,
                    light.range,
                    [light.color.r, light.color.g, light.color.b],
                    light.intensity,
                ));
            }
        }
        result
    }

    fn find_spot_lights(&self, world: &World) -> Vec<(Vec3, f32, [f32; 3], f32, Vec3, f32)> {
        let mut result = Vec::new();
        for entity in world.entities() {
            if entity.has::<SpotLight>() && entity.has::<Transform>() {
                let light = entity.get::<SpotLight>();
                let tr    = entity.get::<Transform>();
                let dir   = (tr.rotation * (-Vec3::Z)).normalize();
                let cos_angle = light.angle.to_radians().cos();
                result.push((
                    tr.position,
                    light.range,
                    [light.color.r, light.color.g, light.color.b],
                    light.intensity,
                    dir,
                    cos_angle,
                ));
            }
        }
        result
    }

    fn record_commands(
        &self,
        cmd: vk::CommandBuffer,
        image_index: usize,
        world: &World,
        world_transforms: &HashMap<String, Mat4>,
    ) {
        let frame = &self.frames[self.current_frame];
        let extent = self.swapchain.extent;

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { self.ctx.device.begin_command_buffer(cmd, &begin_info).unwrap() };

        self.record_shadow_pass(cmd, world, world_transforms);

        let clear_values = [
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.05, 0.05, 0.1, 1.0] } },
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
            vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } },
        ];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.pipeline.render_pass)
            .framebuffer(self.framebuffers[image_index])
            .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent })
            .clear_values(&clear_values);

        unsafe {
            self.ctx.device.cmd_begin_render_pass(cmd, &render_pass_info, vk::SubpassContents::INLINE);
            self.ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline.handle);

            let viewport = vk::Viewport {
                x: 0.0, y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0.0, max_depth: 1.0,
            };
            let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent };
            self.ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
            self.ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

            self.ctx.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline.layout,
                0,
                &[frame.descriptor_set],
                &[],
            );

            let default_tex_set = self.default_texture.as_ref().unwrap().descriptor_set;

            for entity in world.entities() {
                if !entity.has::<MeshRenderer>() || !entity.has::<Transform>() {
                    continue;
                }
                let mr = entity.get::<MeshRenderer>();
                let mesh = &mr.mesh;
                let model = world_transforms
                    .get(&entity.name)
                    .copied()
                    .unwrap_or_else(|| {
                        entity.try_get::<Transform>()
                            .map(|t| t.matrix())
                            .unwrap_or(Mat4::IDENTITY)
                    });

                // bind per-object albedo texture (set = 1)
                let tex_set = mr.texture.as_ref()
                    .map(|t| t.descriptor_set)
                    .unwrap_or(default_tex_set);
                self.ctx.device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline.layout,
                    1,
                    &[tex_set],
                    &[],
                );

                let push = PushConstants {
                    model:      model.to_cols_array(),
                    base_color: mr.base_color,
                };
                let push_bytes = std::slice::from_raw_parts(
                    &push as *const PushConstants as *const u8,
                    std::mem::size_of::<PushConstants>(),
                );
                self.ctx.device.cmd_push_constants(
                    cmd,
                    self.pipeline.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    push_bytes,
                );

                self.ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[mesh.vertex_buffer.handle], &[0]);
                self.ctx.device.cmd_bind_index_buffer(cmd, mesh.index_buffer.handle, 0, vk::IndexType::UINT32);
                self.ctx.device.cmd_draw_indexed(cmd, mesh.index_count, 1, 0, 0, 0);
            }

            self.ctx.device.cmd_end_render_pass(cmd);
            self.ctx.device.end_command_buffer(cmd).unwrap();
        }
    }

    fn record_shadow_pass(
        &self,
        cmd: vk::CommandBuffer,
        world: &World,
        world_transforms: &HashMap<String, Mat4>,
    ) {
        let light_space = self.compute_light_space(world);

        let shadow_extent = vk::Extent2D {
            width:  SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
        };
        let clear_depth = [vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
        }];

        let rp_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.shadow_pipeline.render_pass)
            .framebuffer(self.shadow_map.framebuffer)
            .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: shadow_extent })
            .clear_values(&clear_depth);

        unsafe {
            self.ctx.device.cmd_begin_render_pass(cmd, &rp_info, vk::SubpassContents::INLINE);
            self.ctx.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.shadow_pipeline.handle);

            let viewport = vk::Viewport {
                x: 0.0, y: 0.0,
                width: SHADOW_MAP_SIZE as f32,
                height: SHADOW_MAP_SIZE as f32,
                min_depth: 0.0, max_depth: 1.0,
            };
            let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent: shadow_extent };
            self.ctx.device.cmd_set_viewport(cmd, 0, &[viewport]);
            self.ctx.device.cmd_set_scissor(cmd, 0, &[scissor]);

            for entity in world.entities() {
                if !entity.has::<MeshRenderer>() || !entity.has::<Transform>() {
                    continue;
                }
                let mesh = &entity.get::<MeshRenderer>().mesh;
                let model = world_transforms
                    .get(&entity.name)
                    .copied()
                    .unwrap_or_else(|| {
                        entity.try_get::<Transform>()
                            .map(|t| t.matrix())
                            .unwrap_or(Mat4::IDENTITY)
                    });
                let push = ShadowPush {
                    model:       model.to_cols_array(),
                    light_space: light_space.to_cols_array(),
                };
                let push_bytes = std::slice::from_raw_parts(
                    &push as *const ShadowPush as *const u8,
                    std::mem::size_of::<ShadowPush>(),
                );
                self.ctx.device.cmd_push_constants(
                    cmd,
                    self.shadow_pipeline.layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    push_bytes,
                );

                self.ctx.device.cmd_bind_vertex_buffers(cmd, 0, &[mesh.vertex_buffer.handle], &[0]);
                self.ctx.device.cmd_bind_index_buffer(cmd, mesh.index_buffer.handle, 0, vk::IndexType::UINT32);
                self.ctx.device.cmd_draw_indexed(cmd, mesh.index_count, 1, 0, 0, 0);
            }

            self.ctx.device.cmd_end_render_pass(cmd);
        }
    }

    fn compute_light_space(&self, world: &World) -> Mat4 {
        for entity in world.entities() {
            if entity.has::<DirectionalLight>() && entity.has::<Transform>() {
                let tr  = entity.get::<Transform>();
                let light_view = Mat4::look_at_rh(tr.position, Vec3::ZERO, Vec3::Y);
                let light_proj = Mat4::orthographic_rh(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);
                return light_proj * light_view;
            }
        }
        Mat4::IDENTITY
    }

    fn recreate_swapchain(&mut self, width: u32, height: u32) {
        unsafe { self.ctx.device.device_wait_idle().unwrap() };

        for &fb in &self.framebuffers {
            unsafe { self.ctx.device.destroy_framebuffer(fb, None) };
        }

        self.swapchain.recreate(&self.ctx, width, height);
        self.framebuffers = create_framebuffers(&self.ctx, &self.swapchain, &self.pipeline);
    }

    pub fn set_skybox(&mut self, _texture: ()) {
        self._skybox = Some(());
    }

    pub fn create_cube(&self) -> std::sync::Arc<crate::mesh::Mesh> {
        crate::mesh::Mesh::cube(&self.ctx, self.command_pool)
    }

    pub fn create_plane(&self, size: f32) -> std::sync::Arc<crate::mesh::Mesh> {
        crate::mesh::Mesh::plane(&self.ctx, self.command_pool, size)
    }

    pub fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }

    pub fn ctx(&self) -> &VulkanContext {
        &self.ctx
    }

    pub fn tex_descriptor_pool(&self) -> vk::DescriptorPool {
        self.tex_descriptor_pool
    }

    pub fn tex_desc_set_layout(&self) -> vk::DescriptorSetLayout {
        self.pipeline.tex_desc_set_layout
    }

    /// Upload RGBA8 pixels to GPU and return a cached Arc<GpuTexture>.
    pub fn create_texture(&self, width: u32, height: u32, pixels: &[u8]) -> Arc<GpuTexture> {
        GpuTexture::from_rgba8(
            &self.ctx,
            self.command_pool,
            self.tex_descriptor_pool,
            self.pipeline.tex_desc_set_layout,
            width,
            height,
            pixels,
        )
    }

    pub fn wait_idle(&self) {
        unsafe { self.ctx.device.device_wait_idle().ok(); }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let fences: Vec<vk::Fence> = self.frames.iter().map(|f| f.in_flight).collect();
            self.ctx.device.wait_for_fences(&fences, true, u64::MAX).ok();
            self.ctx.device.device_wait_idle().ok();

            for frame in &self.frames {
                self.ctx.device.destroy_semaphore(frame.image_available, None);
                self.ctx.device.destroy_fence(frame.in_flight, None);
                frame.ubo_buffer.destroy(&self.ctx);
                frame.ssbo_buffer.destroy(&self.ctx);
            }
            for &sem in &self.render_finished_per_image {
                self.ctx.device.destroy_semaphore(sem, None);
            }

            for &fb in &self.framebuffers {
                self.ctx.device.destroy_framebuffer(fb, None);
            }

            self.ctx.device.destroy_framebuffer(self.shadow_map.framebuffer, None);
            self.ctx.device.destroy_sampler(self.shadow_map.sampler, None);
            self.ctx.device.destroy_image_view(self.shadow_map.view, None);
            self.ctx.device.destroy_image(self.shadow_map.image, None);
            self.ctx.device.free_memory(self.shadow_map.memory, None);

            self.swapchain.destroy_resources(&self.ctx.device);
            self.swapchain.loader.destroy_swapchain(self.swapchain.handle, None);

            self.shadow_pipeline.destroy(&self.ctx.device);
            self.pipeline.destroy(&self.ctx.device);
            self.ctx.device.destroy_descriptor_pool(self.descriptor_pool, None);

            // Drop default_texture first so its descriptor set is freed before pool
            self.default_texture = None;
            self.ctx.device.destroy_descriptor_pool(self.tex_descriptor_pool, None);

            self.ctx.device.destroy_command_pool(self.command_pool, None);
        }
    }
}

fn create_shadow_map(
    ctx: &VulkanContext,
    shadow_pipeline: &ShadowPipeline,
    depth_format: vk::Format,
) -> ShadowMap {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(depth_format)
        .extent(vk::Extent3D { width: SHADOW_MAP_SIZE, height: SHADOW_MAP_SIZE, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    let image = unsafe { ctx.device.create_image(&image_info, None).unwrap() };

    let mem_req = unsafe { ctx.device.get_image_memory_requirements(image) };
    let mem_type = ctx.memory_type_index(
        mem_req.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_req.size)
        .memory_type_index(mem_type);
    let memory = unsafe { ctx.device.allocate_memory(&alloc_info, None).unwrap() };
    unsafe { ctx.device.bind_image_memory(image, memory, 0).unwrap() };

    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(depth_format)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask:      vk::ImageAspectFlags::DEPTH,
            base_mip_level:   0,
            level_count:      1,
            base_array_layer: 0,
            layer_count:      1,
        });
    let view = unsafe { ctx.device.create_image_view(&view_info, None).unwrap() };

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
        .compare_enable(false)
        .max_lod(1.0);
    let sampler = unsafe { ctx.device.create_sampler(&sampler_info, None).unwrap() };

    let attachments = [view];
    let fb_info = vk::FramebufferCreateInfo::default()
        .render_pass(shadow_pipeline.render_pass)
        .attachments(&attachments)
        .width(SHADOW_MAP_SIZE)
        .height(SHADOW_MAP_SIZE)
        .layers(1);
    let framebuffer = unsafe { ctx.device.create_framebuffer(&fb_info, None).unwrap() };

    ShadowMap { image, memory, view, sampler, framebuffer }
}

fn create_framebuffers(
    ctx: &VulkanContext,
    swapchain: &Swapchain,
    pipeline: &Pipeline,
) -> Vec<vk::Framebuffer> {
    swapchain
        .image_views
        .iter()
        .map(|&resolve_view| {
            let attachments = [swapchain.msaa_color_view, swapchain.depth_image_view, resolve_view];
            let create_info = vk::FramebufferCreateInfo::default()
                .render_pass(pipeline.render_pass)
                .attachments(&attachments)
                .width(swapchain.extent.width)
                .height(swapchain.extent.height)
                .layers(1);
            unsafe { ctx.device.create_framebuffer(&create_info, None).unwrap() }
        })
        .collect()
}

fn create_command_pool(ctx: &VulkanContext) -> vk::CommandPool {
    let create_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(ctx.queue_families.graphics);
    unsafe { ctx.device.create_command_pool(&create_info, None).unwrap() }
}

fn create_command_buffers(
    ctx: &VulkanContext,
    pool: vk::CommandPool,
    count: usize,
) -> Vec<vk::CommandBuffer> {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(count as u32);
    unsafe { ctx.device.allocate_command_buffers(&alloc_info).unwrap() }
}

fn create_descriptor_pool(ctx: &VulkanContext, count: u32) -> vk::DescriptorPool {
    let pool_sizes = [
        vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER,         descriptor_count: count },
        vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: count },
        vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER,         descriptor_count: count },
    ];
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(count);
    unsafe { ctx.device.create_descriptor_pool(&create_info, None).unwrap() }
}

fn create_tex_descriptor_pool(ctx: &VulkanContext, max_textures: u32) -> vk::DescriptorPool {
    let pool_size = vk::DescriptorPoolSize {
        ty:               vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: max_textures,
    };
    let info = vk::DescriptorPoolCreateInfo::default()
        // FREE_DESCRIPTOR_SET_BIT lets GpuTexture free its set individually on drop
        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
        .max_sets(max_textures)
        .pool_sizes(std::slice::from_ref(&pool_size));
    unsafe { ctx.device.create_descriptor_pool(&info, None).unwrap() }
}

fn create_semaphore(ctx: &VulkanContext) -> vk::Semaphore {
    let info = vk::SemaphoreCreateInfo::default();
    unsafe { ctx.device.create_semaphore(&info, None).unwrap() }
}

fn create_fence(ctx: &VulkanContext, signaled: bool) -> vk::Fence {
    let flags = if signaled { vk::FenceCreateFlags::SIGNALED } else { vk::FenceCreateFlags::empty() };
    let info = vk::FenceCreateInfo::default().flags(flags);
    unsafe { ctx.device.create_fence(&info, None).unwrap() }
}
