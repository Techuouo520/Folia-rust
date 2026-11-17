// Seated perspective and screen hit testing share the same camera basis as the native shaders.

use iced::{Point, Size};

use super::layout::{SCREEN_HEIGHT, SCREEN_WIDTH, SCREEN_Y, SCREEN_Z, Seat};

#[derive(Debug, Default, Clone, Copy)]
pub struct Head {
    pub yaw: f32,
    pub pitch: f32,
}

impl Head {
    pub fn turn(&mut self, delta: iced::Vector) {
        let sensitivity = 0.18_f32.to_radians();
        self.yaw = (self.yaw - delta.x * sensitivity).clamp(-80_f32.to_radians(), 80_f32.to_radians());
        self.pitch -= delta.y * sensitivity;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub tan_horizontal: f32,
    pub tan_vertical: f32,
}

impl Camera {
    pub fn new(seat: &Seat, head: &mut Head, size: Size) -> Self {
        let eye = seat.eye;
        let toward = [-eye[0], SCREEN_Y - 0.2 - eye[1], -eye[2]];
        let base_yaw = (-toward[0]).atan2(-toward[2]);
        let base_pitch = toward[1].atan2(toward[0].hypot(toward[2]));
        let limit = 89_f32.to_radians();
        head.pitch = head.pitch.clamp(-limit - base_pitch, limit - base_pitch);
        let (yaw, pitch) = (base_yaw + head.yaw, base_pitch + head.pitch);
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let aspect = size.width.max(1.0) / size.height.max(1.0);
        let tan_vertical = (0.7 / aspect).min(0.78);
        Self {
            eye,
            right: [cy, 0.0, -sy],
            up: [sy * sp, cp, cy * sp],
            forward: [-sy * cp, sp, -cy * cp],
            tan_horizontal: tan_vertical * aspect,
            tan_vertical,
        }
    }

    pub fn project(self, world: [f32; 3], size: Size) -> Option<Point> {
        let relative = std::array::from_fn(|index| world[index] - self.eye[index]);
        let depth = dot(relative, self.forward);
        (depth > 0.1).then(|| {
            Point::new(
                (dot(relative, self.right) / (depth * self.tan_horizontal) + 1.0) * size.width * 0.5,
                (1.0 - dot(relative, self.up) / (depth * self.tan_vertical)) * size.height * 0.5,
            )
        })
    }

    /// Maps a pointer ray to the projected content's logical coordinates.
    pub fn screen_point(self, point: Point, window: Size, content: Size) -> Option<Point> {
        self.screen_plane_point(point, window, content)
            .filter(|point| (0.0..=content.width).contains(&point.x) && (0.0..=content.height).contains(&point.y))
    }

    /// Captured content drags continue across the screen edge on the same projection plane.
    pub fn screen_plane_point(self, point: Point, window: Size, content: Size) -> Option<Point> {
        let x = (point.x / window.width * 2.0 - 1.0) * self.tan_horizontal;
        let y = (1.0 - point.y / window.height * 2.0) * self.tan_vertical;
        let direction: [f32; 3] =
            std::array::from_fn(|index| self.forward[index] + self.right[index] * x + self.up[index] * y);
        let distance = (SCREEN_Z - self.eye[2]) / direction[2];
        if !distance.is_finite() || distance <= 0.0 {
            return None;
        }
        let hit_x = self.eye[0] + direction[0] * distance;
        let hit_y = self.eye[1] + direction[1] * distance;
        let u = hit_x / SCREEN_WIDTH + 0.5;
        let v = 0.5 - (hit_y - SCREEN_Y) / SCREEN_HEIGHT;
        Some(Point::new(u * content.width, v * content.height))
    }
}

pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = dot(value, value).sqrt().max(f32::EPSILON);
    value.map(|coordinate| coordinate / length)
}
