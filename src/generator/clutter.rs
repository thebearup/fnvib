use super::GenContext;
use crate::content_lists::ContentItem;
use crate::model::room::{Cardinal, PIECE_SIZE};
use crate::records::refr::{RefrDef, Transform};
use rand::Rng;
use std::collections::HashSet;

const WALL_MARGIN: f32 = 17.0;
const HALF_PIECE: f32 = PIECE_SIZE * 0.5;
// Safety clearance from the outer face of each wall piece. FNV wall geometry
// is thin (~16-32 units), so 64 units from the outer face is always clear floor.
const OUTER_WALL_CLEARANCE: f32 = 64.0;
const SURFACE_HEIGHT: f32 = 36.0;

/// Scatters small items on the floor.
pub fn generate_floor(ctx: &mut GenContext) -> Vec<RefrDef> {
    let pool = ctx.floor_clutter.clone();
    scatter(ctx, &pool, 0.0, 3)
}

/// Places items on top of surfaces at a fixed height above the floor.
pub fn generate_surface(ctx: &mut GenContext) -> Vec<RefrDef> {
    let pool = ctx.surface_clutter.clone();
    scatter(ctx, &pool, SURFACE_HEIGHT, 2)
}

/// Places wall decorations at mid-height, one per perimeter tile, 60% spawn chance.
/// Doorway tiles are skipped. Corner tiles are randomly assigned to one adjacent wall.
pub fn generate_wall_decorations(ctx: &mut GenContext) -> Vec<RefrDef> {
    if ctx.wall_decorations.is_empty() {
        return Vec::new();
    }

    use std::f32::consts::{FRAC_PI_2, PI};

    let room = ctx.room;
    let (ox, oy) = ctx.offset;
    let pool = ctx.wall_decorations.clone();

    // Inner-face Y/X positions for each wall row, offset slightly off the surface.
    let south_y = oy - HALF_PIECE + WALL_MARGIN;
    let north_y = oy + (room.length as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - WALL_MARGIN;
    let west_x  = ox - HALF_PIECE + WALL_MARGIN;
    let east_x  = ox + (room.width  as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - WALL_MARGIN;

    let doors_s: HashSet<u32> = room.doorways_on(Cardinal::South).map(|d| d.offset).collect();
    let doors_n: HashSet<u32> = room.doorways_on(Cardinal::North).map(|d| d.offset).collect();
    let doors_w: HashSet<u32> = room.doorways_on(Cardinal::West).map(|d| d.offset).collect();
    let doors_e: HashSet<u32> = room.doorways_on(Cardinal::East).map(|d| d.offset).collect();

    // One candidate (x, y, wall_rz) per non-doorway perimeter tile.
    // wall_rz rotates the decoration to face inward from its wall.
    let mut candidates: Vec<(f32, f32, f32)> = Vec::new();

    for xi in 1..room.width.saturating_sub(1) {
        let x = ox + xi as f32 * PIECE_SIZE;
        if !doors_s.contains(&(xi - 1)) { candidates.push((x, south_y, PI)); }
        if !doors_n.contains(&(xi - 1)) { candidates.push((x, north_y, 0.0)); }
    }
    for yi in 1..room.length.saturating_sub(1) {
        let y = oy + yi as f32 * PIECE_SIZE;
        if !doors_w.contains(&(yi - 1)) { candidates.push((west_x, y, FRAC_PI_2 * 3.0)); }
        if !doors_e.contains(&(yi - 1)) { candidates.push((east_x, y, FRAC_PI_2)); }
    }

    // Corners — randomly assign to one of the two adjacent walls, slide 60 units
    // along that wall away from the corner so the decoration doesn't clip into it.
    // Each entry: (base_x, base_y, [(rot, dx, dy), (rot, dx, dy)])
    const CORNER_SLIDE: f32 = 60.0;
    let corners: [(f32, f32, [(f32, f32, f32); 2]); 4] = [
        (west_x, south_y, [(PI,           CORNER_SLIDE,  0.0),           // SW south-face: slide +x
                           (FRAC_PI_2*3., 0.0,           CORNER_SLIDE)]), // SW west-face:  slide +y
        (east_x, south_y, [(PI,          -CORNER_SLIDE,  0.0),           // SE south-face: slide -x
                           (FRAC_PI_2,    0.0,           CORNER_SLIDE)]), // SE east-face:  slide +y
        (east_x, north_y, [(0.0,         -CORNER_SLIDE,  0.0),           // NE north-face: slide -x
                           (FRAC_PI_2,    0.0,          -CORNER_SLIDE)]), // NE east-face:  slide -y
        (west_x, north_y, [(0.0,          CORNER_SLIDE,  0.0),           // NW north-face: slide +x
                           (FRAC_PI_2*3., 0.0,          -CORNER_SLIDE)]), // NW west-face:  slide -y
    ];
    for (bx, by, choices) in corners {
        let (rot, dx, dy) = choices[ctx.rng.gen_range(0..2)];
        candidates.push((bx + dx, by + dy, rot));
    }

    let mut refs = Vec::new();
    for (x, y, wall_rz) in candidates {
        if !ctx.rng.gen_bool(0.6) {
            continue;
        }
        let item = &pool[ctx.rng.gen_range(0..pool.len())];
        let [jx, jy, jz] = item.jitter;
        let rx_j = if jx > 0.0 { ctx.rng.gen_range(-jx..jx) } else { 0.0 };
        let ry_j = if jy > 0.0 { ctx.rng.gen_range(-jy..jy) } else { 0.0 };
        let rz_j = if jz > 0.0 { ctx.rng.gen_range(-jz..jz) } else { 0.0 };
        let (rx, ry, rz) = item.base_rot;
        refs.push(RefrDef::new(
            item.form_id,
            Transform::at(x, y, 50.0 + item.z_offset).with_rotation(rx + rx_j, ry + ry_j, rz + wall_rz + rz_j),
        ));
    }

    refs
}

fn scatter(ctx: &mut GenContext, pool: &[ContentItem], z_base: f32, per_piece: usize) -> Vec<RefrDef> {
    if pool.is_empty() {
        return Vec::new();
    }

    let room = ctx.room;
    let (ox, oy) = ctx.offset;

    // Room outer faces are at ox ± HALF_PIECE. FNV wall geometry only occupies
    // the outermost portion of each wall piece; OUTER_WALL_CLEARANCE keeps
    // items off the mesh while still allowing scatter near walls and corners.
    let x_min = ox - HALF_PIECE + OUTER_WALL_CLEARANCE;
    let x_max = ox + (room.width  as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - OUTER_WALL_CLEARANCE;
    let y_min = oy - HALF_PIECE + OUTER_WALL_CLEARANCE;
    let y_max = oy + (room.length as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - OUTER_WALL_CLEARANCE;

    if x_min >= x_max || y_min >= y_max {
        return Vec::new();
    }

    let count = (room.width * room.length) as usize * per_piece;
    let mut refs = Vec::new();

    for _ in 0..count {
        let x = ctx.rng.gen_range(x_min..x_max);
        let y = ctx.rng.gen_range(y_min..y_max);
        let item = &pool[ctx.rng.gen_range(0..pool.len())];
        let [jx, jy, jz] = item.jitter;
        let rx_j = if jx > 0.0 { ctx.rng.gen_range(-jx..jx) } else { 0.0 };
        let ry_j = if jy > 0.0 { ctx.rng.gen_range(-jy..jy) } else { 0.0 };
        let rz_j = if jz > 0.0 { ctx.rng.gen_range(-jz..jz) } else { 0.0 };
        let (rx, ry, rz) = item.base_rot;
        refs.push(RefrDef::new(
            item.form_id,
            Transform::at(x, y, z_base + item.z_offset).with_rotation(rx + rx_j, ry + ry_j, rz + rz_j),
        ));
    }

    refs
}
