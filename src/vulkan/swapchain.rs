use crate::vulkan::context::VulkanContext;
use ash::{khr::swapchain, vk};

use anyhow::Result;

pub struct Swapchain {
    device: ash::Device,       // Cloned
    loader: swapchain::Device, // Cloned

    pub image_views: Vec<vk::ImageView>,
    pub images: Vec<vk::Image>,
    pub handle: vk::SwapchainKHR, // Should prob wrap this

    pub surface_capabilities: vk::SurfaceCapabilitiesKHR,
    pub surface_resolution: vk::Extent2D,
    pub surface_format: vk::SurfaceFormatKHR,
}

impl Swapchain {
    pub fn new(
        context: &VulkanContext,
        surface_width: u32,
        surface_height: u32,
        old_swapchain: Option<&Swapchain>,
    ) -> Result<Swapchain> {
        let surface_capabilities = unsafe {
            context
                .surface_instance
                .get_physical_device_surface_capabilities(context.physical_device, context.surface)
                .unwrap()
        };

        let surface_resolution = match surface_capabilities.current_extent.width {
            u32::MAX => vk::Extent2D {
                width: surface_height,
                height: surface_width,
            },

            _ => surface_capabilities.current_extent,
        };
        let surface_format = unsafe {
            context
                .surface_instance
                .get_physical_device_surface_formats(context.physical_device, context.surface)?[0]
        };

        let present_mode = vk::PresentModeKHR::MAILBOX;
        let desired_image_count = surface_capabilities.min_image_count + 1; // ?
        let surface_transform = surface_capabilities.current_transform;

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(context.surface)
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

        if let Some(old) = old_swapchain {
            create_info.old_swapchain(old.handle);
        }

        let handle = unsafe {
            context
                .swapchain_loader
                .create_swapchain(&create_info, None)
                .expect("Unable to create swapchain")
        };

        let images = unsafe {
            context
                .swapchain_loader
                .get_swapchain_images(handle)
                .unwrap()
        };
        let image_views: Vec<vk::ImageView> = images
            .iter()
            .map(|&image| unsafe {
                {
                    let create_info = vk::ImageViewCreateInfo::default()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
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

                    context
                        .device
                        .create_image_view(&create_info, None)
                        .unwrap()
                }
            })
            .collect();

        Ok(Swapchain {
            device: context.device.clone(),
            loader: context.swapchain_loader.clone(),
            image_views,
            images,
            handle,
            surface_capabilities,
            surface_format,
            surface_resolution,
        })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_swapchain(self.handle, None);

            for &view in &self.image_views {
                self.device.destroy_image_view(view, None);
            }
            self.image_views.clear();
            self.images.clear();
        };
    }
}
