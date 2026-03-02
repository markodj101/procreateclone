use crate::canvas::{Canvas, Vertex};
use std::sync::{Arc, Mutex};
use tauri::WebviewWindow;
use wgpu::util::DeviceExt;

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub scale_factor: f64,
    pub zoom: Arc<Mutex<f32>>, // može biti private
    pipeline: wgpu::RenderPipeline,
    cursor_pipeline: wgpu::RenderPipeline,
    resolution_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub async fn new(window: &WebviewWindow, zoom: Arc<Mutex<f32>>) -> Self {
        let size = window.inner_size().unwrap();
        let scale_factor = window.scale_factor().unwrap_or(1.0);
        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();

        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();

        let capabilities = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Uzmi početni zoom iz mutexa
        let initial_zoom = *zoom.lock().unwrap();

        // Uniformni bafer sa 4 vrednosti
        let resolution_data: [f32; 4] = [
            size.width as f32,
            size.height as f32,
            initial_zoom,
            0.0, // padding
        ];
        let resolution_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resolution"),
            contents: bytemuck::cast_slice(&resolution_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: resolution_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stroke"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let cursor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cursor"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_cursor"),
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_cursor"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            scale_factor,
            zoom, // ← sada dodeljujemo
            pipeline,
            cursor_pipeline,
            resolution_buffer,
            bind_group,
        }
    }

    // Nova pomoćna metoda za ažuriranje uniformnog bafera
    fn update_uniform_buffer(&self) {
        let zoom_val = *self.zoom.lock().unwrap();
        let data: [f32; 4] = [
            self.config.width as f32,
            self.config.height as f32,
            zoom_val,
            0.0,
        ];
        self.queue
            .write_buffer(&self.resolution_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.update_uniform_buffer(); // ← ažuriramo celu uniformu
        }
    }

    pub fn render(&self, canvas: &Canvas) -> Result<(), wgpu::SurfaceError> {
        // Pre svakog renderovanja osvežimo uniformu
        self.update_uniform_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });

            pass.set_bind_group(0, &self.bind_group, &[]);

            let verts = canvas.all_vertices();
            if !verts.is_empty() {
                let buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..verts.len() as u32, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }
}
