use super::types::*;
use std::f32::consts::PI;

/// Position and rotation for a placed reference.
#[derive(Debug, Clone, Copy, Default)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32, // radians
    pub rot_y: f32,
    pub rot_z: f32,
}

impl Transform {
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z, ..Default::default() }
    }

    pub fn with_rotation(mut self, rot_x: f32, rot_y: f32, rot_z: f32) -> Self {
        self.rot_x = rot_x;
        self.rot_y = rot_y;
        self.rot_z = rot_z;
        self
    }

    pub fn facing_deg(mut self, degrees: f32) -> Self {
        self.rot_z = degrees * PI / 180.0;
        self
    }

    pub fn facing_rad(mut self, radians: f32) -> Self {
        self.rot_z = radians;
        self
    }

    fn to_bytes(self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.x.to_le_bytes());
        v.extend_from_slice(&self.y.to_le_bytes());
        v.extend_from_slice(&self.z.to_le_bytes());
        v.extend_from_slice(&self.rot_x.to_le_bytes());
        v.extend_from_slice(&self.rot_y.to_le_bytes());
        v.extend_from_slice(&self.rot_z.to_le_bytes());
        v
    }
}

/// Teleport destination data for a door reference (XTEL subrecord).
#[derive(Debug, Clone, Copy)]
pub struct TeleportDest {
    pub target_door_form_id: u32,
    pub transform: Transform,
    pub flags: u32, // bit 0x01 = no alarm on teleport
}

impl TeleportDest {
    fn to_bytes(self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.target_door_form_id.to_le_bytes());
        v.extend_from_slice(&self.transform.to_bytes());
        v.extend_from_slice(&self.flags.to_le_bytes());
        v
    }
}

/// A placed object reference.
#[derive(Debug, Clone)]
pub struct RefrDef {
    pub editor_id: Option<String>,
    /// FormID of the base form (STAT, LIGH, DOOR, CONT, etc.)
    pub base_form_id: u32,
    pub transform: Transform,
    /// Scale, None means omit (engine treats absent as 1.0)
    pub scale: Option<f32>,
    /// If this is a door, its teleport destination
    pub teleport: Option<TeleportDest>,
}

impl RefrDef {
    pub fn new(base_form_id: u32, transform: Transform) -> Self {
        Self {
            editor_id: None,
            base_form_id,
            transform,
            scale: None,
            teleport: None,
        }
    }

    pub fn with_editor_id(mut self, id: impl Into<String>) -> Self {
        self.editor_id = Some(id.into());
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }

    pub fn with_teleport(mut self, dest: TeleportDest) -> Self {
        self.teleport = Some(dest);
        self
    }
}

/// Builds a REFR record.
pub fn build_refr(def: &RefrDef, form_id: u32) -> Record {
    let mut subrecords = Vec::new();

    if let Some(ref eid) = def.editor_id {
        subrecords.push(Subrecord::new(*b"EDID", zstring(eid)));
    }

    subrecords.push(Subrecord::new(*b"NAME", formid_bytes(def.base_form_id)));
    subrecords.push(Subrecord::new(*b"DATA", def.transform.to_bytes()));

    if let Some(scale) = def.scale {
        subrecords.push(Subrecord::new(*b"XSCL", f32_bytes(scale)));
    }

    if let Some(ref tel) = def.teleport {
        subrecords.push(Subrecord::new(*b"XTEL", tel.to_bytes()));
    }

    Record::new(*b"REFR", 0, form_id, &subrecords)
}
