//! Fixed DEEP context, typed commitments and atomic whole-tape transcript.
//!
//! Canonical PrefixFrame/BodyV1 serialization and six-lane hashing are owned by
//! compact_v1. This private candidate supplies only a closed geometry and message
//! schedule. Context construction authenticates no public statement; the caller
//! must perform the OOD AIR identity and all opening/degree checks separately.
//! The bounded engine and producer use this schedule through the offline facade.
//! TODO: Qualify the complete protocol and authenticate ledger context before
//! replacing the node's replay verifier or changing production admission.

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use norito::NoritoSerialize;

use super::{
    compact_protocol::FixedAir,
    compact_public_columns::{COMMITTED_COLUMN_COUNT, LAYOUT_ID},
    compact_v1::{BodyFields, Context as FramingContext, Frame, PreparedHashFrame},
    deep_geometry::{
        CONSTRAINTS, COSET_OFFSET, FRI_ARITIES, FRI_DEGREES, FRI_LENGTHS, LDE_ROOT, LDE_ROWS,
        QUERY_CANDIDATES, QUERY_COUNT, TRACE_ROWS,
    },
    deep_relation::DeepRelation,
};
use crate::field::{GOLDILOCKS_MODULUS_V1 as MODULUS, GoldilocksFp4V1 as F};

/// Complete protocol identity; every fixed geometry field is also in the context.
pub(super) const IDENTITY: &[u8] = b"fastpq:compact:deep-ali:h6:g-field-blocks:row301:qpair:ood604:components606:lambda-powers:trace-shift2:quotient-shift1:arity16-16-8-8-4:terminal128:q64:c74:v1";
const MAX_STATEMENT_BYTES: usize = 240 * 1024;
const MAX_RELATION_IDENTITY_BYTES: usize = 256;
#[cfg(test)]
const FIXTURE_RELATION_IDENTITY: &str = "fastpq:deep:explicit-context-fixture:v1";
const OOD_VALUES: usize = 2 * COMMITTED_COLUMN_COUNT + 2;

/// Failure of the fixed DEEP transcript or its typed input validation.
#[derive(Debug, thiserror::Error)]
pub(super) enum BindingError {
    /// The prepared relation, identity or statement violates the fixed context contract.
    #[error("DEEP relation must match the fixed schema and bounded identity/statement envelope")]
    Context,
    /// Only the ten specified whole-message rounds exist.
    #[error("DEEP verifier round is outside 1..=10")]
    Round,
    /// A commitment input has incorrect shape, position or canonical encoding.
    #[error("DEEP tree or OOD input has invalid geometry or field encoding")]
    Shape,
    /// A tape has the wrong fixed length or a noncanonical coordinate.
    #[error("DEEP whole tape has invalid length or field encoding")]
    Tape,
    /// Query sampling did not produce enough distinct accepted positions.
    #[error("DEEP fixed query tape exhausted")]
    Exhausted,
    /// Sampling an OOD point in the base field permanently aborts this attempt.
    #[error("DEEP OOD challenge lies in the base field")]
    BaseOod,
    /// The requested action differs from the fixed commitment/challenge schedule.
    #[error("DEEP transcript operation is out of sequence")]
    Phase,
    /// The shared canonical framing owner rejected the operation.
    #[error(transparent)]
    Framing(#[from] super::compact_v1::CandidateError),
    /// Canonical Norito framing failed.
    #[error(transparent)]
    Encode(#[from] norito::core::Error),
}

type Result<T> = std::result::Result<T, BindingError>;

/// Ordinal of one indivisible verifier message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Round(u8);

impl Round {
    /// Accept exactly dummy, alpha, z, lambda, five betas and whole query subset.
    pub(super) fn new(ordinal: u8) -> Result<Self> {
        (1..=10)
            .contains(&ordinal)
            .then_some(Self(ordinal))
            .ok_or(BindingError::Round)
    }

    /// Fixed ordinal used by the sole framing owner.
    pub(super) const fn ordinal(self) -> u8 {
        self.0
    }

    /// Include every materialized coordinate, including unused suffix values.
    pub(super) const fn tape_bytes(self) -> usize {
        match self.0 {
            2 => (CONSTRAINTS * 4).div_ceil(6) * 48,
            10 => QUERY_CANDIDATES.div_ceil(6) * 48,
            _ => 48,
        }
    }
}

/// Whole decoded verifier message, with the complete tape retained internally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Message {
    /// Positive-length tape decoding to the initial empty message.
    Dummy,
    /// Independent alpha coefficients, one OOD point, or one lambda/beta scalar.
    Fields(Vec<F>),
    /// Exactly 64 distinct, ascending initial-domain positions.
    Queries(Vec<u32>),
}

/// Fixed oracles accepted by the new profile; no caller-selected geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Oracle {
    /// The 301 retained base-field trace cells.
    Row,
    /// Both extension-field coefficient-half evaluations in one leaf.
    QuotientPair,
    /// Complete fibers of arity 16,16,8,8,4 for source layers zero through four.
    Fri(u8),
    /// All 128 terminal extension-field values in one complete leaf.
    Terminal,
}

impl Oracle {
    pub(super) fn shape(self) -> Result<(u8, u8, usize, usize)> {
        match self {
            Self::Row => Ok((1, 0, LDE_ROWS, COMMITTED_COLUMN_COUNT * 8)),
            Self::QuotientPair => Ok((3, 0, LDE_ROWS, 2 * F::BYTES)),
            Self::Fri(round @ 0..=4) => {
                let i = usize::from(round);
                Ok((4, round, FRI_LENGTHS[i + 1], FRI_ARITIES[i] * F::BYTES))
            }
            Self::Terminal => Ok((4, 5, 1, FRI_LENGTHS[5] * F::BYTES)),
            Self::Fri(_) => Err(BindingError::Shape),
        }
    }
}

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::deep_binding::StatementContext",
    frame = "fastpq_prover::deep::StatementContextV1"
)]
struct StatementContext {
    relation: String,
    layout: String,
    trace_rows: u32,
    lde_rows: u32,
    columns: u32,
    constraints: u32,
    modulus: u64,
    extension_nonresidue: u64,
    lde_root: u64,
    coset_offset: u64,
    fri_arities: [u32; 5],
    fri_lengths: [u32; 6],
    fri_degrees: [u32; 6],
    query_count: u32,
    query_candidates: u32,
    statement: Vec<u8>,
}

/// Full fixed profile plus caller-owned canonical public statement.
#[derive(Clone, Debug)]
pub(super) struct Context {
    framing: FramingContext,
}

impl Context {
    /// Bind the outer prepared relation identity and its exact complete statement.
    ///
    /// Only the closed relation bridge supplies the arithmetic owner. Reject any
    /// schema or statement divergence before framing or allocating context bytes.
    pub(super) fn for_relation(relation: &impl DeepRelation) -> Result<Self> {
        Self::preflight_relation(relation)?;
        Self::with_identity(relation.schema().identity, relation.statement_bytes())
    }

    /// Validate the closed relation and fixed envelope without framing or hashing.
    pub(super) fn preflight_relation(relation: &impl DeepRelation) -> Result<()> {
        let schema = relation.schema();
        let inner = relation.deep_relation();
        let reference = inner.schema();
        if schema.trace_rows != TRACE_ROWS
            || schema.width != 342
            || schema.constraints != CONSTRAINTS
            || reference.trace_rows != schema.trace_rows
            || reference.width != schema.width
            || reference.constraints != schema.constraints
            || relation.statement_bytes() != inner.statement_bytes()
        {
            return Err(BindingError::Context);
        }
        Self::check_envelope(schema.identity, relation.statement_bytes())
    }

    /// Explicit raw context fixture, never the prover/verifier relation entry.
    #[cfg(test)]
    pub(super) fn new(statement: &[u8]) -> Result<Self> {
        Self::with_identity(FIXTURE_RELATION_IDENTITY, statement)
    }

    fn check_envelope(identity: &str, statement: &[u8]) -> Result<()> {
        if identity.is_empty()
            || identity.len() > MAX_RELATION_IDENTITY_BYTES
            || statement.is_empty()
            || statement.len() > MAX_STATEMENT_BYTES
        {
            return Err(BindingError::Context);
        }
        Ok(())
    }

    fn with_identity(identity: &str, statement: &[u8]) -> Result<Self> {
        Self::check_envelope(identity, statement)?;
        let encoded = norito::encode_canonical(&StatementContext {
            relation: identity.to_owned(),
            layout: LAYOUT_ID.to_owned(),
            trace_rows: TRACE_ROWS as u32,
            lde_rows: LDE_ROWS as u32,
            columns: COMMITTED_COLUMN_COUNT as u32,
            constraints: CONSTRAINTS as u32,
            modulus: MODULUS,
            extension_nonresidue: 7,
            lde_root: LDE_ROOT,
            coset_offset: COSET_OFFSET,
            fri_arities: FRI_ARITIES.map(|v| v as u32),
            fri_lengths: FRI_LENGTHS.map(|v| v as u32),
            fri_degrees: FRI_DEGREES.map(|v| v as u32),
            query_count: QUERY_COUNT as u32,
            query_candidates: QUERY_CANDIDATES as u32,
            statement: statement.to_vec(),
        })?;
        Ok(Self {
            framing: FramingContext::new_deep(&encoded)?,
        })
    }

    /// Hash only canonical complete fixed-shape leaves at valid positions.
    pub(super) fn hash_leaf(&self, oracle: Oracle, index: u32, payload: &[u8]) -> Result<Digest> {
        Ok(self
            .framing
            .hash_frame(&self.leaf_frame(oracle, index, payload)?)?)
    }

    fn leaf_frame<'a>(&self, oracle: Oracle, index: u32, payload: &'a [u8]) -> Result<Frame<'a>> {
        let (tag, round, leaves, bytes) = oracle.shape()?;
        if index as usize >= leaves || payload.len() != bytes || !canonical_words(payload) {
            return Err(BindingError::Shape);
        }
        Ok(self
            .framing
            .frame(1, tag, round, 0, index, 48, BodyFields::One(payload)))
    }

    /// Prepare a complete shape-checked leaf under the unchanged canonical owner.
    pub(super) fn prepare_leaf(
        &self,
        oracle: Oracle,
        index: u32,
        payload: &[u8],
    ) -> crate::Result<PreparedHashFrame> {
        let frame = self.leaf_frame(oracle, index, payload).map_err(|error| {
            crate::Error::InvalidTraceShape {
                details: error.to_string(),
            }
        })?;
        self.framing.prepare_hash_frame(&frame)
    }

    /// Hash a binary authentication parent, retaining the sole-leaf duplicate rule.
    pub(super) fn hash_parent(
        &self,
        oracle: Oracle,
        level: u32,
        index: u32,
        left: Digest,
        right: Digest,
    ) -> Result<Digest> {
        let (tag, round, leaves, _) = oracle.shape()?;
        if level == 0
            || level > leaves.ilog2().max(1)
            || index as usize >= (leaves >> level).max(1)
            || (leaves == 1 && left != right)
        {
            return Err(BindingError::Shape);
        }
        Ok(self.framing.hash_frame(&self.framing.frame(
            2,
            tag,
            round,
            level,
            index,
            48,
            BodyFields::Two(&left.to_le_bytes(), &right.to_le_bytes()),
        ))?)
    }

    /// Prepare a complete parent; use the same strict shape checks as verification.
    pub(super) fn prepare_parent(
        &self,
        oracle: Oracle,
        level: u32,
        index: u32,
        left: Digest,
        right: Digest,
    ) -> crate::Result<PreparedHashFrame> {
        let (tag, round, leaves, _) =
            oracle
                .shape()
                .map_err(|error| crate::Error::InvalidTraceShape {
                    details: error.to_string(),
                })?;
        if level == 0
            || level > leaves.ilog2().max(1)
            || index as usize >= (leaves >> level).max(1)
            || (leaves == 1 && left != right)
        {
            return Err(crate::Error::InvalidTraceShape {
                details: BindingError::Shape.to_string(),
            });
        }
        self.framing.prepare_hash_frame(&self.framing.frame(
            2,
            tag,
            round,
            level,
            index,
            48,
            BodyFields::Two(&left.to_le_bytes(), &right.to_le_bytes()),
        ))
    }

    fn hash_ood(&self, current: &[F], next: &[F], quotient: &[F]) -> Result<Digest> {
        let bytes = ood_bytes(current, next, quotient)?;
        Ok(self.framing.hash_frame(&self.framing.frame(
            1,
            5,
            0,
            0,
            0,
            48,
            BodyFields::One(&bytes),
        ))?)
    }

    fn chain(&self, round: Round, tape: &[u8], root: Digest) -> Result<Digest> {
        if round.0 == 10 || tape.len() != round.tape_bytes() || !canonical_words(tape) {
            return Err(BindingError::Phase);
        }
        Ok(self.framing.hash_frame(&self.framing.frame(
            3,
            0,
            round.0,
            0,
            0,
            48,
            BodyFields::Two(tape, &root.to_le_bytes()),
        ))?)
    }
}

fn canonical_words(bytes: &[u8]) -> bool {
    bytes.len().is_multiple_of(8)
        && bytes
            .chunks_exact(8)
            .all(|word| u64::from_le_bytes(word.try_into().expect("exact field word")) < MODULUS)
}

fn ood_bytes(current: &[F], next: &[F], quotient: &[F]) -> Result<Vec<u8>> {
    if current.len() != COMMITTED_COLUMN_COUNT
        || next.len() != COMMITTED_COLUMN_COUNT
        || quotient.len() != 2
        || current
            .iter()
            .chain(next)
            .chain(quotient)
            .any(|v| v.coefficients().iter().any(|&c| c >= MODULUS))
    {
        return Err(BindingError::Shape);
    }
    let mut bytes = Vec::with_capacity(OOD_VALUES * F::BYTES);
    for value in current.iter().chain(next).chain(quotient) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

fn decode(round: Round, raw: &[u8]) -> Result<Message> {
    if raw.len() != round.tape_bytes() || !canonical_words(raw) {
        return Err(BindingError::Tape);
    }
    if round.0 == 1 {
        return Ok(Message::Dummy);
    }
    if round.0 == 10 {
        let limit = MODULUS - MODULUS % LDE_ROWS as u64;
        let mut queries = Vec::with_capacity(QUERY_COUNT);
        for word in raw.chunks_exact(8).take(QUERY_CANDIDATES) {
            let candidate = u64::from_le_bytes(word.try_into().expect("exact field word"));
            if candidate >= limit {
                continue;
            }
            let index = (candidate % LDE_ROWS as u64) as u32;
            match queries.binary_search(&index) {
                Ok(_) => {}
                Err(at) => queries.insert(at, index),
            }
            if queries.len() == QUERY_COUNT {
                return Ok(Message::Queries(queries));
            }
        }
        return Err(BindingError::Exhausted);
    }
    let count = if round.0 == 2 { CONSTRAINTS } else { 1 };
    let values: Vec<_> = raw[..count * F::BYTES]
        .chunks_exact(F::BYTES)
        .map(|bytes| {
            F::new(core::array::from_fn(|i| {
                u64::from_le_bytes(
                    bytes[i * 8..(i + 1) * 8]
                        .try_into()
                        .expect("exact field word"),
                )
            }))
            .expect("complete tape validated")
        })
        .collect();
    if round.0 == 3 && values[0].coefficients()[1..].iter().all(|&v| v == 0) {
        return Err(BindingError::BaseOod);
    }
    Ok(Message::Fields(values))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Ready(Round),
    Pending { round: Round, raw: Vec<u8> },
    Complete,
    Aborted,
}

/// Fixed schedule; success here does not mean the proof or statement is valid.
#[derive(Clone, Debug)]
pub(super) struct Transcript {
    context: Context,
    predecessor: Digest,
    phase: Phase,
}

impl Transcript {
    /// Begin with the fixed zero anchor and the dummy whole-message tape.
    pub(super) fn new(context: Context) -> Self {
        Self {
            context,
            predecessor: Digest::default(),
            phase: Phase::Ready(Round(1)),
        }
    }

    /// Decode one entire tape; a sampling or framing failure permanently aborts.
    pub(super) fn challenge(&mut self) -> Result<Message> {
        self.challenge_with(|context, round, body, output| {
            Ok(context.framing.expand_deep(round, body, output)?)
        })
    }

    fn challenge_with(
        &mut self,
        fill: impl FnOnce(&Context, Round, &[u8], &mut [u8]) -> Result<()>,
    ) -> Result<Message> {
        let Phase::Ready(round) = self.phase else {
            return Err(BindingError::Phase);
        };
        self.phase = Phase::Aborted;
        let predecessor = self.predecessor.to_le_bytes();
        let frame = self.context.framing.frame(
            4,
            0,
            round.0,
            0,
            0,
            round.tape_bytes(),
            BodyFields::One(&predecessor),
        );
        let encoded = norito::encode_canonical(&frame)?;
        let mut raw = vec![0; round.tape_bytes()];
        fill(&self.context, round, &encoded, &mut raw)?;
        let message = decode(round, &raw)?;
        self.phase = if round.0 == 10 {
            Phase::Complete
        } else {
            Phase::Pending { round, raw }
        };
        Ok(message)
    }

    /// Commit exactly the oracle expected after this message; OOD has its own API.
    pub(super) fn commit_root(&mut self, oracle: Oracle, root: Digest) -> Result<()> {
        let Phase::Pending { round, .. } = &self.phase else {
            return Err(BindingError::Phase);
        };
        let expected = match round.0 {
            1 => Oracle::Row,
            2 => Oracle::QuotientPair,
            4..=8 => Oracle::Fri(round.0 - 4),
            9 => Oracle::Terminal,
            _ => return Err(BindingError::Phase),
        };
        if oracle != expected {
            return Err(BindingError::Phase);
        }
        self.commit_digest(root)
    }

    /// Bind every coordinate of both OOD rows and quotient halves before lambda.
    pub(super) fn commit_ood(&mut self, current: &[F], next: &[F], quotient: &[F]) -> Result<()> {
        if !matches!(
            self.phase,
            Phase::Pending {
                round: Round(3),
                ..
            }
        ) {
            return Err(BindingError::Phase);
        }
        let digest = match self.context.hash_ood(current, next, quotient) {
            Ok(digest) => digest,
            Err(error) => {
                self.phase = Phase::Aborted;
                return Err(error);
            }
        };
        self.commit_digest(digest)
    }

    fn commit_digest(&mut self, root: Digest) -> Result<()> {
        let Phase::Pending { round, raw } = std::mem::replace(&mut self.phase, Phase::Aborted)
        else {
            return Err(BindingError::Phase);
        };
        self.predecessor = self.context.chain(round, &raw, root)?;
        self.phase = Phase::Ready(Round::new(round.0 + 1)?);
        Ok(())
    }
}

#[cfg(test)]
#[path = "deep_binding/tests.rs"]
mod tests;
