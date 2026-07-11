use super::types::*;

pub const CELL_FLAG_INTERIOR: u8 = 0x01;
pub const CELL_FLAG_HAS_WATER: u8 = 0x02;
pub const CELL_FLAG_PUBLIC_PLACE: u8 = 0x20;

/// Lighting settings for an interior cell (XCLL subrecord).
#[derive(Debug, Clone)]
pub struct CellLighting {
    pub ambient: [u8; 4],    // RGBA
    pub directional: [u8; 4], // RGBA
    pub fog_color: [u8; 4],   // RGBA
    pub fog_near: f32,
    pub fog_far: f32,
    pub dir_rotation_x: f32,
    pub dir_rotation_y: f32,
    pub dir_fade: f32,
    pub fog_clip_dist: f32,
    pub fog_power: f32,
}

impl Default for CellLighting {
    fn default() -> Self {
        Self {
            ambient: [0x3F, 0x3F, 0x3F, 0xFF],
            directional: [0x80, 0x80, 0x80, 0xFF],
            fog_color: [0x80, 0x80, 0x80, 0xFF],
            fog_near: 0.0,
            fog_far: 27_000.0,
            dir_rotation_x: 0.0,
            dir_rotation_y: 0.0,
            dir_fade: 1.0,
            fog_clip_dist: 27_000.0,
            fog_power: 1.0,
        }
    }
}

impl CellLighting {
    fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.ambient);
        v.extend_from_slice(&self.directional);
        v.extend_from_slice(&self.fog_color);
        v.extend_from_slice(&self.fog_near.to_le_bytes());
        v.extend_from_slice(&self.fog_far.to_le_bytes());
        v.extend_from_slice(&self.dir_rotation_x.to_le_bytes());
        v.extend_from_slice(&self.dir_rotation_y.to_le_bytes());
        v.extend_from_slice(&self.dir_fade.to_le_bytes());
        v.extend_from_slice(&self.fog_clip_dist.to_le_bytes());
        v.extend_from_slice(&self.fog_power.to_le_bytes());
        v
    }
}

/// Parameters for a new interior cell.
#[derive(Debug, Clone)]
pub struct CellDef {
    pub editor_id: String,
    pub name: String,
    pub flags: u8,
    pub lighting: CellLighting,
    /// FormID of an acoustic space record (ASPC), or None
    pub acoustic_space: Option<u32>,
    /// FormID of a music type record (MUSC), or None
    pub music_type: Option<u32>,
    /// FormID of an image space record (IMGS), or None
    pub image_space: Option<u32>,
    /// FormID of an encounter zone record (ECZN), or None
    pub encounter_zone: Option<u32>,
}

impl CellDef {
    pub fn new(editor_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            editor_id: editor_id.into(),
            name: name.into(),
            flags: CELL_FLAG_INTERIOR,
            lighting: CellLighting::default(),
            acoustic_space: None,
            music_type: None,
            image_space: None,
            encounter_zone: None,
        }
    }
}

/// Builds the CELL record for an interior cell.
pub fn build_cell(def: &CellDef, form_id: u32) -> Record {
    let mut subrecords = Vec::new();

    subrecords.push(Subrecord::new(*b"EDID", zstring(&def.editor_id)));
    subrecords.push(Subrecord::new(*b"FULL", zstring(&def.name)));
    subrecords.push(Subrecord::new(*b"DATA", u8_bytes(def.flags)));
    subrecords.push(Subrecord::new(*b"XCLL", def.lighting.to_bytes()));

    if let Some(id) = def.acoustic_space {
        subrecords.push(Subrecord::new(*b"XCAS", formid_bytes(id)));
    }
    if let Some(id) = def.music_type {
        subrecords.push(Subrecord::new(*b"XCMO", formid_bytes(id)));
    }
    if let Some(id) = def.image_space {
        subrecords.push(Subrecord::new(*b"XCIM", formid_bytes(id)));
    }
    if let Some(id) = def.encounter_zone {
        subrecords.push(Subrecord::new(*b"XEZN", formid_bytes(id)));
    }

    Record::new(*b"CELL", 0, form_id, &subrecords)
}

/// Wraps a CELL record and its reference groups into the interior block/sub-block group hierarchy.
///
/// Required layout (matches xEdit / vanilla FNV):
///   GRUP type=2 (interior cell block, label = block number)
///     GRUP type=3 (interior cell sub-block, label = sub-block number)
///       CELL record
///       GRUP type=6 (cell children, label = cell FormID)
///         GRUP type=8 (persistent children, label = cell FormID)  — omitted if empty
///         GRUP type=9 (temporary children, label = cell FormID)   — omitted if empty
///
/// Block/sub-block numbers: block = (formID & 0xFFFF) / 1000, sub-block = (formID & 0xFFFF) / 100.
pub fn wrap_cell_in_groups(cell: &Record, persistent: Group, temporary: Group) -> Group {
    let form_id = cell.form_id;
    let block_num = (form_id & 0xFFFF) / 1000;
    let sub_block_num = (form_id & 0xFFFF) / 100;

    // Type-6 "cell children" group wraps type-8 and type-9; skip empty child groups.
    let mut children = Group::new(form_id.to_le_bytes(), 6);
    if !persistent.content.is_empty() {
        children.push_group(&persistent);
    }
    if !temporary.content.is_empty() {
        children.push_group(&temporary);
    }

    let mut cell_content = Vec::new();
    cell.write_to(&mut cell_content);
    children.write_to(&mut cell_content);

    let mut sub_block = Group::new(sub_block_num.to_le_bytes(), 3);
    sub_block.content = cell_content;

    let mut block = Group::new(block_num.to_le_bytes(), 2);
    block.push_group(&sub_block);

    block
}
