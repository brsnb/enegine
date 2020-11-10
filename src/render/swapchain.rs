use ash::extensions::khr;
use ash::vk;

pub struct Swapchain {
    core: Core,
    swapchain_fn: khr::Swapchain,
    swapchain: vk::SurfaceKHR,
    surface_format: vk::Format
}