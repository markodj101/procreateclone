use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

pub struct Canvas {
    pub strokes: Vec<Vec<Vertex>>,
    pub current_stroke: Vec<Vertex>,
    pub cursor_pos: [f32; 2],
    pub last_pos: [f32; 2],
    pub is_drawing: bool,
    pub pen_color: [f32; 4],
    pub pen_size: f32,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            strokes: vec![],
            current_stroke: vec![],
            cursor_pos: [0.0, 0.0],
            last_pos: [0.0, 0.0],
            is_drawing: false,
            pen_color: [0.0, 0.0, 0.0, 1.0],
            pen_size: 4.0,
        }
    }

    pub fn start_stroke(&mut self, x: f32, y: f32) {
        self.is_drawing = true;
        self.last_pos = [x, y];
        self.current_stroke.clear();
        self.add_point(x, y);
    }

    pub fn add_point(&mut self, x: f32, y: f32) {
        self.cursor_pos = [x, y];
        if self.is_drawing {
            let lx = self.last_pos[0];
            let ly = self.last_pos[1];
            let dx = x - lx;
            let dy = y - ly;
            let distance = (dx * dx + dy * dy).sqrt();

            let step = (self.pen_size * 0.5).max(1.0);
            let steps = (distance / step).ceil() as u32;
            let steps = steps.max(1);

            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let ix = lx + dx * t;
                let iy = ly + dy * t;
                self.draw_circle(ix, iy);
            }

            self.last_pos = [x, y];
        }
    }

    fn draw_circle(&mut self, x: f32, y: f32) {
        let r = self.pen_size;
        let steps = 12u32;
        for i in 0..steps {
            let a1 = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let a2 = ((i + 1) as f32 / steps as f32) * std::f32::consts::TAU;
            self.current_stroke.push(Vertex {
                position: [x, y],
                color: self.pen_color,
            });
            self.current_stroke.push(Vertex {
                position: [x + a1.cos() * r, y + a1.sin() * r],
                color: self.pen_color,
            });
            self.current_stroke.push(Vertex {
                position: [x + a2.cos() * r, y + a2.sin() * r],
                color: self.pen_color,
            });
        }
    }

    pub fn end_stroke(&mut self) {
        if !self.current_stroke.is_empty() {
            self.strokes.push(self.current_stroke.clone());
        }
        self.current_stroke.clear();
        self.is_drawing = false;
    }

    pub fn clear(&mut self) {
        self.strokes.clear();
        self.current_stroke.clear();
    }

    pub fn all_vertices(&self) -> Vec<Vertex> {
        let mut verts = vec![];
        for stroke in &self.strokes {
            verts.extend_from_slice(stroke);
        }
        verts.extend_from_slice(&self.current_stroke);
        verts
    }
}
