use crate::model::room::{Cardinal, Doorway, Room, PIECE_SIZE};
use crate::records::navm::{NavmDef, NavmTriangle};
use std::collections::HashMap;

const WALL_INSET: f32 = 64.0;
const HALF_PIECE: f32 = PIECE_SIZE * 0.5;

fn intern(
    x: f32,
    y: f32,
    vertices: &mut Vec<[f32; 3]>,
    vert_map: &mut HashMap<[u32; 2], u16>,
) -> u16 {
    let key = [x.to_bits(), y.to_bits()];
    if let Some(&i) = vert_map.get(&key) {
        return i;
    }
    let i = vertices.len() as u16;
    vert_map.insert(key, i);
    vertices.push([x, y, 0.0]);
    i
}

/// Rotate triangle vertices so the minimum index is v0 (preserves CCW winding).
fn rot_min(t: [u16; 3]) -> [u16; 3] {
    let i = if t[0] <= t[1] && t[0] <= t[2] { 0 } else if t[1] <= t[2] { 1 } else { 2 };
    [t[i], t[(i + 1) % 3], t[(i + 2) % 3]]
}

/// Navmesh bounds for the inset core of a room — always WALL_INSET inside all physical walls.
fn core_bounds(room: &Room, ox: f32, oy: f32) -> (f32, f32, f32, f32) {
    let x_min = ox - HALF_PIECE + WALL_INSET;
    let x_max = ox + (room.width  as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - WALL_INSET;
    let y_min = oy - HALF_PIECE + WALL_INSET;
    let y_max = oy + (room.length as f32 - 1.0) * PIECE_SIZE + HALF_PIECE - WALL_INSET;
    (x_min, x_max, y_min, y_max)
}

/// Index of the piece (column or row) that holds a doorway.
/// Normal rooms skip the corner piece (+1); corridors use the raw offset.
fn door_piece_idx(room: &Room, offset: u32) -> f32 {
    if room.width == 1 || room.length == 1 { offset as f32 } else { offset as f32 + 1.0 }
}

/// X-range of the navigable opening for a north/south doorway, clamped to [wall_min, wall_max].
/// The opening is one piece wide, inset by WALL_INSET on each side.
fn door_x_range(door: &Doorway, room: &Room, ox: f32, wall_min: f32, wall_max: f32) -> Option<(f32, f32)> {
    let center = ox + door_piece_idx(room, door.offset) * PIECE_SIZE;
    let lo = (center - (HALF_PIECE - WALL_INSET)).max(wall_min);
    let hi = (center + (HALF_PIECE - WALL_INSET)).min(wall_max);
    if lo < hi - 0.5 { Some((lo, hi)) } else { None }
}

/// Y-range of the navigable opening for an east/west doorway, clamped to [wall_min, wall_max].
fn door_y_range(door: &Doorway, room: &Room, oy: f32, wall_min: f32, wall_max: f32) -> Option<(f32, f32)> {
    let center = oy + door_piece_idx(room, door.offset) * PIECE_SIZE;
    let lo = (center - (HALF_PIECE - WALL_INSET)).max(wall_min);
    let hi = (center + (HALF_PIECE - WALL_INSET)).min(wall_max);
    if lo < hi - 0.5 { Some((lo, hi)) } else { None }
}

/// Interior x-positions on a horizontal core edge where tabs attach (for T-junction vertices).
fn tab_edge_xs(room: &Room, dir: Cardinal, ox: f32, wall_min: f32, wall_max: f32) -> Vec<f32> {
    let mut xs: Vec<f32> = room.doorways.iter()
        .filter(|d| d.direction == dir)
        .filter_map(|d| door_x_range(d, room, ox, wall_min, wall_max))
        .flat_map(|(lo, hi)| {
            let mut pts = Vec::new();
            if lo > wall_min + 0.5 { pts.push(lo); }
            if hi < wall_max - 0.5 { pts.push(hi); }
            pts
        })
        .collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup();
    xs
}

/// Interior y-positions on a vertical core edge where tabs attach (for T-junction vertices).
fn tab_edge_ys(room: &Room, dir: Cardinal, oy: f32, wall_min: f32, wall_max: f32) -> Vec<f32> {
    let mut ys: Vec<f32> = room.doorways.iter()
        .filter(|d| d.direction == dir)
        .filter_map(|d| door_y_range(d, room, oy, wall_min, wall_max))
        .flat_map(|(lo, hi)| {
            let mut pts = Vec::new();
            if lo > wall_min + 0.5 { pts.push(lo); }
            if hi < wall_max - 0.5 { pts.push(hi); }
            pts
        })
        .collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup();
    ys
}

/// Add two CCW triangles forming a rectangle (x_lo,y_lo)-(x_hi,y_hi).
fn add_quad(
    x_lo: f32, x_hi: f32,
    y_lo: f32, y_hi: f32,
    vertices: &mut Vec<[f32; 3]>,
    vert_map: &mut HashMap<[u32; 2], u16>,
    raw: &mut Vec<[u16; 3]>,
) {
    let v00 = intern(x_lo, y_lo, vertices, vert_map);
    let v10 = intern(x_hi, y_lo, vertices, vert_map);
    let v11 = intern(x_hi, y_hi, vertices, vert_map);
    let v01 = intern(x_lo, y_hi, vertices, vert_map);
    raw.push(rot_min([v00, v10, v11]));
    raw.push(rot_min([v00, v11, v01]));
}

/// Emit WALL_INSET-deep doorway tabs for every doorway of a room.
///
/// Each tab bridges the inset core to the physical wall at the exact doorway
/// opening (one piece wide, inset by WALL_INSET each side).  Adjacent rooms
/// emit matching tabs that share the physical-wall edge, tying both halves into
/// one connected navmesh.
fn emit_tabs(
    room: &Room,
    ox: f32, oy: f32,
    cx_min: f32, cx_max: f32, cy_min: f32, cy_max: f32,
    vertices: &mut Vec<[f32; 3]>,
    vert_map: &mut HashMap<[u32; 2], u16>,
    raw: &mut Vec<[u16; 3]>,
) {
    for door in &room.doorways {
        match door.direction {
            Cardinal::North => {
                if let Some((lo, hi)) = door_x_range(door, room, ox, cx_min, cx_max) {
                    add_quad(lo, hi, cy_max, cy_max + WALL_INSET, vertices, vert_map, raw);
                }
            }
            Cardinal::South => {
                if let Some((lo, hi)) = door_x_range(door, room, ox, cx_min, cx_max) {
                    add_quad(lo, hi, cy_min - WALL_INSET, cy_min, vertices, vert_map, raw);
                }
            }
            Cardinal::East => {
                if let Some((lo, hi)) = door_y_range(door, room, oy, cy_min, cy_max) {
                    add_quad(cx_max, cx_max + WALL_INSET, lo, hi, vertices, vert_map, raw);
                }
            }
            Cardinal::West => {
                if let Some((lo, hi)) = door_y_range(door, room, oy, cy_min, cy_max) {
                    add_quad(cx_min - WALL_INSET, cx_min, lo, hi, vertices, vert_map, raw);
                }
            }
        }
    }
}

/// Generate a single unified NavmDef covering all rooms.
///
/// Each room contributes an inset core rectangle plus a narrow WALL_INSET-deep
/// tab for each doorway opening.  Adjacent rooms share the physical-wall edge of
/// their tabs so the adjacency table connects them into one contiguous navmesh.
pub fn generate(rooms: &[(&Room, f32, f32)]) -> NavmDef {
    if rooms.is_empty() {
        return NavmDef { vertices: Vec::new(), triangles: Vec::new() };
    }

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut vert_map: HashMap<[u32; 2], u16> = HashMap::new();
    let mut raw: Vec<[u16; 3]> = Vec::new();

    for &(room, ox, oy) in rooms {
        let (cx_min, cx_max, cy_min, cy_max) = core_bounds(room, ox, oy);

        // Interior T-junction vertices on each core edge where tabs attach.
        let south_xs = tab_edge_xs(room, Cardinal::South, ox, cx_min, cx_max);
        let north_xs = tab_edge_xs(room, Cardinal::North, ox, cx_min, cx_max);
        let east_ys  = tab_edge_ys(room, Cardinal::East,  oy, cy_min, cy_max);
        let west_ys  = tab_edge_ys(room, Cardinal::West,  oy, cy_min, cy_max);

        // Build CCW core polygon.
        let mut poly: Vec<u16> = Vec::new();
        poly.push(intern(cx_min, cy_min, &mut vertices, &mut vert_map)); // SW
        for &x in &south_xs {
            poly.push(intern(x, cy_min, &mut vertices, &mut vert_map));
        }
        poly.push(intern(cx_max, cy_min, &mut vertices, &mut vert_map)); // SE
        for &y in &east_ys {
            poly.push(intern(cx_max, y, &mut vertices, &mut vert_map));
        }
        poly.push(intern(cx_max, cy_max, &mut vertices, &mut vert_map)); // NE
        for &x in north_xs.iter().rev() {
            poly.push(intern(x, cy_max, &mut vertices, &mut vert_map));
        }
        poly.push(intern(cx_min, cy_max, &mut vertices, &mut vert_map)); // NW
        for &y in west_ys.iter().rev() {
            poly.push(intern(cx_min, y, &mut vertices, &mut vert_map));
        }

        let ns = south_xs.len();
        let ne_cnt = east_ys.len();
        let nn = north_xs.len();
        let nw = west_ys.len();
        let fan_idx = if ns == 0 && nw == 0 {
            0
        } else if ns == 0 && ne_cnt == 0 {
            1 + ns
        } else if nn == 0 && ne_cnt == 0 {
            2 + ns + ne_cnt
        } else if nn == 0 && nw == 0 {
            3 + ns + ne_cnt + nn
        } else {
            let cx = (cx_min + cx_max) * 0.5;
            let cy = (cy_min + cy_max) * 0.5;
            let c = intern(cx, cy, &mut vertices, &mut vert_map);
            let n = poly.len();
            for i in 0..n {
                raw.push(rot_min([c, poly[i], poly[(i + 1) % n]]));
            }
            emit_tabs(room, ox, oy, cx_min, cx_max, cy_min, cy_max,
                      &mut vertices, &mut vert_map, &mut raw);
            continue;
        };

        let n = poly.len();
        let f = poly[fan_idx];
        for i in 0..n - 2 {
            let a = poly[(fan_idx + 1 + i) % n];
            let b = poly[(fan_idx + 2 + i) % n];
            raw.push(rot_min([f, a, b]));
        }

        emit_tabs(room, ox, oy, cx_min, cx_max, cy_min, cy_max,
                  &mut vertices, &mut vert_map, &mut raw);
    }

    build_navmdef(vertices, raw)
}

fn build_navmdef(vertices: Vec<[f32; 3]>, raw: Vec<[u16; 3]>) -> NavmDef {
    let mut edge_map: HashMap<(u16, u16), (usize, usize)> = HashMap::new();
    for (ti, verts) in raw.iter().enumerate() {
        let [v0, v1, v2] = *verts;
        edge_map.insert((v0, v1), (ti, 0));
        edge_map.insert((v1, v2), (ti, 1));
        edge_map.insert((v2, v0), (ti, 2));
    }

    let triangles = raw.iter().enumerate().map(|(ti, verts)| {
        let [v0, v1, v2] = *verts;
        let adj = |a: u16, b: u16| -> i16 {
            match edge_map.get(&(a, b)) {
                Some(&(tj, _)) if tj != ti => tj as i16,
                _ => -1,
            }
        };
        NavmTriangle {
            v:    [v0, v1, v2],
            adj:  [adj(v1, v0), adj(v2, v1), adj(v0, v2)],
            flags: 0x0000_0800,
        }
    }).collect();

    NavmDef { vertices, triangles }
}
