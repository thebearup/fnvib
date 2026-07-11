use super::types::*;
use crate::records::navm::NavmDef;

/// FalloutNV.esm base NAVI record — plugins override this to register new NAVMs.
const NAVI_FID: u32 = 0x0001_4B92;

/// Build the top-level NAVI record referencing all provided NAVMs.
///
/// GECK requires every NAVM to have a corresponding NVMI entry in a NAVI record
/// before it will accept the file as valid. Without this, GECK reports navmesh
/// warnings for every triangle on load/save even when the geometry is correct.
///
/// The single NAVI record in FNV (fid=0x00014B92, from FalloutNV.esm) is
/// overridden by every plugin that adds navmeshes; each plugin writes a fresh
/// copy containing NVMI+NVCI entries for its own NAVMs.
pub fn build_navi(
    entries: &[(&NavmDef, u32, u32)], // (def, navm_fid, cell_fid)
) -> Record {
    let nver = Subrecord::new(*b"NVER", 11u32.to_le_bytes().to_vec());

    let mut subrecords: Vec<Subrecord> = vec![nver];

    for &(def, navm_fid, cell_fid) in entries {
        subrecords.push(build_nvmi(def, navm_fid, cell_fid));
    }
    for &(_, navm_fid, _) in entries {
        subrecords.push(build_nvci(navm_fid));
    }

    Record::new(*b"NAVI", 0, NAVI_FID, &subrecords)
}

/// NVMI subrecord — full navmesh info (flags=0x20).
///
/// Format (verified against test_fixed.esp NAVI decoded output):
///   u32  flags = 0x00000020  (full entry)
///   u32  navm_fid
///   u32  cell_fid
///   u32  padding = 0
///   f32×3  centroid (average of all vertices)
///   f32×3  bbox_min
///   f32×3  bbox_max
///   u16  n_verts
///   u16  n_tris
///   [n_verts × f32×3]  vertex positions
///   [n_tris  × (u16 v0, u16 v1, u16 v2, u32 flags=0)]
fn build_nvmi(def: &NavmDef, navm_fid: u32, cell_fid: u32) -> Subrecord {
    let mut data = Vec::new();

    data.extend_from_slice(&0x0000_0020u32.to_le_bytes()); // flags = full
    data.extend_from_slice(&navm_fid.to_le_bytes());
    data.extend_from_slice(&cell_fid.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // padding

    let n = def.vertices.len() as f32;
    let cx = def.vertices.iter().map(|v| v[0]).sum::<f32>() / n;
    let cy = def.vertices.iter().map(|v| v[1]).sum::<f32>() / n;
    let cz = def.vertices.iter().map(|v| v[2]).sum::<f32>() / n;
    data.extend_from_slice(&cx.to_le_bytes());
    data.extend_from_slice(&cy.to_le_bytes());
    data.extend_from_slice(&cz.to_le_bytes());

    let mut min_x = f32::INFINITY;  let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;  let mut max_y = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;  let mut max_z = f32::NEG_INFINITY;
    for v in &def.vertices {
        min_x = min_x.min(v[0]); max_x = max_x.max(v[0]);
        min_y = min_y.min(v[1]); max_y = max_y.max(v[1]);
        min_z = min_z.min(v[2]); max_z = max_z.max(v[2]);
    }
    data.extend_from_slice(&min_x.to_le_bytes());
    data.extend_from_slice(&min_y.to_le_bytes());
    data.extend_from_slice(&min_z.to_le_bytes());
    data.extend_from_slice(&max_x.to_le_bytes());
    data.extend_from_slice(&max_y.to_le_bytes());
    data.extend_from_slice(&max_z.to_le_bytes());

    data.extend_from_slice(&(def.vertices.len() as u16).to_le_bytes());
    data.extend_from_slice(&(def.triangles.len() as u16).to_le_bytes());

    for v in &def.vertices {
        data.extend_from_slice(&v[0].to_le_bytes());
        data.extend_from_slice(&v[1].to_le_bytes());
        data.extend_from_slice(&v[2].to_le_bytes());
    }
    for t in &def.triangles {
        data.extend_from_slice(&t.v[0].to_le_bytes());
        data.extend_from_slice(&t.v[1].to_le_bytes());
        data.extend_from_slice(&t.v[2].to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // no external connections
    }

    Subrecord::new(*b"NVMI", data)
}

/// NVCI subrecord — lists this NAVM's fid plus zero-padded space for connections.
fn build_nvci(navm_fid: u32) -> Subrecord {
    let mut data = Vec::new();
    data.extend_from_slice(&navm_fid.to_le_bytes());
    data.extend_from_slice(&[0u8; 12]);
    Subrecord::new(*b"NVCI", data)
}
