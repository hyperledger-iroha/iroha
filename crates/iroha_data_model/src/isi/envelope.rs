//! Opaque instruction envelopes (`specs/live_runtime_upgrades.md` §5.2).
//!
//! An instruction travels in a stable envelope: its wire ID and its framed payload (Norito header
//! and payload). A release decodes the frame of a wire ID it registers into the typed instruction
//! and keeps the envelope of every other well-formed wire ID opaque. An opaque envelope re-encodes
//! as exactly the `(wire_id, frame)` pair it was decoded from, so transaction hashes, signatures and
//! block bytes never depend on which instructions a release knows. Execution never applies one:
//! while no active interface answers its wire ID it is refused with `definition_inactive`, and an
//! envelope whose interface is active but whose frame this release kept opaque is refused with
//! `malformed_payload`.

use std::{any::Any, fmt};

use super::{Instruction, InstructionBox};

/// Largest byte length of an instruction wire ID.
///
/// It equals the text limit of an interface definition
/// ([`crate::runtime::definition::MAX_DEFINITION_TEXT_BYTES`]), so every wire ID an interface can
/// declare fits and no envelope names a wire ID that no interface could ever answer.
pub const MAX_INSTRUCTION_WIRE_ID_BYTES: usize =
    crate::runtime::definition::MAX_DEFINITION_TEXT_BYTES;

/// Whether `wire_id` is a well-formed instruction wire ID.
///
/// The rule is the one an interface definition applies to the wire ID it declares: 1 to
/// [`MAX_INSTRUCTION_WIRE_ID_BYTES`] bytes of ASCII letters, digits and `. _ : - @ /`. It belongs to
/// the frozen envelope framing: a pair whose wire ID breaks it is a structural decode error on
/// every release, while every well-formed wire ID decodes, as an [`OpaqueInstructionV1`] when the
/// release does not register it.
#[must_use]
pub fn is_well_formed_wire_id(wire_id: &str) -> bool {
    !wire_id.is_empty()
        && wire_id.len() <= MAX_INSTRUCTION_WIRE_ID_BYTES
        && wire_id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-' | b'@' | b'/')
        })
}

/// The structural error of an envelope whose wire ID is not well formed.
pub(super) fn malformed_wire_id_error() -> norito::Error {
    norito::Error::Message("malformed instruction wire identifier".to_owned())
}

/// An instruction envelope this release does not interpret (§5.2).
///
/// It is what decoding yields for a well-formed wire ID this release does not register, and for a
/// frame that a tolerant registration (an instruction added after the frozen baseline) does not
/// accept as the canonical frame of a payload it decodes: earlier releases keep both opaque, so
/// this release must too. It holds the wire ID and the exact frame, never interprets the frame,
/// and encodes as the same `(wire_id, frame)` pair byte for byte.
///
/// Execution refuses it: with `definition_inactive` while no active interface answers its wire ID,
/// and with `malformed_payload` once one does.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueInstructionV1 {
    wire_id: String,
    frame: Vec<u8>,
}

impl OpaqueInstructionV1 {
    /// Construct the opaque envelope of `wire_id` carrying `frame`.
    ///
    /// The frame is kept as given: the Norito header and payload of the envelope. Decoding yields
    /// an opaque envelope only for a wire ID the release does not register strictly, so an
    /// envelope built for a strictly registered wire ID encodes bytes that decode as that
    /// instruction or not at all.
    ///
    /// # Errors
    /// The wire ID is not well formed ([`is_well_formed_wire_id`]).
    pub fn new(
        wire_id: impl Into<String>,
        frame: impl Into<Vec<u8>>,
    ) -> Result<Self, norito::Error> {
        let wire_id = wire_id.into();
        if !is_well_formed_wire_id(&wire_id) {
            return Err(malformed_wire_id_error());
        }
        Ok(Self {
            wire_id,
            frame: frame.into(),
        })
    }

    /// Copy one decoded envelope, charging both copies to the active decode allocation budget.
    ///
    /// # Errors
    /// The wire ID is not well formed, or the decode budget refuses the copies.
    pub(super) fn decode_owned(wire_id: &str, frame: &[u8]) -> Result<Self, norito::Error> {
        if !is_well_formed_wire_id(wire_id) {
            return Err(malformed_wire_id_error());
        }
        norito::core::reserve_decode_allocation(wire_id.len())?;
        norito::core::reserve_decode_allocation(frame.len())?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(frame.len())
            .map_err(|_| norito::Error::AllocationFailed {
                bytes: u64::try_from(frame.len()).unwrap_or(u64::MAX),
            })?;
        owned.extend_from_slice(frame);
        Ok(Self {
            wire_id: wire_id.to_owned(),
            frame: owned,
        })
    }

    /// The wire ID the envelope carries.
    #[must_use]
    pub fn wire_id(&self) -> &str {
        &self.wire_id
    }

    /// The exact frame the envelope carries: the Norito header and payload.
    #[must_use]
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    /// Take the wire ID and the frame.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.wire_id, self.frame)
    }

    /// The canonical bare `(wire_id, frame)` pair: the bytes an [`InstructionBox`] holding this
    /// envelope encodes as under the canonical layout.
    fn canonical_pair(&self) -> Vec<u8> {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let mut out = Vec::new();
        // Writing length prefixes into a vector fails only for lengths beyond `u64`, which no
        // in-memory envelope has.
        super::write_instruction_pair_prefix(&mut out, &self.wire_id, self.frame.len())
            .expect("in-memory envelope lengths fit the Norito length prefix");
        out.extend_from_slice(&self.frame);
        out
    }

    /// Length of [`Self::canonical_pair`].
    fn canonical_pair_len(&self) -> Option<usize> {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        super::encoded_instruction_tuple_len(&self.wire_id, self.frame.len())
    }
}

impl fmt::Debug for OpaqueInstructionV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueInstructionV1")
            .field("wire_id", &self.wire_id)
            .field("frame_len", &self.frame.len())
            .finish()
    }
}

impl crate::seal::Instruction for OpaqueInstructionV1 {}

impl Instruction for OpaqueInstructionV1 {
    /// The canonical bare `(wire_id, frame)` pair: two envelopes are equal exactly when their
    /// wire IDs and frames are.
    fn dyn_encode(&self) -> Vec<u8> {
        self.canonical_pair()
    }

    fn dyn_encoded_len(&self) -> Option<usize> {
        self.canonical_pair_len()
    }

    fn dyn_write_frame(&self, writer: &mut dyn std::io::Write) -> Result<(), norito::core::Error> {
        writer.write_all(&self.frame)?;
        Ok(())
    }

    /// Write the envelope's own `(wire_id, frame)` pair; `wire_id` must be the envelope's.
    fn dyn_write_pair(
        &self,
        writer: &mut norito::core::Encoder<'_>,
        wire_id: &str,
    ) -> Result<(), norito::core::Error> {
        if wire_id != self.wire_id {
            return Err(norito::core::Error::InvalidValue {
                context: "opaque instruction envelope written under another wire identifier",
            });
        }
        super::write_instruction_pair_prefix(&mut *writer, &self.wire_id, self.frame.len())?;
        writer.write_all(&self.frame)?;
        Ok(())
    }

    fn dyn_frame_len(&self) -> Result<usize, norito::core::Error> {
        Ok(self.frame.len())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl From<OpaqueInstructionV1> for InstructionBox {
    fn from(instruction: OpaqueInstructionV1) -> Self {
        InstructionBox::from_boxed(Box::new(instruction))
    }
}
