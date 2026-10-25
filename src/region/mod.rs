//! Region capture session: freeze overlay, selection, primary/secondary flows.

mod overlay;
mod selection;

pub use overlay::RegionOverlay;
pub use selection::{SelectionPhase, SelectionState};

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::WindowId;

use crate::capture::{crop_bgra, BgraImage, MonitorRect};
use crate::config::RegionFlow;

#[cfg(feature = "drawing")]
use crate::drawing::{DrawingState, Point, Shape, Tool};

/// Outcome of a finished (or cancelled) region session.
#[derive(Debug)]
pub enum SessionOutcome {
    Cancelled,
    /// Secondary: cropped image ready (clipboard already or pending).
    SecondaryReady {
        image: BgraImage,
        open_editor: bool,
    },
    /// Primary: user pressed Ctrl+C after drawing.
    PrimaryCopied {
        image: BgraImage,
    },
}

pub struct RegionSession {
    pub flow: RegionFlow,
    pub parts: Vec<(i32, i32, BgraImage)>,
    pub overlay: RegionOverlay,
    pub selection: SelectionState,
    #[cfg(feature = "drawing")]
    pub drawing: DrawingState,
    /// Text entry buffer while Tool::Text draft is active.
    #[cfg(feature = "drawing")]
    text_buffer: String,
    modifiers: ModifiersState,
    last_cursor: PhysicalPosition<f64>,
    last_cursor_window: Option<WindowId>,
}

impl RegionSession {
    pub fn start(
        event_loop: &ActiveEventLoop,
        parts: Vec<(i32, i32, BgraImage)>,
        flow: RegionFlow,
    ) -> Result<Self, String> {
        let overlay = RegionOverlay::show(event_loop, &parts)?;
        Ok(Self {
            flow,
            parts,
            overlay,
            selection: SelectionState::default(),
            #[cfg(feature = "drawing")]
            drawing: DrawingState::default(),
            #[cfg(feature = "drawing")]
            text_buffer: String::new(),
            modifiers: ModifiersState::empty(),
            last_cursor: PhysicalPosition::new(0.0, 0.0),
            last_cursor_window: None,
        })
    }

    pub fn contains(&self, id: WindowId) -> bool {
        self.overlay.contains(id)
    }

    pub fn paint(&mut self, id: WindowId) {
        let sel = self.selection.normalized();
        #[cfg(feature = "drawing")]
        {
            let mut shapes: Vec<&Shape> = self.drawing.shapes.iter().collect();
            if let Some(d) = self.drawing.draft.as_ref() {
                shapes.push(d);
            }
            self.overlay.paint_id(id, &self.parts, sel, &shapes);
        }
        #[cfg(not(feature = "drawing"))]
        {
            self.overlay.paint_id(id, &self.parts, sel);
        }
    }

    pub fn paint_all(&mut self) {
        for id in self.overlay.window_ids() {
            self.paint(id);
        }
    }

    pub fn on_modifiers(&mut self, mods: ModifiersState) {
        self.modifiers = mods;
    }

    pub fn on_cursor_moved(
        &mut self,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
    ) -> bool {
        self.last_cursor = position;
        self.last_cursor_window = Some(window_id);

        let Some(origin) = self.overlay.origin_of(window_id) else {
            return false;
        };
        let desktop = (
            origin.0 + position.x.round() as i32,
            origin.1 + position.y.round() as i32,
        );

        match self.selection.phase {
            SelectionPhase::Dragging => {
                self.selection.drag_to(desktop);
                self.paint_all();
                true
            }
            SelectionPhase::Selected => {
                #[cfg(feature = "drawing")]
                {
                    if matches!(self.flow, RegionFlow::Primary) && self.drawing.draft.is_some() {
                        let local = self.to_selection_local(desktop);
                        let ctrl = self.modifiers.control_key();
                        self.drawing.drag(local, ctrl);
                        self.paint_all();
                        return true;
                    }
                }
                false
            }
            SelectionPhase::Idle => false,
        }
    }

    pub fn on_mouse_input(
        &mut self,
        window_id: WindowId,
        button: MouseButton,
        state: ElementState,
    ) -> Option<SessionOutcome> {
        if button != MouseButton::Left {
            return None;
        }
        let position = if self.last_cursor_window == Some(window_id) {
            self.last_cursor
        } else {
            PhysicalPosition::new(0.0, 0.0)
        };
        let Some(origin) = self.overlay.origin_of(window_id) else {
            return None;
        };
        let desktop = (
            origin.0 + position.x.round() as i32,
            origin.1 + position.y.round() as i32,
        );

        match state {
            ElementState::Pressed => {
                match self.selection.phase {
                    SelectionPhase::Idle => {
                        self.selection.begin(desktop);
                        self.paint_all();
                    }
                    SelectionPhase::Selected => {
                        #[cfg(feature = "drawing")]
                        if matches!(self.flow, RegionFlow::Primary) {
                            let local = self.to_selection_local(desktop);
                            let ctrl = self.modifiers.control_key();
                            self.drawing.begin(local, ctrl);
                            if self.drawing.tool == Tool::Text {
                                self.text_buffer.clear();
                            }
                            self.paint_all();
                        }
                    }
                    SelectionPhase::Dragging => {}
                }
                None
            }
            ElementState::Released => match self.selection.phase {
                SelectionPhase::Dragging => {
                    self.selection.end_drag();
                    let rect = self.selection.normalized()?;
                    if rect.width < 2 || rect.height < 2 {
                        self.selection.reset();
                        self.paint_all();
                        return None;
                    }
                    self.paint_all();

                    match self.flow {
                        RegionFlow::Secondary { open_editor } => match self.crop_selection() {
                            Ok(image) => Some(SessionOutcome::SecondaryReady {
                                image,
                                open_editor,
                            }),
                            Err(_) => Some(SessionOutcome::Cancelled),
                        },
                        RegionFlow::Primary => None,
                    }
                }
                SelectionPhase::Selected => {
                    #[cfg(feature = "drawing")]
                    if matches!(self.flow, RegionFlow::Primary) {
                        if self.drawing.tool != Tool::Text {
                            self.drawing.end();
                        }
                        self.paint_all();
                    }
                    None
                }
                SelectionPhase::Idle => None,
            },
        }
    }

    pub fn on_keyboard(
        &mut self,
        key: &Key,
        state: ElementState,
        text: Option<&str>,
    ) -> Option<SessionOutcome> {
        if state != ElementState::Pressed {
            return None;
        }

        if matches!(key, Key::Named(NamedKey::Escape)) {
            return Some(SessionOutcome::Cancelled);
        }

        if matches!(self.flow, RegionFlow::Primary)
            && matches!(self.selection.phase, SelectionPhase::Selected)
        {
            // Ctrl+C → copy (annotated) crop and finish.
            if self.modifiers.control_key()
                && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("c"))
            {
                return match self.crop_with_drawings() {
                    Ok(image) => Some(SessionOutcome::PrimaryCopied { image }),
                    Err(_) => Some(SessionOutcome::Cancelled),
                };
            }

            #[cfg(feature = "drawing")]
            {
                if self.modifiers.control_key()
                    && matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("z"))
                {
                    self.drawing.undo();
                    self.paint_all();
                    return None;
                }

                if let Key::Character(c) = key {
                    let tool = match c.as_str() {
                        "1" => Some(Tool::Rectangle),
                        "2" => Some(Tool::Line),
                        "3" => Some(Tool::Arrow),
                        "4" => Some(Tool::Circle),
                        "5" => Some(Tool::Freehand),
                        "6" => Some(Tool::Text),
                        _ => None,
                    };
                    if let Some(tool) = tool {
                        self.drawing.select_tool(tool);
                        return None;
                    }
                }

                if self.drawing.tool == Tool::Text && self.drawing.draft.is_some() {
                    if matches!(key, Key::Named(NamedKey::Enter)) {
                        if let Some(draft) = self.drawing.draft.as_mut() {
                            draft.set_text(self.text_buffer.clone());
                        }
                        self.drawing.end();
                        self.text_buffer.clear();
                        self.paint_all();
                        return None;
                    }
                    if matches!(key, Key::Named(NamedKey::Backspace)) {
                        self.text_buffer.pop();
                        if let Some(draft) = self.drawing.draft.as_mut() {
                            draft.set_text(self.text_buffer.clone());
                        }
                        self.paint_all();
                        return None;
                    }
                    if let Some(t) = text {
                        self.text_buffer.push_str(t);
                        if let Some(draft) = self.drawing.draft.as_mut() {
                            draft.set_text(self.text_buffer.clone());
                        }
                        self.paint_all();
                    }
                }
            }
        }

        let _ = text;
        None
    }

    #[cfg(feature = "drawing")]
    fn to_selection_local(&self, desktop: (i32, i32)) -> Point {
        let rect = self.selection.normalized().unwrap_or(MonitorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
        Point::new(desktop.0 - rect.x, desktop.1 - rect.y)
    }

    fn crop_selection(&self) -> Result<BgraImage, String> {
        let rect = self
            .selection
            .normalized()
            .ok_or_else(|| "No selection".to_owned())?;
        crop_bgra(&self.parts, rect)
    }

    fn crop_with_drawings(&self) -> Result<BgraImage, String> {
        let mut image = self.crop_selection()?;
        #[cfg(feature = "drawing")]
        {
            let mut shapes = self.drawing.shapes.clone();
            if let Some(d) = self.drawing.draft.as_ref() {
                shapes.push(d.clone());
            }
            crate::drawing::composite_shapes_bgra(&mut image, &shapes);
        }
        Ok(image)
    }
}
