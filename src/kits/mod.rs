/// All FormIDs and pool data needed to generate a room using a specific kit.
///
/// Loaded at runtime from [[kits]] entries in content_lists.toml.
/// No kit logic lives in compiled code — add or modify kits by editing the TOML.
#[derive(Debug, Clone)]
pub struct KitData {
    /// Human-readable description shown by the list-kits command.
    pub description: String,

    // --- Structural pieces (each 256x256 units, all include their own floor and ceiling) ---
    /// Interior piece: floor + ceiling, no walls.
    pub room_mid: u32,
    /// Perimeter wall piece: floor + ceiling + one inward-facing wall face.
    /// Used for the south and north walls (the N/S axis).
    pub wall_straight: u32,
    /// Alternative wall_straight for the east and west walls (the E/W axis).
    /// Most kits leave this `None` (same piece works for all orientations).
    /// Vault kits use a geometrically distinct mesh for the perpendicular axis.
    pub wall_straight_ew: Option<u32>,
    /// Corner piece: floor + ceiling + two wall faces meeting at 90 degrees.
    /// Used for the SW and NE corners (the diagonal A pair).
    pub wall_corner_inner: u32,
    /// Alternative wall_corner_inner for the SE and NW corners (diagonal B pair).
    /// When `None`, `wall_corner_inner` is used for all four corners.
    pub wall_corner_inner_b: Option<u32>,
    /// Corner piece with a doorway opening on the left face of the corner. Used at NE and NW (diagonal A door).
    pub wall_corner_door_l: u32,
    /// Corner piece with a doorway opening on the right face of the corner. Used at SE and NE (diagonal A door).
    pub wall_corner_door_r: u32,
    /// Alternative wall_corner_door_l for diagonal B positions (NW corner).
    /// When `None`, `wall_corner_door_l` is used at NW.
    pub wall_corner_door_l_b: Option<u32>,
    /// Alternative wall_corner_door_r for diagonal B positions (SE corner).
    /// When `None`, `wall_corner_door_r` is used at SE.
    pub wall_corner_door_r_b: Option<u32>,
    /// Wall piece with a doorway opening. Used for south and north walls.
    pub wall_doorway: u32,
    /// Alternative wall_doorway for east and west walls.
    /// When `None`, `wall_doorway` is used for all orientations.
    pub wall_doorway_ew: Option<u32>,
    /// Override rotation for wall_doorway_ew. Replaces door_adj for E/W doorways when set.
    pub wall_doorway_ew_rot_adj: Option<f32>,

    // Per-piece-family rotation adjustments (radians added to the base rotation).
    // All default to 0.0. Most kits only need wall_doorway_rot_adj.
    /// Extra rotation for room_mid (interior floor) pieces.
    pub room_mid_rot_adj: f32,
    /// Extra rotation for wall_straight pieces (south and north walls).
    pub wall_straight_rot_adj: f32,
    /// Override rotation for wall_straight on east and west walls. Replaces wall_straight_rot_adj when set.
    pub wall_straight_ew_rot_adj: Option<f32>,
    /// Extra rotation for wall_doorway pieces, applied ON TOP of wall_straight_rot_adj.
    /// Default PI/2 matches the office kit (doorway mesh is 90° CCW from wall_straight).
    /// Set to 0.0 for kits whose doorway mesh is already aligned with wall_straight.
    pub wall_doorway_rot_adj: f32,
    /// Extra rotation for corner pieces (wall_corner_inner and both door variants).
    /// Applied to SW and NE corners (diagonal A).
    pub wall_corner_rot_adj: f32,
    /// Override rotation for wall_corner_inner_b (SE and NW plain corners). Replaces wall_corner_rot_adj when set.
    pub wall_corner_inner_b_rot_adj: Option<f32>,
    /// Override rotation for wall_corner_door_l. Replaces wall_corner_rot_adj when set.
    pub wall_corner_door_l_rot_adj: Option<f32>,
    /// Positional correction for wall_corner_door_l meshes whose origin is offset from the
    /// nominal corner position. Applied as -X at the NE corner and -Y at the NW corner
    /// (both directions point toward the room interior). Set to 16.0 to pull a piece that
    /// is 16 units too far out back into alignment.
    pub wall_corner_door_l_offset: f32,
    /// Override rotation for wall_corner_door_r. Replaces wall_corner_rot_adj when set.
    pub wall_corner_door_r_rot_adj: Option<f32>,
    /// Override rotation for wall_corner_door_l_b (NW diagonal-B door). Replaces wall_corner_door_l_rot_adj when set.
    pub wall_corner_door_l_b_rot_adj: Option<f32>,
    /// Override rotation for wall_corner_door_r_b (SE diagonal-B door). Replaces wall_corner_door_r_rot_adj when set.
    pub wall_corner_door_r_b_rot_adj: Option<f32>,
    /// Extra rotation for all corridor pieces (middle, entrance, terminal).
    pub corridor_rot_adj: f32,
    /// Override rotation for corridor_terminal pieces. Replaces corridor_rot_adj when set.
    pub corridor_terminal_rot_adj: Option<f32>,
    /// Position offset for corridor_terminal pieces along the corridor axis (units).
    /// Positive shifts the terminal toward the corridor interior; negative moves it away.
    pub corridor_terminal_offset: f32,
    /// Door asset placed in every doorway opening. `None` = no door placed.
    pub door: Option<u32>,
    /// Extra rotation added to every door placement (radians).
    pub door_rot_adj: f32,
    /// Plain corridor section: open on both ends, solid walls on the sides.
    /// Base orientation is E/W (rot=0); rotated PI/2 for N/S use.
    /// `None` if this kit has no corridor mesh variants.
    pub corridor: Option<u32>,
    /// Corridor end piece: door straight ahead through the end face.
    pub corridor_entrance: Option<u32>,
    /// Corridor end piece: door on the left side when facing out through the end.
    pub corridor_entrance_left: Option<u32>,
    /// Corridor end piece: door on the right side when facing out through the end.
    pub corridor_entrance_right: Option<u32>,
    /// Corridor end piece: solid cap, no opening (dead end).
    pub corridor_terminal: Option<u32>,
    /// Height of the room in units (used for clutter/lighting Z placement).
    pub room_height: f32,

    // --- Cell audio/visual properties ---
    pub acoustic_space: Option<u32>,
    pub music_type: Option<u32>,
    pub image_space: Option<u32>,
}
