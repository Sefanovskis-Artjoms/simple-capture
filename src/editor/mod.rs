//! Secondary ShareX-style editor window (egui).
//!
//! Closing without Save is silent (no toast / dialog).

use std::sync::Arc;

use egui::mutex::Mutex;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::app::UserEvent;
use crate::capture::BgraImage;
use crate::gl_window::GlutinWindowContext;

#[cfg(feature = "drawing")]
use crate::drawing::{composite_shapes_bgra, DrawingState, Point, Tool};

pub struct EditorWindow {
    gl_window: GlutinWindowContext,
    gl: Arc<glow::Context>,
    egui_glow: egui_glow::EguiGlow,
    image: BgraImage,
    texture: Option<egui::TextureHandle>,
    #[cfg(feature = "drawing")]
    drawing: DrawingState,
    dirty: bool,
    close_flag: bool,
}

impl EditorWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<UserEvent>,
        image: BgraImage,
    ) -> Result<Self, String> {
        let w = (image.width as f64).clamp(320.0, 1200.0);
        let h = (image.height as f64).clamp(240.0, 800.0) + 56.0;
        let gl_window =
            unsafe { GlutinWindowContext::new(event_loop, "simple-capture — Editor", w, h) };
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let s = std::ffi::CString::new(s).expect("CString");
                gl_window.get_proc_address(&s)
            })
        });
        let egui_glow = egui_glow::EguiGlow::new(event_loop, gl.clone(), None, None, true);
        let proxy = Mutex::new(proxy);
        egui_glow
            .egui_ctx
            .set_request_repaint_callback(move |info| {
                let _ = proxy.lock().send_event(UserEvent::Redraw(info.delay));
            });

        gl_window.window().set_visible(true);
        gl_window.window().focus_window();
        gl_window.window().request_redraw();

        Ok(Self {
            gl_window,
            gl,
            egui_glow,
            image,
            texture: None,
            #[cfg(feature = "drawing")]
            drawing: DrawingState::default(),
            dirty: true,
            close_flag: false,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.gl_window.window().id()
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(size) = event {
            self.gl_window.resize(*size);
        }
        let response = self
            .egui_glow
            .on_window_event(self.gl_window.window(), event);
        if response.repaint {
            self.gl_window.window().request_redraw();
        }
        response.repaint
    }

    pub fn request_redraw(&self) {
        self.gl_window.window().request_redraw();
    }

    pub fn take_close_request(&mut self) -> bool {
        let v = self.close_flag;
        self.close_flag = false;
        v
    }

    pub fn paint(&mut self) {
        let mut save = false;
        let mut close = false;
        let mut undo = false;
        #[cfg(feature = "drawing")]
        let mut tool_pick: Option<Tool> = None;

        // Snapshot image size for UI layout.
        let img_w = self.image.width;
        let img_h = self.image.height;

        // Refresh texture if needed before run (avoids borrow of self.texture in closure via take).
        if self.dirty || self.texture.is_none() {
            let mut rgba = Vec::with_capacity(self.image.bytes.len());
            for px in self.image.bytes.chunks_exact(4) {
                rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
            let color = egui::ColorImage::from_rgba_unmultiplied(
                [img_w as usize, img_h as usize],
                &rgba,
            );
            self.texture = Some(self.egui_glow.egui_ctx.load_texture(
                "editor_capture",
                color,
                Default::default(),
            ));
            self.dirty = false;
        }
        let texture_id = self.texture.as_ref().unwrap().id();

        #[cfg(feature = "drawing")]
        let current_tool = self.drawing.tool;

        let mut drag_start: Option<(f32, f32)> = None;
        let mut drag_pos: Option<(f32, f32)> = None;
        let mut drag_end = false;
        let mut ctrl = false;
        let mut img_rect = egui::Rect::NOTHING;

        self.egui_glow.run(self.gl_window.window(), |ctx| {
            egui::TopBottomPanel::top("editor_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    #[cfg(feature = "drawing")]
                    {
                        for tool in Tool::ALL {
                            if ui
                                .selectable_label(current_tool == tool, tool.label())
                                .clicked()
                            {
                                tool_pick = Some(tool);
                            }
                        }
                        ui.separator();
                        if ui.button("Undo").clicked() {
                            undo = true;
                        }
                    }
                    if ui.button("Save PNG…").clicked() {
                        save = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                let size = egui::vec2(img_w as f32, img_h as f32);
                let (rect, response) =
                    ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                img_rect = rect;
                ui.put(rect, egui::Image::new((texture_id, size)));
                ctrl = ui.input(|i| i.modifiers.ctrl);
                if response.drag_started() {
                    if let Some(p) = response.interact_pointer_pos() {
                        drag_start = Some((p.x, p.y));
                    }
                }
                if response.dragged() {
                    if let Some(p) = response.interact_pointer_pos() {
                        drag_pos = Some((p.x, p.y));
                    }
                }
                if response.drag_stopped() {
                    drag_end = true;
                    if let Some(p) = response.interact_pointer_pos() {
                        drag_pos = Some((p.x, p.y));
                    }
                }
            });
        });

        #[cfg(feature = "drawing")]
        {
            if let Some(tool) = tool_pick {
                self.drawing.select_tool(tool);
            }
            if undo {
                self.drawing.undo();
                self.rebuild_preview();
            }
            let to_pt = |x: f32, y: f32| -> Point {
                let lx = ((x - img_rect.min.x) as i32).clamp(0, img_w.saturating_sub(1) as i32);
                let ly = ((y - img_rect.min.y) as i32).clamp(0, img_h.saturating_sub(1) as i32);
                Point::new(lx, ly)
            };
            if let Some((x, y)) = drag_start {
                self.drawing.begin(to_pt(x, y), ctrl);
            }
            if let Some((x, y)) = drag_pos {
                if self.drawing.draft.is_some() {
                    self.drawing.drag(to_pt(x, y), ctrl);
                }
            }
            if drag_end {
                self.drawing.end();
                self.rebuild_preview();
            }
        }

        if save {
            #[cfg(feature = "drawing")]
            {
                let shapes = self.drawing.shapes.clone();
                composite_shapes_bgra(&mut self.image, &shapes);
                self.drawing.shapes.clear();
                self.dirty = true;
            }
            save_png(&self.image);
        }
        if close {
            self.close_flag = true;
        }

        unsafe {
            use glow::HasContext as _;
            self.gl.clear_color(0.15, 0.15, 0.15, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.egui_glow.paint(self.gl_window.window());
        let _ = self.gl_window.swap_buffers();
    }

    #[cfg(feature = "drawing")]
    fn rebuild_preview(&mut self) {
        // Preview = base image with shapes; we keep base immutable by compositing into texture only.
        // For simplicity, bake shapes into a temp for texture.
        let mut preview = self.image.clone();
        let mut shapes = self.drawing.shapes.clone();
        if let Some(d) = self.drawing.draft.as_ref() {
            shapes.push(d.clone());
        }
        composite_shapes_bgra(&mut preview, &shapes);
        let mut rgba = Vec::with_capacity(preview.bytes.len());
        for px in preview.bytes.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [preview.width as usize, preview.height as usize],
            &rgba,
        );
        self.texture = Some(self.egui_glow.egui_ctx.load_texture(
            "editor_capture",
            color,
            Default::default(),
        ));
    }

    pub fn destroy(&mut self) {
        self.egui_glow.destroy();
    }
}

fn save_png(image: &BgraImage) {
    let path = rfd::FileDialog::new()
        .set_title("Save screenshot")
        .add_filter("PNG", &["png"])
        .set_file_name("capture.png")
        .save_file();
    let Some(path) = path else {
        return;
    };
    let mut rgba = Vec::with_capacity(image.bytes.len());
    for px in image.bytes.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    let _ = image::save_buffer(
        path,
        &rgba,
        image.width,
        image.height,
        image::ColorType::Rgba8,
    );
}
