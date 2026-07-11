use super::GenContext;
use crate::model::room::PIECE_SIZE;
use crate::records::refr::{RefrDef, Transform};
use rand::Rng;

/// Places one light per interior (mid) tile on the ceiling.
/// Corridor rooms light only the middle run, skipping the end/exit tiles.
pub fn generate(ctx: &mut GenContext) -> Vec<RefrDef> {
    if ctx.lights.is_empty() {
        return Vec::new();
    }

    let room = ctx.room;
    let z = ctx.kit.room_height - 8.0;
    let lights = ctx.lights.clone();
    let (ox, oy) = ctx.offset;
    let mut refs = Vec::new();

    let mut positions: Vec<(f32, f32)> = Vec::new();

    if room.width == 1 {
        for yi in 1..room.length.saturating_sub(1) {
            positions.push((ox, oy + yi as f32 * PIECE_SIZE));
        }
    } else if room.length == 1 {
        for xi in 1..room.width.saturating_sub(1) {
            positions.push((ox + xi as f32 * PIECE_SIZE, oy));
        }
    } else {
        for xi in 1..room.width.saturating_sub(1) {
            for yi in 1..room.length.saturating_sub(1) {
                positions.push((ox + xi as f32 * PIECE_SIZE, oy + yi as f32 * PIECE_SIZE));
            }
        }
    }

    // Fallback for 2×2, 2×1, 1×2, 1×1: place one light at the geometric centre.
    if positions.is_empty() {
        positions.push((
            ox + (room.width  as f32 - 1.0) * PIECE_SIZE * 0.5,
            oy + (room.length as f32 - 1.0) * PIECE_SIZE * 0.5,
        ));
    }

    for (x, y) in positions {
        let item = &lights[ctx.rng.gen_range(0..lights.len())];
        let (rx, ry, rz) = item.base_rot;
        let lx = x;
        let ly = y;
        let lz = z + item.z_offset;
        refs.push(RefrDef::new(
            item.form_id,
            Transform::at(lx, ly, lz).with_rotation(rx, ry, rz),
        ));

        for comp in &item.companions {
            let cx = lx + comp.forward * rz.cos() + comp.right * rz.sin();
            let cy = ly + comp.forward * rz.sin() - comp.right * rz.cos();
            let cz = lz + comp.z_offset;
            refs.push(RefrDef::new(
                comp.form_id,
                Transform::at(cx, cy, cz).with_rotation(0.0, 0.0, rz + comp.rot_offset),
            ));
        }
    }

    refs
}
