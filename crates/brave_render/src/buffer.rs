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
