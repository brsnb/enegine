use ash::vk;
use ultraviolet as uv;

pub struct Vertex {
    pub position: uv::Vec3,
    pub normal: uv::Vec3,
    pub texcoord_0: uv::Vec2,
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

enum AlphaMode {
    OPAQUE,
    MASK,
    BLEND,
}

struct Material {
    // Textures
    base_color: Texture,
    metallic_roughness: Texture,
    normal: Texture,
    occlusion: Texture,
    emissive: Texture,

    // Scaling
    base_color_factor: uv::Vec4,
    metallic_factor: f32,
    roughness_factor: f32,
    normal_scale: f32,
    occlusion_strength: f32,
    emissive_factor: uv::Vec4,

    // Other
    alpha_mode: AlphaMode,
    alpha_cutoff: f32,
    double_sided: bool,
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
