use ash::vk::{self};

mod camera;
mod renderer;
mod sync_objects;
mod vertex;
mod vulkan;

use camera::Camera;
use nalgebra_glm::vec3;
use vertex::Vertex;
use vulkan::context;

use log::{info, trace};
use std::u32;

use nalgebra_glm as glm;
use std::io::Write;

use renderer::Renderable;
use renderer::ShaderDataTexture;

const WIDTH: u32 = 10;
const HEIGHT: u32 = 10;
const TOTAL_BLOCKS_IN_CHUNK: u32 = WIDTH * WIDTH * HEIGHT;

struct Chunk {
    occupancy: [bool; WIDTH as usize * HEIGHT as usize],
    // mesh: renderer::Mesh,
}

// impl Chunk {
//     pub fn new(vulkan_context: &context::VulkanContext, command_pool: vk::CommandPool) -> Self {
//         let mesh = renderer::Mesh::new(vulkan_context, command_pool, vec![]);

//         Chunk {
//             occupancy: [false; 100],
//             mesh: mesh,
//         }
//     }
// }

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
    let mut window = video
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

    let mut camera = Camera::new(glm::vec3(0.0, 0.0, 15.0), 15.0);

    let mut model = glm::identity();
    let mut a = glm::translate(&model, &glm::vec3(2.0, 1.0, 1.0));

    // Flip this?
    let mut projection = glm::perspective(
        renderer.swapchain.surface_resolution.width as f32
            / renderer.swapchain.surface_resolution.height as f32,
        45.0_f32.to_radians(),
        0.1,
        1000.0,
    );
    projection[(1, 1)] *= -1.0;

    let mesh = renderer::Mesh::new(&vulkan_context, renderer.command_pool, vertices);

    let mut wireframe = false;

    let mut frame_index = 0;

    let mut running = true;
    let mut event_pump = sdl.event_pump()?;

    // Before the loop
    let mut last_frame = std::time::Instant::now();

    log_step(program_start_time, &mut last_time, "Vertex buffers created");
    let mut fps_timer = 0.0 as f32;
    let mut fps_frames = 0 as u32;
    let mut rotation_angle: f32 = 0.0;

    let cube_mesh =
        renderer::Mesh::new(&vulkan_context, renderer.command_pool, generate_cube_mesh());

    while running {
        let now = std::time::Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32();
        last_frame = now;

        fps_timer += dt;
        fps_frames += 1;

        // Update title 4 times per second
        if fps_timer >= 0.25 {
            let fps = fps_frames as f32 / fps_timer;
            let title = format!("My Vulkan App - {:.1} FPS", fps);

            // sdl3::video::Window::set_title(&mut self, &str)
            let _ = window.set_title(&title);

            fps_timer = 0.0;
            fps_frames = 0;
        }
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
        rotation_angle += dt * 90.0_f32.to_radians();
        // let size = WIDTH * 5;
        // for x in 0..size {
        //     for y in 0..size {
        //         let xf = x as f32 - 5.0;
        //         let yf = y as f32 - 5.0;

        //         let mut model = glm::identity();
        //         model = glm::translate(
        //             &model,
        //             &glm::vec3(xf * 2.0 * WIDTH as f32, 1.0, yf * 2.0 * WIDTH as f32),
        //         );
        //         model = glm::rotate(&model, rotation_angle, &glm::vec3(0.0, 0.0, 1.0));

        //         let r = ((x as f32 * 0.5 + dt).sin() * 0.5 + 0.5);
        //         let g = ((y as f32 * 0.5 + dt).cos() * 0.5 + 0.5);
        //         let b = 0.7;

        //         renderer.record_renderable(Renderable::new(
        //             renderer::ShaderInput::BasicBlockOutlineColor(context::ShaderData {
        //                 model,
        //                 view: camera.view_matrix(),
        //                 projection,
        //                 color: glm::vec4(r, g, b, 1.0),
        //                 texture_index: 0,
        //             }),
        //             mesh,
        //             wireframe,
        //         ));
        //     }
        // }

        renderer.record_renderable(Renderable {
            mesh: cube_mesh,
            shader_data: renderer::ShaderInput::BasicBlockOutlineColor(
                renderer::ShaderDataTexture {
                    model,
                    view: camera.view_matrix(),
                    projection,
                    color: glm::vec4(1.0, 0.4, 0.1, 1.0),
                    texture_index: 0,
                },
            ),
            wireframe,
        });
        renderer.record_renderable(Renderable {
            mesh: cube_mesh,
            shader_data: renderer::ShaderInput::Color(renderer::ShaderDataColor {
                model: glm::translate(&model, &vec3(1.0, 1.0, 1.0)),
                view: camera.view_matrix(),
                projection,
                color: glm::vec4(1.0, 0.4, 0.1, 1.0),
            }),
            wireframe,
        });

        renderer.draw_frame(&vulkan_context, frame_index);

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

fn generate_cube_mesh() -> Vec<Vertex> {
    let mut mesh: Vec<Vertex> = Vec::with_capacity(36);
    let pos = glm::vec3(0.0, 0.0, 0.0);
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
    mesh
}

fn generate_chunk_mesh(_chunk: &Chunk) -> Vec<Vertex> {
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
