#![allow(deprecated)]

mod app;
mod editor;

use std::sync::Arc;

use winit::event::{Event, WindowEvent};
use winit::window::Window;

fn main() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create event loop");
    let window = Arc::new(
        event_loop
            .create_window(Window::default_attributes().with_title("Shader Playground"))
            .expect("Failed to create window"),
    );

    let mut app = pollster::block_on(app::App::new(window.clone()));

    event_loop
        .run(move |event, target| {
            if let Event::WindowEvent {
                event: window_event,
                ..
            } = &event
            {
                if app
                    .egui_state
                    .on_window_event(&app.window, window_event)
                    .consumed
                {
                    return;
                }
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    target.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    app.resize(size.width, size.height);
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    app.update();
                    match app.render() {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            app.resize(app.size.width, app.size.height);
                        }
                        Err(wgpu::SurfaceError::Outdated) => {}
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
                Event::AboutToWait => {
                    app.window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("Failed to run");
}
