//! Canonical fixed claim metadata and inline semantic slot names; no signed-wire encoder.

use super::*;
use std::ffi::OsStr;

/// Advance claims a predecessor revision once; Check claims its original random challenge once.
/// Changing the intended inventory or signature cannot create another name for the same slot.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito(decode_from_slice)]
#[norito_schema(name = "irohad::musubi::PinControlSlotV1")]
pub(super) enum ControlSlotV1 {
    Advance { predecessor_revision: u64 },
    Check { challenge: [u8; 32] },
}

/// Derived only by the future original-owner issuer, never from a caller's supplied hashes.
/// owner_marker_digest uses the separate `iroha:musubi:pin-control-owner:v1` domain over the
/// exact existing pin outbox marker. It must not become part of the pin inventory it controls.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito(decode_from_slice)]
#[norito_schema(name = "irohad::musubi::PinControlBindingV1")]
pub(super) struct ClaimBindingV1 {
    pub(super) network_id: [u8; 32],
    pub(super) owner_marker_digest: [u8; 32],
    pub(super) session_id: [u8; 32],
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito(decode_from_slice)]
#[norito_schema(name = "irohad::musubi::PinControlClaimV1")]
pub(super) struct ClaimDescriptorV1 {
    version: u8,
    pub(super) binding: ClaimBindingV1,
    pub(super) slot: ControlSlotV1,
    pub(super) wire_length: u32,
    wire_digest: [u8; 32],
}

impl ClaimDescriptorV1 {
    pub(super) fn new(
        binding: ClaimBindingV1,
        slot: ControlSlotV1,
        wire: &[u8],
    ) -> Result<Self, ControlJournalErrorV1> {
        if wire.is_empty() || wire.len() > MAX_WIRE_BYTES_V1 {
            return Err(ControlJournalErrorV1::Invalid);
        }
        let descriptor = Self {
            version: 1,
            binding,
            slot,
            wire_length: u32::try_from(wire.len()).map_err(|_| ControlJournalErrorV1::Invalid)?,
            wire_digest: wire_digest(wire),
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub(super) fn validate(&self) -> Result<(), ControlJournalErrorV1> {
        if self.version != 1
            || self.binding.network_id[31] & 1 != 1
            || self.binding.session_id == [0; 32]
            || self.wire_length == 0
            || self.wire_length as usize > MAX_WIRE_BYTES_V1
            || matches!(self.slot, ControlSlotV1::Check { challenge } if challenge == [0; 32])
        {
            return Err(ControlJournalErrorV1::Invalid);
        }
        Ok(())
    }

    pub(super) fn matches_wire(&self, wire: &[u8]) -> bool {
        wire.len() == self.wire_length as usize && wire_digest(wire) == self.wire_digest
    }
}

fn wire_digest(wire: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:pin-control-wire:v1");
    hasher.update(wire);
    *hasher.finalize().as_bytes()
}

pub(super) struct ClaimFrameV1(pub(super) ChargedBuffer<u8>);

impl ClaimFrameV1 {
    pub(super) fn encode(
        descriptor: &ClaimDescriptorV1,
        budget: &AllocationBudget,
    ) -> Result<Self, ControlJournalErrorV1> {
        descriptor.validate()?;
        let length =
            norito::canonical_frame_len(descriptor).map_err(ControlJournalErrorV1::Codec)?;
        if length == 0 || length > MAX_CLAIM_BYTES_V1 {
            return Err(ControlJournalErrorV1::Invalid);
        }
        // The exact requested buffer layout is admitted before allocating its backing.
        // The standalone cold/warm census of these two exact fixed shapes observes only this
        // backing during encoding. It does not fund the derived decoder's observed scratch.
        let mut frame = Self(ChargedBuffer::new(length, budget)?);
        norito::core::write_canonical_to_writer(descriptor, &mut frame)
            .map_err(ControlJournalErrorV1::Codec)?;
        if frame.0.as_slice().len() != length {
            return Err(ControlJournalErrorV1::Invalid);
        }
        Ok(frame)
    }
}

impl io::Write for ClaimFrameV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn decode_claim(bytes: &[u8]) -> Result<ClaimDescriptorV1, ControlJournalErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_CLAIM_BYTES_V1 {
        return Err(ControlJournalErrorV1::Invalid);
    }
    // Only the frame view is borrowed. Despite the fixed fields, the generic derived decoder
    // allocates budget context and alignment/padding scratch. TODO: Resolve actual original-
    // owner backing and typed refusal admission before linking recovery; do not substitute a
    // metric-only reservation or bypass Norito's resource/field/canonical checks.
    let view = norito::core::from_bytes_view(bytes).map_err(ControlJournalErrorV1::Codec)?;
    let descriptor: ClaimDescriptorV1 =
        view.decode_exact().map_err(ControlJournalErrorV1::Codec)?;
    norito::verify_exact_canonical_frame(&descriptor, bytes)
        .map_err(ControlJournalErrorV1::Codec)?;
    descriptor.validate()?;
    Ok(descriptor)
}

/// All name bytes are inline; borrowing them requires no formatted String or retained PathBuf.
pub(super) struct SlotNamesV1 {
    claim: [u8; 80],
    wire: [u8; 80],
    claim_len: usize,
    wire_len: usize,
}

impl SlotNamesV1 {
    pub(super) fn new(slot: ControlSlotV1) -> Result<Self, ControlJournalErrorV1> {
        let mut prefix = [0; 65];
        let prefix_len = match slot {
            ControlSlotV1::Advance {
                predecessor_revision,
            } => {
                prefix[0] = b'a';
                encode_hex(&predecessor_revision.to_be_bytes(), &mut prefix[1..17]);
                17
            }
            ControlSlotV1::Check { challenge } => {
                if challenge == [0; 32] {
                    return Err(ControlJournalErrorV1::Invalid);
                }
                prefix[0] = b'c';
                encode_hex(&challenge, &mut prefix[1..65]);
                65
            }
        };
        let mut names = Self {
            claim: [0; 80],
            wire: [0; 80],
            claim_len: prefix_len + 6,
            wire_len: prefix_len + 5,
        };
        names.claim[..prefix_len].copy_from_slice(&prefix[..prefix_len]);
        names.wire[..prefix_len].copy_from_slice(&prefix[..prefix_len]);
        names.claim[prefix_len..names.claim_len].copy_from_slice(b".claim");
        names.wire[prefix_len..names.wire_len].copy_from_slice(b".wire");
        Ok(names)
    }

    pub(super) fn claim(&self) -> &OsStr {
        OsStr::new(
            std::str::from_utf8(&self.claim[..self.claim_len]).expect("inline ASCII slot name"),
        )
    }

    pub(super) fn wire(&self) -> &OsStr {
        OsStr::new(
            std::str::from_utf8(&self.wire[..self.wire_len]).expect("inline ASCII slot name"),
        )
    }
}

fn encode_hex(bytes: &[u8], destination: &mut [u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (byte, pair) in bytes.iter().zip(destination.chunks_exact_mut(2)) {
        pair[0] = HEX[(byte >> 4) as usize];
        pair[1] = HEX[(byte & 15) as usize];
    }
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
