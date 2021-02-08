use ash::extensions::{
    ext::DebugUtils,
    khr,
};
use ash::{util, vk};
use ash_window;

use std::ffi::CStr;
use std::io::Cursor;
use std::mem;

use glam::{Mat4, Vec2, Vec3};

use super::{
    core,
    device,
    swapchain,
    renderer,
};

use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};

// FIXME: Will be the main render class
pub struct Rename {
    pub core: core::Core,
    pub device: device::Device,
    pub swapchain: swapchain::Swapchain,
}

impl Rename {
    pub fn new(window: &winit::window::Window) -> Self {
        let core = core::Core::new(window);

        unsafe {
            // FIXME: SURFACE
            let surface = ash_window::create_surface(&core.entry, &core.instance, window, None)
                .expect("Could not create surface");
            let surface_pfn = khr::Surface::new(&core.entry, &core.instance);

            // Physical device selection
            let physical_devices = core
                .instance
                .enumerate_physical_devices()
                .expect("Could not enumerate physical devices");

            // FIXME: Implement proper gpu selection
            info!("Selecting first gpu/physical device");
            let physical_device = physical_devices[0];

            let device_extensions = vec![khr::Swapchain::name().as_ptr()];
            let device_features = vk::PhysicalDeviceFeatures::default();
            let queue_types = vk::QueueFlags::GRAPHICS; // NOTE: Only selecting graphics queue for now


            let device = device::Device::new(
                &core,
                physical_device,
                device_extensions,
                device_features,
                queue_types,
            );

            // Swapchain
            let swapchain = swapchain::Swapchain::new(&core, &device, window).unwrap();

            

            Rename { core, device, swapchain }
        }
    }
}