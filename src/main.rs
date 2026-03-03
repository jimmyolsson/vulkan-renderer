use ash::vk::{self, ImageSubresourceRange, PipelineRenderingCreateInfo, ShaderStageFlags};
use winit::event::ElementState;
mod vulkan;

use std::ffi::c_void;
use std::u32;
use std::{default::Default, fs};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::WindowBuilder,
};

use nalgebra_glm as glm;

struct Vertex {
    pos: glm::Vec2,
    color: glm::Vec3,
    tex_coord: glm::Vec2,
}

impl Vertex {
    pub fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }
    pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: std::mem::offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: std::mem::offset_of!(Vertex, color) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: std::mem::offset_of!(Vertex, tex_coord) as u32,
            },
        ]
    }
}

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

#[repr(C)]
struct UniformBufferObject {
    model: glm::Mat4,
    view: glm::Mat4,
    projection: glm::Mat4,
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

    let mut swapchain =
        vulkan::swapchain::Swapchain::new(&vulkan_context, window_height, window_width, None)?;

    let vertices = vec![
        Vertex {
            pos: glm::vec2(-0.5, -0.5),
            color: glm::vec3(1.0, 0.0, 0.0),
            tex_coord: glm::vec2(1.0, 0.0),
        },
        Vertex {
            pos: glm::vec2(0.5, -0.5),
            color: glm::vec3(0.0, 1.0, 0.0),
            tex_coord: glm::vec2(0.0, 0.0),
        },
        Vertex {
            pos: glm::vec2(0.5, 0.5),
            color: glm::vec3(0.0, 0.0, 1.0),
            tex_coord: glm::vec2(0.0, 1.0),
        },
        Vertex {
            pos: glm::vec2(-0.5, 0.5),
            color: glm::vec3(1.0, 1.0, 1.0),
            tex_coord: glm::vec2(1.0, 1.0),
        },
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

    let bd = [Vertex::get_binding_description()];
    let ba = Vertex::get_attribute_descriptions();
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bd)
        .vertex_attribute_descriptions(&ba);

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
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(false)
        .depth_bias_slope_factor(1.0)
        .line_width(1.0);

    let multisampling_create_info = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .sample_shading_enable(false);

    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(false)
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        )];

    let color_blend_create_info = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachment);

    let layout_bindings = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX),
        vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
    ];
    let layout_create_info =
        vk::DescriptorSetLayoutCreateInfo::default().bindings(&layout_bindings);
    let descriptor_set_layout = unsafe {
        vulkan_context
            .device
            .create_descriptor_set_layout(&layout_create_info, None)
            .unwrap()
    };
    let descriptor_set_layouts = vec![descriptor_set_layout; frames_in_flight];
    let pipeline_layout_create_info =
        vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_set_layouts);
    let pipeline_layout = unsafe {
        vulkan_context
            .device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .expect("Unable to create pipeline layout")
    };

    let uniform_buffers = create_uniform_buffers(
        &vulkan_context,
        frames_in_flight as u32,
        size_of::<UniformBufferObject>() as u64,
    );

    let command_pool_create_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(vulkan_context.queue_index);

    let command_pool = unsafe {
        vulkan_context
            .device
            .create_command_pool(&command_pool_create_info, None)
            .expect("Unable to create command pool")
    };

    let texture_image = create_texture_image(&vulkan_context, "textures/texture.jpg", command_pool);
    let texture_image_view = create_texture_image_view(&vulkan_context, texture_image);
    let image_sampler = create_texture_sampler(&vulkan_context);

    // Descriptors
    let descriptor_pool_sizes = [
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(frames_in_flight as u32),
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(frames_in_flight as u32),
    ];

    let descriptor_create_info = vk::DescriptorPoolCreateInfo::default()
        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
        .max_sets(frames_in_flight as u32)
        .pool_sizes(&descriptor_pool_sizes);

    // Can silently fail
    let descriptor_pool = unsafe {
        vulkan_context
            .device
            .create_descriptor_pool(&descriptor_create_info, None)
            .unwrap()
    };

    let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&descriptor_set_layouts);

    let descriptor_sets = unsafe {
        vulkan_context
            .device
            .allocate_descriptor_sets(&descriptor_set_alloc_info)
            .unwrap()
    };

    for i in 0..frames_in_flight {
        let buffer_infos = [vk::DescriptorBufferInfo::default()
            .buffer(uniform_buffers[i].1.buffer)
            .offset(0)
            .range(size_of::<UniformBufferObject>() as u64)];

        let image_infos = [vk::DescriptorImageInfo::default()
            .sampler(image_sampler)
            .image_view(texture_image_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];

        let descriptor_writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&buffer_infos),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[i])
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_infos),
        ];
        unsafe {
            vulkan_context
                .device
                .update_descriptor_sets(&descriptor_writes, &[]);
        }
    }

    let color_formats = [swapchain.surface_format.format];
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

    let allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(frames_in_flight as u32);

    // Vertex buffers
    let indicies = vec![0, 1, 2, 2, 3, 0];

    let size = (vertices.len() * size_of::<Vertex>()) as u64;
    let staging_buffer = create_buffer(
        &vulkan_context,
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let vertex_buffer = create_vertex_buffer(
        &vulkan_context,
        &vertices,
        command_pool,
        staging_buffer.buffer,
        staging_buffer.memory,
        size,
    );
    let index_buffer = create_index_buffer(&vulkan_context, &indicies, command_pool);

    let command_buffers = unsafe {
        vulkan_context
            .device
            .allocate_command_buffers(&allocate_info)
            .expect("Unable to allocate command buffers")
    };

    let sync_objects = SyncObjects::new(&vulkan_context.device, 3, frames_in_flight);

    let mut frame_index = 0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
                    if event.state == ElementState::Pressed
                    {
                        match event.logical_key {
                            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                                std::process::exit(0);
                            }
                            _ => {}
                        }
                    }
                }
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
                        vertex_buffer.buffer,
                        index_buffer.buffer,
                        pipeline_layout,
                        &descriptor_sets
                    );

                    unsafe {
                        vulkan_context
                            .device
                            .reset_fences(&[sync_objects.in_flight_fences[frame_index]])
                            .unwrap()
                    };

                    update_uniform_buffer(swapchain.surface_resolution, &uniform_buffers[frame_index]);

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
// TODO:
//  All the helper functions that submit commands so far have been set up to execute synchronously by waiting for the queue to become idle.
//  For practical applications it is recommended to combine these operations in a single command buffer and execute them asynchronously for higher throughput, especially the transitions and copy in the createTextureImage function.
//  Try to experiment with this by creating a setupCommandBuffer that the helper functions record commands into, and add a flushSetupCommands to execute the commands that have been recorded so far.
//  It’s best to do this after the texture mapping works to check if the texture resources are still set up correctly.

// NOTE: The sample does not contain any reference to an image.
// That is because the sample is a distinct object that provides an interface to extract
// colors from a texture.
// You can use any image you want.
fn create_texture_sampler(context: &VulkanContext) -> vk::Sampler {
    let properties = unsafe {
        context
            .instance
            .get_physical_device_properties(context.physical_device)
    };
    let create_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .mip_lod_bias(0.0)
        .anisotropy_enable(true)
        .max_anisotropy(properties.limits.max_sampler_anisotropy)
        .compare_enable(false)
        .compare_op(vk::CompareOp::ALWAYS);
    unsafe { context.device.create_sampler(&create_info, None).unwrap() }
}

fn create_texture_image_view(context: &VulkanContext, image: vk::Image) -> vk::ImageView {
    let create_info = vk::ImageViewCreateInfo::default()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_SRGB)
        .components(vk::ComponentMapping {
            r: vk::ComponentSwizzle::IDENTITY,
            g: vk::ComponentSwizzle::IDENTITY,
            b: vk::ComponentSwizzle::IDENTITY,
            a: vk::ComponentSwizzle::IDENTITY,
        })
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
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
fn create_texture_image(
    context: &VulkanContext,
    file_name: &str,
    command_pool: vk::CommandPool,
) -> vk::Image {
    use image::GenericImageView;
    let img = image::open(file_name).expect("Unable to load texture");

    let (tex_width, tex_height) = img.dimensions();

    // Convert to RGBA8 format (equivalent to STBI_rgb_alpha)
    let rgba_image = img.to_rgba8();

    let pixels: &[u8] = rgba_image.as_raw();

    let image_size: u64 = (tex_width * tex_height * 4) as u64;

    println!(
        "Loaded texture: [{}] {}x{}, size: {} bytes",
        file_name, tex_width, tex_height, image_size
    );

    let staging_buffer = create_buffer(
        context,
        image_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .unwrap();

    let data = unsafe {
        context
            .device
            .map_memory(
                staging_buffer.memory,
                0,
                image_size,
                vk::MemoryMapFlags::empty(),
            )
            .expect("Unable to map memory")
    };

    unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), data as *mut u8, pixels.len()) }

    unsafe { context.device.unmap_memory(staging_buffer.memory) }

    let image = create_image(
        context,
        tex_width,
        tex_height,
        vk::Format::R8G8B8A8_SRGB,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    );

    transition_image_layout2(
        context,
        command_pool,
        image,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    unsafe {
        copy_buffer_to_img(
            &context.device,
            command_pool,
            context.queue,
            staging_buffer.buffer,
            image,
            tex_width,
            tex_height,
        );
    }
    transition_image_layout2(
        context,
        command_pool,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    image
}

fn transition_image_layout2(
    context: &VulkanContext,
    command_pool: vk::CommandPool,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    unsafe {
        immediate_submit(&context.device, command_pool, context.queue, |cmd| {
            let mut src_access_mask = vk::AccessFlags::NONE;
            #[allow(unused)]
            let mut dst_access_mask = vk::AccessFlags::NONE;
            #[allow(unused)]
            let mut source_stage = vk::PipelineStageFlags::NONE;
            #[allow(unused)]
            let mut dest_stage = vk::PipelineStageFlags::NONE;

            match (old_layout, new_layout) {
                (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => {
                    dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;

                    source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
                    dest_stage = vk::PipelineStageFlags::TRANSFER;
                }
                (
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ) => {
                    src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                    dst_access_mask = vk::AccessFlags::SHADER_READ;

                    source_stage = vk::PipelineStageFlags::TRANSFER;
                    dest_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
                }
                _ => panic!("Unsupported layout transition"),
            }

            let barrier = [vk::ImageMemoryBarrier::default()
                .old_layout(old_layout)
                .new_layout(new_layout)
                .image(image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .src_access_mask(src_access_mask)
                .dst_access_mask(dst_access_mask)];

            context.device.cmd_pipeline_barrier(
                cmd,
                source_stage,
                dest_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &barrier,
            );
        });
    }
}

unsafe fn immediate_submit<F>(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    record: F,
) where
    F: FnOnce(vk::CommandBuffer),
{
    // 1. Allocate
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

fn create_image(
    context: &VulkanContext,
    width: u32,
    height: u32,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> vk::Image {
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
    let image_memory = unsafe { context.device.allocate_memory(&alloc_info, None).unwrap() };
    unsafe {
        context
            .device
            .bind_image_memory(image, image_memory, 0)
            .unwrap()
    };
    image
}

unsafe fn copy_buffer_to_img(
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

fn update_uniform_buffer(
    swapchain_extent: vk::Extent2D,
    uniforms: &(*mut c_void, CreateBufferResult),
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
    let view = glm::look_at(
        &glm::vec3(2.0, 2.0, 2.0),
        &glm::vec3(0.0, 0.0, 0.0),
        &glm::vec3(0.0, 0.0, 1.0),
    );
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
        std::ptr::copy_nonoverlapping(&uniform_object, uniforms.0 as *mut UniformBufferObject, 1)
    };
}

fn create_vertex_buffer(
    context: &VulkanContext,
    vertices: &Vec<Vertex>,
    command_pool: vk::CommandPool,
    staging_buffer: vk::Buffer,
    staging_buffer_memory: vk::DeviceMemory,
    size: vk::DeviceSize,
) -> CreateBufferResult {
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

fn create_index_buffer(
    context: &VulkanContext,
    indexes: &Vec<u32>,
    command_pool: vk::CommandPool,
) -> CreateBufferResult {
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

fn create_uniform_buffers(
    context: &VulkanContext,
    frames_in_flight: u32,
    size: vk::DeviceSize,
) -> Vec<(*mut c_void, CreateBufferResult)> {
    let mut buffers = Vec::with_capacity(frames_in_flight as usize);

    for _ in 0..frames_in_flight {
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

        buffers.push((data_ptr, buffer));
    }

    buffers
}

fn submit_copy_buffer_cmd(
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
    index_buffer: vk::Buffer,
    pipeline_layout: vk::PipelineLayout,
    descriptor_sets: &[vk::DescriptorSet],
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

        let buffer = [vertex_buffer];
        let offsets = [0 as u64];
        device.cmd_bind_vertex_buffers(command_buffer, 0 as u32, &buffer, &offsets);
        device.cmd_bind_index_buffer(command_buffer, index_buffer, 0, vk::IndexType::UINT32);

        device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            pipeline_layout,
            0,
            descriptor_sets,
            &[],
        );
        device.cmd_set_viewport(command_buffer, 0, &viewports);
        device.cmd_set_scissor(command_buffer, 0, &scissors);

        device.cmd_draw_indexed(command_buffer, 6, 1, 0, 0, 0);

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
        .subresource_range(ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
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
use std::io::Read;

use crate::vulkan::context::VulkanContext;
// use crate::vulkan::swapchain;

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
