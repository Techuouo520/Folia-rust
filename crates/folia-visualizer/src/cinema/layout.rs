// Cinema dimensions and seat anchors ported from the original cinemaLayout and cinemaAnchors.

use std::sync::OnceLock;

use folia_core::RoomListener;

pub const SCREEN_WIDTH: f32 = 14.0;
pub const SCREEN_HEIGHT: f32 = 7.57;
pub const SCREEN_Y: f32 = 4.885;
pub const SCREEN_Z: f32 = 0.02;
pub const SCREEN_ASPECT: f32 = SCREEN_WIDTH / SCREEN_HEIGHT;
pub const ROWS: usize = 14;
pub const COLUMNS: usize = 26;
pub const SPEAKERS: [[f32; 3]; 2] = [[-6.3, 6.146_666_5, 0.3], [6.3, 6.146_666_5, 0.3]];

#[derive(Debug, Clone)]
pub struct Seat {
    pub id: String,
    pub row: u8,
    pub number: u8,
    pub eye: [f32; 3],
    pub occupied: bool,
    pub preferred: bool,
}

pub fn seats() -> &'static [Seat] {
    static SEATS: OnceLock<Vec<Seat>> = OnceLock::new();
    SEATS.get_or_init(|| {
        let mut xs = Vec::with_capacity(COLUMNS);
        for index in 0..7 {
            let x = 0.28 + index as f64 * 0.56;
            xs.extend([-x, x]);
        }
        for index in 0..6 {
            let x = 5.08 + 0.28 + index as f64 * 0.56;
            xs.extend([-x, x]);
        }
        xs.sort_by(f64::total_cmp);
        let mut seats = Vec::with_capacity(ROWS * COLUMNS);
        for row in 0..ROWS {
            for (index, x) in xs.iter().copied().enumerate() {
                let radius = 40.0 + row as f64 * 1.05;
                let z = 5.6 - 40.0 + (radius * radius - x * x).sqrt() - 0.3;
                let floor = (row as f64 - 2.0).max(0.0) * 0.32;
                let hash = (row as f64 * 127.1 + x * 311.7).sin() * 43_758.545_3;
                let occupied = row < ROWS - 1 && row != 6 && row != 7 && hash - hash.floor() <= 0.34;
                seats.push(Seat {
                    id: format!("R{}-{}", row + 1, index + 1),
                    row: row as u8 + 1,
                    number: index as u8 + 1,
                    eye: [x as f32, (floor + 1.15) as f32, z as f32],
                    occupied,
                    preferred: false,
                });
            }
        }
        let preferred = seats
            .iter()
            .enumerate()
            .filter(|(_, seat)| seat.row == 10 && !seat.occupied)
            .min_by(|(_, a), (_, b)| a.eye[0].abs().total_cmp(&b.eye[0].abs()))
            .map(|(index, _)| index);
        if let Some(index) = preferred {
            seats[index].preferred = true;
        }
        seats
    })
}

pub fn selected_seat(viewpoint: Option<&str>) -> &'static Seat {
    let seats = seats();
    seats
        .iter()
        .find(|seat| Some(seat.id.as_str()) == viewpoint && !seat.occupied)
        .or_else(|| seats.iter().find(|seat| seat.preferred))
        .unwrap_or(&seats[0])
}

pub fn listener(viewpoint: Option<&str>) -> RoomListener {
    let position = selected_seat(viewpoint).eye;
    let direction = [-position[0], SCREEN_Y - 0.2 - position[1], -position[2]];
    RoomListener { position, forward: super::camera::normalize(direction), up: [0.0, 1.0, 0.0] }
}
