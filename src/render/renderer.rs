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

pub struct Window {
    window: winit::window::Window,
    surface_loader: Surface,
    surface: vk::SurfaceKHR,
    swapchain_loader: Swapchain,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
}

pub struct Renderer {
    entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,

    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    present_queue: vk::Queue,

    surface: vk::SurfaceKHR,
    surface_format: vk::Format,
    surface_extent: vk::Extent2D,

    swapchain: vk::SwapchainKHR,
    present_images: Vec<vk::Image>,
    present_image_views: Vec<vk::ImageView>,

    surface_loader: Surface,
    swapchain_loader: Swapchain,

    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,

    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

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
    pub fn new(window: &window::Window) -> Result<Renderer, &'static str> {
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
                let vs_source = include_str!("../shader/triangle/triangle.vert");
                let fs_source = include_str!("../shader/triangle/triangle.frag");
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

            // Fixed function
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
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
                .front_face(vk::FrontFace::CLOCKWISE) // FIXME: Make CCW
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

            // Pipeline
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default();
            let pipeline_layout = device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap();

            let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
                .stages(&[vs_entry, fs_entry])
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly)
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

            // Command buffers
            let cmd_pool_info =
                vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index as u32);

            let command_pool = device.create_command_pool(&cmd_pool_info, None).unwrap();

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

                device.cmd_draw(*buffer, 3, 1, 0, 0);

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
                surface_format: surface_format.format,
                surface_extent,
                swapchain,
                present_images,
                present_image_views,
                surface_loader,
                swapchain_loader,
                render_pass,
                pipeline_layout,
                graphics_pipeline: graphics_pipeline[0], // FIXME
                framebuffers,
                command_pool,
                command_buffers,
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

    pub fn recreate_swapchain(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
        }
    }

    pub fn render(&mut self) {
        unsafe {
            let fences = vec![self.in_flight_fences[self.current_frame]];
            self.device.wait_for_fences(&fences, true, std::u64::MAX).unwrap();
            self.device.reset_fences(&fences).unwrap();
            let (image_index, _) = self
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    std::u64::MAX,
                    self.image_available_sems[self.current_frame],
                    vk::Fence::null(),
                )
                .unwrap();

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

            let swapchains = vec![self.swapchain];
            let indices = [image_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices);

            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
                .unwrap();
        }

        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
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
            for f in self.framebuffers.iter() {
                self.device.destroy_framebuffer(*f, None);
            }
            if let Some(ref utils) = self.debug_utils {
                utils.destroy_debug_utils_messenger(self.debug_messenger, None);
            }
            for &v in self.present_image_views.iter() {
                self.device.destroy_image_view(v, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
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
