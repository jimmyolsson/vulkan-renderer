use crate::vertex::Vertex;
use crate::vulkan::context;
use crate::vulkan::swapchain;
use anyhow::{Context, Ok, Result};
use ash::vk;

const FRAMES_IN_FLIGHT: usize = 2;
pub struct Renderer {
    pub swapchain: swapchain::Swapchain,
    pipeline_variants: PipelineVariants,
    sync_objects: SyncObjects,

    command_buffers: [vk::CommandBuffer; FRAMES_IN_FLIGHT],

    uniform_buffers: [context::AllocatedMappedBuffer; FRAMES_IN_FLIGHT],

    // Temporary public
    pub command_pool: vk::CommandPool,
}

#[derive(Clone, Copy)]
pub struct PipelineInfo {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub descriptor_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT],
}

#[derive(Clone, Copy)]
pub struct PipelineVariants {
    pub texture: PipelineInfo,
    pub texture_wireframe: PipelineInfo,
}

impl Renderer {
    pub fn new(vulkan_context: &context::VulkanContext) -> Result<Self> {
        let swapchain = Self::create_swapchain(vulkan_context)?;

        let sync_objects = SyncObjects::new(
            &vulkan_context.device,
            swapchain.image_count as usize,
            FRAMES_IN_FLIGHT,
        );

        let command_pool = Self::create_command_pool(vulkan_context)?;

        let command_buffers = Self::create_command_buffers(vulkan_context, command_pool)?;

        let uniform_buffers = context::create_uniform_buffers::<FRAMES_IN_FLIGHT>(
            &vulkan_context,
            size_of::<context::UniformBufferObject>() as u64,
        );

        let pipeline_variants =
            Self::create_pipelines(vulkan_context, &swapchain, &uniform_buffers, command_pool)?;

        Ok(Self {
            swapchain,
            pipeline_variants,
            sync_objects,
            command_buffers,
            uniform_buffers,
            command_pool,
        })
    }

    pub fn handle_resize(
        &mut self,
        context: &context::VulkanContext,
        surface_width: u32,
        surface_height: u32,
    ) {
        self.swapchain
            .recreate(context, surface_width, surface_height);
    }

    fn create_command_pool(vulkan_context: &context::VulkanContext) -> Result<vk::CommandPool> {
        let command_pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(vulkan_context.queue_index);

        unsafe {
            vulkan_context
                .device
                .create_command_pool(&command_pool_create_info, None)
                .context("Unable to create command pool")
        }
    }

    fn create_command_buffers(
        vulkan_context: &context::VulkanContext,
        command_pool: vk::CommandPool,
    ) -> Result<[vk::CommandBuffer; FRAMES_IN_FLIGHT]> {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(FRAMES_IN_FLIGHT as u32);

        let command_buffers = unsafe {
            vulkan_context
                .device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()
        };

        Ok(command_buffers.try_into().unwrap())
    }

    fn create_pipelines(
        vulkan_context: &context::VulkanContext,
        swapchain: &swapchain::Swapchain,
        uniform_buffers: &[context::AllocatedMappedBuffer; FRAMES_IN_FLIGHT],
        command_pool: vk::CommandPool,
    ) -> Result<PipelineVariants> {
        let shader_module = Self::create_shader_module(vulkan_context)?;
        let (descriptor_sets, descriptor_set_layout) = Self::create_descriptors(vulkan_context)?;

        // This whole shit will be replaced when we move to draw_* methods e.g draw_mesh/draw_quad
        let texture_image =
            create_texture_image(&vulkan_context, "textures/texture.jpg", command_pool);

        let texture_image_view = context::create_texture_image_view(
            &vulkan_context,
            texture_image,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageAspectFlags::COLOR,
        );
        let image_sampler = create_texture_sampler(&vulkan_context);

        Self::update_descriptor_sets(
            vulkan_context,
            &descriptor_sets,
            uniform_buffers,
            image_sampler,
            texture_image_view,
        );

        let graphic_pipelines = vec![
            Self::create_pipeline_basic(
                vulkan_context,
                &swapchain,
                shader_module,
                descriptor_set_layout,
                false,
            )?,
            Self::create_pipeline_basic(
                vulkan_context,
                &swapchain,
                shader_module,
                descriptor_set_layout,
                true,
            )?,
        ];

        Ok(PipelineVariants {
            texture: PipelineInfo {
                pipeline: graphic_pipelines[0].0,
                layout: graphic_pipelines[0].1,
                descriptor_sets: descriptor_sets,
            },
            texture_wireframe: PipelineInfo {
                pipeline: graphic_pipelines[1].0,
                layout: graphic_pipelines[1].1,
                descriptor_sets: descriptor_sets,
            },
        })
    }

    fn update_descriptor_sets(
        vulkan_context: &context::VulkanContext,
        descriptor_sets: &[vk::DescriptorSet; FRAMES_IN_FLIGHT],
        uniform_buffers: &[context::AllocatedMappedBuffer; FRAMES_IN_FLIGHT],
        image_sampler: vk::Sampler,
        texture_image_view: vk::ImageView,
    ) {
        for i in 0..FRAMES_IN_FLIGHT {
            let buffer_infos = [vk::DescriptorBufferInfo::default()
                .buffer(uniform_buffers[i].buffer)
                .offset(0)
                .range(size_of::<context::UniformBufferObject>() as u64)];

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
    }

    fn create_descriptors(
        vulkan_context: &context::VulkanContext,
    ) -> Result<(
        [vk::DescriptorSet; FRAMES_IN_FLIGHT],
        [vk::DescriptorSetLayout; FRAMES_IN_FLIGHT],
    )> {
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
        let descriptor_pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(FRAMES_IN_FLIGHT as u32),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(FRAMES_IN_FLIGHT as u32),
        ];

        let descriptor_create_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .max_sets(FRAMES_IN_FLIGHT as u32)
            .pool_sizes(&descriptor_pool_sizes);

        // Can silently fail
        let descriptor_pool = unsafe {
            vulkan_context
                .device
                .create_descriptor_pool(&descriptor_create_info, None)
                .unwrap()
        };

        let descriptor_set_layouts = vec![descriptor_set_layout; FRAMES_IN_FLIGHT];
        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&descriptor_set_layouts);

        let descriptor_sets = unsafe {
            vulkan_context
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
                .unwrap()
        };

        Ok((
            descriptor_sets.try_into().unwrap(),
            descriptor_set_layouts.try_into().unwrap(),
        ))
    }

    pub fn draw_frame<F>(
        &mut self,
        vulkan_context: &context::VulkanContext,
        frame_index: usize,
        record: F,
    ) where
        F: FnOnce(
            vk::CommandBuffer,
            vk::Image,
            vk::Image,
            PipelineVariants,
            vk::Extent2D,
            vk::ImageView,
            vk::ImageView,
            vk::Extent2D,
        ),
    {
        unsafe {
            vulkan_context
                .device
                .wait_for_fences(
                    &[self.sync_objects.in_flight_fences[frame_index]],
                    true,
                    u64::MAX,
                )
                .unwrap()
        };

        let acquire_image_result = unsafe {
            vulkan_context.swapchain_loader.acquire_next_image(
                self.swapchain.handle,
                u64::MAX,
                self.sync_objects.present_complete_semaphores[frame_index],
                vk::Fence::null(),
            )
        };
        if let Err(vk::Result::ERROR_OUT_OF_DATE_KHR) = acquire_image_result {
            panic!(
                "ERROR_OUT_OF_DATE_KHR... should probably fix resizing/minimization correctly now.."
            );
            // swapchain.recreate(&vulkan_context, window_width, window_height);
        }

        let (next_image_index, _) = acquire_image_result.unwrap();

        let image_index = next_image_index as usize;
        let image = self.swapchain.images[image_index];
        let image_view = self.swapchain.image_views[image_index];

        record(
            self.command_buffers[frame_index],
            image,
            self.swapchain.image_depth,
            self.pipeline_variants,
            self.swapchain.surface_resolution,
            image_view,
            self.swapchain.image_view_depth,
            self.swapchain.surface_resolution,
        );

        context::update_uniform_buffer(
            self.swapchain.surface_resolution,
            &self.uniform_buffers[frame_index],
        );
        unsafe {
            vulkan_context
                .device
                .reset_fences(&[self.sync_objects.in_flight_fences[frame_index]])
                .unwrap()
        };

        let wait_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_sem = [self.sync_objects.present_complete_semaphores[frame_index]];
        let semap = [self.sync_objects.render_finished_semaphores[image_index]];
        let command_buffer = [self.command_buffers[frame_index]];

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
                    self.sync_objects.in_flight_fences[frame_index],
                )
                .unwrap()
        };

        let swapchains = [self.swapchain.handle];
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
    }

    fn create_swapchain(vulkan_context: &context::VulkanContext) -> Result<swapchain::Swapchain> {
        let surface_extent = unsafe {
            vulkan_context
                .surface_instance
                .get_physical_device_surface_capabilities(
                    vulkan_context.physical_device,
                    vulkan_context.surface,
                )
                .context("get_physical_device_surface_capabilities failed!")?
                .current_extent
        };

        swapchain::Swapchain::new(
            &vulkan_context,
            surface_extent.width,
            surface_extent.height,
            None,
        )
        .context("Failed to create swapchain")
    }

    fn create_shader_module(vulkan_context: &context::VulkanContext) -> Result<vk::ShaderModule> {
        // Load shaders
        let shader_code = Self::read_spv("shaders\\shader.spv");
        let shader_create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);
        unsafe {
            vulkan_context
                .device
                .create_shader_module(&shader_create_info, None)
                .context("Unable to create shader module")
        }
    }

    fn create_pipeline_basic(
        vulkan_context: &context::VulkanContext,
        swapchain: &swapchain::Swapchain,
        shader_module: vk::ShaderModule,
        descriptor_set_layouts: [vk::DescriptorSetLayout; FRAMES_IN_FLIGHT],
        wireframe: bool,
    ) -> Result<(vk::Pipeline, vk::PipelineLayout)> {
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

        let viewport_state_info = vk::PipelineViewportStateCreateInfo::default()
            .scissor_count(1)
            .viewport_count(1);

        let rasterizer_create_info = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(if wireframe {
                vk::PolygonMode::LINE
            } else {
                vk::PolygonMode::FILL
            })
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

        let pipeline_layout_create_info =
            vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_set_layouts);
        let pipeline_layout = unsafe {
            vulkan_context
                .device
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .expect("Unable to create pipeline layout")
        };
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let color_formats = [swapchain.surface_format.format];
        let mut pipeline_rendering_create_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_formats)
            .depth_attachment_format(vk::Format::D32_SFLOAT);

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .name(c"vertMain")
                .module(shader_module),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .name(c"fragMain")
                .module(shader_module),
        ];

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
            .depth_stencil_state(&depth_stencil_state)
            .render_pass(vk::RenderPass::null()) // dynamic rendering
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1)
            .layout(pipeline_layout);

        let pipeline = unsafe {
            vulkan_context
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
                .unwrap()[0]
        };

        Ok((pipeline, pipeline_layout))
    }

    fn read_spv(path: &str) -> Vec<u32> {
        use std::io::Read;

        println!("cwd = {:?}", std::env::current_dir().unwrap());
        let mut file = std::fs::File::open(path).expect("Failed to open SPIR-V file");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .expect("Failed to read SPIR-V file");

        // Convert &[u8] → &[u32]
        let (_, words, _) = unsafe { bytes.align_to::<u32>() };
        words.to_vec()
    }
}

// TODO:
//  All the helper functions that submit commands so far have been set up to execute synchronously by waiting for the queue to become idle.
//  For practical applications it is recommended to combine these operations in a single command buffer and execute them asynchronously for higher throughput, especially the transitions and copy in the createTextureImage function.
//  Try to experiment with this by creating a setupCommandBuffer that the helper functions record commands into, and add a flushSetupCommands to execute the commands that have been recorded so far.
//  It’s best to do this after the texture mapping works to check if the texture resources are still set up correctly.

// NOTE: The sampler does not contain any reference to an image.
// That is because the sample is a distinct object that provides an interface to extract
// colors from a texture.
// You can use any image you want.
fn create_texture_sampler(context: &context::VulkanContext) -> vk::Sampler {
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

fn create_texture_image(
    context: &context::VulkanContext,
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

    let staging_buffer = context::create_buffer(
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

    let result = context::create_image(
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
        result.image,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    unsafe {
        context::copy_buffer_to_img(
            &context.device,
            command_pool,
            context.queue,
            staging_buffer.buffer,
            result.image,
            tex_width,
            tex_height,
        );
    }
    transition_image_layout2(
        context,
        command_pool,
        result.image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    result.image
}
fn transition_image_layout2(
    context: &context::VulkanContext,
    command_pool: vk::CommandPool,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    unsafe {
        context::immediate_submit(&context.device, command_pool, context.queue, |cmd| {
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
