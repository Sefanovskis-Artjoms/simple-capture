//! Pure selection rectangle state (no windows).

use crate::capture::{DragRect, MonitorRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionPhase {
    #[default]
    Idle,
    Dragging,
    Selected,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    pub phase: SelectionPhase,
    drag: DragRect,
}

impl SelectionState {
    pub fn begin(&mut self, at: (i32, i32)) {
        self.drag = DragRect::from_points(at, at);
        self.phase = SelectionPhase::Dragging;
    }

    pub fn drag_to(&mut self, at: (i32, i32)) {
        self.drag.x1 = at.0;
        self.drag.y1 = at.1;
    }

    pub fn end_drag(&mut self) {
        self.phase = SelectionPhase::Selected;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn normalized(&self) -> Option<MonitorRect> {
        match self.phase {
            SelectionPhase::Idle => None,
            SelectionPhase::Dragging | SelectionPhase::Selected => {
                let r = self.drag.normalized();
                if r.width == 0 || r.height == 0 {
                    None
                } else {
                    Some(r)
                }
            }
        }
    }
}
