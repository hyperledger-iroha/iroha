//! Borrowed validation of the exact V1 argument-record wire, without schema authorization.
//!
//! The authoritative type has a raw 32-byte schema-hash field and one flat atom
//! sequence. This scanner does not resolve the hash, validate pointer-ABI payloads,
//! or interpret active branches/list contents without the actual callable schema.

use super::{ActionFrameScanFailureV1, Budget, Cursor, SCANNER_FLAGS, ScanError, ncore};
use crate::smart_contract::entrypoint::{
    EntrypointArgumentRecordV1, MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
};

pub(super) fn scan(frame: &[u8], budget: &mut Budget) -> Result<(), ActionFrameScanFailureV1> {
    scan_from_parent(frame, None, budget)
}

pub(super) fn scan_nested<'frame>(
    frame: &'frame [u8],
    enclosing: Cursor<'frame>,
    budget: &mut Budget,
) -> Result<(), ActionFrameScanFailureV1> {
    scan_from_parent(frame, Some(enclosing), budget)
}

fn scan_from_parent<'frame>(
    frame: &'frame [u8],
    enclosing: Option<Cursor<'frame>>,
    budget: &mut Budget,
) -> Result<(), ActionFrameScanFailureV1> {
    if frame.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(ScanError::Limit.into());
    }
    budget.begin_frame(frame.len())?;
    if ncore::default_encode_flags() != SCANNER_FLAGS {
        return Err(ActionFrameScanFailureV1::Structure(
            "canonical argument-record codec flags changed",
        ));
    }
    // Header/schema/padding/CRC validation borrows the original payload. The
    // complete traversal was charged by begin_frame before entering Norito.
    let payload =
        norito::canonical_frame_payload::<EntrypointArgumentRecordV1>(frame, SCANNER_FLAGS)?;
    let mut record = match enclosing {
        Some(parent) => parent.nested_root(payload, budget)?,
        None => Cursor::root(payload, budget)?,
    };
    let mut schema_hash = record.field(budget)?;
    // Derive special-cases a struct [u8; N] field as N raw bytes under one
    // prefix. It does not use the stand-alone array element-framing codec.
    for _ in 0..4 {
        schema_hash.u64(budget)?;
    }
    schema_hash.finish()?;
    let mut atoms = record.field(budget)?;
    let count = atoms.u64(budget)?;
    budget.elements(count)?;
    for _ in 0..count {
        scan_atom(atoms.field(budget)?, budget)?;
    }
    atoms.finish()?;
    record.finish()?;
    Ok(())
}

fn scan_atom(mut atom: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    match atom.u32(budget)? {
        0 | 1 => {
            let mut value = atom.field(budget)?;
            if value.byte(budget)? > 1 {
                return Err(ScanError::InvalidValue);
            }
            value.finish()?;
        }
        2 => {
            let mut pointer_bytes = atom.field(budget)?;
            // The wire type is Vec<u8>; its content is opaque at this layer.
            // A zero-length or semantically invalid TLV is still canonical wire.
            pointer_bytes.byte_vector(budget)?;
            pointer_bytes.finish()?;
        }
        3 => {
            let mut count = atom.field(budget)?;
            // All u8 values are canonical here. Actual schema list capacity and
            // following active item atoms are checked at execution preparation.
            count.byte(budget)?;
            count.finish()?;
        }
        4 => {}
        5 | 6 => {
            let mut code = atom.field(budget)?;
            code.u32(budget)?;
            code.finish()?;
        }
        _ => return Err(ScanError::InvalidValue),
    }
    atom.finish()
}
