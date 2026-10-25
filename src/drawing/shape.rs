//! Drawable shapes (static once committed).

use super::tools::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone)]
pub enum Shape {
    Rect { a: Point, b: Point },
    Line { a: Point, b: Point },
    Arrow { a: Point, b: Point },
    Circle { center: Point, edge: Point },
    Freehand { points: Vec<Point> },
    Text { at: Point, content: String },
}

impl Shape {
    pub fn start(tool: Tool, at: Point, _ctrl: bool) -> Self {
        match tool {
            Tool::Rectangle => Shape::Rect { a: at, b: at },
            Tool::Line => Shape::Line { a: at, b: at },
            Tool::Arrow => Shape::Arrow { a: at, b: at },
            Tool::Circle => Shape::Circle {
                center: at,
                edge: at,
            },
            Tool::Freehand => Shape::Freehand { points: vec![at] },
            Tool::Text => Shape::Text {
                at,
                content: String::new(),
            },
        }
    }

    pub fn update(&mut self, at: Point, ctrl: bool) {
        match self {
            Shape::Rect { a, b } => {
                *b = if ctrl {
                    constrain_square(*a, at)
                } else {
                    at
                };
            }
            Shape::Line { a, b } | Shape::Arrow { a, b } => {
                *b = if ctrl { constrain_angle(*a, at) } else { at };
            }
            Shape::Circle { center, edge } => {
                *edge = if ctrl {
                    // Perfect circle: edge distance is uniform already when
                    // we rasterize from center; ctrl just locks aspect via
                    // equalizing dx/dy for the bounding box feel.
                    let dx = at.x - center.x;
                    let dy = at.y - center.y;
                    let r = dx.abs().max(dy.abs());
                    Point::new(center.x + r.signum() * r.max(1), center.y)
                } else {
                    at
                };
            }
            Shape::Freehand { points } => {
                if points.last().copied() != Some(at) {
                    points.push(at);
                }
            }
            Shape::Text { at: pos, .. } => {
                *pos = at;
            }
        }
    }

    pub fn is_meaningful(&self) -> bool {
        match self {
            Shape::Rect { a, b } | Shape::Line { a, b } | Shape::Arrow { a, b } => {
                a.x != b.x || a.y != b.y
            }
            Shape::Circle { center, edge } => center.x != edge.x || center.y != edge.y,
            Shape::Freehand { points } => points.len() >= 2,
            Shape::Text { content, .. } => !content.is_empty(),
        }
    }

    pub fn set_text(&mut self, content: String) {
        if let Shape::Text { content: c, .. } = self {
            *c = content;
        }
    }
}

/// Public helper used by callers that want constrained endpoints.
#[allow(dead_code)]
pub fn constrain_end(tool: Tool, start: Point, current: Point, ctrl: bool) -> Point {
    if !ctrl {
        return current;
    }
    match tool {
        Tool::Rectangle => constrain_square(start, current),
        Tool::Line | Tool::Arrow => constrain_angle(start, current),
        Tool::Circle => {
            let dx = current.x - start.x;
            let dy = current.y - start.y;
            let r = dx.abs().max(dy.abs()).max(1);
            Point::new(start.x + dx.signum() * r, start.y)
        }
        _ => current,
    }
}

fn constrain_square(a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let side = dx.abs().max(dy.abs());
    Point::new(a.x + dx.signum() * side, a.y + dy.signum() * side)
}

fn constrain_angle(a: Point, b: Point) -> Point {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return b;
    }
    let angle = dy.atan2(dx);
    // Snap to 45° increments.
    let step = std::f64::consts::FRAC_PI_4;
    let snapped = (angle / step).round() * step;
    Point::new(
        a.x + (snapped.cos() * len).round() as i32,
        a.y + (snapped.sin() * len).round() as i32,
    )
}
