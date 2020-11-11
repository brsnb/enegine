use ash::extensions::khr::{Surface, Swapchain};
use ash::vk;

use std::ffi::CStr;

pub struct Window {
    pub window: winit::window::Window,

    pub surface_fn: Surface,
    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::Format,
    pub surface_extent: vk::Extent2D,
}

impl Window {
    pub fn new(window: &winit::window::Window) -> Result<(), &'static str> {
        unsafe {
            
        }
        Ok(())
    }
}
