use super::{renderer, window};
use ash::extensions::khr;
use ash::vk;

pub struct Swapchain {
    core: renderer::Core,
    window: window::Window,

    swapchain_fn: khr::Swapchain,
    swapchain: vk::SurfaceKHR,
    surface_format: vk::Format,

    image_count: u32,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
}

impl Swapchain {
    pub fn new(core: &renderer::Core, window: &window::Window) -> Result<Self, &'static str> {
        unsafe {
            // Swapchain
            let surface_caps = window
                .surface_fn
                .get_physical_device_surface_capabilities(core.physical_device, window.surface)
                .unwrap();
            // FIXME: Bad, reliant on window and doesn't have dpi scaling
            //        use surface_caps.min_image_height/width???
            let surface_extent = match surface_caps.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window.window.inner_size().width,
                    height: window.window.inner_size().height,
                },
                _ => surface_caps.current_extent,
            };

            let surface_formats = window
                .surface_fn
                .get_physical_device_surface_formats(core.physical_device, window.surface)
                .unwrap();
            let surface_format = *surface_formats
                .iter()
                .find(|&f| {
                    f.format == vk::Format::B8G8R8A8_SRGB
                        && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .unwrap_or(&surface_formats[0]);

            let present_modes = window
                .surface_fn
                .get_physical_device_surface_present_modes(core.physical_device, window.surface)
                .unwrap();
            let present_mode = *present_modes
                .iter()
                .find(|&&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO);

            let mut image_count = surface_caps.min_image_count + 1;
            if surface_caps.max_image_count > 0 && image_count > surface_caps.max_image_count {
                image_count = surface_caps.max_image_count;
            };

            let swapchain_fn = khr::Swapchain::new(&core.instance, &core.device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(window.surface)
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

            let swapchain = swapchain_fn
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let images = swapchain_fn.get_swapchain_images(swapchain).unwrap();

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
                    core.device.create_image_view(&image_view, None).unwrap()
                })
                .collect();

            Ok(Swapchain {
                core,
                window,

                swapchain_fn,
                swapchain,
                surface_format,

                image_count,
                images,
                image_views,
            })
        }
    }
}
