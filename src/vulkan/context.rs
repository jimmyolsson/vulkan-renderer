use ash::{ext::debug_utils, khr::surface, khr::swapchain, vk};

use crate::Vertex;
use nalgebra_glm as glm;

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
    pub fn new(window: &sdl3::video::Window) -> anyhow::Result<Self> {
        let entry = ash::Entry::linked();

        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layer_names_raw: Vec<*const std::ffi::c_char> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let extensions = unsafe {
            entry
                .enumerate_instance_extension_properties(None)
                .expect("Failed to enumerate extensions")
        };

        println!("Supported instance extensions:");

        for ext in extensions {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            println!("\t{}", name.to_str().unwrap());
        }

        let sdl_extensions = window.vulkan_instance_extensions()?;

        let mut extension_strings: Vec<String> = sdl_extensions
            .into_iter()
            .map(|s| format!("{s}\0"))
            .collect();

        extension_strings.push("VK_EXT_debug_utils\0".to_string());

        let extension_ptrs: Vec<*const i8> = extension_strings
            .iter()
            .map(|s| s.as_ptr() as *const i8)
            .collect();

        println!("Requested extension: {:?}", ash::ext::debug_utils::NAME);
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
            .enabled_extension_names(&extension_ptrs)
            .flags(vk::InstanceCreateFlags::default());

        let instance = unsafe {
            entry
                .create_instance(&create_info, None)
                .expect("Unable to create instance")
        };

        Self::setup_debug_callback(&entry, &instance);

        // Create surface via SDL instead of ash-window
        let surface = unsafe {
            window
                .vulkan_create_surface(instance.handle())
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
        let device_features = vk::PhysicalDeviceFeatures::default()
            .sampler_anisotropy(true)
            .fill_mode_non_solid(true);

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
// TODO: Alignment..?
#[repr(C)]
pub struct UniformBufferObject {
    model: glm::Mat4,
    view: glm::Mat4,
    projection: glm::Mat4,
}
pub fn update_uniform_buffer(
    swapchain_extent: vk::Extent2D,
    uniforms: &AllocatedMappedBuffer,
    view: glm::Mat4,
) {
    use std::time::Instant;
    // Static start time (initialized once)
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

    let start_time = START.get_or_init(Instant::now);

    let current_time = Instant::now();
    let time: f32 = current_time.duration_since(*start_time).as_secs_f32();

    let model = glm::rotate(
        &glm::identity(),
        time * 90.0_f32.to_radians(),
        &glm::vec3(0.0, 0.0, 1.0),
    );
    // let view = glm::look_at(
    //     &glm::vec3(2.0, 2.0, 2.0),
    //     &glm::vec3(0.0, 0.0, 0.0),
    //     &glm::vec3(0.0, 0.0, 1.0),
    // );
    // Flip this?
    let mut projection = glm::perspective(
        swapchain_extent.width as f32 / swapchain_extent.height as f32,
        45.0_f32.to_radians(),
        0.1,
        10.0,
    );
    projection[(1, 1)] *= -1.0;
    let uniform_object = UniformBufferObject {
        model,
        view,
        projection,
    };
    unsafe {
        std::ptr::copy_nonoverlapping(
            &uniform_object,
            uniforms.data_ptr as *mut UniformBufferObject,
            1,
        )
    };
}

pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

pub struct AllocatedMappedBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub data_ptr: *mut std::ffi::c_void,
}

pub fn create_buffer(
    context: &VulkanContext,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> anyhow::Result<AllocatedBuffer> {
    let buffer_create_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe {
        context
            .device
            .create_buffer(&buffer_create_info, None)
            .unwrap()
    };
    let buffer_memory_req = unsafe { context.device.get_buffer_memory_requirements(buffer) };
    let memory_type_index =
        find_memory_type(&context, buffer_memory_req.memory_type_bits, properties);

    let memory_allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(buffer_memory_req.size)
        .memory_type_index(memory_type_index);

    let memory = unsafe {
        context
            .device
            .allocate_memory(&memory_allocate_info, None)
            .expect("Failed to allocate vertex buffer memory")
    };
    unsafe {
        context
            .device
            .bind_buffer_memory(buffer, memory, 0)
            .expect("Failed to bind buffer memory")
    }

    Ok(AllocatedBuffer { buffer, memory })
}

pub fn create_uniform_buffers<const N: usize>(
    context: &VulkanContext,
    size: vk::DeviceSize,
) -> [AllocatedMappedBuffer; N] {
    std::array::from_fn(|_| {
        let buffer = create_buffer(
            context,
            size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .unwrap();

        let data_ptr = unsafe {
            context
                .device
                .map_memory(buffer.memory, 0, size, vk::MemoryMapFlags::empty())
                .expect("Unable to map memory")
        };

        AllocatedMappedBuffer {
            buffer: buffer.buffer,
            memory: buffer.memory,
            data_ptr,
        }
    })
}

pub fn submit_copy_buffer_cmd(
    context: &VulkanContext,
    src: vk::Buffer,
    dst: vk::Buffer,
    size: vk::DeviceSize,
    command_pool: vk::CommandPool,
) {
    unsafe {
        immediate_submit(&context.device, command_pool, context.queue, |cmd| {
            let buffer_copy = vk::BufferCopy::default()
                .dst_offset(0)
                .src_offset(0)
                .size(size);
            context
                .device
                .cmd_copy_buffer(cmd, src, dst, &[buffer_copy]);
        })
    };
}

pub fn create_vertex_buffer(
    context: &VulkanContext,
    vertices: &Vec<Vertex>,
    command_pool: vk::CommandPool,
    staging_buffer: vk::Buffer,
    staging_buffer_memory: vk::DeviceMemory,
    size: vk::DeviceSize,
) -> AllocatedBuffer {
    let data = unsafe {
        context
            .device
            .map_memory(staging_buffer_memory, 0, size, vk::MemoryMapFlags::empty())
            .expect("Unable to map memory")
    };

    unsafe { std::ptr::copy_nonoverlapping(vertices.as_ptr(), data as *mut Vertex, vertices.len()) }

    unsafe { context.device.unmap_memory(staging_buffer_memory) }

    let vertex_buffer = create_buffer(
        &context,
        size,
        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .unwrap();

    submit_copy_buffer_cmd(
        context,
        staging_buffer,
        vertex_buffer.buffer,
        size,
        command_pool,
    );

    vertex_buffer
}

pub fn create_index_buffer(
    context: &VulkanContext,
    indexes: &Vec<u32>,
    command_pool: vk::CommandPool,
) -> AllocatedBuffer {
    let size = (indexes.len() * size_of::<u32>()) as u64;
    let staging_buffer = create_buffer(
        &context,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let data = unsafe {
        context
            .device
            .map_memory(staging_buffer.memory, 0, size, vk::MemoryMapFlags::empty())
            .expect("Unable to map memory")
    };

    unsafe { std::ptr::copy_nonoverlapping(indexes.as_ptr(), data as *mut u32, indexes.len()) }

    unsafe { context.device.unmap_memory(staging_buffer.memory) }

    let index_buffer = create_buffer(
        &context,
        size,
        vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .unwrap();

    submit_copy_buffer_cmd(
        context,
        staging_buffer.buffer,
        index_buffer.buffer,
        size,
        command_pool,
    );

    index_buffer
}

pub unsafe fn immediate_submit<F>(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    record: F,
) where
    F: FnOnce(vk::CommandBuffer),
{
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);

    let command_buffers = [unsafe {
        device
            .allocate_command_buffers(&alloc_info)
            .expect("Failed to allocate command buffer")[0]
    }];

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe {
        device
            .begin_command_buffer(command_buffers[0], &begin_info)
            .expect("Failed to begin command buffer")
    };

    record(command_buffers[0]);

    unsafe {
        device
            .end_command_buffer(command_buffers[0])
            .expect("Failed to end command buffer");
    }

    let submit_info = [vk::SubmitInfo::default().command_buffers(&command_buffers)];

    unsafe {
        device
            .queue_submit(queue, &submit_info, vk::Fence::null())
            .expect("Queue submit failed");

        device.queue_wait_idle(queue).expect("Queue wait failed");

        device.free_command_buffers(command_pool, &command_buffers);
    };
}

pub unsafe fn copy_buffer_to_img(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    buffer: vk::Buffer,
    image: vk::Image,
    width: u32,
    height: u32,
) {
    unsafe {
        immediate_submit(device, command_pool, queue, |cmd| {
            let regions = [vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })];

            device.cmd_copy_buffer_to_image(
                cmd,
                buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
        });
    }
}

pub fn transition_image_layout(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_access_mask: vk::AccessFlags2,
    dst_access_mask: vk::AccessFlags2,
    src_stage_mask: vk::PipelineStageFlags2,
    dst_stage_mask: vk::PipelineStageFlags2,
    image_aspect_flags: vk::ImageAspectFlags,
) {
    let barriers = [vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src_stage_mask)
        .src_access_mask(src_access_mask)
        .dst_stage_mask(dst_stage_mask)
        .dst_access_mask(dst_access_mask)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: image_aspect_flags,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })];

    let dependency_info = vk::DependencyInfo::default()
        .dependency_flags(vk::DependencyFlags::empty())
        .image_memory_barriers(&barriers);
    unsafe {
        device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
    };
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
