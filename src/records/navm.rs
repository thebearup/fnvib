use super::types::*;

pub struct NavmTriangle {
    pub v:     [u16; 3], // vertex indices
    pub adj:   [i16; 3], // adjacent triangle index per edge, -1 = boundary
    pub flags: u32,
}

pub struct NavmDef {
    pub vertices:  Vec<[f32; 3]>,
    pub triangles: Vec<NavmTriangle>,
}

/// Build a NAVM record from pre-computed geometry.
///
/// `cell_form_id` is the CELL record this navmesh belongs to; it must be
/// embedded in the DATA subrecord before the record is written.
pub fn build_navm(def: &NavmDef, form_id: u32, cell_form_id: u32) -> Record {
    // NVER — version 11 for FNV
    let nver = Subrecord::new(*b"NVER", 11u32.to_le_bytes().to_vec());

    // DATA — cell ref + counts
    let mut db = Vec::with_capacity(24);
    db.extend_from_slice(&cell_form_id.to_le_bytes());
    db.extend_from_slice(&(def.vertices.len() as u32).to_le_bytes());
    db.extend_from_slice(&(def.triangles.len() as u32).to_le_bytes());
    db.extend_from_slice(&0u32.to_le_bytes()); // external connections
    db.extend_from_slice(&0u32.to_le_bytes()); // door portals
    db.extend_from_slice(&0u32.to_le_bytes()); // unused
    let data_sr = Subrecord::new(*b"DATA", db);

    // NVVX — vertex array, 12 bytes each (3 × f32)
    let mut vx = Vec::with_capacity(def.vertices.len() * 12);
    for v in &def.vertices {
        vx.extend_from_slice(&v[0].to_le_bytes());
        vx.extend_from_slice(&v[1].to_le_bytes());
        vx.extend_from_slice(&v[2].to_le_bytes());
    }
    let nvvx = Subrecord::new(*b"NVVX", vx);

    // NVTR — triangle array, 16 bytes each:
    //   3 × i16 vertex indices, 3 × i16 adjacency, u32 flags
    let mut tr = Vec::with_capacity(def.triangles.len() * 16);
    for t in &def.triangles {
        tr.extend_from_slice(&(t.v[0] as i16).to_le_bytes());
        tr.extend_from_slice(&(t.v[1] as i16).to_le_bytes());
        tr.extend_from_slice(&(t.v[2] as i16).to_le_bytes());
        tr.extend_from_slice(&t.adj[0].to_le_bytes());
        tr.extend_from_slice(&t.adj[1].to_le_bytes());
        tr.extend_from_slice(&t.adj[2].to_le_bytes());
        tr.extend_from_slice(&t.flags.to_le_bytes());
    }
    let nvtr = Subrecord::new(*b"NVTR", tr);

    // NVGD — spatial grid
    let nvgd_data = build_nvgd(def);
    let nvgd = Subrecord::new(*b"NVGD", nvgd_data);

    Record::new(*b"NAVM", 0, form_id, &[nver, data_sr, nvvx, nvtr, nvgd])
}

/// Encode the NVGD spatial acceleration structure.
///
/// Format (verified against test.esp):
///   u32  divisor
///   f32  cell_width  = (max_x - min_x) / divisor
///   f32  cell_height = (max_y - min_y) / divisor
///   f32  min_x, min_y, min_z
///   f32  max_x, max_y, max_z
///   For each cell (gy=0..div-1, gx=0..div-1, gx varies fastest):
///     u16  count
///     count × u16  triangle_index
/// Pick a grid `divisor` that scales with triangle count, matching real GECK-generated
/// values decoded from FalloutNV.esm. `divisor=1` on a 100+ triangle mesh (a single
/// bucket listing every triangle) reliably crashes GECK, so this must never default to 1
/// regardless of mesh size.
fn nvgd_divisor(n_tris: usize) -> u32 {
    match n_tris {
        0..=100 => 3,
        101..=200 => 5,
        201..=300 => 7,
        301..=400 => 9,
        401..=500 => 11,
        _ => 12,
    }
}

fn build_nvgd(def: &NavmDef) -> Vec<u8> {
    if def.vertices.is_empty() {
        return Vec::new();
    }

    let (mut min_x, mut min_y, mut min_z) = (f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let (mut max_x, mut max_y, mut max_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for v in &def.vertices {
        if v[0] < min_x { min_x = v[0]; }
        if v[1] < min_y { min_y = v[1]; }
        if v[2] < min_z { min_z = v[2]; }
        if v[0] > max_x { max_x = v[0]; }
        if v[1] > max_y { max_y = v[1]; }
        if v[2] > max_z { max_z = v[2]; }
    }

    let divisor = nvgd_divisor(def.triangles.len());
    let cell_w = (max_x - min_x) / divisor as f32;
    let cell_h = (max_y - min_y) / divisor as f32;

    let mut out = Vec::new();
    out.extend_from_slice(&divisor.to_le_bytes());
    out.extend_from_slice(&cell_w.to_le_bytes());
    out.extend_from_slice(&cell_h.to_le_bytes());
    out.extend_from_slice(&min_x.to_le_bytes());
    out.extend_from_slice(&min_y.to_le_bytes());
    out.extend_from_slice(&min_z.to_le_bytes());
    out.extend_from_slice(&max_x.to_le_bytes());
    out.extend_from_slice(&max_y.to_le_bytes());
    out.extend_from_slice(&max_z.to_le_bytes());

    for gy in 0..divisor {
        for gx in 0..divisor {
            let cx_min = min_x + gx as f32 * cell_w;
            let cx_max = min_x + (gx + 1) as f32 * cell_w;
            let cy_min = min_y + gy as f32 * cell_h;
            let cy_max = min_y + (gy + 1) as f32 * cell_h;

            let mut cell_tris: Vec<u16> = Vec::new();
            for (ti, tri) in def.triangles.iter().enumerate() {
                // AABB of triangle vs cell bounds
                let xs = tri.v.map(|vi| def.vertices[vi as usize][0]);
                let ys = tri.v.map(|vi| def.vertices[vi as usize][1]);
                let tx_min = xs.iter().cloned().fold(f32::INFINITY, f32::min);
                let tx_max = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let ty_min = ys.iter().cloned().fold(f32::INFINITY, f32::min);
                let ty_max = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                if tx_max >= cx_min && tx_min <= cx_max && ty_max >= cy_min && ty_min <= cy_max {
                    cell_tris.push(ti as u16);
                }
            }

            out.extend_from_slice(&(cell_tris.len() as u16).to_le_bytes());
            for idx in cell_tris {
                out.extend_from_slice(&idx.to_le_bytes());
            }
        }
    }

    out
}
