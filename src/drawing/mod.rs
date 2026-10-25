//! Phase-2 annotation toolkit (pure geometry + rasterize).
//!
//! No window / OS code here — easy to unit-test and study in isolation.

mod raster;
mod shape;
mod tools;

pub use raster::{composite_shapes_bgra, draw_shape_xrgb};
pub use shape::{Point, Shape};
pub use tools::Tool;

/// Red only (Phase 2). Softbuffer `0x00RRGGBB` and BGRA channel forms.
pub const COLOR_RED_XRGB: u32 = 0x00E0_2020;
pub const COLOR_RED_BGRA: [u8; 4] = [0x20, 0x20, 0xE0, 0xFF];

#[derive(Debug, Clone, Default)]
pub struct DrawingState {
    pub tool: Tool,
    pub shapes: Vec<Shape>,
    /// In-progress stroke (freehand points or two-point shapes).
    pub draft: Option<Shape>,
}

impl DrawingState {
    pub fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.draft = None;
    }

    pub fn begin(&mut self, at: Point, ctrl: bool) {
        self.draft = Some(Shape::start(self.tool, at, ctrl));
    }

    pub fn drag(&mut self, at: Point, ctrl: bool) {
        if let Some(draft) = self.draft.as_mut() {
            draft.update(at, ctrl);
        }
    }

    pub fn end(&mut self) {
        if let Some(draft) = self.draft.take() {
            if draft.is_meaningful() {
                self.shapes.push(draft);
            }
        }
    }

    pub fn undo(&mut self) {
        let _ = self.draft.take();
        self.shapes.pop();
    }

    #[allow(dead_code)]
    pub fn cancel_draft(&mut self) {
        self.draft = None;
    }
}
