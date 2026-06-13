use std::sync::Arc;
use std::time::{Duration, Instant};

use wgpu;
use winit::{dpi::PhysicalSize, window::Window};

use crate::editor::Editor;

const DEBOUNCE_MS: u64 = 300;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    time: f32,
    _pad: f32,
    resolution: [f32; 2],
    mouse: [f32; 2],
}

pub struct App {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,

    quad_shader: wgpu::ShaderModule,
    active_pipeline: wgpu::RenderPipeline,
    pipeline_layout: wgpu::PipelineLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,

    time: f32,

    editor: Editor,
    pending_compile: bool,

    pub egui_state: egui_winit::State,
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    clipped_meshes: Vec<egui::epaint::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,

    start_time: Instant,
    frame_count: u32,
    fps_timer: Instant,
    fps: f32,
}

impl App {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .unwrap_or(&caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_vs"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/quad.wgsl"
            ))),
        });

        let default_frag_src = include_str!("../shaders/default.wgsl");
        let default_frag = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("default_frag"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(default_frag_src)),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                }),
            }],
        });

        let active_pipeline = Self::create_pipeline(
            &device,
            &pipeline_layout,
            config.format,
            &quad_shader,
            &default_frag,
        );

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(4096),
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1, false);

        let now = Instant::now();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            quad_shader,
            active_pipeline,
            pipeline_layout,
            bind_group,
            uniform_buffer,
            time: 0.0,
            editor: Editor::new(default_frag_src),
            pending_compile: false,
            egui_state,
            egui_ctx,
            egui_renderer,
            clipped_meshes: Vec::new(),
            textures_delta: egui::TexturesDelta::default(),
            start_time: now,
            frame_count: 0,
            fps_timer: now,
            fps: 0.0,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        format: wgpu::TextureFormat,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shader_pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: vs_module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fs_module,
                entry_point: Some("main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = PhysicalSize::new(width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time);
        self.time = elapsed.as_secs_f32();

        self.frame_count += 1;
        if now.duration_since(self.fps_timer) >= Duration::from_secs(1) {
            self.fps = self.frame_count as f32 / now.duration_since(self.fps_timer).as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = now;
        }
        self.editor.fps = self.fps;

        let uniforms = Uniforms {
            time: self.time,
            _pad: 0.0,
            resolution: [self.config.width as f32, self.config.height as f32],
            mouse: [0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        if self.pending_compile && self.editor.needs_recompile(DEBOUNCE_MS) {
            self.pending_compile = false;
            self.try_compile_shader();
        }

        let scale = self.window.scale_factor() as f32;
        let raw_input = self.egui_state.take_egui_input(&self.window);

        let mut changed = false;
        let full_output = self.egui_ctx.clone().run(raw_input, |ctx| {
            if self.editor.ui(ctx) {
                changed = true;
            }
        });

        if changed {
            self.pending_compile = true;
        }

        let clipped = self.egui_ctx.tessellate(full_output.shapes, scale);
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        self.clipped_meshes = clipped;
        self.textures_delta = full_output.textures_delta;
    }

    fn try_compile_shader(&mut self) {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("user_frag"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&self.editor.source)),
            });

        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = Self::create_pipeline(
            &self.device,
            &self.pipeline_layout,
            self.config.format,
            &self.quad_shader,
            &module,
        );

        let error = pollster::block_on(self.device.pop_error_scope());
        if let Some(err) = error {
            self.editor.error_message = Some(format!("{:#}", err));
        } else {
            self.active_pipeline = pipeline;
            self.editor.error_message = None;
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, delta) in &self.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        for id in &self.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.clipped_meshes,
            &screen_descriptor,
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shader_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.active_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let pass_static = unsafe {
                std::mem::transmute::<&mut wgpu::RenderPass<'_>, &mut wgpu::RenderPass<'static>>(
                    &mut pass,
                )
            };
            self.egui_renderer
                .render(pass_static, &self.clipped_meshes, &screen_descriptor);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}
