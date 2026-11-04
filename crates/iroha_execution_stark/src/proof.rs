//! Statements, proofs and their canonical byte encoding.

use crate::field::{Fp, Fp4};
use crate::fri::{FriLayerProof, FriProof};
use crate::hash::DIGEST_BYTES;
use crate::merkle::Digest;

/// First-release encoding for the BLAKE2b-384 execution-STARK suite.
const MAGIC: &[u8; 4] = b"IEB1";

/// The public claim: `steps` applications of the circuit's step function take `initial` to
/// `final_state` (passing through `checkpoint`, when given) under the public columns supplied by
/// the verifier. The application-defined `binding` is separately committed into the transcript.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Statement {
    /// Number of steps applied.
    pub steps: usize,
    /// Register values before the first step.
    pub initial: Vec<i64>,
    /// Register values after the last step.
    pub final_state: Vec<i64>,
    /// Register values after an intermediate number of steps, for example a certified checkpoint.
    pub checkpoint: Option<Checkpoint>,
    /// Application-defined bytes bound into the transcript (for example a replay digest).
    pub binding: Vec<u8>,
}

/// Registers after `step` steps (`0 <= step <= steps`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Checkpoint {
    /// Steps applied before the checkpoint.
    pub step: usize,
    /// Register values at that step.
    pub registers: Vec<i64>,
}

impl Statement {
    /// The checkpoint step, if any.
    pub fn checkpoint_step(&self) -> Option<usize> {
        self.checkpoint.as_ref().map(|c| c.step)
    }

    /// The checkpoint registers, or zeros (the checkpoint selector is then zero on every row).
    pub fn checkpoint_registers(&self, registers: usize) -> Vec<i64> {
        self.checkpoint
            .as_ref()
            .map_or_else(|| vec![0; registers], |c| c.registers.clone())
    }

    /// Canonical bytes of the checkpoint for the transcript.
    pub fn checkpoint_bytes(&self) -> Vec<u8> {
        match &self.checkpoint {
            None => vec![0],
            Some(c) => {
                let mut out = vec![1];
                out.extend_from_slice(&(c.step as u64).to_le_bytes());
                for v in &c.registers {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                out
            }
        }
    }
}

/// An upper bound on the encoded size of any proof for `circuit` under `params`.
///
/// Every variable-length part is bounded by its worst case: one Merkle path per query and layer,
/// one FRI leaf per query, and multiplicities no larger than the total lookup count (which the
/// verifier enforces).
///
/// # Errors
/// Rejects parameters the circuit cannot use and sizes not representable on this platform.
pub fn max_proof_bytes(
    circuit: &crate::Circuit,
    params: &crate::Params,
) -> Result<usize, crate::params::ParamsError> {
    params.check(circuit)?;
    let as_wide = |value: usize| u128::try_from(value).expect("usize fits u128");
    let queries = as_wide(params.queries);
    let log_lde = u128::from(circuit.log_rows + params.log_blowup);
    let width = as_wide(circuit.columns.len());
    let has_lookups = !circuit.lookups.is_empty();
    let aux_ext = if has_lookups {
        as_wide(circuit.helper_count() + 1)
    } else {
        0
    };
    let chunks = as_wide(crate::air::quotient_shape(circuit).chunks());
    let (digest, fp4, len) = (DIGEST_BYTES as u128, 32u128, 8u128);
    let rows = |w: u128| len + len + queries * w * 8 + len + queries * log_lde * digest;
    let mut total = 4 + digest + len + 1 + digest + digest;
    if has_lookups {
        let max_count = crate::params::lookup_occurrences(circuit)?;
        let varint = u128::from((64 - max_count.leading_zeros()).div_ceil(7).max(1));
        total += (1u128 << circuit.range_bits) * varint;
    }
    let next = as_wide(circuit.next_columns().len());
    total += 5 * len + (width + next + aux_ext + u128::from(has_lookups) + chunks) * fp4;
    let arity = 1u128 << params.fri_log_arity;
    let (mut log_degree, mut log_codeword) =
        (circuit.log_rows, circuit.log_rows + params.log_blowup);
    let (mut layers, mut fri) = (0u128, 0u128);
    while log_degree > params.fri_log_final_degree {
        log_codeword -= params.fri_log_arity;
        fri +=
            len + len + queries * arity * fp4 + len + queries * u128::from(log_codeword) * digest;
        layers += 1;
        log_degree = log_degree.saturating_sub(params.fri_log_arity);
    }
    total += len + layers * digest + len + (1u128 << log_degree) * fp4 + len + fri + 8;
    usize::try_from(total + rows(width) + rows(4 * aux_ext) + rows(4 * chunks))
        .map_err(|_| crate::params::ParamsError::Overflow)
}

/// Maximum siblings in a binary batch opening of `queries` distinct leaves. At tree height
/// `h` at most `min(queries, 2^h)` ancestors are occupied. Summing missing children telescopes
/// to `2 - queries + sum(occupied ancestors)`; the root-only tree needs no siblings.
fn max_batch_siblings(queries: u128, depth: u32) -> u128 {
    if depth == 0 {
        return 0;
    }
    let ancestors: u128 = (1..depth).map(|h| queries.min(1u128 << h)).sum();
    2 + ancestors - queries
}

/// Upper bound on the exact element charges used by `Reader`. Call only after checking scalar
/// parameters, circuit dimensions, row widths and FRI geometry. Unlike encoded byte size, an Fp4
/// and a digest each count once here; an opened base-field row counts once per coordinate.
///
/// Supported configurations must fit for every query layout. This can intentionally reject a
/// configuration whose unusually compact individual proof would fit the decoding budget.
pub(crate) fn max_proof_elements(circuit: &crate::Circuit, params: &crate::Params) -> u128 {
    let queries = params.queries as u128;
    let log_lde = circuit.log_rows + params.log_blowup;
    let width = circuit.columns.len() as u128;
    let has_lookups = !circuit.lookups.is_empty();
    let aux = if has_lookups {
        (circuit.helper_count() + 1) as u128
    } else {
        0
    };
    let chunks = crate::air::quotient_shape(circuit).chunks() as u128;
    let next = circuit.next_columns().len() as u128;
    let mut total = if has_lookups {
        1u128 << circuit.range_bits
    } else {
        0
    };
    // The five OOD vectors, all opened main/auxiliary/quotient rows, and their batch siblings.
    total += width + next + aux + u128::from(has_lookups) + chunks;
    total += queries * (width + 4 * aux + 4 * chunks);
    total += (2 + u128::from(has_lookups)) * max_batch_siblings(queries, log_lde);

    let arity = 1u128 << params.fri_log_arity;
    let (mut log_degree, mut log_codeword) = (circuit.log_rows, log_lde);
    while log_degree > params.fri_log_final_degree {
        log_codeword -= params.fri_log_arity;
        // Folding can merge query positions; never charge more leaves than the layer contains.
        let leaves = queries.min(1u128 << log_codeword);
        total += 1 + leaves * arity + max_batch_siblings(leaves, log_codeword);
        log_degree = log_degree.saturating_sub(params.fri_log_arity);
    }
    // Final coefficients; vector lengths and singleton root/nonce fields consume no budget.
    total + (1u128 << log_degree)
}

/// A complete proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    /// Main-trace commitment.
    pub main_root: Digest,
    /// Range-table multiplicities (sent before the LogUp challenge; empty without lookups).
    pub multiplicities: Vec<u64>,
    /// LogUp-column commitment (absent without lookups).
    pub aux_root: Option<Digest>,
    /// Quotient commitment.
    pub quotient_root: Digest,
    /// Main columns at `z`.
    pub ood_main: Vec<Fp4>,
    /// Next-referenced main columns at `w*z`.
    pub ood_main_next: Vec<Fp4>,
    /// LogUp columns at `z`.
    pub ood_aux: Vec<Fp4>,
    /// Accumulator at `w*z` (empty without lookups).
    pub ood_aux_next: Vec<Fp4>,
    /// Quotient chunks at `z`.
    pub ood_quotient: Vec<Fp4>,
    /// FRI commitments and openings.
    pub fri: FriProof,
    /// Proof-of-work nonce.
    pub pow_nonce: u64,
    /// Main rows at the sorted query positions.
    pub main_rows: Vec<Vec<Fp>>,
    /// Main batch-opening siblings.
    pub main_siblings: Vec<Digest>,
    /// LogUp rows (four base coordinates per column) at the sorted query positions.
    pub aux_rows: Vec<Vec<Fp>>,
    /// LogUp batch-opening siblings.
    pub aux_siblings: Vec<Digest>,
    /// Quotient rows at the sorted query positions.
    pub quotient_rows: Vec<Vec<Fp>>,
    /// Quotient batch-opening siblings.
    pub quotient_siblings: Vec<Digest>,
}

/// Canonical decoding failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Input ended early.
    #[error("proof bytes ended early")]
    Truncated,
    /// A field element is not canonical.
    #[error("noncanonical field element")]
    NonCanonical,
    /// A length exceeds its bound.
    #[error("length out of bounds")]
    Length,
    /// Bytes remain after the proof.
    #[error("trailing bytes after the proof")]
    Trailing,
    /// A tag byte is invalid.
    #[error("invalid tag")]
    Tag,
}

const MAX_VECTOR: u64 = 1 << 24;
/// Most query rows a proof may carry (also the largest accepted query count).
pub const MAX_ROWS: usize = 1 << 10;
/// Widest opened row (base coordinates).
pub const MAX_WIDTH: usize = 1 << 16;
/// Most FRI layers.
const MAX_LAYERS: usize = 32;
/// Widest FRI leaf (`Params::fri_log_arity <= 4`).
pub const MAX_ARITY: usize = 16;
/// Total decoded field elements and digests allowed in one proof.
pub(crate) const MAX_ELEMENTS: u64 = 1 << 26;

struct Writer(Vec<u8>);

impl Writer {
    fn varint(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.0.push((v as u8) | 0x80);
            v >>= 7;
        }
        self.0.push(v as u8);
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, v: usize) {
        self.u64(v as u64);
    }
    fn digest(&mut self, d: &Digest) {
        self.0.extend_from_slice(d);
    }
    fn digests(&mut self, ds: &[Digest]) {
        self.len(ds.len());
        for d in ds {
            self.digest(d);
        }
    }
    fn fp(&mut self, v: Fp) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn fp4(&mut self, v: Fp4) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn fp4s(&mut self, vs: &[Fp4]) {
        self.len(vs.len());
        for v in vs {
            self.fp4(*v);
        }
    }
    fn rows(&mut self, rows: &[Vec<Fp>]) {
        self.len(rows.len());
        let width = rows.first().map_or(0, Vec::len);
        self.len(width);
        for row in rows {
            debug_assert_eq!(row.len(), width);
            for v in row {
                self.fp(*v);
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    budget: u64,
}

impl Reader<'_> {
    fn varint(&mut self) -> Result<u64, DecodeError> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.take(1)?[0];
            let part = u64::from(byte & 0x7f);
            if shift == 63 && part > 1 {
                return Err(DecodeError::NonCanonical);
            }
            value |= part << shift;
            if byte & 0x80 == 0 {
                // Canonical: no redundant trailing zero groups.
                if byte == 0 && shift > 0 {
                    return Err(DecodeError::NonCanonical);
                }
                return Ok(value);
            }
        }
        Err(DecodeError::NonCanonical)
    }
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        if self.bytes.len() < n {
            return Err(DecodeError::Truncated);
        }
        let (head, tail) = self.bytes.split_at(n);
        self.bytes = tail;
        Ok(head)
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    fn len(&mut self) -> Result<usize, DecodeError> {
        let v = self.u64()?;
        if v > MAX_VECTOR {
            return Err(DecodeError::Length);
        }
        usize::try_from(v).map_err(|_| DecodeError::Length)
    }
    /// Charges `count` elements against the decoding budget; every element needs at least one
    /// input byte, so a count beyond the remaining input is rejected before allocating.
    fn charge(&mut self, count: u64) -> Result<(), DecodeError> {
        if count > self.budget || count > self.bytes.len() as u64 {
            return Err(DecodeError::Length);
        }
        self.budget -= count;
        Ok(())
    }
    fn digest(&mut self) -> Result<Digest, DecodeError> {
        Ok(self.take(DIGEST_BYTES)?.try_into().expect("digest width"))
    }
    fn digests(&mut self) -> Result<Vec<Digest>, DecodeError> {
        let n = self.len()?;
        self.charge(n as u64)?;
        (0..n).map(|_| self.digest()).collect()
    }
    fn fp(&mut self) -> Result<Fp, DecodeError> {
        Fp::from_canonical_le_bytes(self.take(8)?.try_into().expect("8"))
            .ok_or(DecodeError::NonCanonical)
    }
    fn fp4(&mut self) -> Result<Fp4, DecodeError> {
        Ok(Fp4([self.fp()?, self.fp()?, self.fp()?, self.fp()?]))
    }
    fn fp4s(&mut self) -> Result<Vec<Fp4>, DecodeError> {
        let n = self.len()?;
        self.charge(n as u64)?;
        (0..n).map(|_| self.fp4()).collect()
    }
    fn rows(&mut self) -> Result<Vec<Vec<Fp>>, DecodeError> {
        let n = self.len()?;
        let width = self.len()?;
        if n > MAX_ROWS || width > MAX_WIDTH {
            return Err(DecodeError::Length);
        }
        if n == 0 && width != 0 {
            return Err(DecodeError::NonCanonical);
        }
        self.charge((n as u64) * (width as u64))?;
        (0..n)
            .map(|_| (0..width).map(|_| self.fp()).collect())
            .collect()
    }
}

impl Proof {
    /// Canonical encoding.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.0.extend_from_slice(MAGIC);
        w.digest(&self.main_root);
        w.len(self.multiplicities.len());
        for m in &self.multiplicities {
            w.varint(*m);
        }
        match &self.aux_root {
            Some(root) => {
                w.0.push(1);
                w.digest(root);
            }
            None => w.0.push(0),
        }
        w.digest(&self.quotient_root);
        w.fp4s(&self.ood_main);
        w.fp4s(&self.ood_main_next);
        w.fp4s(&self.ood_aux);
        w.fp4s(&self.ood_aux_next);
        w.fp4s(&self.ood_quotient);
        w.digests(&self.fri.roots);
        w.fp4s(&self.fri.final_coefficients);
        w.len(self.fri.layers.len());
        for layer in &self.fri.layers {
            w.len(layer.leaves.len());
            let arity = layer.leaves.first().map_or(0, Vec::len);
            w.len(arity);
            for leaf in &layer.leaves {
                for v in leaf {
                    w.fp4(*v);
                }
            }
            w.digests(&layer.siblings);
        }
        w.u64(self.pow_nonce);
        w.rows(&self.main_rows);
        w.digests(&self.main_siblings);
        w.rows(&self.aux_rows);
        w.digests(&self.aux_siblings);
        w.rows(&self.quotient_rows);
        w.digests(&self.quotient_siblings);
        w.0
    }

    /// Strict canonical decoding.
    ///
    /// # Errors
    /// Rejects truncated, noncanonical, oversized or trailing input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader {
            bytes,
            budget: MAX_ELEMENTS,
        };
        if r.take(MAGIC.len())? != MAGIC {
            return Err(DecodeError::Tag);
        }
        let main_root = r.digest()?;
        let multiplicity_count = r.len()?;
        r.charge(multiplicity_count as u64)?;
        let multiplicities = (0..multiplicity_count)
            .map(|_| r.varint())
            .collect::<Result<Vec<u64>, _>>()?;
        let aux_root = match r.take(1)?[0] {
            0 => None,
            1 => Some(r.digest()?),
            _ => return Err(DecodeError::Tag),
        };
        let quotient_root = r.digest()?;
        let ood_main = r.fp4s()?;
        let ood_main_next = r.fp4s()?;
        let ood_aux = r.fp4s()?;
        let ood_aux_next = r.fp4s()?;
        let ood_quotient = r.fp4s()?;
        let roots = r.digests()?;
        let final_coefficients = r.fp4s()?;
        let layer_count = r.len()?;
        if layer_count > MAX_LAYERS {
            return Err(DecodeError::Length);
        }
        let mut layers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            let leaf_count = r.len()?;
            let arity = r.len()?;
            if leaf_count > MAX_ROWS || arity > MAX_ARITY || (leaf_count > 0 && arity == 0) {
                return Err(DecodeError::Length);
            }
            if leaf_count == 0 && arity != 0 {
                return Err(DecodeError::NonCanonical);
            }
            r.charge((leaf_count as u64) * (arity as u64))?;
            let leaves = (0..leaf_count)
                .map(|_| (0..arity).map(|_| r.fp4()).collect())
                .collect::<Result<Vec<Vec<Fp4>>, _>>()?;
            let siblings = r.digests()?;
            layers.push(FriLayerProof { leaves, siblings });
        }
        let pow_nonce = r.u64()?;
        let main_rows = r.rows()?;
        let main_siblings = r.digests()?;
        let aux_rows = r.rows()?;
        let aux_siblings = r.digests()?;
        let quotient_rows = r.rows()?;
        let quotient_siblings = r.digests()?;
        if !r.bytes.is_empty() {
            return Err(DecodeError::Trailing);
        }
        Ok(Self {
            main_root,
            multiplicities,
            aux_root,
            quotient_root,
            ood_main,
            ood_main_next,
            ood_aux,
            ood_aux_next,
            ood_quotient,
            fri: FriProof {
                roots,
                final_coefficients,
                layers,
            },
            pow_nonce,
            main_rows,
            main_siblings,
            aux_rows,
            aux_siblings,
            quotient_rows,
            quotient_siblings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_rejects_noncanonical_empty_row_width() {
        for width in [1, MAX_WIDTH] {
            let mut bytes = 0u64.to_le_bytes().to_vec();
            bytes.extend_from_slice(&(width as u64).to_le_bytes());
            let mut reader = Reader {
                bytes: &bytes,
                budget: MAX_ELEMENTS,
            };
            assert_eq!(reader.rows(), Err(DecodeError::NonCanonical));
        }
        let mut reader = Reader {
            bytes: &[0; 16],
            budget: MAX_ELEMENTS,
        };
        assert_eq!(reader.rows().unwrap(), Vec::<Vec<Fp>>::new());
    }

    #[test]
    fn decoder_rejects_noncanonical_empty_fri_arity() {
        // Complete proof encoding with empty vectors and one empty FRI layer. This is a codec
        // fixture, not a valid cryptographic proof. The only accepted empty-leaf arity is zero.
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; DIGEST_BYTES]);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0u8; DIGEST_BYTES]);
        for _ in 0..7 {
            bytes.extend_from_slice(&0u64.to_le_bytes());
        }
        bytes.extend_from_slice(&1u64.to_le_bytes()); // FRI layer count
        bytes.extend_from_slice(&0u64.to_le_bytes()); // empty leaves
        let arity_offset = bytes.len();
        bytes.extend_from_slice(&0u64.to_le_bytes()); // canonical empty arity
        bytes.extend_from_slice(&0u64.to_le_bytes()); // FRI siblings
        bytes.extend_from_slice(&0u64.to_le_bytes()); // nonce
        for _ in 0..9 {
            bytes.extend_from_slice(&0u64.to_le_bytes()); // three empty matrices and siblings
        }
        assert_eq!(Proof::from_bytes(&bytes).unwrap().to_bytes(), bytes);
        for arity in [1, MAX_ARITY] {
            let mut bad = bytes.clone();
            bad[arity_offset..arity_offset + 8].copy_from_slice(&(arity as u64).to_le_bytes());
            assert_eq!(Proof::from_bytes(&bad), Err(DecodeError::NonCanonical));
        }
    }

    #[test]
    fn verified_proof_rejects_noncanonical_empty_aux_width() {
        let mut builder = crate::Builder::new("canonical constant register", 8);
        let value = builder.register("value", 0, 9);
        builder.set_next(&value, &value);
        let circuit = builder.finish();
        let params = crate::Params {
            queries: 8,
            grinding_bits: 0,
            ..crate::Params::standard()
        };
        let statement = Statement {
            steps: 2,
            initial: vec![7],
            final_state: vec![7],
            checkpoint: None,
            binding: b"canonical codec".to_vec(),
        };
        let states = vec![vec![7]; 3];
        let proof = crate::prove(&circuit, &params, &statement, &[], &states).unwrap();
        crate::verify(&circuit, &params, &statement, &[], &proof).unwrap();
        assert!(proof.aux_rows.is_empty());
        let bytes = proof.to_bytes();
        assert_eq!(Proof::from_bytes(&bytes).unwrap(), proof);
        // The retired 256-bit suite is never accepted or guessed from payload geometry.
        let mut retired = bytes.clone();
        retired[..4].copy_from_slice(b"IES1");
        assert_eq!(Proof::from_bytes(&retired), Err(DecodeError::Tag));
        // Reconstruct the exact tail after aux_rows. Its width is the preceding u64; mutate only
        // that otherwise-discarded metadata, preserving all transcript-bound proof values.
        let mut tail = Writer(Vec::new());
        tail.digests(&proof.aux_siblings);
        tail.rows(&proof.quotient_rows);
        tail.digests(&proof.quotient_siblings);
        let width_offset = bytes.len() - tail.0.len() - 8;
        assert_eq!(&bytes[width_offset..width_offset + 8], &[0; 8]);
        for width in [1, MAX_WIDTH] {
            let mut bad = bytes.clone();
            bad[width_offset..width_offset + 8].copy_from_slice(&(width as u64).to_le_bytes());
            assert_eq!(Proof::from_bytes(&bad), Err(DecodeError::NonCanonical));
        }
    }

    #[test]
    fn decoder_rejects_oversized_dimensions_without_allocating() {
        // Header, root, zero multiplicities, no aux root, quotient root, five empty OOD vectors,
        // no FRI roots or final coefficients, then one FRI layer of 2^24 zero-arity leaves.
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; DIGEST_BYTES]);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0u8; DIGEST_BYTES]);
        for _ in 0..7 {
            bytes.extend_from_slice(&0u64.to_le_bytes());
        }
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&(1u64 << 24).to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(Proof::from_bytes(&bytes), Err(DecodeError::Length));
    }
}
