use binrw::binrw;

/// 4-byte ASCII record/subrecord type tag, e.g. b"CELL", b"REFR"
pub type Tag = [u8; 4];

pub const TAG_GRUP: Tag = *b"GRUP";
pub const TAG_TES4: Tag = *b"TES4";
pub const TAG_CELL: Tag = *b"CELL";
pub const TAG_REFR: Tag = *b"REFR";
pub const TAG_NAVM: Tag = *b"NAVM";

/// Record flag: data field is zlib-compressed
pub const FLAG_COMPRESSED: u32 = 0x0004_0000;
/// TES4 flag: this file is a master (ESM)
pub const FLAG_MASTER: u32 = 0x0000_0001;

/// 24-byte record header (little-endian)
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct RecordHeader {
    pub tag: Tag,
    pub data_size: u32,
    pub flags: u32,
    pub form_id: u32,
    pub timestamp: u16,
    pub vci: u16,
    pub internal_version: u16,
    pub unknown: u16,
}

impl RecordHeader {
    pub fn new(tag: Tag, data_size: u32, flags: u32, form_id: u32) -> Self {
        Self {
            tag,
            data_size,
            flags,
            form_id,
            timestamp: 0,
            vci: 0,
            internal_version: 0,
            unknown: 0,
        }
    }
}

/// 24-byte group header (little-endian)
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct GrupHeader {
    pub tag: Tag,       // always TAG_GRUP
    pub group_size: u32, // total size including this 24-byte header
    pub label: [u8; 4],
    pub group_type: u32,
    pub timestamp: u16,
    pub vci: u16,
    pub unknown: u32,
}

impl GrupHeader {
    pub fn new(label: [u8; 4], group_type: u32, group_size: u32) -> Self {
        Self {
            tag: TAG_GRUP,
            group_size,
            label,
            group_type,
            timestamp: 0,
            vci: 0,
            unknown: 0,
        }
    }
}

/// 6-byte subrecord header
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct SubrecordHeader {
    pub tag: Tag,
    pub size: u16,
}

/// A raw subrecord: header + opaque bytes
#[derive(Debug, Clone)]
pub struct Subrecord {
    pub tag: Tag,
    pub data: Vec<u8>,
}

impl Subrecord {
    pub fn new(tag: Tag, data: Vec<u8>) -> Self {
        Self { tag, data }
    }

    /// Total serialized size: 6-byte header + data
    pub fn serialized_len(&self) -> u32 {
        6 + self.data.len() as u32
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.tag);
        out.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.data);
    }
}

/// Helpers to build subrecord data payloads
pub fn zstring(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

pub fn formid_bytes(id: u32) -> Vec<u8> {
    id.to_le_bytes().to_vec()
}

pub fn u8_bytes(v: u8) -> Vec<u8> {
    vec![v]
}

pub fn f32_bytes(v: f32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

pub fn u32_bytes(v: u32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// A fully assembled record ready to write: header flags + subrecords serialized
#[derive(Debug, Clone)]
pub struct Record {
    pub tag: Tag,
    pub flags: u32,
    pub form_id: u32,
    pub data: Vec<u8>, // serialized subrecords
}

impl Record {
    pub fn new(tag: Tag, flags: u32, form_id: u32, subrecords: &[Subrecord]) -> Self {
        let mut data = Vec::new();
        for sr in subrecords {
            sr.write_to(&mut data);
        }
        Self { tag, flags, form_id, data }
    }

    pub fn serialized_len(&self) -> u32 {
        24 + self.data.len() as u32
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        let header = RecordHeader::new(self.tag, self.data.len() as u32, self.flags, self.form_id);
        out.extend_from_slice(&header.tag);
        out.extend_from_slice(&header.data_size.to_le_bytes());
        out.extend_from_slice(&header.flags.to_le_bytes());
        out.extend_from_slice(&header.form_id.to_le_bytes());
        out.extend_from_slice(&header.timestamp.to_le_bytes());
        out.extend_from_slice(&header.vci.to_le_bytes());
        out.extend_from_slice(&header.internal_version.to_le_bytes());
        out.extend_from_slice(&header.unknown.to_le_bytes());
        out.extend_from_slice(&self.data);
    }
}

/// A group: header + raw content bytes (records/subgroups already serialized)
#[derive(Debug, Clone)]
pub struct Group {
    pub label: [u8; 4],
    pub group_type: u32,
    pub content: Vec<u8>,
}

impl Group {
    pub fn new(label: [u8; 4], group_type: u32) -> Self {
        Self { label, group_type, content: Vec::new() }
    }

    pub fn push_record(&mut self, rec: &Record) {
        rec.write_to(&mut self.content);
    }

    pub fn push_group(&mut self, grp: &Group) {
        grp.write_to(&mut self.content);
    }

    pub fn serialized_len(&self) -> u32 {
        24 + self.content.len() as u32
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        let size = self.serialized_len();
        out.extend_from_slice(b"GRUP");
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&self.label);
        out.extend_from_slice(&self.group_type.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // timestamp
        out.extend_from_slice(&0u16.to_le_bytes()); // vci
        out.extend_from_slice(&0u32.to_le_bytes()); // unknown
        out.extend_from_slice(&self.content);
    }
}
