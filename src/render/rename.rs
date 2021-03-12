use ash::{extensions::{ext::DebugUtils, khr}, vk::{DescriptorSet, DescriptorSetLayout}};
use ash::{util, vk};
use ash_window;
use image::{flat::SampleLayout, math::Rect};
use renderer::Vertex;

use std::ffi::CStr;
use std::io::Cursor;
use std::mem;

use glam::{Mat4, Vec2, Vec3};

use super::{core, device, renderer, swapchain};

use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};

// FIXME: Will be the main render class
pub struct Renderer {
    core: core::Core,
    device: device::Device,
    swapchain: swapchain::Swapchain,

    render_pass: vk::RenderPass,
    pipeline: vk::Pipeline,

    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
}

impl Renderer {
    pub fn new(window: &winit::window::Window) -> Self {
        //
        // Core
        //
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

            // TODO: Implement proper gpu selection
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

            //
            // Swapchain
            //
            let swapchain = swapchain::Swapchain::new(&core, &device, window).unwrap();

            //
            // Render pass
            //
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
                    .src_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .build()];

                let render_pass_info = vk::RenderPassCreateInfo::builder()
                    .attachments(&renderpass_attachments)
                    .subpasses(&subpass_desc)
                    .dependencies(&dependencies);

                device
                    .logical_device
                    .create_render_pass(&render_pass_info, None)
                    .unwrap()
            };

            //
            // Pipelines
            //

            // Shader modules
            // TODO: Wrap shader handling
            let (vs_spirv, fs_spirv) = {
                let vs_source = include_str!("../bin/shader/triangle/triangle.vert");
                let fs_source = include_str!("../bin/shader/triangle/triangle.frag");

                let mut compiler = shaderc::Compiler::new().unwrap();

                let vs_spirv = compiler
                    .compile_into_spirv(
                        vs_source,
                        shaderc::ShaderKind::Vertex,
                        "triangle.vert",
                        "main",
                        None,
                    )
                    .unwrap();
                let fs_spirv = compiler
                    .compile_into_spirv(
                        fs_source,
                        shaderc::ShaderKind::Fragment,
                        "triangle.frag",
                        "main",
                        None,
                    )
                    .unwrap();

                (vs_spirv, fs_spirv)
            };

            let vs_module_info = vk::ShaderModuleCreateInfo::builder()
                .code(&util::read_spv(&mut Cursor::new(vs_spirv.as_binary_u8())).unwrap());
            let vs_module = device
                .logical_device
                .create_shader_module(&vs_module_info, None)
                .unwrap();

            let fs_module_info = vk::ShaderModuleCreateInfo::builder()
                .code(&util::read_spv(&mut Cursor::new(fs_spirv.as_binary_u8())).unwrap());
            let fs_module = device
                .logical_device
                .create_shader_module(&fs_module_info, None)
                .unwrap();

            // Shader entry
            let vs_entry = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs_module)
                .name(to_cstr!("main"))
                .build();

            let fs_entry = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fs_module)
                .name(to_cstr!("main"))
                .build();

            // Vertex input / vertex attributes
            // TODO: replace Vertex type
            let vertex_input_bindings = [vk::VertexInputBindingDescription::builder()
                .binding(0)
                .stride(mem::size_of::<Vertex>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX)
                .build()];

            let vertex_input_attributes = [
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: offset_of!(Vertex, position) as u32,
                    ..Default::default()
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 1,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: offset_of!(Vertex, color) as u32,
                    ..Default::default()
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 2,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: offset_of!(Vertex, tex_coord) as u32,
                    ..Default::default()
                },
            ];

            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_binding_descriptions(&vertex_input_bindings)
                .vertex_attribute_descriptions(&vertex_input_attributes);

            // Fixed function
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                .primitive_restart_enable(false);

            let viewport = [vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(swapchain.surface_extent.width as f32)
                .height(swapchain.surface_extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0)
                .build()];

            let scissor = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: swapchain.surface_extent,
            }];

            let dynamic_state_enables = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&dynamic_state_enables);

            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .viewport_count(1)
                .viewports(&viewport)
                .scissor_count(1)
                .scissors(&scissor);

            let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0);

            // TODO: Make config option
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

            let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(
                    vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                )
                .blend_enable(false)
                .build()];

            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op_enable(false)
                .attachments(&color_blend_attachment);

            // Depth stencil
            let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS)
                .depth_bounds_test_enable(false)
                .stencil_test_enable(false);

            // Descriptor set
            let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .descriptor_count(1)
                .build();

            let sampler_layout_binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                .build();

            let layout_bindings = [ubo_layout_binding, sampler_layout_binding];

            let layout_info =
                vk::DescriptorSetLayoutCreateInfo::builder().bindings(&layout_bindings);

            let descriptor_set_layout = device
                .logical_device
                .create_descriptor_set_layout(&layout_info, None)
                .unwrap();

            let desc_set_layouts = [descriptor_set_layout];

            // Graphics pipeline
            // TODO: Collate the above into some sort of default pipeline state if creating multiple pipelines
            let pipeline_layout_info =
                vk::PipelineLayoutCreateInfo::builder().set_layouts(&desc_set_layouts);
            let pipeline_layout = device
                .logical_device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap();

            let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
                .stages(&[vs_entry, fs_entry])
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly)
                .dynamic_state(&dynamic_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&color_blend_state)
                .depth_stencil_state(&depth_stencil_state)
                .layout(pipeline_layout)
                .render_pass(render_pass)
                .subpass(0)
                .build()];

            let pipeline = device
                .logical_device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
                .unwrap();
            
            device.logical_device.destroy_shader_module(vs_module, None);
            device.logical_device.destroy_shader_module(fs_module, None);

            Renderer {
                core,
                device,
                swapchain,
                render_pass,
                descriptor_sets,
                pipeline,
            }
        }
    }
}