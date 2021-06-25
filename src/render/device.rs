use ash::vk;
use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    vk::SharingMode,
};
use vk_mem;

use super::core::Core;

use std::ffi::CStr;

pub struct Buffer {
    pub handle: vk::Buffer,
    pub memory: vk_mem::Allocation,
    pub info: vk_mem::AllocationInfo,
}

pub struct Image {
    pub handle: vk::Image,
    pub memory: vk_mem::Allocation,
    pub info: vk_mem::AllocationInfo,
}

// Deal with non-distinct compute/transfer queues elsewhere
#[derive(Default)]
pub struct QueueFamilyIndices {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub transfer: Option<u32>,
}

pub struct Device {
    pub physical_device: vk::PhysicalDevice,
    pub logical_device: ash::Device,
    pub properties: vk::PhysicalDeviceProperties,
    pub mem_properties: vk::PhysicalDeviceMemoryProperties,
    pub features: vk::PhysicalDeviceFeatures,
    pub enabled_features: vk::PhysicalDeviceFeatures,
    pub supported_exts: Vec<String>,
    pub command_pool: vk::CommandPool,
    pub queue_family_properties: Vec<vk::QueueFamilyProperties>,
    pub queue_family_indices: QueueFamilyIndices,
    pub allocator: vk_mem::Allocator,
}

impl Device {
    pub fn new(
        core: &Core,
        physical_device: vk::PhysicalDevice,
        device_extensions: Vec<*const i8>,
        device_features: vk::PhysicalDeviceFeatures,
        queue_types: vk::QueueFlags,
    ) -> Self {
        unsafe {
            let queue_family_properties = core
                .instance
                .get_physical_device_queue_family_properties(physical_device);

            let mut queue_family_indices = QueueFamilyIndices::default();

            // Logical device creation
            let queue_prios = [0.0];
            let queue_create_infos = Vec::new();

            // NOTE: Assume graphics queue is requested for now
            if !queue_types.contains(vk::QueueFlags::GRAPHICS) {
                panic!("No graphics queue requested");
            }
            let graphics_queue_idx =
                Device::get_queue_family_idx(vk::QueueFlags::GRAPHICS, &queue_family_properties)
                    .unwrap();
            let graphics_queue_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(graphics_queue_idx)
                .build();
            queue_create_infos.push(graphics_queue_info);
            queue_family_indices.graphics = Some(graphics_queue_idx);

            // Compute queue
            let mut compute_queue_idx = None;
            if queue_types.contains(vk::QueueFlags::COMPUTE) {
                let queue_idx = Device::get_queue_family_idx(
                    vk::QueueFlags::TRANSFER,
                    &queue_family_properties,
                )
                .unwrap();

                if queue_idx != graphics_queue_idx {
                    let compute_queue_info = vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(queue_idx) // FIXME: This might be bad
                        .queue_priorities(&queue_prios)
                        .build();
                    queue_create_infos.push(compute_queue_info);
                    compute_queue_idx = Some(queue_idx);
                }
            }
            queue_family_indices.compute = compute_queue_idx;

            // Transfer queue
            let mut transfer_queue_idx = None;
            if queue_types.contains(vk::QueueFlags::TRANSFER) {
                let queue_idx = Device::get_queue_family_idx(
                    vk::QueueFlags::TRANSFER,
                    &queue_family_properties,
                )
                .unwrap();

                // FIXME
                if queue_idx != graphics_queue_idx
                    && queue_idx != compute_queue_idx.unwrap_or(queue_idx + 1)
                {
                    let transfer_queue_info = vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(queue_idx)
                        .queue_priorities(&queue_prios)
                        .build();
                    queue_create_infos.push(transfer_queue_info);
                    transfer_queue_idx = Some(queue_idx);
                }
            }
            queue_family_indices.transfer = transfer_queue_idx;

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_extensions)
                .enabled_features(&device_features);

            let logical_device = core
                .instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap();

            let properties = core
                .instance
                .get_physical_device_properties(physical_device);
            let mem_properties = core
                .instance
                .get_physical_device_memory_properties(physical_device);
            let features = core.instance.get_physical_device_features(physical_device);
            let enabled_features = device_features;
            let supported_exts = core
                .instance
                .enumerate_device_extension_properties(physical_device)
                .unwrap()
                .iter()
                .map(|ext| {
                    String::from(
                        CStr::from_ptr(ext.extension_name.as_ptr())
                            .to_str()
                            .unwrap(),
                    )
                })
                .collect();
            let command_pool_info = vk::CommandPoolCreateInfo::default();
            let command_pool = logical_device
                .create_command_pool(&command_pool_info, None)
                .unwrap();

            // Allocator
            let allocator_create_info = vk_mem::AllocatorCreateInfo {
                physical_device,
                device: logical_device,
                instance: core.instance,
                flags: vk_mem::AllocatorCreateFlags::NONE,
                ..Default::default()
            };

            let allocator = vk_mem::Allocator::new(&allocator_create_info).unwrap();

            Device {
                physical_device,
                logical_device,
                properties,
                mem_properties,
                features,
                enabled_features,
                supported_exts,
                command_pool,
                queue_family_properties,
                queue_family_indices,
                allocator,
            }
        }
    }

    pub fn create_buffer(
        &self,
        buffer_info: vk::BufferCreateInfo,
        properties: vk_mem::MemoryUsage,
    ) -> Option<Buffer> {
        let allocation_info = vk_mem::AllocationCreateInfo {
            usage: properties,
            ..Default::default()
        };

        let alloc = self
            .allocator
            .create_buffer(&buffer_info, &allocation_info)
            .unwrap(); // FIXME

        Some(Buffer {
            handle: alloc.0,
            memory: alloc.1,
            info: alloc.2,
        })
    }

    pub fn create_image(
        &self,
        image_info: &vk::ImageCreateInfo,
        properties: vk_mem::MemoryUsage,
    ) -> Option<Image> {
        let allocation_info = vk_mem::AllocationCreateInfo {
            usage: properties,
            ..Default::default()
        };

        let alloc = self
            .allocator
            .create_image(image_info, &allocation_info)
            .unwrap(); // FIXME

        Some(Image {
            handle: alloc.0,
            memory: alloc.1,
            info: alloc.2,
        })
    }

    pub fn destroy_buffer(&self, buffer: Buffer) {
        self.allocator
            .destroy_buffer(buffer.handle, &buffer.memory)
            .unwrap();
    }

    pub fn desroy_image(&self, image: Image) {
        self.allocator
            .destroy_image(image.handle, &image.memory)
            .unwrap();
    }

    fn get_queue_family_idx(
        queue_flags: vk::QueueFlags,
        queue_family_properties: &Vec<vk::QueueFamilyProperties>,
    ) -> Result<u32, &'static str> {
        // Try to get dedicated compute queue if requested
        if queue_flags.contains(vk::QueueFlags::COMPUTE) {
            for (i, queue) in queue_family_properties.iter().enumerate() {
                if queue.queue_flags.contains(queue_flags)
                    && queue.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                {
                    return Ok(i as u32);
                }
            }
        }

        // Try to get dedicated transfer queue if requested
        if queue_flags.contains(vk::QueueFlags::TRANSFER) {
            for (i, queue) in queue_family_properties.iter().enumerate() {
                if (queue.queue_flags.contains(queue_flags))
                    && queue.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && queue.queue_flags.contains(vk::QueueFlags::COMPUTE)
                {
                    return Ok(i as u32);
                }
            }
        }

        // Get first queue to supported requested
        for (i, queue) in queue_family_properties.iter().enumerate() {
            if queue.queue_flags.contains(queue_flags) {
                return Ok(i as u32);
            }
        }

        Err("No compatible queue foung")
    }
}
