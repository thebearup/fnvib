pub mod clutter;
pub mod furniture;
pub mod lighting;
pub mod navmesh;
pub mod structure;

use crate::content_lists::ContentItem;
use crate::kits::KitData;
use crate::model::room::Room;
use crate::records::refr::RefrDef;
use rand::rngs::StdRng;
use std::collections::HashMap;

/// Shared context threaded through every generation step.
pub struct GenContext<'a> {
    pub room: &'a Room,
    pub kit: &'a KitData,
    pub rng: &'a mut StdRng,
    /// World-space (x, y) origin of this room inside the shared cell.
    pub offset: (f32, f32),
    /// Resolved content pools for this room, derived from the room's list selections.
    pub furniture: Vec<ContentItem>,
    pub floor_clutter: Vec<ContentItem>,
    pub surface_clutter: Vec<ContentItem>,
    pub wall_decorations: Vec<ContentItem>,
    pub lights: Vec<ContentItem>,
    /// Full content list registry, used to resolve surface_scatter list references.
    pub lists: &'a HashMap<String, Vec<ContentItem>>,
}

/// Run all generation layers in order, returning the full list of refs to place.
///
/// Order: structure → furniture → floor clutter → wall decorations → lighting
/// Surface clutter is placed by furniture::generate via [[items.surface_scatter]],
/// not by a separate pass. Navmesh is handled separately (returns a NAVM record).
pub fn run_pipeline(ctx: &mut GenContext) -> Vec<RefrDef> {
    let mut refs = Vec::new();
    refs.extend(structure::generate(ctx));
    refs.extend(furniture::generate(ctx));
    refs.extend(clutter::generate_floor(ctx));
    refs.extend(clutter::generate_wall_decorations(ctx));
    refs.extend(lighting::generate(ctx));
    refs
}
