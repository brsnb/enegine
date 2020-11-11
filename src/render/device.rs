use ash::vk;

pub struct Device {
    physical_device: vk::PhysicalDevice,
    logical_device: ash::Device,
    something: vk::Device,
}