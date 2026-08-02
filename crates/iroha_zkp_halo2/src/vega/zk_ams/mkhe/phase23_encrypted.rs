//! Canonical encrypted evaluation for the ZK-AMS Phase-II/III fold.
//!
//! This module owns the bounded CSR representation of the public `A`, `B`,
//! `C`, and module-commitment maps and the compact packed-ciphertext evaluator
//! shared by Equations (6), (7), and (9)--(11).  The evaluator never decodes a
//! ciphertext and never substitutes a plaintext calculation.  Plaintext
//! calculations appear only in tests as an independent oracle.
//!
//! The frozen release certificate deliberately remains open.  A small-profile
//! KAT exercises the complete native path, but it is not evidence for the
//! release degree, roster, memory ceiling, or wall-clock budget.

use std::collections::BTreeMap;

use once_cell::sync::Lazy;

#[cfg(test)]
use super::phase23::{
    ZkAmsPhase23ChallengeContextV1, zk_ams_phase23_challenge_v1, zk_ams_phase23_fold_linear_v1,
    zk_ams_phase23_fold_quadratic_v1,
};
use super::{
    BgvProfile, GaloisKey, LinearCiphertext, MKHE_VERSION_V1, PartySet, PlaintextModulus,
    ProductRelinearizationKey, RnsPolynomial, Scalar, ZkAmsMkheErrorV1,
    checked_ring_multiplication_work, hash_linear_ciphertext, keccak256,
    manifest::{
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1, release_profile_v1,
    },
    packing::{
        ZkAmsT256PackedPlaintextV1, ZkAmsT256RotationDirectionV1,
        decode_zk_ams_t256_packed_plaintext_v1, encode_zk_ams_t256_packed_plaintext_v1,
        packed_plaintext_to_rns_v1, zk_ams_t256_packing_layout_v1,
        zk_ams_t256_rotation_exponent_for_direction_v1, zk_ams_t256_rotation_exponent_v1,
    },
    phase23_rotation_ring_multiplication_count, relinearize, rotate_ciphertext,
};
use crate::vega::{
    VegaT256PointV1,
    commitment::{Commitment, CommitmentKey},
    masked_relaxed::{
        MASKED_RELAXED_COMMITMENT_COLUMNS_V1, masked_relaxed_composition_transcript_v1,
    },
    nifs::NovaNifs,
    r1cs::{Instance, RelaxedInstance, Shape, SparseMatrix},
    sponge::Keccak256,
};

const PHASE23_ENCRYPTED_VERSION_V1: u8 = 1;
const PHASE23_MAX_BATCH_SIZE_V1: u8 = 8;
const PHASE23_MAX_ROWS_V1: u32 = 1_048_576;
const PHASE23_MAX_COLUMNS_V1: u32 = 1_048_576;
const PHASE23_MAX_ACCUMULATOR_VALUES_V1: usize = 4_194_304;
const PHASE23_MAX_DIAGONALS_V1: usize = 8_388_608;
const PHASE23_MAX_COMPOSITION_CONTEXT_FRAME_BYTES_V1: usize = 2_048;
/// Exact strict public-input scalar count retained in every release fold.
pub const ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1: usize = 89;
/// Exact Hyrax point count in one release witness commitment.
pub const ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1: usize = 512;
/// Exact Hyrax point count in one release error or cross-term commitment.
pub const ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1: usize = 1_024;
const PHASE23_SPARSE_MAP_WIRE_HEADER_BYTES_V1: usize = 18;
const PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1: usize = 186;
const PHASE23_SPARSE_MAP_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.sparse-map";
const PHASE23_ENCRYPTED_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.encrypted-binding";
const PHASE23_PACKED_VECTOR_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.packed-vector";
const PHASE23_MATERIALIZED_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.materialized";
const PHASE23_IMPLEMENTATION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.encrypted-implementation";
const PHASE23_RELEASE_MAP_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.release-map-set";
const PHASE23_COMMITMENT_PREIMAGE_LAYOUT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.hyrax-commitment-preimage-layout";
const PHASE23_COMMITMENT_G_MAP_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.hyrax-commitment-g-map";
const PHASE23_COMMITMENT_H_MAP_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.hyrax-commitment-h-map";
const PHASE23_PAPER_COLUMN_ORDER_V1: &[u8] = b"paper-columns:[W,x,u]|internal-columns:[W,u,x]";
const PHASE23_PUBLIC_INPUT_VECTOR_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.strict-public-input-vector";
const PHASE23_STRICT_INSTANCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.strict-public-instance";
const PHASE23_PUBLIC_ACCUMULATOR_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.public-relaxed-accumulator";
const PHASE23_WITNESS_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.hyrax-witness-commitment";
const PHASE23_ERROR_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.hyrax-error-commitment";
const PHASE23_CROSS_TERM_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.hyrax-cross-term-commitment";
const PHASE23_COMPOSITION_CONTEXT_FRAME_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.core-composition-context-frame";
const PHASE23_PUBLIC_FOLD_RECORD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.public-fold-record";
const PHASE23_PUBLIC_FOLD_HISTORY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.public-fold-history";
const PHASE23_ENCRYPTED_ALGEBRA_V1: &[u8] = b"A/B/C:canonical-csr:packed-diagonals:minimal-signed-binary-galois-composition|U:row-count-replicated-single-ciphertext-clones|Eq6:direct-replicated-U-mul+relinearize-four-terms|Eq7:G*rT+H*T|Eq9-10:x,u,W,rW=linear;E=quadratic;rE=quadratic-scalars|Eq11:Ebar=linear+quadratic;Wbar=linear";

static PHASE23_RELEASE_MAPS_V1: Lazy<Result<ZkAmsPhase23ReleaseMapsV1, ZkAmsMkheErrorV1>> =
    Lazy::new(compile_release_maps_v1);

/// Maximum number of nonzero entries admitted by one canonical sparse map.
///
/// This is an allocation ceiling, not a claim about the compiled relation.
/// The exact encrypted work ceiling is checked again from the number of unique
/// packed diagonals before any ciphertext output is allocated.
pub const ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1: u32 = 8_388_608;
/// Deterministic release KAT digest of canonical A/B/C and the Hyrax G/H
/// commitment-preimage layout.
pub const ZK_AMS_PHASE23_RELEASE_MAP_SET_KAT_DIGEST_V1: [u8; 32] = [
    50, 223, 102, 25, 53, 242, 201, 62, 35, 225, 87, 33, 238, 138, 181, 190, 252, 179, 254, 190,
    130, 190, 137, 137, 72, 81, 106, 187, 199, 149, 22, 169,
];

/// Domain tag for one canonical public linear map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ZkAmsPhase23MapKindV1 {
    /// Relaxed-R1CS matrix `A`.
    A = 1,
    /// Relaxed-R1CS matrix `B`.
    B = 2,
    /// Relaxed-R1CS matrix `C`.
    C = 3,
    /// Equation (7) randomness matrix `G_T`.
    CommitmentG = 4,
    /// Equation (7) message matrix `H_T`.
    CommitmentH = 5,
}

impl ZkAmsPhase23MapKindV1 {
    fn from_tag(tag: u8) -> Result<Self, ZkAmsMkheErrorV1> {
        match tag {
            1 => Ok(Self::A),
            2 => Ok(Self::B),
            3 => Ok(Self::C),
            4 => Ok(Self::CommitmentG),
            5 => Ok(Self::CommitmentH),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }

    const fn tag(self) -> u8 {
        self as u8
    }
}

/// Canonical release layout of the Hyrax cross-term commitment preimage.
///
/// This is deliberately not a scalar sparse map and not a curve commitment.
/// `G` places one blinding scalar at the hiding-generator position of each
/// row, while `H` places each message scalar at its canonical row/generator
/// position.  Only full-roster decryption followed by the canonical Hyrax
/// commitment implementation may turn this preimage into curve points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23CommitmentPreimageLayoutV1 {
    version: u8,
    message_value_count: u32,
    row_count: u32,
    message_columns: u32,
    blinding_count: u32,
    last_row_message_count: u32,
    hiding_generator_index: u32,
    commitment_key_label_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    g_map_digest: [u8; 32],
    h_map_digest: [u8; 32],
    digest: [u8; 32],
}

impl ZkAmsPhase23CommitmentPreimageLayoutV1 {
    /// Encoding version.
    #[must_use]
    pub const fn version(self) -> u8 {
        self.version
    }

    /// Exact number of cross-term message scalars.
    #[must_use]
    pub const fn message_value_count(self) -> u32 {
        self.message_value_count
    }

    /// Exact number of Hyrax commitment rows and blinding scalars.
    #[must_use]
    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    /// Exact number of message generators per full Hyrax row.
    #[must_use]
    pub const fn message_columns(self) -> u32 {
        self.message_columns
    }

    /// Exact number of row-blinding inputs consumed by the canonical `G` map.
    #[must_use]
    pub const fn blinding_count(self) -> u32 {
        self.blinding_count
    }

    /// Exact number of message scalars in the final row.
    #[must_use]
    pub const fn last_row_message_count(self) -> u32 {
        self.last_row_message_count
    }

    /// Generator index occupied by the hiding generator in every row.
    #[must_use]
    pub const fn hiding_generator_index(self) -> u32 {
        self.hiding_generator_index
    }

    /// Digest of the exact canonical Hyrax key label.
    #[must_use]
    pub const fn commitment_key_label_digest(self) -> [u8; 32] {
        self.commitment_key_label_digest
    }

    /// Digest of all message generators plus the hiding generator.
    #[must_use]
    pub const fn generator_basis_digest(self) -> [u8; 32] {
        self.generator_basis_digest
    }

    /// Digest of the canonical `G` row/blinding incidence map.
    #[must_use]
    pub const fn g_map_digest(self) -> [u8; 32] {
        self.g_map_digest
    }

    /// Digest of the canonical `H` row/message-generator incidence map.
    #[must_use]
    pub const fn h_map_digest(self) -> [u8; 32] {
        self.h_map_digest
    }

    /// Digest binding the complete commitment-preimage layout.
    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Resolve one `G`-map input to `(row, hiding-generator index)`.
    pub fn blinding_position(self, index: u32) -> Result<(u32, u32), ZkAmsMkheErrorV1> {
        validate_commitment_preimage_layout(self)?;
        if index >= self.blinding_count {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok((index, self.hiding_generator_index))
    }

    /// Resolve one `H`-map input to `(row, message-generator index)`.
    pub fn message_position(self, index: u32) -> Result<(u32, u32), ZkAmsMkheErrorV1> {
        validate_commitment_preimage_layout(self)?;
        if index >= self.message_value_count {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok((index / self.message_columns, index % self.message_columns))
    }
}

/// Sole canonical release A/B/C maps and Hyrax G/H preimage layout.
///
/// The fields are intentionally private.  Release entrypoints obtain this
/// immutable static value and never accept caller-authoritative alternatives.
#[derive(Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23ReleaseMapsV1 {
    a: ZkAmsPhase23SparseMapV1,
    b: ZkAmsPhase23SparseMapV1,
    c: ZkAmsPhase23SparseMapV1,
    commitment_preimage_layout: ZkAmsPhase23CommitmentPreimageLayoutV1,
    digest: [u8; 32],
}

impl ZkAmsPhase23ReleaseMapsV1 {
    /// Canonical paper-order `A` map.
    #[must_use]
    pub const fn a(&self) -> &ZkAmsPhase23SparseMapV1 {
        &self.a
    }

    /// Canonical paper-order `B` map.
    #[must_use]
    pub const fn b(&self) -> &ZkAmsPhase23SparseMapV1 {
        &self.b
    }

    /// Canonical paper-order `C` map.
    #[must_use]
    pub const fn c(&self) -> &ZkAmsPhase23SparseMapV1 {
        &self.c
    }

    /// Canonical paper-order map references in exact `A`, `B`, `C` order.
    #[must_use]
    pub const fn abc(&self) -> [&ZkAmsPhase23SparseMapV1; 3] {
        [&self.a, &self.b, &self.c]
    }

    /// Canonical Hyrax `G`/`H` commitment-preimage row layout.
    #[must_use]
    pub const fn commitment_preimage_layout(&self) -> ZkAmsPhase23CommitmentPreimageLayoutV1 {
        self.commitment_preimage_layout
    }

    /// Digest binding the relation source, paper column order, A/B/C maps,
    /// canonical commitment key, and G/H preimage maps.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Return the sole deterministic release A/B/C maps and Hyrax G/H preimage
/// layout compiled from the canonical ZK-AMS relation.
pub fn zk_ams_phase23_release_maps_v1()
-> Result<&'static ZkAmsPhase23ReleaseMapsV1, ZkAmsMkheErrorV1> {
    match &*PHASE23_RELEASE_MAPS_V1 {
        Ok(maps) => Ok(maps),
        Err(error) => Err(*error),
    }
}

/// Return the digest of the sole canonical release map set.
pub fn zk_ams_phase23_release_map_set_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    Ok(zk_ams_phase23_release_maps_v1()?.digest())
}

/// Require exact structural identity with the sole canonical A/B/C release
/// maps.  Semantic equivalence or alternate CSR metadata is not admitted.
pub(super) fn require_release_relation_maps_v1(
    maps: [&ZkAmsPhase23SparseMapV1; 3],
) -> Result<(), ZkAmsMkheErrorV1> {
    for map in maps {
        validate_sparse_map(map)?;
    }
    let canonical = zk_ams_phase23_release_maps_v1()?.abc();
    if maps
        .into_iter()
        .zip(canonical)
        .any(|(provided, expected)| provided != expected)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// Canonical public relaxed instance retained for fold-history replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23PublicAccumulatorV1 {
    version: u8,
    relaxation: [u8; 32],
    public_inputs: [[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1],
    public_input_digest: [u8; 32],
    witness_commitment: Vec<[u8; 33]>,
    witness_commitment_digest: [u8; 32],
    error_commitment: Vec<[u8; 33]>,
    error_commitment_digest: [u8; 32],
    digest: [u8; 32],
}

impl ZkAmsPhase23PublicAccumulatorV1 {
    /// Construct one exact release-shape relaxed public instance.
    pub fn new(
        relaxation: [u8; 32],
        public_inputs: [[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1],
        witness_commitment: Vec<[u8; 33]>,
        error_commitment: Vec<[u8; 33]>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        scalar_from_canonical_bytes(relaxation)?;
        let public_input_digest = public_input_vector_digest(&public_inputs)?;
        let witness_commitment_digest = point_vector_digest(
            PHASE23_WITNESS_COMMITMENT_DOMAIN_V1,
            &witness_commitment,
            ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        )?;
        let error_commitment_digest = point_vector_digest(
            PHASE23_ERROR_COMMITMENT_DOMAIN_V1,
            &error_commitment,
            ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        )?;
        if [
            public_input_digest,
            witness_commitment_digest,
            error_commitment_digest,
        ]
        .contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut value = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            relaxation,
            public_inputs,
            public_input_digest,
            witness_commitment,
            witness_commitment_digest,
            error_commitment,
            error_commitment_digest,
            digest: [0; 32],
        };
        value.digest = public_accumulator_digest_from_bound_fields(&value);
        if value.digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(value)
    }

    /// Relaxation scalar `u` as exact canonical T256 bytes.
    #[must_use]
    pub const fn relaxation(&self) -> [u8; 32] {
        self.relaxation
    }

    /// Exact strict-order public input scalar bytes.
    #[must_use]
    pub const fn public_inputs(&self) -> &[[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1] {
        &self.public_inputs
    }

    /// Digest of the exact public-input vector.
    #[must_use]
    pub const fn public_input_digest(&self) -> [u8; 32] {
        self.public_input_digest
    }

    /// Canonical nonidentity witness-commitment point encodings.
    #[must_use]
    pub fn witness_commitment(&self) -> &[[u8; 33]] {
        &self.witness_commitment
    }

    /// Canonical nonidentity error-commitment point encodings.
    #[must_use]
    pub fn error_commitment(&self) -> &[[u8; 33]] {
        &self.error_commitment
    }

    /// Digest of the complete public relaxed instance.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Digest of the witness commitment used by Fiat--Shamir replay.
    pub const fn witness_commitment_digest(&self) -> [u8; 32] {
        self.witness_commitment_digest
    }

    /// Digest of the error commitment used by Fiat--Shamir replay.
    pub const fn error_commitment_digest(&self) -> [u8; 32] {
        self.error_commitment_digest
    }
}

/// Canonical strict public instance supplied by the ZK-AMS core for one fold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23StrictPublicInstanceV1 {
    version: u8,
    public_inputs: [[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1],
    public_input_digest: [u8; 32],
    witness_commitment: Vec<[u8; 33]>,
    witness_commitment_digest: [u8; 32],
    digest: [u8; 32],
}

impl ZkAmsPhase23StrictPublicInstanceV1 {
    /// Construct one exact release-shape strict public instance.
    pub fn new(
        public_inputs: [[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1],
        expected_public_input_digest: [u8; 32],
        witness_commitment: Vec<[u8; 33]>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let public_input_digest = public_input_vector_digest(&public_inputs)?;
        if expected_public_input_digest == [0; 32]
            || expected_public_input_digest != public_input_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let witness_commitment_digest = point_vector_digest(
            PHASE23_WITNESS_COMMITMENT_DOMAIN_V1,
            &witness_commitment,
            ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        )?;
        if witness_commitment_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut value = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            public_inputs,
            public_input_digest,
            witness_commitment,
            witness_commitment_digest,
            digest: [0; 32],
        };
        value.digest = strict_public_instance_digest(&value);
        if value.digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(value)
    }

    /// Exact canonical strict public-input scalar bytes.
    #[must_use]
    pub const fn public_inputs(&self) -> &[[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1] {
        &self.public_inputs
    }

    /// Digest independently supplied by the core and rederived from values.
    #[must_use]
    pub const fn public_input_digest(&self) -> [u8; 32] {
        self.public_input_digest
    }

    /// Canonical nonidentity strict-witness commitment points.
    #[must_use]
    pub fn witness_commitment(&self) -> &[[u8; 33]] {
        &self.witness_commitment
    }

    /// Digest of the exact strict-witness commitment points.
    #[must_use]
    pub const fn witness_commitment_digest(&self) -> [u8; 32] {
        self.witness_commitment_digest
    }

    /// Digest of the complete strict public instance.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Canonical decrypted Hyrax commitment to one hidden Equation (6) cross term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23CrossTermCommitmentV1 {
    version: u8,
    points: Vec<[u8; 33]>,
    preimage_layout_digest: [u8; 32],
    digest: [u8; 32],
}

impl ZkAmsPhase23CrossTermCommitmentV1 {
    /// Bind full-roster-decrypted/PBS commitment points to the sole release
    /// preimage layout.
    pub fn new(
        points: Vec<[u8; 33]>,
        preimage_layout_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let release_layout = canonical_release_commitment_preimage_layout_v1()?;
        if preimage_layout_digest != release_layout.digest() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let digest = point_vector_digest(
            PHASE23_CROSS_TERM_COMMITMENT_DOMAIN_V1,
            &points,
            ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        )?;
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let value = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            points,
            preimage_layout_digest,
            digest,
        };
        Ok(value)
    }

    /// Canonical nonidentity cross-term commitment points.
    #[must_use]
    pub fn points(&self) -> &[[u8; 33]] {
        &self.points
    }

    /// Sole canonical Hyrax commitment-preimage layout digest.
    #[must_use]
    pub const fn preimage_layout_digest(&self) -> [u8; 32] {
        self.preimage_layout_digest
    }

    /// Digest of the exact cross-term commitment points.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// One verifier-replayable public Phase-II/III fold record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23PublicFoldRecordV1 {
    version: u8,
    fold_index: u8,
    terminal_context_digest: [u8; 32],
    composition_context_digest: [u8; 32],
    prior_public_accumulator_digest: [u8; 32],
    strict: ZkAmsPhase23StrictPublicInstanceV1,
    cross_term_commitment: ZkAmsPhase23CrossTermCommitmentV1,
    challenge: [u8; 32],
    resulting_public_accumulator: ZkAmsPhase23PublicAccumulatorV1,
    digest: [u8; 32],
}

impl ZkAmsPhase23PublicFoldRecordV1 {
    /// One-based position in the exact ordered batch.
    #[must_use]
    pub const fn fold_index(&self) -> u8 {
        self.fold_index
    }

    /// Digest of the governed terminal context bound into the audit record.
    #[must_use]
    pub const fn terminal_context_digest(&self) -> [u8; 32] {
        self.terminal_context_digest
    }

    /// Digest of the exact core admission composition context frame.
    #[must_use]
    pub const fn composition_context_digest(&self) -> [u8; 32] {
        self.composition_context_digest
    }

    /// Prior relaxed public-instance digest.
    #[must_use]
    pub const fn prior_public_accumulator_digest(&self) -> [u8; 32] {
        self.prior_public_accumulator_digest
    }

    /// Exact strict public instance for this fold.
    #[must_use]
    pub const fn strict(&self) -> &ZkAmsPhase23StrictPublicInstanceV1 {
        &self.strict
    }

    /// Full-roster-decrypted canonical cross-term commitment.
    #[must_use]
    pub const fn cross_term_commitment(&self) -> &ZkAmsPhase23CrossTermCommitmentV1 {
        &self.cross_term_commitment
    }

    /// Canonical T256 challenge bytes squeezed by the sole masked-Nova
    /// transcript after absorbing full `U1`, `U2`, and `comm_T`.
    #[must_use]
    pub const fn challenge(&self) -> [u8; 32] {
        self.challenge
    }

    /// Resulting relaxed public instance, fully replayed at construction.
    #[must_use]
    pub const fn resulting_public_accumulator(&self) -> &ZkAmsPhase23PublicAccumulatorV1 {
        &self.resulting_public_accumulator
    }

    /// Digest of the complete fold record.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Complete verifier-replayable public history beginning at one fresh relaxed
/// mask and containing at most eight strict folds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23PublicFoldHistoryV1 {
    version: u8,
    terminal_context_digest: [u8; 32],
    composition_context_frame: Vec<u8>,
    composition_context_digest: [u8; 32],
    initial_public_mask: ZkAmsPhase23PublicAccumulatorV1,
    folds: Vec<ZkAmsPhase23PublicFoldRecordV1>,
    digest: [u8; 32],
}

impl ZkAmsPhase23PublicFoldHistoryV1 {
    /// Construct a complete release history with the sole core admission
    /// transcript, canonical shape, and canonical Hyrax key.
    ///
    /// The context frame must be the exact output of the core ZK-AMS admission
    /// context builder. It is retained byte-for-byte so settlement can compare
    /// it with its independently reconstructed frame. Every result and
    /// challenge is derived internally with Nova; callers cannot nominate a
    /// terminal public accumulator.
    pub fn new(
        terminal_context: super::terminal::ZkAmsPhase3TerminalContextV1,
        composition_context_frame: Vec<u8>,
        initial_public_mask: ZkAmsPhase23PublicAccumulatorV1,
        strict_instances: Vec<ZkAmsPhase23StrictPublicInstanceV1>,
        cross_term_commitments: Vec<ZkAmsPhase23CrossTermCommitmentV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        build_public_fold_history_v1(
            terminal_context,
            composition_context_frame,
            initial_public_mask,
            strict_instances,
            cross_term_commitments,
        )
    }

    /// Independently replay every Nova challenge and resulting public
    /// accumulator against the governed terminal context and exact core frame.
    pub fn verify(
        &self,
        terminal_context: super::terminal::ZkAmsPhase3TerminalContextV1,
        expected_composition_context_frame: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        super::terminal::validate_terminal_context(terminal_context)?;
        if terminal_context.digest != self.terminal_context_digest
            || expected_composition_context_frame != self.composition_context_frame
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        validate_public_fold_history(self)
    }

    /// Digest of the governed terminal context bound into every record.
    #[must_use]
    pub const fn terminal_context_digest(&self) -> [u8; 32] {
        self.terminal_context_digest
    }

    /// Exact core admission composition context frame.
    #[must_use]
    pub fn composition_context_frame(&self) -> &[u8] {
        &self.composition_context_frame
    }

    /// Digest of the exact composition context frame.
    #[must_use]
    pub const fn composition_context_digest(&self) -> [u8; 32] {
        self.composition_context_digest
    }

    /// Initial fresh full relaxed public mask instance.
    #[must_use]
    pub const fn initial_public_mask(&self) -> &ZkAmsPhase23PublicAccumulatorV1 {
        &self.initial_public_mask
    }

    /// Exact ordered fold records.
    #[must_use]
    pub fn folds(&self) -> &[ZkAmsPhase23PublicFoldRecordV1] {
        &self.folds
    }

    /// Final replayed relaxed public accumulator.
    #[must_use]
    pub fn final_public_accumulator(&self) -> &ZkAmsPhase23PublicAccumulatorV1 {
        self.folds.last().map_or(&self.initial_public_mask, |fold| {
            &fold.resulting_public_accumulator
        })
    }

    /// Digest of the complete mask and ordered fold history.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

fn canonical_release_commitment_preimage_layout_v1()
-> Result<ZkAmsPhase23CommitmentPreimageLayoutV1, ZkAmsMkheErrorV1> {
    let layout = compile_commitment_preimage_layout_v1(PHASE23_MAX_ROWS_V1)?;
    if layout.message_value_count != PHASE23_MAX_ROWS_V1
        || layout.row_count
            != u32::try_from(ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(layout)
}

fn scalar_from_canonical_bytes(bytes: [u8; 32]) -> Result<Scalar, ZkAmsMkheErrorV1> {
    Scalar::from_be_bytes_exact(bytes).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn point_from_canonical_bytes(bytes: &[u8; 33]) -> Result<VegaT256PointV1, ZkAmsMkheErrorV1> {
    VegaT256PointV1::from_non_identity_wire_bytes_exact(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn public_input_vector_digest(
    public_inputs: &[[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_PUBLIC_INPUT_VECTOR_DOMAIN_V1);
    hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    hash.update(
        &u32::try_from(public_inputs.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for input in public_inputs {
        scalar_from_canonical_bytes(*input)?;
        hash.update(input);
    }
    Ok(hash.finalize())
}

fn point_vector_digest(
    domain: &[u8],
    points: &[[u8; 33]],
    expected_len: usize,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if domain.is_empty() || points.len() != expected_len {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    hash.update(
        &u32::try_from(points.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for point in points {
        point_from_canonical_bytes(point)?;
        hash.update(point);
    }
    Ok(hash.finalize())
}

fn validate_public_accumulator_fields(
    accumulator: &ZkAmsPhase23PublicAccumulatorV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if accumulator.version != PHASE23_ENCRYPTED_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    scalar_from_canonical_bytes(accumulator.relaxation)?;
    let public_input_digest = public_input_vector_digest(&accumulator.public_inputs)?;
    let witness_commitment_digest = point_vector_digest(
        PHASE23_WITNESS_COMMITMENT_DOMAIN_V1,
        &accumulator.witness_commitment,
        ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
    )?;
    let error_commitment_digest = point_vector_digest(
        PHASE23_ERROR_COMMITMENT_DOMAIN_V1,
        &accumulator.error_commitment,
        ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
    )?;
    if accumulator.public_input_digest == [0; 32]
        || accumulator.public_input_digest != public_input_digest
        || accumulator.witness_commitment_digest == [0; 32]
        || accumulator.witness_commitment_digest != witness_commitment_digest
        || accumulator.error_commitment_digest == [0; 32]
        || accumulator.error_commitment_digest != error_commitment_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn public_accumulator_digest_from_bound_fields(
    accumulator: &ZkAmsPhase23PublicAccumulatorV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_PUBLIC_ACCUMULATOR_DOMAIN_V1);
    hash.update(&[accumulator.version]);
    hash.update(&accumulator.relaxation);
    hash.update(&accumulator.public_input_digest);
    hash.update(&accumulator.witness_commitment_digest);
    hash.update(&accumulator.error_commitment_digest);
    hash.finalize()
}

fn public_accumulator_digest(
    accumulator: &ZkAmsPhase23PublicAccumulatorV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_public_accumulator_fields(accumulator)?;
    Ok(public_accumulator_digest_from_bound_fields(accumulator))
}

fn validate_public_accumulator(
    accumulator: &ZkAmsPhase23PublicAccumulatorV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if accumulator.digest == [0; 32]
        || accumulator.digest != public_accumulator_digest(accumulator)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn strict_public_instance_digest(strict: &ZkAmsPhase23StrictPublicInstanceV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_STRICT_INSTANCE_DOMAIN_V1);
    hash.update(&[strict.version]);
    hash.update(&strict.public_input_digest);
    hash.update(&strict.witness_commitment_digest);
    hash.finalize()
}

fn validate_strict_public_instance(
    strict: &ZkAmsPhase23StrictPublicInstanceV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if strict.version != PHASE23_ENCRYPTED_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let public_input_digest = public_input_vector_digest(&strict.public_inputs)?;
    let witness_commitment_digest = point_vector_digest(
        PHASE23_WITNESS_COMMITMENT_DOMAIN_V1,
        &strict.witness_commitment,
        ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
    )?;
    if strict.public_input_digest == [0; 32]
        || strict.public_input_digest != public_input_digest
        || strict.witness_commitment_digest == [0; 32]
        || strict.witness_commitment_digest != witness_commitment_digest
        || strict.digest == [0; 32]
        || strict.digest != strict_public_instance_digest(strict)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_cross_term_commitment(
    cross_term: &ZkAmsPhase23CrossTermCommitmentV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let layout = canonical_release_commitment_preimage_layout_v1()?;
    let digest = point_vector_digest(
        PHASE23_CROSS_TERM_COMMITMENT_DOMAIN_V1,
        &cross_term.points,
        ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
    )?;
    if cross_term.version != PHASE23_ENCRYPTED_VERSION_V1
        || cross_term.preimage_layout_digest != layout.digest()
        || cross_term.digest == [0; 32]
        || cross_term.digest != digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn composition_context_digest_v1(
    composition_context_frame: &[u8],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if composition_context_frame.is_empty()
        || composition_context_frame.len() > PHASE23_MAX_COMPOSITION_CONTEXT_FRAME_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(PHASE23_COMPOSITION_CONTEXT_FRAME_DOMAIN_V1);
    hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    hash.update(super::super::COMPOSITION_DOMAIN_V1);
    hash.update(super::super::COMMITMENT_KEY_LABEL_V1);
    hash.update(
        &u32::try_from(composition_context_frame.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    hash.update(composition_context_frame);
    Ok(hash.finalize())
}

fn commitment_from_canonical_points_v1(
    points: &[[u8; 33]],
    expected_len: usize,
) -> Result<Commitment, ZkAmsMkheErrorV1> {
    if points.len() != expected_len {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let points = points
        .iter()
        .map(point_from_canonical_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    Commitment::from_points(points).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn commitment_to_canonical_points_v1(
    commitment: &Commitment,
    expected_len: usize,
) -> Result<Vec<[u8; 33]>, ZkAmsMkheErrorV1> {
    if commitment.len() != expected_len {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    commitment
        .points()
        .iter()
        .map(|point| {
            point
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
        })
        .collect()
}

fn public_accumulator_to_protocol_v1(
    accumulator: &ZkAmsPhase23PublicAccumulatorV1,
) -> Result<RelaxedInstance, ZkAmsMkheErrorV1> {
    let public_inputs = accumulator
        .public_inputs
        .iter()
        .copied()
        .map(scalar_from_canonical_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelaxedInstance {
        witness_commitment: commitment_from_canonical_points_v1(
            &accumulator.witness_commitment,
            ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        )?,
        error_commitment: commitment_from_canonical_points_v1(
            &accumulator.error_commitment,
            ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        )?,
        relaxation: scalar_from_canonical_bytes(accumulator.relaxation)?,
        public_inputs,
    })
}

fn strict_public_instance_to_protocol_v1(
    strict: &ZkAmsPhase23StrictPublicInstanceV1,
) -> Result<Instance, ZkAmsMkheErrorV1> {
    Ok(Instance {
        witness_commitment: commitment_from_canonical_points_v1(
            &strict.witness_commitment,
            ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        )?,
        public_inputs: strict
            .public_inputs
            .iter()
            .copied()
            .map(scalar_from_canonical_bytes)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn cross_term_commitment_to_protocol_v1(
    cross_term: &ZkAmsPhase23CrossTermCommitmentV1,
) -> Result<Commitment, ZkAmsMkheErrorV1> {
    commitment_from_canonical_points_v1(
        &cross_term.points,
        ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
    )
}

fn public_accumulator_from_protocol_v1(
    accumulator: &RelaxedInstance,
) -> Result<ZkAmsPhase23PublicAccumulatorV1, ZkAmsMkheErrorV1> {
    let public_inputs: [[u8; 32]; ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1] = accumulator
        .public_inputs
        .iter()
        .map(|scalar| scalar.to_be_bytes())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    ZkAmsPhase23PublicAccumulatorV1::new(
        accumulator.relaxation.to_be_bytes(),
        public_inputs,
        commitment_to_canonical_points_v1(
            &accumulator.witness_commitment,
            ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
        )?,
        commitment_to_canonical_points_v1(
            &accumulator.error_commitment,
            ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        )?,
    )
}

fn release_shape_and_commitment_key_v1() -> Result<(&'static Shape, CommitmentKey), ZkAmsMkheErrorV1>
{
    let shape =
        super::super::canonical_shape_ref().map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    if shape.public_input_count() != ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1
        || shape
            .variable_count()
            .div_ceil(MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
            != ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1
        || shape
            .constraint_count()
            .div_ceil(MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
            != ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let key = CommitmentKey::derive(
        super::super::COMMITMENT_KEY_LABEL_V1,
        MASKED_RELAXED_COMMITMENT_COLUMNS_V1,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    Ok((shape, key))
}

fn public_fold_record_digest(record: &ZkAmsPhase23PublicFoldRecordV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_PUBLIC_FOLD_RECORD_DOMAIN_V1);
    hash.update(&[record.version, record.fold_index]);
    hash.update(super::super::COMPOSITION_DOMAIN_V1);
    hash.update(super::super::COMMITMENT_KEY_LABEL_V1);
    hash.update(&record.terminal_context_digest);
    hash.update(&record.composition_context_digest);
    hash.update(&record.prior_public_accumulator_digest);
    hash.update(&record.strict.digest);
    hash.update(&record.cross_term_commitment.digest);
    hash.update(&record.challenge);
    hash.update(&record.resulting_public_accumulator.digest);
    hash.finalize()
}

fn validate_public_fold_record(
    record: &ZkAmsPhase23PublicFoldRecordV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if record.version != PHASE23_ENCRYPTED_VERSION_V1
        || record.fold_index == 0
        || record.fold_index > PHASE23_MAX_BATCH_SIZE_V1
        || [
            record.terminal_context_digest,
            record.composition_context_digest,
            record.prior_public_accumulator_digest,
            record.digest,
        ]
        .contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    if record.digest != public_fold_record_digest(record) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    scalar_from_canonical_bytes(record.challenge)?;
    validate_strict_public_instance(&record.strict)?;
    validate_cross_term_commitment(&record.cross_term_commitment)?;
    validate_public_accumulator(&record.resulting_public_accumulator)?;
    Ok(())
}

fn replay_public_fold_history_v1(
    history: &ZkAmsPhase23PublicFoldHistoryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if history.version != PHASE23_ENCRYPTED_VERSION_V1
        || history.terminal_context_digest == [0; 32]
        || history.folds.is_empty()
        || history.folds.len() > usize::from(PHASE23_MAX_BATCH_SIZE_V1)
        || composition_context_digest_v1(&history.composition_context_frame)?
            != history.composition_context_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    validate_public_accumulator(&history.initial_public_mask)?;
    let mut prior_digest = history.initial_public_mask.digest;
    let mut seen_public_inputs = Vec::with_capacity(history.folds.len());
    for (zero_based_index, record) in history.folds.iter().enumerate() {
        validate_public_fold_record(record)?;
        let expected_fold_index = u8::try_from(zero_based_index + 1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if record.fold_index != expected_fold_index
            || record.terminal_context_digest != history.terminal_context_digest
            || record.composition_context_digest != history.composition_context_digest
            || record.prior_public_accumulator_digest != prior_digest
            || seen_public_inputs.contains(&record.strict.public_input_digest)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen_public_inputs.push(record.strict.public_input_digest);
        prior_digest = record.resulting_public_accumulator.digest;
    }
    let (shape, key) = release_shape_and_commitment_key_v1()?;
    let strict_instances = history
        .folds
        .iter()
        .map(|fold| strict_public_instance_to_protocol_v1(&fold.strict))
        .collect::<Result<Vec<_>, _>>()?;
    let strict_public_inputs = strict_instances
        .iter()
        .map(|instance| instance.public_inputs.clone())
        .collect::<Vec<_>>();
    let mut transcript = masked_relaxed_composition_transcript_v1(
        super::super::COMPOSITION_DOMAIN_V1,
        &history.composition_context_frame,
        shape,
        &strict_public_inputs,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut prior = public_accumulator_to_protocol_v1(&history.initial_public_mask)?;
    for (record, strict) in history.folds.iter().zip(&strict_instances) {
        let nifs = NovaNifs {
            cross_term_commitment: cross_term_commitment_to_protocol_v1(
                &record.cross_term_commitment,
            )?,
        };
        let (result, challenge) = nifs
            .verify_with_challenge(&key, shape, &mut transcript, &prior, strict)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let result_public = public_accumulator_from_protocol_v1(&result)?;
        if record.challenge != challenge.to_be_bytes()
            || record.resulting_public_accumulator != result_public
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        prior = result;
    }
    Ok(())
}

fn public_fold_history_digest(history: &ZkAmsPhase23PublicFoldHistoryV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_PUBLIC_FOLD_HISTORY_DOMAIN_V1);
    hash.update(&[history.version]);
    hash.update(super::super::COMPOSITION_DOMAIN_V1);
    hash.update(super::super::COMMITMENT_KEY_LABEL_V1);
    hash.update(&history.terminal_context_digest);
    hash.update(&history.composition_context_digest);
    hash.update(
        &u32::try_from(history.composition_context_frame.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    hash.update(&history.composition_context_frame);
    hash.update(&history.initial_public_mask.digest);
    hash.update(
        &u32::try_from(history.folds.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    for fold in &history.folds {
        hash.update(&fold.digest);
    }
    hash.finalize()
}

fn validate_public_fold_history(
    history: &ZkAmsPhase23PublicFoldHistoryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if history.digest == [0; 32] || history.digest != public_fold_history_digest(history) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    replay_public_fold_history_v1(history)
}

fn build_public_fold_history_v1(
    terminal_context: super::terminal::ZkAmsPhase3TerminalContextV1,
    composition_context_frame: Vec<u8>,
    initial_public_mask: ZkAmsPhase23PublicAccumulatorV1,
    strict_instances: Vec<ZkAmsPhase23StrictPublicInstanceV1>,
    cross_term_commitments: Vec<ZkAmsPhase23CrossTermCommitmentV1>,
) -> Result<ZkAmsPhase23PublicFoldHistoryV1, ZkAmsMkheErrorV1> {
    super::terminal::validate_terminal_context(terminal_context)?;
    if strict_instances.is_empty()
        || strict_instances.len() > usize::from(PHASE23_MAX_BATCH_SIZE_V1)
        || strict_instances.len() != cross_term_commitments.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let composition_context_digest = composition_context_digest_v1(&composition_context_frame)?;
    validate_public_accumulator(&initial_public_mask)?;
    let mut seen_public_inputs = Vec::with_capacity(strict_instances.len());
    for strict in &strict_instances {
        validate_strict_public_instance(strict)?;
        if seen_public_inputs.contains(&strict.public_input_digest) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen_public_inputs.push(strict.public_input_digest);
    }
    for cross_term in &cross_term_commitments {
        validate_cross_term_commitment(cross_term)?;
    }

    let (shape, key) = release_shape_and_commitment_key_v1()?;
    let protocol_strict = strict_instances
        .iter()
        .map(strict_public_instance_to_protocol_v1)
        .collect::<Result<Vec<_>, _>>()?;
    let strict_public_inputs = protocol_strict
        .iter()
        .map(|instance| instance.public_inputs.clone())
        .collect::<Vec<_>>();
    let mut transcript = masked_relaxed_composition_transcript_v1(
        super::super::COMPOSITION_DOMAIN_V1,
        &composition_context_frame,
        shape,
        &strict_public_inputs,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut prior_protocol = public_accumulator_to_protocol_v1(&initial_public_mask)?;
    let mut prior_digest = initial_public_mask.digest;
    let mut folds = Vec::with_capacity(strict_instances.len());
    for (zero_based_index, ((strict, strict_protocol), cross_term_commitment)) in strict_instances
        .into_iter()
        .zip(protocol_strict)
        .zip(cross_term_commitments)
        .enumerate()
    {
        let nifs = NovaNifs {
            cross_term_commitment: cross_term_commitment_to_protocol_v1(&cross_term_commitment)?,
        };
        let (result, challenge) = nifs
            .verify_with_challenge(
                &key,
                shape,
                &mut transcript,
                &prior_protocol,
                &strict_protocol,
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let resulting_public_accumulator = public_accumulator_from_protocol_v1(&result)?;
        let mut record = ZkAmsPhase23PublicFoldRecordV1 {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            fold_index: u8::try_from(zero_based_index + 1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            terminal_context_digest: terminal_context.digest,
            composition_context_digest,
            prior_public_accumulator_digest: prior_digest,
            strict,
            cross_term_commitment,
            challenge: challenge.to_be_bytes(),
            resulting_public_accumulator,
            digest: [0; 32],
        };
        record.digest = public_fold_record_digest(&record);
        validate_public_fold_record(&record)?;
        prior_digest = record.resulting_public_accumulator.digest;
        prior_protocol = result;
        folds.push(record);
    }
    let mut history = ZkAmsPhase23PublicFoldHistoryV1 {
        version: PHASE23_ENCRYPTED_VERSION_V1,
        terminal_context_digest: terminal_context.digest,
        composition_context_frame,
        composition_context_digest,
        initial_public_mask,
        folds,
        digest: [0; 32],
    };
    history.digest = public_fold_history_digest(&history);
    Ok(history)
}

/// Canonical bounded CSR encoding of one public T256 linear map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23SparseMapV1 {
    /// Encoding version.
    pub version: u8,
    /// Protocol role of this map.
    pub kind: ZkAmsPhase23MapKindV1,
    /// Exact output dimension.
    pub row_count: u32,
    /// Exact input dimension.  For `A`/`B`/`C`, columns use the paper's
    /// canonical `Z=(W,x,u)` order; any backend-specific assignment order is
    /// remapped only at that backend boundary.
    pub column_count: u32,
    /// Exact admitted maximum nonzero count per row.
    pub max_row_fan_in: u32,
    /// CSR offsets, exactly `row_count + 1`, beginning at zero.
    pub row_offsets: Vec<u32>,
    /// Strictly increasing column indices within every row, in the map-kind
    /// order described by `column_count`.
    pub column_indices: Vec<u32>,
    /// Nonzero canonical T256 coefficients corresponding one-for-one to columns.
    pub coefficients: Vec<[u8; 32]>,
    /// Digest of every field and every CSR word.
    pub digest: [u8; 32],
}

impl ZkAmsPhase23SparseMapV1 {
    /// Construct and fully validate one canonical sparse map.
    pub fn new(
        kind: ZkAmsPhase23MapKindV1,
        row_count: u32,
        column_count: u32,
        max_row_fan_in: u32,
        row_offsets: Vec<u32>,
        column_indices: Vec<u32>,
        coefficients: Vec<[u8; 32]>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut map = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            kind,
            row_count,
            column_count,
            max_row_fan_in,
            row_offsets,
            column_indices,
            coefficients,
            digest: [0; 32],
        };
        validate_sparse_map_structure(&map)?;
        map.digest = sparse_map_digest(&map)?;
        validate_sparse_map(&map)?;
        Ok(map)
    }

    /// Encode the exact canonical bounded wire representation.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        validate_sparse_map(self)?;
        let length = sparse_map_wire_length(
            self.row_count,
            u32::try_from(self.column_indices.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        let mut bytes = Vec::with_capacity(length);
        bytes.push(self.version);
        bytes.push(self.kind.tag());
        bytes.extend_from_slice(&self.row_count.to_be_bytes());
        bytes.extend_from_slice(&self.column_count.to_be_bytes());
        bytes.extend_from_slice(&self.max_row_fan_in.to_be_bytes());
        bytes.extend_from_slice(
            &u32::try_from(self.column_indices.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        for offset in &self.row_offsets {
            bytes.extend_from_slice(&offset.to_be_bytes());
        }
        for (column, coefficient) in self.column_indices.iter().zip(&self.coefficients) {
            bytes.extend_from_slice(&column.to_be_bytes());
            bytes.extend_from_slice(coefficient);
        }
        bytes.extend_from_slice(&self.digest);
        debug_assert_eq!(bytes.len(), length);
        Ok(bytes)
    }

    /// Decode after checking all length and allocation ceilings from the fixed
    /// header.  No count from the wire is used as a capacity before these
    /// checks complete.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() < PHASE23_SPARSE_MAP_WIRE_HEADER_BYTES_V1 + 4 + 32 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let version = bytes[0];
        let kind = ZkAmsPhase23MapKindV1::from_tag(bytes[1])?;
        let row_count = read_u32(bytes, 2)?;
        let column_count = read_u32(bytes, 6)?;
        let max_row_fan_in = read_u32(bytes, 10)?;
        let nonzero_count = read_u32(bytes, 14)?;
        let expected = sparse_map_wire_length(row_count, nonzero_count)?;
        if bytes.len() != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let offset_count = usize::try_from(row_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let nonzero_count = usize::try_from(nonzero_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut cursor = 18;
        let mut row_offsets = Vec::with_capacity(offset_count);
        for _ in 0..offset_count {
            row_offsets.push(read_u32(bytes, cursor)?);
            cursor += 4;
        }
        let mut column_indices = Vec::with_capacity(nonzero_count);
        let mut coefficients = Vec::with_capacity(nonzero_count);
        for _ in 0..nonzero_count {
            column_indices.push(read_u32(bytes, cursor)?);
            cursor += 4;
            let mut coefficient = [0_u8; 32];
            coefficient.copy_from_slice(
                bytes
                    .get(cursor..cursor + 32)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            cursor += 32;
            coefficients.push(coefficient);
        }
        let mut digest = [0_u8; 32];
        digest.copy_from_slice(
            bytes
                .get(cursor..cursor + 32)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
        let map = Self {
            version,
            kind,
            row_count,
            column_count,
            max_row_fan_in,
            row_offsets,
            column_indices,
            coefficients,
            digest,
        };
        validate_sparse_map(&map).map_err(|error| match error {
            ZkAmsMkheErrorV1::ResourceCeilingExceeded => error,
            _ => ZkAmsMkheErrorV1::InvalidWireEncoding,
        })?;
        Ok(map)
    }
}

/// Exact binding shared by every ciphertext and evaluated key in one fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23EncryptedBindingV1 {
    /// Binding version.
    pub version: u8,
    /// Exact BGV profile digest.
    pub profile_digest: [u8; 32],
    /// Exact fixed-roster digest.
    pub roster_digest: [u8; 32],
    /// Transcript/key-ceremony digest.
    pub transcript_digest: [u8; 32],
    /// Nonzero batch identifier.
    pub batch_id: [u8; 32],
    /// Digest of the exact NIFS verifier profile.
    pub nifs_verifier_digest: [u8; 32],
    /// Digest of the canonical ordered settlement inputs.
    pub ordered_batch_input_digest: [u8; 32],
    /// Digest of the exact accumulated encrypted state being consumed.
    pub accumulated_state_digest: [u8; 32],
    /// Digest of the exact incoming encrypted state being consumed.
    pub incoming_state_digest: [u8; 32],
    /// One-based fold index.
    pub fold_index: u8,
    /// Digest binding every preceding field.
    pub digest: [u8; 32],
}

impl ZkAmsPhase23EncryptedBindingV1 {
    /// Construct a complete, non-replayable fold binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        transcript_digest: [u8; 32],
        batch_id: [u8; 32],
        nifs_verifier_digest: [u8; 32],
        ordered_batch_input_digest: [u8; 32],
        accumulated_state_digest: [u8; 32],
        incoming_state_digest: [u8; 32],
        fold_index: u8,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut binding = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            profile_digest,
            roster_digest,
            transcript_digest,
            batch_id,
            nifs_verifier_digest,
            ordered_batch_input_digest,
            accumulated_state_digest,
            incoming_state_digest,
            fold_index,
            digest: [0; 32],
        };
        validate_encrypted_binding_fields(binding)?;
        binding.digest = encrypted_binding_digest(binding);
        validate_encrypted_binding(binding)?;
        Ok(binding)
    }
}

/// Exact lengths of all six accumulator families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23AccumulatorShapeV1 {
    /// Number of public-input values.
    pub x: u32,
    /// Number of error-vector values.
    pub e: u32,
    /// Number of error-commitment randomness values.
    pub r_e: u32,
    /// Number of witness values.
    pub w: u32,
    /// Number of witness-commitment randomness values.
    pub r_w: u32,
}

impl ZkAmsPhase23AccumulatorShapeV1 {
    /// Construct a bounded, nonempty shape. The encrypted `U` family has
    /// `e` row replicas, while canonical materialization returns one scalar.
    pub fn new(x: u32, e: u32, r_e: u32, w: u32, r_w: u32) -> Result<Self, ZkAmsMkheErrorV1> {
        let shape = Self { x, e, r_e, w, r_w };
        validate_accumulator_shape(shape)?;
        Ok(shape)
    }

    fn total_values(self) -> Result<usize, ZkAmsMkheErrorV1> {
        [self.x, 1, self.e, self.r_e, self.w, self.r_w]
            .into_iter()
            .try_fold(0_usize, |total, value| {
                total
                    .checked_add(
                        usize::try_from(value)
                            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                    )
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
            })
    }
}

/// Canonical release-packed input to six-family materialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23PackedAccumulatorSetV1 {
    /// Complete accumulator shape.
    pub shape: ZkAmsPhase23AccumulatorShapeV1,
    /// Packed chunks for `x`.
    pub x: Vec<ZkAmsT256PackedPlaintextV1>,
    /// Packed chunks for `u`, repeated in every one of the `shape.e` used
    /// slots. Materialization rejects any non-identical replica.
    pub u: Vec<ZkAmsT256PackedPlaintextV1>,
    /// Packed chunks for `E`.
    pub e: Vec<ZkAmsT256PackedPlaintextV1>,
    /// Packed chunks for `r_E`.
    pub r_e: Vec<ZkAmsT256PackedPlaintextV1>,
    /// Packed chunks for `W`.
    pub w: Vec<ZkAmsT256PackedPlaintextV1>,
    /// Packed chunks for `r_W`.
    pub r_w: Vec<ZkAmsT256PackedPlaintextV1>,
}

/// Canonical, padding-free materialization of all six accumulator families.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23MaterializedAccumulatorsV1 {
    /// Materialization version.
    pub version: u8,
    /// Exact release profile digest.
    pub profile_digest: [u8; 32],
    /// Exact fixed-roster digest.
    pub roster_digest: [u8; 32],
    /// Transcript digest from which shares were released.
    pub transcript_digest: [u8; 32],
    /// Batch identifier.
    pub batch_id: [u8; 32],
    /// Ordered settlement-input digest.
    pub ordered_batch_input_digest: [u8; 32],
    /// Number of completed folds represented by this state.
    pub fold_count: u8,
    /// Complete accumulator shape.
    pub shape: ZkAmsPhase23AccumulatorShapeV1,
    /// Materialized `x`.
    pub x: Vec<[u8; 32]>,
    /// Materialized scalar `u`, always length one.
    pub u: Vec<[u8; 32]>,
    /// Materialized `E`.
    pub e: Vec<[u8; 32]>,
    /// Materialized `r_E`.
    pub r_e: Vec<[u8; 32]>,
    /// Materialized `W`.
    pub w: Vec<[u8; 32]>,
    /// Materialized `r_W`.
    pub r_w: Vec<[u8; 32]>,
    /// Digest of the complete canonical padding-free representation.
    pub digest: [u8; 32],
}

/// Digestible implementation status for this encrypted slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsPhase23EncryptedImplementationV1 {
    /// Implementation schema version.
    pub version: u8,
    /// Maximum canonical map nonzero count.
    pub max_sparse_entries: u32,
    /// Digest of the exact implemented algebra and representations.
    pub algebra_digest: [u8; 32],
    /// Pinned release-parameter positive/negative KAT digest, absent for now.
    pub release_kat_digest: [u8; 32],
    /// True only after the pinned release-size KAT has actually executed.
    pub release_kat_complete: bool,
    /// Digest binding the status including its open release evidence.
    pub digest: [u8; 32],
}

/// Return the exact implementation identity while keeping release evidence open.
#[must_use]
pub fn zk_ams_phase23_encrypted_implementation_v1() -> ZkAmsPhase23EncryptedImplementationV1 {
    let mut implementation = ZkAmsPhase23EncryptedImplementationV1 {
        version: PHASE23_ENCRYPTED_VERSION_V1,
        max_sparse_entries: ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1,
        algebra_digest: keccak256(PHASE23_ENCRYPTED_ALGEBRA_V1),
        release_kat_digest: [0; 32],
        release_kat_complete: false,
        digest: [0; 32],
    };
    let mut frame = Vec::with_capacity(128);
    frame.extend_from_slice(PHASE23_IMPLEMENTATION_DOMAIN_V1);
    frame.push(implementation.version);
    frame.extend_from_slice(&implementation.max_sparse_entries.to_be_bytes());
    frame.extend_from_slice(&implementation.algebra_digest);
    frame.extend_from_slice(&implementation.release_kat_digest);
    frame.push(implementation.release_kat_complete.into());
    implementation.digest = keccak256(&frame);
    implementation
}

impl ZkAmsPhase23MaterializedAccumulatorsV1 {
    /// Encode all six families without padding or implicit dimensions.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        validate_materialized(self)?;
        let length = materialized_wire_length(self.shape)?;
        let mut bytes = Vec::with_capacity(length);
        bytes.push(self.version);
        bytes.extend_from_slice(&self.profile_digest);
        bytes.extend_from_slice(&self.roster_digest);
        bytes.extend_from_slice(&self.transcript_digest);
        bytes.extend_from_slice(&self.batch_id);
        bytes.extend_from_slice(&self.ordered_batch_input_digest);
        bytes.push(self.fold_count);
        for value in [
            self.shape.x,
            1,
            self.shape.e,
            self.shape.r_e,
            self.shape.w,
            self.shape.r_w,
        ] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        for family in [
            self.x.as_slice(),
            self.u.as_slice(),
            self.e.as_slice(),
            self.r_e.as_slice(),
            self.w.as_slice(),
            self.r_w.as_slice(),
        ] {
            for value in family {
                bytes.extend_from_slice(value);
            }
        }
        bytes.extend_from_slice(&self.digest);
        debug_assert_eq!(bytes.len(), length);
        Ok(bytes)
    }

    /// Decode only after the exact six lengths pass every global bound and the
    /// resulting byte length is proven equal to the input length.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() < PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1 + 32 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let version = bytes[0];
        let mut cursor = 1;
        let profile_digest = read_array_32(bytes, &mut cursor)?;
        let roster_digest = read_array_32(bytes, &mut cursor)?;
        let transcript_digest = read_array_32(bytes, &mut cursor)?;
        let batch_id = read_array_32(bytes, &mut cursor)?;
        let ordered_batch_input_digest = read_array_32(bytes, &mut cursor)?;
        let fold_count = *bytes
            .get(cursor)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        cursor += 1;
        let lengths = (0..6)
            .map(|_| {
                let value = read_u32(bytes, cursor)?;
                cursor += 4;
                Ok(value)
            })
            .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?;
        if lengths[1] != 1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let shape = ZkAmsPhase23AccumulatorShapeV1 {
            x: lengths[0],
            e: lengths[2],
            r_e: lengths[3],
            w: lengths[4],
            r_w: lengths[5],
        };
        validate_accumulator_shape(shape).map_err(|error| match error {
            ZkAmsMkheErrorV1::ResourceCeilingExceeded => error,
            _ => ZkAmsMkheErrorV1::InvalidWireEncoding,
        })?;
        if bytes.len() != materialized_wire_length(shape)? {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut decode_family = |length: u32| -> Result<Vec<[u8; 32]>, ZkAmsMkheErrorV1> {
            let length =
                usize::try_from(length).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let mut output = Vec::with_capacity(length);
            for _ in 0..length {
                output.push(read_array_32(bytes, &mut cursor)?);
            }
            Ok(output)
        };
        let x = decode_family(shape.x)?;
        let u = decode_family(1)?;
        let e = decode_family(shape.e)?;
        let r_e = decode_family(shape.r_e)?;
        let w = decode_family(shape.w)?;
        let r_w = decode_family(shape.r_w)?;
        let digest = read_array_32(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let materialized = Self {
            version,
            profile_digest,
            roster_digest,
            transcript_digest,
            batch_id,
            ordered_batch_input_digest,
            fold_count,
            shape,
            x,
            u,
            e,
            r_e,
            w,
            r_w,
            digest,
        };
        validate_materialized(&materialized).map_err(|error| match error {
            ZkAmsMkheErrorV1::ResourceCeilingExceeded => error,
            _ => ZkAmsMkheErrorV1::InvalidWireEncoding,
        })?;
        Ok(materialized)
    }
}

/// Decode release-packed, padding-checked chunks into the sole canonical six-
/// family accumulator representation.
#[allow(clippy::too_many_arguments)]
pub fn zk_ams_phase23_materialize_release_accumulators_v1(
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    transcript_digest: [u8; 32],
    batch_id: [u8; 32],
    ordered_batch_input_digest: [u8; 32],
    fold_count: u8,
    packed: &ZkAmsPhase23PackedAccumulatorSetV1,
) -> Result<ZkAmsPhase23MaterializedAccumulatorsV1, ZkAmsMkheErrorV1> {
    validate_accumulator_shape(packed.shape)?;
    if profile_digest != release_profile_v1().digest()?
        || [
            profile_digest,
            roster_digest,
            transcript_digest,
            batch_id,
            ordered_batch_input_digest,
        ]
        .contains(&[0; 32])
        || fold_count == 0
        || fold_count > PHASE23_MAX_BATCH_SIZE_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let x = decode_release_family(packed.shape.x, &packed.x)?;
    let u = collapse_replicated_u_values(
        decode_release_family(packed.shape.e, &packed.u)?,
        packed.shape.e,
    )?;
    let e = decode_release_family(packed.shape.e, &packed.e)?;
    let r_e = decode_release_family(packed.shape.r_e, &packed.r_e)?;
    let w = decode_release_family(packed.shape.w, &packed.w)?;
    let r_w = decode_release_family(packed.shape.r_w, &packed.r_w)?;
    materialized_from_values(
        profile_digest,
        roster_digest,
        transcript_digest,
        batch_id,
        ordered_batch_input_digest,
        fold_count,
        packed.shape,
        x,
        u,
        e,
        r_e,
        w,
        r_w,
    )
}

fn validate_sparse_map_structure(map: &ZkAmsPhase23SparseMapV1) -> Result<(), ZkAmsMkheErrorV1> {
    let nonzero_count = map.column_indices.len();
    if map.version != PHASE23_ENCRYPTED_VERSION_V1
        || map.row_count == 0
        || map.row_count > PHASE23_MAX_ROWS_V1
        || map.column_count == 0
        || map.column_count > PHASE23_MAX_COLUMNS_V1
        || map.max_row_fan_in == 0
        || map.max_row_fan_in > map.column_count
        || nonzero_count != map.coefficients.len()
        || nonzero_count
            > usize::try_from(ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(
            if nonzero_count
                > usize::try_from(ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            {
                ZkAmsMkheErrorV1::ResourceCeilingExceeded
            } else {
                ZkAmsMkheErrorV1::InvalidPhase23Fold
            },
        );
    }
    let row_count =
        usize::try_from(map.row_count).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if map.row_offsets.len() != row_count + 1
        || map.row_offsets.first() != Some(&0)
        || usize::try_from(*map.row_offsets.last().unwrap_or(&u32::MAX)).ok() != Some(nonzero_count)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for row in 0..row_count {
        let start = usize::try_from(map.row_offsets[row])
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let end = usize::try_from(map.row_offsets[row + 1])
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if start > end
            || end > nonzero_count
            || end - start
                > usize::try_from(map.max_row_fan_in)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let columns = &map.column_indices[start..end];
        if columns.iter().any(|column| *column >= map.column_count)
            || columns.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        for coefficient in &map.coefficients[start..end] {
            let scalar = Scalar::from_be_bytes_exact(*coefficient)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            if scalar.is_zero() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }
    }
    Ok(())
}

fn validate_sparse_map(map: &ZkAmsPhase23SparseMapV1) -> Result<(), ZkAmsMkheErrorV1> {
    validate_sparse_map_structure(map)?;
    if map.digest == [0; 32] || map.digest != sparse_map_digest(map)? {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// Validate a canonical sparse map without materializing its potentially
/// release-sized wire encoding.
pub(super) fn validate_sparse_map_v1(
    map: &ZkAmsPhase23SparseMapV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_sparse_map(map)
}

fn sparse_map_digest(map: &ZkAmsPhase23SparseMapV1) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_sparse_map_structure(map)?;
    let mut hash = Keccak256::new();
    hash.update(PHASE23_SPARSE_MAP_DOMAIN_V1);
    hash.update(&[map.version, map.kind.tag()]);
    hash.update(&map.row_count.to_be_bytes());
    hash.update(&map.column_count.to_be_bytes());
    hash.update(&map.max_row_fan_in.to_be_bytes());
    hash.update(
        &u32::try_from(map.column_indices.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for offset in &map.row_offsets {
        hash.update(&offset.to_be_bytes());
    }
    for (column, coefficient) in map.column_indices.iter().zip(&map.coefficients) {
        hash.update(&column.to_be_bytes());
        hash.update(coefficient);
    }
    Ok(hash.finalize())
}

fn compile_release_maps_v1() -> Result<ZkAmsPhase23ReleaseMapsV1, ZkAmsMkheErrorV1> {
    let shape =
        super::super::canonical_shape_ref().map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let variable_count = shape.variable_count();
    let public_input_count = shape.public_input_count();
    let a = compile_paper_order_relation_map_v1(
        ZkAmsPhase23MapKindV1::A,
        &shape.a,
        variable_count,
        public_input_count,
    )?;
    let b = compile_paper_order_relation_map_v1(
        ZkAmsPhase23MapKindV1::B,
        &shape.b,
        variable_count,
        public_input_count,
    )?;
    let c = compile_paper_order_relation_map_v1(
        ZkAmsPhase23MapKindV1::C,
        &shape.c,
        variable_count,
        public_input_count,
    )?;
    let message_value_count = u32::try_from(shape.constraint_count())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if message_value_count != PHASE23_MAX_ROWS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let commitment_preimage_layout = canonical_release_commitment_preimage_layout_v1()?;
    let digest = release_map_set_digest_v1(&a, &b, &c, commitment_preimage_layout)?;
    if digest != ZK_AMS_PHASE23_RELEASE_MAP_SET_KAT_DIGEST_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(ZkAmsPhase23ReleaseMapsV1 {
        a,
        b,
        c,
        commitment_preimage_layout,
        digest,
    })
}

fn compile_paper_order_relation_map_v1(
    kind: ZkAmsPhase23MapKindV1,
    matrix: &SparseMatrix,
    variable_count: usize,
    public_input_count: usize,
) -> Result<ZkAmsPhase23SparseMapV1, ZkAmsMkheErrorV1> {
    if !matches!(
        kind,
        ZkAmsPhase23MapKindV1::A | ZkAmsPhase23MapKindV1::B | ZkAmsPhase23MapKindV1::C
    ) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let column_count = variable_count
        .checked_add(public_input_count)
        .and_then(|value| value.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if matrix.rows() == 0
        || matrix.columns() != column_count
        || matrix.rows() > PHASE23_MAX_ROWS_V1 as usize
        || column_count > PHASE23_MAX_COLUMNS_V1 as usize
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let mut row_offsets = Vec::with_capacity(
        matrix
            .rows()
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    );
    let mut column_indices = Vec::new();
    let mut coefficients = Vec::new();
    let mut row = Vec::new();
    let mut max_row_fan_in = 0_usize;
    row_offsets.push(0);
    for row_index in 0..matrix.rows() {
        row.clear();
        let entries = matrix
            .row_entries(row_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        for (internal_column, coefficient) in entries {
            let paper_column =
                internal_to_paper_column_v1(internal_column, variable_count, public_input_count)?;
            row.push((paper_column, coefficient.to_be_bytes()));
        }
        row.sort_unstable_by_key(|(column, _)| *column);
        if row.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        max_row_fan_in = max_row_fan_in.max(row.len());
        let prospective_entries = column_indices
            .len()
            .checked_add(row.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if prospective_entries
            > usize::try_from(ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        column_indices.extend(row.iter().map(|(column, _)| *column));
        coefficients.extend(row.iter().map(|(_, coefficient)| *coefficient));
        row_offsets.push(
            u32::try_from(prospective_entries)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        );
    }
    if max_row_fan_in == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    ZkAmsPhase23SparseMapV1::new(
        kind,
        u32::try_from(matrix.rows()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        u32::try_from(column_count).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        u32::try_from(max_row_fan_in).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        row_offsets,
        column_indices,
        coefficients,
    )
}

fn internal_to_paper_column_v1(
    internal_column: usize,
    variable_count: usize,
    public_input_count: usize,
) -> Result<u32, ZkAmsMkheErrorV1> {
    let column_count = variable_count
        .checked_add(public_input_count)
        .and_then(|value| value.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if variable_count == 0
        || public_input_count == 0
        || internal_column >= column_count
        || column_count > PHASE23_MAX_COLUMNS_V1 as usize
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let paper_column = if internal_column < variable_count {
        internal_column
    } else if internal_column == variable_count {
        variable_count
            .checked_add(public_input_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    } else {
        internal_column - 1
    };
    u32::try_from(paper_column).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn compile_commitment_preimage_layout_v1(
    message_value_count: u32,
) -> Result<ZkAmsPhase23CommitmentPreimageLayoutV1, ZkAmsMkheErrorV1> {
    let message_columns =
        u32::try_from(crate::vega::masked_relaxed::MASKED_RELAXED_COMMITMENT_COLUMNS_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if message_value_count == 0
        || message_value_count > PHASE23_MAX_ROWS_V1
        || message_columns == 0
        || !message_columns.is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let row_count = message_value_count.div_ceil(message_columns);
    let blinding_count = row_count;
    let last_row_message_count = message_value_count
        .checked_sub(
            row_count
                .checked_sub(1)
                .and_then(|rows| rows.checked_mul(message_columns))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let hiding_generator_index = message_columns;
    let commitment_key_label_digest = keccak256(super::super::COMMITMENT_KEY_LABEL_V1);
    let generator_basis_digest = super::super::zk_ams_t256_generator_digest_v1();
    let (g_map_digest, h_map_digest, digest) =
        compile_commitment_preimage_layout_without_validation_v1(
            message_value_count,
            message_columns,
            row_count,
            last_row_message_count,
        )?;
    let layout = ZkAmsPhase23CommitmentPreimageLayoutV1 {
        version: PHASE23_ENCRYPTED_VERSION_V1,
        message_value_count,
        row_count,
        message_columns,
        blinding_count,
        last_row_message_count,
        hiding_generator_index,
        commitment_key_label_digest,
        generator_basis_digest,
        g_map_digest,
        h_map_digest,
        digest,
    };
    validate_commitment_preimage_layout(layout)?;
    Ok(layout)
}

fn validate_commitment_preimage_layout(
    layout: ZkAmsPhase23CommitmentPreimageLayoutV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if layout.version != PHASE23_ENCRYPTED_VERSION_V1
        || layout.message_value_count == 0
        || layout.message_value_count > PHASE23_MAX_ROWS_V1
        || layout.row_count == 0
        || layout.message_columns == 0
        || !layout.message_columns.is_power_of_two()
        || layout.blinding_count != layout.row_count
        || layout.hiding_generator_index != layout.message_columns
        || layout.commitment_key_label_digest != keccak256(super::super::COMMITMENT_KEY_LABEL_V1)
        || layout.generator_basis_digest != super::super::zk_ams_t256_generator_digest_v1()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let expected_rows = layout.message_value_count.div_ceil(layout.message_columns);
    let expected_last = layout
        .message_value_count
        .checked_sub(
            expected_rows
                .checked_sub(1)
                .and_then(|rows| rows.checked_mul(layout.message_columns))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if layout.row_count != expected_rows || layout.last_row_message_count != expected_last {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let expected = compile_commitment_preimage_layout_without_validation_v1(
        layout.message_value_count,
        layout.message_columns,
        layout.row_count,
        expected_last,
    )?;
    if layout.g_map_digest != expected.0
        || layout.h_map_digest != expected.1
        || layout.digest != expected.2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn compile_commitment_preimage_layout_without_validation_v1(
    message_value_count: u32,
    message_columns: u32,
    row_count: u32,
    last_row_message_count: u32,
) -> Result<([u8; 32], [u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    let blinding_count = row_count;
    let hiding_generator_index = message_columns;
    let commitment_key_label_digest = keccak256(super::super::COMMITMENT_KEY_LABEL_V1);
    let generator_basis_digest = super::super::zk_ams_t256_generator_digest_v1();
    let mut g_hash = Keccak256::new();
    g_hash.update(PHASE23_COMMITMENT_G_MAP_DOMAIN_V1);
    g_hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    g_hash.update(&row_count.to_be_bytes());
    g_hash.update(&blinding_count.to_be_bytes());
    g_hash.update(&hiding_generator_index.to_be_bytes());
    g_hash.update(&commitment_key_label_digest);
    g_hash.update(&generator_basis_digest);
    g_hash.update(b"r[row]->(row,hiding-generator)");
    let g_map_digest = g_hash.finalize();
    let mut h_hash = Keccak256::new();
    h_hash.update(PHASE23_COMMITMENT_H_MAP_DOMAIN_V1);
    h_hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    h_hash.update(&message_value_count.to_be_bytes());
    h_hash.update(&row_count.to_be_bytes());
    h_hash.update(&message_columns.to_be_bytes());
    h_hash.update(&last_row_message_count.to_be_bytes());
    h_hash.update(&commitment_key_label_digest);
    h_hash.update(&generator_basis_digest);
    h_hash.update(b"T[i]->(floor(i/columns),i%columns)");
    let h_map_digest = h_hash.finalize();
    let mut layout_hash = Keccak256::new();
    layout_hash.update(PHASE23_COMMITMENT_PREIMAGE_LAYOUT_DOMAIN_V1);
    layout_hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    for value in [
        message_value_count,
        row_count,
        message_columns,
        blinding_count,
        last_row_message_count,
        hiding_generator_index,
    ] {
        layout_hash.update(&value.to_be_bytes());
    }
    layout_hash.update(super::super::COMMITMENT_KEY_LABEL_V1);
    layout_hash.update(&commitment_key_label_digest);
    layout_hash.update(&generator_basis_digest);
    layout_hash.update(&g_map_digest);
    layout_hash.update(&h_map_digest);
    Ok((g_map_digest, h_map_digest, layout_hash.finalize()))
}

fn release_map_set_digest_v1(
    a: &ZkAmsPhase23SparseMapV1,
    b: &ZkAmsPhase23SparseMapV1,
    c: &ZkAmsPhase23SparseMapV1,
    layout: ZkAmsPhase23CommitmentPreimageLayoutV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    for (map, kind) in [
        (a, ZkAmsPhase23MapKindV1::A),
        (b, ZkAmsPhase23MapKindV1::B),
        (c, ZkAmsPhase23MapKindV1::C),
    ] {
        validate_sparse_map(map)?;
        if map.kind != kind {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    validate_commitment_preimage_layout(layout)?;
    if a.row_count != b.row_count
        || a.row_count != c.row_count
        || a.column_count != b.column_count
        || a.column_count != c.column_count
        || layout.message_value_count != a.row_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(PHASE23_RELEASE_MAP_SET_DOMAIN_V1);
    hash.update(&[PHASE23_ENCRYPTED_VERSION_V1]);
    hash.update(super::super::SOURCE_PROFILE_V1);
    hash.update(PHASE23_PAPER_COLUMN_ORDER_V1);
    hash.update(&a.row_count.to_be_bytes());
    hash.update(&a.column_count.to_be_bytes());
    hash.update(&a.digest);
    hash.update(&b.digest);
    hash.update(&c.digest);
    hash.update(&layout.digest);
    Ok(hash.finalize())
}

fn sparse_map_wire_length(row_count: u32, nonzero_count: u32) -> Result<usize, ZkAmsMkheErrorV1> {
    if row_count == 0 || row_count > PHASE23_MAX_ROWS_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if nonzero_count > ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    usize::try_from(row_count)
        .ok()
        .and_then(|rows| rows.checked_add(1))
        .and_then(|offsets| offsets.checked_mul(4))
        .and_then(|offset_bytes| {
            usize::try_from(nonzero_count)
                .ok()
                .and_then(|entries| entries.checked_mul(36))
                .and_then(|entry_bytes| offset_bytes.checked_add(entry_bytes))
        })
        .and_then(|body| body.checked_add(PHASE23_SPARSE_MAP_WIRE_HEADER_BYTES_V1 + 32))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn validate_encrypted_binding_fields(
    binding: ZkAmsPhase23EncryptedBindingV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if binding.version != PHASE23_ENCRYPTED_VERSION_V1
        || binding.fold_index == 0
        || binding.fold_index > PHASE23_MAX_BATCH_SIZE_V1
        || [
            binding.profile_digest,
            binding.roster_digest,
            binding.transcript_digest,
            binding.batch_id,
            binding.nifs_verifier_digest,
            binding.ordered_batch_input_digest,
            binding.accumulated_state_digest,
            binding.incoming_state_digest,
        ]
        .contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_encrypted_binding(
    binding: ZkAmsPhase23EncryptedBindingV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_encrypted_binding_fields(binding)?;
    if binding.digest == [0; 32] || binding.digest != encrypted_binding_digest(binding) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn encrypted_binding_digest(binding: ZkAmsPhase23EncryptedBindingV1) -> [u8; 32] {
    let mut frame = Vec::with_capacity(320);
    frame.extend_from_slice(PHASE23_ENCRYPTED_BINDING_DOMAIN_V1);
    frame.push(binding.version);
    frame.extend_from_slice(&binding.profile_digest);
    frame.extend_from_slice(&binding.roster_digest);
    frame.extend_from_slice(&binding.transcript_digest);
    frame.extend_from_slice(&binding.batch_id);
    frame.extend_from_slice(&binding.nifs_verifier_digest);
    frame.extend_from_slice(&binding.ordered_batch_input_digest);
    frame.extend_from_slice(&binding.accumulated_state_digest);
    frame.extend_from_slice(&binding.incoming_state_digest);
    frame.push(binding.fold_index);
    keccak256(&frame)
}

fn validate_accumulator_shape(
    shape: ZkAmsPhase23AccumulatorShapeV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if [shape.x, shape.e, shape.r_e, shape.w, shape.r_w].contains(&0)
        || [shape.x, shape.e, shape.r_e, shape.w, shape.r_w]
            .into_iter()
            .any(|length| length > PHASE23_MAX_ROWS_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    if shape.total_values()? > PHASE23_MAX_ACCUMULATOR_VALUES_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialized_from_values(
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    transcript_digest: [u8; 32],
    batch_id: [u8; 32],
    ordered_batch_input_digest: [u8; 32],
    fold_count: u8,
    shape: ZkAmsPhase23AccumulatorShapeV1,
    x: Vec<[u8; 32]>,
    u: Vec<[u8; 32]>,
    e: Vec<[u8; 32]>,
    r_e: Vec<[u8; 32]>,
    w: Vec<[u8; 32]>,
    r_w: Vec<[u8; 32]>,
) -> Result<ZkAmsPhase23MaterializedAccumulatorsV1, ZkAmsMkheErrorV1> {
    let mut materialized = ZkAmsPhase23MaterializedAccumulatorsV1 {
        version: PHASE23_ENCRYPTED_VERSION_V1,
        profile_digest,
        roster_digest,
        transcript_digest,
        batch_id,
        ordered_batch_input_digest,
        fold_count,
        shape,
        x,
        u,
        e,
        r_e,
        w,
        r_w,
        digest: [0; 32],
    };
    validate_materialized_fields(&materialized)?;
    materialized.digest = materialized_digest(&materialized)?;
    validate_materialized(&materialized)?;
    Ok(materialized)
}

fn validate_materialized_fields(
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_accumulator_shape(materialized.shape)?;
    if materialized.version != PHASE23_ENCRYPTED_VERSION_V1
        || materialized.fold_count == 0
        || materialized.fold_count > PHASE23_MAX_BATCH_SIZE_V1
        || [
            materialized.profile_digest,
            materialized.roster_digest,
            materialized.transcript_digest,
            materialized.batch_id,
            materialized.ordered_batch_input_digest,
        ]
        .contains(&[0; 32])
        || materialized.x.len() != materialized.shape.x as usize
        || materialized.u.len() != 1
        || materialized.e.len() != materialized.shape.e as usize
        || materialized.r_e.len() != materialized.shape.r_e as usize
        || materialized.w.len() != materialized.shape.w as usize
        || materialized.r_w.len() != materialized.shape.r_w as usize
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for family in [
        materialized.x.as_slice(),
        materialized.u.as_slice(),
        materialized.e.as_slice(),
        materialized.r_e.as_slice(),
        materialized.w.as_slice(),
        materialized.r_w.as_slice(),
    ] {
        if family
            .iter()
            .any(|value| Scalar::from_be_bytes_exact(*value).is_err())
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    Ok(())
}

fn validate_materialized(
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_materialized_fields(materialized)?;
    if materialized.digest == [0; 32] || materialized.digest != materialized_digest(materialized)? {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// Validate an already materialized accumulator set without constructing its
/// potentially large canonical byte representation.
pub(super) fn validate_materialized_accumulators_v1(
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_materialized(materialized)
}

fn materialized_digest(
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_materialized_fields(materialized)?;
    let mut hash = Keccak256::new();
    hash.update(PHASE23_MATERIALIZED_DOMAIN_V1);
    hash.update(&[materialized.version]);
    hash.update(&materialized.profile_digest);
    hash.update(&materialized.roster_digest);
    hash.update(&materialized.transcript_digest);
    hash.update(&materialized.batch_id);
    hash.update(&materialized.ordered_batch_input_digest);
    hash.update(&[materialized.fold_count]);
    for value in [
        materialized.shape.x,
        1,
        materialized.shape.e,
        materialized.shape.r_e,
        materialized.shape.w,
        materialized.shape.r_w,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for family in [
        materialized.x.as_slice(),
        materialized.u.as_slice(),
        materialized.e.as_slice(),
        materialized.r_e.as_slice(),
        materialized.w.as_slice(),
        materialized.r_w.as_slice(),
    ] {
        for value in family {
            hash.update(value);
        }
    }
    Ok(hash.finalize())
}

fn materialized_wire_length(
    shape: ZkAmsPhase23AccumulatorShapeV1,
) -> Result<usize, ZkAmsMkheErrorV1> {
    validate_accumulator_shape(shape)?;
    shape
        .total_values()?
        .checked_mul(32)
        .and_then(|body| body.checked_add(PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1 + 32))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn decode_release_family(
    logical_value_count: u32,
    chunks: &[ZkAmsT256PackedPlaintextV1],
) -> Result<Vec<[u8; 32]>, ZkAmsMkheErrorV1> {
    let layout = zk_ams_t256_packing_layout_v1(logical_value_count)?;
    if chunks.len() != layout.chunk_count as usize {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut values = Vec::with_capacity(logical_value_count as usize);
    for (expected_index, chunk) in chunks.iter().enumerate() {
        if chunk.chunk_index != expected_index as u32 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let decoded = decode_zk_ams_t256_packed_plaintext_v1(layout, chunk)?;
        values.extend_from_slice(&decoded[..chunk.used_slots as usize]);
    }
    if values.len() != logical_value_count as usize {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(values)
}

fn collapse_replicated_u_values(
    values: Vec<[u8; 32]>,
    expected_row_count: u32,
) -> Result<Vec<[u8; 32]>, ZkAmsMkheErrorV1> {
    let expected_row_count = usize::try_from(expected_row_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if expected_row_count == 0 || values.len() != expected_row_count {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let scalar = values[0];
    if Scalar::from_be_bytes_exact(scalar).is_err()
        || values[1..].iter().any(|replica| *replica != scalar)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(vec![scalar])
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ZkAmsMkheErrorV1> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    ))
}

fn read_array_32(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let value = bytes
        .get(*cursor..*cursor + 32)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor += 32;
    Ok(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum EncryptedFamily {
    X = 1,
    U = 2,
    E = 3,
    RE = 4,
    W = 5,
    RW = 6,
    AZ = 7,
    BZ = 8,
    CZ = 9,
    CrossTerm = 10,
    CrossTermRandomness = 11,
    CrossTermCommitment = 12,
}

impl EncryptedFamily {
    const fn tag(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EncryptedPackedVector {
    version: u8,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    session_digest: [u8; 32],
    family: EncryptedFamily,
    logical_value_count: u32,
    slots_per_chunk: u32,
    chunks: Vec<LinearCiphertext>,
    digest: [u8; 32],
}

impl EncryptedPackedVector {
    fn new(
        profile: &BgvProfile,
        binding: ZkAmsPhase23EncryptedBindingV1,
        family: EncryptedFamily,
        logical_value_count: u32,
        chunks: Vec<LinearCiphertext>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_encrypted_binding(binding)?;
        let mut vector = Self {
            version: PHASE23_ENCRYPTED_VERSION_V1,
            profile_digest: profile.digest()?,
            roster_digest: binding.roster_digest,
            session_digest: encrypted_session_digest(binding),
            family,
            logical_value_count,
            slots_per_chunk: u32::try_from(slots_per_chunk(profile)?)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
            chunks,
            digest: [0; 32],
        };
        validate_encrypted_vector_fields(profile, binding, &vector)?;
        vector.digest = encrypted_vector_digest(profile, &vector)?;
        validate_encrypted_vector(profile, binding, &vector)?;
        Ok(vector)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EncryptedAccumulatorState {
    x: EncryptedPackedVector,
    u: EncryptedPackedVector,
    e: EncryptedPackedVector,
    r_e: EncryptedPackedVector,
    w: EncryptedPackedVector,
    r_w: EncryptedPackedVector,
    e_commitment: Vec<Scalar>,
    w_commitment: Vec<Scalar>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EncryptedCrossTerm {
    t: EncryptedPackedVector,
    r_t: EncryptedPackedVector,
    encrypted_commitment: EncryptedPackedVector,
}

fn encrypted_session_digest(binding: ZkAmsPhase23EncryptedBindingV1) -> [u8; 32] {
    let mut frame = Vec::with_capacity(256);
    frame.extend_from_slice(b"iroha.zk-ams.v1.phase23.encrypted-session");
    frame.push(binding.version);
    frame.extend_from_slice(&binding.profile_digest);
    frame.extend_from_slice(&binding.roster_digest);
    frame.extend_from_slice(&binding.transcript_digest);
    frame.extend_from_slice(&binding.batch_id);
    frame.extend_from_slice(&binding.nifs_verifier_digest);
    frame.extend_from_slice(&binding.ordered_batch_input_digest);
    frame.push(binding.fold_index);
    keccak256(&frame)
}

fn slots_per_chunk(profile: &BgvProfile) -> Result<usize, ZkAmsMkheErrorV1> {
    profile.validate()?;
    match profile.plaintext_modulus {
        PlaintextModulus::T256 => {
            if profile.digest()? != release_profile_v1().digest()?
                || profile.ring_degree != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            {
                return Err(ZkAmsMkheErrorV1::InvalidProfile);
            }
            Ok(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1)
        }
        #[cfg(test)]
        PlaintextModulus::Tiny(17) if profile.ring_degree == 8 => Ok(4),
        #[cfg(test)]
        PlaintextModulus::Tiny(_) => Err(ZkAmsMkheErrorV1::InvalidProfile),
    }
}

fn packed_chunk_count(logical_values: u32, slots: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    if logical_values == 0 || logical_values > PHASE23_MAX_ROWS_V1 || slots == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    usize::try_from(logical_values)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_add(slots - 1)
        .and_then(|value| value.checked_div(slots))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn validate_encrypted_vector_fields(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    vector: &EncryptedPackedVector,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_encrypted_binding(binding)?;
    let slots = slots_per_chunk(profile)?;
    if vector.version != PHASE23_ENCRYPTED_VERSION_V1
        || vector.profile_digest != profile.digest()?
        || vector.profile_digest != binding.profile_digest
        || vector.roster_digest != binding.roster_digest
        || vector.session_digest != encrypted_session_digest(binding)
        || vector.slots_per_chunk != slots as u32
        || vector.chunks.len() != packed_chunk_count(vector.logical_value_count, slots)?
        || vector.chunks.is_empty()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    if vector.family == EncryptedFamily::U
        && vector.chunks[1..]
            .iter()
            .any(|chunk| chunk != &vector.chunks[0])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut level = None;
    for chunk in &vector.chunks {
        chunk.validate(profile)?;
        if chunk.party_set.digest != binding.roster_digest {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        if level
            .replace(chunk.level)
            .is_some_and(|prior| prior != chunk.level)
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
    }
    Ok(())
}

fn validate_encrypted_vector(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    vector: &EncryptedPackedVector,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_encrypted_vector_fields(profile, binding, vector)?;
    if vector.digest == [0; 32] || vector.digest != encrypted_vector_digest(profile, vector)? {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn encrypted_vector_digest(
    profile: &BgvProfile,
    vector: &EncryptedPackedVector,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PHASE23_PACKED_VECTOR_DOMAIN_V1);
    hash.update(&[vector.version]);
    hash.update(&vector.profile_digest);
    hash.update(&vector.roster_digest);
    hash.update(&vector.session_digest);
    hash.update(&[vector.family.tag()]);
    hash.update(&vector.logical_value_count.to_be_bytes());
    hash.update(&vector.slots_per_chunk.to_be_bytes());
    hash.update(
        &u32::try_from(vector.chunks.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for chunk in &vector.chunks {
        hash_linear_ciphertext(&mut hash, chunk, profile)?;
    }
    Ok(hash.finalize())
}

fn commitment_vector_digest(
    domain: &[u8],
    values: &[Scalar],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if values.is_empty() || values.len() > PHASE23_MAX_ROWS_V1 as usize {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(
        &u32::try_from(values.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for value in values {
        hash.update(&value.to_be_bytes());
    }
    Ok(hash.finalize())
}

fn accumulator_state_digest(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    state: &EncryptedAccumulatorState,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_accumulator_state(profile, binding, state)?;
    let mut frame = Vec::with_capacity(320);
    frame.extend_from_slice(b"iroha.zk-ams.v1.phase23.encrypted-accumulator-state");
    for vector in [
        &state.x, &state.u, &state.e, &state.r_e, &state.w, &state.r_w,
    ] {
        frame.extend_from_slice(&vector.digest);
    }
    frame.extend_from_slice(&commitment_vector_digest(
        b"iroha.zk-ams.v1.phase23.error-commitment",
        &state.e_commitment,
    )?);
    frame.extend_from_slice(&commitment_vector_digest(
        b"iroha.zk-ams.v1.phase23.witness-commitment",
        &state.w_commitment,
    )?);
    Ok(keccak256(&frame))
}

fn validate_accumulator_state(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    state: &EncryptedAccumulatorState,
) -> Result<(), ZkAmsMkheErrorV1> {
    for (vector, family) in [
        (&state.x, EncryptedFamily::X),
        (&state.u, EncryptedFamily::U),
        (&state.e, EncryptedFamily::E),
        (&state.r_e, EncryptedFamily::RE),
        (&state.w, EncryptedFamily::W),
        (&state.r_w, EncryptedFamily::RW),
    ] {
        validate_encrypted_vector(profile, binding, vector)?;
        if vector.family != family {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    if state.u.logical_value_count != state.e.logical_value_count
        || state.e_commitment.is_empty()
        || state.e_commitment.len() != state.w_commitment.len()
        || state.e_commitment.len() > PHASE23_MAX_ROWS_V1 as usize
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let expected_level = state.x.chunks[0].level;
    if [&state.u, &state.r_e, &state.w, &state.r_w]
        .into_iter()
        .any(|vector| vector.chunks[0].level != expected_level)
        || expected_level != 0
        || state.e.chunks[0].level > 1
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DiagonalKey {
    output_chunk: usize,
    segment: usize,
    input_chunk: usize,
    shift: usize,
}

fn evaluate_sparse_map(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    map: &ZkAmsPhase23SparseMapV1,
    inputs: &[&EncryptedPackedVector],
    output_family: EncryptedFamily,
    galois_keys: &[GaloisKey],
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_sparse_map(map)?;
    validate_encrypted_binding(binding)?;
    if inputs.is_empty() || inputs.len() > 8 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let slots = slots_per_chunk(profile)?;
    let mut column_ends = Vec::with_capacity(inputs.len());
    let mut total_columns = 0_u32;
    let mut roster = None;
    let mut level = None;
    let mut saw_replicated_u = false;
    for input in inputs {
        validate_encrypted_vector(profile, binding, input)?;
        let input_column_count = if input.family == EncryptedFamily::U {
            if saw_replicated_u || input.logical_value_count != map.row_count {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            saw_replicated_u = true;
            1
        } else {
            input.logical_value_count
        };
        total_columns = total_columns
            .checked_add(input_column_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        column_ends.push(total_columns);
        let input_roster = input.chunks[0].party_set.clone();
        if roster
            .replace(input_roster.clone())
            .is_some_and(|prior| prior != input_roster)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let input_level = input.chunks[0].level;
        if level
            .replace(input_level)
            .is_some_and(|prior| prior != input_level)
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
    }
    if total_columns != map.column_count {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let roster = roster.ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
    let level = level.ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
    let mut diagonals: BTreeMap<DiagonalKey, Vec<(usize, Scalar)>> = BTreeMap::new();
    for row in 0..map.row_count as usize {
        let start = map.row_offsets[row] as usize;
        let end = map.row_offsets[row + 1] as usize;
        let output_chunk = row / slots;
        let output_slot = row % slots;
        for index in start..end {
            let column = map.column_indices[index];
            let segment = column_ends
                .partition_point(|end| *end <= column)
                .min(inputs.len());
            if segment == inputs.len() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let segment_start = if segment == 0 {
                0
            } else {
                column_ends[segment - 1]
            };
            let local_column = usize::try_from(column - segment_start)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let (input_chunk, shift) = if inputs[segment].family == EncryptedFamily::U {
                if local_column != 0 {
                    return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
                }
                // `U` is already replicated in every row slot and every
                // chunk is an exact clone, so consume the row-aligned copy
                // directly. No automorphism or key switch is needed.
                (output_chunk, 0)
            } else {
                let input_chunk = local_column / slots;
                let input_slot = local_column % slots;
                (input_chunk, (input_slot + slots - output_slot) % slots)
            };
            let coefficient = Scalar::from_be_bytes_exact(map.coefficients[index])
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            diagonals
                .entry(DiagonalKey {
                    output_chunk,
                    segment,
                    input_chunk,
                    shift,
                })
                .or_default()
                .push((output_slot, coefficient));
        }
    }
    if diagonals.len() > PHASE23_MAX_DIAGONALS_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let party_count = roster.parties.len();
    let ring_multiplications =
        diagonals
            .values()
            .zip(diagonals.keys())
            .try_fold(0_usize, |total, (_, key)| {
                let plaintext_product = party_count
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let rotations = if key.shift == 0 {
                    0
                } else {
                    let (_, decomposition) = canonical_slot_shift_decomposition(slots, key.shift)?;
                    let key_switch_count = usize::try_from(decomposition.count_ones())
                        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                    phase23_rotation_ring_multiplication_count(
                        profile,
                        party_count,
                        key_switch_count,
                    )?
                };
                total
                    .checked_add(plaintext_product)
                    .and_then(|value| value.checked_add(rotations))
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
            })?;
    checked_ring_multiplication_work(profile, ring_multiplications)?;
    let output_chunks = packed_chunk_count(map.row_count, slots)?;
    let mut chunks = (0..output_chunks)
        .map(|_| zero_ciphertext(profile, &roster, level))
        .collect::<Result<Vec<_>, _>>()?;
    for (key, entries) in diagonals {
        let source = inputs
            .get(key.segment)
            .and_then(|vector| vector.chunks.get(key.input_chunk))
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let rotated = if key.shift == 0 {
            source.clone()
        } else {
            rotate_ciphertext_by_slot_shift(profile, binding, source, key.shift, galois_keys)?
        };
        let mut mask_slots = vec![Scalar::zero(); slots];
        for (slot, coefficient) in entries {
            if !mask_slots[slot].is_zero() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            mask_slots[slot] = coefficient;
        }
        let mask = encode_slots_to_rns(profile, &mask_slots)?;
        let contribution = rotated.mul_plaintext(&mask, profile)?;
        chunks[key.output_chunk] = chunks[key.output_chunk].add(&contribution, profile)?;
    }
    EncryptedPackedVector::new(profile, binding, output_family, map.row_count, chunks)
}

fn zero_ciphertext(
    profile: &BgvProfile,
    roster: &PartySet,
    level: u8,
) -> Result<LinearCiphertext, ZkAmsMkheErrorV1> {
    let ciphertext = LinearCiphertext {
        version: MKHE_VERSION_V1,
        profile_digest: profile.digest()?,
        party_set: roster.clone(),
        level,
        constant: RnsPolynomial::zero(profile),
        linear: vec![RnsPolynomial::zero(profile); roster.parties.len()],
    };
    ciphertext.validate(profile)?;
    Ok(ciphertext)
}

fn rotation_exponent(profile: &BgvProfile, shift: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    rotation_exponent_for_direction(profile, shift, false)
}

fn rotation_exponent_for_direction(
    profile: &BgvProfile,
    shift: usize,
    inverse: bool,
) -> Result<usize, ZkAmsMkheErrorV1> {
    let slots = slots_per_chunk(profile)?;
    if shift == 0 || shift >= slots {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    match profile.plaintext_modulus {
        PlaintextModulus::T256 => {
            let shift = u32::try_from(shift).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            let exponent = if inverse {
                zk_ams_t256_rotation_exponent_for_direction_v1(
                    shift,
                    ZkAmsT256RotationDirectionV1::Inverse,
                )?
            } else {
                zk_ams_t256_rotation_exponent_v1(shift)?
            };
            usize::try_from(exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)
        }
        #[cfg(test)]
        PlaintextModulus::Tiny(17) => Ok(super::mod_pow(
            5,
            u64::try_from(if inverse { slots - shift } else { shift })
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            u64::try_from(2 * profile.ring_degree).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        ) as usize),
        #[cfg(test)]
        PlaintextModulus::Tiny(_) => Err(ZkAmsMkheErrorV1::InvalidProfile),
    }
}

fn canonical_slot_shift_decomposition(
    slots: usize,
    shift: usize,
) -> Result<(bool, usize), ZkAmsMkheErrorV1> {
    if slots == 0 || !slots.is_power_of_two() || shift == 0 || shift >= slots {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let inverse = slots - shift;
    if inverse.count_ones() < shift.count_ones() {
        Ok((true, inverse))
    } else {
        Ok((false, shift))
    }
}

/// Compose an arbitrary forward slot shift from the governed binary Galois-key
/// schedule.  No direct key for a non-power-of-two shift is required or
/// admitted by the release topology.
fn rotate_ciphertext_by_slot_shift(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    ciphertext: &LinearCiphertext,
    shift: usize,
    galois_keys: &[GaloisKey],
) -> Result<LinearCiphertext, ZkAmsMkheErrorV1> {
    ciphertext.validate(profile)?;
    let slots = slots_per_chunk(profile)?;
    if shift == 0 || shift >= slots {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let (inverse, decomposition) = canonical_slot_shift_decomposition(slots, shift)?;
    let key_switch_count = usize::try_from(decomposition.count_ones())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ring_multiplications = phase23_rotation_ring_multiplication_count(
        profile,
        ciphertext.party_set.parties.len(),
        key_switch_count,
    )?;
    checked_ring_multiplication_work(profile, ring_multiplications)?;
    let mut rotated = ciphertext.clone();
    for bit in 0..usize::BITS {
        let step = 1_usize
            .checked_shl(bit)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if step >= slots {
            break;
        }
        if decomposition & step == 0 {
            continue;
        }
        let exponent = rotation_exponent_for_direction(profile, step, inverse)?;
        let selected = select_rotation_keys(profile, binding, &rotated, exponent, galois_keys)?;
        rotated = rotate_ciphertext(profile, &rotated, exponent, &selected)?;
    }
    Ok(rotated)
}

fn encode_slots_to_rns(
    profile: &BgvProfile,
    slots: &[Scalar],
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if slots.len() != slots_per_chunk(profile)? {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    match profile.plaintext_modulus {
        PlaintextModulus::T256 => {
            let logical_count = u32::try_from(slots.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let layout = zk_ams_t256_packing_layout_v1(logical_count)?;
            let bytes = slots
                .iter()
                .copied()
                .map(Scalar::to_be_bytes)
                .collect::<Vec<_>>();
            let packed = encode_zk_ams_t256_packed_plaintext_v1(layout, 0, &bytes)?;
            packed_plaintext_to_rns_v1(layout, &packed)
        }
        #[cfg(test)]
        PlaintextModulus::Tiny(17) => encode_tiny_slots_to_rns(profile, slots),
        #[cfg(test)]
        PlaintextModulus::Tiny(_) => Err(ZkAmsMkheErrorV1::InvalidProfile),
    }
}

fn select_rotation_keys(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    ciphertext: &LinearCiphertext,
    exponent: usize,
    provisioned: &[GaloisKey],
) -> Result<Vec<GaloisKey>, ZkAmsMkheErrorV1> {
    ciphertext.validate(profile)?;
    let mut selected = Vec::with_capacity(ciphertext.party_set.parties.len());
    for party in &ciphertext.party_set.parties {
        let matching = provisioned
            .iter()
            .filter(|key| key.party == *party && key.exponent == exponent)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        if matching[0].profile_digest != binding.profile_digest
            || matching[0].transcript_digest != binding.transcript_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        selected.push(matching[0].clone());
    }
    Ok(selected)
}

fn multiply_packed_vectors(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    left: &EncryptedPackedVector,
    right: &EncryptedPackedVector,
    output_family: EncryptedFamily,
    product_keys: &[ProductRelinearizationKey],
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, left)?;
    validate_encrypted_vector(profile, binding, right)?;
    if left.logical_value_count != right.logical_value_count
        || left.chunks.len() != right.chunks.len()
        || left.chunks[0].level != 0
        || right.chunks[0].level != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let party_count = left.chunks[0]
        .party_set
        .union(&right.chunks[0].party_set)?
        .parties
        .len();
    let quadratic_components = party_count
        .checked_mul(party_count + 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let ciphertext_product_multiplications = party_count
        .checked_mul(party_count)
        .and_then(|value| value.checked_add(2 * party_count + 1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let relinearization_multiplications = quadratic_components
        .checked_mul(profile.gadget_digits)
        .and_then(|value| value.checked_mul(party_count + 1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_multiplications = ciphertext_product_multiplications
        .checked_add(relinearization_multiplications)
        .and_then(|value| value.checked_mul(left.chunks.len()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    checked_ring_multiplication_work(profile, total_multiplications)?;
    let mut chunks = Vec::with_capacity(left.chunks.len());
    for (left_chunk, right_chunk) in left.chunks.iter().zip(&right.chunks) {
        let quadratic = left_chunk.mul(right_chunk, profile)?;
        let keys = select_product_keys(profile, binding, &quadratic, product_keys)?;
        chunks.push(relinearize(profile, &quadratic, &keys)?);
    }
    EncryptedPackedVector::new(
        profile,
        binding,
        output_family,
        left.logical_value_count,
        chunks,
    )
}

fn select_product_keys(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    ciphertext: &super::QuadraticCiphertext,
    provisioned: &[ProductRelinearizationKey],
) -> Result<Vec<ProductRelinearizationKey>, ZkAmsMkheErrorV1> {
    ciphertext.validate(profile)?;
    let mut selected = Vec::with_capacity(ciphertext.quadratic.len());
    for component in &ciphertext.quadratic {
        let matching = provisioned
            .iter()
            .filter(|key| key.left == component.left && key.right == component.right)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
        }
        let key = matching[0];
        if key.profile_digest != binding.profile_digest
            || key.transcript_digest != binding.transcript_digest
            || key
                .target_set
                .parties
                .iter()
                .any(|party| ciphertext.party_set.index_of(*party).is_none())
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        selected.push(key.clone());
    }
    Ok(selected)
}

fn scale_packed_vector(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    vector: &EncryptedPackedVector,
    scalar: Scalar,
    output_family: EncryptedFamily,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, vector)?;
    let multiplications_per_chunk = vector.chunks[0]
        .party_set
        .parties
        .len()
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_multiplications = vector
        .chunks
        .len()
        .checked_mul(multiplications_per_chunk)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    checked_ring_multiplication_work(profile, total_multiplications)?;
    let mask = encode_slots_to_rns(profile, &vec![scalar; slots_per_chunk(profile)?])?;
    let chunks = vector
        .chunks
        .iter()
        .map(|chunk| chunk.mul_plaintext(&mask, profile))
        .collect::<Result<Vec<_>, _>>()?;
    EncryptedPackedVector::new(
        profile,
        binding,
        output_family,
        vector.logical_value_count,
        chunks,
    )
}

fn negate_packed_vector(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    vector: &EncryptedPackedVector,
    output_family: EncryptedFamily,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, vector)?;
    let chunks = vector
        .chunks
        .iter()
        .map(|chunk| {
            let output = LinearCiphertext {
                version: chunk.version,
                profile_digest: chunk.profile_digest,
                party_set: chunk.party_set.clone(),
                level: chunk.level,
                constant: chunk.constant.negate(profile)?,
                linear: chunk
                    .linear
                    .iter()
                    .map(|component| component.negate(profile))
                    .collect::<Result<Vec<_>, _>>()?,
            };
            output.validate(profile)?;
            Ok(output)
        })
        .collect::<Result<Vec<_>, ZkAmsMkheErrorV1>>()?;
    EncryptedPackedVector::new(
        profile,
        binding,
        output_family,
        vector.logical_value_count,
        chunks,
    )
}

fn promote_packed_vector_to_level_one(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    vector: &EncryptedPackedVector,
    output_family: EncryptedFamily,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, vector)?;
    let chunks = vector
        .chunks
        .iter()
        .map(|chunk| {
            if chunk.level > 1 {
                return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
            }
            let mut output = chunk.clone();
            output.level = 1;
            output.validate(profile)?;
            Ok(output)
        })
        .collect::<Result<Vec<_>, _>>()?;
    EncryptedPackedVector::new(
        profile,
        binding,
        output_family,
        vector.logical_value_count,
        chunks,
    )
}

fn add_packed_vectors(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    left: &EncryptedPackedVector,
    right: &EncryptedPackedVector,
    output_family: EncryptedFamily,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, left)?;
    validate_encrypted_vector(profile, binding, right)?;
    if left.logical_value_count != right.logical_value_count
        || left.chunks.len() != right.chunks.len()
        || left.chunks[0].level != right.chunks[0].level
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let chunks = left
        .chunks
        .iter()
        .zip(&right.chunks)
        .map(|(left, right)| left.add(right, profile))
        .collect::<Result<Vec<_>, _>>()?;
    EncryptedPackedVector::new(
        profile,
        binding,
        output_family,
        left.logical_value_count,
        chunks,
    )
}

#[allow(clippy::too_many_arguments)]
fn encrypted_equation_6(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    maps: [&ZkAmsPhase23SparseMapV1; 3],
    accumulated: &EncryptedAccumulatorState,
    incoming: &EncryptedAccumulatorState,
    galois_keys: &[GaloisKey],
    product_keys: &[ProductRelinearizationKey],
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    validate_accumulator_state(profile, binding, accumulated)?;
    validate_accumulator_state(profile, binding, incoming)?;
    if accumulator_state_digest(profile, binding, accumulated)? != binding.accumulated_state_digest
        || accumulator_state_digest(profile, binding, incoming)? != binding.incoming_state_digest
        || maps[0].kind != ZkAmsPhase23MapKindV1::A
        || maps[1].kind != ZkAmsPhase23MapKindV1::B
        || maps[2].kind != ZkAmsPhase23MapKindV1::C
        || maps.iter().any(|map| map.row_count != maps[0].row_count)
        || maps
            .iter()
            .any(|map| map.column_count != maps[0].column_count)
        || maps[0].row_count != accumulated.e.logical_value_count
        || maps[0].row_count != incoming.e.logical_value_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let accumulated_z = [&accumulated.w, &accumulated.x, &accumulated.u];
    let incoming_z = [&incoming.w, &incoming.x, &incoming.u];
    let az_acc = evaluate_sparse_map(
        profile,
        binding,
        maps[0],
        &accumulated_z,
        EncryptedFamily::AZ,
        galois_keys,
    )?;
    let bz_acc = evaluate_sparse_map(
        profile,
        binding,
        maps[1],
        &accumulated_z,
        EncryptedFamily::BZ,
        galois_keys,
    )?;
    let cz_acc = evaluate_sparse_map(
        profile,
        binding,
        maps[2],
        &accumulated_z,
        EncryptedFamily::CZ,
        galois_keys,
    )?;
    let az_in = evaluate_sparse_map(
        profile,
        binding,
        maps[0],
        &incoming_z,
        EncryptedFamily::AZ,
        galois_keys,
    )?;
    let bz_in = evaluate_sparse_map(
        profile,
        binding,
        maps[1],
        &incoming_z,
        EncryptedFamily::BZ,
        galois_keys,
    )?;
    let cz_in = evaluate_sparse_map(
        profile,
        binding,
        maps[2],
        &incoming_z,
        EncryptedFamily::CZ,
        galois_keys,
    )?;
    let first = multiply_packed_vectors(
        profile,
        binding,
        &az_acc,
        &bz_in,
        EncryptedFamily::CrossTerm,
        product_keys,
    )?;
    let second = multiply_packed_vectors(
        profile,
        binding,
        &az_in,
        &bz_acc,
        EncryptedFamily::CrossTerm,
        product_keys,
    )?;
    let third = multiply_packed_vectors(
        profile,
        binding,
        &accumulated.u,
        &cz_in,
        EncryptedFamily::CrossTerm,
        product_keys,
    )?;
    let fourth = multiply_packed_vectors(
        profile,
        binding,
        &incoming.u,
        &cz_acc,
        EncryptedFamily::CrossTerm,
        product_keys,
    )?;
    let negative_third =
        negate_packed_vector(profile, binding, &third, EncryptedFamily::CrossTerm)?;
    let negative_fourth =
        negate_packed_vector(profile, binding, &fourth, EncryptedFamily::CrossTerm)?;
    let sum = add_packed_vectors(
        profile,
        binding,
        &first,
        &second,
        EncryptedFamily::CrossTerm,
    )?;
    let sum = add_packed_vectors(
        profile,
        binding,
        &sum,
        &negative_third,
        EncryptedFamily::CrossTerm,
    )?;
    add_packed_vectors(
        profile,
        binding,
        &sum,
        &negative_fourth,
        EncryptedFamily::CrossTerm,
    )
}

fn encrypted_equation_7(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    g_map: &ZkAmsPhase23SparseMapV1,
    h_map: &ZkAmsPhase23SparseMapV1,
    cross_term: &EncryptedPackedVector,
    r_t: &EncryptedPackedVector,
    galois_keys: &[GaloisKey],
) -> Result<EncryptedCrossTerm, ZkAmsMkheErrorV1> {
    validate_encrypted_vector(profile, binding, cross_term)?;
    validate_encrypted_vector(profile, binding, r_t)?;
    if cross_term.family != EncryptedFamily::CrossTerm
        || cross_term.chunks[0].level != 1
        || r_t.family != EncryptedFamily::CrossTermRandomness
        || r_t.chunks[0].level != 0
        || g_map.kind != ZkAmsPhase23MapKindV1::CommitmentG
        || h_map.kind != ZkAmsPhase23MapKindV1::CommitmentH
        || g_map.row_count != h_map.row_count
        || g_map.column_count != r_t.logical_value_count
        || h_map.column_count != cross_term.logical_value_count
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let g_r = evaluate_sparse_map(
        profile,
        binding,
        g_map,
        &[r_t],
        EncryptedFamily::CrossTermCommitment,
        galois_keys,
    )?;
    let h_t = evaluate_sparse_map(
        profile,
        binding,
        h_map,
        &[cross_term],
        EncryptedFamily::CrossTermCommitment,
        galois_keys,
    )?;
    let g_r = promote_packed_vector_to_level_one(
        profile,
        binding,
        &g_r,
        EncryptedFamily::CrossTermCommitment,
    )?;
    let encrypted_commitment = add_packed_vectors(
        profile,
        binding,
        &g_r,
        &h_t,
        EncryptedFamily::CrossTermCommitment,
    )?;
    Ok(EncryptedCrossTerm {
        t: cross_term.clone(),
        r_t: r_t.clone(),
        encrypted_commitment,
    })
}

fn fold_linear_encrypted(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    accumulated: &EncryptedPackedVector,
    incoming: &EncryptedPackedVector,
    challenge: Scalar,
    family: EncryptedFamily,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    if challenge.is_zero()
        || accumulated.family != family
        || incoming.family != family
        || accumulated.logical_value_count != incoming.logical_value_count
        || accumulated.chunks[0].level != 0
        || incoming.chunks[0].level != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let scaled = scale_packed_vector(profile, binding, incoming, challenge, family)?;
    add_packed_vectors(profile, binding, accumulated, &scaled, family)
}

fn fold_error_encrypted(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    accumulated: &EncryptedPackedVector,
    cross_term: &EncryptedPackedVector,
    incoming: &EncryptedPackedVector,
    challenge: Scalar,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    if challenge.is_zero()
        || accumulated.family != EncryptedFamily::E
        || cross_term.family != EncryptedFamily::CrossTerm
        || incoming.family != EncryptedFamily::E
        || accumulated.logical_value_count != cross_term.logical_value_count
        || accumulated.logical_value_count != incoming.logical_value_count
        || accumulated.chunks[0].level > 1
        || cross_term.chunks[0].level != 1
        || incoming.chunks[0].level != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let accumulated =
        promote_packed_vector_to_level_one(profile, binding, accumulated, EncryptedFamily::E)?;
    let scaled_cross =
        scale_packed_vector(profile, binding, cross_term, challenge, EncryptedFamily::E)?;
    let scaled_incoming = scale_packed_vector(
        profile,
        binding,
        incoming,
        challenge.square(),
        EncryptedFamily::E,
    )?;
    let scaled_incoming =
        promote_packed_vector_to_level_one(profile, binding, &scaled_incoming, EncryptedFamily::E)?;
    let sum = add_packed_vectors(
        profile,
        binding,
        &accumulated,
        &scaled_cross,
        EncryptedFamily::E,
    )?;
    add_packed_vectors(profile, binding, &sum, &scaled_incoming, EncryptedFamily::E)
}

fn fold_error_randomness_encrypted(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    accumulated: &EncryptedPackedVector,
    cross_randomness: &EncryptedPackedVector,
    incoming: &EncryptedPackedVector,
    challenge: Scalar,
) -> Result<EncryptedPackedVector, ZkAmsMkheErrorV1> {
    if challenge.is_zero()
        || accumulated.family != EncryptedFamily::RE
        || cross_randomness.family != EncryptedFamily::CrossTermRandomness
        || incoming.family != EncryptedFamily::RE
        || accumulated.logical_value_count != cross_randomness.logical_value_count
        || accumulated.logical_value_count != incoming.logical_value_count
        || accumulated.chunks[0].level != 0
        || cross_randomness.chunks[0].level != 0
        || incoming.chunks[0].level != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let scaled_cross = scale_packed_vector(
        profile,
        binding,
        cross_randomness,
        challenge,
        EncryptedFamily::RE,
    )?;
    let scaled_incoming = scale_packed_vector(
        profile,
        binding,
        incoming,
        challenge.square(),
        EncryptedFamily::RE,
    )?;
    let sum = add_packed_vectors(
        profile,
        binding,
        accumulated,
        &scaled_cross,
        EncryptedFamily::RE,
    )?;
    add_packed_vectors(
        profile,
        binding,
        &sum,
        &scaled_incoming,
        EncryptedFamily::RE,
    )
}

#[cfg(test)]
fn fold_encrypted_accumulators(
    profile: &BgvProfile,
    binding: ZkAmsPhase23EncryptedBindingV1,
    accumulated: &EncryptedAccumulatorState,
    incoming: &EncryptedAccumulatorState,
    cross: &EncryptedCrossTerm,
    public_cross_commitment: &[Scalar],
    challenge_context: ZkAmsPhase23ChallengeContextV1,
) -> Result<EncryptedAccumulatorState, ZkAmsMkheErrorV1> {
    validate_encrypted_binding(binding)?;
    validate_accumulator_state(profile, binding, accumulated)?;
    validate_accumulator_state(profile, binding, incoming)?;
    validate_encrypted_vector(profile, binding, &cross.t)?;
    validate_encrypted_vector(profile, binding, &cross.r_t)?;
    validate_encrypted_vector(profile, binding, &cross.encrypted_commitment)?;
    if accumulator_state_digest(profile, binding, accumulated)? != binding.accumulated_state_digest
        || accumulator_state_digest(profile, binding, incoming)? != binding.incoming_state_digest
        || accumulated.x.logical_value_count != incoming.x.logical_value_count
        || accumulated.e.logical_value_count != incoming.e.logical_value_count
        || accumulated.r_e.logical_value_count != incoming.r_e.logical_value_count
        || accumulated.w.logical_value_count != incoming.w.logical_value_count
        || accumulated.r_w.logical_value_count != incoming.r_w.logical_value_count
        || cross.t.logical_value_count != accumulated.e.logical_value_count
        || cross.r_t.logical_value_count != accumulated.r_e.logical_value_count
        || public_cross_commitment.len() != accumulated.e_commitment.len()
        || cross.encrypted_commitment.logical_value_count
            != u32::try_from(public_cross_commitment.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let expected_context = ZkAmsPhase23ChallengeContextV1 {
        batch_id: binding.batch_id,
        nifs_verifier_digest: binding.nifs_verifier_digest,
        ordered_batch_input_digest: binding.ordered_batch_input_digest,
        accumulated_error_commitment_digest: commitment_vector_digest(
            b"iroha.zk-ams.v1.phase23.error-commitment",
            &accumulated.e_commitment,
        )?,
        accumulated_witness_commitment_digest: commitment_vector_digest(
            b"iroha.zk-ams.v1.phase23.witness-commitment",
            &accumulated.w_commitment,
        )?,
        incoming_error_commitment_digest: commitment_vector_digest(
            b"iroha.zk-ams.v1.phase23.error-commitment",
            &incoming.e_commitment,
        )?,
        incoming_witness_commitment_digest: commitment_vector_digest(
            b"iroha.zk-ams.v1.phase23.witness-commitment",
            &incoming.w_commitment,
        )?,
        cross_term_commitment_digest: commitment_vector_digest(
            b"iroha.zk-ams.v1.phase23.cross-term-commitment",
            public_cross_commitment,
        )?,
        fold_index: binding.fold_index,
    };
    if challenge_context != expected_context {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let challenge = zk_ams_phase23_challenge_v1(challenge_context)?;
    let x = fold_linear_encrypted(
        profile,
        binding,
        &accumulated.x,
        &incoming.x,
        challenge,
        EncryptedFamily::X,
    )?;
    let u = fold_linear_encrypted(
        profile,
        binding,
        &accumulated.u,
        &incoming.u,
        challenge,
        EncryptedFamily::U,
    )?;
    let e = fold_error_encrypted(
        profile,
        binding,
        &accumulated.e,
        &cross.t,
        &incoming.e,
        challenge,
    )?;
    let r_e = fold_error_randomness_encrypted(
        profile,
        binding,
        &accumulated.r_e,
        &cross.r_t,
        &incoming.r_e,
        challenge,
    )?;
    let w = fold_linear_encrypted(
        profile,
        binding,
        &accumulated.w,
        &incoming.w,
        challenge,
        EncryptedFamily::W,
    )?;
    let r_w = fold_linear_encrypted(
        profile,
        binding,
        &accumulated.r_w,
        &incoming.r_w,
        challenge,
        EncryptedFamily::RW,
    )?;
    let e_commitment = zk_ams_phase23_fold_quadratic_v1(
        &accumulated.e_commitment,
        public_cross_commitment,
        &incoming.e_commitment,
        challenge,
    )?;
    let w_commitment = zk_ams_phase23_fold_linear_v1(
        &accumulated.w_commitment,
        &incoming.w_commitment,
        challenge,
    )?;
    let state = EncryptedAccumulatorState {
        x,
        u,
        e,
        r_e,
        w,
        r_w,
        e_commitment,
        w_commitment,
    };
    validate_accumulator_state(profile, binding, &state)?;
    Ok(state)
}

#[cfg(test)]
fn encode_tiny_slots_to_rns(
    profile: &BgvProfile,
    slots: &[Scalar],
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if profile.ring_degree != 8 || slots.len() != 4 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let values = slots
        .iter()
        .copied()
        .map(tiny_scalar_value)
        .collect::<Result<Vec<_>, _>>()?;
    let degree = profile.ring_degree;
    let modulus = 17_u64;
    let primitive = 3_u64;
    let generator = 5_u64;
    let inverse_degree = 15_u64; // 8^{-1} mod 17.
    let mut evaluations = [0_u64; 16];
    let mut exponent = 1_u64;
    for value in values {
        let negative = (2 * degree) as u64 - exponent;
        evaluations[exponent as usize] = value;
        evaluations[negative as usize] = value;
        exponent = exponent * generator % (2 * degree) as u64;
    }
    let mut coefficients = vec![0_u64; degree];
    for (coefficient, output) in coefficients.iter_mut().enumerate() {
        let mut sum = 0_u64;
        for odd_exponent in (1..2 * degree).step_by(2) {
            let value = evaluations[odd_exponent];
            let root = super::mod_pow(primitive, odd_exponent as u64, modulus);
            let inverse_root =
                super::mod_inverse(root, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
            let basis = super::mod_pow(inverse_root, coefficient as u64, modulus);
            sum = (sum + value * basis) % modulus;
        }
        *output = sum * inverse_degree % modulus;
    }
    RnsPolynomial::from_test_plaintext(profile, &coefficients)
}

#[cfg(test)]
fn decode_tiny_slots_from_coefficients(
    profile: &BgvProfile,
    coefficients: &[u64],
) -> Result<Vec<Scalar>, ZkAmsMkheErrorV1> {
    if profile.ring_degree != 8
        || coefficients.len() != profile.ring_degree
        || coefficients.iter().any(|value| *value >= 17)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let degree = profile.ring_degree;
    let primitive = 3_u64;
    let generator = 5_u64;
    let mut exponent = 1_u64;
    let mut slots = Vec::with_capacity(4);
    for _ in 0..4 {
        let root = super::mod_pow(primitive, exponent, 17);
        let negative_root = super::mod_pow(primitive, (2 * degree) as u64 - exponent, 17);
        let evaluate = |point: u64| {
            coefficients.iter().rev().fold(0_u64, |value, coefficient| {
                (value * point + coefficient) % 17
            })
        };
        let value = evaluate(root);
        if value != evaluate(negative_root) {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        slots.push(Scalar::from_u64(value));
        exponent = exponent * generator % (2 * degree) as u64;
    }
    Ok(slots)
}

#[cfg(test)]
fn tiny_scalar_value(value: Scalar) -> Result<u64, ZkAmsMkheErrorV1> {
    let bytes = value.to_be_bytes();
    Ok(bytes.iter().fold(0_u64, |remainder, byte| {
        (remainder * 256 + u64::from(*byte)) % 17
    }))
}

#[cfg(test)]
mod tests {
    use super::super::{
        AuthenticationSecret, IndependentPublicKey, IndependentSecretKey,
        MaskedRelaxedRandomSourceV1, ZkAmsMkhePartyIdV1, aggregate_rkg_round_one,
        aggregate_rkg_round_two, decrypt_test_plaintext, encrypt, generate_galois_key,
        independent_keygen, rkg_round_one, rkg_round_two, shake256,
    };
    use super::*;
    use crate::vega::{MaskedRelaxedRandomErrorV1, VEGA_T256_SCALAR_MODULUS_BE_V1};

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0x6e; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: false,
            gadget_base_log: 8,
            gadget_digits: 8,
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 1 << 22,
        }
    }

    struct KatRandom {
        state: [u8; 32],
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                state: keccak256(label),
                counter: 0,
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for KatRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                let mut frame = Vec::with_capacity(40);
                frame.extend_from_slice(&self.state);
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = shake256(&frame, 64);
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                self.state = keccak256(&block);
                self.counter = self.counter.wrapping_add(1);
                written += take;
            }
            Ok(())
        }
    }

    fn s(value: u64) -> Scalar {
        Scalar::from_u64(value)
    }

    fn sparse_map(
        kind: ZkAmsPhase23MapKindV1,
        column_count: u32,
        rows: &[Vec<(u32, u64)>],
    ) -> ZkAmsPhase23SparseMapV1 {
        let mut offsets = Vec::with_capacity(rows.len() + 1);
        let mut columns = Vec::new();
        let mut coefficients = Vec::new();
        offsets.push(0);
        for row in rows {
            for (column, coefficient) in row {
                columns.push(*column);
                coefficients.push(s(*coefficient).to_be_bytes());
            }
            offsets.push(columns.len() as u32);
        }
        ZkAmsPhase23SparseMapV1::new(
            kind,
            rows.len() as u32,
            column_count,
            rows.iter().map(Vec::len).max().unwrap_or(1) as u32,
            offsets,
            columns,
            coefficients,
        )
        .unwrap()
    }

    fn sample_map() -> ZkAmsPhase23SparseMapV1 {
        sparse_map(
            ZkAmsPhase23MapKindV1::A,
            8,
            &[
                vec![(0, 2), (5, 1)],
                vec![(1, 3), (7, 1)],
                vec![(4, 1), (6, 4)],
                vec![(2, 5)],
                vec![(3, 2), (7, 2)],
                vec![(0, 1), (6, 1)],
            ],
        )
    }

    #[test]
    fn public_release_history_types_enforce_exact_geometry_and_canonical_encodings() {
        type PublicHistoryConstructor =
            fn(
                super::super::terminal::ZkAmsPhase3TerminalContextV1,
                Vec<u8>,
                ZkAmsPhase23PublicAccumulatorV1,
                Vec<ZkAmsPhase23StrictPublicInstanceV1>,
                Vec<ZkAmsPhase23CrossTermCommitmentV1>,
            ) -> Result<ZkAmsPhase23PublicFoldHistoryV1, ZkAmsMkheErrorV1>;
        let _public_constructor: PublicHistoryConstructor = ZkAmsPhase23PublicFoldHistoryV1::new;

        assert_eq!(ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1, 89);
        assert_eq!(ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1, 512);
        assert_eq!(ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1, 1_024);
        let generator = VegaT256PointV1::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap();
        let public_inputs = [s(3).to_be_bytes(); ZK_AMS_PHASE23_RELEASE_PUBLIC_INPUT_COUNT_V1];
        let witness = vec![generator; ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1];
        let error = vec![generator; ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1];

        assert_eq!(
            ZkAmsPhase23PublicAccumulatorV1::new(
                s(2).to_be_bytes(),
                public_inputs,
                witness[..witness.len() - 1].to_vec(),
                error.clone(),
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            ZkAmsPhase23PublicAccumulatorV1::new(
                VEGA_T256_SCALAR_MODULUS_BE_V1,
                public_inputs,
                witness.clone(),
                error.clone(),
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let accumulator = ZkAmsPhase23PublicAccumulatorV1::new(
            s(2).to_be_bytes(),
            public_inputs,
            witness.clone(),
            error.clone(),
        )
        .unwrap();
        assert_ne!(accumulator.public_input_digest(), [0; 32]);
        assert_ne!(accumulator.witness_commitment_digest(), [0; 32]);
        assert_ne!(accumulator.error_commitment_digest(), [0; 32]);
        assert_ne!(accumulator.digest(), [0; 32]);

        let public_input_digest = public_input_vector_digest(&public_inputs).unwrap();
        assert_eq!(
            ZkAmsPhase23StrictPublicInstanceV1::new(public_inputs, [0; 32], witness.clone(),),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let strict =
            ZkAmsPhase23StrictPublicInstanceV1::new(public_inputs, public_input_digest, witness)
                .unwrap();
        assert_eq!(strict.public_input_digest(), public_input_digest);
        assert_ne!(strict.witness_commitment_digest(), [0; 32]);
        assert_ne!(strict.digest(), [0; 32]);

        let layout = canonical_release_commitment_preimage_layout_v1().unwrap();
        assert_eq!(
            ZkAmsPhase23CrossTermCommitmentV1::new(error.clone(), [0; 32]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let cross = ZkAmsPhase23CrossTermCommitmentV1::new(error, layout.digest()).unwrap();
        assert_eq!(cross.preimage_layout_digest(), layout.digest());
        assert_ne!(cross.digest(), [0; 32]);

        assert_eq!(
            composition_context_digest_v1(&[]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            composition_context_digest_v1(&vec![
                1;
                PHASE23_MAX_COMPOSITION_CONTEXT_FRAME_BYTES_V1 + 1
            ]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_ne!(
            composition_context_digest_v1(b"exact-core-context-frame").unwrap(),
            [0; 32]
        );
    }

    #[test]
    fn canonical_release_maps_layout_order_and_source_shape_are_pinned() {
        let release = zk_ams_phase23_release_maps_v1().expect("canonical release maps compile");
        let [a, b, c] = release.abc();
        assert_eq!(
            [a.kind, b.kind, c.kind],
            [
                ZkAmsPhase23MapKindV1::A,
                ZkAmsPhase23MapKindV1::B,
                ZkAmsPhase23MapKindV1::C,
            ]
        );
        for map in [a, b, c] {
            assert_eq!(map.row_count, 1_048_576);
            assert_eq!(map.column_count, 524_378);
            validate_sparse_map(map).unwrap();
        }
        assert_eq!(require_release_relation_maps_v1([a, b, c]), Ok(()));
        assert_eq!(
            require_release_relation_maps_v1([b, a, c]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let tiny_substitute = sparse_map(ZkAmsPhase23MapKindV1::A, 1, &[vec![(0, 1)]]);
        assert_eq!(
            require_release_relation_maps_v1([&tiny_substitute, b, c]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let type_confused = sparse_map(ZkAmsPhase23MapKindV1::CommitmentG, 1, &[vec![(0, 1)]]);
        assert_eq!(
            require_release_relation_maps_v1([&type_confused, b, c]),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let variable_count = 524_288;
        let public_input_count = 89;
        assert_eq!(
            internal_to_paper_column_v1(variable_count - 1, variable_count, public_input_count),
            Ok(524_287)
        );
        assert_eq!(
            internal_to_paper_column_v1(variable_count, variable_count, public_input_count),
            Ok(524_377)
        );
        assert_eq!(
            internal_to_paper_column_v1(variable_count + 1, variable_count, public_input_count),
            Ok(524_288)
        );
        assert_eq!(
            internal_to_paper_column_v1(
                variable_count + public_input_count,
                variable_count,
                public_input_count,
            ),
            Ok(524_376)
        );
        assert_eq!(
            internal_to_paper_column_v1(
                variable_count + public_input_count + 1,
                variable_count,
                public_input_count,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let shape = super::super::super::canonical_shape_ref().unwrap();
        assert_eq!(
            compile_paper_order_relation_map_v1(
                ZkAmsPhase23MapKindV1::A,
                &shape.a,
                shape.variable_count(),
                shape.public_input_count() - 1,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let layout = release.commitment_preimage_layout();
        assert_eq!(layout.version(), 1);
        assert_eq!(layout.message_value_count(), 1_048_576);
        assert_eq!(layout.row_count(), 1_024);
        assert_eq!(layout.message_columns(), 1_024);
        assert_eq!(layout.blinding_count(), 1_024);
        assert_eq!(layout.last_row_message_count(), 1_024);
        assert_eq!(layout.hiding_generator_index(), 1_024);
        assert_eq!(layout.blinding_position(0), Ok((0, 1_024)));
        assert_eq!(layout.blinding_position(1_023), Ok((1_023, 1_024)));
        assert_eq!(
            layout.blinding_position(1_024),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(layout.message_position(0), Ok((0, 0)));
        assert_eq!(layout.message_position(1_023), Ok((0, 1_023)));
        assert_eq!(layout.message_position(1_024), Ok((1, 0)));
        assert_eq!(layout.message_position(1_048_575), Ok((1_023, 1_023)));
        assert_eq!(
            layout.message_position(1_048_576),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_ne!(layout.commitment_key_label_digest(), [0; 32]);
        assert_ne!(layout.generator_basis_digest(), [0; 32]);
        assert_ne!(layout.g_map_digest(), [0; 32]);
        assert_ne!(layout.h_map_digest(), [0; 32]);
        assert_ne!(layout.g_map_digest(), layout.h_map_digest());
        let mut malformed_layout = layout;
        malformed_layout.hiding_generator_index -= 1;
        assert_eq!(
            validate_commitment_preimage_layout(malformed_layout),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        let mut spliced_layout = layout;
        spliced_layout.g_map_digest = layout.h_map_digest;
        assert_eq!(
            validate_commitment_preimage_layout(spliced_layout),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        assert_eq!(
            zk_ams_phase23_release_map_set_digest_v1(),
            Ok(release.digest())
        );
        assert_eq!(
            release.digest(),
            ZK_AMS_PHASE23_RELEASE_MAP_SET_KAT_DIGEST_V1,
            "the canonical release-map KAT drifted"
        );
    }

    struct TestKeys {
        authentication_a: AuthenticationSecret,
        authentication_b: AuthenticationSecret,
        secret_a: IndependentSecretKey,
        secret_b: IndependentSecretKey,
        public_a: IndependentPublicKey,
        public_b: IndependentPublicKey,
        roster: PartySet,
    }

    impl TestKeys {
        fn generate(profile: &BgvProfile, random: &mut KatRandom) -> Self {
            let authentication_a = AuthenticationSecret::generate(random).unwrap();
            let authentication_b = AuthenticationSecret::generate(random).unwrap();
            let party_a = authentication_a.party_id().unwrap();
            let party_b = authentication_b.party_id().unwrap();
            let (secret_a, public_a) = independent_keygen(profile, party_a, random).unwrap();
            let (secret_b, public_b) = independent_keygen(profile, party_b, random).unwrap();
            let roster = PartySet::singleton(party_a)
                .union(&PartySet::singleton(party_b))
                .unwrap();
            Self {
                authentication_a,
                authentication_b,
                secret_a,
                secret_b,
                public_a,
                public_b,
                roster,
            }
        }

        fn ordered_secrets(&self) -> Vec<&IndependentSecretKey> {
            if self.secret_a.party < self.secret_b.party {
                vec![&self.secret_a, &self.secret_b]
            } else {
                vec![&self.secret_b, &self.secret_a]
            }
        }

        fn ordered_participants(&self) -> Vec<(&IndependentSecretKey, &AuthenticationSecret)> {
            let mut participants = vec![
                (&self.secret_a, &self.authentication_a),
                (&self.secret_b, &self.authentication_b),
            ];
            participants.sort_by_key(|(secret, _)| secret.party);
            participants
        }
    }

    fn test_binding(
        profile: &BgvProfile,
        roster: &PartySet,
        accumulated_state_digest: [u8; 32],
        incoming_state_digest: [u8; 32],
    ) -> ZkAmsPhase23EncryptedBindingV1 {
        ZkAmsPhase23EncryptedBindingV1::new(
            profile.digest().unwrap(),
            roster.digest,
            keccak256(b"phase23-encrypted-test-key-transcript"),
            keccak256(b"phase23-encrypted-test-batch"),
            keccak256(b"phase23-encrypted-test-nifs"),
            keccak256(b"phase23-encrypted-test-ordered-inputs"),
            accumulated_state_digest,
            incoming_state_digest,
            2,
        )
        .unwrap()
    }

    fn encrypt_collective_vector(
        profile: &BgvProfile,
        binding: ZkAmsPhase23EncryptedBindingV1,
        family: EncryptedFamily,
        values: &[u64],
        keys: &TestKeys,
        random: &mut KatRandom,
    ) -> EncryptedPackedVector {
        let slots = slots_per_chunk(profile).unwrap();
        let zero = encode_slots_to_rns(profile, &vec![Scalar::zero(); slots]).unwrap();
        let chunks = values
            .chunks(slots)
            .map(|values| {
                let mut packed = vec![Scalar::zero(); slots];
                for (destination, value) in packed.iter_mut().zip(values) {
                    *destination = s(*value % 17);
                }
                let message = encode_slots_to_rns(profile, &packed).unwrap();
                let owner = encrypt(profile, &keys.public_a, &message, random).unwrap();
                let other = encrypt(profile, &keys.public_b, &zero, random).unwrap();
                owner.add(&other, profile).unwrap()
            })
            .collect();
        EncryptedPackedVector::new(profile, binding, family, values.len() as u32, chunks).unwrap()
    }

    fn encrypt_collective_replicated_u(
        profile: &BgvProfile,
        binding: ZkAmsPhase23EncryptedBindingV1,
        scalar: u64,
        row_count: u32,
        keys: &TestKeys,
        random: &mut KatRandom,
    ) -> EncryptedPackedVector {
        let slots = slots_per_chunk(profile).unwrap();
        let message = encode_slots_to_rns(profile, &vec![s(scalar % 17); slots]).unwrap();
        let zero = encode_slots_to_rns(profile, &vec![Scalar::zero(); slots]).unwrap();
        let owner = encrypt(profile, &keys.public_a, &message, random).unwrap();
        let other = encrypt(profile, &keys.public_b, &zero, random).unwrap();
        let replicated_chunk = owner.add(&other, profile).unwrap();
        let chunk_count = packed_chunk_count(row_count, slots).unwrap();
        EncryptedPackedVector::new(
            profile,
            binding,
            EncryptedFamily::U,
            row_count,
            vec![replicated_chunk; chunk_count],
        )
        .unwrap()
    }

    fn decrypt_collective_vector(
        profile: &BgvProfile,
        vector: &EncryptedPackedVector,
        keys: &TestKeys,
    ) -> Result<Vec<u64>, ZkAmsMkheErrorV1> {
        let slots = slots_per_chunk(profile)?;
        let mut output = Vec::with_capacity(vector.chunks.len() * slots);
        let secrets = keys.ordered_secrets();
        for chunk in &vector.chunks {
            let coefficients = decrypt_test_plaintext(profile, chunk, &secrets)?;
            output.extend(
                decode_tiny_slots_from_coefficients(profile, &coefficients)?
                    .into_iter()
                    .map(tiny_scalar_value)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let logical = vector.logical_value_count as usize;
        if vector.family == EncryptedFamily::U {
            let scalar = *output.first().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            if output.iter().any(|replica| *replica != scalar) {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        } else if output[logical..].iter().any(|value| *value != 0) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        output.truncate(logical);
        Ok(output)
    }

    fn generate_product_key(
        profile: &BgvProfile,
        party_set: &PartySet,
        transcript_digest: [u8; 32],
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        participants: &[(&IndependentSecretKey, &AuthenticationSecret)],
        random: &mut KatRandom,
    ) -> ProductRelinearizationKey {
        let mut ordered = participants.to_vec();
        ordered.sort_by_key(|(secret, _)| secret.party);
        let mut states = Vec::with_capacity(ordered.len());
        let mut first = Vec::with_capacity(ordered.len());
        for &(secret, authentication) in &ordered {
            let (state, contribution) = rkg_round_one(
                profile,
                party_set,
                transcript_digest,
                left,
                right,
                secret,
                authentication,
                random,
            )
            .unwrap();
            states.push(state);
            first.push(contribution);
        }
        let aggregate =
            aggregate_rkg_round_one(profile, party_set, transcript_digest, left, right, &first)
                .unwrap();
        let second = states
            .into_iter()
            .zip(ordered)
            .map(|(state, (secret, authentication))| {
                rkg_round_two(profile, &aggregate, state, secret, authentication, random).unwrap()
            })
            .collect::<Vec<_>>();
        aggregate_rkg_round_two(profile, &aggregate, &second).unwrap()
    }

    fn evaluation_keys(
        profile: &BgvProfile,
        binding: ZkAmsPhase23EncryptedBindingV1,
        keys: &TestKeys,
        random: &mut KatRandom,
    ) -> (Vec<GaloisKey>, Vec<ProductRelinearizationKey>) {
        let mut galois = Vec::new();
        let schedule_bits = slots_per_chunk(profile).unwrap().trailing_zeros();
        for bit in 0..schedule_bits {
            let shift = 1_usize << bit;
            let exponent = rotation_exponent(profile, shift).unwrap();
            galois.push(
                generate_galois_key(
                    profile,
                    binding.transcript_digest,
                    exponent,
                    &keys.secret_a,
                    &keys.public_a,
                    &keys.authentication_a,
                    random,
                )
                .unwrap(),
            );
            galois.push(
                generate_galois_key(
                    profile,
                    binding.transcript_digest,
                    exponent,
                    &keys.secret_b,
                    &keys.public_b,
                    &keys.authentication_b,
                    random,
                )
                .unwrap(),
            );
        }
        // The release schedule omits inverse half-turn because its exponent is
        // identical to the forward half-turn.
        for bit in 0..schedule_bits - 1 {
            let shift = 1_usize << bit;
            let exponent = rotation_exponent_for_direction(profile, shift, true).unwrap();
            galois.push(
                generate_galois_key(
                    profile,
                    binding.transcript_digest,
                    exponent,
                    &keys.secret_a,
                    &keys.public_a,
                    &keys.authentication_a,
                    random,
                )
                .unwrap(),
            );
            galois.push(
                generate_galois_key(
                    profile,
                    binding.transcript_digest,
                    exponent,
                    &keys.secret_b,
                    &keys.public_b,
                    &keys.authentication_b,
                    random,
                )
                .unwrap(),
            );
        }
        let party_a = keys.secret_a.party;
        let party_b = keys.secret_b.party;
        let participants = keys.ordered_participants();
        let (left, right) = if party_a < party_b {
            (party_a, party_b)
        } else {
            (party_b, party_a)
        };
        let product = vec![
            generate_product_key(
                profile,
                &PartySet::singleton(party_a),
                binding.transcript_digest,
                party_a,
                party_a,
                &[(&keys.secret_a, &keys.authentication_a)],
                random,
            ),
            generate_product_key(
                profile,
                &keys.roster,
                binding.transcript_digest,
                left,
                right,
                &participants,
                random,
            ),
            generate_product_key(
                profile,
                &PartySet::singleton(party_b),
                binding.transcript_digest,
                party_b,
                party_b,
                &[(&keys.secret_b, &keys.authentication_b)],
                random,
            ),
        ];
        (galois, product)
    }

    fn make_state(
        profile: &BgvProfile,
        binding: ZkAmsPhase23EncryptedBindingV1,
        values: [&[u64]; 6],
        strict: bool,
        e_commitment: &[u64],
        w_commitment: &[u64],
        keys: &TestKeys,
        random: &mut KatRandom,
    ) -> EncryptedAccumulatorState {
        assert_eq!(values[1].len(), 1, "u ingress accepts one relaxed scalar");
        if strict {
            assert_eq!(values[1][0], 1, "strict ingress fixes u=1");
        }
        EncryptedAccumulatorState {
            x: encrypt_collective_vector(
                profile,
                binding,
                EncryptedFamily::X,
                values[0],
                keys,
                random,
            ),
            u: encrypt_collective_replicated_u(
                profile,
                binding,
                values[1][0],
                u32::try_from(values[2].len()).unwrap(),
                keys,
                random,
            ),
            e: encrypt_collective_vector(
                profile,
                binding,
                EncryptedFamily::E,
                values[2],
                keys,
                random,
            ),
            r_e: encrypt_collective_vector(
                profile,
                binding,
                EncryptedFamily::RE,
                values[3],
                keys,
                random,
            ),
            w: encrypt_collective_vector(
                profile,
                binding,
                EncryptedFamily::W,
                values[4],
                keys,
                random,
            ),
            r_w: encrypt_collective_vector(
                profile,
                binding,
                EncryptedFamily::RW,
                values[5],
                keys,
                random,
            ),
            e_commitment: e_commitment.iter().copied().map(s).collect(),
            w_commitment: w_commitment.iter().copied().map(s).collect(),
        }
    }

    fn evaluate_sparse_oracle(map: &ZkAmsPhase23SparseMapV1, input: &[u64]) -> Vec<u64> {
        (0..map.row_count as usize)
            .map(|row| {
                let start = map.row_offsets[row] as usize;
                let end = map.row_offsets[row + 1] as usize;
                (start..end).fold(0_u64, |sum, index| {
                    let coefficient = tiny_scalar_value(
                        Scalar::from_be_bytes_exact(map.coefficients[index]).unwrap(),
                    )
                    .unwrap();
                    (sum + coefficient * input[map.column_indices[index] as usize]) % 17
                })
            })
            .collect()
    }

    fn linear_fold_oracle(left: &[u64], right: &[u64], challenge: u64) -> Vec<u64> {
        left.iter()
            .zip(right)
            .map(|(left, right)| (left + challenge * right) % 17)
            .collect()
    }

    fn quadratic_fold_oracle(
        accumulated: &[u64],
        cross: &[u64],
        incoming: &[u64],
        challenge: u64,
        challenge_squared: u64,
    ) -> Vec<u64> {
        accumulated
            .iter()
            .zip(cross)
            .zip(incoming)
            .map(|((accumulated, cross), incoming)| {
                (accumulated + challenge * cross + challenge_squared * incoming) % 17
            })
            .collect()
    }

    #[test]
    fn canonical_sparse_csr_wire_roundtrip_and_digest_are_exact() {
        let map = sample_map();
        let bytes = map.to_canonical_bytes().unwrap();
        assert_eq!(
            bytes.len(),
            PHASE23_SPARSE_MAP_WIRE_HEADER_BYTES_V1
                + (map.row_count as usize + 1) * 4
                + map.column_indices.len() * 36
                + 32
        );
        assert_eq!(
            ZkAmsPhase23SparseMapV1::from_canonical_bytes(&bytes),
            Ok(map.clone())
        );
        assert_ne!(map.digest, [0; 32]);
        let status = zk_ams_phase23_encrypted_implementation_v1();
        assert_ne!(status.algebra_digest, [0; 32]);
        assert_ne!(status.digest, [0; 32]);
        assert_eq!(status.release_kat_digest, [0; 32]);
        assert!(!status.release_kat_complete);
    }

    #[test]
    fn malformed_csr_noncanonical_coefficients_and_resource_bombs_fail_before_use() {
        let baseline = sample_map();
        let invalid_mutations: Vec<Box<dyn Fn(&mut ZkAmsPhase23SparseMapV1)>> = vec![
            Box::new(|map| map.version = 2),
            Box::new(|map| map.row_count = 0),
            Box::new(|map| map.column_count = 0),
            Box::new(|map| map.max_row_fan_in = 0),
            Box::new(|map| map.row_offsets[0] = 1),
            Box::new(|map| map.row_offsets[2] = map.row_offsets[1] - 1),
            Box::new(|map| *map.row_offsets.last_mut().unwrap() -= 1),
            Box::new(|map| map.column_indices[1] = map.column_indices[0]),
            Box::new(|map| map.column_indices[1] = 0),
            Box::new(|map| map.column_indices[0] = map.column_count),
            Box::new(|map| map.coefficients[0] = [0; 32]),
            Box::new(|map| map.coefficients[0] = VEGA_T256_SCALAR_MODULUS_BE_V1),
            Box::new(|map| map.digest[0] ^= 1),
        ];
        for mutate in invalid_mutations {
            let mut invalid = baseline.clone();
            mutate(&mut invalid);
            assert!(validate_sparse_map(&invalid).is_err());
        }

        let bytes = baseline.to_canonical_bytes().unwrap();
        for length in [0, 1, 17, bytes.len() - 1] {
            assert!(ZkAmsPhase23SparseMapV1::from_canonical_bytes(&bytes[..length]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            ZkAmsPhase23SparseMapV1::from_canonical_bytes(&trailing),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        let mut bad_kind = bytes.clone();
        bad_kind[1] = 0xff;
        assert_eq!(
            ZkAmsPhase23SparseMapV1::from_canonical_bytes(&bad_kind),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        let mut resource_bomb = bytes;
        resource_bomb[14..18]
            .copy_from_slice(&(ZK_AMS_PHASE23_MAX_CANONICAL_SPARSE_ENTRIES_V1 + 1).to_be_bytes());
        assert_eq!(
            ZkAmsPhase23SparseMapV1::from_canonical_bytes(&resource_bomb),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );
    }

    #[test]
    fn tiny_conjugate_slot_packing_multiplies_and_rotates_as_the_ciphertext_oracle_requires() {
        let profile = test_profile();
        let slots = vec![s(1), s(2), s(4), s(8)];
        let encoded = encode_tiny_slots_to_rns(&profile, &slots).unwrap();
        let coefficients = super::super::reduce_test_polynomial(&profile, &encoded).unwrap();
        assert_eq!(
            decode_tiny_slots_from_coefficients(&profile, &coefficients).unwrap(),
            slots
        );
        let squared = encoded.mul(&encoded, &profile).unwrap();
        let coefficients = super::super::reduce_test_polynomial(&profile, &squared).unwrap();
        assert_eq!(
            decode_tiny_slots_from_coefficients(&profile, &coefficients).unwrap(),
            vec![s(1), s(4), s(16), s(13)]
        );
        for shift in 1..4 {
            let transformed = encoded
                .automorphism(rotation_exponent(&profile, shift).unwrap(), &profile)
                .unwrap();
            let coefficients =
                super::super::reduce_test_polynomial(&profile, &transformed).unwrap();
            let decoded = decode_tiny_slots_from_coefficients(&profile, &coefficients).unwrap();
            assert_eq!(
                decoded,
                (0..4)
                    .map(|slot| slots[(slot + shift) % 4])
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn signed_binary_rotation_preflights_the_complete_work() {
        for slots in [2, 4, 8, 16, 32, 256, 65_536] {
            let observed = (1..slots)
                .map(|shift| {
                    let (_, decomposition) =
                        canonical_slot_shift_decomposition(slots, shift).unwrap();
                    usize::try_from(decomposition.count_ones()).unwrap()
                })
                .max()
                .unwrap();
            assert_eq!(
                observed,
                super::super::phase23_max_composed_rotation_key_switch_count(slots).unwrap()
            );
        }

        let base = test_profile();
        let party_a = ZkAmsMkhePartyIdV1::new([1; 32]).unwrap();
        let party_b = ZkAmsMkhePartyIdV1::new([2; 32]).unwrap();
        let roster = PartySet::singleton(party_a)
            .union(&PartySet::singleton(party_b))
            .unwrap();

        let rotation_multiplications =
            phase23_rotation_ring_multiplication_count(&base, roster.parties.len(), 1).unwrap();
        let ring_work = super::super::ring_multiplication_work(&base).unwrap();
        let rotation_work = ring_work * u64::try_from(rotation_multiplications).unwrap();

        let mut below_rotation = base.clone();
        below_rotation.max_work_units = rotation_work - 1;
        let below_binding = test_binding(&below_rotation, &roster, [3; 32], [4; 32]);
        let below_ciphertext = zero_ciphertext(&below_rotation, &roster, 0).unwrap();
        assert_eq!(
            rotate_ciphertext_by_slot_shift(
                &below_rotation,
                below_binding,
                &below_ciphertext,
                1,
                &[],
            ),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );

        let mut exact_rotation = base.clone();
        exact_rotation.max_work_units = rotation_work;
        let exact_binding = test_binding(&exact_rotation, &roster, [3; 32], [4; 32]);
        let exact_ciphertext = zero_ciphertext(&exact_rotation, &roster, 0).unwrap();
        assert_eq!(
            rotate_ciphertext_by_slot_shift(
                &exact_rotation,
                exact_binding,
                &exact_ciphertext,
                1,
                &[],
            ),
            Err(ZkAmsMkheErrorV1::MissingEvaluatedKey)
        );
    }

    #[test]
    fn materialized_six_family_wire_is_canonical_and_mutation_closed() {
        let shape = ZkAmsPhase23AccumulatorShapeV1::new(2, 6, 3, 5, 2).unwrap();
        let materialized = materialized_from_values(
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            2,
            shape,
            vec![s(1).to_be_bytes(), s(2).to_be_bytes()],
            vec![s(3).to_be_bytes()],
            (4..10).map(|value| s(value).to_be_bytes()).collect(),
            (10..13).map(|value| s(value).to_be_bytes()).collect(),
            (1..6).map(|value| s(value).to_be_bytes()).collect(),
            vec![s(6).to_be_bytes(), s(7).to_be_bytes()],
        )
        .unwrap();
        let bytes = materialized.to_canonical_bytes().unwrap();
        assert_eq!(
            ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(&bytes),
            Ok(materialized.clone())
        );
        let mut corrupt_digest = bytes.clone();
        *corrupt_digest.last_mut().unwrap() ^= 1;
        assert!(
            ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(&corrupt_digest).is_err()
        );
        let mut noncanonical = bytes.clone();
        let first_value = PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1;
        noncanonical[first_value..first_value + 32]
            .copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
        assert!(
            ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(&noncanonical).is_err()
        );
        let mut wrong_u_length = bytes.clone();
        let u_length_offset = 1 + 5 * 32 + 1 + 4;
        wrong_u_length[u_length_offset..u_length_offset + 4].copy_from_slice(&2_u32.to_be_bytes());
        assert!(
            ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(&wrong_u_length).is_err()
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(&trailing).is_err());
    }

    #[test]
    fn replicated_u_rejects_mismatched_slots_chunks_lengths_and_legacy_scalar_shape() {
        let u = s(3).to_be_bytes();
        assert_eq!(collapse_replicated_u_values(vec![u; 8], 8), Ok(vec![u]));

        let mut mismatched_slot = vec![u; 8];
        mismatched_slot[2] = s(4).to_be_bytes();
        assert_eq!(
            collapse_replicated_u_values(mismatched_slot, 8),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut mismatched_chunk = vec![u; 8];
        mismatched_chunk[4] = s(5).to_be_bytes();
        assert_eq!(
            collapse_replicated_u_values(mismatched_chunk, 8),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            collapse_replicated_u_values(vec![u; 4], 8),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert_eq!(
            collapse_replicated_u_values(vec![u], 8),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
            "the pre-release single-slot U shape must not be accepted"
        );

        let profile = test_profile();
        let mut random = KatRandom::new(b"phase23-replicated-u-negative-kat");
        let keys = TestKeys::generate(&profile, &mut random);
        let binding = test_binding(&profile, &keys.roster, [0x81; 32], [0x82; 32]);
        let x = [2, 5];
        let scalar_u = [3];
        let e = [1, 2, 3, 4, 5, 6];
        let r_e = [4, 7, 2];
        let w = [1, 4, 6, 8, 3];
        let r_w = [9, 2];
        let valid = make_state(
            &profile,
            binding,
            [&x, &scalar_u, &e, &r_e, &w, &r_w],
            false,
            &[2, 4, 6, 8],
            &[1, 3, 5, 7],
            &keys,
            &mut random,
        );
        validate_accumulator_state(&profile, binding, &valid).unwrap();
        assert_eq!(valid.u.logical_value_count, valid.e.logical_value_count);
        assert_eq!(valid.u.chunks.len(), 2);
        assert_eq!(valid.u.chunks[0], valid.u.chunks[1]);

        let mut different_ciphertext_chunk = valid.clone();
        different_ciphertext_chunk.u.chunks[1] =
            zero_ciphertext(&profile, &keys.roster, 0).unwrap();
        assert_eq!(
            validate_accumulator_state(&profile, binding, &different_ciphertext_chunk),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut wrong_logical_length = valid.clone();
        wrong_logical_length.u.logical_value_count = 5;
        wrong_logical_length.u.digest =
            encrypted_vector_digest(&profile, &wrong_logical_length.u).unwrap();
        assert_eq!(
            validate_accumulator_state(&profile, binding, &wrong_logical_length),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut legacy_single_slot = valid.clone();
        legacy_single_slot.u = EncryptedPackedVector::new(
            &profile,
            binding,
            EncryptedFamily::U,
            1,
            vec![valid.u.chunks[0].clone()],
        )
        .unwrap();
        assert_eq!(
            validate_accumulator_state(&profile, binding, &legacy_single_slot),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    #[test]
    fn encrypted_sparse_equations_6_7_and_9_11_match_independent_two_party_scalar_oracle() {
        let profile = test_profile();
        let mut random = KatRandom::new(b"phase23-encrypted-complete-kat");
        let keys = TestKeys::generate(&profile, &mut random);
        let provisional = test_binding(&profile, &keys.roster, [0x91; 32], [0x92; 32]);

        let acc_x = [2, 5];
        let acc_u = [3];
        let acc_e = [1, 2, 3, 4, 5, 6];
        let acc_re = [4, 7, 2];
        let acc_w = [1, 4, 6, 8, 3];
        let acc_rw = [9, 2];
        let in_x = [7, 1];
        let in_u = [1];
        let in_e = [6, 5, 4, 3, 2, 1];
        let in_re = [8, 3, 6];
        let in_w = [2, 7, 5, 1, 9];
        let in_rw = [4, 6];
        let accumulated = make_state(
            &profile,
            provisional,
            [&acc_x, &acc_u, &acc_e, &acc_re, &acc_w, &acc_rw],
            false,
            &[2, 4, 6, 8],
            &[1, 3, 5, 7],
            &keys,
            &mut random,
        );
        let incoming = make_state(
            &profile,
            provisional,
            [&in_x, &in_u, &in_e, &in_re, &in_w, &in_rw],
            true,
            &[9, 11, 13, 15],
            &[8, 6, 4, 2],
            &keys,
            &mut random,
        );
        let accumulated_digest =
            accumulator_state_digest(&profile, provisional, &accumulated).unwrap();
        let incoming_digest = accumulator_state_digest(&profile, provisional, &incoming).unwrap();
        let binding = test_binding(&profile, &keys.roster, accumulated_digest, incoming_digest);
        validate_accumulator_state(&profile, binding, &accumulated).unwrap();
        validate_accumulator_state(&profile, binding, &incoming).unwrap();

        let map_a = sample_map();
        let map_b = sparse_map(
            ZkAmsPhase23MapKindV1::B,
            8,
            &[
                vec![(1, 1), (6, 2)],
                vec![(0, 4), (5, 1)],
                vec![(2, 3), (7, 2)],
                vec![(4, 5)],
                vec![(3, 1), (6, 1)],
                vec![(1, 2), (5, 3)],
            ],
        );
        let map_c = sparse_map(
            ZkAmsPhase23MapKindV1::C,
            8,
            &[
                vec![(0, 1), (7, 1)],
                vec![(2, 2), (5, 4)],
                vec![(1, 5)],
                vec![(3, 3), (6, 2)],
                vec![(4, 1), (5, 2)],
                vec![(0, 6), (7, 3)],
            ],
        );
        let (galois_keys, product_keys) = evaluation_keys(&profile, binding, &keys, &mut random);
        let schedule_bits = slots_per_chunk(&profile).unwrap().trailing_zeros() as usize;
        assert_eq!(galois_keys.len(), 2 * (2 * schedule_bits - 1));
        assert_eq!(canonical_slot_shift_decomposition(4, 3), Ok((true, 1)));
        assert_eq!(canonical_slot_shift_decomposition(16, 7), Ok((true, 9)));

        let acc_z = acc_w
            .iter()
            .chain(&acc_x)
            .chain(&acc_u)
            .copied()
            .collect::<Vec<_>>();
        let in_z = in_w
            .iter()
            .chain(&in_x)
            .chain(&in_u)
            .copied()
            .collect::<Vec<_>>();
        let encrypted_az = evaluate_sparse_map(
            &profile,
            binding,
            &map_a,
            &[&accumulated.w, &accumulated.x, &accumulated.u],
            EncryptedFamily::AZ,
            &galois_keys,
        )
        .unwrap();
        assert_eq!(
            decrypt_collective_vector(&profile, &encrypted_az, &keys).unwrap(),
            evaluate_sparse_oracle(&map_a, &acc_z)
        );

        let cross = encrypted_equation_6(
            &profile,
            binding,
            [&map_a, &map_b, &map_c],
            &accumulated,
            &incoming,
            &galois_keys,
            &product_keys,
        )
        .unwrap();
        let az_acc = evaluate_sparse_oracle(&map_a, &acc_z);
        let bz_acc = evaluate_sparse_oracle(&map_b, &acc_z);
        let cz_acc = evaluate_sparse_oracle(&map_c, &acc_z);
        let az_in = evaluate_sparse_oracle(&map_a, &in_z);
        let bz_in = evaluate_sparse_oracle(&map_b, &in_z);
        let cz_in = evaluate_sparse_oracle(&map_c, &in_z);
        let cross_oracle = (0..map_a.row_count as usize)
            .map(|index| {
                (az_acc[index] * bz_in[index] + az_in[index] * bz_acc[index] + 17
                    - acc_u[0] * cz_in[index] % 17
                    + 17
                    - in_u[0] * cz_acc[index] % 17)
                    % 17
            })
            .collect::<Vec<_>>();
        let decrypted_cross = decrypt_collective_vector(&profile, &cross, &keys).unwrap();
        assert_eq!(decrypted_cross, cross_oracle);
        assert_eq!(cross.chunks[0].level, 1);

        let r_t_values = [3, 10, 12];
        let r_t = encrypt_collective_vector(
            &profile,
            binding,
            EncryptedFamily::CrossTermRandomness,
            &r_t_values,
            &keys,
            &mut random,
        );
        let g_map = sparse_map(
            ZkAmsPhase23MapKindV1::CommitmentG,
            3,
            &[
                vec![(0, 1), (2, 2)],
                vec![(1, 3)],
                vec![(0, 4)],
                vec![(2, 5)],
            ],
        );
        let h_map = sparse_map(
            ZkAmsPhase23MapKindV1::CommitmentH,
            6,
            &[
                vec![(0, 2), (4, 1)],
                vec![(1, 3), (5, 2)],
                vec![(2, 4)],
                vec![(3, 5), (4, 2)],
            ],
        );
        let committed = encrypted_equation_7(
            &profile,
            binding,
            &g_map,
            &h_map,
            &cross,
            &r_t,
            &galois_keys,
        )
        .unwrap();
        let public_cross_commitment =
            decrypt_collective_vector(&profile, &committed.encrypted_commitment, &keys).unwrap();
        let g_oracle = evaluate_sparse_oracle(&g_map, &r_t_values);
        let h_oracle = evaluate_sparse_oracle(&h_map, &cross_oracle);
        assert_eq!(
            public_cross_commitment,
            g_oracle
                .iter()
                .zip(h_oracle)
                .map(|(left, right)| (left + right) % 17)
                .collect::<Vec<_>>()
        );
        let public_cross_scalars = public_cross_commitment
            .iter()
            .copied()
            .map(s)
            .collect::<Vec<_>>();
        let challenge_context = ZkAmsPhase23ChallengeContextV1 {
            batch_id: binding.batch_id,
            nifs_verifier_digest: binding.nifs_verifier_digest,
            ordered_batch_input_digest: binding.ordered_batch_input_digest,
            accumulated_error_commitment_digest: commitment_vector_digest(
                b"iroha.zk-ams.v1.phase23.error-commitment",
                &accumulated.e_commitment,
            )
            .unwrap(),
            accumulated_witness_commitment_digest: commitment_vector_digest(
                b"iroha.zk-ams.v1.phase23.witness-commitment",
                &accumulated.w_commitment,
            )
            .unwrap(),
            incoming_error_commitment_digest: commitment_vector_digest(
                b"iroha.zk-ams.v1.phase23.error-commitment",
                &incoming.e_commitment,
            )
            .unwrap(),
            incoming_witness_commitment_digest: commitment_vector_digest(
                b"iroha.zk-ams.v1.phase23.witness-commitment",
                &incoming.w_commitment,
            )
            .unwrap(),
            cross_term_commitment_digest: commitment_vector_digest(
                b"iroha.zk-ams.v1.phase23.cross-term-commitment",
                &public_cross_scalars,
            )
            .unwrap(),
            fold_index: binding.fold_index,
        };
        let challenge = zk_ams_phase23_challenge_v1(challenge_context).unwrap();
        let tiny_challenge = tiny_scalar_value(challenge).unwrap();
        let tiny_challenge_squared = tiny_scalar_value(challenge.square()).unwrap();
        assert_ne!(
            tiny_challenge, 0,
            "the pinned KAT must exercise a nonzero tiny challenge"
        );
        let folded = fold_encrypted_accumulators(
            &profile,
            binding,
            &accumulated,
            &incoming,
            &committed,
            &public_cross_scalars,
            challenge_context,
        )
        .unwrap();
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.x, &keys).unwrap(),
            linear_fold_oracle(&acc_x, &in_x, tiny_challenge)
        );
        let expected_u = linear_fold_oracle(&acc_u, &in_u, tiny_challenge)[0];
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.u, &keys).unwrap(),
            vec![expected_u; acc_e.len()]
        );
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.e, &keys).unwrap(),
            quadratic_fold_oracle(
                &acc_e,
                &cross_oracle,
                &in_e,
                tiny_challenge,
                tiny_challenge_squared,
            )
        );
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.r_e, &keys).unwrap(),
            quadratic_fold_oracle(
                &acc_re,
                &r_t_values,
                &in_re,
                tiny_challenge,
                tiny_challenge_squared,
            )
        );
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.w, &keys).unwrap(),
            linear_fold_oracle(&acc_w, &in_w, tiny_challenge)
        );
        assert_eq!(
            decrypt_collective_vector(&profile, &folded.r_w, &keys).unwrap(),
            linear_fold_oracle(&acc_rw, &in_rw, tiny_challenge)
        );
        assert_eq!(folded.e.chunks[0].level, 1);
        assert_eq!(folded.r_e.chunks[0].level, 0);

        let x = decrypt_collective_vector(&profile, &folded.x, &keys).unwrap();
        let u = decrypt_collective_vector(&profile, &folded.u, &keys).unwrap();
        let e = decrypt_collective_vector(&profile, &folded.e, &keys).unwrap();
        let r_e = decrypt_collective_vector(&profile, &folded.r_e, &keys).unwrap();
        let w = decrypt_collective_vector(&profile, &folded.w, &keys).unwrap();
        let r_w = decrypt_collective_vector(&profile, &folded.r_w, &keys).unwrap();
        let shape = ZkAmsPhase23AccumulatorShapeV1::new(2, 6, 3, 5, 2).unwrap();
        let materialized_u = collapse_replicated_u_values(
            u.iter().copied().map(s).map(Scalar::to_be_bytes).collect(),
            shape.e,
        )
        .unwrap();
        let materialized = materialized_from_values(
            binding.profile_digest,
            binding.roster_digest,
            binding.transcript_digest,
            binding.batch_id,
            binding.ordered_batch_input_digest,
            binding.fold_index,
            shape,
            x.iter().copied().map(s).map(Scalar::to_be_bytes).collect(),
            materialized_u,
            e.iter().copied().map(s).map(Scalar::to_be_bytes).collect(),
            r_e.iter()
                .copied()
                .map(s)
                .map(Scalar::to_be_bytes)
                .collect(),
            w.iter().copied().map(s).map(Scalar::to_be_bytes).collect(),
            r_w.iter()
                .copied()
                .map(s)
                .map(Scalar::to_be_bytes)
                .collect(),
        )
        .unwrap();
        assert_eq!(
            ZkAmsPhase23MaterializedAccumulatorsV1::from_canonical_bytes(
                &materialized.to_canonical_bytes().unwrap()
            ),
            Ok(materialized.clone())
        );

        // Missing, duplicated, or transcript-spliced evaluated keys must never
        // trigger a plaintext or partial-roster fallback.
        assert!(
            evaluate_sparse_map(
                &profile,
                binding,
                &map_a,
                &[&accumulated.w, &accumulated.x, &accumulated.u],
                EncryptedFamily::AZ,
                &[],
            )
            .is_err()
        );
        let mut duplicate_galois_keys = galois_keys.clone();
        duplicate_galois_keys.extend(galois_keys.clone());
        assert!(
            evaluate_sparse_map(
                &profile,
                binding,
                &map_a,
                &[&accumulated.w, &accumulated.x, &accumulated.u],
                EncryptedFamily::AZ,
                &duplicate_galois_keys,
            )
            .is_err()
        );
        let mut spliced_galois_keys = galois_keys.clone();
        for key in &mut spliced_galois_keys {
            key.transcript_digest[0] ^= 1;
        }
        assert!(
            evaluate_sparse_map(
                &profile,
                binding,
                &map_a,
                &[&accumulated.w, &accumulated.x, &accumulated.u],
                EncryptedFamily::AZ,
                &spliced_galois_keys,
            )
            .is_err()
        );
        assert!(
            encrypted_equation_6(
                &profile,
                binding,
                [&map_a, &map_b, &map_c],
                &accumulated,
                &incoming,
                &galois_keys,
                &product_keys[..2],
            )
            .is_err()
        );
        let mut duplicate_product_keys = product_keys.clone();
        duplicate_product_keys.extend(product_keys.clone());
        assert!(
            encrypted_equation_6(
                &profile,
                binding,
                [&map_a, &map_b, &map_c],
                &accumulated,
                &incoming,
                &galois_keys,
                &duplicate_product_keys,
            )
            .is_err()
        );
        let mut spliced_product_keys = product_keys.clone();
        for key in &mut spliced_product_keys {
            key.transcript_digest[0] ^= 1;
        }
        assert!(
            encrypted_equation_6(
                &profile,
                binding,
                [&map_a, &map_b, &map_c],
                &accumulated,
                &incoming,
                &galois_keys,
                &spliced_product_keys,
            )
            .is_err()
        );

        // Session, fold, state, and Fiat--Shamir replay/substitution attempts
        // are rejected even when each substituted object is otherwise valid.
        let different_batch_binding = ZkAmsPhase23EncryptedBindingV1::new(
            binding.profile_digest,
            binding.roster_digest,
            binding.transcript_digest,
            [0x73; 32],
            binding.nifs_verifier_digest,
            binding.ordered_batch_input_digest,
            binding.accumulated_state_digest,
            binding.incoming_state_digest,
            binding.fold_index,
        )
        .unwrap();
        assert!(
            validate_accumulator_state(&profile, different_batch_binding, &accumulated).is_err()
        );
        let different_fold_binding = ZkAmsPhase23EncryptedBindingV1::new(
            binding.profile_digest,
            binding.roster_digest,
            binding.transcript_digest,
            binding.batch_id,
            binding.nifs_verifier_digest,
            binding.ordered_batch_input_digest,
            binding.accumulated_state_digest,
            binding.incoming_state_digest,
            binding.fold_index + 1,
        )
        .unwrap();
        assert!(
            validate_accumulator_state(&profile, different_fold_binding, &accumulated).is_err()
        );
        assert!(
            encrypted_equation_6(
                &profile,
                binding,
                [&map_a, &map_b, &map_c],
                &incoming,
                &accumulated,
                &galois_keys,
                &product_keys,
            )
            .is_err()
        );
        assert!(
            fold_encrypted_accumulators(
                &profile,
                binding,
                &incoming,
                &accumulated,
                &committed,
                &public_cross_scalars,
                challenge_context,
            )
            .is_err()
        );
        let mut tampered_accumulated = accumulated.clone();
        tampered_accumulated.x.digest[0] ^= 1;
        assert!(
            fold_encrypted_accumulators(
                &profile,
                binding,
                &tampered_accumulated,
                &incoming,
                &committed,
                &public_cross_scalars,
                challenge_context,
            )
            .is_err()
        );
        let mut replayed_context = challenge_context;
        replayed_context.cross_term_commitment_digest[0] ^= 1;
        assert!(
            fold_encrypted_accumulators(
                &profile,
                binding,
                &accumulated,
                &incoming,
                &committed,
                &public_cross_scalars,
                replayed_context,
            )
            .is_err()
        );
        let mut substituted_public_commitment = public_cross_scalars.clone();
        substituted_public_commitment[0] += s(1);
        assert!(
            fold_encrypted_accumulators(
                &profile,
                binding,
                &accumulated,
                &incoming,
                &committed,
                &substituted_public_commitment,
                challenge_context,
            )
            .is_err()
        );

        let wrong_dimension_map = sparse_map(
            ZkAmsPhase23MapKindV1::A,
            7,
            &[
                vec![(0, 1)],
                vec![(1, 1)],
                vec![(2, 1)],
                vec![(3, 1)],
                vec![(4, 1)],
                vec![(5, 1)],
            ],
        );
        assert!(
            evaluate_sparse_map(
                &profile,
                binding,
                &wrong_dimension_map,
                &[&accumulated.w, &accumulated.x, &accumulated.u],
                EncryptedFamily::AZ,
                &galois_keys,
            )
            .is_err()
        );
        let padded = encrypt_collective_vector(
            &profile,
            binding,
            EncryptedFamily::X,
            &[1, 9],
            &keys,
            &mut random,
        );
        let nonzero_padding =
            EncryptedPackedVector::new(&profile, binding, EncryptedFamily::X, 1, padded.chunks)
                .unwrap();
        assert_eq!(
            decrypt_collective_vector(&profile, &nonzero_padding, &keys),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let mut kat = Keccak256::new();
        kat.update(b"iroha.zk-ams.v1.phase23.encrypted-tiny-complete-kat");
        for map in [&map_a, &map_b, &map_c, &g_map, &h_map] {
            kat.update(&map.digest);
        }
        for values in [
            &decrypted_cross,
            &public_cross_commitment,
            &x,
            &u,
            &e,
            &r_e,
            &w,
            &r_w,
        ] {
            kat.update(&(values.len() as u32).to_be_bytes());
            for value in values {
                kat.update(&value.to_be_bytes());
            }
        }
        kat.update(&materialized.digest);
        assert_eq!(
            kat.finalize(),
            [
                62, 190, 250, 154, 107, 168, 20, 80, 59, 34, 205, 32, 194, 3, 115, 133, 219, 184,
                176, 147, 16, 127, 141, 96, 41, 69, 239, 167, 223, 43, 124, 181,
            ],
            "the independently checked two-party encrypted Phase-II/III KAT drifted"
        );
    }
}
