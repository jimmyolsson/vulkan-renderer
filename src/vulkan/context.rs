use ash::{ext::debug_utils, khr::surface, khr::swapchain, vk};

use anyhow::Result;

#[allow(unused)]
pub struct VulkanContext {
    pub instance: ash::Instance,
    pub device: ash::Device,
    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,

    device_properties: vk::PhysicalDeviceProperties,
    queue_family_properties: vk::QueueFamilyProperties,

    // Both for present and graphics
    pub queue: vk::Queue,
    pub queue_index: u32,

    queue_transfer: vk::Queue,
    queue_transfer_index: u32,

    pub surface_instance: surface::Instance,
    pub surface: vk::SurfaceKHR,

    pub swapchain_loader: swapchain::Device,
}

impl VulkanContext {
    pub fn new(
        display_handle: winit::raw_window_handle::RawDisplayHandle,
        window_handle: winit::raw_window_handle::RawWindowHandle,
    ) -> Result<Self> {
        let entry = ash::Entry::linked();

        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layer_names_raw: Vec<*const std::ffi::c_char> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
            .unwrap()
            .to_vec();
        extension_names.push(debug_utils::NAME.as_ptr());

        let app_name = c"Hello triangle";
        let engine_name = c"No engine";
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name)
            .application_version(0)
            .engine_name(engine_name)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_3);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layer_names_raw)
            .enabled_extension_names(&extension_names)
            .flags(vk::InstanceCreateFlags::default());

        let instance = unsafe {
            entry
                .create_instance(&create_info, None)
                .expect("Unable to create instance")
        };

        Self::setup_debug_callback(&entry, &instance);

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
                .expect("Unable to create surface")
        };

        let surface_instance = surface::Instance::new(&entry, &instance);

        // Physical device
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };

        // Finds a queue family on a physical device that supports both graphics commands
        // and presentation to the given surface.
        //
        // Note:
        // On some systems, graphics and presentation are supported by different
        // queue families:
        //
        //   graphics_queue  -> supports GRAPHICS
        //   present_queue   -> supports PRESENT (surface support)
        //   graphics_index != present_index
        //
        // This implementation intentionally selects only queue families that support
        // BOTH graphics and presentation, so a single queue can be used for rendering
        // and presenting.
        let (physical_device, queue_family_index, queue_family_properties, device_properties) =
            physical_devices
                .iter()
                .find_map(|pdevice| unsafe {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let surface_support =
                                surface::Instance::get_physical_device_surface_support(
                                    &surface_instance,
                                    *pdevice,
                                    index as u32,
                                    surface,
                                )
                                .unwrap_or(false);

                            // Should prob check for dynamic rendering support here..
                            if info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                && surface_support
                            {
                                let properies = instance.get_physical_device_properties(*pdevice);
                                Some((*pdevice, index as u32, *info, properies))
                            } else {
                                None
                            }
                        })
                })
                .expect("Unable to find suitable device");

        unsafe {
            println!(
                "Using physical device: {}",
                std::ffi::CStr::from_ptr(device_properties.device_name.as_ptr()).to_string_lossy()
            );
        }

        // Find transfer queue
        let (queue_transfer_index, _queue_transfer_properties) =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
                .iter()
                .enumerate()
                .find_map(|(index, properties)| {
                    (properties.queue_flags.contains(vk::QueueFlags::TRANSFER)
                        && !properties.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                    .then_some((index as u32, *properties))
                })
                .expect("Unable to find transfer queue");

        // Create device
        let queue_priorities = [1.0];
        let device_queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);

        let device_transfer_queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_transfer_index)
            .queue_priorities(&queue_priorities);

        let mut shader_draw_feature = vk::PhysicalDeviceShaderDrawParametersFeatures {
            shader_draw_parameters: vk::TRUE,
            ..Default::default()
        };

        let mut khr_dynamic_rendering = vk::PhysicalDeviceDynamicRenderingFeaturesKHR {
            dynamic_rendering: vk::TRUE,
            ..Default::default()
        };

        let mut khr_synchronization2 = vk::PhysicalDeviceSynchronization2FeaturesKHR {
            synchronization2: vk::TRUE,
            ..Default::default()
        };

        let mut ext_dynamic_state = vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT {
            extended_dynamic_state: vk::TRUE,
            ..Default::default()
        };

        let queue_create_infos = [device_queue_create_info, device_transfer_queue_create_info];

        let enabled_extension_names = [swapchain::NAME.as_ptr()];
        let device_features = vk::PhysicalDeviceFeatures::default().sampler_anisotropy(true);
        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&enabled_extension_names)
            .enabled_features(&device_features)
            .push_next(&mut shader_draw_feature)
            .push_next(&mut ext_dynamic_state)
            .push_next(&mut khr_dynamic_rendering)
            .push_next(&mut khr_synchronization2);

        let device = unsafe {
            instance
                .create_device(physical_device, &device_create_info, None)
                .expect("Failed to create device!")
        };

        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
        let queue_transfer = unsafe { device.get_device_queue(queue_transfer_index, 0) };

        let swapchain_loader = swapchain::Device::new(&instance, &device);

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        Self::setup_debug_callback(&entry, &instance);

        Ok(Self {
            instance,
            device,
            physical_device,
            device_properties,
            device_memory_properties,
            queue_family_properties,
            queue,
            queue_index: queue_family_index,
            queue_transfer,
            queue_transfer_index,
            surface,
            surface_instance,
            swapchain_loader,
        })
    }

    fn setup_debug_callback(entry: &ash::Entry, instance: &ash::Instance) {
        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let debug_utils_instance = debug_utils::Instance::new(&entry, &instance);
        unsafe {
            debug_utils_instance
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap()
        };
    }
}

// ** Utility functions lives here for now, TODO: wrap them correctly **

pub struct CreateImageResult {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
}

pub fn create_image(
    context: &VulkanContext,
    width: u32,
    height: u32,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> CreateImageResult {
    let create_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(tiling)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let image = unsafe { context.device.create_image(&create_info, None).unwrap() };
    let requirements = unsafe { context.device.get_image_memory_requirements(image) };
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(find_memory_type(
            context,
            requirements.memory_type_bits,
            properties,
        ));
    let memory = unsafe { context.device.allocate_memory(&alloc_info, None).unwrap() };
    unsafe { context.device.bind_image_memory(image, memory, 0).unwrap() };

    CreateImageResult { image, memory }
}

pub fn find_memory_type(
    context: &VulkanContext,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> u32 {
    let memory_count = context.device_memory_properties.memory_type_count;
    context.device_memory_properties.memory_types[..memory_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (type_filter & (1 << index)) != 0 && memory_type.property_flags.contains(properties)
        })
        .map(|(index, _)| index as u32)
        .expect("Unable to find suitable memory type!")
}

pub fn create_texture_image_view(
    context: &VulkanContext,
    image: vk::Image,
    format: vk::Format,
    aspect: vk::ImageAspectFlags,
) -> vk::ImageView {
    let create_info = vk::ImageViewCreateInfo::default()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format) //vk::Format::R8G8B8A8_SRGB)
        .components(vk::ComponentMapping {
            r: vk::ComponentSwizzle::IDENTITY,
            g: vk::ComponentSwizzle::IDENTITY,
            b: vk::ComponentSwizzle::IDENTITY,
            a: vk::ComponentSwizzle::IDENTITY,
        })
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: aspect, //vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image(image);

    unsafe {
        context
            .device
            .create_image_view(&create_info, None)
            .unwrap()
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { &*p_callback_data };

    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        std::borrow::Cow::from("")
    } else {
        unsafe { std::ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy() }
    };

    let message = if callback_data.p_message.is_null() {
        std::borrow::Cow::from("")
    } else {
        unsafe { std::ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy() }
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}
