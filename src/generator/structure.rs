use super::GenContext;
use crate::model::room::{Cardinal, PIECE_SIZE};
use crate::records::refr::{RefrDef, Transform};
use std::collections::HashSet;
use std::f32::consts::PI;

/// Generates all structural room pieces.
///
/// Layout for an W×L room (piece grid, 0-indexed from SW):
///
///   (0,L-1) NW corner │ N straight … │ (W-1,L-1) NE corner
///   W straight …      │  room_mid …  │  E straight …
///   (0,0)   SW corner │ S straight … │ (W-1,0)   SE corner
///
/// Doorway `offset` is 0-indexed among the non-corner positions along that wall:
///   offset 0 → column/row index 1 (immediately east/north of the left/bottom corner)
pub fn generate(ctx: &GenContext) -> Vec<RefrDef> {
    let mut refs = Vec::new();
    let room = ctx.room;
    let kit = ctx.kit;
    let (ox, oy) = ctx.offset;

    let w = room.width;
    let l = room.length;

    // ── Corridors ────────────────────────────────────────────────────────────
    // width=1  → N/S corridor (pieces stacked along Y, base piece rotated PI/2)
    // length=1 → E/W corridor (pieces stacked along X, base piece at rot=0)
    //
    // Piece layout (minimum length 2):
    //   End 0   : entrance or terminal piece
    //   Middle  : plain corridor pieces
    //   End 1   : entrance or terminal piece
    //
    // End-piece selection uses the corridor's declared doorways:
    //   straight door (out through the end face) → corridor_entrance
    //   left-side door                           → corridor_entrance_left
    //   right-side door                          → corridor_entrance_right
    //   no door                                  → corridor_terminal
    //
    // For N/S corridors, east/west doorway offsets are ABSOLUTE piece indices
    // (0 = south end, length-1 = north end). Straight N/S doorways use any offset.
    //
    // Rotation convention (matches FNV wall pieces — rot=0 faces east):
    //   rot=0      → facing east   (E/W east-end and middle pieces)
    //   rot=PI*0.5 → facing north  (N/S north-end and middle pieces)
    //   rot=PI     → facing west   (E/W west-end pieces)
    //   rot=PI*1.5 → facing south  (N/S south-end pieces)
    //
    // Left/right assignments may need in-game verification.
    // Canonical door-placement rule: place from the North/East side of each
    // connection so linked pairs don't both place a door. South/West only place
    // for dead-ends (no links_to), which also covers corridor end doorways whose
    // partner room would otherwise be responsible but has the wrong direction.
    let place_door = |dir: Cardinal, offset: u32| -> bool {
        match dir {
            Cardinal::North | Cardinal::East => true,
            Cardinal::South | Cardinal::West => room
                .doorways_on(dir)
                .find(|d| d.offset == offset)
                .map(|d| d.links_to.is_none())
                .unwrap_or(false),
        }
    };

    // Per-kit rotation adjustments, derived from kit fields.
    let mid_adj    = kit.room_mid_rot_adj;
    let wall_adj   = kit.wall_straight_rot_adj;
    let door_adj   = kit.wall_straight_rot_adj + kit.wall_doorway_rot_adj;
    let corner_adj = kit.wall_corner_rot_adj;
    let corr_adj   = kit.corridor_rot_adj;
    let term_adj   = kit.corridor_terminal_rot_adj.unwrap_or(corr_adj);
    let term_off   = kit.corridor_terminal_offset;
    let door_rot   = kit.door_rot_adj;

    // Vault-style kits use geometrically distinct pieces for the perpendicular axis.
    // Fall back to the primary piece when the alternate is absent.
    let wall_ew        = kit.wall_straight_ew.unwrap_or(kit.wall_straight);
    let doorway_ew     = kit.wall_doorway_ew.unwrap_or(kit.wall_doorway);
    let door_ew_adj    = kit.wall_doorway_ew_rot_adj.unwrap_or(door_adj);
    let corner_inner_b  = kit.wall_corner_inner_b.unwrap_or(kit.wall_corner_inner);
    let corner_door_l_b = kit.wall_corner_door_l_b.unwrap_or(kit.wall_corner_door_l);
    let corner_door_r_b = kit.wall_corner_door_r_b.unwrap_or(kit.wall_corner_door_r);
    let wall_ew_adj     = kit.wall_straight_ew_rot_adj.unwrap_or(wall_adj);
    let corner_b_adj    = kit.wall_corner_inner_b_rot_adj.unwrap_or(corner_adj);

    if w == 1 || l == 1 {
        let corridor_piece = |form: Option<u32>, field: &str| -> u32 {
            form.unwrap_or_else(|| {
                eprintln!("error: room '{}' is a corridor but kit does not define '{}'", room.id, field);
                std::process::exit(1);
            })
        };

        let doors_s: HashSet<u32> = room.doorways_on(Cardinal::South).map(|d| d.offset).collect();
        let doors_n: HashSet<u32> = room.doorways_on(Cardinal::North).map(|d| d.offset).collect();
        let doors_w: HashSet<u32> = room.doorways_on(Cardinal::West).map(|d| d.offset).collect();
        let doors_e: HashSet<u32> = room.doorways_on(Cardinal::East).map(|d| d.offset).collect();

        let c          = corridor_piece(kit.corridor,               "corridor");
        let c_ent      = corridor_piece(kit.corridor_entrance,      "corridor_entrance");
        let c_ent_l    = corridor_piece(kit.corridor_entrance_left,  "corridor_entrance_left");
        let c_ent_r    = corridor_piece(kit.corridor_entrance_right, "corridor_entrance_right");
        let c_term     = corridor_piece(kit.corridor_terminal,       "corridor_terminal");

        if w == 1 {
            // N/S corridor — pieces stacked along Y
            if l == 1 {
                // Degenerate single piece: use plain corridor
                refs.push(RefrDef::new(c, Transform::at(ox, oy, 0.0).facing_rad(PI * 0.5 + corr_adj)));
                return refs;
            }

            // South end (yi=0): end piece faces inward (north = PI*0.5)
            //   straight south door → entrance; east door at yi=0 → entrance_left
            //   (facing south: left=east, right=west)
            let south_is_term = doors_s.is_empty() && !doors_e.contains(&0) && !doors_w.contains(&0);
            let south_form = if !doors_s.is_empty()       { c_ent }
                        else if doors_e.contains(&0)       { c_ent_l }
                        else if doors_w.contains(&0)       { c_ent_r }
                        else                               { c_term };
            let south_adj = if south_is_term { term_adj } else { corr_adj };
            let south_y   = oy + if south_is_term { term_off } else { 0.0 };
            refs.push(RefrDef::new(south_form, Transform::at(ox, south_y, 0.0).facing_rad(PI * 0.5 + south_adj)));
            if !doors_s.is_empty() && place_door(Cardinal::South, 0) {
                if let Some(door) = kit.door {
                    refs.push(RefrDef::new(door, Transform::at(ox, oy - PIECE_SIZE * 0.5, 0.0).facing_rad(PI * 0.5 + door_rot)));
                }
            }

            // Middle pieces
            for yi in 1..l - 1 {
                refs.push(RefrDef::new(c, Transform::at(ox, oy + yi as f32 * PIECE_SIZE, 0.0).facing_rad(PI * 0.5 + corr_adj)));
            }

            // North end (yi=L-1): end piece faces inward (south = PI*1.5)
            //   straight north door → entrance; west door at yi=L-1 → entrance_left
            //   (facing north: left=west, right=east)
            let north_is_term = doors_n.is_empty() && !doors_w.contains(&(l - 1)) && !doors_e.contains(&(l - 1));
            let north_form = if !doors_n.is_empty()             { c_ent }
                        else if doors_w.contains(&(l - 1))      { c_ent_l }
                        else if doors_e.contains(&(l - 1))      { c_ent_r }
                        else                                     { c_term };
            let north_adj = if north_is_term { term_adj } else { corr_adj };
            let north_y   = oy + (l - 1) as f32 * PIECE_SIZE - if north_is_term { term_off } else { 0.0 };
            refs.push(RefrDef::new(north_form, Transform::at(ox, north_y, 0.0).facing_rad(PI * 1.5 + north_adj)));
            if !doors_n.is_empty() && place_door(Cardinal::North, 0) {
                if let Some(door) = kit.door {
                    refs.push(RefrDef::new(door, Transform::at(ox, oy + (l - 1) as f32 * PIECE_SIZE + PIECE_SIZE * 0.5, 0.0).facing_rad(PI * 0.5 + door_rot)));
                }
            }

        } else {
            // E/W corridor — pieces stacked along X
            if w == 1 {
                // Degenerate single piece
                refs.push(RefrDef::new(c, Transform::at(ox, oy, 0.0).facing_rad(corr_adj)));
                return refs;
            }

            // West end (xi=0): end piece faces outward (west = rot PI)
            //   straight west door → entrance; south door at xi=0 → entrance_left
            //   (facing west: left=south, right=north)
            let west_is_term = doors_w.is_empty() && !doors_s.contains(&0) && !doors_n.contains(&0);
            let west_form = if !doors_w.is_empty()        { c_ent }
                       else if doors_s.contains(&0)        { c_ent_l }
                       else if doors_n.contains(&0)        { c_ent_r }
                       else                                { c_term };
            let west_adj = if west_is_term { term_adj } else { corr_adj };
            let west_x   = ox + if west_is_term { term_off } else { 0.0 };
            refs.push(RefrDef::new(west_form, Transform::at(west_x, oy, 0.0).facing_rad(PI + west_adj)));
            if !doors_w.is_empty() && place_door(Cardinal::West, 0) {
                if let Some(door) = kit.door {
                    refs.push(RefrDef::new(door, Transform::at(ox - PIECE_SIZE * 0.5, oy, 0.0).facing_rad(door_rot)));
                }
            }

            // Middle pieces
            for xi in 1..w - 1 {
                refs.push(RefrDef::new(c, Transform::at(ox + xi as f32 * PIECE_SIZE, oy, 0.0).facing_rad(corr_adj)));
            }

            // East end (xi=W-1): end piece faces outward (east = rot 0)
            //   straight east door → entrance; north door at xi=W-1 → entrance_left
            //   (facing east: left=north, right=south)
            let east_is_term = doors_e.is_empty() && !doors_n.contains(&(w - 1)) && !doors_s.contains(&(w - 1));
            let east_form = if !doors_e.is_empty()              { c_ent }
                       else if doors_n.contains(&(w - 1))       { c_ent_l }
                       else if doors_s.contains(&(w - 1))       { c_ent_r }
                       else                                      { c_term };
            let east_adj = if east_is_term { term_adj } else { corr_adj };
            let east_x   = ox + (w - 1) as f32 * PIECE_SIZE - if east_is_term { term_off } else { 0.0 };
            refs.push(RefrDef::new(east_form, Transform::at(east_x, oy, 0.0).facing_rad(east_adj)));
            if !doors_e.is_empty() && place_door(Cardinal::East, w - 1) {
                if let Some(door) = kit.door {
                    refs.push(RefrDef::new(door, Transform::at(ox + (w - 1) as f32 * PIECE_SIZE + PIECE_SIZE * 0.5, oy, 0.0).facing_rad(door_rot)));
                }
            }
        }

        return refs;
    }

    // Build doorway offset sets per wall (0-indexed among non-corner positions)
    let doors_s: HashSet<u32> = room.doorways_on(Cardinal::South).map(|d| d.offset).collect();
    let doors_n: HashSet<u32> = room.doorways_on(Cardinal::North).map(|d| d.offset).collect();
    let doors_w: HashSet<u32> = room.doorways_on(Cardinal::West).map(|d| d.offset).collect();
    let doors_e: HashSet<u32> = room.doorways_on(Cardinal::East).map(|d| d.offset).collect();

    // ── Interior (room_mid): every position not on the perimeter ──────────────
    for xi in 1..w.saturating_sub(1) {
        for yi in 1..l.saturating_sub(1) {
            let x = ox + xi as f32 * PIECE_SIZE;
            let y = oy + yi as f32 * PIECE_SIZE;
            refs.push(RefrDef::new(kit.room_mid, Transform::at(x, y, 0.0).facing_rad(mid_adj)));
        }
    }

    // Wall rotation constants.
    const ROT_S: f32 = PI * -1.0;
    const ROT_N: f32 = PI *  2.0;
    const ROT_W: f32 = PI * -0.5;
    const ROT_E: f32 = PI *  0.5;

    // ── South wall (y=0): non-corner positions xi ∈ [1, w-2] ─────────────────
    for xi in 1..w.saturating_sub(1) {
        let door_offset = xi - 1;
        let is_door = doors_s.contains(&door_offset);
        let (base, rot) = if is_door { (kit.wall_doorway, ROT_S + door_adj) } else { (kit.wall_straight, ROT_S + wall_adj) };
        refs.push(RefrDef::new(base, Transform::at(ox + xi as f32 * PIECE_SIZE, oy, 0.0).facing_rad(rot)));
        if is_door && place_door(Cardinal::South, door_offset) {
            if let Some(door) = kit.door {
                refs.push(RefrDef::new(door, Transform::at(ox + xi as f32 * PIECE_SIZE, oy - PIECE_SIZE * 0.5, 0.0).facing_rad(PI * 0.5 + door_rot)));
            }
        }
    }

    // ── North wall (y=L-1): non-corner positions xi ∈ [1, w-2] ──────────────
    let ny = (l - 1) as f32 * PIECE_SIZE;
    for xi in 1..w.saturating_sub(1) {
        let door_offset = xi - 1;
        let is_door = doors_n.contains(&door_offset);
        let (base, rot) = if is_door { (kit.wall_doorway, ROT_N + door_adj) } else { (kit.wall_straight, ROT_N + wall_adj) };
        refs.push(RefrDef::new(base, Transform::at(ox + xi as f32 * PIECE_SIZE, oy + ny, 0.0).facing_rad(rot)));
        if is_door && place_door(Cardinal::North, door_offset) {
            if let Some(door) = kit.door {
                refs.push(RefrDef::new(door, Transform::at(ox + xi as f32 * PIECE_SIZE, oy + ny + PIECE_SIZE * 0.5, 0.0).facing_rad(PI * 0.5 + door_rot)));
            }
        }
    }

    // ── West wall (x=0): non-corner positions yi ∈ [1, l-2] ─────────────────
    for yi in 1..l.saturating_sub(1) {
        let door_offset = yi - 1;
        let is_door = doors_w.contains(&door_offset);
        let (base, rot) = if is_door { (doorway_ew, ROT_W + door_ew_adj) } else { (wall_ew, ROT_W + wall_ew_adj) };
        refs.push(RefrDef::new(base, Transform::at(ox, oy + yi as f32 * PIECE_SIZE, 0.0).facing_rad(rot)));
        if is_door && place_door(Cardinal::West, door_offset) {
            if let Some(door) = kit.door {
                refs.push(RefrDef::new(door, Transform::at(ox - PIECE_SIZE * 0.5, oy + yi as f32 * PIECE_SIZE, 0.0).facing_rad(door_rot)));
            }
        }
    }

    // ── East wall (x=W-1): non-corner positions yi ∈ [1, l-2] ───────────────
    let ex = (w - 1) as f32 * PIECE_SIZE;
    for yi in 1..l.saturating_sub(1) {
        let door_offset = yi - 1;
        let is_door = doors_e.contains(&door_offset);
        let (base, rot) = if is_door { (doorway_ew, ROT_E + door_ew_adj) } else { (wall_ew, ROT_E + wall_ew_adj) };
        refs.push(RefrDef::new(base, Transform::at(ox + ex, oy + yi as f32 * PIECE_SIZE, 0.0).facing_rad(rot)));
        if is_door && place_door(Cardinal::East, door_offset) {
            if let Some(door) = kit.door {
                refs.push(RefrDef::new(door, Transform::at(ox + ex + PIECE_SIZE * 0.5, oy + yi as f32 * PIECE_SIZE, 0.0).facing_rad(door_rot)));
            }
        }
    }

    // ── Corners ───────────────────────────────────────────────────────────────
    // Pieces face +X (east) at rot=0. Going counter-clockwise around the room
    // from SW, each corner adds PI/2; SE and NW are the exceptions that subtract
    // PI/2 from the naive sequence (confirmed by in-game observation).
    //   SW (0,0)       rot PI/2    opens NE
    //   SE (W-1,0)     rot 0       opens NW
    //   NE (W-1,L-1)   rot 3PI/2  opens SW
    //   NW (0,L-1)     rot PI      opens SE
    //
    // Corner-door pieces are used when a doorway's offset resolves to a corner
    // position (i.e. no non-corner wall slot exists there). The offset math
    // mirrors the wall loops: a doorway at offset W−2 on the south wall would
    // be at xi=W−1 (SE corner) if it were a wall piece.
    //
    // Mapping:
    //   SE south face: doors_s offset = w-2
    //   NE north face: doors_n offset = w-2
    //   NE east  face: doors_e offset = l-2  (east takes priority over north)
    //   NW west  face: doors_w offset = l-2
    //
    // L/R: _l = door on the left of the corner seam; _r = door on the right.
    let se_south = w >= 2 && doors_s.contains(&(w - 2));
    let ne_north = w >= 2 && doors_n.contains(&(w - 2));
    let ne_east  = l >= 2 && doors_e.contains(&(l - 2));
    let nw_west  = l >= 2 && doors_w.contains(&(l - 2));

    let ne_form = if ne_east  { kit.wall_corner_door_l }
             else if ne_north { kit.wall_corner_door_r }
             else             { kit.wall_corner_inner   };

    // Per-door-type rotation overrides (replace corner_adj entirely when set).
    let cdl_adj    = kit.wall_corner_door_l_rot_adj.unwrap_or(corner_adj);
    let cdr_adj    = kit.wall_corner_door_r_rot_adj.unwrap_or(corner_adj);
    // Diagonal-B door rotation overrides fall back to their diagonal-A counterparts.
    let cdl_b_adj  = kit.wall_corner_door_l_b_rot_adj.unwrap_or(cdl_adj);
    let cdr_b_adj  = kit.wall_corner_door_r_b_rot_adj.unwrap_or(cdr_adj);
    let cdl_offset = kit.wall_corner_door_l_offset;

    let sw_rot = PI * -0.5 + corner_adj;
    let se_rot = PI * -1.0 + if se_south { cdr_b_adj } else { corner_b_adj };
    let ne_rot = PI * -1.5 + if ne_east  { cdl_adj  } else if ne_north { cdr_adj } else { corner_adj };
    let nw_rot = PI * -2.0 + if nw_west  { cdl_b_adj } else { corner_b_adj };

    // wall_corner_door_l meshes on some kits have their origin offset from the nominal
    // corner position. cdl_offset corrects this: -X at NE, -Y at NW (both toward interior).
    let ne_x = ox + ex - if ne_east { cdl_offset } else { 0.0 };
    let nw_y = oy + ny - if nw_west { cdl_offset } else { 0.0 };

    let corners = [
        (ox,      oy,      sw_rot, kit.wall_corner_inner),                                                          // SW — diagonal A
        (ox + ex, oy,      se_rot, if se_south { corner_door_r_b } else { corner_inner_b }),                        // SE — diagonal B
        (ne_x,    oy + ny, ne_rot, ne_form),                                                                        // NE — diagonal A
        (ox,      nw_y,    nw_rot, if nw_west  { corner_door_l_b } else { corner_inner_b }),                        // NW — diagonal B
    ];
    for (cx, cy, cr, form) in corners {
        refs.push(RefrDef::new(form, Transform::at(cx, cy, 0.0).facing_rad(cr)));
    }

    refs
}
