use ash::vk::{self};

mod camera;
mod renderer;
mod vertex;
mod vulkan;

use camera::Camera;
use vertex::Vertex;
use vulkan::context;

use std::u32;
use std::{default::Default, time::Instant};

use log::{error, info, trace, warn};

use nalgebra_glm as glm;
use std::io::Write;

const WIDTH: u32 = 10;
const HEIGHT: u32 = 10;
const TOTAL_BLOCKS_IN_CHUNK: u32 = WIDTH * WIDTH * HEIGHT;

struct Chunk {
    occupancy: [bool; WIDTH as usize * HEIGHT as usize],
}
fn log_step(start: std::time::Instant, last: &mut std::time::Instant, label: &str) {
    let now = std::time::Instant::now();
    trace!(
        "{}: +{}ms (total {}ms)",
        label,
        now.duration_since(*last).as_millis(),
        now.duration_since(start).as_millis()
    );
    *last = now;
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    pretty_env_logger::formatted_timed_builder()
        .format(|buf, record| {
            let level_style = buf.default_level_style(record.level());
            writeln!(
                buf,
                "[{}][{}] - {}",
                buf.timestamp(),
                level_style.value(record.level()),
                record.args()
            )
        })
        .filter_level(log::LevelFilter::Trace) // default level
        .parse_default_env() // override with RUST_LOG if set
        .init();

    info!("Starting!");
    let program_start_time = std::time::Instant::now();
    let mut last_time = program_start_time;

    let window_width = 1280;
    let window_height = 960;

    let sdl = sdl3::init()?;

    log_step(program_start_time, &mut last_time, "SDL3 intialized");

    let video = sdl.video()?;
    let window = video
        .window("Hello vulkan", window_width, window_height)
        .vulkan()
        .position_centered()
        .resizable()
        .build()?;

    sdl.mouse().set_relative_mouse_mode(&window, true);

    let frames_in_flight: usize = 2;

    log_step(program_start_time, &mut last_time, "Window created");
    let vulkan_context = vulkan::context::VulkanContext::new(&window)?;
    log_step(program_start_time, &mut last_time, "Vulkan initialized");

    let mut renderer = renderer::Renderer::new(&vulkan_context)?;
    log_step(program_start_time, &mut last_time, "Renderer initialized");

    let chunk = Chunk {
        occupancy: [true; WIDTH as usize * HEIGHT as usize],
    };

    let vertices = generate_chunk_mesh(&chunk);
    trace!(
        "Size of chunk: {} bytes",
        vertices.capacity() * std::mem::size_of::<Vertex>()
    );

    // Vertex buffers
    let staging_buffer2 = context::create_buffer(
        &vulkan_context,
        (vertices.len() * size_of::<Vertex>()) as u64,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let vertex_buffer2 = context::create_vertex_buffer(
        &vulkan_context,
        &vertices,
        renderer.command_pool,
        staging_buffer2.buffer,
        staging_buffer2.memory,
        (vertices.len() * size_of::<Vertex>()) as u64,
    );

    let mut wireframe = false;

    let mut camera = Camera::new(glm::vec3(0.0, 0.0, 15.0));
    let mut frame_index = 0;

    let mut running = true;
    let mut event_pump = sdl.event_pump()?;

    // Before the loop
    let mut last_frame = std::time::Instant::now();

    log_step(program_start_time, &mut last_time, "Vertex buffers created");
    while running {
        let now = std::time::Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32();
        last_frame = now;

        use sdl3::event::Event;
        use sdl3::keyboard::Keycode;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => running = false,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    repeat: false,
                    ..
                } => running = false,
                Event::KeyDown {
                    keycode: Some(Keycode::_1),
                    repeat: false,
                    ..
                } => wireframe = !wireframe,
                Event::MouseMotion { xrel, yrel, .. } => {
                    camera.process_mouse(xrel as f32, yrel as f32, dt);
                }
                Event::Window { win_event, .. } => {
                    if let sdl3::event::WindowEvent::Resized(width, height) = win_event {
                        renderer.handle_resize(&vulkan_context, width as u32, height as u32);
                    }
                }
                _ => {}
            }
        }

        camera.process_keyboard(&event_pump.keyboard_state(), dt);

        renderer.draw_frame(
            camera.view_matrix(),
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
                        .begin_command_buffer(command_buffer, &command_buffer_begin_info)
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
                    depth_stencil: vk::ClearDepthStencilValue::default().depth(1.0).stencil(0),
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
                let pipeline_to_use = if wireframe {
                    pipelines.texture_wireframe
                } else {
                    pipelines.texture
                };

                unsafe {
                    vulkan_context
                        .device
                        .cmd_begin_rendering(command_buffer, &rendering_info);

                    vulkan_context.device.cmd_bind_pipeline(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_to_use.pipeline,
                    );

                    let buffer2 = [vertex_buffer2.buffer];
                    let offsets = [0 as u64];
                    vulkan_context.device.cmd_bind_vertex_buffers(
                        command_buffer,
                        0 as u32,
                        &buffer2,
                        &offsets,
                    );

                    vulkan_context.device.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_to_use.layout,
                        0,
                        &[pipeline_to_use.descriptor_sets[frame_index]],
                        &[],
                    );
                    vulkan_context
                        .device
                        .cmd_set_viewport(command_buffer, 0, &viewports);
                    vulkan_context
                        .device
                        .cmd_set_scissor(command_buffer, 0, &scissors);

                    vulkan_context
                        .device
                        .cmd_draw(command_buffer, vertices.len() as u32, 1, 0, 0);

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

        // Request the next frame (this is the "loop")
        frame_index = (frame_index + 1) % frames_in_flight;
    }

    Ok(())
}

fn to_1d_array(x: u32, y: u32, z: u32) -> u32 {
    (z * WIDTH as u32 * HEIGHT as u32) + (y * WIDTH as u32) + x
}

fn offset_face(face: &[Vertex; 6], offset: glm::Vec3) -> [Vertex; 6] {
    core::array::from_fn(|i| Vertex {
        pos: face[i].pos + offset,
        ..face[i]
    })
}

fn generate_chunk_mesh(chunk: &Chunk) -> Vec<Vertex> {
    let mut mesh: Vec<Vertex> = vec![];
    for z in 0..WIDTH {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let pos = glm::vec3(x as f32, y as f32, z as f32);

                mesh.extend(
                    [
                        offset_face(&CUBE_FACE_FRONT, pos),
                        offset_face(&CUBE_FACE_BACK, pos),
                        offset_face(&CUBE_FACE_RIGHT, pos),
                        offset_face(&CUBE_FACE_LEFT, pos),
                        offset_face(&CUBE_FACE_TOP, pos),
                        offset_face(&CUBE_FACE_BOTTOM, pos),
                    ]
                    .concat(),
                );
            }
        }
    }
    mesh
}

// Back face (normal: 0, 0, -1)
pub static CUBE_FACE_BACK: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, -1.0),
    },
];

// Front face (normal: 0, 0, 1)
pub static CUBE_FACE_FRONT: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, 0.0, 1.0),
    },
];

// Left face (normal: -1, 0, 0)
pub static CUBE_FACE_LEFT: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(-1.0, 0.0, 0.0),
    },
];

// Right face (normal: 1, 0, 0)
pub static CUBE_FACE_RIGHT: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(1.0, 0.0, 0.0),
    },
];

// Bottom face (normal: 0, -1, 0)
pub static CUBE_FACE_BOTTOM: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, -0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, -1.0, 0.0),
    },
];

// Top face (normal: 0, 1, 0)
pub static CUBE_FACE_TOP: [Vertex; 6] = [
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 1.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(1.0, 0.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, 0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 0.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
    Vertex {
        pos: glm::Vec3::new(-0.5, 0.5, -0.5),
        color: glm::Vec3::new(0.0, 0.0, 0.0),
        tex_coord: glm::Vec2::new(0.0, 1.0),
        normals: glm::Vec3::new(0.0, 1.0, 0.0),
    },
];
