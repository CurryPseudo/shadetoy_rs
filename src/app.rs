use naga::valid::{Capabilities, ValidationFlags, Validator};

use eframe::egui;
use eframe::egui_wgpu;
use eframe::egui_wgpu::RenderState;
use eframe::epaint::PaintCallbackInfo;
use eframe::wgpu::util::DeviceExt;
use egui::panel::Side;
use egui::Id;
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::{CommandBuffer, CommandEncoder, Device, Queue, RenderPass};
use egui_wgpu::{CallbackResources, ScreenDescriptor};
use log::{error, info};
#[cfg(not(target_arch = "wasm32"))]
use notify::Watcher;
use std::borrow::Cow;
use std::path::Path;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct TemplateApp {
    wgpu_callback: WgpuCallback,
    render_state: RenderState,
    shader_dirty: bool,
    show_logger: bool,
    #[cfg(not(target_arch = "wasm32"))]
    _vertex_shader_file_watcher: notify::RecommendedWatcher,
    #[cfg(not(target_arch = "wasm32"))]
    vertex_shader_file_watch_rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    #[cfg(not(target_arch = "wasm32"))]
    _fragment_shader_file_watcher: notify::RecommendedWatcher,
    #[cfg(not(target_arch = "wasm32"))]
    fragment_shader_file_watch_rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
}

fn convert_shader(source: &str, stage: naga::ShaderStage) -> Result<String> {
    let mut parser = naga::front::glsl::Frontend::default();
    let module = parser.parse(
        &naga::front::glsl::Options {
            stage,
            defines: Default::default(),
        },
        source,
    )?;

    // Validate the module
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let _info = validator.validate(&module)?;

    // Convert to WGSL
    let wgsl =
        naga::back::wgsl::write_string(&module, &_info, naga::back::wgsl::WriterFlags::empty())?;

    Ok(wgsl)
}
fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

macro_rules! load_shader {
    ($path:literal, $stage:expr) => {
        Ok(Cow::<'static, str>::from(if cfg!(target_arch = "wasm32") {
            convert_shader(include_str!($path), $stage)?
        } else {
            let path = format!("src/{}", $path);
            convert_shader(&std::fs::read_to_string(&Path::new(&path))?, $stage)?
        }))
    };
}

fn load_vertex_shader() -> Result<Cow<'static, str>> {
    load_shader!("shader.vert", naga::ShaderStage::Vertex)
}
fn load_fragment_shader() -> Result<Cow<'static, str>> {
    load_shader!("shader.frag", naga::ShaderStage::Fragment)
}
fn create_pipeline(
    device: &wgpu::Device,
    vertex_wgsl: Cow<'_, str>,
    fragment_wgsl: Cow<'_, str>,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let bind_group_layout = create_bind_group_layout(device);
    let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vertex_shader"),
        source: wgpu::ShaderSource::Wgsl(vertex_wgsl),
    });
    let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fragment_shader"),
        // convert u8 to u32
        source: wgpu::ShaderSource::Wgsl(fragment_wgsl),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vertex_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &fragment_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            targets: &[Some(target_format.into())],
        }),
        multiview: None,
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        cache: None,
    })
}
impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_logger::builder().init().unwrap();
        let render_state = cc.wgpu_render_state.as_ref().expect("WGPU enabled");

        let device = render_state.device.as_ref();

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.0]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let bind_group_layout = create_bind_group_layout(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(TriangleRenderResources {
                pipeline: None,
                bind_group,
                uniform_buffer,
            });

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut vertex_shader_file_watcher;
            let vertex_shader_file_watch_rx;
            {
                let (tx, rx) = std::sync::mpsc::channel();
                vertex_shader_file_watcher =
                    notify::RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
                vertex_shader_file_watcher
                    .watch(
                        Path::new("src/shader.vert"),
                        notify::RecursiveMode::NonRecursive,
                    )
                    .unwrap();
                vertex_shader_file_watch_rx = rx;
            }
            let mut fragment_shader_file_watcher;
            let fragment_shader_file_watch_rx;
            {
                let (tx, rx) = std::sync::mpsc::channel();
                fragment_shader_file_watcher =
                    notify::RecommendedWatcher::new(tx, notify::Config::default()).unwrap();
                fragment_shader_file_watcher
                    .watch(
                        Path::new("src/shader.frag"),
                        notify::RecursiveMode::NonRecursive,
                    )
                    .unwrap();
                fragment_shader_file_watch_rx = rx;
            }
            Self {
                wgpu_callback: WgpuCallback::default(),
                render_state: render_state.clone(),
                shader_dirty: true,
                show_logger: false,
                _vertex_shader_file_watcher: vertex_shader_file_watcher,
                vertex_shader_file_watch_rx,
                _fragment_shader_file_watcher: fragment_shader_file_watcher,
                fragment_shader_file_watch_rx,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                wgpu_callback: WgpuCallback::default(),
                render_state: render_state.clone(),
                shader_dirty: true,
                show_logger: false,
            }
        }
    }
}

struct TriangleRenderResources {
    pipeline: Option<wgpu::RenderPipeline>,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}
#[derive(Default, Clone)]
struct WgpuCallback {
    angle: f32,
}

impl egui_wgpu::CallbackTrait for WgpuCallback {
    fn prepare(
        &self,
        _device: &Device,
        queue: &Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> {
        let resources: &TriangleRenderResources = callback_resources.get().unwrap();
        queue.write_buffer(
            &resources.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.angle]),
        );
        Vec::new()
    }

    fn finish_prepare(
        &self,
        _device: &Device,
        _queue: &Queue,
        _egui_encoder: &mut CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> {
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut RenderPass<'static>,
        callback_resources: &CallbackResources,
    ) {
        let resources: &TriangleRenderResources = callback_resources.get().unwrap();
        if let Some(pipeline) = &resources.pipeline {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &resources.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}

impl eframe::App for TemplateApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        {
            let mut renderer = self.render_state.renderer.write();

            let triangle_render_resources = renderer
                .callback_resources
                .get_mut::<TriangleRenderResources>()
                .unwrap();
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Ok(Ok(notify::Event {
                    kind: notify::EventKind::Modify(notify::event::ModifyKind::Data(_)),
                    ..
                })) = self.vertex_shader_file_watch_rx.try_recv()
                {
                    info!("Vertex shader file modified");
                    self.shader_dirty = true;
                    while let Ok(Ok(_)) = self.vertex_shader_file_watch_rx.try_recv() {}
                }

                if let Ok(Ok(notify::Event {
                    kind: notify::EventKind::Modify(notify::event::ModifyKind::Data(_)),
                    ..
                })) = self.fragment_shader_file_watch_rx.try_recv()
                {
                    info!("Vertex shader file modified");
                    self.shader_dirty = true;
                    while let Ok(Ok(_)) = self.fragment_shader_file_watch_rx.try_recv() {}
                }
            }
            if self.shader_dirty {
                match (load_vertex_shader(), load_fragment_shader()) {
                    (Ok(vertex_wgsl), Ok(fragment_wgsl)) => {
                        triangle_render_resources.pipeline = Some(create_pipeline(
                            &self.render_state.device,
                            vertex_wgsl,
                            fragment_wgsl,
                            self.render_state.target_format,
                        ));
                        info!("Shader reloaded successfully");
                    }
                    (Err(vertex_error), _) => {
                        error!("Error loading vertex shader: {}", vertex_error);
                    }
                    (_, Err(fragment_error)) => {
                        error!("Error loading fragment shader: {}", fragment_error);
                    }
                }
                self.shader_dirty = false;
            }
        }
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                //egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        egui::SidePanel::new(Side::Right, Id::new("right_panel")).show(ctx, |ui| {
            ui.add(egui::Slider::new(
                &mut self.wgpu_callback.angle,
                0.0..=std::f32::consts::PI,
            ));
            if ui
                .button("Toggle Logger")
                .on_hover_ui(|ui| {
                    ui.label("Toggle the logger");
                })
                .clicked()
            {
                self.show_logger = !self.show_logger;
            };
            if self.show_logger {
                egui_logger::logger_ui().show(ui);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // allocate rect as big as possible
            let rect = ui.available_rect_before_wrap();
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                self.wgpu_callback.clone(),
            ));
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}
