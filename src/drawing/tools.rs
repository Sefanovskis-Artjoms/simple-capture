//! Tool selection for the Phase-2 toolkit.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Rectangle,
    Line,
    Arrow,
    Circle,
    Freehand,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Rectangle,
        Tool::Line,
        Tool::Arrow,
        Tool::Circle,
        Tool::Freehand,
        Tool::Text,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Rectangle => "Rect",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Circle => "Circle",
            Tool::Freehand => "Pen",
            Tool::Text => "Text",
        }
    }
}
