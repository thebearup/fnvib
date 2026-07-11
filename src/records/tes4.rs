use super::types::*;

/// Builds the TES4 file-header record.
///
/// `masters` — list of .esm/.esp filenames this plugin depends on (e.g. ["FalloutNV.esm"])
/// `next_object_id` — first FormID available for new records in this plugin
pub fn build_tes4(author: &str, description: &str, masters: &[&str], next_object_id: u32) -> Record {
    let num_records: u32 = 0; // filled in accurately later if needed; 0 is accepted by the engine
    let mut subrecords = Vec::new();

    // HEDR: version=1.32, numRecords, nextObjectId
    let mut hedr = Vec::new();
    hedr.extend_from_slice(&1.32f32.to_le_bytes());
    hedr.extend_from_slice(&num_records.to_le_bytes());
    hedr.extend_from_slice(&next_object_id.to_le_bytes());
    subrecords.push(Subrecord::new(*b"HEDR", hedr));

    subrecords.push(Subrecord::new(*b"CNAM", zstring(author)));
    subrecords.push(Subrecord::new(*b"SNAM", zstring(description)));

    for master in masters {
        subrecords.push(Subrecord::new(*b"MAST", zstring(master)));
        // DATA is vestigial file-size field, always 0
        subrecords.push(Subrecord::new(*b"DATA", 0u64.to_le_bytes().to_vec()));
    }

    // 0 flags = plain ESP. Callers that want an ESM can OR in FLAG_MASTER themselves.
    Record::new(*b"TES4", 0, 0, &subrecords)
}
