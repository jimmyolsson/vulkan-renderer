use ash::vk::{self};
use winit::event::ElementState;

mod renderer;

mod vulkan;
use vulkan::context;

mod vertex;
use vertex::Vertex;

use std::default::Default;
use std::u32;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowBuilder,
};

use nalgebra_glm as glm;

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

    let mut renderer = renderer::Renderer::new(&vulkan_context)?;

    // let mut swapchain =
    //     vulkan::swapchain::Swapchain::new(&vulkan_context, window_height, window_width, None)?;

    let vertices = vec![
        Vertex {
            pos: glm::vec3(-0.5, -0.5, 0.0),
            color: glm::vec3(1.0, 0.0, 0.0),
            tex_coord: glm::vec2(1.0, 0.0),
        },
        Vertex {
            pos: glm::vec3(0.5, -0.5, 0.0),
            color: glm::vec3(0.0, 1.0, 0.0),
            tex_coord: glm::vec2(0.0, 0.0),
        },
        Vertex {
            pos: glm::vec3(0.5, 0.5, 0.0),
            color: glm::vec3(0.0, 0.0, 1.0),
            tex_coord: glm::vec2(0.0, 1.0),
        },
        Vertex {
            pos: glm::vec3(-0.5, 0.5, 0.0),
            color: glm::vec3(1.0, 1.0, 1.0),
            tex_coord: glm::vec2(1.0, 1.0),
        },
        // Second
        Vertex {
            pos: glm::vec3(-0.5, -0.5, -0.5),
            color: glm::vec3(1.0, 0.0, 0.0),
            tex_coord: glm::vec2(1.0, 0.0),
        },
        Vertex {
            pos: glm::vec3(0.5, -0.5, -0.5),
            color: glm::vec3(0.0, 1.0, 0.0),
            tex_coord: glm::vec2(0.0, 0.0),
        },
        Vertex {
            pos: glm::vec3(0.5, 0.5, -0.5),
            color: glm::vec3(0.0, 0.0, 1.0),
            tex_coord: glm::vec2(0.0, 1.0),
        },
        Vertex {
            pos: glm::vec3(-0.5, 0.5, -0.5),
            color: glm::vec3(1.0, 1.0, 1.0),
            tex_coord: glm::vec2(1.0, 1.0),
        },
    ];

    let indicies = vec![0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4];

    // Vertex buffers
    let size = (vertices.len() * size_of::<Vertex>()) as u64;
    let staging_buffer = context::create_buffer(
        &vulkan_context,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let vertex_buffer = context::create_vertex_buffer(
        &vulkan_context,
        &vertices,
        renderer.command_pool,
        staging_buffer.buffer,
        staging_buffer.memory,
        size,
    );
    let index_buffer =
        context::create_index_buffer(&vulkan_context, &indicies, renderer.command_pool);

    let mut frame_index = 0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    device_id: _,
                    event,
                    is_synthetic: _,
                } => {
                    if event.state == ElementState::Pressed {
                        match event.logical_key {
                            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                                std::process::exit(0);
                            }
                            _ => {}
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    renderer.draw_frame(
                        &vulkan_context,
                        frame_index,
                        |command_buffer,
                         image,
                         image_depth,
                         pipelines,
                         swapchain_extent,
                         image_view,
                         image_view_depth,
                         resolution| {
                            let command_buffer_begin_info = vk::CommandBufferBeginInfo::default();
                            unsafe {
                                vulkan_context
                                    .device
                                    .begin_command_buffer(
                                        command_buffer,
                                        &command_buffer_begin_info,
                                    )
                                    .unwrap()
                            };

                            context::transition_image_layout(
                                &vulkan_context.device,
                                command_buffer,
                                image,
                                vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                vk::AccessFlags2::empty(),
                                vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                                vk::ImageAspectFlags::COLOR,
                            );

                            // Depth
                            context::transition_image_layout(
                                &vulkan_context.device,
                                command_buffer,
                                image_depth,
                                vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                                vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                                    | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
                                vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                                    | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
                                vk::ImageAspectFlags::DEPTH,
                            );

                            let clear_value = vk::ClearValue {
                                color: vk::ClearColorValue {
                                    float32: [0.0, 0.0, 0.0, 1.0],
                                },
                            };
                            let clear_value_depth = vk::ClearValue {
                                depth_stencil: vk::ClearDepthStencilValue::default()
                                    .depth(1.0)
                                    .stencil(0),
                            };

                            let attachment_infos_color = [vk::RenderingAttachmentInfo::default()
                                .image_view(image_view)
                                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                .load_op(vk::AttachmentLoadOp::CLEAR)
                                .store_op(vk::AttachmentStoreOp::STORE)
                                .clear_value(clear_value)];

                            let attachment_info_depth = vk::RenderingAttachmentInfo::default()
                                .image_view(image_view_depth)
                                .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                                .load_op(vk::AttachmentLoadOp::CLEAR)
                                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                                .clear_value(clear_value_depth);

                            let rendering_info = vk::RenderingInfo::default()
                                .render_area(vk::Rect2D {
                                    offset: vk::Offset2D { x: 0, y: 0 },
                                    extent: swapchain_extent,
                                })
                                .color_attachments(&attachment_infos_color)
                                .depth_attachment(&attachment_info_depth)
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
                                vulkan_context
                                    .device
                                    .cmd_begin_rendering(command_buffer, &rendering_info);

                                vulkan_context.device.cmd_bind_pipeline(
                                    command_buffer,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    pipelines.texture.pipeline,
                                );

                                let buffer = [vertex_buffer.buffer];
                                let offsets = [0 as u64];
                                // Control
                                vulkan_context.device.cmd_bind_vertex_buffers(
                                    command_buffer,
                                    0 as u32,
                                    &buffer,
                                    &offsets,
                                );
                                vulkan_context.device.cmd_bind_index_buffer(
                                    command_buffer,
                                    index_buffer.buffer,
                                    0,
                                    vk::IndexType::UINT32,
                                );

                                vulkan_context.device.cmd_bind_descriptor_sets(
                                    command_buffer,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    pipelines.texture.layout,
                                    0,
                                    &pipelines.texture.descriptor_sets,
                                    &[],
                                );
                                vulkan_context.device.cmd_set_viewport(
                                    command_buffer,
                                    0,
                                    &viewports,
                                );
                                vulkan_context
                                    .device
                                    .cmd_set_scissor(command_buffer, 0, &scissors);

                                vulkan_context.device.cmd_draw_indexed(
                                    command_buffer,
                                    indicies.iter().count() as u32,
                                    1,
                                    0,
                                    0,
                                    0,
                                );

                                vulkan_context.device.cmd_end_rendering(command_buffer);
                            }

                            context::transition_image_layout(
                                &vulkan_context.device,
                                command_buffer,
                                image,
                                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                                vk::ImageLayout::PRESENT_SRC_KHR,
                                vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
                                vk::AccessFlags2::empty(),
                                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                                vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                                vk::ImageAspectFlags::COLOR,
                            );

                            unsafe {
                                vulkan_context
                                    .device
                                    .end_command_buffer(command_buffer)
                                    .unwrap()
                            };
                        },
                    );

                    // // Request the next frame (this is the "loop")
                    frame_index = (frame_index + 1) % frames_in_flight;
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => std::process::exit(0),
                WindowEvent::Resized(size) => {
                    renderer.handle_resize(&vulkan_context, size.width, size.height);
                }
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}
