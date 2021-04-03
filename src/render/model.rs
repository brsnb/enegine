use ash::vk;
use ultraviolet as uv;

pub struct Vertex {
    pub position: uv::Vec3,
    pub normal: uv::Vec3,
    pub uv_0: uv::Vec2,
    pub uv_1: uv::Vec2,
}

struct SamplerAddressModes {
    pub u: vk::SamplerAddressMode,
    pub v: vk::SamplerAddressMode,
    pub w:vk::SamplerAddressMode,
}

struct TextureSampler {
    pub mag_filter: vk::Filter,
    pub min_filter: vk::Filter,
    pub address_modes: SamplerAddressModes,
}

struct Texture {
    image: vk::Image,
    image_layout: vk::ImageLayout,
    
}

struct Model {

}

impl Model {
    pub fn new() {
        let (document, buffers, images) =
            gltf::import("/home/bn/projects/ency/enegine/src/bin/models/DamagedHelmet.gltf")
                .unwrap();
    }
}
