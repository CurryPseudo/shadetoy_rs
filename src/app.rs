// Note: The GLSL shaders (shader.vert and shader.frag) must be compiled to SPIR-V before use.
// Use glslangValidator or a similar tool to compile them:
// glslangValidator -V shader.vert -o shader.vert.spv
// glslangValidator -V shader.frag -o shader.frag.spv

use eframe::egui;
use eframe::egui_wgpu;
use eframe::epaint::PaintCallbackInfo;
use eframe::wgpu::util::DeviceExt;
use egui::panel::Side;
use egui::Id;
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::{CommandBuffer, CommandEncoder, Device, Queue, RenderPass};
use egui_wgpu::{CallbackResources, ScreenDescriptor};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)]
    wgpu_callback: WgpuCallback,
}

#[allow(dead_code)]
fn convert_u8_to_u32(input: &[u8]) -> Vec<u32> {
    // 确保输入长度是 4 的倍数
    assert!(input.len() % 4 == 0, "Input length must be a multiple of 4");

    // 使用迭代器进行转换
    (0..input.len() / 4)
        .map(|i| {
            // 将每四个 u8 转换为一个 u32
            u32::from_le_bytes([
                input[i * 4],
                input[i * 4 + 1],
                input[i * 4 + 2],
                input[i * 4 + 3],
            ])
        })
        .collect()
}
impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu_render_state = cc.wgpu_render_state.as_ref().expect("WGPU enabled");

        let device = wgpu_render_state.device.as_ref();
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shader.vert.wgsl").into()
            ),
        });
        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fragment_shader"),
            // convert u8 to u32
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shader.frag.wgsl").into()
            ),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                targets: &[Some(wgpu_render_state.target_format.into())],
            }),
            multiview: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.0]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Because the graphics pipeline must have the same lifetime as the egui render pass,
        // instead of storing the pipeline in our `Custom3D` struct, we insert it into the
        // `paint_callback_resources` type map, which is stored alongside the render pass.
        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(TriangleRenderResources {
                pipeline,
                bind_group,
                uniform_buffer,
            });
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

struct TriangleRenderResources {
    pipeline: wgpu::RenderPipeline,
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
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        egui::SidePanel::new(Side::Left, Id::new("left_panel")).show(ctx, |ui| {
            ui.add(egui::Slider::new(
                &mut self.wgpu_callback.angle,
                0.0..=std::f32::consts::PI,
            ));
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
}
