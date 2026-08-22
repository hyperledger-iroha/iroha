//! Acyclic parent-lineage and field-neutral STATE ABI contract for Offline Cash V2.
//!
//! This module is intentionally a codec and ordering contract, not a recursive
//! verifier.  A STATE proof exposes one aggregate accumulator produced entirely
//! before that proof's transcript.  The proof's own accumulator is derived only
//! after its public instances and proof transcript have been read, so it has no
//! representation in the current proof's public instances.

use core::fmt;

use halo2_proofs::halo2curves::{
    CurveAffine,
    ff::PrimeField,
    pasta::{EpAffine, EqAffine},
};
use iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2;

use super::{
    OFFLINE_CASH_HALO2_K_V2, OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2,
    OFFLINE_CASH_STATE_ABI_WORDS_V2, OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2,
    OFFLINE_CASH_STATE_INSTANCE_CELLS_V2, OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2,
    OfflineCashHalo2ParityV2,
};
use crate::zk::pasta_ipa_recursion::PastaIpaInstanceQueryV1;

const SCALAR_BYTES: usize = 32;
const POINT_BYTES: usize = 32;
const LINEAGE_ROUNDS: usize = OFFLINE_CASH_HALO2_K_V2 as usize;
const LINEAGE_BYTES: usize = OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2 as usize;
const LINEAGE_WORDS: usize = LINEAGE_BYTES / 4;
const STATE_ABI_WORDS: usize = OFFLINE_CASH_STATE_ABI_WORDS_V2 as usize;
const STATE_WORDS_PER_INSTANCE: usize = OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2 as usize;
const STATE_INSTANCE_CELLS: usize = OFFLINE_CASH_STATE_INSTANCE_CELLS_V2 as usize;
const PACKED_CELL_BYTES: usize = STATE_WORDS_PER_INSTANCE * 4;

/// STATE ABI version committed in header word zero.
pub(super) const OFFLINE_CASH_STATE_ABI_VERSION_V2: u32 = 2;
/// V2 wire-version word reserved by this source-only ABI contract.
pub(super) const OFFLINE_CASH_STATE_WIRE_VERSION_V2: u32 = 2;
/// Fixed semantic parent-digest slots in header word five.
pub(super) const OFFLINE_CASH_STATE_FIXED_SEMANTIC_PARENT_COUNT_V2: u32 = 2;
/// Number of little-endian `u32` words in every digest.
pub(super) const OFFLINE_CASH_STATE_DIGEST_WORDS_V2: usize = 8;
/// Number of digest fields preceding amount and scale.
pub(super) const OFFLINE_CASH_STATE_DIGEST_FIELDS_V2: usize = 10;
/// First release-digest word.
pub(super) const OFFLINE_CASH_STATE_RELEASE_WORD_START_V2: usize = 8;
/// First parity-protocol-digest word.
pub(super) const OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2: usize = 16;
/// First semantic-digest word.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_WORD_START_V2: usize = 24;
/// First context-digest word.
pub(super) const OFFLINE_CASH_STATE_CONTEXT_WORD_START_V2: usize = 32;
/// First request-digest word.
pub(super) const OFFLINE_CASH_STATE_REQUEST_WORD_START_V2: usize = 40;
/// First semantic parent-zero digest word.
pub(super) const OFFLINE_CASH_STATE_PARENT_0_WORD_START_V2: usize = 48;
/// First semantic parent-one digest word.
pub(super) const OFFLINE_CASH_STATE_PARENT_1_WORD_START_V2: usize = 56;
/// First result-digest word.
pub(super) const OFFLINE_CASH_STATE_RESULT_WORD_START_V2: usize = 64;
/// First link-digest word.
pub(super) const OFFLINE_CASH_STATE_LINK_WORD_START_V2: usize = 72;
/// First transition-digest word.
pub(super) const OFFLINE_CASH_STATE_TRANSITION_WORD_START_V2: usize = 80;
/// First amount word.
pub(super) const OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2: usize = 88;
/// Scale word.
pub(super) const OFFLINE_CASH_STATE_SCALE_WORD_V2: usize = 92;
/// First aggregate recursive-parent-lineage word.
pub(super) const OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2: usize = 93;

/// Shared instance-query policy: direct scalars, never queried commitments.
pub(super) const OFFLINE_CASH_STATE_INSTANCE_QUERY_V2: PastaIpaInstanceQueryV1 =
    PastaIpaInstanceQueryV1::Direct;
/// Associated prover/verifier contract implied by the shared Direct policy.
pub(super) const OFFLINE_CASH_STATE_QUERY_INSTANCE_V2: bool =
    match OFFLINE_CASH_STATE_INSTANCE_QUERY_V2 {
        PastaIpaInstanceQueryV1::Direct => false,
        PastaIpaInstanceQueryV1::Queried => true,
    };
/// A current proof accumulator is never a current-proof public instance.
pub(super) const OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2: bool = false;
/// No public ABI words are allocated to the current proof accumulator.
pub(super) const OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_PUBLIC_WORDS_V2: u32 = 0;
/// The current proof accumulator is excluded from every semantic digest field.
pub(super) const OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_DIGESTS_V2: bool = false;
/// Current proof bytes are excluded from every semantic digest field.
pub(super) const OFFLINE_CASH_STATE_CURRENT_PROOF_BYTES_IN_DIGESTS_V2: bool = false;
/// The public lineage is finalized before the current transcript starts.
pub(super) const OFFLINE_CASH_STATE_PARENT_LINEAGE_PRECEDES_CURRENT_TRANSCRIPT_V2: bool = true;
/// Post-proof accumulation is successor state, not current payment input.
pub(super) const OFFLINE_CASH_STATE_POST_PROOF_FOLD_IN_PAYMENT_V2: bool = false;

const _: () = assert!(LINEAGE_BYTES == LINEAGE_ROUNDS * SCALAR_BYTES + POINT_BYTES);
const _: () = assert!(LINEAGE_BYTES % 4 == 0);
const _: () =
    assert!(OFFLINE_CASH_STATE_DIGEST_WORDS_V2 * OFFLINE_CASH_STATE_DIGEST_FIELDS_V2 == 80);
const _: () =
    assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2 + LINEAGE_WORDS == STATE_ABI_WORDS);
const _: () = assert!(STATE_INSTANCE_CELLS == STATE_ABI_WORDS.div_ceil(STATE_WORDS_PER_INSTANCE));
const _: () = assert!(
    STATE_INSTANCE_CELLS * STATE_WORDS_PER_INSTANCE - STATE_ABI_WORDS
        == OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2 as usize
);
const _: () = assert!(matches!(
    OFFLINE_CASH_STATE_INSTANCE_QUERY_V2,
    PastaIpaInstanceQueryV1::Direct
));
const _: () = assert!(!OFFLINE_CASH_STATE_QUERY_INSTANCE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2);
const _: () = assert!(OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_PUBLIC_WORDS_V2 == 0);
const _: () = assert!(!OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_DIGESTS_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_CURRENT_PROOF_BYTES_IN_DIGESTS_V2);
const _: () = assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_PRECEDES_CURRENT_TRANSCRIPT_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_POST_PROOF_FOLD_IN_PAYMENT_V2);

/// Canonical parent-lineage decoding failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashParentLineageCodecErrorV2 {
    /// The byte string is not exactly 576 bytes.
    InvalidLength {
        /// Supplied byte length.
        actual: usize,
    },
    /// One of the 17 scalar encodings is not canonical in the selected field.
    NonCanonicalRoundChallenge {
        /// Zero-based round index.
        index: usize,
    },
    /// The folded-generator encoding is invalid or non-canonical.
    InvalidFoldedGenerator,
    /// A live parent lineage used the identity point.
    IdentityFoldedGenerator,
    /// Typed live values collided with the reserved all-zero bootstrap encoding.
    ReservedBootstrapEncoding,
}

impl fmt::Display for OfflineCashParentLineageCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual } => write!(
                formatter,
                "offline-cash V2 parent lineage has {actual} bytes instead of {LINEAGE_BYTES}"
            ),
            Self::NonCanonicalRoundChallenge { index } => write!(
                formatter,
                "offline-cash V2 parent-lineage round challenge {index} is non-canonical"
            ),
            Self::InvalidFoldedGenerator => formatter
                .write_str("offline-cash V2 parent-lineage folded generator is non-canonical"),
            Self::IdentityFoldedGenerator => {
                formatter.write_str("offline-cash V2 live parent lineage uses the identity point")
            }
            Self::ReservedBootstrapEncoding => formatter.write_str(
                "offline-cash V2 live parent lineage uses the reserved bootstrap encoding",
            ),
        }
    }
}

impl std::error::Error for OfflineCashParentLineageCodecErrorV2 {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OfflineCashLiveParentLineageV2<C: CurveAffine> {
    round_challenges: [C::Scalar; LINEAGE_ROUNDS],
    folded_generator: C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OfflineCashParentLineageV2<C: CurveAffine> {
    Bootstrap,
    Live(OfflineCashLiveParentLineageV2<C>),
}

/// Parity-safe Eq/Fp aggregate parent lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashEqParentLineageV2(OfflineCashParentLineageV2<EqAffine>);

/// Parity-safe Ep/Fq aggregate parent lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashEpParentLineageV2(OfflineCashParentLineageV2<EpAffine>);

impl OfflineCashEqParentLineageV2 {
    /// Construct the reserved all-zero initialization sentinel.
    pub(super) const fn bootstrap() -> Self {
        Self(OfflineCashParentLineageV2::Bootstrap)
    }

    /// Decode the sentinel or one canonical live Eq lineage without authorizing it.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, OfflineCashParentLineageCodecErrorV2> {
        decode_parent_lineage::<EqAffine>(bytes).map(Self)
    }

    /// Construct a live Eq lineage from typed canonical curve values.
    pub(super) fn live(
        round_challenges: [<EqAffine as CurveAffine>::Scalar; LINEAGE_ROUNDS],
        folded_generator: EqAffine,
    ) -> Result<Self, OfflineCashParentLineageCodecErrorV2> {
        live_parent_lineage(round_challenges, folded_generator).map(Self)
    }

    /// Encode to the unique 576-byte representation.
    pub(super) fn encode(&self) -> [u8; LINEAGE_BYTES] {
        encode_parent_lineage(&self.0)
    }

    /// Whether this is the reserved all-zero bootstrap sentinel.
    pub(super) const fn is_bootstrap(&self) -> bool {
        matches!(&self.0, OfflineCashParentLineageV2::Bootstrap)
    }
}

impl OfflineCashEpParentLineageV2 {
    /// Construct the reserved all-zero initialization sentinel.
    pub(super) const fn bootstrap() -> Self {
        Self(OfflineCashParentLineageV2::Bootstrap)
    }

    /// Decode the sentinel or one canonical live Ep lineage without authorizing it.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, OfflineCashParentLineageCodecErrorV2> {
        decode_parent_lineage::<EpAffine>(bytes).map(Self)
    }

    /// Construct a live Ep lineage from typed canonical curve values.
    pub(super) fn live(
        round_challenges: [<EpAffine as CurveAffine>::Scalar; LINEAGE_ROUNDS],
        folded_generator: EpAffine,
    ) -> Result<Self, OfflineCashParentLineageCodecErrorV2> {
        live_parent_lineage(round_challenges, folded_generator).map(Self)
    }

    /// Encode to the unique 576-byte representation.
    pub(super) fn encode(&self) -> [u8; LINEAGE_BYTES] {
        encode_parent_lineage(&self.0)
    }

    /// Whether this is the reserved all-zero bootstrap sentinel.
    pub(super) const fn is_bootstrap(&self) -> bool {
        matches!(&self.0, OfflineCashParentLineageV2::Bootstrap)
    }
}

fn live_parent_lineage<C>(
    round_challenges: [C::Scalar; LINEAGE_ROUNDS],
    folded_generator: C,
) -> Result<OfflineCashParentLineageV2<C>, OfflineCashParentLineageCodecErrorV2>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    if bool::from(folded_generator.is_identity()) {
        return Err(OfflineCashParentLineageCodecErrorV2::IdentityFoldedGenerator);
    }
    let lineage = OfflineCashParentLineageV2::Live(OfflineCashLiveParentLineageV2 {
        round_challenges,
        folded_generator,
    });
    if encode_parent_lineage(&lineage)
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(OfflineCashParentLineageCodecErrorV2::ReservedBootstrapEncoding);
    }
    Ok(lineage)
}

fn decode_parent_lineage<C>(
    bytes: &[u8],
) -> Result<OfflineCashParentLineageV2<C>, OfflineCashParentLineageCodecErrorV2>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    if bytes.len() != LINEAGE_BYTES {
        return Err(OfflineCashParentLineageCodecErrorV2::InvalidLength {
            actual: bytes.len(),
        });
    }
    if bytes.iter().all(|byte| *byte == 0) {
        return Ok(OfflineCashParentLineageV2::Bootstrap);
    }

    let round_challenges = bytes[..LINEAGE_ROUNDS * SCALAR_BYTES]
        .chunks_exact(SCALAR_BYTES)
        .enumerate()
        .map(|(index, scalar)| parse_scalar::<C::Scalar>(scalar, index))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| OfflineCashParentLineageCodecErrorV2::InvalidLength {
            actual: bytes.len(),
        })?;
    let folded_generator = parse_point::<C>(&bytes[LINEAGE_ROUNDS * SCALAR_BYTES..])?;
    live_parent_lineage(round_challenges, folded_generator)
}

fn parse_scalar<F: PrimeField>(
    bytes: &[u8],
    index: usize,
) -> Result<F, OfflineCashParentLineageCodecErrorV2> {
    let mut repr = F::Repr::default();
    if repr.as_ref().len() != SCALAR_BYTES || bytes.len() != SCALAR_BYTES {
        return Err(OfflineCashParentLineageCodecErrorV2::NonCanonicalRoundChallenge { index });
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr))
        .ok_or(OfflineCashParentLineageCodecErrorV2::NonCanonicalRoundChallenge { index })
}

fn parse_point<C: CurveAffine>(bytes: &[u8]) -> Result<C, OfflineCashParentLineageCodecErrorV2> {
    let mut repr = C::Repr::default();
    if repr.as_ref().len() != POINT_BYTES || bytes.len() != POINT_BYTES {
        return Err(OfflineCashParentLineageCodecErrorV2::InvalidFoldedGenerator);
    }
    repr.as_mut().copy_from_slice(bytes);
    let point = Option::<C>::from(C::from_bytes(&repr))
        .ok_or(OfflineCashParentLineageCodecErrorV2::InvalidFoldedGenerator)?;
    if point.to_bytes().as_ref() != bytes {
        return Err(OfflineCashParentLineageCodecErrorV2::InvalidFoldedGenerator);
    }
    Ok(point)
}

fn encode_parent_lineage<C>(lineage: &OfflineCashParentLineageV2<C>) -> [u8; LINEAGE_BYTES]
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    let mut bytes = [0_u8; LINEAGE_BYTES];
    let OfflineCashParentLineageV2::Live(lineage) = lineage else {
        return bytes;
    };
    for (index, scalar) in lineage.round_challenges.iter().enumerate() {
        bytes[index * SCALAR_BYTES..(index + 1) * SCALAR_BYTES]
            .copy_from_slice(scalar.to_repr().as_ref());
    }
    bytes[LINEAGE_ROUNDS * SCALAR_BYTES..]
        .copy_from_slice(lineage.folded_generator.to_bytes().as_ref());
    bytes
}

/// STATE operation encoded at ABI word four.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum OfflineCashStateOperationV2 {
    /// Split a sender balance into a remainder and receiver-bound credit.
    SendSplit = 1,
    /// Fold one verified receiver credit into a receiver balance.
    ReceiveFold = 2,
}

/// Uninhabited authorization for the reserved all-zero bootstrap lineage.
///
/// A future authenticated release/bootstrap verifier may replace this marker
/// with a move-only token. It is uninhabited while V2 release types and
/// bootstrap protocol identity are unavailable, so this scaffold cannot admit
/// the zero sentinel into public instances.
#[derive(Debug)]
pub(super) enum OfflineCashAuthenticatedBootstrapModeV2 {}

/// Exact non-lineage values accepted by the source-only STATE ABI builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashStateAbiFieldsV2 {
    /// Selected STATE operation.
    pub(super) operation: OfflineCashStateOperationV2,
    /// Authenticated release digest.
    pub(super) release_digest: [u8; 32],
    /// Parity-specific direct-instance compiled-protocol digest.
    pub(super) protocol_digest: [u8; 32],
    /// Pre-transcript cross-parity semantic digest; excludes proof/accumulator bytes.
    pub(super) semantic_digest: [u8; 32],
    /// State-context digest.
    pub(super) context_digest: [u8; 32],
    /// Payment-request digest.
    pub(super) request_digest: [u8; 32],
    /// First semantic parent digest.
    pub(super) parent_0: [u8; 32],
    /// Second semantic parent digest.
    pub(super) parent_1: [u8; 32],
    /// Resulting state-head digest.
    pub(super) result: [u8; 32],
    /// Operation-specific link digest.
    pub(super) link: [u8; 32],
    /// Transition digest.
    pub(super) transition_digest: [u8; 32],
    /// Positive transferred amount.
    pub(super) amount: u128,
    /// Fixed-point scale.
    pub(super) scale: u32,
}

/// Host-side rejection while constructing the exact V2 STATE public ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashStateAbiErrorV2 {
    /// A digest, amount, scale, header, or operation was structurally invalid.
    InvalidLayout,
    /// A parent-lineage byte string was not canonical for its header parity.
    InvalidParentLineage,
    /// The all-zero sentinel was presented without authenticated bootstrap mode.
    UnauthenticatedBootstrap,
    /// A parity-specific accessor was used for the other parity.
    ParityMismatch,
    /// Packed cells had the wrong count or nonzero final padding.
    NonCanonicalPacking,
}

impl fmt::Display for OfflineCashStateAbiErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLayout => "invalid offline-cash V2 STATE instance layout",
            Self::InvalidParentLineage => "invalid offline-cash V2 STATE parent lineage",
            Self::UnauthenticatedBootstrap => {
                "offline-cash V2 STATE bootstrap authorization is unavailable"
            }
            Self::ParityMismatch => "offline-cash V2 STATE parity mismatch",
            Self::NonCanonicalPacking => "non-canonical offline-cash V2 STATE instance packing",
        })
    }
}

impl std::error::Error for OfflineCashStateAbiErrorV2 {}

/// Exact 237-word field-neutral STATE public instances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashStatePublicInstancesV2 {
    parity: OfflineCashHalo2ParityV2,
    words: [u32; STATE_ABI_WORDS],
}

impl OfflineCashStatePublicInstancesV2 {
    /// Build Eq/Fp instances with one prior Eq aggregate lineage.
    pub(super) fn eq(
        fields: OfflineCashStateAbiFieldsV2,
        parent_lineage: &OfflineCashEqParentLineageV2,
    ) -> Result<Self, OfflineCashStateAbiErrorV2> {
        if parent_lineage.is_bootstrap() {
            return Err(OfflineCashStateAbiErrorV2::UnauthenticatedBootstrap);
        }
        Self::build(
            OfflineCashHalo2ParityV2::Eq,
            fields,
            parent_lineage.encode(),
        )
    }

    /// Build Ep/Fq instances with one prior Ep aggregate lineage.
    pub(super) fn ep(
        fields: OfflineCashStateAbiFieldsV2,
        parent_lineage: &OfflineCashEpParentLineageV2,
    ) -> Result<Self, OfflineCashStateAbiErrorV2> {
        if parent_lineage.is_bootstrap() {
            return Err(OfflineCashStateAbiErrorV2::UnauthenticatedBootstrap);
        }
        Self::build(
            OfflineCashHalo2ParityV2::Ep,
            fields,
            parent_lineage.encode(),
        )
    }

    /// Reserved Eq bootstrap constructor; uncallable until authentication exists.
    pub(super) fn eq_authenticated_bootstrap(
        _fields: OfflineCashStateAbiFieldsV2,
        authorization: OfflineCashAuthenticatedBootstrapModeV2,
    ) -> Result<Self, OfflineCashStateAbiErrorV2> {
        match authorization {}
    }

    /// Reserved Ep bootstrap constructor; uncallable until authentication exists.
    pub(super) fn ep_authenticated_bootstrap(
        _fields: OfflineCashStateAbiFieldsV2,
        authorization: OfflineCashAuthenticatedBootstrapModeV2,
    ) -> Result<Self, OfflineCashStateAbiErrorV2> {
        match authorization {}
    }

    fn build(
        parity: OfflineCashHalo2ParityV2,
        fields: OfflineCashStateAbiFieldsV2,
        parent_lineage: [u8; LINEAGE_BYTES],
    ) -> Result<Self, OfflineCashStateAbiErrorV2> {
        if fields.amount == 0
            || fields.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || [
                fields.release_digest,
                fields.protocol_digest,
                fields.semantic_digest,
                fields.context_digest,
                fields.request_digest,
                fields.parent_0,
                fields.parent_1,
                fields.result,
                fields.link,
                fields.transition_digest,
            ]
            .iter()
            .any(|digest| *digest == [0; 32])
        {
            return Err(OfflineCashStateAbiErrorV2::InvalidLayout);
        }

        let mut words = [0_u32; STATE_ABI_WORDS];
        words[..8].copy_from_slice(&[
            OFFLINE_CASH_STATE_ABI_VERSION_V2,
            OFFLINE_CASH_STATE_WIRE_VERSION_V2,
            OFFLINE_CASH_HALO2_K_V2,
            parity as u32,
            fields.operation as u32,
            OFFLINE_CASH_STATE_FIXED_SEMANTIC_PARENT_COUNT_V2,
            OFFLINE_CASH_STATE_DIGEST_WORDS_V2 as u32,
            LINEAGE_WORDS as u32,
        ]);
        for (offset, digest) in [
            (
                OFFLINE_CASH_STATE_RELEASE_WORD_START_V2,
                fields.release_digest,
            ),
            (
                OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2,
                fields.protocol_digest,
            ),
            (
                OFFLINE_CASH_STATE_SEMANTIC_WORD_START_V2,
                fields.semantic_digest,
            ),
            (
                OFFLINE_CASH_STATE_CONTEXT_WORD_START_V2,
                fields.context_digest,
            ),
            (
                OFFLINE_CASH_STATE_REQUEST_WORD_START_V2,
                fields.request_digest,
            ),
            (OFFLINE_CASH_STATE_PARENT_0_WORD_START_V2, fields.parent_0),
            (OFFLINE_CASH_STATE_PARENT_1_WORD_START_V2, fields.parent_1),
            (OFFLINE_CASH_STATE_RESULT_WORD_START_V2, fields.result),
            (OFFLINE_CASH_STATE_LINK_WORD_START_V2, fields.link),
            (
                OFFLINE_CASH_STATE_TRANSITION_WORD_START_V2,
                fields.transition_digest,
            ),
        ] {
            write_digest_words(&mut words, offset, digest);
        }
        for (target, chunk) in words
            [OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2..OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2 + 4]
            .iter_mut()
            .zip(fields.amount.to_le_bytes().chunks_exact(4))
        {
            *target = u32::from_le_bytes(chunk.try_into().expect("four-byte amount limb"));
        }
        words[OFFLINE_CASH_STATE_SCALE_WORD_V2] = fields.scale;
        for (target, chunk) in words[OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2..]
            .iter_mut()
            .zip(parent_lineage.chunks_exact(4))
        {
            *target = u32::from_le_bytes(chunk.try_into().expect("four-byte lineage limb"));
        }

        let instances = Self { parity, words };
        instances.validate_structure()?;
        Ok(instances)
    }

    /// Selected Pasta parity.
    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV2 {
        self.parity
    }

    /// Exact semantic words in canonical transcript order.
    pub(super) const fn words(&self) -> &[u32; STATE_ABI_WORDS] {
        &self.words
    }

    /// Recover and validate the aggregate Eq parent lineage.
    pub(super) fn eq_parent_lineage(
        &self,
    ) -> Result<OfflineCashEqParentLineageV2, OfflineCashStateAbiErrorV2> {
        if self.parity != OfflineCashHalo2ParityV2::Eq {
            return Err(OfflineCashStateAbiErrorV2::ParityMismatch);
        }
        OfflineCashEqParentLineageV2::decode(&self.parent_lineage_bytes())
            .map_err(|_| OfflineCashStateAbiErrorV2::InvalidParentLineage)
    }

    /// Recover and validate the aggregate Ep parent lineage.
    pub(super) fn ep_parent_lineage(
        &self,
    ) -> Result<OfflineCashEpParentLineageV2, OfflineCashStateAbiErrorV2> {
        if self.parity != OfflineCashHalo2ParityV2::Ep {
            return Err(OfflineCashStateAbiErrorV2::ParityMismatch);
        }
        OfflineCashEpParentLineageV2::decode(&self.parent_lineage_bytes())
            .map_err(|_| OfflineCashStateAbiErrorV2::InvalidParentLineage)
    }

    /// Canonical 28-byte cells before conversion into the selected Pasta field.
    pub(super) fn packed_cell_bytes(&self) -> [[u8; PACKED_CELL_BYTES]; STATE_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let mut bytes = [0_u8; PACKED_CELL_BYTES];
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = start
                .saturating_add(STATE_WORDS_PER_INSTANCE)
                .min(STATE_ABI_WORDS);
            for (lane, word) in self.words[start..end].iter().enumerate() {
                bytes[lane * 4..lane * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            bytes
        })
    }

    /// Strictly unpack canonical cells and reject nonzero final padding.
    pub(super) fn unpack_cell_bytes(
        cells: &[[u8; PACKED_CELL_BYTES]],
    ) -> Result<[u32; STATE_ABI_WORDS], OfflineCashStateAbiErrorV2> {
        if cells.len() != STATE_INSTANCE_CELLS
            || cells[STATE_INSTANCE_CELLS - 1][PACKED_CELL_BYTES
                - OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2 as usize * 4..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(OfflineCashStateAbiErrorV2::NonCanonicalPacking);
        }
        let mut words = [0_u32; STATE_ABI_WORDS];
        for (index, word) in words.iter_mut().enumerate() {
            let cell = &cells[index / STATE_WORDS_PER_INSTANCE];
            let offset = index % STATE_WORDS_PER_INSTANCE * 4;
            *word = u32::from_le_bytes(
                cell[offset..offset + 4]
                    .try_into()
                    .expect("four-byte packed word"),
            );
        }
        Ok(words)
    }

    fn parent_lineage_bytes(&self) -> [u8; LINEAGE_BYTES] {
        let mut bytes = [0_u8; LINEAGE_BYTES];
        for (chunk, word) in bytes
            .chunks_exact_mut(4)
            .zip(&self.words[OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2..])
        {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    fn validate_structure(&self) -> Result<(), OfflineCashStateAbiErrorV2> {
        if self.words[..8]
            != [
                OFFLINE_CASH_STATE_ABI_VERSION_V2,
                OFFLINE_CASH_STATE_WIRE_VERSION_V2,
                OFFLINE_CASH_HALO2_K_V2,
                self.parity as u32,
                self.words[4],
                OFFLINE_CASH_STATE_FIXED_SEMANTIC_PARENT_COUNT_V2,
                OFFLINE_CASH_STATE_DIGEST_WORDS_V2 as u32,
                LINEAGE_WORDS as u32,
            ]
            || !matches!(
                self.words[4],
                value if value == OfflineCashStateOperationV2::SendSplit as u32
                    || value == OfflineCashStateOperationV2::ReceiveFold as u32
            )
            || self.words[OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2
                ..OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2 + 4]
                .iter()
                .all(|word| *word == 0)
            || self.words[OFFLINE_CASH_STATE_SCALE_WORD_V2] > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
        {
            return Err(OfflineCashStateAbiErrorV2::InvalidLayout);
        }
        for offset in [
            OFFLINE_CASH_STATE_RELEASE_WORD_START_V2,
            OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2,
            OFFLINE_CASH_STATE_SEMANTIC_WORD_START_V2,
            OFFLINE_CASH_STATE_CONTEXT_WORD_START_V2,
            OFFLINE_CASH_STATE_REQUEST_WORD_START_V2,
            OFFLINE_CASH_STATE_PARENT_0_WORD_START_V2,
            OFFLINE_CASH_STATE_PARENT_1_WORD_START_V2,
            OFFLINE_CASH_STATE_RESULT_WORD_START_V2,
            OFFLINE_CASH_STATE_LINK_WORD_START_V2,
            OFFLINE_CASH_STATE_TRANSITION_WORD_START_V2,
        ] {
            if self.words[offset..offset + OFFLINE_CASH_STATE_DIGEST_WORDS_V2]
                .iter()
                .all(|word| *word == 0)
            {
                return Err(OfflineCashStateAbiErrorV2::InvalidLayout);
            }
        }
        match self.parity {
            OfflineCashHalo2ParityV2::Eq => self.eq_parent_lineage().map(|_| ()),
            OfflineCashHalo2ParityV2::Ep => self.ep_parent_lineage().map(|_| ()),
        }
    }
}

fn write_digest_words(words: &mut [u32; STATE_ABI_WORDS], offset: usize, digest: [u8; 32]) {
    for (target, chunk) in words[offset..offset + OFFLINE_CASH_STATE_DIGEST_WORDS_V2]
        .iter_mut()
        .zip(digest.chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte digest limb"));
    }
}

/// Canonical child order used to aggregate one STATE predecessor lineage.
///
/// Each selected child contributes its proof-derived current accumulator plus
/// its already-public predecessor lineage. A future recursive implementation
/// must verify those claims and fold them in this order before exposing the
/// aggregate as the outer STATE proof's `parent_lineage` tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashStateLineageChildRoleV2 {
    /// First semantic STATE parent.
    StateParent0 = 1,
    /// Second semantic STATE parent.
    StateParent1 = 2,
    /// GuardBundle child after its own fixed helper aggregation.
    GuardBundle = 3,
}

/// Fixed STATE predecessor-lineage fold order.
pub(super) const OFFLINE_CASH_STATE_LINEAGE_CHILD_ORDER_V2: [OfflineCashStateLineageChildRoleV2;
    3] = [
    OfflineCashStateLineageChildRoleV2::StateParent0,
    OfflineCashStateLineageChildRoleV2::StateParent1,
    OfflineCashStateLineageChildRoleV2::GuardBundle,
];

/// Canonical helper order inside a GuardBundle predecessor lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashGuardBundleLineageChildRoleV2 {
    /// Guard-use relation.
    GuardUse = 1,
    /// Platform-binding relation.
    PlatformBind = 2,
    /// Optional Android key-certificate relation.
    AndroidKeyCert = 3,
    /// Optional P-256 signature relation.
    P256Signature = 4,
}

/// Fixed GuardBundle helper-lineage fold order; absent optional roles are gated.
pub(super) const OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2:
    [OfflineCashGuardBundleLineageChildRoleV2; 4] = [
    OfflineCashGuardBundleLineageChildRoleV2::GuardUse,
    OfflineCashGuardBundleLineageChildRoleV2::PlatformBind,
    OfflineCashGuardBundleLineageChildRoleV2::AndroidKeyCert,
    OfflineCashGuardBundleLineageChildRoleV2::P256Signature,
];

/// Acyclic value-production order committed by the V2 direct transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum OfflineCashStateTranscriptStageV2 {
    /// Prior proofs and folds finalize one aggregate predecessor lineage.
    ParentLineageFinalized = 1,
    /// Direct public scalars, including that prior lineage, enter the transcript.
    PublicInstancesAbsorbed = 2,
    /// Current proof commitments, evaluations, and IPA rounds are read.
    CurrentProofRead = 3,
    /// The current accumulator is derived from the completed transcript.
    CurrentAccumulatorDerived = 4,
    /// An optional post-proof fold creates successor-only lineage state.
    SuccessorLineageProduced = 5,
    /// Only a future proof may expose that successor lineage.
    FuturePublicInstances = 6,
}

/// Exact acyclic transcript dependency order.
pub(super) const OFFLINE_CASH_STATE_TRANSCRIPT_ORDER_V2: [OfflineCashStateTranscriptStageV2; 6] = [
    OfflineCashStateTranscriptStageV2::ParentLineageFinalized,
    OfflineCashStateTranscriptStageV2::PublicInstancesAbsorbed,
    OfflineCashStateTranscriptStageV2::CurrentProofRead,
    OfflineCashStateTranscriptStageV2::CurrentAccumulatorDerived,
    OfflineCashStateTranscriptStageV2::SuccessorLineageProduced,
    OfflineCashStateTranscriptStageV2::FuturePublicInstances,
];

/// Required fail-closed terminal sequencing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum OfflineCashStateTerminalStageV2 {
    /// Bound and canonically decode all frames.
    CanonicalWireDecode = 1,
    /// Validate request liveness and semantic/context relations.
    StatementAndLiveness = 2,
    /// Authenticate parameters, keys, releases, and exact direct protocols.
    ArtifactAndProtocolAuthentication = 3,
    /// Reconstruct complete typed Eq and Ep public instances.
    ReconstructPublicInstances = 4,
    /// Verify the current Eq proof and derive its accumulator.
    EqCurrentProof = 5,
    /// Verify the current Ep proof and derive its accumulator.
    EpCurrentProof = 6,
    /// Decide the current Eq accumulator.
    EqCurrentDecision = 7,
    /// Decide the current Ep accumulator.
    EpCurrentDecision = 8,
    /// Decide the live Eq predecessor lineage, if not bootstrap.
    EqParentLineageDecision = 9,
    /// Decide the live Ep predecessor lineage, if not bootstrap.
    EpParentLineageDecision = 10,
    /// Atomically persist completed successor lineages.
    PersistSuccessorLineages = 11,
    /// Issue a receipt only after every prior stage succeeded.
    IssueReceipt = 12,
}

/// Exact terminal order; a production implementation must not skip or reorder it.
pub(super) const OFFLINE_CASH_STATE_TERMINAL_ORDER_V2: [OfflineCashStateTerminalStageV2; 12] = [
    OfflineCashStateTerminalStageV2::CanonicalWireDecode,
    OfflineCashStateTerminalStageV2::StatementAndLiveness,
    OfflineCashStateTerminalStageV2::ArtifactAndProtocolAuthentication,
    OfflineCashStateTerminalStageV2::ReconstructPublicInstances,
    OfflineCashStateTerminalStageV2::EqCurrentProof,
    OfflineCashStateTerminalStageV2::EpCurrentProof,
    OfflineCashStateTerminalStageV2::EqCurrentDecision,
    OfflineCashStateTerminalStageV2::EpCurrentDecision,
    OfflineCashStateTerminalStageV2::EqParentLineageDecision,
    OfflineCashStateTerminalStageV2::EpParentLineageDecision,
    OfflineCashStateTerminalStageV2::PersistSuccessorLineages,
    OfflineCashStateTerminalStageV2::IssueReceipt,
];

/// Uninhabited marker: no recursive fold implementation exists in this scaffold.
#[derive(Debug)]
pub(super) enum OfflineCashStateRecursiveFoldImplementationV2 {}

/// Uninhabited verified receipt; only a future production terminal may create one.
#[derive(Debug)]
pub(super) enum OfflineCashStateVerifiedReceiptV2 {}

/// Fail-closed STATE terminal error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashStateTerminalErrorV2 {
    /// No production direct verifier, recursive fold, or receipt path exists.
    VerificationUnavailable,
}

impl fmt::Display for OfflineCashStateTerminalErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("offline-cash V2 STATE verification is unavailable")
    }
}

impl std::error::Error for OfflineCashStateTerminalErrorV2 {}

/// Fail closed at the receipt boundary until the complete terminal is implemented.
pub(super) const fn fail_closed_offline_cash_state_terminal_v2()
-> Result<OfflineCashStateVerifiedReceiptV2, OfflineCashStateTerminalErrorV2> {
    Err(OfflineCashStateTerminalErrorV2::VerificationUnavailable)
}
