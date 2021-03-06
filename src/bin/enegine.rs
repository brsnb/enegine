#[macro_use]
extern crate log;

use enegine::render::renderer;

use winit::{event_loop::EventLoop, window};

fn main() {
    env_logger::init();


    let event_loop = EventLoop::new();
    let window = window::WindowBuilder::new()
        .with_title("enegine")
        .build(&event_loop)
        .unwrap();

    let mut renderer = renderer::Renderer::new(&window).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    info!("Exiting...");
                    *control_flow = winit::event_loop::ControlFlow::Exit
                }
                winit::event::WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => {
                    info!("Exit requested via keypress");
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::WindowEvent::Resized(_dims) => {
                    // TODO: Recreate swapchain
                    println!("WINIT RESIZED");
                    renderer.should_recreate_swapchain = true;
                }
                _ => {}
            },
            winit::event::Event::MainEventsCleared => window.request_redraw(),
            winit::event::Event::RedrawRequested(_) => {
                // TODO: Render
                renderer.render();
            }
            _ => {}
        }
    });
}
