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
mod shader;
pub use shader::*;

pub(crate) type Result<T> = anyhow::Result<T>;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct TemplateApp {
    wgpu_callback: WgpuCallback,
    render_state: RenderState,
    shader_dirty: bool,
    show_logger: bool,
    shader_editor: bool,
    shader_content: String,
    #[cfg(not(target_arch = "wasm32"))]
    _vertex_shader_file_watcher: notify::RecommendedWatcher,
    #[cfg(not(target_arch = "wasm32"))]
    vertex_shader_file_watch_rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    #[cfg(not(target_arch = "wasm32"))]
    _fragment_shader_file_watcher: notify::RecommendedWatcher,
    #[cfg(not(target_arch = "wasm32"))]
    fragment_shader_file_watch_rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind_group_layout"),
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
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    vertex_spirv: Cow<'_, [u32]>,
    fragment_spirv: Cow<'_, [u32]>,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let bind_group_layout = create_bind_group_layout(device);
    let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vertex_shader"),
        source: wgpu::ShaderSource::SpirV(vertex_spirv),
    });
    let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fragment_shader"),
        // convert u8 to u32
        source: wgpu::ShaderSource::SpirV(fragment_spirv),
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
            contents: bytemuck::cast_slice(&[0.0f32; 52]),
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
                        std::path::Path::new("src/app/shader.vert"),
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
                        std::path::Path::new("src/app/shader.frag"),
                        notify::RecursiveMode::NonRecursive,
                    )
                    .unwrap();
                fragment_shader_file_watch_rx = rx;
            }
            Self {
                wgpu_callback: WgpuCallback::default(),
                render_state: render_state.clone(),
                shader_dirty: true,
                show_logger: true,
                shader_editor: true,
                shader_content: include_str!("app/default.glsl").to_string(),
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
                show_logger: true,
                shader_editor: false,
                shader_content: include_str!("app/default.glsl").to_string(),
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
    resolution: [f32; 2],
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
        let mut buffer = [0.0f32; 26];
        buffer[0] = self.resolution[0];
        buffer[1] = self.resolution[1];
        queue.write_buffer(
            &resources.uniform_buffer,
            0,
            bytemuck::cast_slice(&buffer[..]),
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
            render_pass.draw(0..6, 0..1);
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
                match (
                    load_vertex_shader(),
                    load_fragment_shader(&self.shader_content),
                ) {
                    (Ok(vertex_spirv), Ok(fragment_spirv)) => {
                        triangle_render_resources.pipeline = Some(create_pipeline(
                            &self.render_state.device,
                            vertex_spirv,
                            fragment_spirv,
                            self.render_state.target_format,
                        ));
                        info!("Shader reloaded successfully");
                    }
                    (Err(vertex_error), _) => {
                        error!("Error loading vertex shader: {}", vertex_error);
                    }
                    (_, Err(fragment_error)) => {
                        error!("Error loading fragment shader: {}", fragment_error,);
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
            ui.horizontal(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                ui.checkbox(&mut self.shader_editor, "Shader Editor");
                ui.checkbox(&mut self.show_logger, "Log");
            });
            if self.shader_editor {
                let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
                let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                    let mut layout_job = egui_extras::syntax_highlighting::highlight(
                        ui.ctx(),
                        ui.style(),
                        &theme,
                        string,
                        "c",
                    );
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };
                egui::ScrollArea::new(egui::Vec2b::new(true, true))
                    .id_salt(Id::new("shader_editor_scroll_area"))
                    .auto_shrink(egui::Vec2b::new(true, true))
                    .max_height(ui.available_height() / 4.0 * 3.0)
                    .show(ui, |ui| {
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut self.shader_content)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .lock_focus(true)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(10)
                                    .layouter(&mut layouter),
                            )
                            .changed()
                        {
                            self.shader_dirty = true;
                        }
                    });
            }
            if self.show_logger {
                egui_logger::logger_ui().show(ui);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // allocate rect as big as possible
            let rect = ui.available_rect_before_wrap();
            let (width, height) = (rect.width(), rect.height());
            //let aspect = 4.0 / 3.0;
            //let (width, height) = if rect.width() / rect.height() > aspect {
            //    (rect.height() * aspect, rect.height())
            //} else {
            //    (rect.width(), rect.width() / aspect)
            //};
            //rect.set_width(width);
            //rect.set_height(height);
            self.wgpu_callback.resolution = [width, height];
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                self.wgpu_callback.clone(),
            ));
        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}
