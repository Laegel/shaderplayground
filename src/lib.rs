#![allow(deprecated)]

mod app;
mod editor;

use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::window::Window;

#[cfg(not(target_os = "android"))]
pub fn run() {
    env_logger::init();
    run_event_loop(winit::event_loop::EventLoop::new().expect("Failed to create event loop"));
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("ShaderPlayground"),
    );
    run_event_loop(
        winit::event_loop::EventLoop::builder()
            .with_android_app(app)
            .build()
            .expect("Failed to create event loop"),
    );
}

fn run_event_loop(event_loop: winit::event_loop::EventLoop<()>) {
    let window = Arc::new(
        event_loop
            .create_window(Window::default_attributes().with_title("Shader Playground"))
            .expect("Failed to create window"),
    );

    let mut app = {
        #[cfg(not(target_os = "android"))]
        {
            Some(pollster::block_on(app::App::new(window.clone())))
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    };

    event_loop
        .run(move |event, target| {
            #[cfg(target_os = "android")]
            match &event {
                Event::Resumed => {
                    if app.is_none() {
                        app = Some(pollster::block_on(app::App::new(window.clone())));
                    }
                    return;
                }
                Event::Suspended => {
                    app = None;
                    return;
                }
                _ => {}
            }

            if let Some(ref mut app) = app {
                if let Event::WindowEvent {
                    event: window_event, ..
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
            }
        })
        .expect("Failed to run");
}
