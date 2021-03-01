use std::unimplemented;

use ash::extensions::khr;
use ash::version::DeviceV1_0;
use ash::vk;
use winit::window;

use super::{core, device};

pub struct Swapchain {
    pub surface_pfn: khr::Surface,
    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::Format,
    pub surface_extent: vk::Extent2D,

    swapchain_pfn: khr::Swapchain,
    swapchain: vk::SwapchainKHR,

    image_count: u32,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
}

impl Swapchain {
    pub fn new(
        core: &core::Core,
        device: &device::Device,
        window: &window::Window,
    ) -> Result<Self, &'static str> {
        unsafe {
            // Surface
            let surface_pfn = khr::Surface::new(&core.entry, &core.instance);
            let surface = ash_window::create_surface(&core.entry, &core.instance, window, None)
                .expect("Could not create surface");

            // Swapchain
            let surface_caps = surface_pfn
                .get_physical_device_surface_capabilities(device.physical_device, surface)
                .unwrap();
            // FIXME: Bad, reliant on window and doesn't have dpi scaling
            //        use surface_caps.min_image_height/width???
            let surface_extent = match surface_caps.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window.inner_size().width,
                    height: window.inner_size().height,
                },
                _ => surface_caps.current_extent,
            };

            let surface_formats = surface_pfn
                .get_physical_device_surface_formats(device.physical_device, surface)
                .unwrap();

            // FIXME
            let surface_format = *surface_formats
                .iter()
                .find(|&f| {
                    f.format == vk::Format::B8G8R8A8_SRGB
                        && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .unwrap_or(&surface_formats[0]);

            let present_modes = surface_pfn
                .get_physical_device_surface_present_modes(device.physical_device, surface)
                .unwrap();
            let present_mode = *present_modes
                .iter()
                .find(|&&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO);

            let mut image_count = surface_caps.min_image_count + 1;
            if surface_caps.max_image_count > 0 && image_count > surface_caps.max_image_count {
                image_count = surface_caps.max_image_count;
            };

            let swapchain_pfn = khr::Swapchain::new(&core.instance, &device.logical_device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(image_count)
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(surface_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE) // FIXME: Only if present_queue == graphics queue
                .pre_transform(surface_caps.current_transform) // NOTE: Identity transform?
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true);

            let swapchain = swapchain_pfn
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let images = swapchain_pfn.get_swapchain_images(swapchain).unwrap();

            let image_views: Vec<vk::ImageView> = images
                .iter()
                .map(|i| {
                    let image_view = vk::ImageViewCreateInfo::builder()
                        .image(*i)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    device
                        .logical_device
                        .create_image_view(&image_view, None)
                        .unwrap()
                })
                .collect();

            Ok(Swapchain {
                surface_pfn,
                surface,
                surface_extent,
                surface_format: surface_format.format,
                swapchain_pfn,
                swapchain,
                image_count,
                images,
                image_views,
            })
        }
    }

    pub unsafe fn acquire_next_image(&self, sem: vk::Semaphore) -> Result<(u32, bool), vk::Result> {
        self.swapchain_pfn
            .acquire_next_image(self.swapchain, u64::MAX, sem, vk::Fence::null())
    }

    pub unsafe fn queue_present(&self) {
        unimplemented!()
    }
}
