//! Bounded Poseidon-authenticated views of the exact result-hash byte stream.
//!
//! The tape contains `RESULT_TAG || original_R || zero_padding`, split into
//! 4096 32-byte leaves. Its 131072-byte capacity includes the maximum 65536-byte
//! R plus its domain and final compression padding. The complete hash scan
//! authenticates every consumed byte. Unvisited leaves grant no read authority:
//! parser reads are bounded to the original R length and never address them.

use ff::Field;
use iroha_pasta::{Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{
    GlueChip, Uint, UintChip, Word, WordHasher,
    blake2b::{Blake2bChip, Blake2bDigest},
};

use super::super::result::{MAX_RESULT_BYTES, RESULT_TAG, ResultHashStream};

/// Fixed internal byte-tree depth (4096 leaves, 32 bytes per leaf).
pub const TAPE_DEPTH: usize = 12;
/// Indexed raw-byte leaf domain, distinct from every wallet tree domain.
pub const TAPE_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrblf1");
/// Ordered internal byte-tree node domain.
pub const TAPE_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrbnd1");

/// An assigned opening, with no authenticity until checked against its tape.
#[derive(Clone, Debug)]
pub struct ChunkOpening {
    /// The original 32 raw bytes; each is range checked during verification.
    pub bytes: [Word<Fp>; 32],
    /// Twelve siblings in bottom-up order.
    pub siblings: [Word<Fp>; TAPE_DEPTH],
}

#[derive(Clone, Debug)]
struct WitnessTree {
    bytes: Vec<u8>,
    levels: Vec<Vec<Fp>>,
}

/// Local opening witness generator for the fixed tape. Its native computations
/// grant no authority: every resulting byte/path is verified by [`ResultTape`].
#[derive(Clone, Debug)]
pub struct ResultTapeWitness {
    tree: Value<WitnessTree>,
}

impl ResultTapeWitness {
    /// Empty witness assignments for metadata-only layout synthesis; this is not a tape root.
    pub const fn unknown() -> Self {
        Self {
            tree: Value::unknown(),
        }
    }

    /// Build witness paths from the original frame, with exact tag and zero pad.
    /// Unknown input preserves the same circuit layout through every read.
    /// # Errors
    /// A known frame exceeds the protocol ceiling.
    pub fn from_frame(frame: &Value<Vec<u8>>) -> Result<Self, Error> {
        frame.error_if_known_and(|frame| frame.len() > MAX_RESULT_BYTES as usize)?;
        let tree = frame.as_ref().map(|frame| {
            let mut bytes = vec![0; (1 << TAPE_DEPTH) * 32];
            bytes[..RESULT_TAG.len()].copy_from_slice(RESULT_TAG);
            bytes[RESULT_TAG.len()..RESULT_TAG.len() + frame.len()].copy_from_slice(frame);
            let leaves = bytes
                .chunks_exact(32)
                .enumerate()
                .map(|(index, chunk)| {
                    let pack = |part: &[u8]| {
                        part.iter().rev().fold(Fp::ZERO, |sum, byte| {
                            sum * Fp::from(256) + Fp::from(u64::from(*byte))
                        })
                    };
                    hash_with_domain(
                        TAPE_LEAF_DOMAIN,
                        &[
                            Fp::from(index as u64),
                            pack(&chunk[..16]),
                            pack(&chunk[16..]),
                        ],
                    )
                })
                .collect::<Vec<_>>();
            let mut levels = vec![leaves];
            for level in 0..TAPE_DEPTH {
                let next = levels[level]
                    .chunks_exact(2)
                    .map(|children| hash_with_domain(TAPE_NODE_DOMAIN, children))
                    .collect();
                levels.push(next);
            }
            WitnessTree { bytes, levels }
        });
        Ok(Self { tree })
    }

    /// The untrusted root witness, to be bound in the shared recursive context.
    pub fn root(&self) -> Value<Fp> {
        self.tree.as_ref().map(|tree| tree.levels[TAPE_DEPTH][0])
    }

    /// Assign an opening at a circuit-selected index; verification is separate.
    /// # Errors
    /// Layout errors. Invalid indices yield dummy witnesses and fail constraints.
    pub fn opening(
        &self,
        glue: &mut GlueChip<Fp>,
        region: &mut Region<'_, Fp>,
        index: Value<u128>,
    ) -> Result<ChunkOpening, Error> {
        let selected = self.tree.as_ref().zip(index);
        let bytes: [Value<Fp>; 32] = core::array::from_fn(|byte| {
            selected.map(|(tree, index)| {
                usize::try_from(index)
                    .ok()
                    .and_then(|index| index.checked_mul(32))
                    .and_then(|start| start.checked_add(byte))
                    .and_then(|position| tree.bytes.get(position))
                    .map_or(Fp::ZERO, |byte| Fp::from(u64::from(*byte)))
            })
        });
        let siblings: [Value<Fp>; TAPE_DEPTH] = core::array::from_fn(|level| {
            selected.map(|(tree, index)| {
                usize::try_from(index)
                    .ok()
                    .and_then(|index| tree.levels[level].get((index >> level) ^ 1))
                    .copied()
                    .unwrap_or(Fp::ZERO)
            })
        });
        Ok(ChunkOpening {
            bytes: glue
                .witnesses(region, &bytes)?
                .try_into()
                .map_err(|_| Error::Synthesis)?,
            siblings: glue
                .witnesses(region, &siblings)?
                .try_into()
                .map_err(|_| Error::Synthesis)?,
        })
    }

    /// Generate and verify a window from a constrained original-frame offset.
    /// Host indexing only generates witnesses; circuit membership and selection
    /// bind every byte to the requested offset, root and complete frame length.
    /// # Errors
    /// Layout errors; malformed paths or out-of-frame reads are unsatisfied.
    pub fn read<const N: usize>(
        &self,
        tape: &ResultTape,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        offset: &Uint<Fp, 32>,
    ) -> Result<[Word<Fp>; N], Error> {
        let index = offset
            .value()
            .map(|offset| (offset + RESULT_TAG.len() as u128) / 32);
        let first = self.opening(uint.glue(), region, index)?;
        let second = self.opening(uint.glue(), region, index.map(|index| index + 1))?;
        tape.read_window(uint, hash, region, offset, &[first, second])
    }

    /// Read a bounded frame suffix and synthesize zero bytes past its end.
    /// The offset itself must be inside or exactly at the frame end. Bytes
    /// outside the original frame cannot influence the returned cells.
    /// # Errors
    /// Layout errors; invalid paths or an offset past the frame are unsatisfied.
    pub fn read_padded<const N: usize>(
        &self,
        tape: &ResultTape,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        offset: &Uint<Fp, 32>,
    ) -> Result<[Word<Fp>; N], Error> {
        let index = offset
            .value()
            .map(|offset| (offset + RESULT_TAG.len() as u128) / 32);
        let first = self.opening(uint.glue(), region, index)?;
        let second = self.opening(uint.glue(), region, index.map(|index| index + 1))?;
        tape.read_padded_window(uint, hash, region, offset, &[first, second])
    }
}

/// One bounded internal tape commitment, not a native finality assertion.
#[derive(Clone, Debug)]
pub struct ResultTape {
    root: Word<Fp>,
    frame_len: Uint<Fp, 32>,
}

impl ResultTape {
    /// Bind one root and a result length below the protocol ceiling.
    ///
    /// The enclosing source must connect this exact tuple to a complete hash
    /// scan; construction and Merkle membership alone confer no authority.
    /// # Errors
    /// Layout errors; an oversized frame is unsatisfied.
    pub fn new(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        root: &Word<Fp>,
        frame_len: &Uint<Fp, 32>,
    ) -> Result<Self, Error> {
        let bound = uint.constant::<32>(region, u128::from(MAX_RESULT_BYTES))?;
        uint.assert_le(region, frame_len, &bound)?;
        Ok(Self {
            root: root.clone(),
            frame_len: frame_len.clone(),
        })
    }

    /// Internal root to carry unchanged across source-qualified stages.
    pub const fn root(&self) -> &Word<Fp> {
        &self.root
    }
    /// Exact original R length, excluding its domain and compression padding.
    pub const fn frame_len(&self) -> &Uint<Fp, 32> {
        &self.frame_len
    }

    /// Verify a leaf at its exact constrained index, binding every raw byte.
    /// # Errors
    /// Layout errors; a changed index, byte, path or root is unsatisfied.
    pub fn open_chunk(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        index: &Uint<Fp, 12>,
        opening: &ChunkOpening,
    ) -> Result<(), Error> {
        let mut halves = Vec::with_capacity(2);
        for half in opening.bytes.chunks_exact(16) {
            let mut packed = uint.glue().constant(region, Fp::ZERO)?;
            for byte in half.iter().rev() {
                uint.range_check::<8>(region, byte)?;
                packed = uint.glue().linear(
                    region,
                    &[(Fp::from(256), &packed), (Fp::ONE, byte)],
                    Fp::ZERO,
                )?;
            }
            halves.push(packed);
        }
        let mut node = hash.hash_words(
            region,
            TAPE_LEAF_DOMAIN,
            &[index.word().clone(), halves[0].clone(), halves[1].clone()],
        )?;
        let mut packed_index = uint.glue().constant(region, Fp::ZERO)?;
        for (level, sibling) in opening.siblings.iter().enumerate() {
            let right = uint
                .glue()
                .boolean(region, index.value().map(|value| (value >> level) & 1 == 1))?;
            packed_index = uint.glue().linear(
                region,
                &[
                    (Fp::ONE, &packed_index),
                    (Fp::from(1_u64 << level), right.word()),
                ],
                Fp::ZERO,
            )?;
            let left = uint.glue().select(region, &right, sibling, &node)?;
            let right = uint.glue().select(region, &right, &node, sibling)?;
            node = hash.hash_words(region, TAPE_NODE_DOMAIN, &[left, right])?;
        }
        GlueChip::assert_equal(region, &packed_index, index.word())?;
        GlueChip::assert_equal(region, &node, &self.root)
    }

    /// Read up to 32 consecutive bytes at a constrained offset in original R.
    ///
    /// Both source chunks are verified; the offset is decomposed and each byte
    /// selected in circuit. No host-selected offset or decoded field is trusted.
    /// This component grants authority only after the same tape's hash scan closes.
    /// # Errors
    /// Layout errors; an out-of-R window or incorrect opening is unsatisfied.
    pub fn read_window<const N: usize>(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        offset: &Uint<Fp, 32>,
        openings: &[ChunkOpening; 2],
    ) -> Result<[Word<Fp>; N], Error> {
        if !(1..=32).contains(&N) {
            return Err(Error::Synthesis);
        }
        let end = uint.checked_add_constant(region, offset, N as u128)?;
        uint.assert_le(region, &end, &self.frame_len)?;
        self.raw_window(uint, hash, region, offset, openings)
    }

    /// Read the original frame suffix, replacing every out-of-frame byte with
    /// a constrained zero. Membership of padding is never treated as authority.
    /// # Errors
    /// Layout errors; invalid paths or an offset past the frame are unsatisfied.
    pub fn read_padded_window<const N: usize>(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        offset: &Uint<Fp, 32>,
        openings: &[ChunkOpening; 2],
    ) -> Result<[Word<Fp>; N], Error> {
        uint.assert_le(region, offset, &self.frame_len)?;
        let source: [Word<Fp>; N] = self.raw_window(uint, hash, region, offset, openings)?;
        let mut out = Vec::with_capacity(N);
        for (i, byte) in source.iter().enumerate() {
            let position = uint.checked_add_constant(region, offset, i as u128)?;
            let active = uint.lt(region, &position, &self.frame_len)?;
            out.push(uint.glue().mul(region, active.word(), byte)?);
        }
        out.try_into().map_err(|_| Error::Synthesis)
    }

    fn raw_window<const N: usize>(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        offset: &Uint<Fp, 32>,
        openings: &[ChunkOpening; 2],
    ) -> Result<[Word<Fp>; N], Error> {
        if !(1..=32).contains(&N) {
            return Err(Error::Synthesis);
        }
        let absolute = uint.checked_add_constant(region, offset, RESULT_TAG.len() as u128)?;
        let index = uint.assign::<12>(region, absolute.value().map(|value| value / 32))?;
        let within = uint.assign::<5>(region, absolute.value().map(|value| value % 32))?;
        let joined = uint.glue().linear(
            region,
            &[(Fp::from(32), index.word()), (Fp::ONE, within.word())],
            Fp::ZERO,
        )?;
        GlueChip::assert_equal(region, &joined, absolute.word())?;
        self.open_chunk(uint, hash, region, &index, &openings[0])?;
        let following = uint.checked_add_constant(region, &index, 1)?;
        self.open_chunk(uint, hash, region, &following, &openings[1])?;
        // One shared one-hot selection of the starting offset is reused for all
        // output bytes. Its index and sum are both constrained.
        let mut selectors = Vec::with_capacity(32);
        let mut count = uint.glue().constant(region, Fp::ZERO)?;
        let mut selected_index = uint.glue().constant(region, Fp::ZERO)?;
        for i in 0_u64..32 {
            let selected = uint
                .glue()
                .boolean(region, within.value().map(|value| value == u128::from(i)))?;
            count = uint.glue().add(region, &count, selected.word())?;
            selected_index = uint.glue().linear(
                region,
                &[(Fp::ONE, &selected_index), (Fp::from(i), selected.word())],
                Fp::ZERO,
            )?;
            selectors.push(selected);
        }
        GlueChip::assert_constant(region, &count, Fp::ONE)?;
        GlueChip::assert_equal(region, &selected_index, within.word())?;
        let mut out = Vec::with_capacity(N);
        for byte in 0..N {
            let mut value = uint.glue().constant(region, Fp::ZERO)?;
            for (start, selected) in selectors.iter().enumerate() {
                let position = start + byte;
                let term = uint.glue().mul(
                    region,
                    selected.word(),
                    &openings[position / 32].bytes[position % 32],
                )?;
                value = uint.glue().add(region, &value, &term)?;
            }
            out.push(value);
        }
        out.try_into().map_err(|_| Error::Synthesis)
    }
}

/// Complete result hash scan over one unchanged internal tape commitment.
#[derive(Clone, Debug)]
pub struct TapeResultHashStream {
    tape: ResultTape,
    stream: ResultHashStream,
}

impl TapeResultHashStream {
    /// Reopen the complete state committed by a preceding scan endpoint.
    /// The caller must authenticate that endpoint and this unchanged tape root;
    /// importing bounded words alone never authenticates a hash prefix.
    /// # Errors
    /// Layout errors; invalid lengths, progress or chaining words fail.
    pub fn resume(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        tape: &ResultTape,
        processed: &Uint<Fp, 32>,
        words: &[Word<Fp>; 8],
    ) -> Result<Self, Error> {
        Ok(Self {
            tape: tape.clone(),
            stream: ResultHashStream::resume(
                uint,
                blake,
                region,
                tape.frame_len(),
                processed,
                words,
            )?,
        })
    }

    /// Start at the exact native IV and offset zero.
    /// # Errors
    /// Layout errors or an oversized result length.
    pub fn start(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        tape: &ResultTape,
    ) -> Result<Self, Error> {
        Ok(Self {
            tape: tape.clone(),
            stream: ResultHashStream::start(uint, blake, region, tape.frame_len())?,
        })
    }

    /// Open exactly the next four consecutive chunks and compress those bytes.
    /// Domain, final padding, byte counter and final flag are all constrained by
    /// the original result stream. No skipped or repeated chunk can advance it.
    /// # Errors
    /// Layout errors; wrong membership, cursor, tag or padding is unsatisfied.
    pub fn absorb(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        openings: &[ChunkOpening; 4],
    ) -> Result<Self, Error> {
        let block = uint.assign::<10>(
            region,
            self.stream.processed().value().map(|value| value / 128),
        )?;
        let packed = uint
            .glue()
            .linear(region, &[(Fp::from(128), block.word())], Fp::ZERO)?;
        GlueChip::assert_equal(region, &packed, self.stream.processed().word())?;
        let first = uint
            .glue()
            .linear(region, &[(Fp::from(4), block.word())], Fp::ZERO)?;
        let first = uint.range_check::<12>(region, &first)?;
        for (i, opening) in openings.iter().enumerate() {
            let index = uint.checked_add_constant(region, &first, i as u128)?;
            self.tape.open_chunk(uint, hash, region, &index, opening)?;
        }
        let bytes: [Word<Fp>; 128] =
            core::array::from_fn(|i| openings[i / 32].bytes[i % 32].clone());
        Ok(Self {
            tape: self.tape.clone(),
            stream: self.stream.absorb(uint, blake, region, &bytes)?,
        })
    }

    /// Advance an unfinished stream, or preserve every completed state word.
    ///
    /// This supports a fixed source schedule. Activity is derived from the
    /// constrained byte progress; a witness cannot disable an unfinished block.
    /// Completed streams authenticate the first four tape chunks and execute a
    /// discarded compression at offset zero, keeping the same circuit layout.
    /// Their actual progress and all eight chaining words remain unchanged.
    /// # Errors
    /// Layout errors; invalid membership, alignment, domain or padding fails.
    pub fn absorb_padded(
        &self,
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        openings: &[ChunkOpening; 4],
    ) -> Result<Self, Error> {
        let total =
            uint.checked_add_constant(region, self.tape.frame_len(), RESULT_TAG.len() as u128)?;
        let active = uint.lt(region, self.stream.processed(), &total)?;
        let zero = uint.glue().constant(region, Fp::ZERO)?;
        let safe_progress =
            uint.glue()
                .select(region, &active, self.stream.processed().word(), &zero)?;
        let safe_progress = uint.range_check::<32>(region, &safe_progress)?;
        let words = blake
            .state_words(region, self.stream.state())?
            .map(|word| word.word().clone());
        let safe = Self::resume(uint, blake, region, &self.tape, &safe_progress, &words)?;
        let next = safe.absorb(uint, hash, blake, region, openings)?;
        let next_words = blake.state_words(region, next.stream.state())?;
        let mut selected = Vec::with_capacity(8);
        for (before, after) in words.iter().zip(&next_words) {
            selected.push(uint.glue().select(region, &active, after.word(), before)?);
        }
        let selected = selected.try_into().map_err(|_| Error::Synthesis)?;
        let processed = uint.glue().select(
            region,
            &active,
            next.stream.processed().word(),
            self.stream.processed().word(),
        )?;
        let processed = uint.range_check::<32>(region, &processed)?;
        Self::resume(uint, blake, region, &self.tape, &processed, &selected)
    }

    /// Require complete consumption and release its exact native result digest.
    /// The enclosing source must equate this digest to the authenticated vote R.
    /// # Errors
    /// Layout errors; an incomplete scan is unsatisfied.
    pub fn finish(
        &self,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Blake2bDigest<Fp>, Error> {
        self.stream.finish(blake, region)
    }

    /// Exact root and length retained throughout this scan.
    pub const fn tape(&self) -> &ResultTape {
        &self.tape
    }
    /// Complete hash state and byte progress for source-qualified recursion.
    pub const fn stream(&self) -> &ResultHashStream {
        &self.stream
    }
}

#[cfg(test)]
mod tests;
