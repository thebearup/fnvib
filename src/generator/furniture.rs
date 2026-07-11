use super::GenContext;
use crate::content_lists::PieceClass;
use crate::model::room::{Cardinal, PIECE_SIZE};
use crate::records::refr::{RefrDef, Transform};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashSet;
use std::f32::consts::PI;

/// Full piece classification, including cardinal direction for wall pieces
/// and position flags for corner pieces.
enum PieceKind {
    Interior,
    Wall(Cardinal),
    Doorway,
    /// `north`/`east` identify which corner: (F,F)=SW (F,T)=SE (T,T)=NE (T,F)=NW.
    Corner { north: bool, east: bool },
}

impl PieceKind {
    fn class(&self) -> PieceClass {
        match self {
            PieceKind::Interior      => PieceClass::Interior,
            PieceKind::Wall(_)       => PieceClass::Wall,
            PieceKind::Doorway       => PieceClass::Doorway,
            PieceKind::Corner { .. } => PieceClass::Corner,
        }
    }
}

fn classify(
    xi: u32, yi: u32, w: u32, l: u32,
    ds: &HashSet<u32>, dn: &HashSet<u32>,
    dw: &HashSet<u32>, de: &HashSet<u32>,
) -> PieceKind {
    let south = yi == 0;
    let north = yi == l - 1;
    let west  = xi == 0;
    let east  = xi == w - 1;

    if (south || north) && (west || east) {
        // When a room is too narrow for non-corner wall slots, doorways fall
        // on corner positions and structure.rs places wall_corner_door_l/r.
        // Mirror the same offset logic so furniture is suppressed there.
        let has_corner_door = (east && south && w >= 2 && ds.contains(&(w - 2)))
            || (east && north && w >= 2 && dn.contains(&(w - 2)))
            || (east && north && l >= 2 && de.contains(&(l - 2)))
            || (west && north && l >= 2 && dw.contains(&(l - 2)));
        if has_corner_door {
            return PieceKind::Doorway;
        }
        return PieceKind::Corner { north, east };
    }
    if south {
        return if ds.contains(&(xi - 1)) { PieceKind::Doorway } else { PieceKind::Wall(Cardinal::South) };
    }
    if north {
        return if dn.contains(&(xi - 1)) { PieceKind::Doorway } else { PieceKind::Wall(Cardinal::North) };
    }
    if west {
        return if dw.contains(&(yi - 1)) { PieceKind::Doorway } else { PieceKind::Wall(Cardinal::West) };
    }
    if east {
        return if de.contains(&(yi - 1)) { PieceKind::Doorway } else { PieceKind::Wall(Cardinal::East) };
    }
    PieceKind::Interior
}

/// The angle (radians) an item should face when aligned to a wall, pointing inward.
/// Convention: rot_z = 0 → facing +X (East); positive = CCW.
fn wall_inward_angle(wall: Cardinal) -> f32 {
    match wall {
        Cardinal::South =>  PI / 2.0,  // face North
        Cardinal::North => -PI / 2.0,  // face South
        Cardinal::East  =>  PI,         // face West
        Cardinal::West  =>  0.0,        // face East
    }
}

/// Maps a corner piece to the wall face it most naturally backs against.
/// Furniture is then aligned exactly as it would be on that wall.
fn corner_assigned_wall(north: bool, east: bool) -> Cardinal {
    match (north, east) {
        (false, false) => Cardinal::West,   // SW corner → West wall
        (false, true)  => Cardinal::South,  // SE corner → South wall
        (true,  true)  => Cardinal::East,   // NE corner → East wall
        (true,  false) => Cardinal::North,  // NW corner → North wall
    }
}

/// Places one furniture item per eligible room piece.
///
/// Grid positions that have no eligible candidates are skipped. Doorway pieces
/// are always skipped. Each placed item's companions are placed immediately
/// after, offset from the parent's final world position and rotation.
pub fn generate(ctx: &mut GenContext) -> Vec<RefrDef> {
    if ctx.furniture.is_empty() {
        return Vec::new();
    }

    let room = ctx.room;
    // Corridors are a single piece wide/tall — too narrow for furniture.
    if room.width == 1 || room.length == 1 {
        return Vec::new();
    }

    let (ox, oy) = ctx.offset;
    let w = room.width;
    let l = room.length;

    let doors_s: HashSet<u32> = room.doorways_on(Cardinal::South).map(|d| d.offset).collect();
    let doors_n: HashSet<u32> = room.doorways_on(Cardinal::North).map(|d| d.offset).collect();
    let doors_w: HashSet<u32> = room.doorways_on(Cardinal::West).map(|d| d.offset).collect();
    let doors_e: HashSet<u32> = room.doorways_on(Cardinal::East).map(|d| d.offset).collect();

    let pool = ctx.furniture.clone();
    let mut refs = Vec::new();

    for xi in 0..w {
        for yi in 0..l {
            let kind = classify(xi, yi, w, l, &doors_s, &doors_n, &doors_w, &doors_e);
            let class = kind.class();

            let candidates: Vec<usize> = (0..pool.len())
                .filter(|&i| pool[i].allowed.allows(class))
                .collect();

            if candidates.is_empty() {
                continue;
            }

            let item = &pool[candidates[ctx.rng.gen_range(0..candidates.len())]];

            // Piece origin in world space. Meshes are centered at their origin,
            // so this is both the grid coordinate and the piece centre.
            let px = ox + xi as f32 * PIECE_SIZE;
            let py = oy + yi as f32 * PIECE_SIZE;

            // Corners are treated as the wall they back against (SW→W, SE→S, NE→E, NW→N).
            // Both wall and corner pieces resolve to a single cardinal for alignment.
            let aligned_cardinal: Option<Cardinal> = if item.align_to_wall {
                match &kind {
                    PieceKind::Wall(dir)              => Some(*dir),
                    PieceKind::Corner { north, east } => Some(corner_assigned_wall(*north, *east)),
                    _ => None,
                }
            } else {
                None
            };

            let wall_angle = aligned_cardinal.map_or(0.0, wall_inward_angle);

            let tilt = if item.jitter[2] > 0.0 {
                ctx.rng.gen_range(-item.jitter[2]..item.jitter[2])
            } else {
                0.0
            };

            // world_facing: the direction the item logically faces (away from wall).
            // Used for companion offsets and wall_depth push. For wall-mounted items,
            // jitter is a visual tilt that does not change the facing direction used
            // for positioning, so it is excluded here.
            let world_facing = if item.align_to_wall {
                wall_angle
            } else {
                wall_angle + tilt
            };

            // FNV uses clockwise-from-South rotation. N/S walls end up 180° off
            // without this correction.
            let ns_flip = match aligned_cardinal {
                Some(Cardinal::North | Cardinal::South) => PI,
                _ => 0.0,
            };

            // For wall-mounted items, jitter tilts the model around the wall's inward
            // normal rather than around world-Z, so it reads as a crooked-hang effect.
            // N/S walls face along Y → tilt around Y; E/W walls face along X → tilt around X.
            // For floor items, jitter is folded into world_facing (Z rotation) as before.
            let (rx, ry, rz) = item.base_rot;
            let (final_rx, final_ry, final_rz) = if item.align_to_wall {
                let base_rz = rz + world_facing + ns_flip;
                match aligned_cardinal {
                    Some(Cardinal::North | Cardinal::South) => (rx,       ry + tilt, base_rz),
                    Some(Cardinal::East  | Cardinal::West)  => (rx + tilt, ry,       base_rz),
                    _ =>                                       (rx,        ry,        base_rz),
                }
            } else {
                (rx, ry, rz + world_facing)
            };

            // Push item from piece centre toward the wall.
            // world_facing points INTO the room, so subtracting along it moves
            // toward the wall. wall_depth=0 → piece centre; wall_depth=64 → 64
            // units toward the outer wall face (where the wall geometry is).
            let (px, py) = if item.align_to_wall {
                (
                    px - item.wall_depth * world_facing.cos(),
                    py - item.wall_depth * world_facing.sin(),
                )
            } else {
                (px, py)
            };

            refs.push(RefrDef::new(
                item.form_id,
                Transform::at(px, py, item.z_offset).with_rotation(final_rx, final_ry, final_rz),
            ));

            // Companions use world_facing so their offset is always perpendicular
            // to the wall regardless of base_rot.
            for comp in &item.companions {
                let cx = px + comp.forward * world_facing.cos() + comp.right * world_facing.sin();
                let cy = py + comp.forward * world_facing.sin() - comp.right * world_facing.cos();
                let cr = world_facing + comp.rot_offset + ns_flip;
                refs.push(RefrDef::new(
                    comp.form_id,
                    Transform::at(cx, cy, comp.z_offset).with_rotation(0.0, 0.0, cr),
                ));

                for scatter in &comp.surface_scatter {
                    let pool = match ctx.lists.get(&scatter.list) {
                        Some(p) if !p.is_empty() => p,
                        _ => continue,
                    };
                    for sitem in pool.choose_multiple(ctx.rng, scatter.count) {
                        let fwd = if scatter.y_range > 0.0 {
                            ctx.rng.gen_range(-scatter.y_range..scatter.y_range)
                        } else { 0.0 };
                        let rgt = if scatter.x_range > 0.0 {
                            ctx.rng.gen_range(-scatter.x_range..scatter.x_range)
                        } else { 0.0 };
                        let sx = cx + fwd * world_facing.cos() + rgt * world_facing.sin();
                        let sy = cy + fwd * world_facing.sin() - rgt * world_facing.cos();
                        let sz = scatter.z_offset + sitem.z_offset;
                        let [sjx, sjy, sjz] = sitem.jitter;
                        let srx_j = if sjx > 0.0 { ctx.rng.gen_range(-sjx..sjx) } else { 0.0 };
                        let sry_j = if sjy > 0.0 { ctx.rng.gen_range(-sjy..sjy) } else { 0.0 };
                        let srz_j = if sjz > 0.0 { ctx.rng.gen_range(-sjz..sjz) } else { 0.0 };
                        let (srx, sry, srz) = sitem.base_rot;
                        refs.push(RefrDef::new(
                            sitem.form_id,
                            Transform::at(sx, sy, sz).with_rotation(srx + srx_j, sry + sry_j, srz + srz_j),
                        ));
                    }
                }
            }

            // Surface scatter: place random items from named pools on the parent's
            // surface. x_range/y_range define the scatter zone in the parent's local
            // frame (right/forward axes), z_offset is the absolute surface height.
            for scatter in &item.surface_scatter {
                let pool = match ctx.lists.get(&scatter.list) {
                    Some(p) if !p.is_empty() => p,
                    _ => continue,
                };
                for sitem in pool.choose_multiple(ctx.rng, scatter.count) {
                    let fwd = if scatter.y_range > 0.0 {
                        ctx.rng.gen_range(-scatter.y_range..scatter.y_range)
                    } else { 0.0 };
                    let rgt = if scatter.x_range > 0.0 {
                        ctx.rng.gen_range(-scatter.x_range..scatter.x_range)
                    } else { 0.0 };
                    let sx = px + fwd * world_facing.cos() + rgt * world_facing.sin();
                    let sy = py + fwd * world_facing.sin() - rgt * world_facing.cos();
                    let sz = scatter.z_offset + sitem.z_offset;
                    let [sjx, sjy, sjz] = sitem.jitter;
                    let srx_j = if sjx > 0.0 { ctx.rng.gen_range(-sjx..sjx) } else { 0.0 };
                    let sry_j = if sjy > 0.0 { ctx.rng.gen_range(-sjy..sjy) } else { 0.0 };
                    let srz_j = if sjz > 0.0 { ctx.rng.gen_range(-sjz..sjz) } else { 0.0 };
                    let (srx, sry, srz) = sitem.base_rot;
                    refs.push(RefrDef::new(
                        sitem.form_id,
                        Transform::at(sx, sy, sz).with_rotation(srx + srx_j, sry + sry_j, srz + srz_j),
                    ));
                }
            }
        }
    }

    refs
}
