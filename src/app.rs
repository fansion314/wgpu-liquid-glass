use std::{sync::Arc, time::Duration};
use web_time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;

use crate::{
    glass::{GlassParams, panel_width},
    image_view::{self, ImageView},
    renderer::{Effects, ImageMapping, SCENE_FORMAT},
    scene,
};
use anyhow::{Context, Result};
use egui::{Pos2, Rect, Vec2};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

// WebGPU and browser file reads resolve asynchronously on the browser thread.
// Feed their results back through winit rather than blocking JavaScript.
enum AppEvent {
    #[cfg(target_arch = "wasm32")]
    Initialized(Result<Box<State>>),
    #[cfg(target_arch = "wasm32")]
    ImageLoaded { name: String, bytes: Vec<u8> },
    #[cfg(target_arch = "wasm32")]
    ImagePickerClosed,
}

struct State {
    window: Arc<Window>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    effects: Effects,
    scene_ctx: egui::Context,
    ui_ctx: egui::Context,
    scene_renderer: egui_wgpu::Renderer,
    ui_renderer: egui_wgpu::Renderer,
    input: egui_winit::State,
    pictures: Vec<scene::SceneImage>,
    selected_picture: usize,
    image_error: Option<String>,
    image_view: ImageView,
    glass: GlassParams,
    pointer: Pos2,
    physical_pointer: Pos2,
    ui_scale: scene::UiScale,
    drag_offset: Option<Vec2>,
    image_drag: Option<(Pos2, Vec2)>,
    shift_down: bool,
    occluded: bool,
    adapter_name: String,
    #[cfg(target_arch = "wasm32")]
    proxy: EventLoopProxy<AppEvent>,
    #[cfg(target_arch = "wasm32")]
    image_picker_open: bool,
    start: Instant,
    next_repaint: Option<Instant>,
}

impl State {
    async fn new(
        window: Arc<Window>,
        display: winit::event_loop::OwnedDisplayHandle,
        #[cfg(target_arch = "wasm32")] proxy: EventLoopProxy<AppEvent>,
    ) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        #[cfg(target_arch = "wasm32")]
        let instance = {
            let _ = display;
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            wgpu::Instance::new(descriptor)
        };
        let surface = instance
            .create_surface(window.clone())
            .context("无法创建窗口 GPU surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .context("没有可用于此窗口的 GPU，请检查图形驱动与 Metal / Vulkan / D3D12 / 浏览器 WebGPU 支持")?;
        let info = adapter.get_info();
        let adapter_name = format!("{} / {:?}", info.name, info.backend);
        log::info!("GPU: {adapter_name}");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Glass device"),
                ..Default::default()
            })
            .await
            .context("创建 GPU device 失败")?;
        #[cfg(target_arch = "wasm32")]
        device.on_uncaptured_error(Arc::new(|error| {
            log::error!("WebGPU: {error}");
            crate::web::status("error", &format!("WebGPU 渲染失败：{error}"));
        }));
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("GPU surface 没有兼容配置")?;
        // Match egui's gamma-space output when the surface offers a non-sRGB format.
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let effects = Effects::new(
            &device,
            &queue,
            [config.width, config.height],
            config.format,
        );
        let scene_ctx = scene::context();
        let ui_ctx = scene::context();
        ui_ctx.set_zoom_factor(scene::default_ui_scale());
        let pictures = scene::images(
            &scene_ctx,
            device.limits().max_texture_dimension_2d as usize,
        )?;
        let options = egui_wgpu::RendererOptions::default();
        let scene_renderer = egui_wgpu::Renderer::new(&device, SCENE_FORMAT, options);
        let ui_renderer = egui_wgpu::Renderer::new(&device, config.format, options);
        let input = egui_winit::State::new(
            ui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        Ok(Self {
            window,
            instance,
            surface: Some(surface),
            config,
            device,
            queue,
            effects,
            scene_ctx,
            ui_ctx,
            scene_renderer,
            ui_renderer,
            input,
            pictures: Vec::from(pictures),
            selected_picture: scene::DEFAULT_PICTURE,
            image_error: None,
            image_view: ImageView::default(),
            glass: GlassParams::default(),
            pointer: Pos2::ZERO,
            physical_pointer: Pos2::ZERO,
            ui_scale: scene::UiScale::default(),
            drag_offset: None,
            image_drag: None,
            shift_down: false,
            occluded: false,
            adapter_name,
            #[cfg(target_arch = "wasm32")]
            proxy,
            #[cfg(target_arch = "wasm32")]
            image_picker_open: false,
            start: Instant::now(),
            next_repaint: None,
        })
    }

    fn pixels_per_point(&self) -> f32 {
        self.window.scale_factor() as f32 * self.ui_ctx.zoom_factor()
    }

    fn stage(&self) -> Vec2 {
        let size = self.window.inner_size();
        let ppp = self.pixels_per_point();
        Vec2::new(
            (size.width as f32 / ppp - panel_width(size.width as f32 / ppp)).max(1.0),
            (size.height as f32 / ppp).max(1.0),
        )
    }

    fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.effects.resize(&self.device, [size.width, size.height]);
        self.glass.normalize_shape();
    }

    fn restore_surface(&mut self) -> Result<()> {
        self.surface = Some(self.instance.create_surface(self.window.clone())?);
        self.resize();
        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 || self.occluded {
            return Ok(());
        }
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let mut reconfigure = false;
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                reconfigure = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize();
                self.next_repaint = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.restore_surface()?;
                self.next_repaint = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.next_repaint = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("获取窗口纹理时发生 GPU validation 错误")
            }
        };
        self.ui_ctx.set_zoom_factor(self.ui_scale.applied);
        let previous_ppp = self.pixels_per_point();
        let mut open_image = false;
        let mut ui_output = self
            .ui_ctx
            .run_ui(self.input.take_egui_input(&self.window), |ui| {
                open_image |= scene::controls(
                    ui,
                    &mut self.glass,
                    &self.adapter_name,
                    &self.pictures,
                    &mut self.selected_picture,
                    self.image_error.as_deref(),
                    (&mut self.image_view, &mut self.ui_scale),
                );
            });
        let ppp = ui_output.pixels_per_point;
        if ppp != previous_ppp {
            self.drag_offset = None;
            self.image_drag = None;
        }
        self.pointer = self.physical_pointer / ppp;
        let stage = Vec2::new(
            (size.width as f32 / ppp - panel_width(size.width as f32 / ppp)).max(1.0),
            size.height as f32 / ppp,
        );
        self.glass.normalize_shape();
        let mut scene_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(size.width as f32, size.height as f32) / ppp,
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            max_texture_side: Some(self.device.limits().max_texture_dimension_2d as usize),
            ..Default::default()
        };
        scene_input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(ppp);
        let mut scene_output = self.scene_ctx.run_ui(scene_input, |ui| {
            scene::background(
                ui,
                stage,
                &self.pictures[self.selected_picture],
                &self.image_view,
            )
        });
        let scene_jobs = self
            .scene_ctx
            .tessellate(scene_output.shapes, scene_output.pixels_per_point);
        let ui_jobs = self.ui_ctx.tessellate(ui_output.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: ppp,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Glass frame"),
            });
        for (id, deltas) in scene_output.textures_delta.set.drain() {
            for delta in deltas {
                self.scene_renderer
                    .update_texture(&self.device, &self.queue, id, &delta);
            }
        }
        for (id, deltas) in ui_output.textures_delta.set.drain() {
            for delta in deltas {
                self.ui_renderer
                    .update_texture(&self.device, &self.queue, id, &delta);
            }
        }
        let picture = &self.pictures[self.selected_picture];
        let canvas = image_view::canvas(stage);
        let texture = self
            .scene_renderer
            .texture(&picture.texture.id())
            .and_then(|t| t.texture.as_ref())
            .expect("selected image uploaded");
        self.effects.set_image(
            &self.device,
            &self.queue,
            texture,
            ImageMapping::new(
                self.image_view.image_rect(canvas, picture.dimensions),
                canvas,
            ),
        );
        let mut commands = self.scene_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &scene_jobs,
            &screen,
        );
        commands.extend(self.ui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &ui_jobs,
            &screen,
        ));
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Background text, picture and geometry"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.effects.scene.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.scene_renderer.render(&mut pass, &scene_jobs, &screen);
        }
        let output_view = frame.texture.create_view(&Default::default());
        self.effects.render(
            &self.queue,
            &mut encoder,
            &output_view,
            &self.glass,
            ppp,
            stage.x,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Sharp parameter panel"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.ui_renderer.render(&mut pass, &ui_jobs, &screen);
        }
        commands.push(encoder.finish());
        self.queue.submit(commands);
        self.window.pre_present_notify();
        self.queue.present(frame);
        // A swapchain must not be configured while its acquired frame is alive.
        if reconfigure {
            self.resize();
        }
        for id in scene_output.textures_delta.free.drain() {
            self.scene_renderer.free_texture(&id);
        }
        for id in ui_output.textures_delta.free.drain() {
            self.ui_renderer.free_texture(&id);
        }
        self.input
            .handle_platform_output(&self.window, ui_output.platform_output);
        if self.pointer.x < stage.x {
            self.window
                .set_cursor(if self.drag_offset.is_some() || self.image_drag.is_some() {
                    winit::window::CursorIcon::Grabbing
                } else if (!self.image_view.edit_image
                    && !self.shift_down
                    && self.glass.contains(self.pointer))
                    || image_view::canvas(stage).contains(self.pointer)
                {
                    winit::window::CursorIcon::Grab
                } else {
                    winit::window::CursorIcon::Default
                });
        }
        let delay = ui_output
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::MAX);
        self.next_repaint = Instant::now().checked_add(delay);
        if self.ui_scale.applied != self.ui_ctx.zoom_factor() {
            self.window.request_redraw();
        }
        if open_image {
            self.open_image();
            self.window.request_redraw();
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_image(&mut self) {
        self.drag_offset = None;
        self.image_drag = None;
        self.image_error = None;
        // Open the native dialog only after presenting the frame, so modal
        // window events cannot invalidate an acquired swapchain texture.
        let Some(path) = rfd::FileDialog::new()
            .set_parent(self.window.as_ref())
            .set_title("选择背景图片")
            .add_filter("图片", &["jpg", "jpeg", "png", "webp", "bmp"])
            .pick_file()
        else {
            return;
        };
        let picture = scene::local_image(
            &self.scene_ctx,
            self.device.limits().max_texture_dimension_2d as usize,
            &path,
        );
        self.set_user_image(picture);
    }

    #[cfg(target_arch = "wasm32")]
    fn open_image(&mut self) {
        if self.image_picker_open {
            return;
        }
        self.drag_offset = None;
        self.image_drag = None;
        self.image_error = None;
        self.image_picker_open = true;
        let proxy = self.proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("选择背景图片")
                .add_filter("图片", &["jpg", "jpeg", "png", "webp", "bmp"])
                .pick_file()
                .await
            {
                let name = file.file_name();
                let bytes = file.read().await;
                let _ = proxy.send_event(AppEvent::ImageLoaded { name, bytes });
            }
            let _ = proxy.send_event(AppEvent::ImagePickerClosed);
        });
    }

    fn set_user_image(&mut self, picture: Result<scene::SceneImage>) {
        match picture {
            Ok(picture) => {
                log::info!(
                    "Loaded picture: {} / {} × {}",
                    picture.title,
                    picture.dimensions[0],
                    picture.dimensions[1]
                );
                if self.pictures.len() > scene::BUILTIN_PICTURES {
                    self.pictures[scene::BUILTIN_PICTURES] = picture;
                } else {
                    self.pictures.push(picture);
                }
                self.selected_picture = scene::BUILTIN_PICTURES;
                self.image_view.reset();
            }
            Err(error) => self.image_error = Some(format!("无法打开图片：{error:#}")),
        }
    }

    fn event(&mut self, event: &WindowEvent) {
        let response = self.input.on_window_event(&self.window, event);
        match event {
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.resize(),
            WindowEvent::CursorMoved { position, .. } => {
                let ppp = self.pixels_per_point();
                self.physical_pointer = Pos2::new(position.x as f32, position.y as f32);
                self.pointer = self.physical_pointer / ppp;
                if let Some(offset) = self.drag_offset {
                    self.glass.center = self.pointer - offset;
                    self.glass.normalize_shape();
                }
                if let Some((start, pan)) = self.image_drag {
                    self.image_view.pan = pan + (self.pointer - start);
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                if *state == ElementState::Released {
                    self.drag_offset = None;
                    self.image_drag = None;
                } else if self.pointer.x < self.stage().x && !self.ui_ctx.egui_is_using_pointer() {
                    if !self.shift_down
                        && !self.image_view.edit_image
                        && self.glass.contains(self.pointer)
                    {
                        self.drag_offset = Some(self.pointer - self.glass.center);
                    } else if image_view::canvas(self.stage()).contains(self.pointer) {
                        self.image_drag = Some((self.pointer, self.image_view.pan));
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.shift_down = modifiers.state().shift_key()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let area = image_view::canvas(self.stage());
                if area.contains(self.pointer) && !response.consumed {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y * 0.12,
                        MouseScrollDelta::PixelDelta(position) => {
                            position.y as f32 / self.pixels_per_point() * 0.002
                        }
                    };
                    self.image_view.zoom_at(amount.exp(), self.pointer, area);
                }
            }
            WindowEvent::Focused(false) => {
                self.drag_offset = None;
                self.image_drag = None;
                self.shift_down = false;
            }
            WindowEvent::Occluded(occluded) => self.occluded = *occluded,
            _ => {}
        }
    }
}

struct App {
    state: Option<State>,
    failure: Option<anyhow::Error>,
    #[cfg(target_arch = "wasm32")]
    proxy: EventLoopProxy<AppEvent>,
    #[cfg(target_arch = "wasm32")]
    initializing: bool,
}

impl App {
    fn new(_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            state: None,
            failure: None,
            #[cfg(target_arch = "wasm32")]
            proxy: _proxy,
            #[cfg(target_arch = "wasm32")]
            initializing: false,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        log::error!("{error:#}");
        #[cfg(target_arch = "wasm32")]
        crate::web::status("error", &format!("启动或渲染失败：{error:#}"));
        self.failure = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            if let Err(error) = state.restore_surface() {
                self.fail(event_loop, error);
            } else {
                state.window.request_redraw();
            }
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if self.initializing {
            return;
        }
        let attributes = Window::default_attributes().with_title("玻璃实验室 · wgpu");
        #[cfg(not(target_arch = "wasm32"))]
        let attributes = attributes
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(LogicalSize::new(720.0, 520.0));
        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            match crate::web::canvas() {
                Ok(canvas) => attributes.with_canvas(Some(canvas)),
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        };
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };
        let display = event_loop.owned_display_handle();
        #[cfg(not(target_arch = "wasm32"))]
        match pollster::block_on(State::new(window, display)) {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(error) => self.fail(event_loop, error),
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.initializing = true;
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = State::new(window, display, proxy.clone())
                    .await
                    .map(Box::new);
                let _ = proxy.send_event(AppEvent::Initialized(result));
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            #[cfg(target_arch = "wasm32")]
            AppEvent::Initialized(result) => {
                self.initializing = false;
                match result {
                    Ok(state) => {
                        state.window.request_redraw();
                        self.state = Some(*state);
                    }
                    Err(error) => self.fail(_event_loop, error),
                }
            }
            #[cfg(target_arch = "wasm32")]
            AppEvent::ImageLoaded { name, bytes } => {
                if let Some(state) = &mut self.state {
                    let picture = scene::memory_image(
                        &state.scene_ctx,
                        state.device.limits().max_texture_dimension_2d as usize,
                        &bytes,
                        &name,
                    );
                    state.set_user_image(picture);
                    state.window.request_redraw();
                }
            }
            #[cfg(target_arch = "wasm32")]
            AppEvent::ImagePickerClosed => {
                if let Some(state) = &mut self.state {
                    state.image_picker_open = false;
                    state.window.request_redraw();
                }
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            state.surface = None;
            state.drag_offset = None;
            state.image_drag = None;
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = state.render() {
                    self.fail(event_loop, error);
                } else {
                    #[cfg(target_arch = "wasm32")]
                    crate::web::status("ready", "");
                }
            }
            _ => {
                state.event(&event);
                state.window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Some(state) = &mut self.state
            && !state.occluded
            && state.surface.is_some()
            && let Some(when) = state.next_repaint
        {
            if when <= Instant::now() {
                state.next_repaint = None;
                state.window.request_redraw();
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(when));
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("hello_wgpu=info,wgpu=warn"),
    )
    .init();
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn run_web() -> Result<()> {
    use winit::platform::web::EventLoopExtWebSys;
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let app = App::new(event_loop.create_proxy());
    event_loop.spawn_app(app);
    Ok(())
}
