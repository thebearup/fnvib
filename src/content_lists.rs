use crate::kits::KitData;
use serde::Deserialize;
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::Path;

/// Jitter specification in TOML.
/// A scalar applies to Z only (yaw); an array gives per-axis [rx, ry, rz] control.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum JitterToml {
    Scalar(f32),
    Axes([f32; 3]),
}

impl JitterToml {
    fn to_axes(self) -> [f32; 3] {
        match self {
            Self::Scalar(v) => [0.0, 0.0, v],
            Self::Axes(a)   => a,
        }
    }
}

// ── Piece classification ──────────────────────────────────────────────────────

/// Coarse piece classification used by placement filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceClass {
    Interior,
    Wall,
    Corner,
    Doorway,
}

/// Controls which piece types an item may occupy.
#[derive(Debug, Clone)]
pub struct PieceMask {
    pub interior: bool,
    pub wall: bool,
    pub corner: bool,
    // doorway is always false — items never block a door opening
}

impl PieceMask {
    pub fn anywhere() -> Self { Self { interior: true, wall: true, corner: false } }
    pub fn wall_only() -> Self { Self { interior: false, wall: true, corner: false } }
    pub fn interior_only() -> Self { Self { interior: true, wall: false, corner: false } }
    pub fn corner_only() -> Self { Self { interior: false, wall: false, corner: true } }
    pub fn wall_and_corner() -> Self { Self { interior: false, wall: true, corner: true } }

    pub fn allows(&self, class: PieceClass) -> bool {
        match class {
            PieceClass::Interior => self.interior,
            PieceClass::Wall     => self.wall,
            PieceClass::Corner   => self.corner,
            PieceClass::Doorway  => false,
        }
    }
}

// ── Runtime types (used by generators) ───────────────────────────────────────

/// A pool of objects scattered randomly on the parent item's surface.
#[derive(Debug, Clone)]
pub struct SurfaceScatter {
    /// Name of the content list to draw items from.
    pub list: String,
    /// Number of items to place.
    pub count: usize,
    /// Height of the surface above the floor (absolute z).
    pub z_offset: f32,
    /// Half-width of the scatter zone along the parent's right axis (along the wall).
    pub x_range: f32,
    /// Half-depth of the scatter zone along the parent's forward axis (into the room).
    pub y_range: f32,
}

/// An object placed at a fixed offset relative to a parent item.
#[derive(Debug, Clone)]
pub struct Companion {
    pub form_id: u32,
    pub z_offset: f32,
    /// Distance along the direction the parent faces.
    pub forward: f32,
    /// Distance to the parent's right (perpendicular to facing direction).
    pub right: f32,
    /// Yaw rotation relative to the parent's final rotation (radians).
    pub rot_offset: f32,
    pub surface_scatter: Vec<SurfaceScatter>,
}

/// A placeable object with full positioning and pairing metadata.
#[derive(Debug, Clone)]
pub struct ContentItem {
    pub form_id: u32,
    pub z_offset: f32,
    /// Fixed base rotation (rx, ry, rz) in radians applied before wall
    /// alignment and jitter. Use to correct a model's default orientation.
    /// Note: this is a visual correction only — companion positions are computed
    /// from the semantic wall-inward direction, not from base_rot.
    pub base_rot: (f32, f32, f32),
    /// Per-axis jitter [jx, jy, jz]. Scalar TOML value sets Z only; array sets each axis.
    pub jitter: [f32; 3],
    pub allowed: PieceMask,
    /// Rotate to face inward from the wall when placed on a wall piece.
    pub align_to_wall: bool,
    /// Distance to push the item from the piece centre toward the outer wall face.
    /// 64 (default) = near the wall surface; 0 = piece centre; 128 = at the
    /// outer face. Only used for wall-aligned items.
    pub wall_depth: f32,
    pub companions: Vec<Companion>,
    pub surface_scatter: Vec<SurfaceScatter>,
}

// ── TOML deserialization types ────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum PlacementToml {
    /// Interior and straight wall pieces (default).
    #[default]
    Anywhere,
    /// Wall pieces only; item auto-rotates to face inward.
    Walls,
    /// Non-perimeter interior pieces only.
    Interior,
    /// Corner pieces only.
    Corners,
    /// Both straight wall and corner pieces; item auto-rotates to face inward,
    /// using the corner's backing wall for orientation.
    #[serde(rename = "walls_and_corners")]
    WallsAndCorners,
}

#[derive(Deserialize)]
struct SurfaceScatterToml {
    list: String,
    #[serde(default = "default_scatter_count")]
    count: usize,
    #[serde(default)]
    z_offset: f32,
    #[serde(default)]
    x_range: f32,
    #[serde(default)]
    y_range: f32,
}

#[derive(Deserialize)]
struct CompanionToml {
    form_id: u32,
    #[serde(default)]
    z_offset: f32,
    #[serde(default)]
    forward: f32,
    #[serde(default)]
    right: f32,
    #[serde(default)]
    rot_offset: f32,
    #[serde(default)]
    surface_scatter: Vec<SurfaceScatterToml>,
}

#[derive(Deserialize)]
struct ItemToml {
    list: String,
    form_id: u32,
    #[serde(default)]
    z_offset: f32,
    /// Base rotation as [rx, ry, rz] in radians.
    #[serde(default)]
    base_rot: [f32; 3],
    #[serde(default = "default_jitter")]
    jitter: JitterToml,
    #[serde(default)]
    placement: PlacementToml,
    /// Distance from the wall edge. Default 128 = piece centre.
    #[serde(default = "default_wall_depth")]
    wall_depth: f32,
    #[serde(default)]
    companions: Vec<CompanionToml>,
    #[serde(default)]
    surface_scatter: Vec<SurfaceScatterToml>,
}

fn default_jitter() -> JitterToml { JitterToml::Scalar(TAU) }
fn default_wall_depth() -> f32 { 64.0 }
fn default_scatter_count() -> usize { 1 }
fn default_wall_doorway_rot_adj() -> f32 { std::f32::consts::FRAC_PI_2 }

/// TOML representation of a single kit definition ([[kits]] entry).
#[derive(Deserialize)]
struct KitToml {
    name: String,
    #[serde(default)]
    description: String,
    room_height: f32,
    room_mid: u32,
    wall_straight: u32,
    #[serde(default)]
    wall_straight_ew: Option<u32>,
    wall_corner_inner: u32,
    #[serde(default)]
    wall_corner_inner_b: Option<u32>,
    wall_corner_door_l: u32,
    wall_corner_door_r: u32,
    #[serde(default)]
    wall_corner_door_l_b: Option<u32>,
    #[serde(default)]
    wall_corner_door_r_b: Option<u32>,
    wall_doorway: u32,
    #[serde(default)]
    wall_doorway_ew: Option<u32>,
    #[serde(default)]
    wall_doorway_ew_rot_adj: Option<f32>,
    #[serde(default)]
    room_mid_rot_adj: f32,
    #[serde(default)]
    wall_straight_rot_adj: f32,
    #[serde(default)]
    wall_straight_ew_rot_adj: Option<f32>,
    #[serde(default = "default_wall_doorway_rot_adj")]
    wall_doorway_rot_adj: f32,
    #[serde(default)]
    wall_corner_rot_adj: f32,
    #[serde(default)]
    wall_corner_inner_b_rot_adj: Option<f32>,
    #[serde(default)]
    wall_corner_door_l_rot_adj: Option<f32>,
    #[serde(default)]
    wall_corner_door_l_offset: f32,
    #[serde(default)]
    wall_corner_door_r_rot_adj: Option<f32>,
    #[serde(default)]
    wall_corner_door_l_b_rot_adj: Option<f32>,
    #[serde(default)]
    wall_corner_door_r_b_rot_adj: Option<f32>,
    #[serde(default)]
    corridor_rot_adj: f32,
    #[serde(default)]
    corridor_terminal_rot_adj: Option<f32>,
    #[serde(default)]
    corridor_terminal_offset: f32,
    #[serde(default)]
    door: Option<u32>,
    #[serde(default)]
    door_rot_adj: f32,
    #[serde(default)]
    corridor: Option<u32>,
    #[serde(default)]
    corridor_entrance: Option<u32>,
    #[serde(default)]
    corridor_entrance_left: Option<u32>,
    #[serde(default)]
    corridor_entrance_right: Option<u32>,
    #[serde(default)]
    corridor_terminal: Option<u32>,
    #[serde(default)]
    acoustic_space: Option<u32>,
    #[serde(default)]
    music_type: Option<u32>,
    #[serde(default)]
    image_space: Option<u32>,
}

#[derive(Deserialize, Default)]
struct ListsFile {
    #[serde(default)]
    kits: Vec<KitToml>,
    /// category name → list of pool names assigned to that category
    #[serde(default)]
    categories: HashMap<String, Vec<String>>,
    #[serde(default)]
    items: Vec<ItemToml>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Combined output of a loaded content_lists.toml file.
#[derive(Default)]
pub struct ContentData {
    /// Kit definitions keyed by name (e.g. "office", "hotel").
    pub kits: HashMap<String, KitData>,
    /// Named item pools used for furniture, clutter, lights, etc.
    pub lists: HashMap<String, Vec<ContentItem>>,
    /// UI categories each named list belongs to: any of "furniture", "floor_clutter",
    /// "surface_clutter", "wall_decorations", "lights". A list may belong to several
    /// (e.g. a list can be both floor_clutter and surface_clutter).
    /// Lists absent from this map (or mapped to an empty vec) appear in all sections.
    pub list_categories: HashMap<String, Vec<String>>,
}

/// Load a `content_lists.toml` file and return kit definitions and item pools.
///
/// Returns empty data (no error) if the file does not exist.
/// Returns an error if the file exists but cannot be parsed.
pub fn load(path: &Path) -> std::io::Result<ContentData> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ContentData::default()),
        Err(e) => return Err(e),
    };

    let file: ListsFile = toml::from_str(&src)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut kits: HashMap<String, KitData> = HashMap::new();
    for k in file.kits {
        kits.insert(k.name, KitData {
            description:       k.description,
            room_height:       k.room_height,
            room_mid:          k.room_mid,
            wall_straight:       k.wall_straight,
            wall_straight_ew:    k.wall_straight_ew,
            wall_corner_inner:   k.wall_corner_inner,
            wall_corner_inner_b: k.wall_corner_inner_b,
            wall_corner_door_l:   k.wall_corner_door_l,
            wall_corner_door_r:   k.wall_corner_door_r,
            wall_corner_door_l_b: k.wall_corner_door_l_b,
            wall_corner_door_r_b: k.wall_corner_door_r_b,
            wall_doorway:    k.wall_doorway,
            wall_doorway_ew:         k.wall_doorway_ew,
            wall_doorway_ew_rot_adj: k.wall_doorway_ew_rot_adj,
            room_mid_rot_adj:      k.room_mid_rot_adj,
            wall_straight_rot_adj:    k.wall_straight_rot_adj,
            wall_straight_ew_rot_adj: k.wall_straight_ew_rot_adj,
            wall_doorway_rot_adj:  k.wall_doorway_rot_adj,
            wall_corner_rot_adj:         k.wall_corner_rot_adj,
            wall_corner_inner_b_rot_adj: k.wall_corner_inner_b_rot_adj,
            wall_corner_door_l_rot_adj:  k.wall_corner_door_l_rot_adj,
            wall_corner_door_l_offset:   k.wall_corner_door_l_offset,
            wall_corner_door_r_rot_adj:    k.wall_corner_door_r_rot_adj,
            wall_corner_door_l_b_rot_adj:  k.wall_corner_door_l_b_rot_adj,
            wall_corner_door_r_b_rot_adj:  k.wall_corner_door_r_b_rot_adj,
            corridor_rot_adj:          k.corridor_rot_adj,
            corridor_terminal_rot_adj: k.corridor_terminal_rot_adj,
            corridor_terminal_offset:  k.corridor_terminal_offset,
            door:         k.door,
            door_rot_adj: k.door_rot_adj,
            corridor:               k.corridor,
            corridor_entrance:      k.corridor_entrance,
            corridor_entrance_left: k.corridor_entrance_left,
            corridor_entrance_right: k.corridor_entrance_right,
            corridor_terminal:      k.corridor_terminal,
            acoustic_space:    k.acoustic_space,
            music_type:        k.music_type,
            image_space:       k.image_space,
        });
    }

    let mut lists: HashMap<String, Vec<ContentItem>> = HashMap::new();
    for rec in file.items {
        let (allowed, align_to_wall) = match rec.placement {
            PlacementToml::Anywhere        => (PieceMask::anywhere(),      false),
            PlacementToml::Walls           => (PieceMask::wall_only(),     true),
            PlacementToml::Interior        => (PieceMask::interior_only(), false),
            PlacementToml::Corners         => (PieceMask::corner_only(),   true),
            PlacementToml::WallsAndCorners => (PieceMask::wall_and_corner(), true),
        };

        let item = ContentItem {
            form_id: rec.form_id,
            z_offset: rec.z_offset,
            base_rot: (rec.base_rot[0], rec.base_rot[1], rec.base_rot[2]),
            jitter: rec.jitter.to_axes(),
            allowed,
            align_to_wall,
            wall_depth: rec.wall_depth,
            companions: rec.companions.into_iter().map(|c| Companion {
                form_id:    c.form_id,
                z_offset:   c.z_offset,
                forward:    c.forward,
                right:      c.right,
                rot_offset: c.rot_offset,
                surface_scatter: c.surface_scatter.into_iter().map(|s| SurfaceScatter {
                    list:     s.list,
                    count:    s.count,
                    z_offset: s.z_offset,
                    x_range:  s.x_range,
                    y_range:  s.y_range,
                }).collect(),
            }).collect(),
            surface_scatter: rec.surface_scatter.into_iter().map(|s| SurfaceScatter {
                list:     s.list,
                count:    s.count,
                z_offset: s.z_offset,
                x_range:  s.x_range,
                y_range:  s.y_range,
            }).collect(),
        };

        lists.entry(rec.list).or_default().push(item);
    }

    // Invert categories (category → [lists]) into list_categories (list → [categories]).
    // A list can appear under more than one category, so accumulate rather than overwrite.
    let mut list_categories: HashMap<String, Vec<String>> = HashMap::new();
    for (category, names) in file.categories {
        for name in names {
            list_categories.entry(name).or_default().push(category.clone());
        }
    }

    Ok(ContentData { kits, lists, list_categories })
}
