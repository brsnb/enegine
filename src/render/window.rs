use ash::extensions::khr;
use ash::vk;

use std::ffi::CStr;

use super::core;

pub struct Window {
    pub window: winit::window::Window,

    pub surface_pfn: khr::Surface,
    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::Format,
    pub surface_extent: vk::Extent2D,
}

impl Window {
    pub fn new(core: &core::Core, window: &winit::window::Window) -> Result<(), &'static str> {
        let surface_pfn = khr::Surface::new(&core.entry, &core.instance);
        unsafe {
            
        }
        Ok(())
    }
}
