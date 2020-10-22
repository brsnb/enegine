use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain},
};
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::{util, vk};
use ash_window;

use std::borrow::Cow;
use std::ffi::CStr;
use std::io::Cursor;
use std::mem;

use glam::{Mat4, Vec2, Vec3};

lazy_static! {
    static ref START_TIME: std::time::Instant = std::time::Instant::now();
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: Vec2,
    pub color: Vec3,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct UniformBufferObject {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}

lazy_static! {
    static ref VERTICES: Vec<Vertex> = vec![
        Vertex {
            position: Vec2::new(-0.5, -0.5),
            color: Vec3::new(1.0, 0.0, 0.0)
        },
        Vertex {
            position: Vec2::new(0.5, -0.5),
            color: Vec3::new(0.0, 1.0, 0.0),
        },
        Vertex {
            position: Vec2::new(0.5, 0.5),
            color: Vec3::new(0.0, 0.0, 1.0),
        },
        Vertex {
            position: Vec2::new(-0.5, 0.5),
            color: Vec3::new(1.0, 1.0, 1.0),
        },
    ];
}

static INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

pub struct Renderer {
    entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,

    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    present_queue: vk::Queue,

    surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
    surface_extent: vk::Extent2D,
    present_mode: vk::PresentModeKHR,

    swapchain: vk::SwapchainKHR,
    present_images: Vec<vk::Image>,
    present_image_views: Vec<vk::ImageView>,
    pub should_recreate_swapchain: bool,

    surface_loader: Surface,
    swapchain_loader: Swapchain,

    render_pass: vk::RenderPass,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    vertex_buffer: vk::Buffer,
    vertex_buffer_mem: vk::DeviceMemory,
    index_buffer: vk::Buffer,
    index_buffer_mem: vk::DeviceMemory,
    uniform_buffers: Vec<vk::Buffer>,
    uniform_buffers_mem: Vec<vk::DeviceMemory>,

    texture_image: vk::Image,
    texture_image_mem: vk::DeviceMemory,

    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,

    frames_in_flight: usize,
    current_frame: usize,
    in_flight_fences: Vec<vk::Fence>,
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,

    // Debug
    pub debug_utils: Option<DebugUtils>,
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Renderer {
    // TODO: Don't really need window here, just required exts
    pub fn new(window: &winit::window::Window) -> Result<Renderer, &'static str> {
        let entry = ash::Entry::new().unwrap();

        unsafe {
            // Instance
            let app_name = to_cstr!("enegine");
            let app_info = vk::ApplicationInfo::builder()
                .application_name(app_name)
                .application_version(0)
                .engine_name(app_name)
                .engine_version(0)
                .api_version(vk::make_version(1, 0, 0));

            let mut surface_extensions = ash_window::enumerate_required_extensions(window).unwrap();
            info!("Surface required extensions: {:?}", surface_extensions);

            // Check for debug
            let supported_extensions = entry.enumerate_instance_extension_properties().unwrap();
            let debug_enabled = supported_extensions
                .iter()
                .any(|ext| CStr::from_ptr(ext.extension_name.as_ptr()) == DebugUtils::name());

            if debug_enabled {
                info!("Debug enabled");
                surface_extensions.push(DebugUtils::name());
            } else {
                info!("Debug not available");
            };

            let surface_extensions_raw = surface_extensions
                .iter()
                .map(|ext| ext.as_ptr())
                .collect::<Vec<_>>();

            // Enable validation layers
            let supported_layers = entry.enumerate_instance_layer_properties().unwrap();

            // FIXME
            let validation_layers = to_cstr!("VK_LAYER_KHRONOS_validation");
            let enabled_layers = if supported_layers
                .iter()
                .any(|layer| CStr::from_ptr(layer.layer_name.as_ptr()) == validation_layers)
            {
                [to_cstr!("VK_LAYER_KHRONOS_validation")]
            } else {
                [to_cstr!("")]
            };

            let enabled_layers_raw: Vec<_> =
                enabled_layers.iter().map(|layer| layer.as_ptr()).collect();
            /*
                        if enabled_layers.{
                        } else {
                            return Err("Validation layers requested but are not supported");
                        };
            */
            let mut create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&surface_extensions_raw)
                .enabled_layer_names(&enabled_layers_raw);

            let mut debug_utils_messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback))
                .user_data(std::ptr::null_mut());
            if debug_enabled {
                create_info = create_info.push_next(&mut debug_utils_messenger_info);
            }

            let instance = entry.create_instance(&create_info, None).unwrap();

            let debug_utils;
            let debug_messenger;
            if debug_enabled {
                let utils = DebugUtils::new(&entry, &instance);
                debug_messenger = utils
                    .create_debug_utils_messenger(&debug_utils_messenger_info, None)
                    .unwrap();
                debug_utils = Some(utils);
            } else {
                debug_utils = None;
                debug_messenger = vk::DebugUtilsMessengerEXT::null();
            }

            // Physical device
            let surface = ash_window::create_surface(&entry, &instance, window, None).unwrap();
            let surface_loader = Surface::new(&entry, &instance);
            // FIXME: Only selects for graphics queues that also support the surface/present
            let (physical_device, queue_family_index) = instance
                .enumerate_physical_devices()
                .unwrap()
                .iter()
                .map(|device| {
                    instance
                        .get_physical_device_queue_family_properties(*device)
                        .iter()
                        .enumerate()
                        .filter_map(|(index, ref info)| {
                            let supports_graphics_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *device,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap();
                            if supports_graphics_and_surface {
                                Some((*device, index))
                            } else {
                                None
                            }
                        })
                        .next()
                })
                .filter_map(|v| v)
                .next()
                .unwrap();

            // Logical device
            let prios = [1.0];
            let queue_info = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index as u32)
                .queue_priorities(&prios)
                .build()];

            let device_extensions = [Swapchain::name().as_ptr()];
            let device_features = vk::PhysicalDeviceFeatures::default();

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extensions)
                .enabled_features(&device_features);

            let device = instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap();

            // Queue
            let present_queue = device.get_device_queue(queue_family_index as u32, 0);

            // Swapchain
            let surface_caps = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .unwrap();
            // FIXME: Bad, reliant on window and doesn't have dpi scaling
            //        use surface_caps.min_image_height/width???
            let surface_extent = match surface_caps.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window.inner_size().width,
                    height: window.inner_size().height,
                },
                _ => surface_caps.current_extent,
            };

            let surface_formats = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap();
            let surface_format = *surface_formats
                .iter()
                .find(|&f| {
                    f.format == vk::Format::B8G8R8A8_SRGB
                        && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                })
                .unwrap_or(&surface_formats[0]);

            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .unwrap();
            let present_mode = *present_modes
                .iter()
                .find(|&&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO);

            let mut image_count = surface_caps.min_image_count + 1;
            if surface_caps.max_image_count > 0 && image_count > surface_caps.max_image_count {
                image_count = surface_caps.max_image_count;
            };

            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(image_count)
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(surface_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE) // FIXME: Only if present_queue == graphics queue
                .pre_transform(surface_caps.current_transform) // NOTE: Identity transform?
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|i| {
                    let image_view = vk::ImageViewCreateInfo::builder()
                        .image(*i)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    device.create_image_view(&image_view, None).unwrap()
                })
                .collect();

            // Render pass
            let renderpass_attachment = [vk::AttachmentDescription::builder()
                .format(surface_format.format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .build()];

            let color_attachment_ref = [vk::AttachmentReference::builder()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .build()];

            let subpass = [vk::SubpassDescription::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_attachment_ref)
                .build()];

            let dependency = [vk::SubpassDependency::builder()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .build()];

            let render_pass_info = vk::RenderPassCreateInfo::builder()
                .attachments(&renderpass_attachment)
                .subpasses(&subpass)
                .dependencies(&dependency);

            let render_pass = device.create_render_pass(&render_pass_info, None).unwrap();

            // Shader modules
            // FIXME
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

            let (vs_spirv_bytes, fs_spirv_bytes) =
                (vs_spirv.as_binary_u8(), fs_spirv.as_binary_u8());

            let vs_code = util::read_spv(&mut Cursor::new(vs_spirv_bytes)).unwrap();
            let vs_module_info = vk::ShaderModuleCreateInfo::builder().code(&vs_code);
            let vs_module = device.create_shader_module(&vs_module_info, None).unwrap();

            let fs_code = util::read_spv(&mut Cursor::new(fs_spirv_bytes)).unwrap();
            let fs_module_info = vk::ShaderModuleCreateInfo::builder().code(&fs_code);
            let fs_module = device.create_shader_module(&fs_module_info, None).unwrap();

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

            // Vertex input/attrib
            let vertex_input_bindings = [vk::VertexInputBindingDescription {
                binding: 0,
                stride: mem::size_of::<Vertex>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
                ..Default::default()
            }];

            let vertex_input_attributes = [
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 0,
                    format: vk::Format::R32G32_SFLOAT,
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
                .width(surface_extent.width as f32) // FIXME: Swapchain image size vs surface
                .height(surface_extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0)
                .build()];

            let scissor = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: surface_extent,
            }];

            let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .viewport_count(1)
                .viewports(&viewport)
                .scissor_count(1)
                .scissors(&scissor);

            let rasterizer_info = vk::PipelineRasterizationStateCreateInfo::builder()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false);

            let multisample_info = vk::PipelineMultisampleStateCreateInfo::builder()
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
                .src_color_blend_factor(vk::BlendFactor::ONE)
                .dst_color_blend_factor(vk::BlendFactor::ZERO)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                .alpha_blend_op(vk::BlendOp::ADD)
                .build()];

            let color_blend_info = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op_enable(false)
                .attachments(&color_blend_attachment);

            // Descriptor set
            let ubo_layout_bindings = [vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .descriptor_count(1)
                .build()];

            let layout_info =
                vk::DescriptorSetLayoutCreateInfo::builder().bindings(&ubo_layout_bindings);

            let descriptor_set_layout = device
                .create_descriptor_set_layout(&layout_info, None)
                .unwrap();

            let desc_set_layouts = [descriptor_set_layout];

            // Pipeline
            let pipeline_layout_info =
                vk::PipelineLayoutCreateInfo::builder().set_layouts(&desc_set_layouts);
            let pipeline_layout = device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap();

            let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
                .stages(&[vs_entry, fs_entry])
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly)
                .dynamic_state(&dynamic_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterizer_info)
                .multisample_state(&multisample_info)
                .color_blend_state(&color_blend_info)
                .layout(pipeline_layout)
                .render_pass(render_pass)
                .subpass(0)
                .build()];

            let graphics_pipeline = device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
                .unwrap();

            device.destroy_shader_module(vs_module, None);
            device.destroy_shader_module(fs_module, None);

            // Framebuffer
            let mut framebuffers = Vec::with_capacity(present_image_views.len());

            for &view in present_image_views.iter() {
                let view = [view];
                let framebuffer_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(render_pass)
                    .attachments(&view)
                    .width(surface_extent.width)
                    .height(surface_extent.height)
                    .layers(1);

                framebuffers.push(device.create_framebuffer(&framebuffer_info, None).unwrap());
            }

            let mem_properties = instance.get_physical_device_memory_properties(physical_device);


            // Vertex buffer
            let buffer_size = (mem::size_of::<Vertex>() * VERTICES.len()) as u64;

            let (staging_buffer, staging_buffer_mem) = Renderer::create_buffer(
                &device,
                buffer_size,
                mem_properties,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .unwrap();

            let data = device
                .map_memory(
                    staging_buffer_mem,
                    0,
                    buffer_size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();

            let mut align =
                ash::util::Align::new(data, mem::align_of::<Vertex>() as u64, buffer_size);
            align.copy_from_slice(&VERTICES);
            device.unmap_memory(staging_buffer_mem);

            let (vertex_buffer, vertex_buffer_mem) = Renderer::create_buffer(
                &device,
                buffer_size,
                mem_properties,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .unwrap();

            Renderer::copy_buffer(
                &device,
                present_queue,
                queue_family_index as u32,
                staging_buffer,
                vertex_buffer,
                buffer_size,
            );

            device.destroy_buffer(staging_buffer, None);
            device.free_memory(staging_buffer_mem, None);

            // Index buffer
            let buffer_size = (mem::size_of::<Vertex>() * INDICES.len()) as u64;

            let (staging_buffer, staging_buffer_mem) = Renderer::create_buffer(
                &device,
                buffer_size,
                mem_properties,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .unwrap();

            let data = device
                .map_memory(
                    staging_buffer_mem,
                    0,
                    buffer_size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();

            let mut align = ash::util::Align::new(data, mem::align_of::<u16>() as u64, buffer_size);
            align.copy_from_slice(&INDICES);
            device.unmap_memory(staging_buffer_mem);

            let (index_buffer, index_buffer_mem) = Renderer::create_buffer(
                &device,
                buffer_size,
                mem_properties,
                vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .unwrap();

            Renderer::copy_buffer(
                &device,
                present_queue,
                queue_family_index as u32,
                staging_buffer,
                index_buffer,
                buffer_size,
            );

            device.destroy_buffer(staging_buffer, None);
            device.free_memory(staging_buffer_mem, None);

            // Uniform buffers
            let buffer_size = mem::size_of::<UniformBufferObject>();
            let mut uniform_buffers = Vec::with_capacity(present_images.len());
            let mut uniform_buffers_mem = Vec::with_capacity(present_images.len());

            for _i in 0..present_images.len() {
                let (buf, buf_mem) = Renderer::create_buffer(
                    &device,
                    buffer_size as u64,
                    mem_properties,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )
                .unwrap();
                uniform_buffers.push(buf);
                uniform_buffers_mem.push(buf_mem);
            }

            // Descriptor pool
            let pool_sizes = [vk::DescriptorPoolSize::builder()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(present_images.len() as u32)
                .build()];

            let pool_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&pool_sizes)
                .max_sets(present_images.len() as u32);

            let descriptor_pool = device.create_descriptor_pool(&pool_info, None).unwrap();

            // Descriptor sets
            let mut descriptor_set_layouts = Vec::with_capacity(present_images.len());
            for _i in 0..descriptor_set_layouts.capacity() {
                descriptor_set_layouts.push(descriptor_set_layout);
            }

            let descriptor_set_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&descriptor_set_layouts);

            let descriptor_sets = device
                .allocate_descriptor_sets(&descriptor_set_info)
                .unwrap();

            for (i, s) in descriptor_sets.iter().enumerate() {
                let buffer_info = vk::DescriptorBufferInfo::builder()
                    .buffer(uniform_buffers[i])
                    .offset(0)
                    .range(mem::size_of::<UniformBufferObject>() as u64)
                    .build();

                let descriptor_writes = vk::WriteDescriptorSet::builder()
                    .dst_set(*s)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[buffer_info])
                    .build();

                device.update_descriptor_sets(&[descriptor_writes], &[]);
            }

            // Command pool
            let cmd_pool_info =
                vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index as u32);

            let command_pool = device.create_command_pool(&cmd_pool_info, None).unwrap();

            // Texture image
            // FIXME: Lazy
            let image = image::load_from_memory(include_bytes!("../bin/textures/uv_test.png"))
                .unwrap()
                .to_rgba();
            let image_size = (mem::size_of::<u8>() * image.len()) as u64;

            let (staging_buffer, staging_buffer_mem) = Renderer::create_buffer(
                &device,
                image_size,
                mem_properties,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
            .unwrap();

            let data = device
                .map_memory(
                    staging_buffer_mem,
                    0,
                    image_size,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();

            let mut align = ash::util::Align::new(data, image_size, image_size);
            align.copy_from_slice(&image);
            device.unmap_memory(staging_buffer_mem);

            let (texture_image, texture_image_mem) = Renderer::create_image(
                &device,
                image.width(),
                image.height(),
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                mem_properties,
            )
            .unwrap();

            let transition_buf_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .command_buffer_count(1)
                .level(vk::CommandBufferLevel::PRIMARY);

            let transition_buf = device.allocate_command_buffers(&transition_buf_info).unwrap();

            Renderer::do_single_command(&device, transition_buf[0], present_queue, |device, transition_buf| {
                let barrier = vk::ImageMemoryBarrier::builder()
                    .old_layout(old_layout)
            });

            // Command buffers
            // One buffer for each framebuffer
            let buf_alloc_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .command_buffer_count(present_image_views.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffers = device.allocate_command_buffers(&buf_alloc_info).unwrap();

            for (i, buffer) in command_buffers.iter().enumerate() {
                let buf_begin_info = vk::CommandBufferBeginInfo::default();
                device
                    .begin_command_buffer(*buffer, &buf_begin_info)
                    .unwrap();

                let clear_values = [vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, 0.0, 1.0],
                    },
                }];

                let render_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(render_pass)
                    .framebuffer(framebuffers[i])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: surface_extent,
                    })
                    .clear_values(&clear_values);

                device.cmd_begin_render_pass(
                    *buffer,
                    &render_begin_info,
                    vk::SubpassContents::INLINE,
                );

                device.cmd_bind_pipeline(
                    *buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    graphics_pipeline[0],
                ); // FIXME: graphics_pipeline

                // bind vertex buffer
                let vertex_buffers = vec![vertex_buffer];
                let offsets = vec![0];
                device.cmd_bind_vertex_buffers(*buffer, 0, &vertex_buffers, &offsets);

                // Bind index buffer
                device.cmd_bind_index_buffer(*buffer, index_buffer, 0, vk::IndexType::UINT16);

                // Dynamic state
                device.cmd_set_viewport(*buffer, 0, &viewport);
                device.cmd_set_scissor(*buffer, 0, &scissor);

                // Bind descriptor sets
                device.cmd_bind_descriptor_sets(
                    *buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    0,
                    &[descriptor_sets[i]],
                    &[],
                );

                device.cmd_draw_indexed(*buffer, INDICES.len() as u32, 1, 0, 0, 0);

                device.cmd_end_render_pass(*buffer);

                device.end_command_buffer(*buffer).unwrap();
            }

            let frames_in_flight = 2;

            let semaphore_info = vk::SemaphoreCreateInfo::default();
            let mut image_available_sems = Vec::with_capacity(frames_in_flight);
            let mut render_finished_sems = Vec::with_capacity(frames_in_flight);
            for _ in 0..frames_in_flight {
                image_available_sems.push(device.create_semaphore(&semaphore_info, None).unwrap());
            }
            for _ in 0..frames_in_flight {
                render_finished_sems.push(device.create_semaphore(&semaphore_info, None).unwrap());
            }

            let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
            let mut in_flight_fences = Vec::with_capacity(frames_in_flight);
            for _ in 0..frames_in_flight {
                in_flight_fences.push(device.create_fence(&fence_info, None).unwrap());
            }

            Ok(Renderer {
                entry,
                instance,
                device,
                physical_device,
                queue_family_index: queue_family_index as u32,
                present_queue,
                surface,
                surface_format,
                surface_extent,
                present_mode,
                swapchain,
                present_images,
                present_image_views,
                should_recreate_swapchain: false,
                surface_loader,
                swapchain_loader,
                render_pass,
                descriptor_set_layouts,
                pipeline_layout,
                graphics_pipeline: graphics_pipeline[0], // FIXME
                framebuffers,
                command_pool,
                command_buffers,
                vertex_buffer,
                vertex_buffer_mem,
                index_buffer,
                index_buffer_mem,
                uniform_buffers,
                uniform_buffers_mem,
                descriptor_pool,
                descriptor_sets,
                frames_in_flight,
                current_frame: 0,
                in_flight_fences,
                image_available_sems,
                render_finished_sems,
                debug_utils,
                debug_messenger,
            })
        }
    }

    pub fn render(&mut self) {
        unsafe {
            let fences = vec![self.in_flight_fences[self.current_frame]];
            self.device
                .wait_for_fences(&fences, true, std::u64::MAX)
                .unwrap();
            self.device.reset_fences(&fences).unwrap();
            let (image_index, mut is_suboptimal) = self
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    std::u64::MAX,
                    self.image_available_sems[self.current_frame],
                    vk::Fence::null(),
                )
                .unwrap_or((0, true)); //FIXME: Bad
            if is_suboptimal {
                self.recreate_swapchain();
                self.should_recreate_swapchain = false;
                return;
            }

            // UBO
            //let current_time = std::time::Instant::now();
            //let time = current_time.duration_since(*START_TIME).as_secs();

            let ubo = UniformBufferObject {
                model: glam::Mat4::from_rotation_z(90.0_f32.to_radians()),
                view: glam::Mat4::look_at_lh(
                    Vec3::new(2.0, 2.0, 2.0),
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 1.0),
                ),
                proj: glam::Mat4::perspective_lh(
                    45.0_f32.to_radians(),
                    self.surface_extent.width as f32 / self.surface_extent.height as f32,
                    0.1,
                    10.0,
                ),
            };

            let data = self
                .device
                .map_memory(
                    self.uniform_buffers_mem[image_index as usize],
                    0,
                    mem::size_of::<UniformBufferObject>() as u64,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap();

            let mut align = ash::util::Align::new(
                data,
                mem::align_of::<UniformBufferObject>() as u64,
                mem::size_of::<UniformBufferObject>() as u64,
            );
            align.copy_from_slice(&[ubo]);
            self.device
                .unmap_memory(self.uniform_buffers_mem[image_index as usize]);

            // Semaphore
            let wait_semaphores = [self.image_available_sems[self.current_frame]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal_semaphores = [self.render_finished_sems[self.current_frame]];
            let command_buffer = [self.command_buffers[image_index as usize]];
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffer)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(
                    self.present_queue,
                    &[submit_info.build()],
                    self.in_flight_fences[self.current_frame],
                )
                .unwrap();

            let swapchains = [self.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            is_suboptimal = self
                .swapchain_loader
                .queue_present(self.present_queue, &present_info)
                .unwrap_or(true); //FIXME: Bad

            if is_suboptimal || self.should_recreate_swapchain {
                self.should_recreate_swapchain = false;
                self.recreate_swapchain();
            }
        }

        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;
    }

    // FIXME: Sus
    pub fn recreate_swapchain(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.destroy_swapchain();

            let surface_caps = self
                .surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .unwrap();

            self.surface_extent = match surface_caps.current_extent.width {
                std::u32::MAX => {
                    vk::Extent2D {
                        // FIXME: Awful
                        width: 800,
                        height: 600,
                    }
                }
                _ => surface_caps.current_extent,
            };

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(self.surface)
                .min_image_count(self.present_images.len() as u32) //FIXME: Sus
                .image_format(self.surface_format.format)
                .image_color_space(self.surface_format.color_space)
                .image_extent(self.surface_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE) // FIXME: Only if present_queue == graphics queue
                .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY) // NOTE: Identity transform?
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(self.present_mode)
                .clipped(true);

            self.swapchain = self
                .swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            self.present_images = self
                .swapchain_loader
                .get_swapchain_images(self.swapchain)
                .unwrap();

            self.present_image_views = self
                .present_images
                .iter()
                .map(|i| {
                    let image_view = vk::ImageViewCreateInfo::builder()
                        .image(*i)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    self.device.create_image_view(&image_view, None).unwrap()
                })
                .collect();

            // Framebuffer
            self.framebuffers = Vec::with_capacity(self.present_image_views.len());

            for &view in self.present_image_views.iter() {
                let view = [view];
                let framebuffer_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(self.render_pass)
                    .attachments(&view)
                    .width(self.surface_extent.width)
                    .height(self.surface_extent.height)
                    .layers(1);

                self.framebuffers.push(
                    self.device
                        .create_framebuffer(&framebuffer_info, None)
                        .unwrap(),
                );
            }

            // Uniform buffers
            let mem_properties = self
                .instance
                .get_physical_device_memory_properties(self.physical_device);
            let buffer_size = mem::size_of::<UniformBufferObject>();

            self.uniform_buffers.clear();
            self.uniform_buffers_mem.clear();
            for _i in 0..self.present_images.len() {
                let (buf, buf_mem) = Renderer::create_buffer(
                    &self.device,
                    buffer_size as u64,
                    mem_properties,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )
                .unwrap();
                self.uniform_buffers.push(buf);
                self.uniform_buffers_mem.push(buf_mem);
            }
            // Descriptor pool
            let pool_sizes = [vk::DescriptorPoolSize::builder()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(self.present_images.len() as u32)
                .build()];

            let pool_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&pool_sizes)
                .max_sets(self.present_images.len() as u32);

            self.descriptor_pool = self
                .device
                .create_descriptor_pool(&pool_info, None)
                .unwrap();

            // Descriptor sets
            let descriptor_set_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(&self.descriptor_set_layouts);

            self.descriptor_sets = self
                .device
                .allocate_descriptor_sets(&descriptor_set_info)
                .unwrap();

            for (i, s) in self.descriptor_sets.iter().enumerate() {
                let buffer_info = vk::DescriptorBufferInfo::builder()
                    .buffer(self.uniform_buffers[i])
                    .offset(0)
                    .range(mem::size_of::<UniformBufferObject>() as u64)
                    .build();

                let descriptor_writes = vk::WriteDescriptorSet::builder()
                    .dst_set(*s)
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[buffer_info])
                    .build();

                self.device
                    .update_descriptor_sets(&[descriptor_writes], &[]);
            }

            // One buffer for each framebuffer
            let buf_alloc_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(self.command_pool)
                .command_buffer_count(self.present_image_views.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY);

            self.command_buffers = self
                .device
                .allocate_command_buffers(&buf_alloc_info)
                .unwrap();

            for (i, buffer) in self.command_buffers.iter().enumerate() {
                let buf_begin_info = vk::CommandBufferBeginInfo::default();
                self.device
                    .begin_command_buffer(*buffer, &buf_begin_info)
                    .unwrap();

                let clear_values = [vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.0, 0.0, 0.0, 1.0],
                    },
                }];

                let render_begin_info = vk::RenderPassBeginInfo::builder()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffers[i])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: self.surface_extent,
                    })
                    .clear_values(&clear_values);

                self.device.cmd_begin_render_pass(
                    *buffer,
                    &render_begin_info,
                    vk::SubpassContents::INLINE,
                );

                self.device.cmd_bind_pipeline(
                    *buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.graphics_pipeline,
                );

                // bind vertex buffer
                let vertex_buffers = vec![self.vertex_buffer];
                let offsets = vec![0];
                self.device
                    .cmd_bind_vertex_buffers(*buffer, 0, &vertex_buffers, &offsets);

                // bind index buffer
                self.device.cmd_bind_index_buffer(
                    *buffer,
                    self.index_buffer,
                    0,
                    vk::IndexType::UINT16,
                );

                let viewport = [vk::Viewport::builder()
                    .x(0.0)
                    .y(0.0)
                    .width(self.surface_extent.width as f32) // FIXME: Swapchain image size vs surface
                    .height(self.surface_extent.height as f32)
                    .min_depth(0.0)
                    .max_depth(1.0)
                    .build()];

                let scissor = [vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.surface_extent,
                }];

                // Dynamic state
                self.device.cmd_set_viewport(*buffer, 0, &viewport);
                self.device.cmd_set_scissor(*buffer, 0, &scissor);

                // Bind descriptor sets
                self.device.cmd_bind_descriptor_sets(
                    *buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_sets[i]],
                    &[],
                );

                self.device
                    .cmd_draw_indexed(*buffer, INDICES.len() as u32, 1, 0, 0, 0);

                self.device.cmd_end_render_pass(*buffer);

                self.device.end_command_buffer(*buffer).unwrap();
            }
        }
    }

    // FIXME: Also sus
    fn destroy_swapchain(&mut self) {
        unsafe {
            for f in self.framebuffers.iter() {
                self.device.destroy_framebuffer(*f, None);
            }
            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);
            for i in self.present_image_views.iter() {
                self.device.destroy_image_view(*i, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            for b in self.uniform_buffers.iter() {
                self.device.destroy_buffer(*b, None);
            }
            for m in self.uniform_buffers_mem.iter() {
                self.device.free_memory(*m, None);
            }
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }

    // TODO: At least unwrap_or()
    //       Something like this is a good candidate for a Context struct
    fn create_buffer(
        device: &ash::Device,
        size: vk::DeviceSize,
        physical_props: vk::PhysicalDeviceMemoryProperties,
        usage: vk::BufferUsageFlags,
        props: vk::MemoryPropertyFlags,
    ) -> Option<(vk::Buffer, vk::DeviceMemory)> {
        unsafe {
            let create_info = vk::BufferCreateInfo::builder()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let buffer = device.create_buffer(&create_info, None).unwrap();

            let mem_requirements = device.get_buffer_memory_requirements(buffer);

            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(mem_requirements.size)
                .memory_type_index(
                    find_memorytype_index(&mem_requirements, &physical_props, props).unwrap(),
                );

            let buffer_mem = device.allocate_memory(&alloc_info, None).unwrap();

            device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap();

            Some((buffer, buffer_mem))
        }
    }

    fn do_single_command<D: DeviceV1_0, F: FnOnce(&D, vk::CommandBuffer)>(
        device: &D,
        command_buffer: vk::CommandBuffer,
        queue: vk::Queue,
        f: F,
    ) {
        unsafe {
            device
                .reset_command_buffer(
                    command_buffer,
                    vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                )
                .unwrap();

            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            device
                .begin_command_buffer(command_buffer, &begin_info)
                .unwrap();

            f(device, command_buffer);

            device.end_command_buffer(command_buffer).unwrap();

            let submit_fence = device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .unwrap();

            let command_buffers = [command_buffer];

            let submit_info = vk::SubmitInfo::builder().command_buffers(&command_buffers);

            device
                .queue_submit(queue, &[submit_info.build()], submit_fence)
                .unwrap();
            device
                .wait_for_fences(&[submit_fence], true, std::u64::MAX)
                .unwrap();

            device.destroy_fence(submit_fence, None);
        }
    }

    fn copy_buffer(
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        src: vk::Buffer,
        dst: vk::Buffer,
        size: u64,
    ) {
        unsafe {
            // NOTE: Separate command pool for transient buffers?
            //       would need to store this
            let cmd_pool_info = vk::CommandPoolCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::TRANSIENT);
            let transient_cmd_pool = device.create_command_pool(&cmd_pool_info, None).unwrap();
            let buf_alloc_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(transient_cmd_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let transfer_cmd_buf = device.allocate_command_buffers(&buf_alloc_info).unwrap();

            Renderer::do_single_command(
                device,
                transfer_cmd_buf[0],
                queue,
                |device, transfer_cmd_buf| {
                    let copy_region = [vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size,
                        ..Default::default()
                    }];

                    device.cmd_copy_buffer(transfer_cmd_buf, src, dst, &copy_region);
                },
            );

            // NOTE: Would need to free command buffer if not for this
            device.destroy_command_pool(transient_cmd_pool, None);
        }
    }

    fn create_image(
        device: &ash::Device,
        width: u32,
        height: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        props: vk::MemoryPropertyFlags,
        physical_props: vk::PhysicalDeviceMemoryProperties,
    ) -> Option<(vk::Image, vk::DeviceMemory)> {
        unsafe {
            let image_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .format(format)
                .tiling(tiling)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .samples(vk::SampleCountFlags::TYPE_1);

            let image = device.create_image(&image_info, None).unwrap();

            let mem_requirements = device.get_image_memory_requirements(image);
            let alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(mem_requirements.size)
                .memory_type_index(
                    find_memorytype_index(&mem_requirements, &physical_props, props).unwrap(),
                );

            let image_mem = device.allocate_memory(&alloc_info, None).unwrap();

            device.bind_image_memory(image, image_mem, 0);

            Some((image, image_mem))
        }
    }

    fn transition_image_layout(device: &ash::Device, queue: vk::Queue, queue_family_index: u32, image: vk::Image, format: vk::Format, old_layout: vk::ImageLayout, new_layout: vk::ImageLayout) {
        unsafe {
            // NOTE: Separate command pool for transient buffers?
            //       would need to store this
            let cmd_pool_info = vk::CommandPoolCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::TRANSIENT);
            let transient_cmd_pool = device.create_command_pool(&cmd_pool_info, None).unwrap();

            let transition_buf_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(transient_cmd_pool)
                .command_buffer_count(1)
                .level(vk::CommandBufferLevel::PRIMARY);

            let transition_buf = device.allocate_command_buffers(&transition_buf_info).unwrap();

            Renderer::do_single_command(device, transition_buf[0], queue, |device, transition_buf| {
                let barrier = vk::ImageMemoryBarrier::builder()
                    .old_layout(old_layout)
                    .new_layout(new_layout)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                        ..Default::default()
                    })
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::empty());

                device.cmd_pipeline_barrier(transition_buf,
                    vk::PipelineStageFlags::empty(),
                    vk::PipelineStageFlags::empty(),
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier.build()]);
            });

            device.destroy_command_pool(transient_cmd_pool, None);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.destroy_swapchain();
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);

            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layouts[0], None);

            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_mem, None);
            self.device.destroy_buffer(self.index_buffer, None);
            self.device.free_memory(self.index_buffer_mem, None);
            self.device.destroy_command_pool(self.command_pool, None);
            for s in self.image_available_sems.iter() {
                self.device.destroy_semaphore(*s, None);
            }
            for s in self.render_finished_sems.iter() {
                self.device.destroy_semaphore(*s, None);
            }
            for f in self.in_flight_fences.iter() {
                self.device.destroy_fence(*f, None);
            }
            if let Some(ref utils) = self.debug_utils {
                utils.destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    // Try to find an exactly matching memory flag
    let best_suitable_index =
        find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
            property_flags == flags
        });
    if best_suitable_index.is_some() {
        return best_suitable_index;
    }
    // Otherwise find a memory flag that works
    find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
        property_flags & flags == flags
    })
}

pub fn find_memorytype_index_f<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, ref memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 && f(memory_type.property_flags, flags) {
            return Some(index as u32);
        }
        memory_type_bits >>= 1;
    }
    None
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    info!(
        "{:?}:\n{:?} [{} ({})] : {}\n",
        message_severity,
        message_type,
        message_id_name,
        &message_id_number.to_string(),
        message,
    );

    vk::FALSE
}
