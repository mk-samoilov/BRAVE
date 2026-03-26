use std::ffi::{CStr, CString};
use std::sync::Arc;

use ash::{khr, vk};
#[cfg(debug_assertions)]
use ash::ext;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

#[cfg(debug_assertions)]
const VALIDATION_LAYER: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") };

pub struct QueueFamilies {
    pub graphics: u32,
    pub present: u32,
}

pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface_loader: khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub queue_families: QueueFamilies,
    #[cfg(debug_assertions)]
    debug: Option<(ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}

impl VulkanContext {
    pub fn new(window: &Arc<winit::window::Window>) -> Self {
        let entry = unsafe { ash::Entry::load().expect("Failed to load Vulkan library") };

        let instance = Self::create_instance(&entry, window);
        let surface_loader = khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .expect("Failed to create surface")
        };

        #[cfg(debug_assertions)]
        let debug = Self::setup_debug_messenger(&entry, &instance);

        let physical_device = Self::pick_physical_device(&instance, &surface_loader, surface);
        let queue_families =
            Self::find_queue_families(&instance, physical_device, &surface_loader, surface);
        let device = Self::create_logical_device(&instance, physical_device, &queue_families);

        let graphics_queue =
            unsafe { device.get_device_queue(queue_families.graphics, 0) };
        let present_queue =
            unsafe { device.get_device_queue(queue_families.present, 0) };

        Self {
            entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            queue_families,
            #[cfg(debug_assertions)]
            debug,
        }
    }

    fn create_instance(entry: &ash::Entry, window: &Arc<winit::window::Window>) -> ash::Instance {
        let app_name = CString::new("BRAVE").unwrap();
        let engine_name = CString::new("BRAVE Engine").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_2);

        #[cfg(not(debug_assertions))]
        let extensions = ash_window::enumerate_required_extensions(
            window.display_handle().unwrap().as_raw(),
        )
        .unwrap()
        .to_vec();

        #[cfg(debug_assertions)]
        let extensions = {
            let mut v = ash_window::enumerate_required_extensions(
                window.display_handle().unwrap().as_raw(),
            )
            .unwrap()
            .to_vec();
            v.push(ext::debug_utils::NAME.as_ptr());
            v
        };

        let layers: Vec<*const i8>;
        #[cfg(debug_assertions)]
        {
            let available = unsafe { entry.enumerate_instance_layer_properties() }
                .unwrap_or_default()
                .iter()
                .any(|l| unsafe { CStr::from_ptr(l.layer_name.as_ptr()) } == VALIDATION_LAYER);
            if !available {
                log::warn!("VK_LAYER_KHRONOS_validation not found, validation layers disabled");
            }
            layers = if available { vec![VALIDATION_LAYER.as_ptr()] } else { vec![] };
        }
        #[cfg(not(debug_assertions))]
        {
            layers = vec![];
        }

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);

        unsafe { entry.create_instance(&create_info, None).expect("Failed to create instance") }
    }

    #[cfg(debug_assertions)]
    fn setup_debug_messenger(
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Option<(ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)> {
        let loader = ext::debug_utils::Instance::new(entry, instance);
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
            .pfn_user_callback(Some(debug_callback));

        let messenger = unsafe {
            loader
                .create_debug_utils_messenger(&create_info, None)
                .expect("Failed to create debug messenger")
        };

        Some((loader, messenger))
    }

    fn pick_physical_device(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> vk::PhysicalDevice {
        let devices = unsafe { instance.enumerate_physical_devices().unwrap() };
        assert!(!devices.is_empty(), "No Vulkan-capable GPU found");

        let device = devices.iter().find(|&&d| {
            let props = unsafe { instance.get_physical_device_properties(d) };
            props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                && Self::is_device_suitable(instance, d, surface_loader, surface)
        });

        let device = device
            .or_else(|| {
                devices
                    .iter()
                    .find(|&&d| Self::is_device_suitable(instance, d, surface_loader, surface))
            })
            .expect("No suitable GPU found");

        let props = unsafe { instance.get_physical_device_properties(*device) };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) };
        log::info!("GPU: {}", name.to_string_lossy());

        *device
    }

    fn is_device_suitable(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> bool {
        let families = Self::find_queue_families(instance, device, surface_loader, surface);
        let formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(device, surface)
                .unwrap_or_default()
        };
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(device, surface)
                .unwrap_or_default()
        };
        let extensions = unsafe {
            instance
                .enumerate_device_extension_properties(device)
                .unwrap_or_default()
        };

        let has_swapchain = extensions.iter().any(|e| {
            let name = unsafe { CStr::from_ptr(e.extension_name.as_ptr()) };
            name == khr::swapchain::NAME
        });

        !formats.is_empty() && !present_modes.is_empty() && has_swapchain
            && families.graphics < u32::MAX
            && families.present < u32::MAX
    }

    pub fn find_queue_families(
        instance: &ash::Instance,
        device: vk::PhysicalDevice,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> QueueFamilies {
        let props = unsafe { instance.get_physical_device_queue_family_properties(device) };
        let mut graphics = u32::MAX;
        let mut present = u32::MAX;

        for (i, family) in props.iter().enumerate() {
            let i = i as u32;
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics = i;
            }
            let present_support = unsafe {
                surface_loader
                    .get_physical_device_surface_support(device, i, surface)
                    .unwrap_or(false)
            };
            if present_support {
                present = i;
            }
            if graphics != u32::MAX && present != u32::MAX {
                break;
            }
        }

        QueueFamilies { graphics, present }
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        families: &QueueFamilies,
    ) -> ash::Device {
        let priority = [1.0f32];
        let mut unique_families = vec![families.graphics];
        if families.present != families.graphics {
            unique_families.push(families.present);
        }

        let queue_infos: Vec<vk::DeviceQueueCreateInfo> = unique_families
            .iter()
            .map(|&index| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(index)
                    .queue_priorities(&priority)
            })
            .collect();

        let extensions = [khr::swapchain::NAME.as_ptr()];
        let features = vk::PhysicalDeviceFeatures::default();

        let create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&extensions)
            .enabled_features(&features);

        unsafe {
            instance
                .create_device(physical_device, &create_info, None)
                .expect("Failed to create logical device")
        }
    }

    pub fn memory_type_index(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> u32 {
        let mem_props = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical_device)
        };
        for i in 0..mem_props.memory_type_count {
            if type_filter & (1 << i) != 0
                && mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(properties)
            {
                return i;
            }
        }
        panic!("Failed to find suitable memory type");
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            #[cfg(debug_assertions)]
            if let Some((ref loader, messenger)) = self.debug {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

#[cfg(debug_assertions)]
unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _type: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let msg = unsafe { CStr::from_ptr((*data).p_message) }.to_string_lossy();
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("[Vulkan] {}", msg);
    } else {
        log::warn!("[Vulkan] {}", msg);
    }
    vk::FALSE
}
