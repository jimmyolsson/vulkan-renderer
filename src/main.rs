use ash::{
    Entry,
    ext::debug_utils,
    khr::{surface, swapchain},
    vk::{
        self, ComponentMapping, DeviceQueueCreateInfo, ImageSubresourceRange,
        KHR_SYNCHRONIZATION2_NAME, PipelineRenderingCreateInfo, PipelineStageFlags2,
        ShaderStageFlags,
    },
};
use std::{borrow::Cow, default::Default, ffi, fs, os::raw::c_char};
use std::{ffi::CStr, u32};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowBuilder,
};

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        unsafe { ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy() }
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        unsafe { ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy() }
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create window
    let window_width = 1280;
    let window_height = 960;
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Hello vulkan")
        .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
        .build(&event_loop)
        .unwrap();

    let entry = Entry::linked();

    let layer_names = [c"VK_LAYER_KHRONOS_validation"];
    let layer_names_raw: Vec<*const c_char> = layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect();

    let mut extension_names =
        ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())
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

    let surface = unsafe {
        ash_window::create_surface(
            &entry,
            &instance,
            event_loop.display_handle()?.as_raw(),
            window.window_handle()?.as_raw(),
            None,
        )
        .expect("Unable to create surface")
    };

    let surface_instance = surface::Instance::new(&entry, &instance);
    // Debug
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
    let debug_messenger =
        unsafe { debug_utils_instance.create_debug_utils_messenger(&debug_info, None) };

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
    let selected_physical_device = physical_devices
        .iter()
        .find_map(|pdevice| unsafe {
            instance
                .get_physical_device_queue_family_properties(*pdevice)
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    let surface_support = surface::Instance::get_physical_device_surface_support(
                        &surface_instance,
                        *pdevice,
                        index as u32,
                        surface,
                    )
                    .unwrap_or(false);

                    // Should prob check for dynamic rendering support here..
                    if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) && surface_support {
                        let properies = instance.get_physical_device_properties(*pdevice);
                        Some((*pdevice, index, *info, properies))
                    } else {
                        None
                    }
                })
        })
        .expect("Unable to find suitable device");

    let name = unsafe {
        CStr::from_ptr(selected_physical_device.3.device_name.as_ptr()).to_string_lossy()
    };

    println!("Using physical device: {}", name);

    // Create device

    let queue_priorities = [1.0];
    let device_queue_create_info = DeviceQueueCreateInfo::default()
        .queue_family_index(selected_physical_device.1 as u32)
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

    let queue_create_infos = [device_queue_create_info];

    let enabled_extension_names = [swapchain::NAME.as_ptr()];

    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&enabled_extension_names)
        .push_next(&mut shader_draw_feature)
        .push_next(&mut ext_dynamic_state)
        .push_next(&mut khr_dynamic_rendering)
        .push_next(&mut khr_synchronization2);

    let device = unsafe {
        instance
            .create_device(selected_physical_device.0, &device_create_info, None)
            .expect("Failed to create device!")
    };

    let graphics_and_present_queue =
        unsafe { device.get_device_queue(selected_physical_device.1 as u32, 0) };

    // Create Swapchain
    let surface_capabilities = unsafe {
        surface_instance
            .get_physical_device_surface_capabilities(selected_physical_device.0, surface)
            .unwrap()
    };

    let surface_format = unsafe {
        surface_instance
            .get_physical_device_surface_formats(selected_physical_device.0, surface)
            .unwrap()[0]
    };

    let surface_transform = surface_capabilities.current_transform;
    let surface_resolution = match surface_capabilities.current_extent.width {
        u32::MAX => vk::Extent2D {
            width: window_width,
            height: window_height,
        },
        _ => surface_capabilities.current_extent,
    };
    let present_mode = vk::PresentModeKHR::MAILBOX;

    let desired_image_count = surface_capabilities.min_image_count + 1; // ?
    let create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(desired_image_count)
        .image_color_space(surface_format.color_space)
        .image_format(surface_format.format)
        .image_extent(surface_resolution) // Res of swapchain images
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(surface_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .image_array_layers(1);

    let swapchain_loader = swapchain::Device::new(&instance, &device);
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&create_info, None)
            .expect("Unable to create swapchain")
    };
    let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

    let present_image_views: Vec<vk::ImageView> = swapchain_images
        .iter()
        .map(|&image| unsafe {
            {
                let create_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .components(ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);

                device.create_image_view(&create_info, None).unwrap()
            }
        })
        .collect();

    // Load shaders
    let shader_code = read_spv("shaders\\slang.spv");
    let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);
    let shader_module = unsafe {
        device
            .create_shader_module(&shader_create_info, None)
            .expect("Unable to create shader module")
    };

    let vert_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(ShaderStageFlags::VERTEX)
        .name(c"vertMain")
        .module(shader_module);

    let frag_stage_create_info = vk::PipelineShaderStageCreateInfo::default()
        .stage(ShaderStageFlags::FRAGMENT)
        .name(c"fragMain")
        .module(shader_module);

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state_create_info =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default();

    let input_assembly_state_create_info = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: surface_resolution.width as f32,
        height: surface_resolution.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [surface_resolution.into()];
    let viewport_state_info = vk::PipelineViewportStateCreateInfo::default()
        .scissors(&scissors)
        .viewports(&viewports);

    let rasterizer_create_info = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false)
        .depth_bias_slope_factor(1.0)
        .line_width(1.0);

    let multisampling_create_info = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .sample_shading_enable(false);

    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(false)
        .color_write_mask(vk::ColorComponentFlags::R | vk::ColorComponentFlags::G)];

    let color_blend_create_info = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachment);

    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default();
    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .expect("Unable to create pipeline layout")
    };

    let color_formats = [surface_format.format];
    // let color_attachment_format =
    let mut pipeline_rendering_create_info =
        PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);

    let shader_stages = [vert_stage_create_info, frag_stage_create_info];
    let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
        .push_next(&mut pipeline_rendering_create_info) // 🔴 THIS IS REQUIRED
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input_info)
        .input_assembly_state(&input_assembly_state_create_info)
        .viewport_state(&viewport_state_info)
        .rasterization_state(&rasterizer_create_info)
        .multisample_state(&multisampling_create_info)
        .color_blend_state(&color_blend_create_info)
        .dynamic_state(&dynamic_state_create_info)
        .render_pass(vk::RenderPass::null()) // dynamic rendering
        .base_pipeline_handle(vk::Pipeline::null())
        .base_pipeline_index(-1)
        .layout(pipeline_layout);

    let graphics_pipelines = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
            .expect("Unable to create graphics pipeline")
    };

    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

    let command_pool = unsafe {
        device
            .create_command_pool(&command_pool_create_info, None)
            .expect("Unable to create command pool")
    };

    let allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);

    let command_buffers = unsafe {
        device
            .allocate_command_buffers(&allocate_info)
            .expect("Unable to allocate command buffers")
    };

    // let command_buffer = command_buffers[0];
    // let command_buffer_begin_info = vk::CommandBufferBeginInfo::default();

    // unsafe { device.begin_command_buffer(command_buffer, &command_buffer_begin_info)? };

    let semaphore_create_info = vk::SemaphoreCreateInfo::default();
    let present_completed_semaphore =
        unsafe { device.create_semaphore(&semaphore_create_info, None)? };
    let render_finished_semaphore =
        unsafe { device.create_semaphore(&semaphore_create_info, None)? };

    let fence_draw_create_info =
        vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    let fence_draw = unsafe { device.create_fence(&fence_draw_create_info, None)? };

    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match event {
            Event::WindowEvent { window_id, event } => match event {
                WindowEvent::RedrawRequested => {
                    unsafe {
                        device
                            .wait_for_fences(&[fence_draw], true, u64::MAX)
                            .unwrap()
                    };

                    let next_image = unsafe {
                        swapchain_loader
                            .acquire_next_image(
                                swapchain,
                                u64::MAX,
                                present_completed_semaphore,
                                vk::Fence::null(),
                            )
                            .expect("Failed to aquire next image")
                    };

                    let image_index = next_image.0 as usize;
                    let image = swapchain_images[image_index];
                    let image_view = present_image_views[image_index];
                    let pipeline = graphics_pipelines[0];
                    let command_buffer = command_buffers[0];

                    record_command_buffer(
                        &device,
                        image,
                        command_buffer,
                        surface_resolution,
                        image_view,
                        pipeline,
                        surface_resolution,
                    );

                    unsafe { device.reset_fences(&[fence_draw]).unwrap() };

                    let wait_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                    let wait_sem = [present_completed_semaphore];
                    let semap = [render_finished_semaphore];

                    let submit_info = vk::SubmitInfo::default()
                        .wait_semaphores(&wait_sem)
                        .wait_dst_stage_mask(&wait_mask)
                        .command_buffers(&command_buffers)
                        .signal_semaphores(&semap);

                    unsafe {
                        device
                            .queue_submit(graphics_and_present_queue, &[submit_info], fence_draw)
                            .unwrap()
                    };

                    let swapchains = [swapchain];
                    let image_indicies = [image_index as u32];
                    let present_info_khr = vk::PresentInfoKHR::default()
                        .wait_semaphores(&semap)
                        .swapchains(&swapchains)
                        .image_indices(&image_indicies);

                    unsafe {
                        swapchain_loader
                            .queue_present(graphics_and_present_queue, &present_info_khr)
                            .unwrap()
                    };
                }
                WindowEvent::CloseRequested => std::process::exit(0),
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}

fn record_command_buffer(
    device: &ash::Device,
    image: vk::Image,
    command_buffer: vk::CommandBuffer,
    swapchain_extent: vk::Extent2D,
    image_view: vk::ImageView,
    pipeline: vk::Pipeline,
    resolution: vk::Extent2D,
) {
    let command_buffer_begin_info = vk::CommandBufferBeginInfo::default();
    unsafe {
        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .unwrap()
    };

    transition_image_layout(
        device,
        command_buffer,
        image,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::AccessFlags2::empty(),
        vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
    );

    let clear_value = vk::ClearValue {
        color: vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        },
    };

    let attachment_info = vk::RenderingAttachmentInfo::default()
        .image_view(image_view)
        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .clear_value(clear_value);

    let color_attachments = [attachment_info];
    let rendering_info = vk::RenderingInfo::default()
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: swapchain_extent,
        })
        .color_attachments(&color_attachments)
        .layer_count(1);

    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: resolution.width as f32,
        height: resolution.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [resolution.into()];
    unsafe {
        device.cmd_begin_rendering(command_buffer, &rendering_info);
        device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);

        device.cmd_set_viewport(command_buffer, 0, &viewports);
        device.cmd_set_scissor(command_buffer, 0, &scissors);

        device.cmd_draw(command_buffer, 3, 1, 0, 0);

        device.cmd_end_rendering(command_buffer);
    }

    transition_image_layout(
        device,
        command_buffer,
        image,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
        vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        vk::AccessFlags2::empty(),
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
    );

    unsafe { device.end_command_buffer(command_buffer).unwrap() };
}

fn transition_image_layout(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_access_mask: vk::AccessFlags2,
    dst_access_mask: vk::AccessFlags2,
    src_stage_mask: vk::PipelineStageFlags2,
    dst_stage_mask: vk::PipelineStageFlags2,
) {
    let barrier = vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src_stage_mask)
        .src_access_mask(src_access_mask)
        .dst_stage_mask(dst_stage_mask)
        .dst_access_mask(dst_access_mask)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

    let barrier_a = [barrier];
    let dependency_info = vk::DependencyInfo::default()
        .dependency_flags(vk::DependencyFlags::empty())
        .image_memory_barriers(&barrier_a);
    unsafe {
        device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
    };
}
use std::io::Read;

fn read_spv(path: &str) -> Vec<u32> {
    println!("cwd = {:?}", std::env::current_dir().unwrap());
    let mut file = fs::File::open(path).expect("Failed to open SPIR-V file");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read SPIR-V file");

    // Convert &[u8] → &[u32]
    let (_, words, _) = unsafe { bytes.align_to::<u32>() };
    words.to_vec()
}
