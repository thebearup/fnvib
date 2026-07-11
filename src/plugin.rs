use crate::content_lists::{ContentData, ContentItem};
use crate::generator::{self, navmesh, GenContext};
use crate::model::layout::Layout;
use crate::model::room::{Cardinal, Room, PIECE_SIZE};
use crate::records::navm::NavmDef;
use crate::records::types::*;
use crate::records::{cell, navi, navm, refr, tes4};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::collections::{HashMap, VecDeque};
use std::io::Write;

/// Resolves a list of named content lists into a flat ContentItem pool.
/// Unknown list names are silently skipped.
fn resolve_pool(registry: &HashMap<String, Vec<ContentItem>>, selections: &[String]) -> Vec<ContentItem> {
    selections.iter()
        .flat_map(|name| registry.get(name).into_iter().flatten())
        .cloned()
        .collect()
}

/// Tracks FormID allocation for a plugin.
///
/// The high byte of a FormID is a mod-index: an index into the plugin's master list,
/// with the plugin itself being the entry after all masters.
///
/// Example: one master (FalloutNV.esm at index 0) → plugin's own records use index 1.
/// So new FormIDs start at 0x01_000800, NOT 0x00_000800 (which would mean FalloutNV.esm).
pub struct FormIdAllocator {
    next: u32,
}

impl FormIdAllocator {
    pub fn new(start: u32) -> Self {
        Self { next: start }
    }

    pub fn alloc(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }

    pub fn peek_next(&self) -> u32 {
        self.next
    }
}

/// A fully populated interior cell ready for serialization.
pub struct InteriorCell {
    pub def: cell::CellDef,
    pub persistent_refs: Vec<refr::RefrDef>,
    pub temporary_refs: Vec<refr::RefrDef>,
    pub navmeshes: Vec<NavmDef>,
}

impl InteriorCell {
    pub fn new(def: cell::CellDef) -> Self {
        Self { def, persistent_refs: Vec::new(), temporary_refs: Vec::new(), navmeshes: Vec::new() }
    }

    pub fn add_temporary(&mut self, r: refr::RefrDef) {
        self.temporary_refs.push(r);
    }

    pub fn add_persistent(&mut self, r: refr::RefrDef) {
        self.persistent_refs.push(r);
    }
}

/// Maps a doorway offset to the absolute piece index used for alignment.
///
/// Normal rooms skip the corner piece: offset 0 → piece index 1.
/// Corridors use the raw piece index: a side doorway at yi/xi=0 has offset 0.
fn doorway_piece_idx(room: &Room, offset: u32) -> i32 {
    if room.width == 1 || room.length == 1 { offset as i32 } else { offset as i32 + 1 }
}

/// BFS over the doorway graph to assign each room a world-space (x, y) origin.
///
/// Room[0] starts at (0, 0). Each linked neighbor is placed so that the two
/// doorway openings share the same wall edge and the door piece columns align.
///
/// Both sides of a connection must declare `links_to`; if the reciprocal link is
/// missing the connection is skipped and the orphaned room overlaps at (0, 0).
pub fn compute_room_offsets(layout: &Layout) -> HashMap<String, (f32, f32)> {
    let mut offsets: HashMap<String, (f32, f32)> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    offsets.insert(layout.rooms[0].id.clone(), (0.0, 0.0));
    queue.push_back(layout.rooms[0].id.clone());

    while let Some(room_id) = queue.pop_front() {
        let room = layout.room(&room_id).unwrap();
        let (ox, oy) = *offsets.get(&room_id).unwrap();

        for door in &room.doorways {
            let neighbor_id = match &door.links_to {
                Some(id) => id.clone(),
                None => continue,
            };
            if offsets.contains_key(&neighbor_id) {
                continue;
            }
            let neighbor = match layout.room(&neighbor_id) {
                Some(r) => r,
                None => continue,
            };
            // Find the neighbor's doorway that links back so we can align offsets.
            let ndoor = match neighbor.doorways.iter()
                .find(|d| d.links_to.as_deref() == Some(room_id.as_str()))
            {
                Some(d) => d,
                None => continue, // one-way link; skip
            };

            let d  = doorway_piece_idx(room,     door.offset);
            let nd = doorway_piece_idx(neighbor, ndoor.offset);

            let (nx, ny) = match door.direction {
                Cardinal::North => (
                    ox + (d - nd) as f32 * PIECE_SIZE,
                    oy + room.length as f32 * PIECE_SIZE,
                ),
                Cardinal::South => (
                    ox + (d - nd) as f32 * PIECE_SIZE,
                    oy - neighbor.length as f32 * PIECE_SIZE,
                ),
                Cardinal::East => (
                    ox + room.width as f32 * PIECE_SIZE,
                    oy + (d - nd) as f32 * PIECE_SIZE,
                ),
                Cardinal::West => (
                    ox - neighbor.width as f32 * PIECE_SIZE,
                    oy + (d - nd) as f32 * PIECE_SIZE,
                ),
            };

            offsets.insert(neighbor_id.clone(), (nx, ny));
            queue.push_back(neighbor_id);
        }
    }

    offsets
}

/// Returns the world-space center of the interior tile adjacent to the first door
/// placed in the layout, for COCMarker placement.
fn first_door_interior_tile(layout: &Layout, offsets: &HashMap<String, (f32, f32)>) -> Option<(f32, f32)> {
    for room in &layout.rooms {
        let &(ox, oy) = match offsets.get(&room.id) {
            Some(o) => o,
            None => continue,
        };
        for door in &room.doorways {
            let has_door = match door.direction {
                Cardinal::North | Cardinal::East => true,
                Cardinal::South | Cardinal::West => door.links_to.is_none(),
            };
            if !has_door { continue; }
            let pi = doorway_piece_idx(room, door.offset) as f32;
            let pos = match door.direction {
                Cardinal::North => {
                    let yi = (room.length as i32 - 2).max(0) as f32;
                    (ox + pi * PIECE_SIZE, oy + yi * PIECE_SIZE)
                },
                Cardinal::South => {
                    let yi = (1_i32).min(room.length as i32 - 1) as f32;
                    (ox + pi * PIECE_SIZE, oy + yi * PIECE_SIZE)
                },
                Cardinal::East => {
                    let xi = (room.width as i32 - 2).max(0) as f32;
                    (ox + xi * PIECE_SIZE, oy + pi * PIECE_SIZE)
                },
                Cardinal::West => {
                    let xi = (1_i32).min(room.width as i32 - 1) as f32;
                    (ox + xi * PIECE_SIZE, oy + pi * PIECE_SIZE)
                },
            };
            return Some(pos);
        }
    }
    None
}

/// Assembles a complete .esp plugin file in memory.
pub struct Plugin {
    pub author: String,
    pub description: String,
    pub masters: Vec<String>,
    pub cells: Vec<InteriorCell>,
}

impl Plugin {
    pub fn new(author: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            author: author.into(),
            description: description.into(),
            masters: vec!["FalloutNV.esm".into()],
            cells: Vec::new(),
        }
    }

    /// Build a Plugin by running the full generation pipeline over a Layout.
    ///
    /// All rooms in a layout share a single CELL. Each room's world-space (x, y)
    /// origin is computed by BFS over the `links_to` doorway graph, so connected
    /// rooms physically abut with their doorway openings aligned.
    pub fn from_layout(layout: &Layout, seed: u64, content: &ContentData) -> Self {
        let mut plugin = Plugin::new("fnvib", &layout.description);
        if layout.rooms.is_empty() {
            return plugin;
        }

        let offsets = compute_room_offsets(layout);

        // One CELL for the entire layout; audio properties from the first room's kit.
        let first_kit = content.kits.get(&layout.rooms[0].kit)
            .unwrap_or_else(|| { eprintln!("error: unknown kit '{}'", layout.rooms[0].kit); std::process::exit(1); });
        let cell_id = layout.editor_id();
        let mut cell_def = cell::CellDef::new(&cell_id, &layout.name);
        cell_def.acoustic_space = first_kit.acoustic_space;
        cell_def.music_type     = first_kit.music_type;
        cell_def.image_space    = first_kit.image_space;

        let mut interior = InteriorCell::new(cell_def);
        let mut rng = StdRng::seed_from_u64(seed);

        for room in &layout.rooms {
            let kit = content.kits.get(&room.kit)
                .unwrap_or_else(|| { eprintln!("error: unknown kit '{}' in room '{}'", room.kit, room.id); std::process::exit(1); });
            let offset = *offsets.get(&room.id).unwrap_or(&(0.0, 0.0));
            let furniture        = resolve_pool(&content.lists, &room.furniture);
            let floor_clutter    = resolve_pool(&content.lists, &room.floor_clutter);
            let surface_clutter  = resolve_pool(&content.lists, &room.surface_clutter);
            let wall_decorations = resolve_pool(&content.lists, &room.wall_decorations);
            let lights           = resolve_pool(&content.lists, &room.lights);
            let mut ctx = GenContext {
                room, kit, rng: &mut rng, offset,
                furniture, floor_clutter, surface_clutter, wall_decorations, lights,
                lists: &content.lists,
            };
            for r in generator::run_pipeline(&mut ctx) {
                interior.add_temporary(r);
            }
        }

        // Single unified navmesh for the whole cell.
        let nav_rooms: Vec<(&Room, f32, f32)> = layout.rooms.iter()
            .map(|r| {
                let &(ox, oy) = offsets.get(&r.id).unwrap_or(&(0.0, 0.0));
                (r, ox, oy)
            })
            .collect();
        interior.navmeshes.push(navmesh::generate(&nav_rooms));

        // COCMarker — player spawn point near the first door
        if let Some((mx, my)) = first_door_interior_tile(layout, &offsets) {
            interior.add_persistent(refr::RefrDef::new(
                0x0000_0032,
                refr::Transform::at(mx, my, 0.0),
            ));
        }

        // North Marker — outside cell bounds by convention
        let north_y = layout.rooms.iter()
            .filter_map(|r| offsets.get(&r.id).map(|&(_, oy)| oy + r.length as f32 * PIECE_SIZE))
            .fold(f32::NEG_INFINITY, f32::max);
        interior.add_persistent(refr::RefrDef::new(
            0x0000_0003,
            refr::Transform::at(0.0, north_y + PIECE_SIZE, 0.0),
        ));

        plugin.cells.push(interior);
        plugin
    }

    /// Serialize the plugin to a byte buffer.
    pub fn serialize(&self) -> Vec<u8> {
        // High byte = number of masters: index 0..N-1 are masters, index N is this plugin.
        let mod_index = self.masters.len() as u32;
        let mut ids = FormIdAllocator::new((mod_index << 24) | 0x0000_0800);

        struct CellIds { cell_fid: u32, navm_fids: Vec<u32> }
        let cell_ids: Vec<CellIds> = self.cells.iter().map(|interior| {
            let cell_fid = ids.alloc();
            let navm_fids = (0..interior.navmeshes.len()).map(|_| ids.alloc()).collect();
            CellIds { cell_fid, navm_fids }
        }).collect();

        // Build CELL blocks (REFR IDs allocated here, after NAVM IDs).
        let mut cell_blocks: Vec<Group> = Vec::new();
        for (interior, cids) in self.cells.iter().zip(&cell_ids) {
            let mut persistent = Group::new(cids.cell_fid.to_le_bytes(), 8);
            for def in &interior.persistent_refs {
                let rec = refr::build_refr(def, ids.alloc());
                persistent.push_record(&rec);
            }

            let mut temporary = Group::new(cids.cell_fid.to_le_bytes(), 9);
            for def in &interior.temporary_refs {
                let rec = refr::build_refr(def, ids.alloc());
                temporary.push_record(&rec);
            }
            for (navm_def, &navm_fid) in interior.navmeshes.iter().zip(&cids.navm_fids) {
                let rec = navm::build_navm(navm_def, navm_fid, cids.cell_fid);
                temporary.push_record(&rec);
            }

            let cell_rec = cell::build_cell(&interior.def, cids.cell_fid);
            let block = cell::wrap_cell_in_groups(&cell_rec, persistent, temporary);
            cell_blocks.push(block);
        }

        let mut top_cell_group = Group::new(*b"CELL", 0);
        for block in &cell_blocks {
            top_cell_group.push_group(block);
        }

        // NAVI group — one entry per NAVM across all cells.
        let navi_entries: Vec<(&NavmDef, u32, u32)> = self.cells.iter()
            .zip(&cell_ids)
            .flat_map(|(interior, cids)| {
                interior.navmeshes.iter()
                    .zip(&cids.navm_fids)
                    .map(|(def, &navm_fid)| (def, navm_fid, cids.cell_fid))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut navi_group = Group::new(*b"NAVI", 0);
        if !navi_entries.is_empty() {
            let navi_rec = navi::build_navi(&navi_entries);
            navi_group.push_record(&navi_rec);
        }

        let master_refs: Vec<&str> = self.masters.iter().map(|s| s.as_str()).collect();
        // Store nextObjectId as a 24-bit object ID only (strip the mod-index high byte).
        let next_object_id = ids.peek_next() & 0x00FF_FFFF;
        let tes4 = tes4::build_tes4(&self.author, &self.description, &master_refs, next_object_id);

        let mut out = Vec::new();
        tes4.write_to(&mut out);
        navi_group.write_to(&mut out);
        top_cell_group.write_to(&mut out);
        out
    }

    /// Write the plugin to a file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let bytes = self.serialize();
        let mut f = std::fs::File::create(path)?;
        f.write_all(&bytes)
    }
}
