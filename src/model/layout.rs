use serde::{Deserialize, Serialize};
use super::room::Room;

/// A complete interior layout: one or more rooms linked by doorways.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layout {
    /// In-game display name shown in the UI and loading screen.
    pub name: String,
    /// Editor ID written to EDID and used as the file stem.
    /// If empty, derived from `name` (lowercase, spaces → underscores).
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub description: String,
    pub rooms: Vec<Room>,
}

impl Layout {
    /// Returns the editor ID, deriving one from `name` when `id` is empty.
    pub fn editor_id(&self) -> String {
        if self.id.is_empty() {
            self.name.to_lowercase().replace(' ', "_")
        } else {
            self.id.clone()
        }
    }
}

impl Layout {
    /// Look up a room by its ID, returning None if not found.
    pub fn room(&self, id: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    /// Validate referential integrity: every `links_to` must name a real room ID.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for room in &self.rooms {
            for door in &room.doorways {
                if let Some(ref target) = door.links_to {
                    if self.room(target).is_none() {
                        errors.push(format!(
                            "room '{}': doorway links_to '{}' which does not exist",
                            room.id, target
                        ));
                    }
                }
            }
        }
        errors
    }
}
