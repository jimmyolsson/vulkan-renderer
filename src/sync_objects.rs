use ash::vk;

pub struct SyncObjects {
    pub present_complete_semaphores: Vec<vk::Semaphore>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
}

impl SyncObjects {
    pub fn new(
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
