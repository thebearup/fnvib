use serde::{Deserialize, Serialize};

pub const PIECE_SIZE: f32 = 256.0; // units per room piece

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Cardinal {
    North,
    South,
    East,
    West,
}

impl Cardinal {
    pub fn opposite(self) -> Self {
        match self {
            Cardinal::North => Cardinal::South,
            Cardinal::South => Cardinal::North,
            Cardinal::East  => Cardinal::West,
            Cardinal::West  => Cardinal::East,
        }
    }
}

/// A doorway punched through one wall of the room.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Doorway {
    pub direction: Cardinal,
    pub offset: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub links_to: Option<String>,
}

/// A single interior space described by its room-piece dimensions and kit.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub kit: String,
    pub width: u32,
    pub length: u32,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub doorways: Vec<Doorway>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub furniture: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub floor_clutter: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub surface_clutter: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wall_decorations: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub lights: Vec<String>,
    /// Grid position set by the UI editor; not used during generation.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grid_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub grid_y: Option<i32>,
}

impl Room {
    pub fn width_units(&self) -> f32 {
        self.width as f32 * PIECE_SIZE
    }

    pub fn length_units(&self) -> f32 {
        self.length as f32 * PIECE_SIZE
    }

    /// Returns doorways on a given wall.
    pub fn doorways_on(&self, dir: Cardinal) -> impl Iterator<Item = &Doorway> {
        self.doorways.iter().filter(move |d| d.direction == dir)
    }
}
