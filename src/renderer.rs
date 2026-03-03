use crate::vertex::Vertex;
use crate::vulkan::context;
use crate::vulkan::swapchain;
use anyhow::Ok;
use anyhow::{Context, Result};
use ash::vk;

pub struct Renderer {
    pub swapchain: swapchain::Swapchain,
    pub graphic_pipelines: Vec<vk::Pipeline>,
}

// Must match the order of pipelines in Renderer::new
pub enum PipelineType {
    Texture,
    TextureWireframe,
}

impl Renderer {
    pub fn new(vulkan_context: &context::VulkanContext) -> Result<Self> {
        let frames_in_flight: usize = 2;

        let swapchain = Self::create_swapchain(vulkan_context)?;

        let shader_module = Self::create_shader_module(vulkan_context)?;

        let graphic_pipelines = vec![
            Self::create_pipeline_basic(
                vulkan_context,
                &swapchain,
                shader_module,
                frames_in_flight,
            )?,
            Self::create_pipeline_basic(
                vulkan_context,
                &swapchain,
                shader_module,
                frames_in_flight,
            )?,
        ];

        Ok(Self {
            swapchain,
            graphic_pipelines,
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
        frames_in_flight: usize,
    ) -> Result<vk::Pipeline> {
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

        Ok(unsafe {
            vulkan_context
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
                .unwrap()[0]
        })
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
