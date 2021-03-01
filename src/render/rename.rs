use ash::extensions::{ext::DebugUtils, khr};
use ash::{util, vk};
use ash_window;
use image::flat::SampleLayout;

use std::ffi::CStr;
use std::io::Cursor;
use std::mem;

use glam::{Mat4, Vec2, Vec3};

use super::{core, device, renderer, swapchain};

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

            // Render pass
            let render_pass = {
                let renderpass_attachments = [
                    // Color attachment
                    vk::AttachmentDescription::builder()
                        .format(swapchain.surface_format)
                        .samples(vk::SampleCountFlags::TYPE_1) // NOTE: Non multisample currently
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                        .initial_layout(vk::ImageLayout::UNDEFINED)
                        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .build(),
                    //Depth attachment
                    vk::AttachmentDescription::builder()
                        .format(vk::Format::D32_SFLOAT) // FIXME: Maybe don't hardcode this
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::DONT_CARE)
                        .stencil_load_op(vk::AttachmentLoadOp::CLEAR)
                        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                        .initial_layout(vk::ImageLayout::UNDEFINED)
                        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .build(),
                ];

                let color_attachment_ref = [vk::AttachmentReference::builder()
                    .attachment(0)
                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .build()];

                let depth_attachment_ref = vk::AttachmentReference::builder()
                    .attachment(1)
                    .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .build();

                let subpass_desc = [vk::SubpassDescription::builder()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .color_attachments(&color_attachment_ref)
                    .depth_stencil_attachment(&depth_attachment_ref)
                    .build()];

                let dependencies = [vk::SubpassDependency::builder()
                    .src_subpass(vk::SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .build()];
            };

            Rename {
                core,
                device,
                swapchain,
            }
        }
    }
}
