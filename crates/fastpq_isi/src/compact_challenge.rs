//! Bounded whole SHAKE256 tapes for the reviewed q77 compact candidate.
//!
//! A tape is materialized exactly once at its fixed raw-byte length. Rejected and
//! unused words remain owned until the caller binds the entire tape into its next
//! commitment-chain frame. No rejection path squeezes extra bytes. These primitive
//! owners do not authenticate statements, implement the transcript phase machine,
//! or register a production profile.
//! TODO: Replace the compact framing/transcript and wire owners together, then
//! qualify their complete resource bounds before admitting this candidate.

use crate::{keccak256::Shake256V1, poseidon::FIELD_MODULUS};
use core::fmt;
use zeroize::{Zeroize, Zeroizing};

/// Exact number of distinct initial-domain queries in the candidate.
pub const QUERY_COUNT: usize = 77;
/// Accepted Goldilocks words passed to the bounded query-position sampler.
pub const QUERY_CANDIDATES: usize = 87;
/// Raw u64 words materialized before query-word rejection.
pub const QUERY_RAW_WORDS: usize = QUERY_CANDIDATES + 6;
/// Fixed complete alpha vector; each coefficient has four Goldilocks limbs.
pub const CONSTRAINTS: usize = 923;
/// Fixed execution subgroup size.
pub const TRACE_ROWS: usize = 65_536;
/// Fixed blowup-128 initial evaluation domain.
pub const LDE_ROWS: usize = 8_388_608;
/// Conservative trace mask rank: two base positions per query and two Fp4 OODs.
pub const TRACE_MASK_COEFFICIENTS: usize = 2 * QUERY_COUNT + 8;
/// Quotient mask rank: one Fp4 point per query and one Fp4 OOD point.
pub const QUOTIENT_MASK_COEFFICIENTS: usize = QUERY_COUNT + 1;
/// Unchanged exclusive composition-mask degree.
pub const COMPOSITION_MASK_COEFFICIENTS: usize = 2 * TRACE_ROWS;
/// Largest complete materialized raw transcript tape.
pub const MAX_RAW_TAPE_BYTES: usize = (CONSTRAINTS * 4 + 6) * 8;

/// One of the ten indivisible candidate verifier messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawTapeRoundV1(u8);
impl RawTapeRoundV1 {
    /// Accept only dummy, alpha, OOD, lambda, five betas, and final query subset.
    #[must_use]
    pub const fn new(ordinal: u8) -> Option<Self> {
        if ordinal >= 1 && ordinal <= 10 {
            Some(Self(ordinal))
        } else {
            None
        }
    }
    /// Exact ordinal bound by the canonical framing owner.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        self.0
    }
    /// Fixed raw size, including all rejected and unused suffix words.
    #[must_use]
    pub const fn tape_bytes(self) -> usize {
        match self.0 {
            1 => 32,
            2 => MAX_RAW_TAPE_BYTES,
            10 => QUERY_RAW_WORDS * 8,
            _ => 80,
        }
    }
}

/// A public decoded challenge; the owning tape retains the complete raw input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawTapeMessageV1 {
    /// Initial positive-length tape with an empty decoded message.
    Dummy,
    /// Exactly 923 alphas or one scalar, represented by canonical Fp4 limbs.
    Fields(Vec<[u64; 4]>),
    /// Exactly 77 sorted, distinct initial-domain positions.
    Queries(Vec<u32>),
}

/// A complete tape allocation or finite decoder failure; callers must abort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawTapeErrorV1 {
    /// The bounded owner could not reserve its fixed storage.
    Allocation,
    /// A supplied tape did not have its exact round-specific raw length.
    Length,
    /// The finite tape did not contain enough accepted words or query positions.
    Exhausted,
    /// The sampled OOD extension point lies in the base field.
    BaseOod,
}
impl fmt::Display for RawTapeErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Allocation => "compact raw tape allocation failed",
            Self::Length => "compact raw tape has incorrect exact length",
            Self::Exhausted => "compact fixed raw tape exhausted",
            Self::BaseOod => "compact OOD challenge lies in the base field",
        })
    }
}
impl std::error::Error for RawTapeErrorV1 {}

/// Fixed-extent clearing storage for one complete raw SHAKE message.
///
/// No reader or mutable storage is exposed after construction. This owner never
/// retries or expands; the full raw bytes, not only decoded coordinates, must be
/// included in the next H frame. Debug output contains only public geometry.
pub struct RawTapeV1 {
    round: RawTapeRoundV1,
    bytes: Zeroizing<Vec<u8>>,
}
impl RawTapeV1 {
    fn zeroed(round: RawTapeRoundV1) -> Result<Self, RawTapeErrorV1> {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(round.tape_bytes())
            .map_err(|_| RawTapeErrorV1::Allocation)?;
        bytes.resize(round.tape_bytes(), 0);
        Ok(Self { round, bytes })
    }
    /// Materialize the whole fixed tape from an exact absorbed prefix and body.
    ///
    /// The caller supplies the canonical Norito G-domain/context prefix and body.
    /// Their concatenation remains the entire logical XOF input; no context hash
    /// replaces prefix bytes. The prefix clone owns its partial rate-block state.
    ///
    /// # Errors
    /// Returns an allocation error before absorbing or exposing challenge bytes.
    pub fn derive(
        round: RawTapeRoundV1,
        prefix: &Shake256V1,
        body: &[u8],
    ) -> Result<Self, RawTapeErrorV1> {
        let mut tape = Self::zeroed(round)?;
        let mut xof = prefix.clone();
        xof.update(body);
        xof.finalize().read(&mut tape.bytes);
        Ok(tape)
    }
    /// Own an exact test/replay tape without silently trimming or padding it.
    ///
    /// This does not authenticate an oracle response; verifier code must derive
    /// its own tape from its canonical transcript rather than trust proof bytes.
    ///
    /// # Errors
    /// Rejects a wrong raw length before copying, or a failed bounded allocation.
    pub fn from_bytes(round: RawTapeRoundV1, raw: &[u8]) -> Result<Self, RawTapeErrorV1> {
        if raw.len() != round.tape_bytes() {
            return Err(RawTapeErrorV1::Length);
        }
        let mut tape = Self::zeroed(round)?;
        tape.bytes.copy_from_slice(raw);
        Ok(tape)
    }
    /// Complete raw tape, including rejection values and unused suffixes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Exact round this complete tape belongs to.
    #[must_use]
    pub const fn round(&self) -> RawTapeRoundV1 {
        self.round
    }
    /// Actual retained allocation bytes, separate from inline owner size.
    #[must_use]
    pub fn allocation_bytes(&self) -> usize {
        self.bytes.capacity()
    }
    /// Decode only the fixed protocol-selected coordinates of this entire tape.
    ///
    /// # Errors
    /// Rejects finite rejection/occupancy exhaustion and base-field OOD values;
    /// never squeezes another byte. A transcript must permanently abort on error.
    pub fn decode(&self) -> Result<RawTapeMessageV1, RawTapeErrorV1> {
        if self.round.0 == 1 {
            return Ok(RawTapeMessageV1::Dummy);
        }
        let mut accepted = self.bytes.chunks_exact(8).filter_map(|bytes| {
            let word = u64::from_le_bytes(bytes.try_into().expect("exact raw u64 word"));
            (word < FIELD_MODULUS).then_some(word)
        });
        if self.round.0 == 10 {
            // First obtain every one of the 87 canonical candidates. Even if 77
            // distinct positions happen to appear earlier, raw-word exhaustion
            // cannot silently alter the reviewed finite decoder distribution.
            let mut candidates = Zeroizing::new([0_u64; QUERY_CANDIDATES]);
            for word in candidates.iter_mut() {
                *word = accepted.next().ok_or(RawTapeErrorV1::Exhausted)?;
            }
            let mut queries = Vec::new();
            queries
                .try_reserve_exact(QUERY_COUNT)
                .map_err(|_| RawTapeErrorV1::Allocation)?;
            let limit = FIELD_MODULUS - FIELD_MODULUS % LDE_ROWS as u64;
            for candidate in candidates.iter().copied() {
                if candidate >= limit {
                    continue;
                }
                // The fixed 2^23 domain bounds every remainder to a u32.
                let position = u32::try_from(candidate % LDE_ROWS as u64)
                    .map_err(|_| RawTapeErrorV1::Exhausted)?;
                if let Err(at) = queries.binary_search(&position) {
                    queries.insert(at, position);
                }
                if queries.len() == QUERY_COUNT {
                    return Ok(RawTapeMessageV1::Queries(queries));
                }
            }
            return Err(RawTapeErrorV1::Exhausted);
        }
        let count = if self.round.0 == 2 { CONSTRAINTS } else { 1 };
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(count)
            .map_err(|_| RawTapeErrorV1::Allocation)?;
        for _ in 0..count {
            let mut limbs = Zeroizing::new([0; 4]);
            for limb in limbs.iter_mut() {
                *limb = accepted.next().ok_or(RawTapeErrorV1::Exhausted)?;
            }
            fields.push(*limbs);
        }
        if self.round.0 == 3 && fields[0][1..].iter().all(|&word| word == 0) {
            return Err(RawTapeErrorV1::BaseOod);
        }
        Ok(RawTapeMessageV1::Fields(fields))
    }
}
impl fmt::Debug for RawTapeV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawTapeV1")
            .field("round", &self.round)
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}
impl Drop for RawTapeV1 {
    fn drop(&mut self) {
        self.bytes.as_mut_slice().zeroize();
        #[cfg(test)]
        ERASURE.with(|count| {
            if let Some((clean, dirty)) = count.get() {
                let observed = self
                    .bytes
                    .iter()
                    .fold((clean, dirty), |(clean, dirty), &byte| {
                        if byte == 0 {
                            (clean + 1, dirty)
                        } else {
                            (clean, dirty + 1)
                        }
                    });
                count.set(Some(observed));
            }
        });
    }
}
#[cfg(test)]
std::thread_local! {
    static ERASURE: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
#[path = "compact_challenge/tests.rs"]
mod tests;
