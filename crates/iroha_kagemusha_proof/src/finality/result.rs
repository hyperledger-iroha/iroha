//! Byte-level components for the canonical native execution-result preimage.
//!
//! The result digest is marked BLAKE2b-256 of `iroha/sumeragi/result/v1`
//! followed by the complete canonical Norito frame. Canonical struct fields
//! use compact length prefixes; lengths and offsets must be decoded from the
//! same original bytes consumed by that hash, never supplied as decoded host
//! facts. These components do not establish native finality or epoch authority.
//!
//! The fixture-pinned prefix extractor is available only through a stream that
//! hashes its source bytes. It does not check the frame checksum or the complete
//! result grammar: use requires an honest native quorum authenticating canonical
//! execution results. The complete current/boundary schedule still needs parsing
//! and authentication from genesis before these components can grant authority.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    GlueChip, Uint, UintChip, Word,
    blake2b::{Blake2bChip, Blake2bDigest, Blake2bState},
};

/// Largest native result frame, including header and any alignment padding.
pub const MAX_RESULT_BYTES: u32 = 65_536;

/// Exact native result-hash domain, with no trailing NUL.
pub const RESULT_TAG: &[u8] = b"iroha/sumeragi/result/v1";

/// Native result schema identity, pinned by the production codec fixture.
pub const RESULT_CODEC_ID: [u8; 16] = [
    0x01, 0x65, 0xbe, 0x47, 0xe2, 0xcd, 0x7f, 0x3b, 0x8d, 0xc6, 0xd6, 0x0c, 0xc8, 0xe9, 0x65, 0x8a,
];

/// Result fields extracted from the exact original hashed byte stream.
///
/// A value is released only after the complete declared result has been hashed.
/// This is an extraction under the honest native quorum/canonical-result
/// assumption, not a complete result decoder or evidence of consensus finality.
#[derive(Clone, Debug)]
pub struct HashedResultPrefix {
    digest: Blake2bDigest<Fp>,
    height: Uint<Fp, 64>,
    event_root: [Word<Fp>; 32],
    event_count: Uint<Fp, 64>,
}

impl HashedResultPrefix {
    /// Marked digest of the full original canonical result frame and domain.
    #[must_use]
    pub const fn digest(&self) -> &Blake2bDigest<Fp> {
        &self.digest
    }
    /// Block height encoded in the original result.
    #[must_use]
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Counted event-tree root encoded in the original execution commitment.
    #[must_use]
    pub const fn event_root(&self) -> &[Word<Fp>; 32] {
        &self.event_root
    }
    /// Nonzero event-tree leaf count encoded beside the root.
    #[must_use]
    pub const fn event_count(&self) -> &Uint<Fp, 64> {
        &self.event_count
    }
}

#[derive(Clone, Debug)]
struct ResultPrefix {
    height: Uint<Fp, 64>,
    event_root: [Word<Fp>; 32],
    event_count: Uint<Fp, 64>,
}

/// A result stream that retains a prefix extracted from its first three blocks.
///
/// There is no independent prefix constructor: the parser sees the same cells
/// consumed by the hash, at fixed offsets verified against the native codec.
/// The first three blocks cover the domain, header and event commitment; further
/// blocks remain mandatory until the complete declared original frame is read.
#[derive(Clone, Debug)]
pub struct ResultPrefixStream {
    stream: ResultHashStream,
    prefix: ResultPrefix,
}

impl ResultPrefixStream {
    /// Parse and hash the first three blocks of the original result transcript.
    ///
    /// The result type, canonical compact-length flag, uncompressed encoding,
    /// payload size, height and event `Some` framing are constrained. The codec
    /// has alignment 8 and a 40-byte header, so no alignment bytes precede the
    /// first field. Offsets never depend on a host decoder or witness branch.
    ///
    /// # Errors
    /// Layout errors; a short frame or incorrect framing is unsatisfied.
    pub fn start(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        frame_len: &Uint<Fp, 32>,
        blocks: &[[Word<Fp>; 128]; 3],
    ) -> Result<Self, Error> {
        let minimum = uint.constant::<32>(region, (384 - RESULT_TAG.len()) as u128)?;
        uint.assert_le(region, &minimum, frame_len)?;
        let bytes: Vec<_> = blocks.iter().flatten().cloned().collect();
        let prefix = ResultPrefix::parse(uint, region, frame_len, &bytes[RESULT_TAG.len()..])?;
        let mut stream = ResultHashStream::start(uint, blake, region, frame_len)?;
        for block in blocks {
            stream = stream.absorb(uint, blake, region, block)?;
        }
        Ok(Self { stream, prefix })
    }

    /// Hash the next fixed block while retaining the original constrained prefix.
    ///
    /// # Errors
    /// Layout errors; overrun or nonzero final padding is unsatisfied.
    pub fn absorb(
        &self,
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        block: &[Word<Fp>; 128],
    ) -> Result<Self, Error> {
        Ok(Self {
            stream: self.stream.absorb(uint, blake, region, block)?,
            prefix: self.prefix.clone(),
        })
    }

    /// Release the prefix together with the full original result digest.
    ///
    /// # Errors
    /// Layout errors; incomplete consumption is unsatisfied.
    pub fn finish(
        &self,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<HashedResultPrefix, Error> {
        Ok(HashedResultPrefix {
            digest: self.stream.finish(blake, region)?,
            height: self.prefix.height.clone(),
            event_root: self.prefix.event_root.clone(),
            event_count: self.prefix.event_count.clone(),
        })
    }
}

fn little_endian_u64(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    bytes: &[Word<Fp>],
) -> Result<Uint<Fp, 64>, Error> {
    if bytes.len() != 8 {
        return Err(Error::Synthesis);
    }
    for byte in bytes {
        uint.range_check::<8>(region, byte)?;
    }
    let mut word = uint.glue().constant(region, Fp::ZERO)?;
    for byte in bytes.iter().rev() {
        word = uint
            .glue()
            .linear(region, &[(Fp::from(256), &word), (Fp::ONE, byte)], Fp::ZERO)?;
    }
    uint.range_check::<64>(region, &word)
}

impl ResultPrefix {
    // Private: callers can only receive parsed fields with the full-stream hash.
    fn parse(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        frame_len: &Uint<Fp, 32>,
        bytes: &[Word<Fp>],
    ) -> Result<Self, Error> {
        if bytes.len() < 303 {
            return Err(Error::Synthesis);
        }
        for (i, expected) in b"NRT0\0\0".iter().chain(RESULT_CODEC_ID.iter()).enumerate() {
            GlueChip::assert_constant(region, &bytes[i], Fp::from(u64::from(*expected)))?;
        }
        GlueChip::assert_constant(region, &bytes[22], Fp::ZERO)?; // No compression.
        GlueChip::assert_constant(region, &bytes[39], Fp::from(2))?; // COMPACT_LEN only.
        let payload_len = little_endian_u64(uint, region, &bytes[23..31])?;
        let payload_len = uint.range_check::<32>(region, payload_len.word())?;
        let original_len = uint.checked_add_constant(region, &payload_len, 40)?;
        GlueChip::assert_equal(region, original_len.word(), frame_len.word())?;
        // Header CRC bytes 31..39 are included in the authenticated hash. Native
        // canonical validity is supplied by the honest quorum, not re-proven here.
        GlueChip::assert_constant(region, &bytes[40], Fp::from(8))?;
        let height = little_endian_u64(uint, region, &bytes[41..49])?;
        uint.assert_nonzero(region, &height)?;
        let execution_len = CompactResultLength::from_window(
            uint,
            region,
            &[bytes[49].clone(), bytes[50].clone(), bytes[51].clone()],
        )?;
        GlueChip::assert_constant(region, execution_len.encoded_bytes().word(), Fp::from(2))?;
        // With an event Some, the two later optional Merkle commitments yield
        // exactly these three native execution payload sizes.
        let mut allowed = uint.glue().constant(region, Fp::ONE)?;
        for size in [256, 299, 342] {
            let difference =
                uint.glue()
                    .add_constant(region, execution_len.value().word(), -Fp::from(size))?;
            allowed = uint.glue().mul(region, &allowed, &difference)?;
        }
        GlueChip::assert_constant(region, &allowed, Fp::ZERO)?;
        let execution_start = uint.constant::<32>(region, 49)?;
        execution_len.end_from(uint, region, &execution_start, frame_len)?;
        for i in 0..5 {
            GlueChip::assert_constant(region, &bytes[51 + 33 * i], Fp::from(32))?;
        }
        for (offset, expected) in [(216, 44), (217, 1), (218, 42), (219, 32), (252, 8)] {
            GlueChip::assert_constant(region, &bytes[offset], Fp::from(expected))?;
        }
        let event_root: [Word<Fp>; 32] = bytes[220..252]
            .to_vec()
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        for byte in &event_root {
            uint.range_check::<8>(region, byte)?;
        }
        let last = uint.range_check::<8>(region, &event_root[31])?;
        let high = uint.assign::<7>(region, last.value().map(|value| value >> 1))?;
        let marked = uint
            .glue()
            .linear(region, &[(Fp::from(2), high.word())], Fp::ONE)?;
        GlueChip::assert_equal(region, &marked, last.word())?;
        let event_count = little_endian_u64(uint, region, &bytes[253..261])?;
        uint.assert_nonzero(region, &event_count)?;
        Ok(Self {
            height,
            event_root,
            event_count,
        })
    }
}

/// Constrained streaming hash of the original result frame and its domain.
///
/// A parser must read the same block cells passed to [`Self::absorb`] and bind
/// its header payload length to `frame_len`. This type proves only the hash
/// schedule; it does not validate a Norito result, a quorum or epoch authority.
/// Recursive owners must bind all state words, processed length and total
/// length at each step; they cannot replace the stream by a claimed digest.
#[derive(Clone, Debug)]
pub struct ResultHashStream {
    state: Blake2bState<Fp>,
    processed: Uint<Fp, 32>,
    total: Uint<Fp, 32>,
    frame_len: Uint<Fp, 32>,
}

impl ResultHashStream {
    /// Start from the exact BLAKE2b-256 IV with a bounded original frame length.
    /// The first block will constrain the native domain bytes itself.
    ///
    /// # Errors
    /// Layout errors; a frame above the native 64 KiB ceiling is unsatisfied.
    pub fn start(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        frame_len: &Uint<Fp, 32>,
    ) -> Result<Self, Error> {
        let maximum = uint.constant::<32>(region, u128::from(MAX_RESULT_BYTES))?;
        uint.assert_le(region, frame_len, &maximum)?;
        let total = uint.checked_add_constant(region, frame_len, RESULT_TAG.len() as u128)?;
        Ok(Self {
            state: blake.initial_state(region)?,
            processed: uint.constant(region, 0)?,
            total,
            frame_len: frame_len.clone(),
        })
    }

    /// Consume one fixed 128-byte block of `RESULT_TAG || original_frame`.
    ///
    /// Remaining length determines the final flag and active byte count in
    /// circuit. The final suffix must be zero, an exact full final block does
    /// not append an empty block, and calls after completion are unsatisfied.
    /// The compression counter is the new exact total byte count, high limb 0.
    ///
    /// # Errors
    /// Layout errors; changed domain/padding or an overrun is unsatisfied.
    pub fn absorb(
        &self,
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        block: &[Word<Fp>; 128],
    ) -> Result<Self, Error> {
        uint.assert_lt(region, &self.processed, &self.total)?;
        let remaining = uint.checked_sub(region, &self.total, &self.processed)?;
        let full = uint.constant::<32>(region, 128)?;
        let more = uint.lt(region, &full, &remaining)?;
        let last = uint.glue().not(region, &more)?;
        let count = uint
            .glue()
            .select(region, &last, remaining.word(), full.word())?;
        let count = uint.range_check::<32>(region, &count)?;
        let first = uint.glue().is_zero(region, self.processed.word())?;
        for (i, byte) in block.iter().enumerate() {
            let index = uint.constant::<32>(region, i as u128)?;
            let active = uint.lt(region, &index, &count)?;
            let inactive = uint.glue().not(region, &active)?;
            let padding = uint.glue().mul(region, inactive.word(), byte)?;
            GlueChip::assert_constant(region, &padding, Fp::ZERO)?;
            if let Some(expected) = RESULT_TAG.get(i) {
                let difference =
                    uint.glue()
                        .add_constant(region, byte, -Fp::from(u64::from(*expected)))?;
                let mismatch = uint.glue().mul(region, first.word(), &difference)?;
                GlueChip::assert_constant(region, &mismatch, Fp::ZERO)?;
            }
        }
        let processed = uint.checked_add(region, &self.processed, &count)?;
        let zero = uint.glue().constant(region, Fp::ZERO)?;
        let state = blake.compress_block(
            region,
            &self.state,
            block,
            &[processed.word().clone(), zero],
            &last,
        )?;
        Ok(Self {
            state,
            processed,
            total: self.total.clone(),
            frame_len: self.frame_len.clone(),
        })
    }

    /// Return the native marked result digest only after every declared byte
    /// has been consumed. An incomplete stream cannot produce this output.
    ///
    /// # Errors
    /// Layout errors; incomplete consumption is unsatisfied.
    pub fn finish(
        &self,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Blake2bDigest<Fp>, Error> {
        GlueChip::assert_equal(region, self.processed.word(), self.total.word())?;
        blake.digest_marked(region, &self.state)
    }

    /// Full chaining state to export through the `BLAKE2b` chip for recursion.
    #[must_use]
    pub const fn state(&self) -> &Blake2bState<Fp> {
        &self.state
    }
    /// Exact number of hashed domain-plus-frame bytes so far.
    #[must_use]
    pub const fn processed(&self) -> &Uint<Fp, 32> {
        &self.processed
    }
    /// Exact original frame byte length, to bind to its decoded header.
    #[must_use]
    pub const fn frame_len(&self) -> &Uint<Fp, 32> {
        &self.frame_len
    }
}

/// One minimally encoded compact field length below the result-frame ceiling.
///
/// The three-byte lookahead may include bytes of the following payload when
/// the prefix is shorter. Only the active prefix contributes to the value.
/// A decoder must bind this lookahead to its original stream at the current
/// cursor; this length parser grants no authenticity to independently assigned
/// byte cells or to a host-selected offset.
#[derive(Clone, Debug)]
pub struct CompactResultLength {
    value: Uint<Fp, 32>,
    encoded_bytes: Uint<Fp, 8>,
}

impl CompactResultLength {
    /// Decode canonical unsigned LEB128 in one fixed three-byte window.
    ///
    /// Every byte, payload limb and continuation flag is constrained. An
    /// overlong encoding, unterminated prefix or value above 65,536 fails the
    /// constraints; witness contents never select the circuit's layout.
    ///
    /// # Errors
    /// Range/glue layout errors. Malformed encodings yield an unsatisfied circuit.
    pub fn from_window(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        bytes: &[Word<Fp>; 3],
    ) -> Result<Self, Error> {
        let mut payload = Vec::with_capacity(3);
        let mut continuation = Vec::with_capacity(3);
        for byte in bytes {
            let byte = uint.range_check::<8>(region, byte)?;
            let low = uint.assign::<7>(region, byte.value().map(|byte| byte & 127))?;
            let high = uint
                .glue()
                .boolean(region, byte.value().map(|byte| byte >= 128))?;
            let joined = uint.glue().linear(
                region,
                &[(Fp::ONE, low.word()), (Fp::from(128), high.word())],
                Fp::ZERO,
            )?;
            GlueChip::assert_equal(region, &joined, byte.word())?;
            payload.push(low);
            continuation.push(high);
        }
        let third = uint
            .glue()
            .and(region, &continuation[0], &continuation[1])?;
        let unterminated = uint
            .glue()
            .mul(region, third.word(), continuation[2].word())?;
        GlueChip::assert_constant(region, &unterminated, Fp::ZERO)?;
        let ends_second = uint.glue().not(region, &continuation[1])?;
        let ends_second = uint.glue().and(region, &continuation[0], &ends_second)?;
        for (active_last, payload) in [(&ends_second, &payload[1]), (&third, &payload[2])] {
            let zero = uint.glue().is_zero(region, payload.word())?;
            let overlong = uint.glue().mul(region, active_last.word(), zero.word())?;
            GlueChip::assert_constant(region, &overlong, Fp::ZERO)?;
        }
        let second_value = uint
            .glue()
            .mul(region, continuation[0].word(), payload[1].word())?;
        let third_value = uint.glue().mul(region, third.word(), payload[2].word())?;
        let value = uint.glue().linear(
            region,
            &[
                (Fp::ONE, payload[0].word()),
                (Fp::from(128), &second_value),
                (Fp::from(16_384), &third_value),
            ],
            Fp::ZERO,
        )?;
        let value = uint.range_check::<32>(region, &value)?;
        let maximum = uint.constant::<32>(region, u128::from(MAX_RESULT_BYTES))?;
        uint.assert_le(region, &value, &maximum)?;
        let width = uint.glue().linear(
            region,
            &[(Fp::ONE, continuation[0].word()), (Fp::ONE, third.word())],
            Fp::ONE,
        )?;
        let encoded_bytes = uint.range_check::<8>(region, &width)?;
        Ok(Self {
            value,
            encoded_bytes,
        })
    }

    /// Decoded unsigned field length.
    #[must_use]
    pub const fn value(&self) -> &Uint<Fp, 32> {
        &self.value
    }

    /// Exactly one, two or three prefix bytes, selected by constrained flags.
    #[must_use]
    pub const fn encoded_bytes(&self) -> &Uint<Fp, 8> {
        &self.encoded_bytes
    }

    /// Advance a constrained cursor past the prefix and payload, requiring
    /// that the complete field fits within its original enclosing end offset.
    ///
    /// # Errors
    /// Layout errors; overflow or an out-of-container field is unsatisfied.
    pub fn end_from(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        start: &Uint<Fp, 32>,
        enclosing_end: &Uint<Fp, 32>,
    ) -> Result<Uint<Fp, 32>, Error> {
        let width = UintChip::widen::<8, 32>(&self.encoded_bytes);
        let body_start = uint.checked_add(region, start, &width)?;
        let end = uint.checked_add(region, &body_start, &self.value)?;
        uint.assert_le(region, &end, enclosing_end)?;
        Ok(end)
    }
}

#[cfg(test)]
mod tests;
