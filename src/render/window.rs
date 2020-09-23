use ash::extensions::khr::{Surface, Swapchain};
use ash::vk;

pub struct Window {
    window: winit::window::Window,

    surface_loader: Surface,
    surface: vk::SurfaceKHR,
    surface_format: vk::Format,
    surface_extent: vk::Extent2D,

    swapchain_loader: Swapchain,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,

    frames_in_flight: usize,
    current_frame: usize,
    in_flight_fences: Vec<vk::Fence>,
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,
}

impl Window {
    pub fn new() -> Result<(), &'static str> {
        Ok(())
    }
}
