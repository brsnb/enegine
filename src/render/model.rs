use ultraviolet as uv;

struct Vertex {
    position: uv::Vec3,
    normal: uv::Vec3,
    uv_0: uv::Vec2,
    uv_1: uv::Vec2,
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
