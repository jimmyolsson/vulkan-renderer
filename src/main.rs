use ash::nv::memory_decompression::Device;
use ash::vk::{
    self, BufferUsageFlags, Format, ImageSubresourceRange, PipelineRenderingCreateInfo,
    ShaderStageFlags,
};
mod vulkan;

use std::u32;
use std::{default::Default, fs};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowBuilder,
};

struct SyncObjects {
    present_complete_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
}

impl SyncObjects {
    fn new(
        device: &ash::Device,
        swapchain_images_count: usize,
        max_frames_in_flight_count: usize,
    ) -> Self {
        let mut present_semaphores = Vec::with_capacity(swapchain_images_count);
        let mut render_semaphores = Vec::with_capacity(max_frames_in_flight_count);
        let mut in_flight_fences = Vec::with_capacity(max_frames_in_flight_count);

        for _ in 0..swapchain_images_count {
            render_semaphores.push(unsafe {
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
            });
            present_semaphores.push(unsafe {
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
            });
        }

        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        for _ in 0..max_frames_in_flight_count {
            in_flight_fences
                .push(unsafe { device.create_fence(&fence_create_info, None).unwrap() });
        }

        return SyncObjects {
            present_complete_semaphores: present_semaphores,
            render_finished_semaphores: render_semaphores,

            in_flight_fences: in_flight_fences,
        };
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { std::env::set_var("RUST_BACKTRACE", "1") };

    let window_width = 1280;
    let window_height = 960;
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Hello vulkan")
        .with_inner_size(winit::dpi::LogicalSize::new(window_width, window_height))
        .build(&event_loop)
        .unwrap();

    let frames_in_flight: usize = 2;

    let vulkan_context = vulkan::context::VulkanContext::new(
        event_loop.display_handle()?.as_raw(),
        window.window_handle()?.as_raw(),
    )?;

    struct Vertex {
        pos: glm::Vec2,
        color: glm::Vec3,
    }
    println!("size of {}", std::mem::size_of::<Vertex>() as u32);
    let vertices = vec![
        Vertex {
            pos: glm::Vec2::new(0.0, -0.5),
            color: glm::Vec3::new(1.0, 0.0, 0.0),
        },
        Vertex {
            pos: glm::Vec2::new(0.5, 0.5),
            color: glm::Vec3::new(0.0, 1.0, 0.0),
        },
        Vertex {
            pos: glm::Vec2::new(-0.5, 0.5),
            color: glm::Vec3::new(0.0, 0.0, 1.0),
        },
    ];

    let vertex_input_desc = vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(size_of::<Vertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX);

    let vertex_attr_desc = [
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, pos) as u32),
        vk::VertexInputAttributeDescription::default()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(std::mem::offset_of!(Vertex, color) as u32),
    ];

    // Load shaders
    let shader_code = read_spv("shaders\\slang.spv");
    let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);
    let shader_module = unsafe {
        vulkan_context
            .device
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

    let vertex_binding_description = [vertex_input_desc];
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_binding_description)
        .vertex_attribute_descriptions(&vertex_attr_desc);

    let input_assembly_state_create_info = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: window_width as f32,
        height: window_height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    }];

    let res = ash::vk::Extent2D {
        width: 1920,
        height: 1080,
    }
    .into();

    let scissors = [res];
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
        vulkan_context
            .device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .expect("Unable to create pipeline layout")
    };

    let color_formats = [Format::B8G8R8A8_UNORM];
    let mut pipeline_rendering_create_info =
        PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);

    let shader_stages = [vert_stage_create_info, frag_stage_create_info];
    let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
        .push_next(&mut pipeline_rendering_create_info)
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
        vulkan_context
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
            .expect("Unable to create graphics pipeline")
    };

    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(vulkan_context.queue_index);

    let command_pool = unsafe {
        vulkan_context
            .device
            .create_command_pool(&command_pool_create_info, None)
            .expect("Unable to create command pool")
    };

    let allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(frames_in_flight as u32);

    // Vertex buffers
    let buffer_size = 3 * size_of::<Vertex>() as u64;

    let staging_buffer = create_buffer(
        &vulkan_context,
        buffer_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let vertex_buffer = create_buffer(
        &vulkan_context,
        buffer_size,
        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .unwrap();

    let data = unsafe {
        vulkan_context
            .device
            .map_memory(
                staging_buffer.memory,
                0,
                buffer_size,
                vk::MemoryMapFlags::empty(),
            )
            .expect("Unable to map memory")
    };

    unsafe { std::ptr::copy_nonoverlapping(vertices.as_ptr(), data as *mut Vertex, vertices.len()) }

    unsafe { vulkan_context.device.unmap_memory(staging_buffer.memory) }

    print!("AJSLDKHA");
    copy_buffer(
        &vulkan_context,
        staging_buffer.buffer,
        vertex_buffer.buffer,
        buffer_size,
        command_pool,
    );

    let command_buffers = unsafe {
        vulkan_context
            .device
            .allocate_command_buffers(&allocate_info)
            .expect("Unable to allocate command buffers")
    };

    let sync_objects = SyncObjects::new(&vulkan_context.device, 3, frames_in_flight);

    let mut swapchain =
        vulkan::swapchain::Swapchain::new(&vulkan_context, window_height, window_width, None)?;
    let mut frame_index = 0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::RedrawRequested => {
                    unsafe {
                        vulkan_context
                            .device
                            .wait_for_fences(
                                &[sync_objects.in_flight_fences[frame_index]],
                                true,
                                u64::MAX,
                            )
                            .unwrap()
                    };

                    let acquire_image_result = unsafe {
                        vulkan_context.swapchain_loader.acquire_next_image(
                            swapchain.handle,
                            u64::MAX,
                            sync_objects.present_complete_semaphores[frame_index],
                            vk::Fence::null(),
                        )
                    };
                    if let Err(vk::Result::ERROR_OUT_OF_DATE_KHR) = acquire_image_result {
                        panic!("ERROR_OUT_OF_DATE_KHR... should probably fix resizing/minimization correctly now..");
                        // swapchain.recreate(&vulkan_context, window_width, window_height);
                    }

                    let (next_image_index, _) = acquire_image_result.unwrap();

                    let image_index = next_image_index as usize;
                    let image = swapchain.images[image_index];
                    let image_view = swapchain.image_views[image_index];
                    let pipeline = graphics_pipelines[0];

                    record_command_buffer(
                        &vulkan_context.device,
                        image,
                        &command_buffers,
                        frame_index,
                        swapchain.surface_resolution,
                        image_view,
                        pipeline,
                        swapchain.surface_resolution,
                        vertex_buffer.buffer
                    );

                    unsafe {
                        vulkan_context
                            .device
                            .reset_fences(&[sync_objects.in_flight_fences[frame_index]])
                            .unwrap()
                    };

                    let wait_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                    let wait_sem = [sync_objects.present_complete_semaphores[frame_index]];
                    let semap = [sync_objects.render_finished_semaphores[frame_index]];
                    let command_buffer = [command_buffers[frame_index]];

                    let submit_info = vk::SubmitInfo::default()
                        .wait_semaphores(&wait_sem)
                        .wait_dst_stage_mask(&wait_mask)
                        .command_buffers(&command_buffer)
                        .signal_semaphores(&semap);

                    unsafe {
                        vulkan_context
                            .device
                            .queue_submit(
                                vulkan_context.queue,
                                &[submit_info],
                                sync_objects.in_flight_fences[frame_index],
                            )
                            .unwrap()
                    };

                    let swapchains = [swapchain.handle];
                    let image_indicies = [image_index as u32];
                    let present_info_khr = vk::PresentInfoKHR::default()
                        .wait_semaphores(&semap)
                        .swapchains(&swapchains)
                        .image_indices(&image_indicies);

                    unsafe {
                        vulkan_context
                            .swapchain_loader
                            .queue_present(vulkan_context.queue, &present_info_khr)
                            .unwrap()
                    };

                    // Request the next frame (this is the "loop")
                    frame_index = (frame_index + 1) % frames_in_flight;
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => std::process::exit(0),
                WindowEvent::Resized(size) => {
                    swapchain.recreate(&vulkan_context, size.width, size.height);
                }
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}

fn copy_buffer(
    context: &VulkanContext,
    src: vk::Buffer,
    dst: vk::Buffer,
    size: vk::DeviceSize,
    command_pool: vk::CommandPool,
) {
    let info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .command_buffer_count(1)
        .level(vk::CommandBufferLevel::PRIMARY);

    let command_copy_buffers = unsafe {
        context
            .device
            .allocate_command_buffers(&info)
            .expect("Unable to allocate command buffers")
    };

    let begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
        context
            .device
            .begin_command_buffer(command_copy_buffers[0], &begin_info)
            .unwrap()
    };

    let buffer_copy = vk::BufferCopy::default()
        .dst_offset(0)
        .src_offset(0)
        .size(size);
    unsafe {
        context
            .device
            .cmd_copy_buffer(command_copy_buffers[0], src, dst, &[buffer_copy]);
    }

    unsafe {
        context
            .device
            .end_command_buffer(command_copy_buffers[0])
            .unwrap()
    }

    let submit_info = vk::SubmitInfo::default().command_buffers(&command_copy_buffers);
    unsafe {
        context
            .device
            .queue_submit(context.queue, &[submit_info], vk::Fence::null())
            .unwrap();
        context.device.queue_wait_idle(context.queue).unwrap();
    }
}

struct CreateBufferResult {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}
fn create_buffer(
    context: &VulkanContext,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> anyhow::Result<CreateBufferResult> {
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

    Ok(CreateBufferResult { buffer, memory })
}

fn find_memory_type(
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

fn record_command_buffer(
    device: &ash::Device,
    image: vk::Image,
    command_buffers: &[vk::CommandBuffer],
    frame_index: usize,
    swapchain_extent: vk::Extent2D,
    image_view: vk::ImageView,
    pipeline: vk::Pipeline,
    resolution: vk::Extent2D,
    vertex_buffer: vk::Buffer,
) {
    let command_buffer = command_buffers[frame_index];
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

        let buffers = [vertex_buffer];
        let offsets = [0 as u64];

        device.cmd_bind_vertex_buffers(command_buffer, 0 as u32, &buffers, &offsets);

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

use crate::vulkan::context::VulkanContext;

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
