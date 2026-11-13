//! Canonical bounded header extraction from an independently authenticated proposal prefix.

use super::{BlockHeader, BlockSignatures, SignedBlock};
use norito::core::{
    Compression, DecodeAttemptError, DecodeFlagsGuard, Error, Header, classify_decode_attempt,
    decode_field_canonical, default_encode_flags, inspect_len_from_slice,
};

/// Maximum prefix retained to extract a proposal header, independent of full block size.
pub const MAX_PROPOSAL_PREFIX_BYTES: usize = 128 * 1024;

/// Canonically decoded prefix metadata; neither full-body validity nor finality authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedProposalPrefix {
    header: BlockHeader,
    consumed_len: usize,
}
impl ParsedProposalPrefix {
    /// Original header decoded after the bounded canonical signature collection.
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }
    /// Prefix bytes through the end of the complete original header field.
    pub const fn consumed_len(&self) -> usize {
        self.consumed_len
    }
}

/// Read the header from the canonical `SignedBlock` proposal wire prefix.
///
/// The caller authenticates these exact prefix bytes and total wire length independently.
/// Only signatures and the header are decoded. This does not validate the body, checksum,
/// resultless shape, transaction commitments, signatures, or consensus finality, and cannot
/// construct a block or execution proposal. `None` means more prefix bytes are required.
///
/// # Errors
/// Rejects invalid version, archive metadata, noncanonical fields, impossible extents, more
/// than 31 signatures, and prefixes exceeding the fixed bound. Decoder resource refusals
/// retain their original enclosing-scope identity; they never become incomplete input.
pub fn parse_proposal_prefix(
    bytes: &[u8],
    total_wire_len: usize,
) -> Result<Option<ParsedProposalPrefix>, DecodeAttemptError> {
    classify_decode_attempt(|| parse(bytes, total_wire_len))
}

fn parse(bytes: &[u8], total: usize) -> Result<Option<ParsedProposalPrefix>, Error> {
    const START: usize = 1 + Header::SIZE;
    if total <= START || bytes.len() > total || bytes.len() > MAX_PROPOSAL_PREFIX_BYTES {
        return Err(Error::LengthMismatch);
    }
    let Some(&version) = bytes.first() else {
        return Ok(None);
    };
    if version != SignedBlock::WIRE_VERSION {
        return Err(Error::UnsupportedVersion {
            found: version,
            expected: SignedBlock::WIRE_VERSION,
        });
    }
    let Some(frame) = bytes.get(1..START) else {
        return Ok(None);
    };
    let archive = Header::read(frame)?;
    if archive.schema != norito::schema::identity::frame_hash::<SignedBlock>() {
        return Err(Error::SchemaMismatch);
    }
    if archive.compression != Compression::None {
        return Err(Error::UnsupportedCompression {
            found: archive.compression as u8,
            supported: &[Compression::None],
        });
    }
    if archive.flags != default_encode_flags() {
        return Err(Error::UnsupportedFeature(
            "non-canonical signed block wire layout",
        ));
    }
    let declared = usize::try_from(archive.length).map_err(|_| Error::LengthMismatch)?;
    if declared != total - START {
        return Err(Error::LengthMismatch);
    }
    super::enforce_payload_len_limit(declared)?;
    let _flags = DecodeFlagsGuard::enter(archive.flags);
    let Some(signatures) = extent(bytes, START, total)? else {
        return Ok(None);
    };
    bounded_prefix(signatures.end)?;
    let Some(signature_bytes) = bytes.get(signatures.clone()) else {
        return Ok(None);
    };
    let (_, consumed) = decode_field_canonical::<BlockSignatures>(signature_bytes)?;
    if consumed != signature_bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let Some(payload) = extent(bytes, signatures.end, total)? else {
        return Ok(None);
    };
    // Both trailing optional fields need at least a length prefix and an option tag.
    if payload.end.checked_add(4).is_none_or(|end| end > total) {
        return Err(Error::LengthMismatch);
    }
    let Some(header) = extent(bytes, payload.start, payload.end)? else {
        return Ok(None);
    };
    bounded_prefix(header.end)?;
    let Some(header_bytes) = bytes.get(header.clone()) else {
        return Ok(None);
    };
    let (decoded, consumed) = decode_field_canonical::<BlockHeader>(header_bytes)?;
    if consumed != header_bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(Some(ParsedProposalPrefix {
        header: decoded,
        consumed_len: header.end,
    }))
}

fn bounded_prefix(end: usize) -> Result<(), Error> {
    if end > MAX_PROPOSAL_PREFIX_BYTES {
        return Err(Error::LengthMismatch);
    }
    Ok(())
}

/// Inspect a canonical compact field prefix while bounding the declared child by its parent.
fn extent(
    bytes: &[u8],
    offset: usize,
    parent_end: usize,
) -> Result<Option<std::ops::Range<usize>>, Error> {
    if offset >= parent_end {
        return Err(Error::LengthMismatch);
    }
    bounded_prefix(offset.checked_add(1).ok_or(Error::LengthMismatch)?)?;
    let available_end = bytes.len().min(parent_end);
    let available = bytes.get(offset..available_end).unwrap_or_default();
    // A missing compact length is the only condition inferred before the canonical parser.
    // Ten continuation bytes or exhausting the parent is already invalid, never incomplete.
    if !available.iter().take(10).any(|byte| byte & 0x80 == 0) {
        if available.len() < 10 && available_end < parent_end {
            if bytes.len() == MAX_PROPOSAL_PREFIX_BYTES {
                return Err(Error::LengthMismatch);
            }
            return Ok(None);
        }
        return Err(Error::LengthMismatch);
    }
    let (length, used) = inspect_len_from_slice(available)?;
    let start = offset.checked_add(used).ok_or(Error::LengthMismatch)?;
    let end = start.checked_add(length).ok_or(Error::LengthMismatch)?;
    if end > parent_end {
        return Err(Error::LengthMismatch);
    }
    Ok(Some(start..end))
}

#[cfg(test)]
mod tests;
