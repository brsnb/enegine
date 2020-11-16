use ash::vk;

pub struct QueueFamilyIndices {
    graphics: Option<u32>,
    compute: Option<u32>,
    transfer: Option<u32>,
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
    pub queue_family_indicies: QueueFamilyIndices,
}

impl Device {
    pub fn new(physical_device: vk::PhysicalDevice) {

    }
}