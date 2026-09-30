//! One canonical bounded encoder with explicit ownership of its final bytes.
//!
//! Paired staging chooses a charged fixed owner. The standalone full-value
//! tree still chooses its existing Vec; funding that API remains separate.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};

pub(super) fn buffer_error(error: ChargedBufferError) -> LeafError {
    match error {
        ChargedBufferError::Admission(error) => LeafError::Admission(error),
        ChargedBufferError::Allocator { .. } => LeafError::Allocation,
    }
}

pub(super) fn vector(length: usize) -> Result<Vec<u8>, LeafError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| LeafError::Allocation)?;
    Ok(bytes)
}

/// Fixed canonical bytes retaining the original pool through their actual owner.
pub(super) struct FundedFrame(ChargedBuffer<u8>);

impl FundedFrame {
    pub(super) fn new(length: usize, budget: &AllocationBudget) -> Result<Self, LeafError> {
        ChargedBuffer::new(length, budget)
            .map(Self)
            .map_err(buffer_error)
    }

    pub(super) fn into_buffer(self) -> ChargedBuffer<u8> {
        self.0
    }
}

impl AsRef<[u8]> for FundedFrame {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl Write for FundedFrame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bare_payload_with_bound<T: Encode, B: Write + AsRef<[u8]>>(
    value: &T,
    max_payload_bytes: usize,
    allocate: impl FnOnce(usize) -> Result<B, LeafError>,
) -> Result<B, LeafError> {
    let mut length = 0_usize;
    let mut exceeded = false;
    let first_pass = Hash::new_from_writer(|writer| {
        let mut bounded = BoundedWriter {
            inner: writer,
            remaining: max_payload_bytes,
            exceeded: &mut exceeded,
        };
        length =
            norito::codec::encode_adaptive_into(value, &mut bounded).map_err(io::Error::other)?;
        Ok(())
    });
    if exceeded || length > max_payload_bytes {
        return Err(LeafError::PayloadLimit);
    }
    let first_pass = first_pass.map_err(|error| LeafError::Encoding(error.to_string()))?;
    let mut bytes = allocate(length)?;
    let mut exceeded = false;
    let written = {
        let mut writer = BoundedWriter {
            inner: &mut bytes,
            remaining: length,
            exceeded: &mut exceeded,
        };
        norito::codec::encode_adaptive_into(value, &mut writer)
            .map_err(|error| LeafError::Encoding(error.to_string()))?
    };
    if exceeded
        || written != length
        || bytes.as_ref().len() != length
        || Hash::new(bytes.as_ref()) != first_pass
    {
        return Err(LeafError::Encoding(
            "canonical table payload changed between bounded passes".to_owned(),
        ));
    }
    Ok(bytes)
}

pub(super) fn typed_bare_payload<T: Encode + NoritoSchema, B: Write + AsRef<[u8]>>(
    table: &'static str,
    schema: Schema,
    value: &T,
    max_payload_bytes: usize,
    allocate: impl FnOnce(usize) -> Result<B, LeafError>,
) -> Result<B, LeafError> {
    let Schema::Norito {
        nominal_name,
        layout,
    } = schema
    else {
        return Err(LeafError::UnresolvedSchema(table));
    };
    if layout != V1_LAYOUT {
        return Err(LeafError::NonV1Layout(table));
    }
    if nominal_name() != T::nominal_name() {
        return Err(LeafError::TypeMismatch(table));
    }
    bare_payload_with_bound(value, max_payload_bytes, allocate)
}

#[cfg(test)]
mod tests;
