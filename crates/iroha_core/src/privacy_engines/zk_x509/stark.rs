//! Canonical bounded segmented transparent proof container for zk-X509 AIRs.
//!
//! This module is deliberately below the governed engine boundary. It
//! implements the complete commitment, quotient, Merkle-opening, binary-FRI,
//! grinding, and exact-codec machinery for the verifier-fixed cross-segment
//! byte-memory table and the numeric output-projection AIR.
//!
//! The protocol order is fixed:
//!
//! 1. bind the exact public channel declarations;
//! 2. commit every masked base column;
//! 3. derive every independent relation-specific copy challenge;
//! 4. construct and commit every masked auxiliary product column;
//! 5. derive composition coefficients and commit quotient lanes;
//! 6. derive one shared DEEP point and bind current/next Fp4 openings;
//! 7. derive mixes over every DEEP quotient;
//! 8. commit each FRI layer before deriving its fold challenge;
//! 9. grind the completed commitment transcript;
//! 10. derive 58 unique query positions from the post-grinding transcript.
//!
//! Proof dimensions are reconstructed from the verifier statement.  The wire
//! contains no caller-selected parameter and the strict reader rejects every
//! truncation and trailing suffix.

mod main_aggregate;

#[cfg(test)]
use main_aggregate::{
    MainOpenedProviderSetV1, MainOpenedRowEvaluatorV1, MainTraceColumnKindV1,
    MainTracePolynomialSetV1, MainTraceProviderSetV1, P256OpenedRowEvaluatorV1,
    add_main_composition_coefficient_chunks_v1, main_opened_composition_value_v1,
    record_main_group_commitment_v1, validate_main_fri_mixes_v1,
};
use main_aggregate::{
    ProjectionOpenedRowEvaluatorV1, p256_opened_residues_v1, p256_scalar_opened_residues_v1,
};
#[allow(unused_imports)]
pub(crate) use main_aggregate::{
    ZkX509MainAwaitingCredentialBindingV1, ZkX509MainCompositionPhaseV1,
    commit_zk_x509_main_base_phase_v1_with_rng, verify_zk_x509_main_aggregate_stark_v1,
    zk_x509_main_pre_aux_from_proof_v1,
};

use std::collections::BTreeMap;

use iroha_data_model::privacy::{IrohaZkX509StarkP256StatementV1, PrivacyStatementV1};
use rand::TryRngCore;
use thiserror::Error;

use super::{
    accumulator_air::{
        ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1, ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
        ZkX509CaAccumulatorTraceV1,
    },
    accumulator_stark::{
        ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1, ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_CHUNKS_V1, ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1,
        ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1, ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1, ZkX509AccumulatorStarkErrorV1,
        ZkX509CaAccumulatorStarkMaterialV1, ZkX509CaAccumulatorStarkPublicV1,
        ZkX509CaAccumulatorStarkTerminalClaimsV1, build_ca_accumulator_stark_material_v1,
        ca_accumulator_stark_public_v1, compile_ca_accumulator_fixed_row_v1,
        evaluate_ca_accumulator_stark_residues_v1,
    },
    credential_pre_aux::{
        ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1, ZkX509CredentialMainPostBaseChallengesV1,
        ZkX509CredentialMainPreAuxV1, ZkX509CredentialPreAuxBindingV1,
        absorb_zk_x509_credential_pre_aux_binding_v1,
    },
    credential_stark::{
        ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1, ZkX509CredentialPublicBindingV1,
        ZkX509MainCaBindingV1,
    },
    der_air::ZkX509Rfc5280StatementV1,
    der_stark::{
        FIX_ACTIVE as DER_FIX_ACTIVE, FIX_COMPARATOR, FIX_FINAL_DOCUMENT, FIX_FIRST_ACTIVE,
        FIX_FIRST_AGGREGATE, FIX_FIRST_COMPARATOR, FIX_FIRST_PARSER,
        FIX_LAST_ACTIVE as DER_FIX_LAST_ACTIVE, FIX_LAST_AGGREGATE, FIX_LAST_COMPARATOR,
        FIX_LAST_PARSER, FIX_PADDING, FIX_PARSER, FIX_PARSER_CONTINUE,
        ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1, ZK_X509_DER_STARK_AUX_WIDTH_V1,
        ZK_X509_DER_STARK_BASE_WIDTH_V1, ZK_X509_DER_STARK_BUS_LANES_V1,
        ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1, ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1, ZK_X509_DER_STARK_FIXED_WIDTH_V1,
        ZK_X509_DER_STARK_MAXIMUM_QUOTIENT_DEGREE_V1, ZK_X509_DER_STARK_TRACE_LOG2_V1,
        ZK_X509_DER_STARK_TRACE_SIZE_V1, ZkX509DerStarkBaseV1, ZkX509DerStarkChallengesV1,
        ZkX509DerStarkErrorV1, ZkX509DerStarkFixedScheduleV1, ZkX509DerStarkPublicTerminalsV1,
        ZkX509DerStarkShapeV1, ZkX509DerStarkTerminalClaimsV1, ZkX509DerStarkTraceV1,
        build_zk_x509_der_stark_base_v1, build_zk_x509_der_stark_native_aux_column_v1,
        build_zk_x509_der_stark_native_base_column_v1,
        build_zk_x509_der_stark_native_fixed_column_v1, build_zk_x509_der_stark_trace_v1,
        compile_zk_x509_der_stark_fixed_schedule_v1, derive_zk_x509_der_stark_challenges_v1,
        derive_zk_x509_der_stark_public_terminals_v1, evaluate_zk_x509_der_stark_residues_into_v1,
        evaluate_zk_x509_der_stark_residues_v1,
        evaluate_zk_x509_der_stark_terminal_claim_residues_v1,
        zk_x509_der_stark_terminal_claims_v1,
    },
    engine::{construct_zk_x509_compiled_profile_v1, recompute_zk_x509_compiled_profile_digest_v1},
    fixed_algebraic::{
        ZK_X509_FIXED_ALGEBRAIC_MAX_QUERIES_V1, ZkX509FixedAlgebraicErrorV1,
        ZkX509FixedAlgebraicOpeningsV1,
    },
    fixed_algebraic_p256::{
        ZK_X509_P256_FIXED_ALGEBRAIC_WIDTH_V1, ZkX509P256FixedAlgebraicErrorV1,
        zk_x509_p256_fixed_algebraic_row_for_registration_v1,
        zk_x509_p256_fixed_algebraic_schedule_v1,
    },
    fixed_algebraic_sha::{
        ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1, ZkX509ShaFixedAlgebraicErrorV1,
        zk_x509_sha_fixed_algebraic_schedule_v1,
    },
    io_air::{
        IO_PERMUTATION_LANES_V1, IoAccessV1, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1, ZkX509IoAirErrorV1,
        ZkX509IoChallengesV1, ZkX509IoChannelDeclarationV1, ZkX509IoChannelWitnessV1,
        ZkX509IoEndpointV1, ZkX509IoSegmentRoleV1, build_zk_x509_io_base_tables_v1,
        build_zk_x509_io_trace_v1, byte_memory_capacity_v1, derive_zk_x509_io_challenges_v1,
        validate_declarations_v1,
    },
    main_assembly::{ZkX509MainIoBaseMaterialV1, ZkX509MainTraceAssemblyV1},
    main_io::compile_zk_x509_main_io_declarations_v1,
    p256_aggregate_adapter::{
        P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1, P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_TRACE_LOG2_V1, P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1,
        P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1, P256_BINDING_SINK_BASE_WIDTH_V1,
        P256_BINDING_SINK_FIXED_WIDTH_V1, P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1,
        P256_LOW_S_AGGREGATE_AUX_WIDTH_V1, P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1,
        P256_LOW_S_AGGREGATE_TRACE_LOG2_V1, P256_LOW_S_REGISTERED_CONSTRAINT_COUNT_V1,
        P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1, P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
        P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1, P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
        P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1,
        P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1, P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
        P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1, P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
        P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1,
        P256_VALUE_SORTED_REGISTERED_CONSTRAINT_COUNT_V1, P256_WINDOW_AGGREGATE_AUX_WIDTH_V1,
        P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1, P256_WINDOW_AGGREGATE_TRACE_LOG2_V1,
        P256_WINDOW_REGISTERED_CONSTRAINT_COUNT_V1, P256AggregateAdapterErrorV1,
        P256ArithmeticCopyChallengesV1, P256BusTerminalClaimsV1, P256CrossTraceTerminalClaimV1,
        P256CrossTraceTerminalRoleV1, P256MainAdapterV1, P256MainBaseSourceV1,
        P256MainBoundSourceV1, P256MainRegistrationV1, P256MainVerifierFixedSourceV1,
        P256ValueExecutionAggregateChallengesV1, P256WindowAggregateChallengesV1,
        ZK_X509_P256_AGGREGATE_ADAPTER_DESCRIPTOR_SHA256_V1,
        ZK_X509_P256_AGGREGATE_ADAPTER_DESCRIPTOR_V1, absorb_p256_terminal_claims_v1,
        derive_p256_arithmetic_copy_challenges_v1, evaluate_p256_arithmetic_aggregate_residues_v1,
        evaluate_p256_binding_sink_aggregate_residues_v1,
        evaluate_p256_bus_terminal_claim_equalities_v1,
        evaluate_p256_cross_trace_terminal_claim_equalities_v1,
        evaluate_p256_low_s_aggregate_residues_v1, evaluate_p256_reduction_aggregate_residues_v1,
        evaluate_p256_scalar_bit_bus_aggregate_residues_v1,
        evaluate_p256_scalar_source_terminal_openings_v1, evaluate_p256_terminal_claim_binding_v1,
        evaluate_p256_value_execution_aggregate_residues_v1,
        evaluate_p256_window_aggregate_residues_v1, p256_arithmetic_last_selector_v1,
        p256_arithmetic_scalar_terminal_v1, p256_arithmetic_value_copy_terminal_v1,
        p256_binding_sink_last_selector_v1, p256_binding_sink_terminal_v1,
        p256_low_s_cross_terminal_v1, p256_low_s_last_selector_v1,
        p256_reduction_cross_terminal_v1, p256_reduction_last_selector_v1,
        p256_value_execution_arithmetic_copy_terminal_v1, p256_value_execution_cross_terminal_v1,
        p256_value_execution_last_selector_v1, p256_window_cross_terminal_v1,
        p256_window_last_selector_v1, p256_window_scalar_terminal_v1,
        validate_p256_main_registration_order_v1,
    },
    p256_air::P256_ARITHMETIC_BASE_WIDTH_V1,
    p256_cross_trace_bus::{
        P256_CROSS_TRACE_LANES_V1, P256CrossTraceChallengesV1,
        derive_zk_x509_p256_cross_trace_challenges_v1,
    },
    p256_ecdsa_air::P256EcdsaRoleV1,
    p256_reduction_air::{P256_LOW_S_BASE_WIDTH_V1, P256_REDUCTION_BASE_WIDTH_V1},
    p256_scalar_bit_bus::{
        P256_SCALAR_BIT_BUS_LANES_V1, P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1, P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_COUNT_V1,
        P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1, P256ScalarBitBusChallengesV1,
        derive_zk_x509_p256_scalar_bit_bus_challenges_v1, p256_scalar_bit_bus_opened_terminals_v1,
        p256_scalar_bit_bus_stark_last_active_selector_v1,
    },
    p256_value_bus::{
        P256_VALUE_BUS_STARK_AUX_WIDTH_V1, P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
        P256_VALUE_BUS_STARK_FIXED_WIDTH_V1, P256ValueBusChallengesV1,
        derive_zk_x509_p256_value_bus_challenges_v1, evaluate_p256_value_bus_stark_residues_v1,
        p256_value_bus_stark_last_domain_selector_v1, p256_value_bus_stark_opened_terminal_v1,
    },
    p256_window_air::P256_WINDOW_BASE_WIDTH_V1,
    profile::{
        ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1, ZK_X509_CA_FRI_LDE_LOG2_V1,
        ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1, ZK_X509_CA_FRI_TERMINAL_LOG2_V1,
        ZK_X509_CA_TRACE_MASK_DEGREE_V1, ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1,
        ZK_X509_COMPOSITION_LANES_V1, ZK_X509_FRI_BLOWUP_FACTOR_V1,
        ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1, ZK_X509_FRI_QUERY_COUNT_V1,
        ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1, ZK_X509_GRINDING_BITS_V1,
        ZK_X509_LOGICAL_REGISTRATIONS_V1, ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1,
        ZK_X509_MAIN_COMMON_LDE_LOG2_V1, ZK_X509_MAX_CONSTRAINT_DEGREE_V1,
        ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, ZK_X509_MAX_PROOF_BYTES_V1,
        ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1, ZK_X509_PHYSICAL_COMMITMENT_CHUNKS_V1,
        ZK_X509_PROOF_VERSION_V1, ZK_X509_SUITE_V1, ZK_X509_TRACE_GROUPS_V1,
        ZK_X509_TRACE_MASK_DEGREE_V1,
    },
    projection_air::{
        ZK_X509_PROJECTION_AIR_DESCRIPTOR_V1, ZK_X509_PROJECTION_AUX_WIDTH_V1,
        ZK_X509_PROJECTION_BASE_WIDTH_V1, ZK_X509_PROJECTION_CHALLENGE_LABELS_V1,
        ZK_X509_PROJECTION_COPY_LANES_V1, ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1,
        ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1, ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1,
        ZK_X509_PROJECTION_TRACE_SIZE_V1, ZkX509ProjectionAirErrorV1, ZkX509ProjectionAuxTraceV1,
        ZkX509ProjectionChallengesV1, ZkX509ProjectionCompactionChallengesV1,
        ZkX509ProjectionCopyChallengesV1, ZkX509ProjectionTraceV1, ZkX509ProjectionWitnessV1,
        build_zk_x509_projection_aux_trace_v1, build_zk_x509_projection_trace_v1,
        compile_zk_x509_projection_fixed_trace_v1, compile_zk_x509_projection_stark_fixed_rows_v1,
        evaluate_zk_x509_projection_stark_residues_v1,
    },
    rfc5280_stark::{
        ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1, ZK_X509_RFC5280_STARK_AUX_WIDTH_V1,
        ZK_X509_RFC5280_STARK_BASE_WIDTH_V1, ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1,
        ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1, ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1,
        ZK_X509_RFC5280_STARK_TRACE_LOG2_V1, ZK_X509_RFC5280_STARK_TRACE_SIZE_V1,
        ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1, ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1,
        ZkX509P256TerminalClaimsV1, ZkX509Rfc5280OutputRoleV1, ZkX509Rfc5280StarkAuxRowV1,
        ZkX509Rfc5280StarkBaseMaterialV1, ZkX509Rfc5280StarkBaseRowV1,
        ZkX509Rfc5280StarkChallengesV1, ZkX509Rfc5280StarkColumnProviderV1,
        ZkX509Rfc5280StarkFixedRowV1, ZkX509Rfc5280StarkFixedScheduleV1, ZkX509Rfc5280StarkShapeV1,
        ZkX509Rfc5280StarkTerminalClaimsV1, ZkX509ShaSegmentTerminalClaimsV1,
        compile_zk_x509_rfc5280_stark_fixed_schedule_v1,
        evaluate_zk_x509_rfc5280_stark_residues_v1,
        validate_zk_x509_der_rfc_terminal_equalities_v1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_BATCH_AUX_WIDTH_V1, ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1,
        ZK_X509_SHA_BATCH_BASE_WIDTH_V1, ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1,
        ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1,
        ZK_X509_SHA_CA_CALL_COUNT_V1, ZK_X509_SHA_CA_LEAF_CALL_V1,
        ZK_X509_SHA_CA_NODE_CALL_START_V1, ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1,
        ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1, ZK_X509_SHA_SEGMENT_COUNT_V1,
        ZkX509ShaBatchFixedProviderV1, ZkX509ShaBatchRowV1, ZkX509ShaBatchSegmentAuxSourceV1,
        ZkX509ShaBatchSegmentBaseSourceV1, ZkX509ShaCallActivationV1,
        ZkX509ShaCallBoundaryTerminalV1, ZkX509ShaCallBusChallengesV1, ZkX509ShaCallPublicShapeV1,
        ZkX509ShaCallRoleV1, ZkX509ShaCallScheduleV1, ZkX509ShaCallWitnessV1,
        ZkX509ShaSegmentTerminalV1, evaluate_zk_x509_sha_batch_residues_v1,
    },
};
#[cfg(test)]
use super::{
    credential_pre_aux::derive_zk_x509_credential_pre_aux_binding_v1,
    p256_aggregate_adapter::{
        P256_VALUE_EXECUTION_AGGREGATE_CONSTRAINT_COUNT_V1, p256_cross_trace_terminal_roles_v1,
    },
    profile::{ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1, ZK_X509_PROVER_TARGET_SECONDS_V1},
    rfc5280_stark::{ZkX509P256CertificateTerminalClaimsV1, ZkX509P256WalletTerminalClaimsV1},
    sha_call_bus_stark::ZK_X509_SHA_MAX_ENCODED_PROOF_BYTES_V1,
};
use crate::privacy_engines::{
    aggregate_stark::{self as aggregate, AggregateStarkErrorV1},
    transparent_stark::{
        GOLDILOCKS_GENERATOR_V1, GoldilocksFieldV1 as F, GoldilocksFp4V1 as E, Sha256MerkleTreeV1,
        TransparentStarkErrorV1, TransparentTranscriptV1, append_u16_v1, append_u32_v1,
        append_u64_v1, goldilocks_batch_invert_v1, goldilocks_evaluate_coset_v1,
        goldilocks_fp4_evaluate_coset_v1, goldilocks_fp4_ifft_v1, goldilocks_ifft_v1,
        goldilocks_primitive_root_v1, grind_nonce_v1, masked_trace_lde_column_v1, sha256_frame_v1,
        verify_grinding_nonce_v1,
    },
};

/// Complete proof-system descriptor for the implemented aggregate adapters.
///
/// The descriptor is transcript-bound and records the first-release geometry.
pub(crate) const ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1: &[u8] = b"zk-x509-aggregate-stark-v1-incompatible:wire=outer-X5S1-containing-exactly-one-X5M1-main-and-one-X5C1-ca:X5M1-claims-plus-length-delimited-aggregate-only-no-fixed-sidecar-no-legacy:exact-statement-derived-shape:goldilocks-fp4-w4=7:main-common-lde-log25:compact-ca-local-lde-log14:ordered-native-stride-trace-groups:verifier-owned-logical-adapter-registration:exact-column-ranges-widths-constraint-counts-and-degrees-transcript-bound:64-column-physical-budget-chunks:main-49-registrations-6-groups-logs5,8,15,16,18,19-80-chunks:compact-ca-dedicated-log7-13-chunks:sha256-vector-row-merkle:sha-fixed-algebraic-width472-verifier-derived-no-proof-bytes:p256-fixed-algebraic-width404-verifier-derived-no-proof-bytes:fixed-openings-canonical-sorted-unique-current-next-union-max116-after-grinding:x5b1-shared-challenge-pre-aux=all-six-main-base-roots-then-ca-base-root+main-profile+ca-profile+main-public+ca-public+sample-exact272-goldilocks-post-base-challenges-in-11-family-order=sha-call28,rfc48,projection28,io20,der52,sha-word-memory16,sha-word-base-fold4,p256-value28,p256-cross16,p256-scalar20,p256-arithmetic-copy12+opaque-main-post-base-session:main-io=statement-compiled-40+5d-declarations-logical55922+4736d-active-rows-padded-to262144:rfc5280-output-role-products=18-independent-four-lane-aux-accumulators:all-aux-roots-and-X5M1-terminal-claims-before-fp4-constraint-alphas:one-fp4-composition-lane:main-four-composition-chunks:ca-three-composition-chunks:fri-rate1over32:binary-fri:affine-batching-m3-arities2,2,2:58-uniform-distinct-queries-without-replacement:main-terminal1024-degree31:ca-terminal512-degree15:main-mask802-coefficients:ca-mask306-coefficients:one-transcript-derived-deep-point-per-subproof-current+next-openings:grinding20:p256-four-independent-base-field-bus-lanes-per-family:all-roots-transcript-ordered:subproof-machinery-complete:X5M1-codec-and-accounting-complete:full-main-production-provider-verifier=complete:activation=governance-gated";

const PROOF_MAGIC_V1: [u8; 4] = *b"X5S1";
const SECURITY_LANES: usize = ZK_X509_COMPOSITION_LANES_V1 as usize;
const QUERY_COUNT: usize = ZK_X509_FRI_QUERY_COUNT_V1 as usize;
const BLOWUP: usize = ZK_X509_FRI_BLOWUP_FACTOR_V1 as usize;
const BLOWUP_LOG2: u8 = ZK_X509_FRI_BLOWUP_FACTOR_V1.ilog2() as u8;
const TERMINAL_SIZE: usize = ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1 as usize;
const TERMINAL_LOG2: u8 = 10;
const TERMINAL_DEGREE_BOUND: usize = ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1 as usize;
const COMPOSITION_DEGREE_CHUNKS: usize = ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1 as usize;
/// Inclusive degree of the trace mask multiplier.
///
/// Haböck--Al Kindi Equation (3), with reduced AIR degree six, Fp4
/// extension degree four, one DEEP point, and 58 FRI queries, requires
/// `h = 802` randomizer coefficients.
const MASK_DEGREE: usize = ZK_X509_TRACE_MASK_DEGREE_V1 as usize;
const _: () = assert!(MASK_DEGREE == 801);
const DER_QUOTIENT_COSET_LOG2_V1: u8 = 22;
const DER_QUOTIENT_COSET_SIZE_V1: usize = 1 << DER_QUOTIENT_COSET_LOG2_V1;
const DER_MAXIMUM_QUOTIENT_DEGREE_V1: usize = ZK_X509_DER_STARK_MAXIMUM_QUOTIENT_DEGREE_V1;
const _: () = assert!(DER_MAXIMUM_QUOTIENT_DEGREE_V1 == 3_151_335);
const _: () = assert!(
    DER_MAXIMUM_QUOTIENT_DEGREE_V1 < DER_QUOTIENT_COSET_SIZE_V1
        && DER_QUOTIENT_COSET_SIZE_V1 / ZK_X509_DER_STARK_TRACE_SIZE_V1 == 8
);
const IO_LANES: usize = IO_PERMUTATION_LANES_V1;
const IO_BASE_WIDTH: usize = 28;
const IO_AUX_WIDTH: usize = 39;
const IO_FIXED_WIDTH: usize = 17;
const IO_CONSTRAINT_COUNT: usize = 91;
const IO_CONSTRAINT_DEGREE: u8 = 4;
const MIN_TRACE_LOG2: u8 = 4;
/// Smallest byte-memory trace whose rate-1/64 FRI domain can carry both the
/// release mask and all four exact quotient chunks.
///
/// At log nine the masked trace has degree 1313 while FRI accepts only degree
/// 1023. Log ten raises the FRI input bound to 2047 and the four-chunk
/// composition bound to 8191, covering masked degree 1825 and quotient degree
/// 6276 respectively.
const IO_MIN_SECURE_TRACE_LOG2_V1: u8 = 10;
const _: () = assert!((1_usize << 9) + MASK_DEGREE > 1_023);
const _: () = assert!((1_usize << IO_MIN_SECURE_TRACE_LOG2_V1) + MASK_DEGREE <= 2_047);
const _: () = assert!(
    IO_CONSTRAINT_DEGREE as usize * ((1_usize << IO_MIN_SECURE_TRACE_LOG2_V1) + MASK_DEGREE)
        - (1_usize << IO_MIN_SECURE_TRACE_LOG2_V1)
        <= 8_191
);
const ACCUMULATOR_REGISTRATION_COUNT_V1: usize = 1;
const VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1: usize = QUERY_COUNT * 2;
const P256_CERTIFICATE_REGISTRATION_COUNT_V1: usize = 8;
const P256_WALLET_REGISTRATION_COUNT_V1: usize = 9;
const P256_SIGNATURE_COUNT_V1: usize = 5;
const P256_SIGNATURE_INSTANCE_STRIDE_V1: u16 = 3;
const _: () = assert!(
    P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
        == P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_COUNT_V1 + 2 * P256_SCALAR_BIT_BUS_LANES_V1
);
const CALCULATED_FULL_PROFILE_LOGICAL_REGISTRATIONS_V1: usize = 1
    + 1
    + 1
    + ZK_X509_SHA_SEGMENT_COUNT_V1
    + 1
    + 4 * P256_CERTIFICATE_REGISTRATION_COUNT_V1
    + P256_WALLET_REGISTRATION_COUNT_V1;
const FULL_PROFILE_LOGICAL_REGISTRATIONS_V1: usize = ZK_X509_LOGICAL_REGISTRATIONS_V1;
const FULL_PROFILE_TRACE_GROUPS_V1: usize = ZK_X509_TRACE_GROUPS_V1;
const FULL_PROFILE_PHYSICAL_CHUNKS_V1: usize = ZK_X509_PHYSICAL_COMMITMENT_CHUNKS_V1;
const MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1: [u8; FULL_PROFILE_TRACE_GROUPS_V1] =
    [5, 8, 15, 16, 18, 19];
const _: () = assert!(
    CALCULATED_FULL_PROFILE_LOGICAL_REGISTRATIONS_V1 == FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
);
const _: () = assert!(FULL_PROFILE_TRACE_GROUPS_V1 == ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1);

const AGGREGATE_PARAMETERS_V1: aggregate::AggregateStarkParametersV1 =
    aggregate::AggregateStarkParametersV1 {
        proof_magic: PROOF_MAGIC_V1,
        proof_version: ZK_X509_PROOF_VERSION_V1,
        security_lanes: SECURITY_LANES,
        query_count: QUERY_COUNT,
        blowup_log2: BLOWUP_LOG2,
        terminal_log2: TERMINAL_LOG2,
        terminal_degree_bound: TERMINAL_DEGREE_BOUND,
        composition_degree_chunks: COMPOSITION_DEGREE_CHUNKS,
        minimum_trace_log2: MIN_TRACE_LOG2,
        maximum_trace_log2: 19,
        maximum_trace_groups: FULL_PROFILE_TRACE_GROUPS_V1,
        maximum_segment_instances: FULL_PROFILE_PHYSICAL_CHUNKS_V1,
        maximum_base_columns_per_instance: ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 as usize,
        maximum_aux_columns_per_instance: ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 as usize,
        maximum_proof_bytes: ZK_X509_MAX_PROOF_BYTES_V1 as usize,
    };

const CA_BLOWUP_LOG2_V1: u8 = ZK_X509_CA_FRI_LDE_LOG2_V1 - ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1;
const CA_TERMINAL_SIZE_V1: usize = 1 << ZK_X509_CA_FRI_TERMINAL_LOG2_V1;
const CA_MASK_DEGREE_V1: usize = ZK_X509_CA_TRACE_MASK_DEGREE_V1 as usize;
const CA_COMPOSITION_DEGREE_CHUNKS_V1: usize = ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1 as usize;
const _: () = assert!(CA_BLOWUP_LOG2_V1 == 7);
const _: () = assert!(CA_TERMINAL_SIZE_V1 == 512);
const _: () = assert!(CA_MASK_DEGREE_V1 == 305);
const _: () = assert!(CA_COMPOSITION_DEGREE_CHUNKS_V1 == 3);

const CA_AGGREGATE_PARAMETERS_V1: aggregate::AggregateStarkParametersV1 =
    aggregate::AggregateStarkParametersV1 {
        proof_magic: PROOF_MAGIC_V1,
        proof_version: ZK_X509_PROOF_VERSION_V1,
        security_lanes: SECURITY_LANES,
        query_count: QUERY_COUNT,
        blowup_log2: CA_BLOWUP_LOG2_V1,
        terminal_log2: ZK_X509_CA_FRI_TERMINAL_LOG2_V1,
        terminal_degree_bound: ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1 as usize,
        composition_degree_chunks: CA_COMPOSITION_DEGREE_CHUNKS_V1,
        minimum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        maximum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        maximum_trace_groups: 1,
        maximum_segment_instances: ZK_X509_CA_ACCUMULATOR_CHUNKS_V1,
        maximum_base_columns_per_instance: ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 as usize,
        maximum_aux_columns_per_instance: ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 as usize,
        maximum_proof_bytes: ZK_X509_MAX_PROOF_BYTES_V1 as usize,
    };

const EXEC_CHANNEL: usize = 0;
const EXEC_OFFSET: usize = 1;
const EXEC_VALUE: usize = 2;
const EXEC_WRITE: usize = 3;
const EXEC_ROLE: usize = 4;
const EXEC_INSTANCE: usize = 5;
const EXEC_BITS: usize = 6;
const SORT_CHANNEL: usize = 14;
const SORT_OFFSET: usize = 15;
const SORT_VALUE: usize = 16;
const SORT_WRITE: usize = 17;
const SORT_ROLE: usize = 18;
const SORT_INSTANCE: usize = 19;
const SORT_BITS: usize = 20;

const AUX_EXEC_BEFORE: usize = 0;
const AUX_SORT_BEFORE: usize = AUX_EXEC_BEFORE + IO_LANES;
const AUX_EXEC_AFTER: usize = AUX_SORT_BEFORE + IO_LANES;
const AUX_SORT_AFTER: usize = AUX_EXEC_AFTER + IO_LANES;
const AUX_CONT_SEGMENT_INDEX: usize = AUX_SORT_AFTER + IO_LANES;
const AUX_CONT_GLOBAL_START: usize = AUX_CONT_SEGMENT_INDEX + 1;
const AUX_CONT_GLOBAL_END: usize = AUX_CONT_GLOBAL_START + 1;
const AUX_CONT_LOCAL_START: usize = AUX_CONT_GLOBAL_END + 1;
const AUX_CONT_LOCAL_END: usize = AUX_CONT_LOCAL_START + 1;
const AUX_CONT_MEMORY_START: usize = AUX_CONT_LOCAL_END + 1;
const AUX_CONT_MEMORY_END: usize = AUX_CONT_MEMORY_START + 1;
const AUX_CONT_EXEC_START: usize = AUX_CONT_MEMORY_END + 1;
const AUX_CONT_EXEC_END: usize = AUX_CONT_EXEC_START + IO_LANES;
const AUX_CONT_SORT_START: usize = AUX_CONT_EXEC_END + IO_LANES;
const AUX_CONT_SORT_END: usize = AUX_CONT_SORT_START + IO_LANES;

const _: () = assert!(IO_LANES == 4);
const _: () = assert!(AUX_CONT_SORT_END + IO_LANES == IO_AUX_WIDTH);

const FIX_EXEC_CHANNEL: usize = 0;
const FIX_EXEC_OFFSET: usize = 1;
const FIX_EXEC_WRITE: usize = 2;
const FIX_EXEC_ROLE: usize = 3;
const FIX_EXEC_INSTANCE: usize = 4;
const FIX_SORT_CHANNEL: usize = 5;
const FIX_SORT_OFFSET: usize = 6;
const FIX_SORT_WRITE: usize = 7;
const FIX_SORT_ROLE: usize = 8;
const FIX_SORT_INSTANCE: usize = 9;
const FIX_PUBLIC_SELECTOR: usize = 10;
const FIX_PUBLIC_VALUE: usize = 11;
const FIX_SORT_SAME_ADDRESS_NEXT: usize = 12;
const FIX_ACTIVE: usize = 13;
const FIX_FIRST: usize = 14;
const FIX_LAST_ACTIVE: usize = 15;
const FIX_TRANSITION: usize = 16;

const BASE_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:base-leaf:v1";
const BASE_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:base-node:v1";
const AUX_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:aux-leaf:v1";
const AUX_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:aux-node:v1";
const COMPOSITION_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:composition-leaf:v1";
const COMPOSITION_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:composition-node:v1";
const FRI_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:fri-leaf:v1";
const FRI_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:fri-node:v1";
const PUBLIC_DIGEST_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:io-public:v1";
const QUERY_SEED_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:query-seed:v1";
const DER_TERMINAL_CLAIMS_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:der-terminal-claims:v1";
const MAIN_TERMINAL_CLAIMS_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:stark:main-terminal-claims:v1";
const DER_PUBLIC_DIGEST_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:der-public:v1";
const PROJECTION_PUBLIC_DIGEST_DOMAIN: &[u8] = b"iroha:privacy:zk-x509:stark:projection-public:v1";
const ACCUMULATOR_LAYOUT_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:stark:accumulator-aggregate-layout:v1";
const MAIN_LAYOUT_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:stark:main-aggregate-layout:v1";
const ACCUMULATOR_REGISTRATION_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:stark:accumulator-registration:v1";
const ACCUMULATOR_SCHEDULE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:stark:accumulator-schedule:v1";
const P256_LAYOUT_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:stark:p256-aggregate-layout:v1";
const P256_REGISTRATION_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:stark:p256-registration:v1";

const DER_PROOF_MAGIC_V1: [u8; 4] = *b"X5P1";
const DER_PROOF_CLAIM_COUNT_V1: usize = 2 * ZK_X509_DER_STARK_BUS_LANES_V1;
const DER_PROOF_CLAIM_RECORD_BYTES_V1: usize = 2 + 2 + 8;
const DER_PROOF_LENGTH_OFFSET_V1: usize =
    4 + 2 + 2 + 2 + 2 + DER_PROOF_CLAIM_COUNT_V1 * DER_PROOF_CLAIM_RECORD_BYTES_V1;
const DER_PROOF_ENVELOPE_BYTES_V1: usize = DER_PROOF_LENGTH_OFFSET_V1 + 4;
const DER_SEGMENTED_PROOF_DESCRIPTOR_V1: &[u8] = b"zk-x509-der-segmented-proof-v1:wire=X5P1:version1:strict-der-adapter0:claim-count8:typed-lane-records=input-byte-type1-lanes0-3-then-node-type2-lanes0-3:no-duplicate-or-reordered-claims:canonical-goldilocks-u64be:exact-u32-length-prefixed-X5S1-payload:statement-frame=X5H1-document-count-u16-document-lengths-u16-parser-rows-u32-comparator-rows-u32:terminal-claim-transcript-frame=X5C1:query-only-verifier-fixed-columns:first-release";

const MAIN_PROOF_MAGIC_V1: [u8; 4] = *b"X5M1";
const MAIN_PROOF_ADAPTER_COUNT_V1: u16 = 4;
const MAIN_PROOF_HEADER_BYTES_V1: usize = 4 + 2 + 2;
const MAIN_PROOF_DER_CLAIM_BYTES_V1: usize = DER_PROOF_CLAIM_COUNT_V1 * 8;
const MAIN_PROOF_RFC_OFFSET_V1: usize = MAIN_PROOF_HEADER_BYTES_V1 + MAIN_PROOF_DER_CLAIM_BYTES_V1;
const MAIN_PROOF_SHA_OFFSET_V1: usize =
    MAIN_PROOF_RFC_OFFSET_V1 + ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1;
const MAIN_PROOF_P256_OFFSET_V1: usize =
    MAIN_PROOF_SHA_OFFSET_V1 + ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1;
const MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1: usize =
    MAIN_PROOF_P256_OFFSET_V1 + ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1;
/// Exact fixed framing and terminal-claim bytes around the inner X5S1.
pub(crate) const ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1: usize =
    MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1 + 4;
pub(crate) const ZK_X509_MAIN_PROOF_DESCRIPTOR_V1: &[u8] = b"zk-x509-main-proof-v1-incompatible:wire=X5M1+version1+adapter-count4+eight-canonical-der-terminal-u64be-fields+exact-X5R1-1420+exact-X5Q1-4876+exact-X5V1-5580+u32be-inner-X5S1-length+exact-X5S1:no-fixed-sidecar:no-omitted-reordered-duplicated-or-trailing-records:der-rfc-equalities+all-rfc-output-role-products+exact-four-role-rfc-consumer-to-four-segment-sha-stream-union-equality-validated-before-alphas:fixed-openings-derived-by-verifier-only-after-main-transcript-grinding:no-proof-supplied-fixed-values:x5b1-shared-main-ca-pre-aux-challenges:all-terminal-claims-absorbed-after-aux-roots-before-constraint-alphas:first-release-no-legacy";
const _: () = assert!(ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1 == 11_952);
const _: () = assert!(
    ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1 == ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1 as usize
);

const AGGREGATE_DOMAINS_V1: aggregate::AggregateStarkDomainsV1 =
    aggregate::AggregateStarkDomainsV1 {
        base_leaf: BASE_LEAF_DOMAIN,
        base_node: BASE_NODE_DOMAIN,
        aux_leaf: AUX_LEAF_DOMAIN,
        aux_node: AUX_NODE_DOMAIN,
        composition_leaf: COMPOSITION_LEAF_DOMAIN,
        composition_node: COMPOSITION_NODE_DOMAIN,
        fri_leaf: FRI_LEAF_DOMAIN,
        fri_node: FRI_NODE_DOMAIN,
        layout_label: b"aggregate-stark-layout-v1",
        base_root_label: b"aggregate-stark-base-root-v1",
        aux_root_label: b"aggregate-stark-aux-root-v1",
        composition_root_label: b"aggregate-stark-composition-root-v1",
        fri_root_label: b"aggregate-stark-fri-layer-root-v1",
        fri_beta_label: b"zk-x509-fri-fold-beta-v1",
        query_seed: QUERY_SEED_DOMAIN,
    };

/// Verifier-fixed byte-channel statement for the implemented STARK segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509IoStarkStatementV1 {
    declarations: Vec<ZkX509IoChannelDeclarationV1>,
}

impl ZkX509IoStarkStatementV1 {
    /// Construct the sole canonical statement topology.
    pub(crate) fn new(
        declarations: Vec<ZkX509IoChannelDeclarationV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_declarations_v1(&declarations)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let rows = io_active_rows_v1(&declarations)?;
        if rows > byte_memory_capacity_v1().map_err(|_| ZkX509StarkErrorV1::InvalidStatement)? {
            return Err(ZkX509StarkErrorV1::InvalidStatement);
        }
        let layout = SegmentLayoutV1::for_io(rows)?;
        layout.validate()?;
        Ok(Self { declarations })
    }

    /// Borrow the verifier-fixed channel declarations.
    pub(crate) fn declarations(&self) -> &[ZkX509IoChannelDeclarationV1] {
        &self.declarations
    }
}

/// Failure in the bounded zk-X509 segmented proof implementation.
#[derive(Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509StarkErrorV1 {
    /// Verifier-fixed topology or public bytes are invalid.
    #[error("zk-X509 STARK public statement is invalid")]
    InvalidStatement,
    /// Witness declarations do not exactly equal the verifier statement.
    #[error("zk-X509 STARK witness topology does not match the statement")]
    WitnessStatementMismatch,
    /// Native byte-memory witness construction or validation failed.
    #[error("zk-X509 STARK byte-memory witness is invalid")]
    IoWitness,
    /// Native strict-DER witness construction or validation failed.
    #[error("zk-X509 STARK strict-DER witness is invalid")]
    DerWitness,
    /// Projection witness construction or algebraic compilation failed.
    #[error("zk-X509 STARK projection witness is invalid")]
    ProjectionWitness,
    /// Sparse-accumulator witness, SHA schedule, or numeric material is invalid.
    #[error("zk-X509 STARK accumulator witness is invalid")]
    AccumulatorWitness,
    /// P-256 aggregate witness, fixed topology, or terminal material is invalid.
    #[error("zk-X509 STARK P-256 aggregate witness is invalid")]
    P256Witness,
    /// A compiled segment/domain/column bound was exceeded.
    #[error("zk-X509 STARK segment profile is invalid")]
    ProfileMismatch,
    /// Proof bytes are empty, truncated, trailing, or otherwise malformed.
    #[error("zk-X509 STARK proof wire is malformed")]
    MalformedProof,
    /// Proof bytes exceed the consensus ceiling.
    #[error("zk-X509 STARK proof exceeds the byte ceiling")]
    ProofTooLarge,
    /// One proof field is not a canonical Goldilocks residue.
    #[error("zk-X509 STARK proof contains a non-canonical field")]
    NonCanonicalField,
    /// Masking entropy is unavailable.
    #[error("zk-X509 STARK masking entropy is unavailable")]
    RandomnessUnavailable,
    /// A committed base or auxiliary row opening is invalid.
    #[error("zk-X509 STARK trace opening is invalid")]
    TraceOpening,
    /// A quotient opening or algebraic constraint is invalid.
    #[error("zk-X509 STARK composition opening is invalid")]
    ConstraintOpening,
    /// A FRI opening/fold is invalid.
    #[error("zk-X509 STARK FRI opening is invalid")]
    FriOpening,
    /// The terminal FRI polynomial violates the degree bound.
    #[error("zk-X509 STARK FRI degree bound is invalid")]
    FriDegree,
    /// Transcript order, grinding, or query uniqueness is invalid.
    #[error("zk-X509 STARK transcript is invalid")]
    TranscriptMismatch,
    /// A bounded allocation failed.
    #[error("zk-X509 STARK bounded allocation failed")]
    AllocationFailure,
    /// An invariant in the compiled prover implementation failed.
    #[error("zk-X509 STARK internal invariant failed")]
    InternalInvariant,
}

impl From<ZkX509IoAirErrorV1> for ZkX509StarkErrorV1 {
    fn from(_: ZkX509IoAirErrorV1) -> Self {
        Self::IoWitness
    }
}

impl From<ZkX509ProjectionAirErrorV1> for ZkX509StarkErrorV1 {
    fn from(_: ZkX509ProjectionAirErrorV1) -> Self {
        Self::ProjectionWitness
    }
}

impl From<ZkX509DerStarkErrorV1> for ZkX509StarkErrorV1 {
    fn from(_: ZkX509DerStarkErrorV1) -> Self {
        Self::DerWitness
    }
}

impl From<ZkX509AccumulatorStarkErrorV1> for ZkX509StarkErrorV1 {
    fn from(error: ZkX509AccumulatorStarkErrorV1) -> Self {
        match error {
            ZkX509AccumulatorStarkErrorV1::Resource => Self::AllocationFailure,
            ZkX509AccumulatorStarkErrorV1::Witness
            | ZkX509AccumulatorStarkErrorV1::CallBus
            | ZkX509AccumulatorStarkErrorV1::IoBus
            | ZkX509AccumulatorStarkErrorV1::Shape => Self::AccumulatorWitness,
        }
    }
}

impl From<P256AggregateAdapterErrorV1> for ZkX509StarkErrorV1 {
    fn from(error: P256AggregateAdapterErrorV1) -> Self {
        match error {
            P256AggregateAdapterErrorV1::Resource => Self::AllocationFailure,
            P256AggregateAdapterErrorV1::Phase => Self::TranscriptMismatch,
            P256AggregateAdapterErrorV1::Topology
            | P256AggregateAdapterErrorV1::Source
            | P256AggregateAdapterErrorV1::Challenge
            | P256AggregateAdapterErrorV1::Constraint => Self::P256Witness,
        }
    }
}

/// All proof-carried MAIN terminal claims in canonical adapter order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509MainTerminalClaimsV1 {
    /// Strict-DER byte and parsed-node bus terminals.
    pub(crate) der: ZkX509DerStarkTerminalClaimsV1,
    /// RFC 5280 DER/input-output bus terminals.
    pub(crate) rfc5280: ZkX509Rfc5280StarkTerminalClaimsV1,
    /// Four physical SHA segment terminals.
    pub(crate) sha: ZkX509ShaSegmentTerminalClaimsV1,
    /// Five complete P-256 equation terminals.
    pub(crate) p256: ZkX509P256TerminalClaimsV1,
}

/// RFC output roles consumed by the four SHA RFC-product streams.
///
/// The role code is part of each independently challenge-compressed tuple.
/// Multiplying the four role-addressed RFC products and all four streams from
/// all four physical SHA segments therefore proves equality of the complete
/// verifier-owned multiset without a division or a witness-selected role.
const MAIN_RFC_SHA_CONSUMER_ROLES_V1: [ZkX509Rfc5280OutputRoleV1; 4] = [
    ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
    ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
    ZkX509Rfc5280OutputRoleV1::CrlCommitment,
    ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
];

fn zk_x509_main_rfc_sha_terminal_products_match_v1(
    rfc: ZkX509Rfc5280StarkTerminalClaimsV1,
    sha: ZkX509ShaSegmentTerminalClaimsV1,
) -> bool {
    let sha_products = sha.segments.map(|segment| segment.combined_rfc_products());
    (0..ZK_X509_DER_STARK_BUS_LANES_V1).all(|lane| {
        let rfc_product = MAIN_RFC_SHA_CONSUMER_ROLES_V1
            .into_iter()
            .fold(F::ONE, |product, role| {
                product.mul(rfc.output_role_products_v1(role).consumer_products[lane])
            });
        let sha_product = sha_products
            .iter()
            .fold(F::ONE, |product, segment| product.mul(segment[lane]));
        rfc_product == sha_product
    })
}

/// Decoded canonical MAIN frame borrowing its sole variable proof record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509MainProofEnvelopeV1<'a> {
    /// Transcript-bound terminal claims.
    pub(crate) claims: ZkX509MainTerminalClaimsV1,
    /// Inner 49-registration aggregate X5S1 proof.
    pub(crate) aggregate_proof: &'a [u8],
}

/// Compute one MAIN frame length without allocation.
pub(crate) const fn zk_x509_main_proof_envelope_encoded_len_v1(
    aggregate_bytes: usize,
) -> Option<usize> {
    ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1.checked_add(aggregate_bytes)
}

/// Encode the sole canonical MAIN frame.
pub(crate) fn encode_zk_x509_main_proof_envelope_v1(
    claims: ZkX509MainTerminalClaimsV1,
    aggregate_proof: &[u8],
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    if aggregate_proof.len() < PROOF_MAGIC_V1.len()
        || aggregate_proof[..4] != PROOF_MAGIC_V1
        || aggregate_proof.len() > u32::MAX as usize
        || claims
            .der
            .input_byte
            .iter()
            .chain(&claims.der.node)
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    validate_zk_x509_der_rfc_terminal_equalities_v1(claims.der, claims.rfc5280)
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let rfc = claims
        .rfc5280
        .encode_x5r1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let sha = claims
        .sha
        .encode_x5q1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    if !zk_x509_main_rfc_sha_terminal_products_match_v1(claims.rfc5280, claims.sha) {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    main_p256_terminal_registrations_v1(&claims.p256)
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let p256 = claims
        .p256
        .encode_x5v1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let encoded_len = zk_x509_main_proof_envelope_encoded_len_v1(aggregate_proof.len())
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
    if encoded_len > ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1 {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(encoded_len)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    encoded.extend_from_slice(&MAIN_PROOF_MAGIC_V1);
    append_u16_v1(&mut encoded, ZK_X509_PROOF_VERSION_V1);
    append_u16_v1(&mut encoded, MAIN_PROOF_ADAPTER_COUNT_V1);
    for value in claims.der.input_byte.into_iter().chain(claims.der.node) {
        append_u64_v1(&mut encoded, value.0);
    }
    encoded.extend_from_slice(&rfc);
    encoded.extend_from_slice(&sha);
    encoded.extend_from_slice(&p256);
    append_u32_v1(
        &mut encoded,
        u32::try_from(aggregate_proof.len()).map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?,
    );
    encoded.extend_from_slice(aggregate_proof);
    if encoded.len() != encoded_len {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(encoded)
}

/// Absorb every MAIN cross-adapter terminal before deriving constraint
/// coefficients.
pub(crate) fn absorb_zk_x509_main_terminal_claims_v1(
    transcript: &mut TransparentTranscriptV1,
    claims: ZkX509MainTerminalClaimsV1,
) -> Result<(), ZkX509StarkErrorV1> {
    validate_zk_x509_der_rfc_terminal_equalities_v1(claims.der, claims.rfc5280)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let rfc = claims
        .rfc5280
        .encode_x5r1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let sha = claims
        .sha
        .encode_x5q1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    if !zk_x509_main_rfc_sha_terminal_products_match_v1(claims.rfc5280, claims.sha) {
        return Err(ZkX509StarkErrorV1::InvalidStatement);
    }
    main_p256_terminal_registrations_v1(&claims.p256)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let p256 = claims
        .p256
        .encode_x5v1_v1()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let mut der = [0_u8; MAIN_PROOF_DER_CLAIM_BYTES_V1];
    for (encoded, value) in der
        .chunks_exact_mut(8)
        .zip(claims.der.input_byte.into_iter().chain(claims.der.node))
    {
        encoded.copy_from_slice(&value.0.to_be_bytes());
    }
    transcript
        .absorb(
            MAIN_TERMINAL_CLAIMS_DOMAIN_V1,
            &[ZK_X509_MAIN_PROOF_DESCRIPTOR_V1, &der, &rfc, &sha, &p256],
        )
        .map_err(map_transparent_error_v1)
}

fn main_envelope_u32_v1(encoded: &[u8], offset: usize) -> Result<usize, ZkX509StarkErrorV1> {
    let end = offset
        .checked_add(4)
        .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
    usize::try_from(u32::from_be_bytes(
        encoded
            .get(offset..end)
            .ok_or(ZkX509StarkErrorV1::MalformedProof)?
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
    ))
    .map_err(|_| ZkX509StarkErrorV1::MalformedProof)
}

/// Decode exactly one MAIN frame, rejecting aliases, omissions, reordering,
/// noncanonical fields, length mismatches, and suffixes.
pub(crate) fn decode_zk_x509_main_proof_envelope_v1<'a>(
    encoded: &'a [u8],
) -> Result<ZkX509MainProofEnvelopeV1<'a>, ZkX509StarkErrorV1> {
    if encoded.len() > ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1 {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    if encoded.len() < ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1 + PROOF_MAGIC_V1.len()
        || encoded[..4] != MAIN_PROOF_MAGIC_V1
        || u16::from_be_bytes(
            encoded[4..6]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        ) != ZK_X509_PROOF_VERSION_V1
        || u16::from_be_bytes(
            encoded[6..8]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        ) != MAIN_PROOF_ADAPTER_COUNT_V1
    {
        return Err(ZkX509StarkErrorV1::MalformedProof);
    }
    let mut der_fields = [F::ZERO; DER_PROOF_CLAIM_COUNT_V1];
    for (index, target) in der_fields.iter_mut().enumerate() {
        let start = MAIN_PROOF_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(ZkX509StarkErrorV1::MalformedProof)?,
            )
            .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
        let raw = u64::from_be_bytes(
            encoded[start..start + 8]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        );
        *target = F::canonical(raw).ok_or(ZkX509StarkErrorV1::NonCanonicalField)?;
    }
    let rfc_end = MAIN_PROOF_RFC_OFFSET_V1 + ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1;
    let sha_end = MAIN_PROOF_SHA_OFFSET_V1 + ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1;
    let p256_end = MAIN_PROOF_P256_OFFSET_V1 + ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1;
    let rfc5280 = ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(
        &encoded[MAIN_PROOF_RFC_OFFSET_V1..rfc_end],
    )
    .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    let der = ZkX509DerStarkTerminalClaimsV1 {
        input_byte: der_fields[..ZK_X509_DER_STARK_BUS_LANES_V1]
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
        node: der_fields[ZK_X509_DER_STARK_BUS_LANES_V1..]
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
    };
    validate_zk_x509_der_rfc_terminal_equalities_v1(der, rfc5280)
        .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    let sha = ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(
        &encoded[MAIN_PROOF_SHA_OFFSET_V1..sha_end],
    )
    .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    if !zk_x509_main_rfc_sha_terminal_products_match_v1(rfc5280, sha) {
        return Err(ZkX509StarkErrorV1::MalformedProof);
    }
    let p256 =
        ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&encoded[MAIN_PROOF_P256_OFFSET_V1..p256_end])
            .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    main_p256_terminal_registrations_v1(&p256).map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    let aggregate_len = main_envelope_u32_v1(encoded, MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1)?;
    let aggregate_start = MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1
        .checked_add(4)
        .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
    let aggregate_end = aggregate_start
        .checked_add(aggregate_len)
        .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
    let aggregate_proof = encoded
        .get(aggregate_start..aggregate_end)
        .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
    if aggregate_end != encoded.len()
        || aggregate_proof.len() < PROOF_MAGIC_V1.len()
        || aggregate_proof[..4] != PROOF_MAGIC_V1
    {
        return Err(ZkX509StarkErrorV1::MalformedProof);
    }
    Ok(ZkX509MainProofEnvelopeV1 {
        claims: ZkX509MainTerminalClaimsV1 {
            der,
            rfc5280,
            sha,
            p256,
        },
        aggregate_proof,
    })
}

fn map_aggregate_error_v1(error: AggregateStarkErrorV1) -> ZkX509StarkErrorV1 {
    match error {
        AggregateStarkErrorV1::InvalidLayout | AggregateStarkErrorV1::InvalidProofShape => {
            ZkX509StarkErrorV1::ProfileMismatch
        }
        AggregateStarkErrorV1::MalformedProof => ZkX509StarkErrorV1::MalformedProof,
        AggregateStarkErrorV1::ProofTooLarge => ZkX509StarkErrorV1::ProofTooLarge,
        AggregateStarkErrorV1::NonCanonicalField => ZkX509StarkErrorV1::NonCanonicalField,
        AggregateStarkErrorV1::TraceOpening => ZkX509StarkErrorV1::TraceOpening,
        AggregateStarkErrorV1::ConstraintOpening => ZkX509StarkErrorV1::ConstraintOpening,
        AggregateStarkErrorV1::DeepOpening => ZkX509StarkErrorV1::ConstraintOpening,
        AggregateStarkErrorV1::FriOpening => ZkX509StarkErrorV1::FriOpening,
        AggregateStarkErrorV1::FriDegree => ZkX509StarkErrorV1::FriDegree,
        AggregateStarkErrorV1::TranscriptMismatch => ZkX509StarkErrorV1::TranscriptMismatch,
        AggregateStarkErrorV1::AllocationFailure => ZkX509StarkErrorV1::AllocationFailure,
        AggregateStarkErrorV1::RandomnessUnavailable => ZkX509StarkErrorV1::RandomnessUnavailable,
        AggregateStarkErrorV1::InternalInvariant => ZkX509StarkErrorV1::InternalInvariant,
    }
}

fn map_fixed_algebraic_error_v1(error: ZkX509FixedAlgebraicErrorV1) -> ZkX509StarkErrorV1 {
    match error {
        ZkX509FixedAlgebraicErrorV1::InvalidQuery => ZkX509StarkErrorV1::TraceOpening,
        ZkX509FixedAlgebraicErrorV1::AllocationFailure => ZkX509StarkErrorV1::AllocationFailure,
        ZkX509FixedAlgebraicErrorV1::InvalidDomain
        | ZkX509FixedAlgebraicErrorV1::InvalidWidth
        | ZkX509FixedAlgebraicErrorV1::InvalidAtom
        | ZkX509FixedAlgebraicErrorV1::NonCanonicalSchedule
        | ZkX509FixedAlgebraicErrorV1::NonCanonicalField
        | ZkX509FixedAlgebraicErrorV1::LimitExceeded
        | ZkX509FixedAlgebraicErrorV1::IntegerOverflow
        | ZkX509FixedAlgebraicErrorV1::DivisionByZero
        | ZkX509FixedAlgebraicErrorV1::DescriptorMismatch
        | ZkX509FixedAlgebraicErrorV1::InternalInvariant => ZkX509StarkErrorV1::ProfileMismatch,
    }
}

fn map_sha_fixed_algebraic_error_v1(error: ZkX509ShaFixedAlgebraicErrorV1) -> ZkX509StarkErrorV1 {
    match error {
        ZkX509ShaFixedAlgebraicErrorV1::Resource => ZkX509StarkErrorV1::AllocationFailure,
        ZkX509ShaFixedAlgebraicErrorV1::Topology | ZkX509ShaFixedAlgebraicErrorV1::Algebraic => {
            ZkX509StarkErrorV1::ProfileMismatch
        }
    }
}

fn map_p256_fixed_algebraic_error_v1(error: ZkX509P256FixedAlgebraicErrorV1) -> ZkX509StarkErrorV1 {
    match error {
        ZkX509P256FixedAlgebraicErrorV1::Resource => ZkX509StarkErrorV1::AllocationFailure,
        ZkX509P256FixedAlgebraicErrorV1::Topology => ZkX509StarkErrorV1::ProfileMismatch,
        ZkX509P256FixedAlgebraicErrorV1::Algebraic(error) => map_fixed_algebraic_error_v1(error),
    }
}

/// Stable identity of one physical opened-row evaluator.
///
/// Every identity is part of the sole first-release registration. Numeric
/// material must exist before the full proof API is enabled; an identity or
/// descriptor alone never satisfies registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
enum SegmentAdapterIdV1 {
    ByteMemory = 1,
    StrictDer = 2,
    Rfc5280 = 3,
    Sha256CallBus = 4,
    CaAccumulator = 5,
    Projection = 6,
    P256Arithmetic = 7,
    P256Reduction = 8,
    P256LowS = 9,
    P256Window = 10,
    P256ValueBus = 11,
    P256ScalarBitBus = 12,
}

impl SegmentAdapterIdV1 {
    const fn wire(self) -> u16 {
        self as u16
    }
}

fn p256_instance_v1(signature: usize, local: u16) -> Result<u16, ZkX509StarkErrorV1> {
    if signature >= P256_SIGNATURE_COUNT_V1 || local >= P256_SIGNATURE_INSTANCE_STRIDE_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    u16::try_from(signature)
        .ok()
        .and_then(|signature| signature.checked_mul(P256_SIGNATURE_INSTANCE_STRIDE_V1))
        .and_then(|base| base.checked_add(local))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
}

fn p256_instance_parts_v1(instance: u16) -> Option<(usize, u16)> {
    let signature = usize::from(instance / P256_SIGNATURE_INSTANCE_STRIDE_V1);
    (signature < P256_SIGNATURE_COUNT_V1)
        .then_some((signature, instance % P256_SIGNATURE_INSTANCE_STRIDE_V1))
}

/// Translate one verifier-owned MAIN slice into the sole central P-256
/// registration identity and revalidate every shared native dimension.
///
/// The MAIN layout represents the binding sink as value-bus local instance
/// two, while the central source gives it its own adapter identity at local
/// instance zero. All other mappings are exact.
fn p256_main_registration_from_main_layout_v1(
    registration: RegisteredSegmentLayoutV1,
) -> Result<P256MainRegistrationV1, ZkX509StarkErrorV1> {
    registration.segment.validate()?;
    let (signature, local) = p256_instance_parts_v1(registration.segment.instance)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let (adapter, central_local) = match (registration.segment.adapter, local) {
        (SegmentAdapterIdV1::P256ValueBus, 0 | 1) => {
            (P256MainAdapterV1::ValueBus, usize::from(local))
        }
        (SegmentAdapterIdV1::P256ValueBus, 2) => (P256MainAdapterV1::BindingSink, 0),
        (SegmentAdapterIdV1::P256Arithmetic, 0) => (P256MainAdapterV1::Arithmetic, 0),
        (SegmentAdapterIdV1::P256Window, 0) => (P256MainAdapterV1::WindowBatch, 0),
        (SegmentAdapterIdV1::P256Reduction, 0 | 1) => {
            (P256MainAdapterV1::Reduction, usize::from(local))
        }
        (SegmentAdapterIdV1::P256LowS, 0) => (P256MainAdapterV1::WalletLowS, 0),
        (SegmentAdapterIdV1::P256ScalarBitBus, 0) => (P256MainAdapterV1::ScalarBitBus, 0),
        _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
    };
    let central = P256MainRegistrationV1::new_v1(signature, adapter, central_local)
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let shape = central
        .shape_v1()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    if shape.trace_size != registration.segment.trace_size()
        || shape.base_width != registration.segment.base_width
        || shape.aux_width != registration.segment.aux_width
        || shape.fixed_width != registration.segment.fixed_width
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(central)
}

const REQUIRED_FULL_PROFILE_ADAPTERS_V1: [SegmentAdapterIdV1; 11] = [
    SegmentAdapterIdV1::ByteMemory,
    SegmentAdapterIdV1::StrictDer,
    SegmentAdapterIdV1::Rfc5280,
    SegmentAdapterIdV1::Sha256CallBus,
    SegmentAdapterIdV1::Projection,
    SegmentAdapterIdV1::P256Arithmetic,
    SegmentAdapterIdV1::P256Reduction,
    SegmentAdapterIdV1::P256LowS,
    SegmentAdapterIdV1::P256Window,
    SegmentAdapterIdV1::P256ValueBus,
    SegmentAdapterIdV1::P256ScalarBitBus,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SegmentLayoutV1 {
    adapter: SegmentAdapterIdV1,
    instance: u16,
    active_rows: usize,
    trace_log2: u8,
    lde_log2: u8,
    base_width: usize,
    aux_width: usize,
    fixed_width: usize,
    constraint_count: usize,
    constraint_degree: u8,
    /// Exact number of 64-column streaming commitment slices allocated by
    /// the release plan. This is explicit because the compact-CA adapter
    /// streams its thirteen base and two auxiliary slices independently.
    physical_chunks: usize,
}

#[derive(Clone, Copy)]
struct SegmentDegreeCapacityProfileV1 {
    mask_degree: usize,
    minimum_trace_log2: u8,
    maximum_trace_log2: u8,
    minimum_blowup_log2: u8,
    maximum_lde_log2: u8,
    terminal_log2: u8,
    terminal_degree_bound: usize,
    composition_degree_chunks: usize,
}

const MAIN_DEGREE_CAPACITY_PROFILE_V1: SegmentDegreeCapacityProfileV1 =
    SegmentDegreeCapacityProfileV1 {
        mask_degree: MASK_DEGREE,
        minimum_trace_log2: MIN_TRACE_LOG2,
        maximum_trace_log2: ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
        minimum_blowup_log2: BLOWUP_LOG2,
        maximum_lde_log2: ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
        terminal_log2: TERMINAL_LOG2,
        terminal_degree_bound: TERMINAL_DEGREE_BOUND,
        composition_degree_chunks: COMPOSITION_DEGREE_CHUNKS,
    };

const CA_DEGREE_CAPACITY_PROFILE_V1: SegmentDegreeCapacityProfileV1 =
    SegmentDegreeCapacityProfileV1 {
        mask_degree: CA_MASK_DEGREE_V1,
        minimum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        maximum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        minimum_blowup_log2: CA_BLOWUP_LOG2_V1,
        maximum_lde_log2: ZK_X509_CA_FRI_LDE_LOG2_V1,
        terminal_log2: ZK_X509_CA_FRI_TERMINAL_LOG2_V1,
        terminal_degree_bound: ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1 as usize,
        composition_degree_chunks: CA_COMPOSITION_DEGREE_CHUNKS_V1,
    };

fn checked_segment_degree_capacity_for_profile_v1(
    trace_log2: u8,
    lde_log2: u8,
    constraint_degree: u8,
    profile: SegmentDegreeCapacityProfileV1,
) -> Result<(usize, usize), ZkX509StarkErrorV1> {
    let minimum_lde_log2 = trace_log2
        .checked_add(profile.minimum_blowup_log2)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if !(profile.minimum_trace_log2..=profile.maximum_trace_log2).contains(&trace_log2)
        || !(2..=ZK_X509_MAX_CONSTRAINT_DEGREE_V1).contains(&constraint_degree)
        || !(minimum_lde_log2..=profile.maximum_lde_log2).contains(&lde_log2)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let fri_rounds = lde_log2
        .checked_sub(profile.terminal_log2)
        .filter(|rounds| *rounds != 0)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let trace_size = 1_usize
        .checked_shl(u32::from(trace_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let maximum_masked_trace_degree = trace_size
        .checked_add(profile.mask_degree)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let maximum_quotient_degree = usize::from(constraint_degree)
        .checked_mul(maximum_masked_trace_degree)
        .and_then(|degree| degree.checked_sub(trace_size))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let fold_factor = 1_usize
        .checked_shl(u32::from(fri_rounds))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let maximum_fri_input_degree = profile
        .terminal_degree_bound
        .checked_add(1)
        .and_then(|terminal_coefficients| terminal_coefficients.checked_mul(fold_factor))
        .and_then(|coefficient_capacity| coefficient_capacity.checked_sub(1))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let maximum_composition_degree = maximum_fri_input_degree
        .checked_add(1)
        .and_then(|chunk_size| chunk_size.checked_mul(profile.composition_degree_chunks))
        .and_then(|coefficient_capacity| coefficient_capacity.checked_sub(1))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if maximum_masked_trace_degree > maximum_fri_input_degree
        || maximum_quotient_degree > maximum_composition_degree
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok((maximum_quotient_degree, maximum_fri_input_degree))
}

fn checked_segment_degree_capacity_v1(
    trace_log2: u8,
    lde_log2: u8,
    constraint_degree: u8,
) -> Result<(usize, usize), ZkX509StarkErrorV1> {
    checked_segment_degree_capacity_for_profile_v1(
        trace_log2,
        lde_log2,
        constraint_degree,
        MAIN_DEGREE_CAPACITY_PROFILE_V1,
    )
}

fn checked_compact_ca_degree_capacity_v1(
    trace_log2: u8,
    lde_log2: u8,
    constraint_degree: u8,
) -> Result<(usize, usize), ZkX509StarkErrorV1> {
    checked_segment_degree_capacity_for_profile_v1(
        trace_log2,
        lde_log2,
        constraint_degree,
        CA_DEGREE_CAPACITY_PROFILE_V1,
    )
}

impl SegmentLayoutV1 {
    fn main_capacity_lde_log2_v1(self) -> u8 {
        if matches!(
            self.adapter,
            SegmentAdapterIdV1::P256Arithmetic
                | SegmentAdapterIdV1::P256Reduction
                | SegmentAdapterIdV1::P256LowS
                | SegmentAdapterIdV1::P256Window
                | SegmentAdapterIdV1::P256ValueBus
                | SegmentAdapterIdV1::P256ScalarBitBus
        ) {
            // Every canonical P-256 registration contains its log19 value and
            // arithmetic groups, so all smaller component polynomials are
            // committed on the verifier-fixed MAIN log25 domain. The
            // aggregate layout validates that actual common domain again.
            ZK_X509_MAIN_COMMON_LDE_LOG2_V1
        } else {
            self.lde_log2
        }
    }

    fn with_checked_main_degree_capacity_v1(self) -> Result<Self, ZkX509StarkErrorV1> {
        checked_segment_degree_capacity_v1(
            self.trace_log2,
            self.main_capacity_lde_log2_v1(),
            self.constraint_degree,
        )?;
        Ok(self)
    }

    fn with_checked_compact_ca_degree_capacity_v1(self) -> Result<Self, ZkX509StarkErrorV1> {
        checked_compact_ca_degree_capacity_v1(
            self.trace_log2,
            self.lde_log2,
            self.constraint_degree,
        )?;
        Ok(self)
    }

    fn for_io(active_rows: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let padded = active_rows
            .max(1_usize << IO_MIN_SECURE_TRACE_LOG2_V1)
            .checked_next_power_of_two()
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let trace_log2 =
            u8::try_from(padded.ilog2()).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let lde_log2 = trace_log2
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter: SegmentAdapterIdV1::ByteMemory,
            instance: 0,
            active_rows,
            trace_log2,
            lde_log2,
            base_width: IO_BASE_WIDTH,
            aux_width: IO_AUX_WIDTH,
            fixed_width: IO_FIXED_WIDTH,
            constraint_count: IO_CONSTRAINT_COUNT,
            constraint_degree: IO_CONSTRAINT_DEGREE,
            physical_chunks: 1,
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn for_full_io() -> Result<Self, ZkX509StarkErrorV1> {
        // In the full MAIN registration `active_rows` is the transcript-bound
        // fixed table extent, not the statement-dependent logical prefix.
        // `IoTraceMaterialV1::logical_active_rows` separately controls the
        // active/last-active selectors and continuation endpoints.
        let active_rows = ZK_X509_IO_FIXED_CAPACITY_ROWS_V1;
        let mut layout = Self::for_io(active_rows)?;
        layout.trace_log2 = 18;
        layout.lde_log2 = 18 + BLOWUP_LOG2;
        layout.with_checked_main_degree_capacity_v1()
    }

    fn for_der(active_rows: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let lde_log2 = ZK_X509_DER_STARK_TRACE_LOG2_V1
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter: SegmentAdapterIdV1::StrictDer,
            instance: 0,
            active_rows,
            trace_log2: ZK_X509_DER_STARK_TRACE_LOG2_V1,
            lde_log2,
            base_width: ZK_X509_DER_STARK_BASE_WIDTH_V1,
            aux_width: ZK_X509_DER_STARK_AUX_WIDTH_V1,
            fixed_width: ZK_X509_DER_STARK_FIXED_WIDTH_V1,
            constraint_count: ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1,
            constraint_degree: ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1,
            physical_chunks: ZK_X509_DER_STARK_BASE_WIDTH_V1
                .max(ZK_X509_DER_STARK_AUX_WIDTH_V1)
                .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1)),
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn for_rfc5280(active_rows: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let lde_log2 = ZK_X509_RFC5280_STARK_TRACE_LOG2_V1
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter: SegmentAdapterIdV1::Rfc5280,
            instance: 0,
            active_rows,
            trace_log2: ZK_X509_RFC5280_STARK_TRACE_LOG2_V1,
            lde_log2,
            base_width: ZK_X509_RFC5280_STARK_BASE_WIDTH_V1,
            aux_width: ZK_X509_RFC5280_STARK_AUX_WIDTH_V1,
            fixed_width: ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1,
            constraint_count: ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1,
            constraint_degree: ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1,
            physical_chunks: ZK_X509_RFC5280_STARK_BASE_WIDTH_V1
                .max(ZK_X509_RFC5280_STARK_AUX_WIDTH_V1)
                .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1)),
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn for_projection() -> Result<Self, ZkX509StarkErrorV1> {
        let trace_log2 = u8::try_from(ZK_X509_PROJECTION_TRACE_SIZE_V1.ilog2())
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let lde_log2 = trace_log2
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter: SegmentAdapterIdV1::Projection,
            instance: 0,
            active_rows: ZK_X509_PROJECTION_TRACE_SIZE_V1,
            trace_log2,
            lde_log2,
            base_width: ZK_X509_PROJECTION_BASE_WIDTH_V1,
            aux_width: ZK_X509_PROJECTION_AUX_WIDTH_V1,
            fixed_width: ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1,
            constraint_count: ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1,
            constraint_degree: ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1,
            physical_chunks: 1,
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn for_sha_segment(instance: u16, active_rows: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let lde_log2 = ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter: SegmentAdapterIdV1::Sha256CallBus,
            instance,
            active_rows,
            trace_log2: ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
            lde_log2,
            base_width: ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            aux_width: ZK_X509_SHA_BATCH_AUX_WIDTH_V1,
            fixed_width: ZK_X509_SHA_BATCH_FIXED_WIDTH_V1,
            constraint_count: ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1,
            constraint_degree: ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1,
            physical_chunks: ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1,
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn for_ca_accumulator() -> Result<Self, ZkX509StarkErrorV1> {
        Self {
            adapter: SegmentAdapterIdV1::CaAccumulator,
            instance: 0,
            active_rows: ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1,
            trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            lde_log2: ZK_X509_CA_FRI_LDE_LOG2_V1,
            base_width: ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
            aux_width: ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
            fixed_width: ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
            constraint_count: ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1,
            constraint_degree: ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            physical_chunks: ZK_X509_CA_ACCUMULATOR_CHUNKS_V1,
        }
        .with_checked_compact_ca_degree_capacity_v1()
    }

    fn for_p256_component(
        adapter: SegmentAdapterIdV1,
        instance: u16,
        trace_log2: u8,
        base_width: usize,
        aux_width: usize,
        fixed_width: usize,
        constraint_count: usize,
        constraint_degree: u8,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let active_rows = 1_usize
            .checked_shl(u32::from(trace_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let lde_log2 = trace_log2
            .checked_add(BLOWUP_LOG2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Self {
            adapter,
            instance,
            active_rows,
            trace_log2,
            lde_log2,
            base_width,
            aux_width,
            fixed_width,
            constraint_count,
            constraint_degree,
            physical_chunks: base_width
                .max(aux_width)
                .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1)),
        }
        .with_checked_main_degree_capacity_v1()
    }

    fn trace_size(self) -> usize {
        1_usize << self.trace_log2
    }

    fn lde_size(self) -> usize {
        1_usize << self.lde_log2
    }

    fn column_chunks(self) -> Result<usize, ZkX509StarkErrorV1> {
        let minimum = self
            .base_width
            .max(self.aux_width)
            .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1));
        (self.physical_chunks >= minimum && self.physical_chunks != 0)
            .then_some(self.physical_chunks)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn validate(self) -> Result<(), ZkX509StarkErrorV1> {
        let capacity_profile = if self.adapter == SegmentAdapterIdV1::CaAccumulator {
            checked_compact_ca_degree_capacity_v1(
                self.trace_log2,
                self.lde_log2,
                self.constraint_degree,
            )?;
            CA_DEGREE_CAPACITY_PROFILE_V1
        } else {
            checked_segment_degree_capacity_v1(
                self.trace_log2,
                self.main_capacity_lde_log2_v1(),
                self.constraint_degree,
            )?;
            MAIN_DEGREE_CAPACITY_PROFILE_V1
        };
        let expected_lde_log2 = self
            .trace_log2
            .checked_add(capacity_profile.minimum_blowup_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let fri_rounds = self
            .lde_log2
            .checked_sub(capacity_profile.terminal_log2)
            .filter(|rounds| *rounds != 0)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let terminal_size = 1_usize
            .checked_shl(u32::from(capacity_profile.terminal_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let paired_chunks = self
            .base_width
            .max(self.aux_width)
            .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1));
        if self.active_rows == 0
            || self.active_rows > self.trace_size()
            || self.lde_log2 != expected_lde_log2
            || self.lde_size() < QUERY_COUNT
            || self.base_width == 0
            || self.aux_width == 0
            || self.base_width > usize::from(u16::MAX)
            || self.aux_width > usize::from(u16::MAX)
            || self.fixed_width == 0
            || self.constraint_count == 0
            || self.constraint_count > usize::from(u16::MAX)
            || (self.lde_size() >> fri_rounds) != terminal_size
            || self.column_chunks()? > FULL_PROFILE_PHYSICAL_CHUNKS_V1
            || (self.adapter != SegmentAdapterIdV1::CaAccumulator
                && self.physical_chunks != paired_chunks)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        match self.adapter {
            SegmentAdapterIdV1::ByteMemory
                if self.instance == 0
                    && self.base_width == IO_BASE_WIDTH
                    && self.aux_width == IO_AUX_WIDTH
                    && self.fixed_width == IO_FIXED_WIDTH
                    && self.constraint_count == IO_CONSTRAINT_COUNT
                    && self.constraint_degree == IO_CONSTRAINT_DEGREE => {}
            SegmentAdapterIdV1::StrictDer
                if self.instance == 0
                    && self.trace_log2 == ZK_X509_DER_STARK_TRACE_LOG2_V1
                    && self.active_rows <= ZK_X509_DER_STARK_TRACE_SIZE_V1
                    && self.base_width == ZK_X509_DER_STARK_BASE_WIDTH_V1
                    && self.aux_width == ZK_X509_DER_STARK_AUX_WIDTH_V1
                    && self.fixed_width == ZK_X509_DER_STARK_FIXED_WIDTH_V1
                    && self.constraint_count == ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1 => {}
            SegmentAdapterIdV1::Rfc5280
                if self.instance == 0
                    && self.trace_log2 == ZK_X509_RFC5280_STARK_TRACE_LOG2_V1
                    && self.active_rows <= ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
                    && self.base_width == ZK_X509_RFC5280_STARK_BASE_WIDTH_V1
                    && self.aux_width == ZK_X509_RFC5280_STARK_AUX_WIDTH_V1
                    && self.fixed_width == ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1
                    && self.constraint_count == ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1 => {}
            SegmentAdapterIdV1::Projection
                if self.instance == 0
                    && self.active_rows == ZK_X509_PROJECTION_TRACE_SIZE_V1
                    && self.base_width == ZK_X509_PROJECTION_BASE_WIDTH_V1
                    && self.aux_width == ZK_X509_PROJECTION_AUX_WIDTH_V1
                    && self.fixed_width == ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1
                    && self.constraint_count == ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1 => {}
            SegmentAdapterIdV1::Sha256CallBus
                if usize::from(self.instance) < ZK_X509_SHA_SEGMENT_COUNT_V1
                    && self.trace_log2 == ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
                    && self.active_rows
                        == ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[usize::from(self.instance)]
                    && self.base_width == ZK_X509_SHA_BATCH_BASE_WIDTH_V1
                    && self.aux_width == ZK_X509_SHA_BATCH_AUX_WIDTH_V1
                    && self.fixed_width == ZK_X509_SHA_BATCH_FIXED_WIDTH_V1
                    && self.constraint_count == ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1 => {}
            SegmentAdapterIdV1::CaAccumulator
                if self.instance == 0
                    && self.active_rows == ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1
                    && self.trace_log2 == ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1
                    && self.base_width == ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
                    && self.aux_width == ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
                    && self.fixed_width == ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1
                    && self.constraint_count == ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1
                    && self.physical_chunks == ZK_X509_CA_ACCUMULATOR_CHUNKS_V1 => {}
            SegmentAdapterIdV1::P256Arithmetic
                if p256_instance_parts_v1(self.instance).is_some_and(|(_, local)| local == 0)
                    && self.trace_log2 == P256_ARITHMETIC_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_ARITHMETIC_BASE_WIDTH_V1
                    && self.aux_width == P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1
                    && self.fixed_width == P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1
                    && self.constraint_count == P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 4 => {}
            SegmentAdapterIdV1::P256Reduction
                if p256_instance_parts_v1(self.instance).is_some_and(|(_, local)| local < 2)
                    && self.trace_log2 == P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_REDUCTION_BASE_WIDTH_V1
                    && self.aux_width == P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1
                    && self.fixed_width == P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1
                    && self.constraint_count == P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 4 => {}
            SegmentAdapterIdV1::P256LowS
                if p256_instance_parts_v1(self.instance)
                    .is_some_and(|(signature, local)| signature == 4 && local == 0)
                    && self.trace_log2 == P256_LOW_S_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_LOW_S_BASE_WIDTH_V1
                    && self.aux_width == P256_LOW_S_AGGREGATE_AUX_WIDTH_V1
                    && self.fixed_width == P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1
                    && self.constraint_count == P256_LOW_S_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 3 => {}
            SegmentAdapterIdV1::P256Window
                if p256_instance_parts_v1(self.instance).is_some_and(|(_, local)| local == 0)
                    && self.trace_log2 == P256_WINDOW_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_WINDOW_BASE_WIDTH_V1
                    && self.aux_width == P256_WINDOW_AGGREGATE_AUX_WIDTH_V1
                    && self.fixed_width == P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1
                    && self.constraint_count == P256_WINDOW_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 4 => {}
            SegmentAdapterIdV1::P256ValueBus
                if (p256_instance_parts_v1(self.instance).is_some_and(|(_, local)| local == 0)
                    && self.trace_log2 == P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_VALUE_BUS_STARK_BASE_WIDTH_V1
                    && self.aux_width == P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1
                    && self.fixed_width == P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1
                    && self.constraint_count
                        == P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 3)
                    || (p256_instance_parts_v1(self.instance)
                        .is_some_and(|(_, local)| local == 1)
                        && self.trace_log2 == P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1
                        && self.active_rows == self.trace_size()
                        && self.base_width == P256_VALUE_BUS_STARK_BASE_WIDTH_V1
                        && self.aux_width == P256_VALUE_BUS_STARK_AUX_WIDTH_V1
                        && self.fixed_width == P256_VALUE_BUS_STARK_FIXED_WIDTH_V1
                        && self.constraint_count
                            == P256_VALUE_SORTED_REGISTERED_CONSTRAINT_COUNT_V1
                        && self.constraint_degree == 2)
                    || (p256_instance_parts_v1(self.instance)
                        .is_some_and(|(_, local)| local == 2)
                        && self.trace_log2 == P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1
                        && self.active_rows == self.trace_size()
                        && self.base_width == P256_BINDING_SINK_BASE_WIDTH_V1
                        && self.aux_width
                            == super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1
                        && self.fixed_width == P256_BINDING_SINK_FIXED_WIDTH_V1
                        && self.constraint_count
                            == P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1
                        && self.constraint_degree == 2) => {}
            SegmentAdapterIdV1::P256ScalarBitBus
                if p256_instance_parts_v1(self.instance).is_some_and(|(_, local)| local == 0)
                    && self.trace_log2 == P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1
                    && self.active_rows == self.trace_size()
                    && self.base_width == P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1
                    && self.aux_width == P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1
                    && self.fixed_width == P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1
                    && self.constraint_count
                        == P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
                    && self.constraint_degree == 3 => {}
            _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
        }
        Ok(())
    }
}

fn canonical_accumulator_segment_layouts_v1()
-> Result<[SegmentLayoutV1; ACCUMULATOR_REGISTRATION_COUNT_V1], ZkX509StarkErrorV1> {
    Ok([SegmentLayoutV1::for_ca_accumulator()?])
}

fn canonical_p256_segment_layouts_for_signature_v1(
    role: P256EcdsaRoleV1,
    signature: usize,
) -> Result<Vec<SegmentLayoutV1>, ZkX509StarkErrorV1> {
    let expected_role = if signature == P256_SIGNATURE_COUNT_V1 - 1 {
        P256EcdsaRoleV1::WalletOwnership
    } else {
        P256EcdsaRoleV1::CertificateOrCrl
    };
    if role != expected_role {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let expected_count = match role {
        P256EcdsaRoleV1::CertificateOrCrl => P256_CERTIFICATE_REGISTRATION_COUNT_V1,
        P256EcdsaRoleV1::WalletOwnership => P256_WALLET_REGISTRATION_COUNT_V1,
    };
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(expected_count)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for instance in 0..2 {
        segments.push(SegmentLayoutV1::for_p256_component(
            SegmentAdapterIdV1::P256Reduction,
            p256_instance_v1(signature, instance)?,
            P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1,
            P256_REDUCTION_BASE_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
            P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
            4,
        )?);
    }
    if role == P256EcdsaRoleV1::WalletOwnership {
        segments.push(SegmentLayoutV1::for_p256_component(
            SegmentAdapterIdV1::P256LowS,
            p256_instance_v1(signature, 0)?,
            P256_LOW_S_AGGREGATE_TRACE_LOG2_V1,
            P256_LOW_S_BASE_WIDTH_V1,
            P256_LOW_S_AGGREGATE_AUX_WIDTH_V1,
            P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1,
            P256_LOW_S_REGISTERED_CONSTRAINT_COUNT_V1,
            3,
        )?);
    }
    segments.push(SegmentLayoutV1::for_p256_component(
        SegmentAdapterIdV1::P256ScalarBitBus,
        p256_instance_v1(signature, 0)?,
        P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1,
        P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
        P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1,
        P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1,
        3,
    )?);
    segments.push(SegmentLayoutV1::for_p256_component(
        SegmentAdapterIdV1::P256Window,
        p256_instance_v1(signature, 0)?,
        P256_WINDOW_AGGREGATE_TRACE_LOG2_V1,
        P256_WINDOW_BASE_WIDTH_V1,
        P256_WINDOW_AGGREGATE_AUX_WIDTH_V1,
        P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1,
        P256_WINDOW_REGISTERED_CONSTRAINT_COUNT_V1,
        4,
    )?);
    segments.push(SegmentLayoutV1::for_p256_component(
        SegmentAdapterIdV1::P256ValueBus,
        p256_instance_v1(signature, 2)?,
        P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1,
        P256_BINDING_SINK_BASE_WIDTH_V1,
        super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
        P256_BINDING_SINK_FIXED_WIDTH_V1,
        P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1,
        2,
    )?);
    segments.push(SegmentLayoutV1::for_p256_component(
        SegmentAdapterIdV1::P256Arithmetic,
        p256_instance_v1(signature, 0)?,
        P256_ARITHMETIC_AGGREGATE_TRACE_LOG2_V1,
        P256_ARITHMETIC_BASE_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1,
        P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
        P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1,
        4,
    )?);
    for (instance, aux_width, fixed_width, constraint_count, degree) in [
        (
            0,
            P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1,
            P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
            P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1,
            3,
        ),
        (
            1,
            P256_VALUE_BUS_STARK_AUX_WIDTH_V1,
            P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
            P256_VALUE_SORTED_REGISTERED_CONSTRAINT_COUNT_V1,
            2,
        ),
    ] {
        segments.push(SegmentLayoutV1::for_p256_component(
            SegmentAdapterIdV1::P256ValueBus,
            p256_instance_v1(signature, instance)?,
            P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
            P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
            aux_width,
            fixed_width,
            constraint_count,
            degree,
        )?);
    }
    if segments.len() != expected_count {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(segments)
}

fn canonical_p256_segment_layouts_v1(
    role: P256EcdsaRoleV1,
) -> Result<Vec<SegmentLayoutV1>, ZkX509StarkErrorV1> {
    let signature = match role {
        P256EcdsaRoleV1::CertificateOrCrl => 0,
        P256EcdsaRoleV1::WalletOwnership => P256_SIGNATURE_COUNT_V1 - 1,
    };
    canonical_p256_segment_layouts_for_signature_v1(role, signature)
}

fn canonical_sha_segment_layouts_v1() -> Result<Vec<SegmentLayoutV1>, ZkX509StarkErrorV1> {
    ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1
        .iter()
        .copied()
        .enumerate()
        .map(|(instance, active_rows)| {
            SegmentLayoutV1::for_sha_segment(
                u16::try_from(instance).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                active_rows,
            )
        })
        .collect()
}

/// Reconstruct the sole complete fixed-capacity X5S1 registration.
///
/// Private witness code never supplies an adapter, instance, width, active
/// count, family boundary, order, or padding boundary. Public values affect
/// verifier-fixed input bindings inside these registrations, never the proof
/// container geometry.
fn canonical_full_profile_segment_layouts_v1() -> Result<Vec<SegmentLayoutV1>, ZkX509StarkErrorV1> {
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(FULL_PROFILE_LOGICAL_REGISTRATIONS_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        let role = if signature + 1 == P256_SIGNATURE_COUNT_V1 {
            P256EcdsaRoleV1::WalletOwnership
        } else {
            P256EcdsaRoleV1::CertificateOrCrl
        };
        segments.extend(canonical_p256_segment_layouts_for_signature_v1(
            role, signature,
        )?);
    }
    segments.push(SegmentLayoutV1::for_projection()?);
    segments.push(SegmentLayoutV1::for_full_io()?);
    segments.push(SegmentLayoutV1::for_rfc5280(
        ZK_X509_RFC5280_STARK_TRACE_SIZE_V1,
    )?);
    segments.push(SegmentLayoutV1::for_der(ZK_X509_DER_STARK_TRACE_SIZE_V1)?);
    segments.extend(canonical_sha_segment_layouts_v1()?);
    segments
        .sort_unstable_by_key(|segment| (segment.trace_log2, segment.adapter, segment.instance));
    if segments.len() != FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
        || segments.windows(2).any(|pair| {
            (pair[0].trace_log2, pair[0].adapter, pair[0].instance)
                >= (pair[1].trace_log2, pair[1].adapter, pair[1].instance)
        })
        || segments.iter().try_fold(0_usize, |chunks, segment| {
            chunks
                .checked_add(segment.column_chunks()?)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
        })? != FULL_PROFILE_PHYSICAL_CHUNKS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(segments)
}

/// One canonical trace-commitment group in the aggregate proof.
///
/// Segments with the same native trace size share one vector-row Merkle
/// commitment. Every group is evaluated on the proof's common LDE domain;
/// `next_stride` is therefore derived, never supplied by the proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceGroupLayoutV1 {
    native_trace_log2: u8,
    /// Number of 64-column commitment-budget chunks, not logical adapters.
    column_chunks: usize,
    base_width: usize,
    aux_width: usize,
}

impl TraceGroupLayoutV1 {
    fn as_shared(self) -> aggregate::AggregateTraceGroupLayoutV1 {
        aggregate::AggregateTraceGroupLayoutV1 {
            native_trace_log2: self.native_trace_log2,
            segment_instances: self.column_chunks,
            base_width: self.base_width,
            aux_width: self.aux_width,
        }
    }

    fn next_stride(self, common_lde_log2: u8) -> Result<usize, ZkX509StarkErrorV1> {
        self.as_shared()
            .next_stride(common_lde_log2)
            .map_err(map_aggregate_error_v1)
    }
}

/// One logical adapter's verifier-derived slices inside a trace-group row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RegisteredSegmentLayoutV1 {
    segment: SegmentLayoutV1,
    trace_group: usize,
    base_start: usize,
    aux_start: usize,
    column_chunks: usize,
}

impl RegisteredSegmentLayoutV1 {
    fn base_end(self) -> Result<usize, ZkX509StarkErrorV1> {
        self.base_start
            .checked_add(self.segment.base_width)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn aux_end(self) -> Result<usize, ZkX509StarkErrorV1> {
        self.aux_start
            .checked_add(self.segment.aux_width)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }
}

/// Verifier-derived aggregate commitment and FRI layout.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AggregateProofLayoutV1 {
    common_lde_log2: u8,
    trace_groups: Vec<TraceGroupLayoutV1>,
    registered_segments: Vec<RegisteredSegmentLayoutV1>,
}

impl AggregateProofLayoutV1 {
    fn for_segments(layouts: &[SegmentLayoutV1]) -> Result<Self, ZkX509StarkErrorV1> {
        Self::for_segments_with_equal_log_bucketing_v1(layouts, false)
    }

    fn for_equal_log_buckets_v1(layouts: &[SegmentLayoutV1]) -> Result<Self, ZkX509StarkErrorV1> {
        Self::for_segments_with_equal_log_bucketing_v1(layouts, true)
    }

    fn for_segments_with_equal_log_bucketing_v1(
        layouts: &[SegmentLayoutV1],
        bucket_equal_logs: bool,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if layouts.is_empty() || layouts.len() > FULL_PROFILE_LOGICAL_REGISTRATIONS_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut previous_key = None;
        for layout in layouts {
            layout.validate()?;
            let key = (layout.trace_log2, layout.adapter, layout.instance);
            if previous_key.is_some_and(|previous| previous >= key) {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            previous_key = Some(key);
        }
        // Most focused single-adapter proofs retain independent groups. A
        // role-local composite such as one P-256 signature deterministically
        // buckets equal native logarithms so a single vector-row commitment
        // owns disjoint verifier-fixed slices for every logical adapter.
        let mut trace_groups = Vec::with_capacity(layouts.len());
        let mut registered_segments = Vec::with_capacity(layouts.len());
        for segment in layouts.iter().copied() {
            let column_chunks = segment.column_chunks()?;
            let trace_group = if bucket_equal_logs
                && trace_groups
                    .last()
                    .is_some_and(|group: &TraceGroupLayoutV1| {
                        group.native_trace_log2 == segment.trace_log2
                    }) {
                trace_groups.len() - 1
            } else {
                trace_groups.push(TraceGroupLayoutV1 {
                    native_trace_log2: segment.trace_log2,
                    column_chunks: 0,
                    base_width: 0,
                    aux_width: 0,
                });
                trace_groups.len() - 1
            };
            let group = trace_groups
                .get_mut(trace_group)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            let base_start = group.base_width;
            let aux_start = group.aux_width;
            group.base_width = group
                .base_width
                .checked_add(segment.base_width)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            group.aux_width = group
                .aux_width
                .checked_add(segment.aux_width)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            group.column_chunks = group
                .column_chunks
                .checked_add(column_chunks)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            registered_segments.push(RegisteredSegmentLayoutV1 {
                segment,
                trace_group,
                base_start,
                aux_start,
                column_chunks,
            });
        }
        let maximum_native_log2 = trace_groups
            .last()
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .native_trace_log2;
        let compact_ca =
            layouts.len() == 1 && layouts[0].adapter == SegmentAdapterIdV1::CaAccumulator;
        let common_lde_log2 = if compact_ca {
            layouts[0].lde_log2
        } else {
            maximum_native_log2
                .checked_add(BLOWUP_LOG2)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
        };
        let layout = Self {
            common_lde_log2,
            trace_groups,
            registered_segments,
        };
        layout.validate()?;
        Ok(layout)
    }

    fn for_accumulators_v1() -> Result<Self, ZkX509StarkErrorV1> {
        let layout = Self::for_segments(&canonical_accumulator_segment_layouts_v1()?)?;
        layout.validate_accumulator_registration_v1()?;
        Ok(layout)
    }

    fn for_p256_v1(role: P256EcdsaRoleV1) -> Result<Self, ZkX509StarkErrorV1> {
        let layout = Self::for_equal_log_buckets_v1(&canonical_p256_segment_layouts_v1(role)?)?;
        layout.validate_p256_registration_v1(role)?;
        Ok(layout)
    }

    fn for_full_profile_v1() -> Result<Self, ZkX509StarkErrorV1> {
        let segments = canonical_full_profile_segment_layouts_v1()?;
        let layout = Self::for_equal_log_buckets_v1(&segments)?;
        layout.validate_exact_full_profile_registration_v1()?;
        Ok(layout)
    }

    fn registered_segment(
        &self,
        adapter: SegmentAdapterIdV1,
        instance: u16,
    ) -> Result<RegisteredSegmentLayoutV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .registered_segments
            .iter()
            .copied()
            .filter(|registration| {
                registration.segment.adapter == adapter && registration.segment.instance == instance
            });
        let registration = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(registration)
    }

    fn common_lde_size(&self) -> usize {
        1_usize << self.common_lde_log2
    }

    fn fri_rounds(&self) -> usize {
        usize::from(self.common_lde_log2 - self.parameters_v1().terminal_log2)
    }

    fn parameters_v1(&self) -> aggregate::AggregateStarkParametersV1 {
        if self.registered_segments.len() == 1
            && self.registered_segments[0].segment.adapter == SegmentAdapterIdV1::CaAccumulator
        {
            CA_AGGREGATE_PARAMETERS_V1
        } else {
            AGGREGATE_PARAMETERS_V1
        }
    }

    fn as_shared(&self) -> Result<aggregate::AggregateProofLayoutV1, ZkX509StarkErrorV1> {
        let parameters = self.parameters_v1();
        let shared = aggregate::AggregateProofLayoutV1::new(
            parameters,
            self.trace_groups
                .iter()
                .copied()
                .map(TraceGroupLayoutV1::as_shared)
                .collect(),
        )
        .map_err(map_aggregate_error_v1)?;
        if shared.common_lde_log2() != self.common_lde_log2 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(shared)
    }

    fn validate(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.as_shared()?;
        if self.registered_segments.is_empty() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut previous_key = None;
        let mut next_base = vec![0_usize; self.trace_groups.len()];
        let mut next_aux = vec![0_usize; self.trace_groups.len()];
        let mut chunks = vec![0_usize; self.trace_groups.len()];
        for registration in &self.registered_segments {
            registration.segment.validate()?;
            if registration.segment.adapter == SegmentAdapterIdV1::CaAccumulator {
                checked_compact_ca_degree_capacity_v1(
                    registration.segment.trace_log2,
                    self.common_lde_log2,
                    registration.segment.constraint_degree,
                )?;
            } else {
                checked_segment_degree_capacity_v1(
                    registration.segment.trace_log2,
                    self.common_lde_log2,
                    registration.segment.constraint_degree,
                )?;
            }
            let key = (
                registration.segment.trace_log2,
                registration.segment.adapter,
                registration.segment.instance,
            );
            if previous_key.is_some_and(|previous| previous >= key)
                || registration.trace_group >= self.trace_groups.len()
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let group = self.trace_groups[registration.trace_group];
            if group.native_trace_log2 != registration.segment.trace_log2
                || registration.base_start != next_base[registration.trace_group]
                || registration.aux_start != next_aux[registration.trace_group]
                || registration.column_chunks != registration.segment.column_chunks()?
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            next_base[registration.trace_group] = registration.base_end()?;
            next_aux[registration.trace_group] = registration.aux_end()?;
            chunks[registration.trace_group] = chunks[registration.trace_group]
                .checked_add(registration.column_chunks)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            previous_key = Some(key);
        }
        if self
            .trace_groups
            .iter()
            .zip(next_base)
            .zip(next_aux)
            .zip(chunks)
            .any(|(((group, base), aux), chunks)| {
                group.base_width != base || group.aux_width != aux || group.column_chunks != chunks
            })
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    fn validate_full_profile_registration(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.validate()?;
        if REQUIRED_FULL_PROFILE_ADAPTERS_V1.iter().any(|required| {
            !self
                .registered_segments
                .iter()
                .any(|registration| registration.segment.adapter == *required)
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if self.registered_segments.len() != FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
            || self.trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || self
                .trace_groups
                .iter()
                .try_fold(0_usize, |chunks, group| {
                    chunks
                        .checked_add(group.column_chunks)
                        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
                })?
                != FULL_PROFILE_PHYSICAL_CHUNKS_V1
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    fn validate_exact_full_profile_registration_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.validate_full_profile_registration()?;
        let expected =
            Self::for_equal_log_buckets_v1(&canonical_full_profile_segment_layouts_v1()?)?;
        if self != &expected {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    fn validate_accumulator_instance_set_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.validate()?;
        let expected_segments = canonical_accumulator_segment_layouts_v1()?;
        let mut accumulator_registrations = [None; ACCUMULATOR_REGISTRATION_COUNT_V1];
        let mut count = 0_usize;
        for registration in self
            .registered_segments
            .iter()
            .copied()
            .filter(|registration| {
                registration.segment.adapter == SegmentAdapterIdV1::CaAccumulator
            })
        {
            let slot = accumulator_registrations
                .get_mut(count)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            *slot = Some(registration);
            count += 1;
        }
        if count != ACCUMULATOR_REGISTRATION_COUNT_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut previous_group = None;
        for (registration, expected) in accumulator_registrations.into_iter().zip(expected_segments)
        {
            let registration = registration.ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            let group = self.trace_groups.get(registration.trace_group);
            if previous_group.is_some_and(|previous| previous >= registration.trace_group)
                || registration.segment != expected
                || registration.base_start != 0
                || registration.aux_start != 0
                || group.is_none_or(|group| {
                    group.native_trace_log2 != expected.trace_log2
                        || group.base_width != expected.base_width
                        || group.aux_width != expected.aux_width
                        || group.column_chunks != registration.column_chunks
                })
                || self
                    .registered_segments
                    .iter()
                    .filter(|candidate| candidate.trace_group == registration.trace_group)
                    .count()
                    != 1
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            previous_group = Some(registration.trace_group);
        }
        Ok(())
    }

    fn validate_accumulator_registration_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.validate_accumulator_instance_set_v1()?;
        let expected = Self::for_segments(&canonical_accumulator_segment_layouts_v1()?)?;
        if self != &expected {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    fn validate_p256_instance_set_v1(
        &self,
        role: P256EcdsaRoleV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.validate()?;
        let expected_segments = canonical_p256_segment_layouts_v1(role)?;
        let registrations = self
            .registered_segments
            .iter()
            .copied()
            .filter(|registration| {
                matches!(
                    registration.segment.adapter,
                    SegmentAdapterIdV1::P256Arithmetic
                        | SegmentAdapterIdV1::P256Reduction
                        | SegmentAdapterIdV1::P256LowS
                        | SegmentAdapterIdV1::P256Window
                        | SegmentAdapterIdV1::P256ValueBus
                        | SegmentAdapterIdV1::P256ScalarBitBus
                )
            })
            .collect::<Vec<_>>();
        if registrations.len() != expected_segments.len() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        for (registration, expected) in registrations.into_iter().zip(expected_segments) {
            if registration.segment != expected {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
        }
        Ok(())
    }

    fn validate_p256_registration_v1(
        &self,
        role: P256EcdsaRoleV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.validate_p256_instance_set_v1(role)?;
        let expected = Self::for_equal_log_buckets_v1(&canonical_p256_segment_layouts_v1(role)?)?;
        if self != &expected {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }
}

/// Publicly inspectable census of the sole first-release MAIN registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509MainRegistrationShapeV1 {
    /// Exact logical adapter registrations.
    pub(crate) logical_registrations: usize,
    /// Exact equal-native-log trace groups.
    pub(crate) trace_groups: usize,
    /// Exact 64-column commitment chunks.
    pub(crate) physical_chunks: usize,
}

/// Reconstruct and validate the exact 49-registration MAIN topology.
///
/// This is intentionally verifier-owned and accepts no witness-dependent
/// dimensions.  MAIN assembly calls it before handing material to the
/// aggregate prover.
pub(crate) fn validate_zk_x509_main_registration_shape_v1()
-> Result<ZkX509MainRegistrationShapeV1, ZkX509StarkErrorV1> {
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    let physical_chunks = layout.trace_groups.iter().try_fold(0_usize, |sum, group| {
        sum.checked_add(group.column_chunks)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    })?;
    let shape = ZkX509MainRegistrationShapeV1 {
        logical_registrations: layout.registered_segments.len(),
        trace_groups: layout.trace_groups.len(),
        physical_chunks,
    };
    if shape.logical_registrations != FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
        || shape.trace_groups != FULL_PROFILE_TRACE_GROUPS_V1
        || shape.physical_chunks != FULL_PROFILE_PHYSICAL_CHUNKS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(shape)
}

/// Complete verifier-owned first-release MAIN profile.
///
/// The registration census and sole 28-field compiled-profile digest are
/// constructed together. Fixed rows are evaluated from the manifest-bound
/// algebraic schedules and never selected by a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509MainVerifierProfileV1 {
    /// Exact 49-registration topology.
    pub(crate) registration: ZkX509MainRegistrationShapeV1,
    /// Digest of the complete 28-field algebraic release manifest.
    pub(crate) compiled_profile_digest: [u8; 32],
}

/// Construct the sole MAIN verifier profile from independently pinned
/// release material.
pub(crate) fn construct_zk_x509_main_verifier_profile_v1()
-> Result<ZkX509MainVerifierProfileV1, ZkX509StarkErrorV1> {
    let registration = validate_zk_x509_main_registration_shape_v1()?;
    let compiled =
        construct_zk_x509_compiled_profile_v1().map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    Ok(ZkX509MainVerifierProfileV1 {
        registration,
        compiled_profile_digest: compiled.digest(),
    })
}

/// Reject any supplied MAIN profile that differs from the verifier-owned
/// release pins.
pub(crate) fn validate_zk_x509_main_verifier_profile_v1(
    supplied: ZkX509MainVerifierProfileV1,
) -> Result<(), ZkX509StarkErrorV1> {
    let expected = construct_zk_x509_main_verifier_profile_v1()?;
    if supplied.registration != expected.registration
        || supplied.compiled_profile_digest != expected.compiled_profile_digest
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

/// Verifier-derived fixed rows are evaluated only after MAIN grinding and
/// query derivation. Construction is private so a caller cannot substitute a
/// query set, schedule digest, or fixed row.
const MAIN_LOG19_QUERY_SCHEDULE_DOMAIN_V1: &[u8] = b"iroha.zk-x509.main.log19-query-schedule.v1";

/// Transcript-order current/next pairs for MAIN's native-log19 group.
///
/// The canonical sorted-unique union is used by both algebraic evaluators,
/// while the ordered pairs retain transcript order and current/next pairing.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MainLog19VerifierQueryScheduleV1 {
    pairs: [(usize, usize); QUERY_COUNT],
    indices: Vec<u64>,
    order_digest: [u8; 32],
}

fn main_log19_query_schedule_digest_v1(
    pairs: &[(usize, usize); QUERY_COUNT],
) -> Result<[u8; 32], ZkX509StarkErrorV1> {
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(QUERY_COUNT * 2 * core::mem::size_of::<u64>())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for (current, next) in pairs {
        encoded.extend_from_slice(
            &u64::try_from(*current)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(
            &u64::try_from(*next)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?
                .to_be_bytes(),
        );
    }
    sha256_frame_v1(MAIN_LOG19_QUERY_SCHEDULE_DOMAIN_V1, &[&encoded])
        .map_err(map_transparent_error_v1)
}

impl MainLog19VerifierQueryScheduleV1 {
    fn from_query_coordinates_v1(query_coordinates: &[usize]) -> Result<Self, ZkX509StarkErrorV1> {
        let query_coordinates: [usize; QUERY_COUNT] = query_coordinates
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::TraceOpening)?;
        let common_lde_size = 1_usize
            .checked_shl(u32::from(ZK_X509_MAIN_COMMON_LDE_LOG2_V1))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut sorted_current = query_coordinates;
        sorted_current.sort_unstable();
        if sorted_current
            .windows(2)
            .any(|pair| pair.first() == pair.get(1))
        {
            return Err(ZkX509StarkErrorV1::TraceOpening);
        }
        let mut pairs = [(0_usize, 0_usize); QUERY_COUNT];
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (target, current) in pairs.iter_mut().zip(query_coordinates) {
            if current >= common_lde_size {
                return Err(ZkX509StarkErrorV1::TraceOpening);
            }
            let next = current
                .checked_add(P256_MAIN_LOG19_NEXT_STRIDE_V1)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
                % common_lde_size;
            *target = (current, next);
            indices.push(u64::try_from(current).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
            indices.push(u64::try_from(next).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
        }
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty()
            || indices.len() > VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
            || indices.len() > ZK_X509_FIXED_ALGEBRAIC_MAX_QUERIES_V1
        {
            return Err(ZkX509StarkErrorV1::TraceOpening);
        }
        let order_digest = main_log19_query_schedule_digest_v1(&pairs)?;
        Ok(Self {
            pairs,
            indices,
            order_digest,
        })
    }

    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        let query_coordinates = self.pairs.map(|(current, _)| current);
        let expected = Self::from_query_coordinates_v1(&query_coordinates)?;
        if self != &expected {
            return Err(ZkX509StarkErrorV1::TraceOpening);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkX509MainVerifierDerivedFixedOpeningsV1 {
    query_schedule: MainLog19VerifierQueryScheduleV1,
    sha: ZkX509FixedAlgebraicOpeningsV1,
    p256_log19: ZkX509FixedAlgebraicOpeningsV1,
}

/// Evaluate both manifest-bound schedules against the post-grinding query
/// union. No proof or service supplies fixed bytes.
fn derive_zk_x509_main_fixed_openings_after_grinding_v1(
    verifier_profile: ZkX509MainVerifierProfileV1,
    sha_shape: ZkX509ShaCallPublicShapeV1,
    query_coordinates: &[usize],
) -> Result<ZkX509MainVerifierDerivedFixedOpeningsV1, ZkX509StarkErrorV1> {
    validate_zk_x509_main_verifier_profile_v1(verifier_profile)?;
    derive_zk_x509_main_fixed_openings_after_profile_validation_v1(sha_shape, query_coordinates)
}

fn derive_zk_x509_main_fixed_openings_after_profile_validation_v1(
    sha_shape: ZkX509ShaCallPublicShapeV1,
    query_coordinates: &[usize],
) -> Result<ZkX509MainVerifierDerivedFixedOpeningsV1, ZkX509StarkErrorV1> {
    let query_schedule =
        MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(query_coordinates)?;
    let sha_schedule = zk_x509_sha_fixed_algebraic_schedule_v1(sha_shape)
        .map_err(map_sha_fixed_algebraic_error_v1)?;
    let p256_schedule =
        zk_x509_p256_fixed_algebraic_schedule_v1().map_err(map_p256_fixed_algebraic_error_v1)?;
    let sha = sha_schedule
        .evaluate_query_indices_v1(&query_schedule.indices)
        .map_err(map_fixed_algebraic_error_v1)?;
    let p256_log19 = p256_schedule
        .evaluate_query_indices_v1(&query_schedule.indices)
        .map_err(map_fixed_algebraic_error_v1)?;
    if sha.query_indices_v1() != query_schedule.indices.as_slice()
        || p256_log19.query_indices_v1() != query_schedule.indices.as_slice()
        || sha.schedule_digest_v1() != sha_schedule.descriptor_digest_v1()
        || p256_log19.schedule_digest_v1() != p256_schedule.descriptor_digest_v1()
        || usize::from(sha.width_v1()) != ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1
        || usize::from(p256_log19.width_v1()) != ZK_X509_P256_FIXED_ALGEBRAIC_WIDTH_V1
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(ZkX509MainVerifierDerivedFixedOpeningsV1 {
        query_schedule,
        sha,
        p256_log19,
    })
}

/// Ordered base-commitment phase for the sole six-group MAIN registration.
///
/// The session is intentionally private to this module: future production
/// prover and verifier entry points drive it while committing or decoding each
/// canonical group. No caller can mint pre-auxiliary state from a root array.
struct ZkX509MainBaseCommitmentSessionV1 {
    layout: AggregateProofLayoutV1,
    consensus_context_digest: [u8; 32],
    main_profile_digest: [u8; 32],
    roots: [[u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
    recorded: [bool; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
    next_group: usize,
}

/// Type-level proof that all six canonical MAIN base groups were committed.
///
/// Fields are private and there is no constructor. Only
/// `ZkX509MainBaseCommitmentSessionV1::complete_v1` can create this token.
pub(super) struct ZkX509CompletedMainBaseCommitmentSessionV1 {
    consensus_context_digest: [u8; 32],
    main_profile_digest: [u8; 32],
    roots: [[u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
}

impl ZkX509CompletedMainBaseCommitmentSessionV1 {
    pub(super) fn into_pre_aux_parts_v1(
        self,
    ) -> (
        [u8; 32],
        [u8; 32],
        [[u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
    ) {
        (
            self.consensus_context_digest,
            self.main_profile_digest,
            self.roots,
        )
    }
}

impl ZkX509MainBaseCommitmentSessionV1 {
    fn new_v1(
        layout: &AggregateProofLayoutV1,
        consensus_context_digest: [u8; 32],
        verifier_profile: ZkX509MainVerifierProfileV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_zk_x509_main_verifier_profile_v1(verifier_profile)?;
        Self::new_after_profile_validation_v1(
            layout,
            consensus_context_digest,
            verifier_profile.compiled_profile_digest,
        )
    }

    /// Initialize chronology only after the caller has validated the release
    /// profile. This remains private: production reaches it exclusively
    /// through `new_v1`, while unit tests exercise the isolated chronology
    /// state machine with explicit test profiles.
    fn new_after_profile_validation_v1(
        layout: &AggregateProofLayoutV1,
        consensus_context_digest: [u8; 32],
        main_profile_digest: [u8; 32],
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        if layout.trace_groups.len() != ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1
            || layout
                .trace_groups
                .iter()
                .map(|group| group.native_trace_log2)
                .ne(MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1.into_iter())
            || consensus_context_digest == [0_u8; 32]
            || main_profile_digest == [0_u8; 32]
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let session = Self {
            layout: layout.clone(),
            consensus_context_digest,
            main_profile_digest,
            roots: [[0_u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
            recorded: [false; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
            next_group: 0,
        };
        session.validate_state_v1()?;
        Ok(session)
    }

    fn validate_state_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_exact_full_profile_registration_v1()?;
        if self.layout.trace_groups.len() != ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1
            || self.next_group > ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1
            || self
                .layout
                .trace_groups
                .iter()
                .map(|group| group.native_trace_log2)
                .ne(MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1.into_iter())
            || self.consensus_context_digest == [0_u8; 32]
            || self.main_profile_digest == [0_u8; 32]
            || self
                .recorded
                .iter()
                .enumerate()
                .any(|(index, recorded)| *recorded != (index < self.next_group))
            || self
                .roots
                .iter()
                .enumerate()
                .any(|(index, root)| (*root == [0_u8; 32]) != (index >= self.next_group))
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }

    fn accept_base_root_v1(
        &mut self,
        group_index: usize,
        native_trace_log2: u8,
        root: [u8; 32],
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.validate_state_v1()?;
        let expected_index = self.next_group;
        let expected_log = MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
            .get(expected_index)
            .copied()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        let layout_log = self
            .layout
            .trace_groups
            .get(expected_index)
            .map(|group| group.native_trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if group_index != expected_index
            || native_trace_log2 != expected_log
            || native_trace_log2 != layout_log
            || self.recorded[expected_index]
            || root == [0_u8; 32]
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        self.roots[expected_index] = root;
        self.recorded[expected_index] = true;
        self.next_group = self
            .next_group
            .checked_add(1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        self.validate_state_v1()
    }

    fn accept_streaming_base_commitment_v1(
        &mut self,
        group_index: usize,
        commitment: &aggregate::StreamingRowCommitmentResultV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let native_trace_log2 = self
            .layout
            .trace_groups
            .get(group_index)
            .map(|group| group.native_trace_log2)
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        self.accept_base_root_v1(group_index, native_trace_log2, commitment.commitment.root)
    }

    fn accept_decoded_base_groups_v1(
        &mut self,
        trace_groups: &[TraceGroupProofV1],
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.validate_state_v1()?;
        if self.next_group != 0
            || trace_groups.len() != ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1
            || trace_groups
                .iter()
                .any(|group| group.base_root == [0_u8; 32])
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        for (group_index, group) in trace_groups.iter().enumerate() {
            self.accept_base_root_v1(
                group_index,
                MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1[group_index],
                group.base_root,
            )?;
        }
        Ok(())
    }

    fn complete_v1(self) -> Result<ZkX509CompletedMainBaseCommitmentSessionV1, ZkX509StarkErrorV1> {
        self.validate_state_v1()?;
        if self.next_group != ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1
            || self.recorded.iter().any(|recorded| !*recorded)
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        Ok(ZkX509CompletedMainBaseCommitmentSessionV1 {
            consensus_context_digest: self.consensus_context_digest,
            main_profile_digest: self.main_profile_digest,
            roots: self.roots,
        })
    }

    fn finish_pre_aux_v1(self) -> Result<ZkX509CredentialMainPreAuxV1, ZkX509StarkErrorV1> {
        Ok(ZkX509CredentialMainPreAuxV1::from_completed_main_base_session_v1(self.complete_v1()?))
    }
}

#[derive(Clone)]
struct IoTraceMaterialV1 {
    layout: SegmentLayoutV1,
    /// Statement-compiled non-padding prefix; independent of the fixed MAIN
    /// registration extent carried by `layout.active_rows`.
    logical_active_rows: usize,
    base_columns: Vec<Vec<F>>,
    aux_columns: Vec<Vec<F>>,
    fixed_columns: Vec<Vec<F>>,
}

#[derive(Clone)]
struct ProjectionTraceMaterialV1 {
    layout: SegmentLayoutV1,
    base_columns: Vec<Vec<F>>,
    aux_columns: Vec<Vec<F>>,
    fixed_columns: Vec<Vec<F>>,
}

/// Verifier-bound public terminals for the sole canonical accumulator set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AccumulatorRegistrationPublicV1 {
    ca_membership: ZkX509CaAccumulatorStarkPublicV1,
}

impl AccumulatorRegistrationPublicV1 {
    fn validate(self) -> Result<(), ZkX509StarkErrorV1> {
        if self
            .ca_membership
            .governed_root
            .iter()
            .any(|value| F::canonical(value.0).is_none() || value.0 > u64::from(u8::MAX))
        {
            return Err(ZkX509StarkErrorV1::InvalidStatement);
        }
        Ok(())
    }
}

/// Trace-derived registration compiled before any challenge-dependent columns.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AccumulatorTraceRegistrationV1 {
    layout: AggregateProofLayoutV1,
    schedule: ZkX509ShaCallScheduleV1,
    public: AccumulatorRegistrationPublicV1,
}

impl AccumulatorTraceRegistrationV1 {
    fn validate(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_accumulator_registration_v1()?;
        self.public.validate()?;
        validate_accumulator_schedule_v1(&self.schedule)
    }
}

/// Exact compact-CA numeric material consumed by the aggregate commitment path.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AccumulatorTraceMaterialV1 {
    registration: AccumulatorTraceRegistrationV1,
    ca_membership: ZkX509CaAccumulatorStarkMaterialV1,
}

impl AccumulatorTraceMaterialV1 {
    fn validate(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.registration.validate()?;
        validate_accumulator_material_shape_v1(
            &self.ca_membership,
            canonical_accumulator_segment_layouts_v1()?[0],
        )
    }
}

/// Verifier-derived registration for one complete, one-signature P-256 AIR.
#[derive(Clone, Debug, PartialEq, Eq)]
struct P256TraceRegistrationV1 {
    role: P256EcdsaRoleV1,
    layout: AggregateProofLayoutV1,
}

impl P256TraceRegistrationV1 {
    fn new_v1(role: P256EcdsaRoleV1) -> Result<Self, ZkX509StarkErrorV1> {
        let registration = Self {
            role,
            layout: AggregateProofLayoutV1::for_p256_v1(role)?,
        };
        registration.validate()?;
        Ok(registration)
    }

    fn validate(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_p256_registration_v1(self.role)
    }
}

/// Four independent post-base-commitment P-256 challenge families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct P256AggregateChallengesV1 {
    value: P256ValueBusChallengesV1,
    cross: P256CrossTraceChallengesV1,
    scalar: P256ScalarBitBusChallengesV1,
    arithmetic_copy: P256ArithmeticCopyChallengesV1,
}

impl P256AggregateChallengesV1 {
    fn validate(self) -> Result<(), ZkX509StarkErrorV1> {
        self.value
            .validate()
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        self.cross
            .validate()
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        self.scalar
            .validate_v1()
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        self.arithmetic_copy
            .validate_v1()
            .map_err(ZkX509StarkErrorV1::from)
    }
}

fn derive_p256_aggregate_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<P256AggregateChallengesV1, ZkX509StarkErrorV1> {
    let challenges = P256AggregateChallengesV1 {
        value: derive_zk_x509_p256_value_bus_challenges_v1(transcript)
            .map_err(map_transparent_error_v1)?,
        cross: derive_zk_x509_p256_cross_trace_challenges_v1(transcript)
            .map_err(map_transparent_error_v1)?,
        scalar: derive_zk_x509_p256_scalar_bit_bus_challenges_v1(transcript)
            .map_err(map_transparent_error_v1)?,
        arithmetic_copy: derive_p256_arithmetic_copy_challenges_v1(transcript)
            .map_err(map_transparent_error_v1)?,
    };
    challenges.validate()?;
    Ok(challenges)
}

/// Proof-encoded P-256 product terminals in their sole legal role order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct P256TerminalRegistrationV1 {
    buses: P256BusTerminalClaimsV1,
    cross_sources: Vec<P256CrossTraceTerminalClaimV1>,
    sink: [F; P256_CROSS_TRACE_LANES_V1],
}

impl P256TerminalRegistrationV1 {
    fn validate(&self, role: P256EcdsaRoleV1) -> Result<(), ZkX509StarkErrorV1> {
        let bus_residues = evaluate_p256_bus_terminal_claim_equalities_v1(self.buses)
            .map_err(ZkX509StarkErrorV1::from)?;
        let cross_residues = evaluate_p256_cross_trace_terminal_claim_equalities_v1(
            role,
            &self.cross_sources,
            self.sink,
        )
        .map_err(ZkX509StarkErrorV1::from)?;
        if bus_residues
            .iter()
            .chain(&cross_residues)
            .any(|residue| *residue != F::ZERO)
        {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(())
    }

    fn cross_claim(
        &self,
        role: P256CrossTraceTerminalRoleV1,
    ) -> Result<P256CrossTraceTerminalClaimV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .cross_sources
            .iter()
            .copied()
            .filter(|claim| claim.role == role);
        let claim = matches.next().ok_or(ZkX509StarkErrorV1::P256Witness)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(claim)
    }
}

impl Drop for P256TerminalRegistrationV1 {
    fn drop(&mut self) {
        zeroize_p256_terminal_registration_v1(self);
    }
}

/// Verifier-side P-256 fixed openings paired with the proof terminal envelope.
///
/// Native base and auxiliary columns are streamed by the providers in
/// `p256_aggregate_adapter`; retaining only these sampled verifier-derived
/// rows keeps verification bounded independently of the million-row value bus.
#[derive(Clone, Debug, PartialEq, Eq)]
struct P256OpenedMaterialV1 {
    registration: P256TraceRegistrationV1,
    terminals: P256TerminalRegistrationV1,
    fixed_openings: Vec<BTreeMap<usize, Vec<F>>>,
}

impl P256OpenedMaterialV1 {
    fn validate(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.registration.validate()?;
        self.terminals.validate(self.registration.role)?;
        if self.fixed_openings.len() != self.registration.layout.registered_segments.len()
            || self
                .fixed_openings
                .iter()
                .zip(&self.registration.layout.registered_segments)
                .any(|(openings, registration)| {
                    openings.is_empty()
                        || openings
                            .values()
                            .any(|row| row.len() != registration.segment.fixed_width)
                })
        {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(())
    }
}

fn validate_accumulator_material_shape_v1(
    material: &ZkX509CaAccumulatorStarkMaterialV1,
    layout: SegmentLayoutV1,
) -> Result<(), ZkX509StarkErrorV1> {
    if material.base_columns.len() != layout.base_width
        || material.aux_columns.len() != layout.aux_width
        || material.fixed_columns.len() != layout.fixed_width
        || material
            .base_columns
            .iter()
            .chain(&material.aux_columns)
            .chain(&material.fixed_columns)
            .any(|column| column.len() != layout.trace_size())
    {
        return Err(ZkX509StarkErrorV1::AccumulatorWitness);
    }
    Ok(())
}

fn canonical_accumulator_manifest_address_v1(
    call: usize,
) -> Result<ZkX509ShaCallRoleV1, ZkX509StarkErrorV1> {
    if call == ZK_X509_SHA_CA_LEAF_CALL_V1 {
        return Ok(ZkX509ShaCallRoleV1::CaLeaf);
    }
    call.checked_sub(ZK_X509_SHA_CA_NODE_CALL_START_V1)
        .filter(|level| *level + 1 < ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1)
        .and_then(|level| u8::try_from(level).ok())
        .map(ZkX509ShaCallRoleV1::CaNode)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
}

fn validate_accumulator_schedule_v1(
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<(), ZkX509StarkErrorV1> {
    for call in ZK_X509_SHA_CA_LEAF_CALL_V1
        ..ZK_X509_SHA_CA_LEAF_CALL_V1 + ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1
    {
        let manifest = schedule
            .call(call)
            .map_err(|_| ZkX509StarkErrorV1::AccumulatorWitness)?;
        if usize::from(manifest.call) != call
            || manifest.role != canonical_accumulator_manifest_address_v1(call)?
            || manifest.activation != ZkX509ShaCallActivationV1::Required
            || manifest.maximum_message_bytes == 0
            || manifest.maximum_blocks != 3
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    Ok(())
}

fn accumulator_schedule_digest_v1(
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<[u8; 32], ZkX509StarkErrorV1> {
    validate_accumulator_schedule_v1(schedule)?;
    let mut encoding = Vec::new();
    encoding
        .try_reserve_exact(4 + schedule.calls().len() * 36)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    append_u32_v1(
        &mut encoding,
        u32::try_from(schedule.shape().disclosed_attributes)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
    );
    for manifest in schedule.calls() {
        encoding.push(manifest.call);
        encoding.push(manifest.role.role_code());
        encoding.push(match manifest.activation {
            ZkX509ShaCallActivationV1::Required => 0,
            ZkX509ShaCallActivationV1::OptionalPrivate => 1,
            ZkX509ShaCallActivationV1::Inactive => 2,
        });
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.maximum_message_bytes)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.maximum_blocks)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.first_event).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.first_logical_row)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
    }
    sha256_frame_v1(ACCUMULATOR_SCHEDULE_DIGEST_DOMAIN_V1, &[&encoding])
        .map_err(map_transparent_error_v1)
}

fn compile_accumulator_trace_registration_v1(
    ca_membership: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<AccumulatorTraceRegistrationV1, ZkX509StarkErrorV1> {
    ca_membership
        .validate()
        .map_err(|_| ZkX509StarkErrorV1::AccumulatorWitness)?;
    validate_accumulator_schedule_v1(schedule)?;
    let registration = AccumulatorTraceRegistrationV1 {
        layout: AggregateProofLayoutV1::for_accumulators_v1()?,
        public: AccumulatorRegistrationPublicV1 {
            ca_membership: ca_accumulator_stark_public_v1(ca_membership, schedule)?,
        },
        schedule: schedule.clone(),
    };
    registration.validate()?;
    Ok(registration)
}

fn build_accumulator_trace_material_v1(
    ca_membership: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<AccumulatorTraceMaterialV1, ZkX509StarkErrorV1> {
    let registration = compile_accumulator_trace_registration_v1(ca_membership, schedule)?;
    let material = AccumulatorTraceMaterialV1 {
        ca_membership: build_ca_accumulator_stark_material_v1(
            ca_membership,
            &registration.schedule,
            sha_challenges,
            io_challenges,
        )?,
        registration,
    };
    material.validate()?;
    Ok(material)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkX509SegmentedStarkProofV1 {
    aggregate: aggregate::AggregateStarkProofV1,
    deep: aggregate::AggregateDeepProofV1,
}

impl core::ops::Deref for ZkX509SegmentedStarkProofV1 {
    type Target = aggregate::AggregateStarkProofV1;

    fn deref(&self) -> &Self::Target {
        &self.aggregate
    }
}

impl core::ops::DerefMut for ZkX509SegmentedStarkProofV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.aggregate
    }
}

type TraceGroupProofV1 = aggregate::AggregateTraceGroupProofV1;
type FriLaneProofV1 = aggregate::AggregateFriLaneProofV1;

type FriLaneMaterialV1 = aggregate::AggregateFriLaneMaterialV1;

fn role_field_v1(role: ZkX509IoSegmentRoleV1) -> F {
    F(match role {
        ZkX509IoSegmentRoleV1::StrictDer => 1,
        ZkX509IoSegmentRoleV1::Sha256 => 2,
        ZkX509IoSegmentRoleV1::P256 => 3,
        ZkX509IoSegmentRoleV1::CaAccumulator => 4,
        ZkX509IoSegmentRoleV1::CrlCommitment => 5,
        ZkX509IoSegmentRoleV1::Projection => 6,
        ZkX509IoSegmentRoleV1::PublicInput => 7,
    })
}

fn io_active_rows_v1(
    declarations: &[ZkX509IoChannelDeclarationV1],
) -> Result<usize, ZkX509StarkErrorV1> {
    declarations.iter().try_fold(0_usize, |rows, declaration| {
        let endpoints = declaration
            .consumers
            .len()
            .checked_add(1)
            .ok_or(ZkX509StarkErrorV1::InvalidStatement)?;
        let bytes = usize::try_from(declaration.byte_len)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        rows.checked_add(
            endpoints
                .checked_mul(bytes)
                .ok_or(ZkX509StarkErrorV1::InvalidStatement)?,
        )
        .ok_or(ZkX509StarkErrorV1::InvalidStatement)
    })
}

fn access_base_fields_v1(access: IoAccessV1, row: &mut [F], offset: usize, bits: usize) {
    row[offset] = access.channel;
    row[offset + 1] = access.offset;
    row[offset + 2] = access.value;
    row[offset + 3] = access.is_write;
    row[offset + 4] = role_field_v1(access.endpoint.role);
    row[offset + 5] = F(u64::from(access.endpoint.instance));
    for bit in 0..8 {
        row[bits + bit] = F((access.value.0 >> bit) & 1);
    }
}

#[cfg(test)]
fn access_fixed_fields_v1(access: IoAccessV1, row: &mut [F], offset: usize) {
    row[offset] = access.channel;
    row[offset + 1] = access.offset;
    row[offset + 2] = access.is_write;
    row[offset + 3] = role_field_v1(access.endpoint.role);
    row[offset + 4] = F(u64::from(access.endpoint.instance));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MainIoAccessTopologyV1 {
    channel: F,
    offset: F,
    is_write: F,
    role: F,
    instance: F,
}

impl MainIoAccessTopologyV1 {
    fn new_v1(
        channel: u32,
        offset: usize,
        is_write: bool,
        endpoint: ZkX509IoEndpointV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        Ok(Self {
            channel: F(u64::from(channel)),
            offset: F(u64::try_from(offset).map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?),
            is_write: if is_write { F::ONE } else { F::ZERO },
            role: role_field_v1(endpoint.role),
            instance: F(u64::from(endpoint.instance)),
        })
    }

    fn write_fixed_fields_v1(self, row: &mut [F], offset: usize) {
        row[offset] = self.channel;
        row[offset + 1] = self.offset;
        row[offset + 2] = self.is_write;
        row[offset + 3] = self.role;
        row[offset + 4] = self.instance;
    }

    fn matches_access_v1(self, access: IoAccessV1) -> bool {
        self.channel == access.channel
            && self.offset == access.offset
            && self.is_write == access.is_write
            && self.role == role_field_v1(access.endpoint.role)
            && self.instance == F(u64::from(access.endpoint.instance))
    }

    fn same_address_v1(self, other: Self) -> bool {
        self.channel == other.channel && self.offset == other.offset
    }
}

/// Verifier-owned MAIN I/O fixed schedule compiled only from public channel
/// declarations.
///
/// The schedule deliberately stores no byte value from an execution or sorted
/// witness. Execution-order and address-order topology are compiled directly
/// from declarations, and public bytes are retained only for the dedicated
/// public-input selector.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MainIoFixedScheduleV1 {
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    execution: Vec<MainIoAccessTopologyV1>,
    sorted: Vec<MainIoAccessTopologyV1>,
    execution_public_values: Vec<Option<F>>,
}

impl MainIoFixedScheduleV1 {
    fn compile_v1(
        layout: SegmentLayoutV1,
        statement: &ZkX509IoStarkStatementV1,
        logical_active_rows: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_io_logical_geometry_v1(layout, logical_active_rows)?;
        validate_declarations_v1(statement.declarations())
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        if io_active_rows_v1(statement.declarations())? != logical_active_rows {
            return Err(ZkX509StarkErrorV1::InvalidStatement);
        }

        let mut execution = Vec::new();
        let mut sorted = Vec::new();
        let mut execution_public_values = Vec::new();
        execution
            .try_reserve_exact(logical_active_rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        sorted
            .try_reserve_exact(logical_active_rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        execution_public_values
            .try_reserve_exact(logical_active_rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;

        for declaration in statement.declarations() {
            let byte_len = usize::try_from(declaration.byte_len)
                .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
            for offset in 0..byte_len {
                execution.push(MainIoAccessTopologyV1::new_v1(
                    declaration.channel,
                    offset,
                    true,
                    declaration.producer,
                )?);
                execution_public_values.push(None);
            }
            for endpoint in declaration.consumers.iter().copied() {
                for offset in 0..byte_len {
                    execution.push(MainIoAccessTopologyV1::new_v1(
                        declaration.channel,
                        offset,
                        false,
                        endpoint,
                    )?);
                    let public_value = if endpoint.role == ZkX509IoSegmentRoleV1::PublicInput {
                        Some(F(u64::from(
                            declaration
                                .public_value
                                .as_ref()
                                .and_then(|value| value.get(offset))
                                .copied()
                                .ok_or(ZkX509StarkErrorV1::InvalidStatement)?,
                        )))
                    } else {
                        None
                    };
                    execution_public_values.push(public_value);
                }
            }

            // This is exactly the stable `(channel, offset, write/read)` order
            // used by the native I/O table builder: one write followed by the
            // declaration-canonical consumer order for each address.
            for offset in 0..byte_len {
                sorted.push(MainIoAccessTopologyV1::new_v1(
                    declaration.channel,
                    offset,
                    true,
                    declaration.producer,
                )?);
                for endpoint in declaration.consumers.iter().copied() {
                    sorted.push(MainIoAccessTopologyV1::new_v1(
                        declaration.channel,
                        offset,
                        false,
                        endpoint,
                    )?);
                }
            }
        }
        if execution.len() != logical_active_rows
            || sorted.len() != logical_active_rows
            || execution_public_values.len() != logical_active_rows
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(Self {
            layout,
            logical_active_rows,
            execution,
            sorted,
            execution_public_values,
        })
    }

    fn fixed_row_v1(&self, index: usize) -> Result<[F; IO_FIXED_WIDTH], ZkX509StarkErrorV1> {
        if index >= self.layout.trace_size() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut fixed = [F::ZERO; IO_FIXED_WIDTH];
        if index < self.logical_active_rows {
            self.execution[index].write_fixed_fields_v1(&mut fixed, FIX_EXEC_CHANNEL);
            self.sorted[index].write_fixed_fields_v1(&mut fixed, FIX_SORT_CHANNEL);
            if let Some(value) = self.execution_public_values[index] {
                fixed[FIX_PUBLIC_SELECTOR] = F::ONE;
                fixed[FIX_PUBLIC_VALUE] = value;
            }
            if index + 1 < self.logical_active_rows
                && self.sorted[index].same_address_v1(self.sorted[index + 1])
            {
                fixed[FIX_SORT_SAME_ADDRESS_NEXT] = F::ONE;
            }
        }
        let [active, first, last_active, transition] =
            io_fixed_selector_fields_v1(index, self.logical_active_rows, self.layout.trace_size())?;
        fixed[FIX_ACTIVE] = active;
        fixed[FIX_FIRST] = first;
        fixed[FIX_LAST_ACTIVE] = last_active;
        fixed[FIX_TRANSITION] = transition;
        Ok(fixed)
    }

    fn fixed_columns_v1(&self) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
        let mut fixed_columns =
            allocate_column_matrix_v1(IO_FIXED_WIDTH, self.layout.trace_size())?;
        for index in 0..self.layout.trace_size() {
            push_row_to_columns_v1(&mut fixed_columns, &self.fixed_row_v1(index)?)?;
        }
        Ok(fixed_columns)
    }

    fn validate_witness_topology_v1(
        &self,
        execution: &[IoAccessV1],
        sorted: &[IoAccessV1],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if execution.len() != self.logical_active_rows
            || sorted.len() != self.logical_active_rows
            || execution
                .iter()
                .copied()
                .zip(self.execution.iter().copied())
                .any(|(actual, expected)| !expected.matches_access_v1(actual))
            || sorted
                .iter()
                .copied()
                .zip(self.sorted.iter().copied())
                .any(|(actual, expected)| !expected.matches_access_v1(actual))
        {
            return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
        }
        Ok(())
    }
}

fn transpose_rows_v1(rows: &[Vec<F>], width: usize) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    if rows.is_empty() || rows.iter().any(|row| row.len() != width) {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let mut columns = (0..width)
        .map(|_| Vec::with_capacity(rows.len()))
        .collect::<Vec<_>>();
    for row in rows {
        for (column, value) in columns.iter_mut().zip(row.iter().copied()) {
            column.push(value);
        }
    }
    Ok(columns)
}

fn transpose_array_rows_v1<const WIDTH: usize>(
    rows: &[[F; WIDTH]],
) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    if rows.is_empty() {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let mut columns = (0..WIDTH)
        .map(|_| Vec::with_capacity(rows.len()))
        .collect::<Vec<_>>();
    for row in rows {
        for (column, value) in columns.iter_mut().zip(row.iter().copied()) {
            column.push(value);
        }
    }
    Ok(columns)
}

fn allocate_column_matrix_v1(width: usize, rows: usize) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(width)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for _ in 0..width {
        let mut column = Vec::new();
        column
            .try_reserve_exact(rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        columns.push(column);
    }
    Ok(columns)
}

fn push_row_to_columns_v1(columns: &mut [Vec<F>], row: &[F]) -> Result<(), ZkX509StarkErrorV1> {
    if columns.len() != row.len() {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    for (column, value) in columns.iter_mut().zip(row.iter().copied()) {
        column.push(value);
    }
    Ok(())
}

fn row_at_v1(columns: &[Vec<F>], index: usize) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    columns
        .iter()
        .map(|column| {
            column
                .get(index)
                .copied()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)
        })
        .collect()
}

fn copy_row_at_v1(
    columns: &[Vec<F>],
    index: usize,
    row: &mut [F],
) -> Result<(), ZkX509StarkErrorV1> {
    if columns.len() != row.len() {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    for (value, column) in row.iter_mut().zip(columns) {
        *value = column
            .get(index)
            .copied()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    }
    Ok(())
}

#[cfg(test)]
fn topology_witnesses_v1(
    declarations: &[ZkX509IoChannelDeclarationV1],
) -> Result<Vec<ZkX509IoChannelWitnessV1>, ZkX509StarkErrorV1> {
    declarations
        .iter()
        .map(|declaration| {
            let byte_len = usize::try_from(declaration.byte_len)
                .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
            let common_value = declaration
                .public_value
                .clone()
                .unwrap_or_else(|| vec![0_u8; byte_len]);
            let consumer_values = declaration
                .consumers
                .iter()
                .map(|endpoint| {
                    if endpoint.role == ZkX509IoSegmentRoleV1::PublicInput {
                        declaration
                            .public_value
                            .clone()
                            .ok_or(ZkX509StarkErrorV1::InvalidStatement)
                    } else {
                        Ok(common_value.clone())
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ZkX509IoChannelWitnessV1 {
                declaration: declaration.clone(),
                producer_value: common_value,
                consumer_values,
            })
        })
        .collect()
}

#[cfg(test)]
fn public_value_for_access_v1(
    declarations: &[ZkX509IoChannelDeclarationV1],
    access: IoAccessV1,
) -> Result<Option<u8>, ZkX509StarkErrorV1> {
    if access.endpoint.role != ZkX509IoSegmentRoleV1::PublicInput {
        return Ok(None);
    }
    let declaration = declarations
        .get(usize::try_from(access.channel.0).map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?)
        .ok_or(ZkX509StarkErrorV1::InvalidStatement)?;
    declaration
        .public_value
        .as_ref()
        .and_then(|value| value.get(access.offset.0 as usize))
        .copied()
        .map(Some)
        .ok_or(ZkX509StarkErrorV1::InvalidStatement)
}

fn validate_io_logical_geometry_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
) -> Result<(), ZkX509StarkErrorV1> {
    layout.validate()?;
    if logical_active_rows == 0
        || logical_active_rows > layout.trace_size()
        || logical_active_rows > ZK_X509_IO_FIXED_CAPACITY_ROWS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let focused = SegmentLayoutV1::for_io(logical_active_rows)?;
    let full = SegmentLayoutV1::for_full_io()?;
    if layout != focused && layout != full {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

fn io_fixed_selector_fields_v1(
    index: usize,
    logical_active_rows: usize,
    trace_size: usize,
) -> Result<[F; 4], ZkX509StarkErrorV1> {
    if logical_active_rows == 0 || logical_active_rows > trace_size || index >= trace_size {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok([
        F(u64::from(index < logical_active_rows)),
        F(u64::from(index == 0)),
        F(u64::from(index + 1 == logical_active_rows)),
        F(u64::from(index + 1 < trace_size)),
    ])
}

fn build_io_base_and_fixed_columns_for_layout_v1(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
) -> Result<(Vec<Vec<F>>, Vec<Vec<F>>, Vec<IoAccessV1>, Vec<IoAccessV1>), ZkX509StarkErrorV1> {
    let fixed_schedule = MainIoFixedScheduleV1::compile_v1(layout, statement, logical_active_rows)?;
    build_io_base_and_fixed_columns_from_schedule_v1(statement, witnesses, &fixed_schedule)
}

fn build_io_base_and_fixed_columns_from_schedule_v1(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
    fixed_schedule: &MainIoFixedScheduleV1,
) -> Result<(Vec<Vec<F>>, Vec<Vec<F>>, Vec<IoAccessV1>, Vec<IoAccessV1>), ZkX509StarkErrorV1> {
    let layout = fixed_schedule.layout;
    let logical_active_rows = fixed_schedule.logical_active_rows;
    if witnesses.len() != statement.declarations.len()
        || witnesses
            .iter()
            .zip(&statement.declarations)
            .any(|(witness, declaration)| witness.declaration != *declaration)
    {
        return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
    }
    let (declarations, execution, sorted) = build_zk_x509_io_base_tables_v1(witnesses)?;
    if declarations != statement.declarations {
        return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
    }
    if execution.len() != logical_active_rows || sorted.len() != logical_active_rows {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    fixed_schedule.validate_witness_topology_v1(&execution, &sorted)?;

    let mut base_columns = allocate_column_matrix_v1(IO_BASE_WIDTH, layout.trace_size())?;
    for index in 0..layout.trace_size() {
        let mut base = [F::ZERO; IO_BASE_WIDTH];
        if index < logical_active_rows {
            let execution_access = execution[index];
            let sorted_access = sorted[index];
            access_base_fields_v1(execution_access, &mut base, EXEC_CHANNEL, EXEC_BITS);
            access_base_fields_v1(sorted_access, &mut base, SORT_CHANNEL, SORT_BITS);
        }
        push_row_to_columns_v1(&mut base_columns, &base)?;
    }
    let fixed_columns = fixed_schedule.fixed_columns_v1()?;
    Ok((base_columns, fixed_columns, execution, sorted))
}

fn build_io_base_and_fixed_columns_v1(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
) -> Result<
    (
        SegmentLayoutV1,
        Vec<Vec<F>>,
        Vec<Vec<F>>,
        Vec<IoAccessV1>,
        Vec<IoAccessV1>,
    ),
    ZkX509StarkErrorV1,
> {
    let logical_active_rows = io_active_rows_v1(&statement.declarations)?;
    let layout = SegmentLayoutV1::for_io(logical_active_rows)?;
    let (base_columns, fixed_columns, execution, sorted) =
        build_io_base_and_fixed_columns_for_layout_v1(
            statement,
            witnesses,
            layout,
            logical_active_rows,
        )?;
    Ok((layout, base_columns, fixed_columns, execution, sorted))
}

fn build_io_aux_columns_v1(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
    challenges: ZkX509IoChallengesV1,
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    expected_execution: &[IoAccessV1],
    expected_sorted: &[IoAccessV1],
) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    validate_io_logical_geometry_v1(layout, logical_active_rows)?;
    let trace = build_zk_x509_io_trace_v1(witnesses, challenges)?;
    if trace.declarations != statement.declarations
        || trace.execution != expected_execution
        || trace.sorted != expected_sorted
        || trace.permutation_rows.len() != logical_active_rows
        || expected_execution.len() != logical_active_rows
        || expected_sorted.len() != logical_active_rows
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let final_row = trace
        .permutation_rows
        .last()
        .ok_or(ZkX509StarkErrorV1::IoWitness)?;
    let mut aux_columns = allocate_column_matrix_v1(IO_AUX_WIDTH, layout.trace_size())?;
    let logical_active_rows_field =
        F(u64::try_from(logical_active_rows).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
    for index in 0..layout.trace_size() {
        let mut row = [F::ZERO; IO_AUX_WIDTH];
        if let Some(source) = trace.permutation_rows.get(index) {
            row[AUX_EXEC_BEFORE..AUX_EXEC_BEFORE + IO_LANES]
                .copy_from_slice(&source.execution_product_before);
            row[AUX_SORT_BEFORE..AUX_SORT_BEFORE + IO_LANES]
                .copy_from_slice(&source.sorted_product_before);
            row[AUX_EXEC_AFTER..AUX_EXEC_AFTER + IO_LANES]
                .copy_from_slice(&source.execution_product_after);
            row[AUX_SORT_AFTER..AUX_SORT_AFTER + IO_LANES]
                .copy_from_slice(&source.sorted_product_after);
        } else {
            row[AUX_EXEC_BEFORE..AUX_EXEC_BEFORE + IO_LANES]
                .copy_from_slice(&final_row.execution_product_after);
            row[AUX_SORT_BEFORE..AUX_SORT_BEFORE + IO_LANES]
                .copy_from_slice(&final_row.sorted_product_after);
            row[AUX_EXEC_AFTER..AUX_EXEC_AFTER + IO_LANES]
                .copy_from_slice(&final_row.execution_product_after);
            row[AUX_SORT_AFTER..AUX_SORT_AFTER + IO_LANES]
                .copy_from_slice(&final_row.sorted_product_after);
        }
        row[AUX_CONT_SEGMENT_INDEX] = F::ZERO;
        row[AUX_CONT_GLOBAL_START] = F::ZERO;
        row[AUX_CONT_GLOBAL_END] = logical_active_rows_field;
        row[AUX_CONT_LOCAL_START] = F::ZERO;
        row[AUX_CONT_LOCAL_END] = F::ZERO;
        row[AUX_CONT_MEMORY_START] = F::ZERO;
        row[AUX_CONT_MEMORY_END] = logical_active_rows_field;
        row[AUX_CONT_EXEC_START..AUX_CONT_EXEC_START + IO_LANES].fill(F::ONE);
        row[AUX_CONT_EXEC_END..AUX_CONT_EXEC_END + IO_LANES]
            .copy_from_slice(&final_row.execution_product_after);
        row[AUX_CONT_SORT_START..AUX_CONT_SORT_START + IO_LANES].fill(F::ONE);
        row[AUX_CONT_SORT_END..AUX_CONT_SORT_END + IO_LANES]
            .copy_from_slice(&final_row.sorted_product_after);
        push_row_to_columns_v1(&mut aux_columns, &row)?;
    }
    Ok(aux_columns)
}

fn validate_io_base_phase_shape_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    base_columns: &[Vec<F>],
    fixed_columns: &[Vec<F>],
) -> Result<(), ZkX509StarkErrorV1> {
    validate_io_logical_geometry_v1(layout, logical_active_rows)?;
    let trace_size = layout.trace_size();
    if base_columns.len() != IO_BASE_WIDTH
        || fixed_columns.len() != IO_FIXED_WIDTH
        || base_columns
            .iter()
            .chain(fixed_columns)
            .any(|column| column.len() != trace_size)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    if base_columns
        .iter()
        .chain(fixed_columns)
        .flatten()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::IoWitness);
    }
    for index in 0..trace_size {
        let [active, first, last_active, transition] =
            io_fixed_selector_fields_v1(index, logical_active_rows, trace_size)?;
        if fixed_columns[FIX_ACTIVE][index] != active
            || fixed_columns[FIX_FIRST][index] != first
            || fixed_columns[FIX_LAST_ACTIVE][index] != last_active
            || fixed_columns[FIX_TRANSITION][index] != transition
        {
            return Err(ZkX509StarkErrorV1::IoWitness);
        }
        if index >= logical_active_rows
            && (base_columns.iter().any(|column| column[index] != F::ZERO)
                || fixed_columns
                    .iter()
                    .enumerate()
                    .any(|(column, values)| column != FIX_TRANSITION && values[index] != F::ZERO))
        {
            return Err(ZkX509StarkErrorV1::IoWitness);
        }
    }
    Ok(())
}

fn validate_io_bound_material_shape_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    base_columns: &[Vec<F>],
    aux_columns: &[Vec<F>],
    fixed_columns: &[Vec<F>],
) -> Result<(), ZkX509StarkErrorV1> {
    validate_io_base_phase_shape_v1(layout, logical_active_rows, base_columns, fixed_columns)?;
    let trace_size = layout.trace_size();
    if aux_columns.len() != IO_AUX_WIDTH
        || aux_columns.iter().any(|column| column.len() != trace_size)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    if aux_columns
        .iter()
        .flatten()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::IoWitness);
    }
    let logical_active_rows_field =
        F(u64::try_from(logical_active_rows).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
    for index in 0..trace_size {
        for (column, expected) in [
            (AUX_CONT_SEGMENT_INDEX, F::ZERO),
            (AUX_CONT_GLOBAL_START, F::ZERO),
            (AUX_CONT_GLOBAL_END, logical_active_rows_field),
            (AUX_CONT_LOCAL_START, F::ZERO),
            (AUX_CONT_LOCAL_END, F::ZERO),
            (AUX_CONT_MEMORY_START, F::ZERO),
            (AUX_CONT_MEMORY_END, logical_active_rows_field),
        ] {
            if aux_columns[column][index] != expected {
                return Err(ZkX509StarkErrorV1::IoWitness);
            }
        }
        for start in [AUX_CONT_EXEC_START, AUX_CONT_SORT_START] {
            if (0..IO_LANES).any(|lane| aux_columns[start + lane][index] != F::ONE) {
                return Err(ZkX509StarkErrorV1::IoWitness);
            }
        }
    }
    Ok(())
}

fn validate_io_trace_material_shape_v1(
    material: &IoTraceMaterialV1,
) -> Result<(), ZkX509StarkErrorV1> {
    validate_io_bound_material_shape_v1(
        material.layout,
        material.logical_active_rows,
        &material.base_columns,
        &material.aux_columns,
        &material.fixed_columns,
    )
}

fn build_projection_base_material_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    witness: &ZkX509ProjectionWitnessV1,
) -> Result<
    (
        SegmentLayoutV1,
        ZkX509ProjectionTraceV1,
        Vec<Vec<F>>,
        Vec<Vec<F>>,
    ),
    ZkX509StarkErrorV1,
> {
    let layout = SegmentLayoutV1::for_projection()?;
    layout.validate()?;
    let trace = build_zk_x509_projection_trace_v1(statement, witness)?;
    let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(statement)?;
    if trace.base.rows.len() != layout.trace_size()
        || trace.fixed.rows.len() != layout.trace_size()
        || fixed_rows.len() != layout.trace_size()
    {
        return Err(ZkX509StarkErrorV1::ProjectionWitness);
    }
    let base_columns = transpose_array_rows_v1(&trace.base.rows)?;
    let fixed_columns = transpose_array_rows_v1(&fixed_rows)?;
    Ok((layout, trace, base_columns, fixed_columns))
}

fn derive_projection_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<ZkX509ProjectionChallengesV1, ZkX509StarkErrorV1> {
    let mut sampled = [F::ZERO; ZK_X509_PROJECTION_COPY_LANES_V1 * 7];
    for (index, challenge) in sampled.iter_mut().enumerate() {
        let label = ZK_X509_PROJECTION_CHALLENGE_LABELS_V1[index / 7][index % 7];
        *challenge = transcript
            .challenge_field(label)
            .map_err(map_transparent_error_v1)?;
    }
    Ok(ZkX509ProjectionChallengesV1 {
        copy: core::array::from_fn(|lane| ZkX509ProjectionCopyChallengesV1 {
            beta: sampled[lane * 7],
            gamma: sampled[lane * 7 + 1],
        }),
        compaction: core::array::from_fn(|lane| ZkX509ProjectionCompactionChallengesV1 {
            active: sampled[lane * 7 + 2],
            invocation: sampled[lane * 7 + 3],
            position: sampled[lane * 7 + 4],
            value: sampled[lane * 7 + 5],
            gamma: sampled[lane * 7 + 6],
        }),
    })
}

#[allow(dead_code)]
fn derive_der_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<ZkX509DerStarkChallengesV1, ZkX509StarkErrorV1> {
    let challenges =
        derive_zk_x509_der_stark_challenges_v1(transcript).map_err(map_transparent_error_v1)?;
    challenges
        .validate()
        .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    Ok(challenges)
}

fn build_projection_trace_material_v1(
    layout: SegmentLayoutV1,
    trace: ZkX509ProjectionTraceV1,
    base_columns: Vec<Vec<F>>,
    fixed_columns: Vec<Vec<F>>,
    challenges: ZkX509ProjectionChallengesV1,
) -> Result<ProjectionTraceMaterialV1, ZkX509StarkErrorV1> {
    let aux = build_zk_x509_projection_aux_trace_v1(&trace.base, &trace.fixed, challenges)?;
    if aux.rows.len() != layout.trace_size() {
        return Err(ZkX509StarkErrorV1::ProjectionWitness);
    }
    Ok(ProjectionTraceMaterialV1 {
        layout,
        base_columns,
        aux_columns: transpose_array_rows_v1(&aux.rows)?,
        fixed_columns,
    })
}

fn io_public_digest_v1(
    statement: &ZkX509IoStarkStatementV1,
) -> Result<[u8; 32], ZkX509StarkErrorV1> {
    let mut encoding = Vec::new();
    append_u16_v1(
        &mut encoding,
        u16::try_from(statement.declarations.len())
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?,
    );
    for declaration in &statement.declarations {
        append_u32_v1(&mut encoding, declaration.channel);
        encoding.push(role_field_v1(declaration.producer.role).0 as u8);
        append_u16_v1(&mut encoding, declaration.producer.instance);
        append_u16_v1(
            &mut encoding,
            u16::try_from(declaration.consumers.len())
                .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?,
        );
        for consumer in &declaration.consumers {
            encoding.push(role_field_v1(consumer.role).0 as u8);
            append_u16_v1(&mut encoding, consumer.instance);
        }
        append_u32_v1(&mut encoding, declaration.byte_len);
        match &declaration.public_value {
            Some(value) => {
                encoding.push(1);
                append_u32_v1(
                    &mut encoding,
                    u32::try_from(value.len()).map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?,
                );
                encoding.extend_from_slice(value);
            }
            None => encoding.push(0),
        }
    }
    sha256_frame_v1(PUBLIC_DIGEST_DOMAIN, &[&encoding])
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)
}

fn der_public_digest_v1(shape: &ZkX509DerStarkShapeV1) -> Result<[u8; 32], ZkX509StarkErrorV1> {
    shape
        .validate()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    sha256_frame_v1(
        DER_PUBLIC_DIGEST_DOMAIN,
        &[
            DER_SEGMENTED_PROOF_DESCRIPTOR_V1,
            ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1,
            shape.transcript_bytes(),
        ],
    )
    .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)
}

fn projection_public_digest_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<[u8; 32], ZkX509StarkErrorV1> {
    let statement_digest = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone())
        .digest()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    sha256_frame_v1(
        PROJECTION_PUBLIC_DIGEST_DOMAIN,
        &[
            statement_digest.as_bytes(),
            ZK_X509_PROJECTION_AIR_DESCRIPTOR_V1,
        ],
    )
    .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)
}

fn io_compress_access_v1(
    row: &[F],
    offset: usize,
    challenge: super::io_air::ZkX509IoLaneChallengesV1,
) -> F {
    challenge
        .beta
        .add(challenge.channel.mul(row[offset]))
        .add(challenge.offset.mul(row[offset + 1]))
        .add(challenge.value.mul(row[offset + 2]))
        .add(challenge.is_write.mul(row[offset + 3]))
}

fn io_constraint_residues_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    current_base: &[F],
    next_base: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    challenges: ZkX509IoChallengesV1,
) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    if current_base.len() != IO_BASE_WIDTH
        || next_base.len() != IO_BASE_WIDTH
        || current_aux.len() != IO_AUX_WIDTH
        || next_aux.len() != IO_AUX_WIDTH
        || fixed.len() != IO_FIXED_WIDTH
        || logical_active_rows == 0
        || logical_active_rows > layout.trace_size()
        || logical_active_rows > ZK_X509_IO_FIXED_CAPACITY_ROWS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut residues = Vec::with_capacity(IO_CONSTRAINT_COUNT);
    for (trace, expected) in [
        (EXEC_CHANNEL, FIX_EXEC_CHANNEL),
        (EXEC_OFFSET, FIX_EXEC_OFFSET),
        (EXEC_WRITE, FIX_EXEC_WRITE),
        (EXEC_ROLE, FIX_EXEC_ROLE),
        (EXEC_INSTANCE, FIX_EXEC_INSTANCE),
        (SORT_CHANNEL, FIX_SORT_CHANNEL),
        (SORT_OFFSET, FIX_SORT_OFFSET),
        (SORT_WRITE, FIX_SORT_WRITE),
        (SORT_ROLE, FIX_SORT_ROLE),
        (SORT_INSTANCE, FIX_SORT_INSTANCE),
    ] {
        residues.push(current_base[trace].sub(fixed[expected]));
    }
    for bit in 0..8 {
        for offset in [EXEC_BITS, SORT_BITS] {
            let value = current_base[offset + bit];
            residues.push(value.mul(value.sub(F::ONE)));
        }
    }
    for (value_offset, bits_offset) in [(EXEC_VALUE, EXEC_BITS), (SORT_VALUE, SORT_BITS)] {
        let packed = (0..8).fold(F::ZERO, |sum, bit| {
            sum.add(current_base[bits_offset + bit].mul(F(1_u64 << bit)))
        });
        residues.push(current_base[value_offset].sub(packed));
    }
    let inactive = F::ONE.sub(fixed[FIX_ACTIVE]);
    residues.push(inactive.mul(current_base[EXEC_VALUE]));
    residues.push(inactive.mul(current_base[SORT_VALUE]));
    residues.push(
        fixed[FIX_PUBLIC_SELECTOR].mul(current_base[EXEC_VALUE].sub(fixed[FIX_PUBLIC_VALUE])),
    );
    residues.push(
        fixed[FIX_SORT_SAME_ADDRESS_NEXT].mul(next_base[SORT_VALUE].sub(current_base[SORT_VALUE])),
    );
    for lane in 0..IO_LANES {
        let challenge = challenges.lanes[lane];
        residues.push(fixed[FIX_FIRST].mul(current_aux[AUX_EXEC_BEFORE + lane].sub(F::ONE)));
        residues.push(fixed[FIX_FIRST].mul(current_aux[AUX_SORT_BEFORE + lane].sub(F::ONE)));
        let active_exec = io_compress_access_v1(current_base, EXEC_CHANNEL, challenge);
        let active_sort = io_compress_access_v1(current_base, SORT_CHANNEL, challenge);
        let exec_factor = fixed[FIX_ACTIVE]
            .mul(active_exec)
            .add(F::ONE.sub(fixed[FIX_ACTIVE]));
        let sort_factor = fixed[FIX_ACTIVE]
            .mul(active_sort)
            .add(F::ONE.sub(fixed[FIX_ACTIVE]));
        residues.push(
            current_aux[AUX_EXEC_AFTER + lane]
                .sub(current_aux[AUX_EXEC_BEFORE + lane].mul(exec_factor)),
        );
        residues.push(
            current_aux[AUX_SORT_AFTER + lane]
                .sub(current_aux[AUX_SORT_BEFORE + lane].mul(sort_factor)),
        );
        residues.push(
            fixed[FIX_TRANSITION]
                .mul(next_aux[AUX_EXEC_BEFORE + lane].sub(current_aux[AUX_EXEC_AFTER + lane])),
        );
        residues.push(
            fixed[FIX_TRANSITION]
                .mul(next_aux[AUX_SORT_BEFORE + lane].sub(current_aux[AUX_SORT_AFTER + lane])),
        );
        residues.push(
            fixed[FIX_LAST_ACTIVE]
                .mul(current_aux[AUX_EXEC_AFTER + lane].sub(current_aux[AUX_SORT_AFTER + lane])),
        );
    }
    let logical_active_rows =
        F(u64::try_from(logical_active_rows).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
    for (column, expected) in [
        (AUX_CONT_SEGMENT_INDEX, F::ZERO),
        (AUX_CONT_GLOBAL_START, F::ZERO),
        (AUX_CONT_GLOBAL_END, logical_active_rows),
        (AUX_CONT_LOCAL_START, F::ZERO),
        (AUX_CONT_LOCAL_END, F::ZERO),
        (AUX_CONT_MEMORY_START, F::ZERO),
        (AUX_CONT_MEMORY_END, logical_active_rows),
    ] {
        residues.push(current_aux[column].sub(expected));
    }
    for start in [AUX_CONT_EXEC_START, AUX_CONT_SORT_START] {
        for lane in 0..IO_LANES {
            residues.push(current_aux[start + lane].sub(F::ONE));
        }
    }
    for (end, product_after) in [
        (AUX_CONT_EXEC_END, AUX_EXEC_AFTER),
        (AUX_CONT_SORT_END, AUX_SORT_AFTER),
    ] {
        for lane in 0..IO_LANES {
            residues
                .push(fixed[FIX_TRANSITION].mul(next_aux[end + lane].sub(current_aux[end + lane])));
            residues.push(
                fixed[FIX_LAST_ACTIVE]
                    .mul(current_aux[end + lane].sub(current_aux[product_after + lane])),
            );
        }
    }
    if residues.len() != IO_CONSTRAINT_COUNT {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(residues)
}

fn validate_io_bound_constraints_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    base_columns: &[Vec<F>],
    aux_columns: &[Vec<F>],
    fixed_columns: &[Vec<F>],
    challenges: ZkX509IoChallengesV1,
) -> Result<(), ZkX509StarkErrorV1> {
    validate_io_bound_material_shape_v1(
        layout,
        logical_active_rows,
        base_columns,
        aux_columns,
        fixed_columns,
    )?;
    let mut current_base = [F::ZERO; IO_BASE_WIDTH];
    let mut next_base = [F::ZERO; IO_BASE_WIDTH];
    let mut current_aux = [F::ZERO; IO_AUX_WIDTH];
    let mut next_aux = [F::ZERO; IO_AUX_WIDTH];
    let mut fixed = [F::ZERO; IO_FIXED_WIDTH];
    for index in 0..layout.trace_size() {
        let next = (index + 1) % layout.trace_size();
        copy_row_at_v1(base_columns, index, &mut current_base)?;
        copy_row_at_v1(base_columns, next, &mut next_base)?;
        copy_row_at_v1(aux_columns, index, &mut current_aux)?;
        copy_row_at_v1(aux_columns, next, &mut next_aux)?;
        copy_row_at_v1(fixed_columns, index, &mut fixed)?;
        let residues = io_constraint_residues_v1(
            layout,
            logical_active_rows,
            &current_base,
            &next_base,
            &current_aux,
            &next_aux,
            &fixed,
            challenges,
        )?;
        if residues.iter().any(|value| *value != F::ZERO) {
            return Err(ZkX509StarkErrorV1::IoWitness);
        }
    }
    Ok(())
}

fn validate_io_base_constraints_v1(
    material: &IoTraceMaterialV1,
    challenges: ZkX509IoChallengesV1,
) -> Result<(), ZkX509StarkErrorV1> {
    validate_io_bound_constraints_v1(
        material.layout,
        material.logical_active_rows,
        &material.base_columns,
        &material.aux_columns,
        &material.fixed_columns,
        challenges,
    )
}

fn map_transparent_error_v1(error: TransparentStarkErrorV1) -> ZkX509StarkErrorV1 {
    match error {
        TransparentStarkErrorV1::RandomnessUnavailable => ZkX509StarkErrorV1::RandomnessUnavailable,
        TransparentStarkErrorV1::AllocationFailure => ZkX509StarkErrorV1::AllocationFailure,
        TransparentStarkErrorV1::NonCanonicalField => ZkX509StarkErrorV1::NonCanonicalField,
        TransparentStarkErrorV1::FriDegree => ZkX509StarkErrorV1::FriDegree,
        TransparentStarkErrorV1::MalformedProof => ZkX509StarkErrorV1::MalformedProof,
        TransparentStarkErrorV1::InvalidGrinding
        | TransparentStarkErrorV1::ChallengeSamplingExhausted
        | TransparentStarkErrorV1::QuerySamplingExhausted => ZkX509StarkErrorV1::TranscriptMismatch,
        TransparentStarkErrorV1::InvalidMerkleOpening
        | TransparentStarkErrorV1::InvalidMerkleShape => ZkX509StarkErrorV1::TraceOpening,
        _ => ZkX509StarkErrorV1::InternalInvariant,
    }
}

fn masked_lde_columns_v1<R: TryRngCore>(
    columns: &[Vec<F>],
    layout: SegmentLayoutV1,
    rng: &mut R,
) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    columns
        .iter()
        .map(|column| {
            masked_trace_lde_column_v1(column, layout.trace_log2, layout.lde_log2, MASK_DEGREE, rng)
                .map_err(map_transparent_error_v1)
        })
        .collect()
}

fn fixed_lde_columns_v1(
    columns: &[Vec<F>],
    layout: SegmentLayoutV1,
) -> Result<Vec<Vec<F>>, ZkX509StarkErrorV1> {
    let trace_root =
        goldilocks_primitive_root_v1(layout.trace_log2).map_err(map_transparent_error_v1)?;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    columns
        .iter()
        .map(|column| {
            if column.len() != layout.trace_size() {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            let mut coefficients = column.clone();
            goldilocks_ifft_v1(&mut coefficients, trace_root).map_err(map_transparent_error_v1)?;
            goldilocks_evaluate_coset_v1(
                &coefficients,
                layout.lde_size(),
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map_err(map_transparent_error_v1)
        })
        .collect()
}

fn sampled_verifier_generated_fixed_openings_v1<const WIDTH: usize>(
    segment: SegmentLayoutV1,
    common_lde_log2: u8,
    opening_indices: &[usize],
    mut fixed_row: impl FnMut(usize) -> Result<[F; WIDTH], ZkX509StarkErrorV1>,
) -> Result<BTreeMap<usize, Vec<F>>, ZkX509StarkErrorV1> {
    segment.validate()?;
    let common_lde_size = 1_usize
        .checked_shl(u32::from(common_lde_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if WIDTH != segment.fixed_width
        || common_lde_log2 < segment.lde_log2
        || opening_indices.is_empty()
        || opening_indices.len() > VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        || opening_indices
            .iter()
            .any(|index| *index >= common_lde_size)
        || opening_indices.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let trace_root =
        goldilocks_primitive_root_v1(segment.trace_log2).map_err(map_transparent_error_v1)?;
    let common_lde_root =
        goldilocks_primitive_root_v1(common_lde_log2).map_err(map_transparent_error_v1)?;
    let inverse_trace_size =
        F(u64::try_from(segment.trace_size()).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?)
            .inv()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let mut openings = BTreeMap::new();
    for index in opening_indices.iter().copied() {
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_lde_root.pow(index as u128));
        let common = x
            .pow(segment.trace_size() as u128)
            .sub(F::ONE)
            .mul(inverse_trace_size);
        let mut inverse_denominators = Vec::new();
        inverse_denominators
            .try_reserve_exact(segment.trace_size())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        let mut trace_point = F::ONE;
        for _ in 0..segment.trace_size() {
            inverse_denominators.push(x.sub(trace_point));
            trace_point = trace_point.mul(trace_root);
        }
        goldilocks_batch_invert_v1(&mut inverse_denominators).map_err(map_transparent_error_v1)?;
        let mut opened = [F::ZERO; WIDTH];
        trace_point = F::ONE;
        for (row_index, inverse_denominator) in inverse_denominators.iter().copied().enumerate() {
            let weight = common.mul(trace_point).mul(inverse_denominator);
            for (value, fixed) in opened.iter_mut().zip(fixed_row(row_index)?) {
                *value = value.add(fixed.mul(weight));
            }
            trace_point = trace_point.mul(trace_root);
        }
        if openings.insert(index, opened.to_vec()).is_some() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    Ok(openings)
}

fn accumulator_fixed_openings_v1(
    layout: &AggregateProofLayoutV1,
    opening_indices: &[Vec<usize>; ACCUMULATOR_REGISTRATION_COUNT_V1],
) -> Result<[BTreeMap<usize, Vec<F>>; ACCUMULATOR_REGISTRATION_COUNT_V1], ZkX509StarkErrorV1> {
    layout.validate_accumulator_registration_v1()?;
    let segments = canonical_accumulator_segment_layouts_v1()?;
    Ok([sampled_verifier_generated_fixed_openings_v1(
        segments[0],
        layout.common_lde_log2,
        &opening_indices[0],
        |index| compile_ca_accumulator_fixed_row_v1(index).map_err(ZkX509StarkErrorV1::from),
    )?])
}

#[cfg(test)]
static DER_FIXED_OPENING_EVALUATIONS_V1: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

const DER_FIXED_MAX_SAMPLED_OPENINGS_V1: usize = QUERY_COUNT * 2;

fn checked_der_fixed_sampled_work_v1(
    active_rows: usize,
    trace_size: usize,
    opening_count: usize,
) -> Result<usize, ZkX509StarkErrorV1> {
    if active_rows == 0
        || active_rows > trace_size
        || opening_count == 0
        || opening_count > DER_FIXED_MAX_SAMPLED_OPENINGS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let sampled_roots = active_rows
        .checked_add(usize::from(active_rows < trace_size))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let sampled_work = sampled_roots
        .checked_mul(opening_count)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let maximum_work = trace_size
        .checked_add(1)
        .and_then(|roots| roots.checked_mul(DER_FIXED_MAX_SAMPLED_OPENINGS_V1))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if sampled_work > maximum_work {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(sampled_work)
}

fn der_fixed_row_at_point_for_shape_v1(
    document_count: usize,
    parser_rows: usize,
    active_rows: usize,
    trace_log2: u8,
    x: F,
) -> Result<[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1], ZkX509StarkErrorV1> {
    let trace_size = 1_usize
        .checked_shl(u32::from(trace_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if document_count == 0
        || parser_rows == 0
        || parser_rows > active_rows
        || active_rows > trace_size
        || x.pow(trace_size as u128) == F::ONE
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let trace_root = goldilocks_primitive_root_v1(trace_log2).map_err(map_transparent_error_v1)?;
    let mut inverse_denominators = Vec::new();
    inverse_denominators
        .try_reserve_exact(active_rows + usize::from(active_rows < trace_size))
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    let mut trace_point = F::ONE;
    for _ in 0..active_rows {
        inverse_denominators.push(x.sub(trace_point));
        trace_point = trace_point.mul(trace_root);
    }
    let last_trace_point = trace_root
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    if active_rows < trace_size {
        inverse_denominators.push(x.sub(last_trace_point));
    }
    goldilocks_batch_invert_v1(&mut inverse_denominators).map_err(map_transparent_error_v1)?;
    let inverse_trace_size =
        F(u64::try_from(trace_size).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?)
            .inv()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let common = x
        .pow(trace_size as u128)
        .sub(F::ONE)
        .mul(inverse_trace_size);

    let parser_continue_end = parser_rows - 1;
    let mut prefix = F::ZERO;
    let mut parser_continue = F::ZERO;
    let mut parser = F::ZERO;
    let mut active = F::ZERO;
    let mut first = F::ZERO;
    let mut last_parser = F::ZERO;
    let mut first_comparator = F::ZERO;
    let mut last_active = F::ZERO;
    let mut last_aggregate = F::ZERO;
    trace_point = F::ONE;
    for (index, inverse_denominator) in inverse_denominators
        .iter()
        .copied()
        .take(active_rows)
        .enumerate()
    {
        let weight = common.mul(trace_point).mul(inverse_denominator);
        if index == 0 {
            first = weight;
        }
        if index + 1 == parser_rows {
            last_parser = weight;
        }
        if index == parser_rows && parser_rows < active_rows {
            first_comparator = weight;
        }
        if index + 1 == active_rows {
            last_active = weight;
        }
        if index + 1 == trace_size {
            last_aggregate = weight;
        }
        prefix = prefix.add(weight);
        if index + 1 == parser_continue_end {
            parser_continue = prefix;
        }
        if index + 1 == parser_rows {
            parser = prefix;
        }
        if index + 1 == active_rows {
            active = prefix;
        }
        trace_point = trace_point.mul(trace_root);
    }
    if active_rows == trace_size {
        if prefix != F::ONE {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
    } else {
        last_aggregate = common
            .mul(last_trace_point)
            .mul(inverse_denominators[active_rows]);
    }

    let comparator_present = parser_rows < active_rows;
    let mut row = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
    row[DER_FIX_ACTIVE] = active;
    row[FIX_FIRST_ACTIVE] = first;
    row[DER_FIX_LAST_ACTIVE] = last_active;
    row[FIX_PARSER] = parser;
    row[FIX_FIRST_PARSER] = first;
    row[FIX_LAST_PARSER] = last_parser;
    row[FIX_COMPARATOR] = active.sub(parser);
    row[FIX_FIRST_COMPARATOR] = first_comparator;
    row[FIX_LAST_COMPARATOR] = if comparator_present {
        last_active
    } else {
        F::ZERO
    };
    row[FIX_PADDING] = F::ONE.sub(active);
    row[FIX_FIRST_AGGREGATE] = first;
    row[FIX_LAST_AGGREGATE] = last_aggregate;
    row[FIX_FINAL_DOCUMENT] =
        F(u64::try_from(document_count - 1).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?);
    row[FIX_PARSER_CONTINUE] = parser_continue;
    Ok(row)
}

fn der_fixed_row_at_point_v1(
    schedule: &ZkX509DerStarkFixedScheduleV1,
    trace_log2: u8,
    x: F,
) -> Result<[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1], ZkX509StarkErrorV1> {
    der_fixed_row_at_point_for_shape_v1(
        1,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
        schedule.active_rows(),
        trace_log2,
        x,
    )
}

fn der_fixed_openings_v1(
    schedule: &ZkX509DerStarkFixedScheduleV1,
    layout: SegmentLayoutV1,
    opening_indices: &[usize],
) -> Result<BTreeMap<usize, [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1]>, ZkX509StarkErrorV1> {
    if layout.adapter != SegmentAdapterIdV1::StrictDer
        || layout.fixed_width != ZK_X509_DER_STARK_FIXED_WIDTH_V1
        || opening_indices.is_empty()
        || opening_indices.len() > DER_FIXED_MAX_SAMPLED_OPENINGS_V1
        || opening_indices
            .iter()
            .any(|index| *index >= layout.lde_size())
        || opening_indices.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    checked_der_fixed_sampled_work_v1(
        schedule.active_rows(),
        layout.trace_size(),
        opening_indices.len(),
    )?;
    #[cfg(test)]
    DER_FIXED_OPENING_EVALUATIONS_V1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut rows = BTreeMap::new();
    for index in opening_indices.iter().copied() {
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
        if rows
            .insert(
                index,
                der_fixed_row_at_point_v1(schedule, layout.trace_log2, x)?,
            )
            .is_some()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    Ok(rows)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RegisteredRetainedProverPlanV1 {
    quotient_coset_log2: u8,
    quotient_coset_rows: usize,
    quotient_next_stride: usize,
    maximum_quotient_degree: usize,
}

fn registered_retained_prover_plan_v1(
    segment: SegmentLayoutV1,
    common_lde_log2: u8,
) -> Result<RegisteredRetainedProverPlanV1, ZkX509StarkErrorV1> {
    segment.validate()?;
    if common_lde_log2 < segment.lde_log2 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let (maximum_quotient_degree, _) = checked_segment_degree_capacity_v1(
        segment.trace_log2,
        common_lde_log2,
        segment.constraint_degree,
    )?;
    let quotient_coset_rows = maximum_quotient_degree
        .checked_add(1)
        .and_then(|rows| rows.checked_next_power_of_two())
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let quotient_coset_log2 = u8::try_from(quotient_coset_rows.ilog2())
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let quotient_next_stride = quotient_coset_rows
        .checked_div(segment.trace_size())
        .filter(|stride| *stride != 0 && quotient_coset_rows % segment.trace_size() == 0)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if quotient_coset_log2 > common_lde_log2 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(RegisteredRetainedProverPlanV1 {
        quotient_coset_log2,
        quotient_coset_rows,
        quotient_next_stride,
        maximum_quotient_degree,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DerRetainedProverAllocationPlanV1 {
    quotient_coset_log2: u8,
    quotient_coset_rows: usize,
    quotient_next_stride: usize,
    maximum_quotient_degree: usize,
    retained_masked_coefficient_bytes: usize,
    quotient_trace_matrix_bytes: usize,
    encrypted_trace_scratch_bytes: usize,
    common_domain_trace_matrix_bytes: usize,
}

fn der_retained_prover_allocation_plan_v1(
    layout: SegmentLayoutV1,
) -> Result<DerRetainedProverAllocationPlanV1, ZkX509StarkErrorV1> {
    layout.validate()?;
    if layout.adapter != SegmentAdapterIdV1::StrictDer {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let retained = registered_retained_prover_plan_v1(layout, layout.lde_log2)?;
    let masked_coefficient_count = layout
        .trace_size()
        .checked_add(MASK_DEGREE)
        .and_then(|highest_degree| highest_degree.checked_add(1))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let retained_masked_coefficient_bytes = layout
        .base_width
        .checked_add(layout.aux_width)
        .and_then(|width| width.checked_mul(masked_coefficient_count))
        .and_then(|fields| fields.checked_mul(core::mem::size_of::<F>()))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let quotient_trace_matrix_bytes = layout
        .base_width
        .checked_add(layout.aux_width)
        .and_then(|width| width.checked_add(layout.fixed_width))
        .and_then(|width| width.checked_mul(retained.quotient_coset_rows))
        .and_then(|fields| fields.checked_mul(core::mem::size_of::<F>()))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let plan = DerRetainedProverAllocationPlanV1 {
        quotient_coset_log2: retained.quotient_coset_log2,
        quotient_coset_rows: retained.quotient_coset_rows,
        quotient_next_stride: retained.quotient_next_stride,
        maximum_quotient_degree: retained.maximum_quotient_degree,
        retained_masked_coefficient_bytes,
        quotient_trace_matrix_bytes,
        encrypted_trace_scratch_bytes: 0,
        common_domain_trace_matrix_bytes: 0,
    };
    if plan.quotient_coset_log2 != DER_QUOTIENT_COSET_LOG2_V1
        || plan.quotient_coset_rows != DER_QUOTIENT_COSET_SIZE_V1
        || plan.quotient_next_stride != 8
        || plan.maximum_quotient_degree != DER_MAXIMUM_QUOTIENT_DEGREE_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(plan)
}

struct ZeroizingBaseColumnsV1(Vec<Vec<F>>);

impl core::ops::Deref for ZeroizingBaseColumnsV1 {
    type Target = [Vec<F>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for ZeroizingBaseColumnsV1 {
    fn drop(&mut self) {
        for column in &mut self.0 {
            column.fill(F::ZERO);
        }
    }
}

struct ZeroizingExtensionColumnV1(Vec<E>);

impl core::ops::Deref for ZeroizingExtensionColumnV1 {
    type Target = [E];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for ZeroizingExtensionColumnV1 {
    fn drop(&mut self) {
        self.0.fill(E::ZERO);
    }
}

struct RetainedCompositionMaterialV1 {
    evaluations: Vec<Vec<Vec<E>>>,
    coefficient_chunks: Vec<Vec<Vec<E>>>,
}

impl Drop for RetainedCompositionMaterialV1 {
    fn drop(&mut self) {
        for lane in &mut self.evaluations {
            for chunk in lane {
                chunk.fill(E::ZERO);
            }
        }
        for lane in &mut self.coefficient_chunks {
            for chunk in lane {
                chunk.fill(E::ZERO);
            }
        }
    }
}

fn fp4_coset_coefficients_v1(
    evaluations: &[E],
    coset_log2: u8,
) -> Result<ZeroizingExtensionColumnV1, ZkX509StarkErrorV1> {
    let expected = 1_usize
        .checked_shl(u32::from(coset_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if evaluations.len() != expected || evaluations.iter().any(|value| !value.is_canonical()) {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let root = goldilocks_primitive_root_v1(coset_log2).map_err(map_transparent_error_v1)?;
    let inverse_shift = F(GOLDILOCKS_GENERATOR_V1)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let mut coefficients = ZeroizingExtensionColumnV1(Vec::new());
    coefficients
        .0
        .try_reserve_exact(expected)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    coefficients.0.extend_from_slice(evaluations);
    goldilocks_fp4_ifft_v1(&mut coefficients.0, root).map_err(map_transparent_error_v1)?;
    let mut inverse_shift_power = F::ONE;
    for coefficient in &mut coefficients.0 {
        *coefficient = coefficient.mul_base(inverse_shift_power);
        inverse_shift_power = inverse_shift_power.mul(inverse_shift);
    }
    Ok(coefficients)
}

fn der_fixed_columns_on_coset_v1(
    schedule: &ZkX509DerStarkFixedScheduleV1,
    layout: SegmentLayoutV1,
    evaluation_log2: u8,
) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
    if layout.adapter != SegmentAdapterIdV1::StrictDer
        || layout.fixed_width != ZK_X509_DER_STARK_FIXED_WIDTH_V1
        || evaluation_log2 <= layout.trace_log2
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let evaluation_size = 1_usize
        .checked_shl(u32::from(evaluation_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let trace_root =
        goldilocks_primitive_root_v1(layout.trace_log2).map_err(map_transparent_error_v1)?;
    let evaluation_root =
        goldilocks_primitive_root_v1(evaluation_log2).map_err(map_transparent_error_v1)?;
    let mut columns = ZeroizingBaseColumnsV1(Vec::new());
    columns
        .0
        .try_reserve_exact(ZK_X509_DER_STARK_FIXED_WIDTH_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for column in 0..ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
        let mut coefficients = build_zk_x509_der_stark_native_fixed_column_v1(schedule, column)
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
        if coefficients.len() != layout.trace_size() {
            coefficients.fill(F::ZERO);
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        goldilocks_ifft_v1(&mut coefficients, trace_root).map_err(map_transparent_error_v1)?;
        let evaluations = goldilocks_evaluate_coset_v1(
            &coefficients,
            evaluation_size,
            evaluation_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .map_err(map_transparent_error_v1)?;
        coefficients.fill(F::ZERO);
        columns.0.push(evaluations);
    }
    Ok(columns)
}

fn composition_coefficient_chunks_v1(
    quotient_coefficients: &[E],
    maximum_quotient_degree: usize,
    shared_layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<Vec<E>>, ZkX509StarkErrorV1> {
    let first_forbidden = maximum_quotient_degree
        .checked_add(1)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if quotient_coefficients
        .get(first_forbidden..)
        .is_some_and(|tail| tail.iter().any(|coefficient| *coefficient != E::ZERO))
    {
        return Err(ZkX509StarkErrorV1::ConstraintOpening);
    }
    let chunk_size = shared_layout
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .map_err(map_aggregate_error_v1)?;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(COMPOSITION_DEGREE_CHUNKS)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
        let start = chunk
            .checked_mul(chunk_size)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if start >= quotient_coefficients.len() {
            chunks.push(Vec::new());
            continue;
        }
        let end = start
            .checked_add(chunk_size)
            .map(|end| end.min(quotient_coefficients.len()))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let source = quotient_coefficients
            .get(start..end)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let retained_length = source
            .iter()
            .rposition(|coefficient| *coefficient != E::ZERO)
            .map_or(0, |degree| degree + 1);
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(retained_length)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        coefficients.extend_from_slice(
            source
                .get(..retained_length)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        );
        chunks.push(coefficients);
    }
    Ok(chunks)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn der_composition_material_from_polynomials_v1(
    layout: SegmentLayoutV1,
    schedule: &ZkX509DerStarkFixedScheduleV1,
    base_polynomials: &aggregate::MaskedTracePolynomialSetV1,
    aux_polynomials: &aggregate::MaskedTracePolynomialSetV1,
    challenges: ZkX509DerStarkChallengesV1,
    public: ZkX509DerStarkPublicTerminalsV1,
    claims: ZkX509DerStarkTerminalClaimsV1,
    alphas: &[Vec<E>],
) -> Result<RetainedCompositionMaterialV1, ZkX509StarkErrorV1> {
    let plan = der_retained_prover_allocation_plan_v1(layout)?;
    if base_polynomials.width() != ZK_X509_DER_STARK_BASE_WIDTH_V1
        || aux_polynomials.width() != ZK_X509_DER_STARK_AUX_WIDTH_V1
        || base_polynomials.native_trace_log2() != layout.trace_log2
        || aux_polynomials.native_trace_log2() != layout.trace_log2
        || base_polynomials.commitment_lde_log2() != layout.lde_log2
        || aux_polynomials.commitment_lde_log2() != layout.lde_log2
        || alphas.len() != SECURITY_LANES
        || alphas
            .iter()
            .any(|lane| lane.len() != ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let base_coset = base_polynomials
        .evaluate_columns_on_coset_v1(plan.quotient_coset_log2)
        .map_err(map_aggregate_error_v1)?;
    let aux_coset = aux_polynomials
        .evaluate_columns_on_coset_v1(plan.quotient_coset_log2)
        .map_err(map_aggregate_error_v1)?;
    let fixed_coset = der_fixed_columns_on_coset_v1(schedule, layout, plan.quotient_coset_log2)?;
    if base_coset
        .iter()
        .chain(&aux_coset)
        .any(|column| column.len() != plan.quotient_coset_rows)
        || fixed_coset
            .iter()
            .any(|column| column.len() != plan.quotient_coset_rows)
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let mut numerators = (0..SECURITY_LANES)
        .map(|_| {
            let mut numerator = ZeroizingExtensionColumnV1(Vec::new());
            numerator
                .0
                .try_reserve_exact(plan.quotient_coset_rows)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            Ok::<_, ZkX509StarkErrorV1>(numerator)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut residues = Vec::new();
    residues
        .try_reserve_exact(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for index in 0..plan.quotient_coset_rows {
        let next = (index + plan.quotient_next_stride) % plan.quotient_coset_rows;
        let current_base: [F; ZK_X509_DER_STARK_BASE_WIDTH_V1] =
            core::array::from_fn(|column| base_coset[column][index]);
        let next_base: [F; ZK_X509_DER_STARK_BASE_WIDTH_V1] =
            core::array::from_fn(|column| base_coset[column][next]);
        let current_aux: [F; ZK_X509_DER_STARK_AUX_WIDTH_V1] =
            core::array::from_fn(|column| aux_coset[column][index]);
        let next_aux: [F; ZK_X509_DER_STARK_AUX_WIDTH_V1] =
            core::array::from_fn(|column| aux_coset[column][next]);
        let current_fixed: [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1] =
            core::array::from_fn(|column| fixed_coset[column][index]);
        let next_fixed: [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1] =
            core::array::from_fn(|column| fixed_coset[column][next]);
        evaluate_zk_x509_der_stark_residues_into_v1(
            &current_base,
            &next_base,
            &current_aux,
            &next_aux,
            &current_fixed,
            &next_fixed,
            challenges,
            public,
            claims,
            &mut residues,
        )
        .map_err(|_| ZkX509StarkErrorV1::ConstraintOpening)?;
        if residues.len() != ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1 {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        for lane in 0..SECURITY_LANES {
            numerators[lane].0.push(
                residues
                    .iter()
                    .zip(&alphas[lane])
                    .fold(E::ZERO, |sum, (residue, alpha)| {
                        sum.add(alpha.mul_base(*residue))
                    }),
            );
        }
    }
    drop(base_coset);
    drop(aux_coset);
    drop(fixed_coset);

    let shared_layout = AggregateProofLayoutV1::for_segments(&[layout])?.as_shared()?;
    let mut evaluations = Vec::new();
    let mut coefficient_chunks = Vec::new();
    evaluations
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    coefficient_chunks
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for numerator in &numerators {
        let quotient = ZeroizingExtensionColumnV1(
            aggregate::quotient_evaluations_from_constraint_coset_v1(
                numerator,
                layout.trace_log2,
                plan.quotient_coset_log2,
            )
            .map_err(map_aggregate_error_v1)?,
        );
        let coefficients = fp4_coset_coefficients_v1(&quotient, plan.quotient_coset_log2)?;
        coefficient_chunks.push(composition_coefficient_chunks_v1(
            &coefficients,
            plan.maximum_quotient_degree,
            &shared_layout,
        )?);
        evaluations.push(
            aggregate::composition_chunks_from_quotient_coset_v1(
                &quotient,
                plan.quotient_coset_log2,
                plan.maximum_quotient_degree,
                AGGREGATE_PARAMETERS_V1,
                &shared_layout,
            )
            .map_err(map_aggregate_error_v1)?,
        );
    }
    Ok(RetainedCompositionMaterialV1 {
        evaluations,
        coefficient_chunks,
    })
}

fn accumulate_base_deep_quotient_v1(
    coefficients: &[F],
    point: E,
    expected_value: E,
    scale: E,
    accumulator: &mut [E],
) -> Result<(), ZkX509StarkErrorV1> {
    let Some((&constant, higher)) = coefficients.split_first() else {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    };
    if higher.len() > accumulator.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let Some((&leading, middle)) = higher.split_last() else {
        if E::from_base(constant) != expected_value {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        return Ok(());
    };
    let mut quotient_coefficient = E::from_base(leading);
    accumulator[middle.len()] = accumulator[middle.len()].add(quotient_coefficient.mul(scale));
    for (degree, coefficient) in middle.iter().copied().enumerate().rev() {
        quotient_coefficient = E::from_base(coefficient).add(point.mul(quotient_coefficient));
        accumulator[degree] = accumulator[degree].add(quotient_coefficient.mul(scale));
    }
    if E::from_base(constant).add(point.mul(quotient_coefficient)) != expected_value {
        return Err(ZkX509StarkErrorV1::ConstraintOpening);
    }
    Ok(())
}

fn accumulate_extension_deep_quotient_v1(
    coefficients: &[E],
    point: E,
    expected_value: E,
    scale: E,
    accumulator: &mut [E],
) -> Result<(), ZkX509StarkErrorV1> {
    let Some((&constant, higher)) = coefficients.split_first() else {
        if expected_value != E::ZERO {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        return Ok(());
    };
    if higher.len() > accumulator.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let Some((&leading, middle)) = higher.split_last() else {
        if constant != expected_value {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        return Ok(());
    };
    let mut quotient_coefficient = leading;
    accumulator[middle.len()] = accumulator[middle.len()].add(quotient_coefficient.mul(scale));
    for (degree, coefficient) in middle.iter().copied().enumerate().rev() {
        quotient_coefficient = coefficient.add(point.mul(quotient_coefficient));
        accumulator[degree] = accumulator[degree].add(quotient_coefficient.mul(scale));
    }
    if constant.add(point.mul(quotient_coefficient)) != expected_value {
        return Err(ZkX509StarkErrorV1::ConstraintOpening);
    }
    Ok(())
}

fn evaluate_retained_composition_coefficients_at_deep_v1(
    coefficient_chunks: &[Vec<Vec<E>>],
    point: E,
) -> Result<Vec<Vec<E>>, ZkX509StarkErrorV1> {
    if !point.is_canonical()
        || coefficient_chunks.len() != SECURITY_LANES
        || coefficient_chunks
            .iter()
            .any(|lane| lane.len() != COMPOSITION_DEGREE_CHUNKS)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let coefficient_cap = 1_usize
        .checked_shl(u32::from(
            ZK_X509_MAIN_COMMON_LDE_LOG2_V1
                .checked_sub(TERMINAL_LOG2)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
        ))
        .and_then(|fold_factor| (TERMINAL_DEGREE_BOUND + 1).checked_mul(fold_factor))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if coefficient_chunks.iter().flatten().any(|coefficients| {
        coefficients.len() > coefficient_cap
            || coefficients
                .iter()
                .any(|coefficient| !coefficient.is_canonical())
    }) {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(coefficient_chunks
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|coefficients| {
                    coefficients
                        .iter()
                        .rev()
                        .copied()
                        .fold(E::ZERO, |value, coefficient| {
                            value.mul(point).add(coefficient)
                        })
                })
                .collect()
        })
        .collect())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn der_fri_bases_from_polynomials_v1(
    layout: SegmentLayoutV1,
    base_polynomials: &aggregate::MaskedTracePolynomialSetV1,
    aux_polynomials: &aggregate::MaskedTracePolynomialSetV1,
    composition_coefficients: &[Vec<Vec<E>>],
    mixes: &[FriMixV1],
    deep_point: E,
    deep_trace: &aggregate::AggregateOpenedDeepTraceGroupV1,
    deep_compositions: &[Vec<E>],
) -> Result<Vec<Vec<E>>, ZkX509StarkErrorV1> {
    let _plan = der_retained_prover_allocation_plan_v1(layout)?;
    if base_polynomials.width() != ZK_X509_DER_STARK_BASE_WIDTH_V1
        || aux_polynomials.width() != ZK_X509_DER_STARK_AUX_WIDTH_V1
        || base_polynomials.native_trace_log2() != layout.trace_log2
        || aux_polynomials.native_trace_log2() != layout.trace_log2
        || base_polynomials.commitment_lde_log2() != layout.lde_log2
        || aux_polynomials.commitment_lde_log2() != layout.lde_log2
        || composition_coefficients.len() != SECURITY_LANES
        || composition_coefficients
            .iter()
            .any(|lane| lane.len() != COMPOSITION_DEGREE_CHUNKS)
        || mixes.len() != SECURITY_LANES
        || mixes.iter().any(|mix| {
            mix.base.len() != ZK_X509_DER_STARK_BASE_WIDTH_V1
                || mix.base_next.len() != ZK_X509_DER_STARK_BASE_WIDTH_V1
                || mix.aux.len() != ZK_X509_DER_STARK_AUX_WIDTH_V1
                || mix.aux_next.len() != ZK_X509_DER_STARK_AUX_WIDTH_V1
                || mix.composition.len() != COMPOSITION_DEGREE_CHUNKS
        })
        || deep_trace.base_current.len() != ZK_X509_DER_STARK_BASE_WIDTH_V1
        || deep_trace.base_next.len() != ZK_X509_DER_STARK_BASE_WIDTH_V1
        || deep_trace.aux_current.len() != ZK_X509_DER_STARK_AUX_WIDTH_V1
        || deep_trace.aux_next.len() != ZK_X509_DER_STARK_AUX_WIDTH_V1
        || deep_compositions.len() != SECURITY_LANES
        || deep_compositions
            .iter()
            .any(|values| values.len() != COMPOSITION_DEGREE_CHUNKS)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let shared_layout = AggregateProofLayoutV1::for_segments(&[layout])?.as_shared()?;
    let coefficient_cap = shared_layout
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .map_err(map_aggregate_error_v1)?;
    let native_root =
        goldilocks_primitive_root_v1(layout.trace_log2).map_err(map_transparent_error_v1)?;
    let deep_next_point = deep_point.mul_base(native_root);
    let mut accumulators = (0..SECURITY_LANES)
        .map(|_| {
            let mut accumulator = ZeroizingExtensionColumnV1(Vec::new());
            accumulator
                .0
                .try_reserve_exact(coefficient_cap)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            accumulator.0.resize(coefficient_cap, E::ZERO);
            Ok::<_, ZkX509StarkErrorV1>(accumulator)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for column in 0..base_polynomials.width() {
        let coefficients = base_polynomials
            .column_coefficients_v1(column)
            .map_err(map_aggregate_error_v1)?;
        for lane in 0..SECURITY_LANES {
            accumulate_base_deep_quotient_v1(
                coefficients,
                deep_point,
                deep_trace.base_current[column],
                mixes[lane].base[column],
                &mut accumulators[lane].0,
            )?;
            accumulate_base_deep_quotient_v1(
                coefficients,
                deep_next_point,
                deep_trace.base_next[column],
                mixes[lane].base_next[column],
                &mut accumulators[lane].0,
            )?;
        }
    }
    for column in 0..aux_polynomials.width() {
        let coefficients = aux_polynomials
            .column_coefficients_v1(column)
            .map_err(map_aggregate_error_v1)?;
        for lane in 0..SECURITY_LANES {
            accumulate_base_deep_quotient_v1(
                coefficients,
                deep_point,
                deep_trace.aux_current[column],
                mixes[lane].aux[column],
                &mut accumulators[lane].0,
            )?;
            accumulate_base_deep_quotient_v1(
                coefficients,
                deep_next_point,
                deep_trace.aux_next[column],
                mixes[lane].aux_next[column],
                &mut accumulators[lane].0,
            )?;
        }
    }
    for lane in 0..SECURITY_LANES {
        for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
            accumulate_extension_deep_quotient_v1(
                &composition_coefficients[lane][chunk],
                deep_point,
                deep_compositions[lane][chunk],
                mixes[lane].composition[chunk],
                &mut accumulators[lane].0,
            )?;
        }
    }
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    accumulators
        .iter()
        .map(|coefficients| {
            goldilocks_fp4_evaluate_coset_v1(
                coefficients,
                layout.lde_size(),
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map_err(map_transparent_error_v1)
        })
        .collect()
}

fn row_tree_v1(
    domain: &[u8],
    node_domain: &'static [u8],
    segment: usize,
    columns: &[Vec<F>],
    rows: usize,
) -> Result<Sha256MerkleTreeV1, ZkX509StarkErrorV1> {
    aggregate::row_tree_v1(domain, node_domain, segment, columns, rows)
        .map_err(map_aggregate_error_v1)
}

fn composition_tree_v1(
    lane: usize,
    chunks: &[Vec<E>],
) -> Result<Sha256MerkleTreeV1, ZkX509StarkErrorV1> {
    aggregate::composition_tree_v1(AGGREGATE_DOMAINS_V1, lane, chunks)
        .map_err(map_aggregate_error_v1)
}

fn fri_tree_v1(
    lane: usize,
    round: usize,
    values: &[E],
) -> Result<Sha256MerkleTreeV1, ZkX509StarkErrorV1> {
    aggregate::fri_tree_v1(AGGREGATE_DOMAINS_V1, lane, round, values)
        .map_err(map_aggregate_error_v1)
}

fn new_transcript_v1(
    public_digest: &[u8; 32],
) -> Result<TransparentTranscriptV1, ZkX509StarkErrorV1> {
    let compiled_profile_digest = recompute_zk_x509_compiled_profile_digest_v1()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut transcript =
        TransparentTranscriptV1::new(ZK_X509_SUITE_V1, &compiled_profile_digest, public_digest)
            .map_err(map_transparent_error_v1)?;
    transcript
        .absorb(
            b"zk-x509-segmented-stark-profile-v1",
            &[ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1],
        )
        .map_err(map_transparent_error_v1)?;
    Ok(transcript)
}

/// Construct the release-only MAIN transcript from the sole compiled profile.
fn new_main_transcript_v1(
    public_digest: &[u8; 32],
    verifier_profile: ZkX509MainVerifierProfileV1,
) -> Result<TransparentTranscriptV1, ZkX509StarkErrorV1> {
    validate_zk_x509_main_verifier_profile_v1(verifier_profile)?;
    new_main_transcript_after_profile_validation_v1(
        public_digest,
        verifier_profile.compiled_profile_digest,
    )
}

/// Initialize MAIN after the independent release-profile validation step.
///
/// This split mirrors the MAIN base-commitment session: production reaches it
/// only through `new_main_transcript_v1`, while tests can exercise transcript
/// separation before the sole compiled-manifest pin is installed.
fn new_main_transcript_after_profile_validation_v1(
    public_digest: &[u8; 32],
    release_profile_digest: [u8; 32],
) -> Result<TransparentTranscriptV1, ZkX509StarkErrorV1> {
    if release_profile_digest == [0_u8; 32] {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut transcript =
        TransparentTranscriptV1::new(ZK_X509_SUITE_V1, &release_profile_digest, public_digest)
            .map_err(map_transparent_error_v1)?;
    transcript
        .absorb(
            b"zk-x509-segmented-stark-profile-v1",
            &[ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1],
        )
        .map_err(map_transparent_error_v1)?;
    transcript
        .absorb(
            b"zk-x509-main-release-profile-v1",
            &[&release_profile_digest],
        )
        .map_err(map_transparent_error_v1)?;
    Ok(transcript)
}

fn absorb_aggregate_layout_v1(
    transcript: &mut TransparentTranscriptV1,
    layout_domain: &[u8],
    layout: &AggregateProofLayoutV1,
) -> Result<(), ZkX509StarkErrorV1> {
    layout.validate()?;
    let registration_count = u16::try_from(layout.registered_segments.len())
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut registration = Vec::new();
    registration
        .try_reserve_exact(
            6_usize
                .checked_add(
                    layout
                        .registered_segments
                        .len()
                        .checked_mul(33)
                        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
                )
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
        )
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    registration.extend_from_slice(b"X5R1");
    append_u16_v1(&mut registration, registration_count);
    for registered in &layout.registered_segments {
        append_u16_v1(&mut registration, registered.segment.adapter.wire());
        append_u16_v1(&mut registration, registered.segment.instance);
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.trace_group)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u32_v1(
            &mut registration,
            u32::try_from(registered.segment.active_rows)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        registration.push(registered.segment.trace_log2);
        registration.push(registered.segment.lde_log2);
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.base_start)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.segment.base_width)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.aux_start).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.segment.aux_width)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.segment.fixed_width)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.segment.constraint_count)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
        registration.push(registered.segment.constraint_degree);
        append_u16_v1(
            &mut registration,
            u16::try_from(registered.column_chunks)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
        );
    }
    transcript
        .absorb(
            b"zk-x509-registered-segment-layout-v1",
            &[layout_domain, &registration],
        )
        .map_err(map_transparent_error_v1)?;
    aggregate::absorb_layout_v1(
        transcript,
        layout.parameters_v1(),
        AGGREGATE_DOMAINS_V1,
        layout_domain,
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)
}

fn append_accumulator_public_digest_v1(
    encoding: &mut Vec<u8>,
    digest: &[F; 32],
) -> Result<(), ZkX509StarkErrorV1> {
    for value in digest {
        if F::canonical(value.0).is_none() || value.0 > u64::from(u8::MAX) {
            return Err(ZkX509StarkErrorV1::InvalidStatement);
        }
        append_u64_v1(encoding, value.0);
    }
    Ok(())
}

/// Bind the sole compact-CA registration before trace roots.
///
/// The schedule digest covers every verifier-fixed SHA call manifest but no
/// private preimage or digest byte. Those bytes remain committed in the
/// compact-CA and SHA traces and are joined by four-lane call-bus products.
fn absorb_accumulator_registration_v1(
    transcript: &mut TransparentTranscriptV1,
    registration: &AccumulatorTraceRegistrationV1,
) -> Result<(), ZkX509StarkErrorV1> {
    registration.validate()?;
    let schedule_digest = accumulator_schedule_digest_v1(&registration.schedule)?;
    let mut encoding = Vec::new();
    encoding
        .try_reserve_exact(4 + 2 + 2 + 2 + 32 * 8 + 32)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    encoding.extend_from_slice(b"X5A1");
    append_u16_v1(
        &mut encoding,
        u16::try_from(ACCUMULATOR_REGISTRATION_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
    );
    append_u16_v1(&mut encoding, SegmentAdapterIdV1::CaAccumulator.wire());
    append_u16_v1(&mut encoding, 0);
    append_accumulator_public_digest_v1(
        &mut encoding,
        &registration.public.ca_membership.governed_root,
    )?;
    encoding.extend_from_slice(&schedule_digest);

    absorb_aggregate_layout_v1(
        transcript,
        ACCUMULATOR_LAYOUT_DOMAIN_V1,
        &registration.layout,
    )?;
    transcript
        .absorb(
            b"zk-x509-accumulator-stark-profile-v1",
            &[ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1],
        )
        .map_err(map_transparent_error_v1)?;
    transcript
        .absorb(ACCUMULATOR_REGISTRATION_DOMAIN_V1, &[&encoding])
        .map_err(map_transparent_error_v1)
}

/// Bind one exact role-specific P-256 registration before base commitments.
fn absorb_p256_registration_v1(
    transcript: &mut TransparentTranscriptV1,
    registration: &P256TraceRegistrationV1,
) -> Result<(), ZkX509StarkErrorV1> {
    registration.validate()?;
    absorb_aggregate_layout_v1(transcript, P256_LAYOUT_DOMAIN_V1, &registration.layout)?;
    let role = [match registration.role {
        P256EcdsaRoleV1::CertificateOrCrl => 1,
        P256EcdsaRoleV1::WalletOwnership => 2,
    }];
    let segment_count = u16::try_from(registration.layout.registered_segments.len())
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?
        .to_be_bytes();
    transcript
        .absorb(
            b"zk-x509-p256-aggregate-adapter-profile-v1",
            &[
                ZK_X509_P256_AGGREGATE_ADAPTER_DESCRIPTOR_V1,
                &ZK_X509_P256_AGGREGATE_ADAPTER_DESCRIPTOR_SHA256_V1,
            ],
        )
        .map_err(map_transparent_error_v1)?;
    transcript
        .absorb(
            P256_REGISTRATION_DOMAIN_V1,
            &[b"X5E1", &role, &segment_count],
        )
        .map_err(map_transparent_error_v1)
}

/// Bind all proof-carried P-256 terminals after auxiliary roots.
fn absorb_p256_terminal_registration_v1(
    transcript: &mut TransparentTranscriptV1,
    role: P256EcdsaRoleV1,
    terminals: &P256TerminalRegistrationV1,
) -> Result<(), ZkX509StarkErrorV1> {
    terminals.validate(role)?;
    absorb_p256_terminal_claims_v1(
        transcript,
        role,
        terminals.buses,
        &terminals.cross_sources,
        terminals.sink,
    )
    .map_err(ZkX509StarkErrorV1::from)
}

/// Absorb DER terminal claims in the sole legal cross-adapter role order:
/// input bytes first, parsed-node events second, and lane order within role.
///
/// Provers and verifiers call this only after all auxiliary roots and before
/// deriving constraint alphas, FRI mixes, or queries.
#[allow(dead_code)]
fn absorb_der_terminal_claims_v1(
    transcript: &mut TransparentTranscriptV1,
    claims: ZkX509DerStarkTerminalClaimsV1,
) -> Result<(), ZkX509StarkErrorV1> {
    if claims
        .input_byte
        .iter()
        .chain(&claims.node)
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::InvalidStatement);
    }
    let mut encoding = Vec::new();
    encoding
        .try_reserve_exact(4 + 2 + 2 + 2 + 2 * (2 + 2 + 3 * 8))
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    encoding.extend_from_slice(b"X5C1");
    append_u16_v1(&mut encoding, SegmentAdapterIdV1::StrictDer.wire());
    append_u16_v1(&mut encoding, 0);
    append_u16_v1(&mut encoding, 2);
    for (role, lanes) in [(1_u16, claims.input_byte), (2_u16, claims.node)] {
        append_u16_v1(&mut encoding, role);
        append_u16_v1(
            &mut encoding,
            u16::try_from(lanes.len()).expect("DER lane count fits u16"),
        );
        for value in lanes {
            append_u64_v1(&mut encoding, value.0);
        }
    }
    transcript
        .absorb(DER_TERMINAL_CLAIMS_DOMAIN, &[&encoding])
        .map_err(map_transparent_error_v1)
}

fn encode_der_segmented_proof_envelope_v1(
    claims: ZkX509DerStarkTerminalClaimsV1,
    aggregate_proof: &[u8],
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    if aggregate_proof.is_empty()
        || aggregate_proof.len() > u32::MAX as usize
        || claims
            .input_byte
            .iter()
            .chain(&claims.node)
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let encoded_len = DER_PROOF_ENVELOPE_BYTES_V1
        .checked_add(aggregate_proof.len())
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
    if encoded_len > ZK_X509_MAX_PROOF_BYTES_V1 as usize {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(encoded_len)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    encoded.extend_from_slice(&DER_PROOF_MAGIC_V1);
    append_u16_v1(&mut encoded, ZK_X509_PROOF_VERSION_V1);
    append_u16_v1(&mut encoded, SegmentAdapterIdV1::StrictDer.wire());
    append_u16_v1(&mut encoded, 0);
    append_u16_v1(
        &mut encoded,
        u16::try_from(DER_PROOF_CLAIM_COUNT_V1).expect("DER claim count fits u16"),
    );
    for (claim_type, values) in [(1_u16, claims.input_byte), (2_u16, claims.node)] {
        for (lane, value) in values.into_iter().enumerate() {
            append_u16_v1(&mut encoded, claim_type);
            append_u16_v1(
                &mut encoded,
                u16::try_from(lane).expect("DER lane fits u16"),
            );
            append_u64_v1(&mut encoded, value.0);
        }
    }
    append_u32_v1(
        &mut encoded,
        u32::try_from(aggregate_proof.len()).map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?,
    );
    encoded.extend_from_slice(aggregate_proof);
    if encoded.len() != encoded_len {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(encoded)
}

fn decode_der_segmented_proof_envelope_v1(
    encoded: &[u8],
) -> Result<(ZkX509DerStarkTerminalClaimsV1, &[u8]), ZkX509StarkErrorV1> {
    if encoded.len() > ZK_X509_MAX_PROOF_BYTES_V1 as usize {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    if encoded.len() < DER_PROOF_ENVELOPE_BYTES_V1
        || encoded[..4] != DER_PROOF_MAGIC_V1
        || u16::from_be_bytes(
            encoded[4..6]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        ) != ZK_X509_PROOF_VERSION_V1
        || u16::from_be_bytes(
            encoded[6..8]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        ) != SegmentAdapterIdV1::StrictDer.wire()
        || u16::from_be_bytes(
            encoded[8..10]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        ) != 0
        || usize::from(u16::from_be_bytes(
            encoded[10..12]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        )) != DER_PROOF_CLAIM_COUNT_V1
    {
        return Err(ZkX509StarkErrorV1::MalformedProof);
    }
    let mut fields = [F::ZERO; DER_PROOF_CLAIM_COUNT_V1];
    for (claim_index, slot) in fields.iter_mut().enumerate() {
        let start = 12_usize
            .checked_add(
                claim_index
                    .checked_mul(DER_PROOF_CLAIM_RECORD_BYTES_V1)
                    .ok_or(ZkX509StarkErrorV1::MalformedProof)?,
            )
            .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
        let expected_type = if claim_index < ZK_X509_DER_STARK_BUS_LANES_V1 {
            1
        } else {
            2
        };
        let expected_lane = u16::try_from(claim_index % ZK_X509_DER_STARK_BUS_LANES_V1)
            .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
        let actual_type = u16::from_be_bytes(
            encoded[start..start + 2]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        );
        let actual_lane = u16::from_be_bytes(
            encoded[start + 2..start + 4]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        );
        if actual_type != expected_type || actual_lane != expected_lane {
            return Err(ZkX509StarkErrorV1::MalformedProof);
        }
        let raw = u64::from_be_bytes(
            encoded[start + 4..start + DER_PROOF_CLAIM_RECORD_BYTES_V1]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
        );
        *slot = F::canonical(raw).ok_or(ZkX509StarkErrorV1::NonCanonicalField)?;
    }
    let aggregate_len = usize::try_from(u32::from_be_bytes(
        encoded[DER_PROOF_LENGTH_OFFSET_V1..DER_PROOF_ENVELOPE_BYTES_V1]
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?,
    ))
    .map_err(|_| ZkX509StarkErrorV1::MalformedProof)?;
    let expected_len = DER_PROOF_ENVELOPE_BYTES_V1
        .checked_add(aggregate_len)
        .ok_or(ZkX509StarkErrorV1::MalformedProof)?;
    if aggregate_len == 0 || encoded.len() != expected_len {
        return Err(ZkX509StarkErrorV1::MalformedProof);
    }
    let claims = ZkX509DerStarkTerminalClaimsV1 {
        input_byte: fields[..ZK_X509_DER_STARK_BUS_LANES_V1]
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
        node: fields[ZK_X509_DER_STARK_BUS_LANES_V1..]
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
    };
    Ok((claims, &encoded[DER_PROOF_ENVELOPE_BYTES_V1..]))
}

#[allow(dead_code)]
fn evaluate_der_terminal_claim_opening_v1(
    last_aggregate: F,
    aux: &[F],
    claims: ZkX509DerStarkTerminalClaimsV1,
) -> Result<[F; 2 * ZK_X509_DER_STARK_BUS_LANES_V1], ZkX509StarkErrorV1> {
    let aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1] = aux
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    Ok(evaluate_zk_x509_der_stark_terminal_claim_residues_v1(
        last_aggregate,
        aux,
        claims,
    ))
}

fn challenge_vector_v1(
    transcript: &mut TransparentTranscriptV1,
    label: &[u8],
    count: usize,
) -> Result<Vec<E>, ZkX509StarkErrorV1> {
    (0..count)
        .map(|_| {
            transcript
                .challenge_fp4(label)
                .map_err(map_transparent_error_v1)
        })
        .collect()
}

#[derive(Clone)]
struct FriMixV1 {
    base: Vec<E>,
    base_next: Vec<E>,
    aux: Vec<E>,
    aux_next: Vec<E>,
    composition: Vec<E>,
}

fn derive_constraint_alphas_v1(
    transcript: &mut TransparentTranscriptV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<Vec<E>>>, ZkX509StarkErrorV1> {
    layout
        .registered_segments
        .iter()
        .map(|registration| {
            (0..SECURITY_LANES)
                .map(|_| {
                    challenge_vector_v1(
                        transcript,
                        b"zk-x509-constraint-alpha-v1",
                        registration.segment.constraint_count,
                    )
                })
                .collect()
        })
        .collect()
}

fn derive_fri_mixes_v1(
    transcript: &mut TransparentTranscriptV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<FriMixV1>>, ZkX509StarkErrorV1> {
    let composition_chunks = layout.parameters_v1().composition_degree_chunks;
    let composition = (0..SECURITY_LANES)
        .map(|_| {
            challenge_vector_v1(
                transcript,
                b"zk-x509-fri-composition-mix-v1",
                composition_chunks,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    layout
        .trace_groups
        .iter()
        .map(|group| {
            (0..SECURITY_LANES)
                .map(|lane| {
                    Ok(FriMixV1 {
                        base: challenge_vector_v1(
                            transcript,
                            b"zk-x509-deep-base-current-mix-v1",
                            group.base_width,
                        )?,
                        base_next: challenge_vector_v1(
                            transcript,
                            b"zk-x509-deep-base-next-mix-v1",
                            group.base_width,
                        )?,
                        aux: challenge_vector_v1(
                            transcript,
                            b"zk-x509-deep-aux-current-mix-v1",
                            group.aux_width,
                        )?,
                        aux_next: challenge_vector_v1(
                            transcript,
                            b"zk-x509-deep-aux-next-mix-v1",
                            group.aux_width,
                        )?,
                        composition: composition[lane].clone(),
                    })
                })
                .collect()
        })
        .collect()
}

fn aggregate_deep_lane_mixes_v1(
    mixes: &[Vec<FriMixV1>],
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<aggregate::AggregateDeepLaneMixV1>, ZkX509StarkErrorV1> {
    if mixes.len() != layout.trace_groups.len()
        || mixes.iter().any(|lanes| lanes.len() != SECURITY_LANES)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut lane_mixes = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let trace_groups = mixes
            .iter()
            .map(|lanes| {
                let mix = lanes.get(lane).ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
                Ok(aggregate::AggregateDeepTraceGroupMixV1 {
                    base_current: mix.base.clone(),
                    base_next: mix.base_next.clone(),
                    aux_current: mix.aux.clone(),
                    aux_next: mix.aux_next.clone(),
                })
            })
            .collect::<Result<Vec<_>, ZkX509StarkErrorV1>>()?;
        let composition = mixes
            .first()
            .and_then(|lanes| lanes.get(lane))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .composition
            .clone();
        if mixes
            .iter()
            .any(|lanes| lanes.get(lane).map(|mix| &mix.composition) != Some(&composition))
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        lane_mixes.push(aggregate::AggregateDeepLaneMixV1 {
            trace_groups,
            composition,
        });
    }
    aggregate::validate_deep_lane_mixes_v1(
        &lane_mixes,
        layout.parameters_v1(),
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)?;
    Ok(lane_mixes)
}

fn canonical_deep_values_v1(
    deep: &aggregate::AggregateDeepProofV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(Vec<aggregate::AggregateOpenedDeepTraceGroupV1>, Vec<Vec<E>>), ZkX509StarkErrorV1> {
    let shared_layout = layout.as_shared()?;
    let parameters = layout.parameters_v1();
    let trace_groups = aggregate::canonical_deep_trace_groups_v1(deep, parameters, &shared_layout)
        .map_err(map_aggregate_error_v1)?;
    let composition_values = deep
        .composition_values
        .iter()
        .map(|values| {
            aggregate::canonical_fp4_fields_v1(values, parameters.composition_degree_chunks)
                .map_err(map_aggregate_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((trace_groups, composition_values))
}

fn fp4_values_to_wire_v1(values: Vec<E>) -> Vec<[u64; 4]> {
    values
        .into_iter()
        .map(|value| value.coefficients().map(F::value))
        .collect()
}

fn composition_lanes_v1(
    material: &IoTraceMaterialV1,
    base_lde: &[Vec<F>],
    aux_lde: &[Vec<F>],
    fixed_lde: &[Vec<F>],
    challenges: ZkX509IoChallengesV1,
    alphas: &[Vec<E>],
) -> Result<Vec<Vec<Vec<E>>>, ZkX509StarkErrorV1> {
    if alphas.len() != SECURITY_LANES
        || alphas
            .iter()
            .any(|lane| lane.len() != material.layout.constraint_count)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let layout = material.layout;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    let mut lanes = (0..SECURITY_LANES)
        .map(|_| Vec::with_capacity(layout.lde_size()))
        .collect::<Vec<_>>();
    for index in 0..layout.lde_size() {
        let next = (index + BLOWUP) % layout.lde_size();
        let residues = io_constraint_residues_v1(
            layout,
            material.logical_active_rows,
            &row_at_v1(base_lde, index)?,
            &row_at_v1(base_lde, next)?,
            &row_at_v1(aux_lde, index)?,
            &row_at_v1(aux_lde, next)?,
            &row_at_v1(fixed_lde, index)?,
            challenges,
        )?;
        let inverse_vanishing = x
            .pow(layout.trace_size() as u128)
            .sub(F::ONE)
            .inv()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        for lane in 0..SECURITY_LANES {
            let value = residues
                .iter()
                .zip(&alphas[lane])
                .fold(E::ZERO, |sum, (residue, alpha)| {
                    sum.add(alpha.mul_base(*residue))
                })
                .mul_base(inverse_vanishing);
            lanes[lane].push(value);
        }
        x = x.mul(lde_root);
    }
    let shared_layout = AggregateProofLayoutV1::for_segments(&[layout])?.as_shared()?;
    lanes
        .iter()
        .map(|lane| {
            aggregate::split_composition_evaluations_v1(
                lane,
                AGGREGATE_PARAMETERS_V1,
                &shared_layout,
            )
            .map_err(map_aggregate_error_v1)
        })
        .collect()
}

fn quotient_value_v1(
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    x: F,
    current_base: &[F],
    next_base: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    challenges: ZkX509IoChallengesV1,
    alphas: &[E],
) -> Result<E, ZkX509StarkErrorV1> {
    let residues = io_constraint_residues_v1(
        layout,
        logical_active_rows,
        current_base,
        next_base,
        current_aux,
        next_aux,
        fixed,
        challenges,
    )?;
    if residues.len() != alphas.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let inverse_vanishing = x
        .pow(layout.trace_size() as u128)
        .sub(F::ONE)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    Ok(residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
        .mul_base(inverse_vanishing))
}

fn accumulator_quotient_value_v1(
    layout: SegmentLayoutV1,
    x: F,
    residues: &[F],
    alphas: &[E],
) -> Result<E, ZkX509StarkErrorV1> {
    if residues.len() != layout.constraint_count || alphas.len() != layout.constraint_count {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let inverse_vanishing = x
        .pow(layout.trace_size() as u128)
        .sub(F::ONE)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    Ok(residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
        .mul_base(inverse_vanishing))
}

#[allow(clippy::too_many_arguments)]
fn der_quotient_value_v1(
    layout: SegmentLayoutV1,
    x: F,
    current_base: &[F],
    next_base: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    next_fixed: &[F],
    challenges: ZkX509DerStarkChallengesV1,
    public: ZkX509DerStarkPublicTerminalsV1,
    claims: ZkX509DerStarkTerminalClaimsV1,
    alphas: &[E],
) -> Result<E, ZkX509StarkErrorV1> {
    let current_base: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1] = current_base
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_base: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1] = next_base
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1] = current_aux
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1] = next_aux
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1] = fixed
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1] = next_fixed
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let residues = evaluate_zk_x509_der_stark_residues_v1(
        current_base,
        next_base,
        current_aux,
        next_aux,
        fixed,
        next_fixed,
        challenges,
        public,
        claims,
    )
    .map_err(|_| ZkX509StarkErrorV1::ConstraintOpening)?;
    if residues.len() != layout.constraint_count || residues.len() != alphas.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let inverse_vanishing = x
        .pow(layout.trace_size() as u128)
        .sub(F::ONE)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    Ok(residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
        .mul_base(inverse_vanishing))
}

fn projection_constraint_residues_v1(
    current_base: &[F],
    next_base: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    challenges: ZkX509ProjectionChallengesV1,
) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    let current_base: &[F; ZK_X509_PROJECTION_BASE_WIDTH_V1] = current_base
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_base: &[F; ZK_X509_PROJECTION_BASE_WIDTH_V1] = next_base
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let current_aux: &[F; ZK_X509_PROJECTION_AUX_WIDTH_V1] = current_aux
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_aux: &[F; ZK_X509_PROJECTION_AUX_WIDTH_V1] = next_aux
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let fixed: &[F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1] = fixed
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    evaluate_zk_x509_projection_stark_residues_v1(
        current_base,
        next_base,
        current_aux,
        next_aux,
        fixed,
        challenges,
    )
    .map_err(Into::into)
}

fn projection_composition_lanes_v1(
    material: &ProjectionTraceMaterialV1,
    base_lde: &[Vec<F>],
    aux_lde: &[Vec<F>],
    fixed_lde: &[Vec<F>],
    challenges: ZkX509ProjectionChallengesV1,
    alphas: &[Vec<E>],
) -> Result<Vec<Vec<Vec<E>>>, ZkX509StarkErrorV1> {
    if alphas.len() != SECURITY_LANES
        || alphas
            .iter()
            .any(|lane| lane.len() != material.layout.constraint_count)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let layout = material.layout;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    let mut lanes = (0..SECURITY_LANES)
        .map(|_| Vec::with_capacity(layout.lde_size()))
        .collect::<Vec<_>>();
    for index in 0..layout.lde_size() {
        let next = (index + BLOWUP) % layout.lde_size();
        let residues = projection_constraint_residues_v1(
            &row_at_v1(base_lde, index)?,
            &row_at_v1(base_lde, next)?,
            &row_at_v1(aux_lde, index)?,
            &row_at_v1(aux_lde, next)?,
            &row_at_v1(fixed_lde, index)?,
            challenges,
        )?;
        if residues.len() != layout.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let inverse_vanishing = x
            .pow(layout.trace_size() as u128)
            .sub(F::ONE)
            .inv()
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        for lane in 0..SECURITY_LANES {
            lanes[lane].push(
                residues
                    .iter()
                    .zip(&alphas[lane])
                    .fold(E::ZERO, |sum, (residue, alpha)| {
                        sum.add(alpha.mul_base(*residue))
                    })
                    .mul_base(inverse_vanishing),
            );
        }
        x = x.mul(lde_root);
    }
    let shared_layout = AggregateProofLayoutV1::for_segments(&[layout])?.as_shared()?;
    lanes
        .iter()
        .map(|lane| {
            aggregate::split_composition_evaluations_v1(
                lane,
                AGGREGATE_PARAMETERS_V1,
                &shared_layout,
            )
            .map_err(map_aggregate_error_v1)
        })
        .collect()
}

fn validate_projection_base_constraints_v1(
    material: &ProjectionTraceMaterialV1,
    challenges: ZkX509ProjectionChallengesV1,
) -> Result<(), ZkX509StarkErrorV1> {
    let rows = material.layout.trace_size();
    for index in 0..rows {
        let next = (index + 1) % rows;
        let residues = projection_constraint_residues_v1(
            &row_at_v1(&material.base_columns, index)?,
            &row_at_v1(&material.base_columns, next)?,
            &row_at_v1(&material.aux_columns, index)?,
            &row_at_v1(&material.aux_columns, next)?,
            &row_at_v1(&material.fixed_columns, index)?,
            challenges,
        )?;
        if residues.len() != material.layout.constraint_count
            || residues.iter().any(|residue| *residue != F::ZERO)
        {
            return Err(ZkX509StarkErrorV1::ProjectionWitness);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn projection_quotient_value_v1(
    layout: SegmentLayoutV1,
    x: F,
    current_base: &[F],
    next_base: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    challenges: ZkX509ProjectionChallengesV1,
    alphas: &[E],
) -> Result<E, ZkX509StarkErrorV1> {
    let residues = projection_constraint_residues_v1(
        current_base,
        next_base,
        current_aux,
        next_aux,
        fixed,
        challenges,
    )?;
    if residues.len() != layout.constraint_count || residues.len() != alphas.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let inverse_vanishing = x
        .pow(layout.trace_size() as u128)
        .sub(F::ONE)
        .inv()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    Ok(residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
        .mul_base(inverse_vanishing))
}

fn mix_fri_base_v1(
    layout: SegmentLayoutV1,
    base_lde: &[Vec<F>],
    aux_lde: &[Vec<F>],
    composition: &[Vec<E>],
    mix: &FriMixV1,
    deep_point: E,
    deep_trace: &aggregate::AggregateOpenedDeepTraceGroupV1,
    deep_composition: &[E],
) -> Result<Vec<E>, ZkX509StarkErrorV1> {
    let rows = composition
        .first()
        .map(Vec::len)
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    if base_lde.len() != mix.base.len()
        || base_lde.len() != mix.base_next.len()
        || aux_lde.len() != mix.aux.len()
        || aux_lde.len() != mix.aux_next.len()
        || composition.len() != mix.composition.len()
        || deep_trace.base_current.len() != base_lde.len()
        || deep_trace.base_next.len() != base_lde.len()
        || deep_trace.aux_current.len() != aux_lde.len()
        || deep_trace.aux_next.len() != aux_lde.len()
        || deep_composition.len() != composition.len()
        || rows != layout.lde_size()
        || composition.iter().any(|chunk| chunk.len() != rows)
        || base_lde
            .iter()
            .chain(aux_lde)
            .any(|column| column.len() != rows)
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let native_root =
        goldilocks_primitive_root_v1(layout.trace_log2).map_err(map_transparent_error_v1)?;
    let deep_next_point = deep_point.mul_base(native_root);
    let mut result = Vec::new();
    result
        .try_reserve_exact(rows)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for start in (0..rows).step_by(aggregate::DEFAULT_ENCRYPTED_TRACE_SCRATCH_CHUNK_ROWS_V1) {
        let end = start
            .checked_add(aggregate::DEFAULT_ENCRYPTED_TRACE_SCRATCH_CHUNK_ROWS_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .min(rows);
        let mut inverse_denominators = Vec::new();
        inverse_denominators
            .try_reserve_exact(
                end.checked_sub(start)
                    .and_then(|length| length.checked_mul(2))
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            )
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        let mut x_base = F(GOLDILOCKS_GENERATOR_V1).mul(
            lde_root.pow(u128::try_from(start).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?),
        );
        for _ in start..end {
            let query_point = E::from_base(x_base);
            inverse_denominators.push(query_point.sub(deep_point));
            inverse_denominators.push(query_point.sub(deep_next_point));
            x_base = x_base.mul(lde_root);
        }
        aggregate::batch_invert_fp4_nonzero_v1(&mut inverse_denominators)
            .map_err(map_aggregate_error_v1)?;
        for index in start..end {
            let local_index = index - start;
            let current_inverse = inverse_denominators[2 * local_index];
            let next_inverse = inverse_denominators[2 * local_index + 1];
            let mut quotient = E::ZERO;
            for (column_index, column) in base_lde.iter().enumerate() {
                let value = E::from_base(column[index]);
                quotient = quotient.add(
                    value
                        .sub(deep_trace.base_current[column_index])
                        .mul(current_inverse)
                        .mul(mix.base[column_index]),
                );
                quotient = quotient.add(
                    value
                        .sub(deep_trace.base_next[column_index])
                        .mul(next_inverse)
                        .mul(mix.base_next[column_index]),
                );
            }
            for (column_index, column) in aux_lde.iter().enumerate() {
                let value = E::from_base(column[index]);
                quotient = quotient.add(
                    value
                        .sub(deep_trace.aux_current[column_index])
                        .mul(current_inverse)
                        .mul(mix.aux[column_index]),
                );
                quotient = quotient.add(
                    value
                        .sub(deep_trace.aux_next[column_index])
                        .mul(next_inverse)
                        .mul(mix.aux_next[column_index]),
                );
            }
            for (chunk_index, (chunk, coefficient)) in
                composition.iter().zip(&mix.composition).enumerate()
            {
                quotient = quotient.add(
                    chunk[index]
                        .sub(deep_composition[chunk_index])
                        .mul(current_inverse)
                        .mul(*coefficient),
                );
            }
            result.push(quotient);
        }
    }
    Ok(result)
}

fn mix_opened_composition_chunks_v1(
    chunks: &[E],
    mix: &FriMixV1,
) -> Result<E, AggregateStarkErrorV1> {
    if chunks.is_empty() || chunks.len() != mix.composition.len() {
        return Err(AggregateStarkErrorV1::ConstraintOpening);
    }
    Ok(chunks
        .iter()
        .zip(&mix.composition)
        .fold(E::ZERO, |sum, (value, coefficient)| {
            sum.add(value.mul(*coefficient))
        }))
}

fn build_fri_lane_v1(
    lane: usize,
    layout: SegmentLayoutV1,
    base_values: Vec<E>,
    transcript: &mut TransparentTranscriptV1,
) -> Result<FriLaneMaterialV1, ZkX509StarkErrorV1> {
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&[layout])?;
    let parameters = aggregate_layout.parameters_v1();
    aggregate::build_fri_lane_v1(
        parameters,
        AGGREGATE_DOMAINS_V1,
        &aggregate_layout.as_shared()?,
        lane,
        base_values,
        transcript,
    )
    .map_err(map_aggregate_error_v1)
}

fn maximum_encoded_aggregate_proof_bytes_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<usize, ZkX509StarkErrorV1> {
    aggregate::maximum_encoded_proof_with_deep_bytes_v1(
        layout.parameters_v1(),
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)
}

fn exact_encoded_aggregate_proof_bytes_v1(
    proof: &ZkX509SegmentedStarkProofV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, ZkX509StarkErrorV1> {
    aggregate::exact_encoded_proof_with_deep_bytes_v1(
        &proof.aggregate,
        &proof.deep,
        layout.parameters_v1(),
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)
}

fn canonical_multiproof_frontier_v1(
    tree: &Sha256MerkleTreeV1,
    leaf_count: usize,
    indices: &[usize],
) -> Result<Vec<[u8; 32]>, ZkX509StarkErrorV1> {
    aggregate::canonical_multiproof_frontier_v1(tree, leaf_count, indices)
        .map_err(map_aggregate_error_v1)
}

fn verify_canonical_multiproof_v1(
    node_domain: &[u8],
    root: &[u8; 32],
    leaf_count: usize,
    leaves: &BTreeMap<usize, [u8; 32]>,
    frontier: &[[u8; 32]],
) -> Result<(), ()> {
    aggregate::verify_canonical_multiproof_v1(node_domain, root, leaf_count, leaves, frontier)
        .map_err(|_| ())
}

fn encode_zk_x509_segmented_stark_proof_v1(
    proof: &ZkX509SegmentedStarkProofV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    aggregate::encode_proof_with_deep_v1(
        &proof.aggregate,
        &proof.deep,
        layout.parameters_v1(),
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)
}

fn decode_zk_x509_segmented_stark_proof_v1(
    bytes: &[u8],
    layout: &AggregateProofLayoutV1,
) -> Result<ZkX509SegmentedStarkProofV1, ZkX509StarkErrorV1> {
    let (aggregate, deep) =
        aggregate::decode_proof_with_deep_v1(bytes, layout.parameters_v1(), &layout.as_shared()?)
            .map_err(map_aggregate_error_v1)?;
    Ok(ZkX509SegmentedStarkProofV1 { aggregate, deep })
}

fn query_indices_v1(
    transcript: &TransparentTranscriptV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<usize>, ZkX509StarkErrorV1> {
    aggregate::query_indices_v1(
        transcript,
        layout.parameters_v1(),
        AGGREGATE_DOMAINS_V1,
        &layout.as_shared()?,
    )
    .map_err(map_aggregate_error_v1)
}

fn absorb_grinding_nonce_v1(
    transcript: &mut TransparentTranscriptV1,
    nonce: u64,
) -> Result<(), ZkX509StarkErrorV1> {
    transcript
        .absorb(b"zk-x509-grinding-nonce-v1", &[&nonce.to_be_bytes()])
        .map_err(map_transparent_error_v1)
}

/// Construct the canonical byte-memory segmented proof with an injected RNG.
///
/// The injected fallible RNG is used by deterministic KATs and entropy-failure
/// tests. Production callers of this currently internal API use `OsRng`.
pub(crate) fn prove_zk_x509_io_segmented_stark_v1_with_rng<R: TryRngCore>(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    validate_declarations_v1(&statement.declarations)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let public_digest = io_public_digest_v1(statement)?;
    let (layout, base_columns, fixed_columns, execution, sorted) =
        build_io_base_and_fixed_columns_v1(statement, witnesses)?;
    let layouts = [layout];
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&layouts)?;
    let shared_layout = aggregate_layout.as_shared()?;
    let base_lde = masked_lde_columns_v1(&base_columns, layout, rng)?;
    let base_tree = row_tree_v1(
        BASE_LEAF_DOMAIN,
        BASE_NODE_DOMAIN,
        0,
        &base_lde,
        layout.lde_size(),
    )?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:io-aggregate-layout:v1",
        &aggregate_layout,
    )?;
    let mut trace_group_proofs = vec![TraceGroupProofV1 {
        base_root: base_tree.root(),
        aux_root: [0; 32],
        base_frontier: Vec::new(),
        aux_frontier: Vec::new(),
    }];
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;

    let io_challenges =
        derive_zk_x509_io_challenges_v1(&mut transcript).map_err(map_transparent_error_v1)?;
    let logical_active_rows = io_active_rows_v1(&statement.declarations)?;
    let aux_columns = build_io_aux_columns_v1(
        statement,
        witnesses,
        io_challenges,
        layout,
        logical_active_rows,
        &execution,
        &sorted,
    )?;
    let trace_material = IoTraceMaterialV1 {
        layout,
        logical_active_rows,
        base_columns,
        aux_columns,
        fixed_columns,
    };
    validate_io_base_constraints_v1(&trace_material, io_challenges)?;
    let aux_lde = masked_lde_columns_v1(&trace_material.aux_columns, layout, rng)?;
    let aux_tree = row_tree_v1(
        AUX_LEAF_DOMAIN,
        AUX_NODE_DOMAIN,
        0,
        &aux_lde,
        layout.lde_size(),
    )?;
    trace_group_proofs[0].aux_root = aux_tree.root();
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;

    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;
    let fixed_lde = fixed_lde_columns_v1(&trace_material.fixed_columns, layout)?;
    let compositions = composition_lanes_v1(
        &trace_material,
        &base_lde,
        &aux_lde,
        &fixed_lde,
        io_challenges,
        &alphas[0],
    )?;
    let mut composition_trees = Vec::with_capacity(SECURITY_LANES);
    let mut composition_roots = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let tree = composition_tree_v1(lane, &compositions[lane])?;
        composition_roots.push(tree.root());
        composition_trees.push(tree);
    }
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let fri_masks =
        aggregate::build_fri_mask_oracles_v1(AGGREGATE_PARAMETERS_V1, &shared_layout, rng)
            .map_err(map_aggregate_error_v1)?;
    let fri_mask_roots = fri_masks
        .iter()
        .map(|mask| mask.tree.root())
        .collect::<Vec<_>>();
    aggregate::absorb_fri_mask_roots_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &fri_mask_roots)
        .map_err(map_aggregate_error_v1)?;

    let trace_materials = vec![aggregate::AggregateTraceGroupMaterialV1 {
        base_lde,
        aux_lde,
        base_tree,
        aux_tree,
    }];
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    let deep = aggregate::build_materialized_deep_proof_v1(
        &trace_materials,
        &compositions,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        deep_point,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let (deep_trace_groups, deep_compositions) =
        canonical_deep_values_v1(&deep, &aggregate_layout)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let mut fri_lanes = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let mut base = mix_fri_base_v1(
            layout,
            &trace_materials[0].base_lde,
            &trace_materials[0].aux_lde,
            &compositions[lane],
            &mixes[0][lane],
            deep_point,
            &deep_trace_groups[0],
            &deep_compositions[lane],
        )?;
        aggregate::add_fri_mask_oracle_v1(&mut base, &fri_masks[lane])
            .map_err(map_aggregate_error_v1)?;
        fri_lanes.push(build_fri_lane_v1(lane, layout, base, &mut transcript)?);
    }
    let grinding_state = transcript.state();
    let grinding_nonce = grind_nonce_v1(&grinding_state, ZK_X509_GRINDING_BITS_V1)
        .map_err(map_transparent_error_v1)?;
    absorb_grinding_nonce_v1(&mut transcript, grinding_nonce)?;

    let queries = query_indices_v1(&transcript, &aggregate_layout)?
        .into_iter()
        .map(|index| {
            aggregate::build_query_v1(
                AGGREGATE_PARAMETERS_V1,
                &shared_layout,
                index,
                &trace_materials,
                &compositions,
                &fri_masks,
                &fri_lanes,
            )
            .map_err(map_aggregate_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (trace_frontiers, composition_frontiers, fri_mask_frontiers, fri_round_frontiers) =
        aggregate::build_all_frontiers_v1(
            AGGREGATE_PARAMETERS_V1,
            &shared_layout,
            &queries,
            &trace_materials,
            &composition_trees,
            &fri_masks,
            &fri_lanes,
        )
        .map_err(map_aggregate_error_v1)?;
    for (group, (base_frontier, aux_frontier)) in trace_group_proofs.iter_mut().zip(trace_frontiers)
    {
        group.base_frontier = base_frontier;
        group.aux_frontier = aux_frontier;
    }
    let proof = ZkX509SegmentedStarkProofV1 {
        aggregate: aggregate::AggregateStarkProofV1 {
            version: ZK_X509_PROOF_VERSION_V1,
            trace_groups: trace_group_proofs,
            composition_roots,
            composition_frontiers,
            fri_mask_roots,
            fri_mask_frontiers,
            fri_lanes: fri_lanes
                .into_iter()
                .zip(fri_round_frontiers)
                .map(|(lane, round_frontiers)| FriLaneProofV1 {
                    roots: lane.roots,
                    terminal_values: lane
                        .terminal_values
                        .into_iter()
                        .map(|value| value.coefficients().map(F::value))
                        .collect(),
                    round_frontiers,
                })
                .collect(),
            queries,
            grinding_nonce,
        },
        deep,
    };
    let encoded = encode_zk_x509_segmented_stark_proof_v1(&proof, &aggregate_layout)?;
    verify_zk_x509_io_segmented_stark_v1(statement, &encoded)?;
    Ok(encoded)
}

/// Construct the canonical byte-memory proof with operating-system entropy.
#[allow(dead_code)]
pub(crate) fn prove_zk_x509_io_segmented_stark_v1(
    statement: &ZkX509IoStarkStatementV1,
    witnesses: &[ZkX509IoChannelWitnessV1],
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    prove_zk_x509_io_segmented_stark_v1_with_rng(statement, witnesses, &mut rand::rngs::OsRng)
}

/// Prove the registered projection AIR with injected masking entropy.
///
/// This bounded proof constrains the projection trace itself. Its SHA and DER
/// byte channels remain deliberately outside this proof until the aggregate
/// cross-segment I/O registration is complete.
pub(crate) fn prove_zk_x509_projection_segmented_stark_v1_with_rng<R: TryRngCore>(
    statement: &IrohaZkX509StarkP256StatementV1,
    witness: &ZkX509ProjectionWitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    let public_digest = projection_public_digest_v1(statement)?;
    let (layout, trace, base_columns, fixed_columns) =
        build_projection_base_material_v1(statement, witness)?;
    let layouts = [layout];
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&layouts)?;
    let shared_layout = aggregate_layout.as_shared()?;
    let base_lde = masked_lde_columns_v1(&base_columns, layout, rng)?;
    let base_tree = row_tree_v1(
        BASE_LEAF_DOMAIN,
        BASE_NODE_DOMAIN,
        0,
        &base_lde,
        layout.lde_size(),
    )?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:projection-aggregate-layout:v1",
        &aggregate_layout,
    )?;
    let mut trace_group_proofs = vec![TraceGroupProofV1 {
        base_root: base_tree.root(),
        aux_root: [0; 32],
        base_frontier: Vec::new(),
        aux_frontier: Vec::new(),
    }];
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;

    let projection_challenges = derive_projection_challenges_v1(&mut transcript)?;
    let material = build_projection_trace_material_v1(
        layout,
        trace,
        base_columns,
        fixed_columns,
        projection_challenges,
    )?;
    validate_projection_base_constraints_v1(&material, projection_challenges)?;
    let aux_lde = masked_lde_columns_v1(&material.aux_columns, layout, rng)?;
    let aux_tree = row_tree_v1(
        AUX_LEAF_DOMAIN,
        AUX_NODE_DOMAIN,
        0,
        &aux_lde,
        layout.lde_size(),
    )?;
    trace_group_proofs[0].aux_root = aux_tree.root();
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;

    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;
    let fixed_lde = fixed_lde_columns_v1(&material.fixed_columns, layout)?;
    let compositions = projection_composition_lanes_v1(
        &material,
        &base_lde,
        &aux_lde,
        &fixed_lde,
        projection_challenges,
        &alphas[0],
    )?;
    let mut composition_trees = Vec::with_capacity(SECURITY_LANES);
    let mut composition_roots = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let tree = composition_tree_v1(lane, &compositions[lane])?;
        composition_roots.push(tree.root());
        composition_trees.push(tree);
    }
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let fri_masks =
        aggregate::build_fri_mask_oracles_v1(AGGREGATE_PARAMETERS_V1, &shared_layout, rng)
            .map_err(map_aggregate_error_v1)?;
    let fri_mask_roots = fri_masks
        .iter()
        .map(|mask| mask.tree.root())
        .collect::<Vec<_>>();
    aggregate::absorb_fri_mask_roots_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &fri_mask_roots)
        .map_err(map_aggregate_error_v1)?;

    let trace_materials = vec![aggregate::AggregateTraceGroupMaterialV1 {
        base_lde,
        aux_lde,
        base_tree,
        aux_tree,
    }];
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    let deep = aggregate::build_materialized_deep_proof_v1(
        &trace_materials,
        &compositions,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        deep_point,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let (deep_trace_groups, deep_compositions) =
        canonical_deep_values_v1(&deep, &aggregate_layout)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let mut fri_lanes = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let mut base = mix_fri_base_v1(
            layout,
            &trace_materials[0].base_lde,
            &trace_materials[0].aux_lde,
            &compositions[lane],
            &mixes[0][lane],
            deep_point,
            &deep_trace_groups[0],
            &deep_compositions[lane],
        )?;
        aggregate::add_fri_mask_oracle_v1(&mut base, &fri_masks[lane])
            .map_err(map_aggregate_error_v1)?;
        fri_lanes.push(build_fri_lane_v1(lane, layout, base, &mut transcript)?);
    }
    let grinding_state = transcript.state();
    let grinding_nonce = grind_nonce_v1(&grinding_state, ZK_X509_GRINDING_BITS_V1)
        .map_err(map_transparent_error_v1)?;
    absorb_grinding_nonce_v1(&mut transcript, grinding_nonce)?;

    let queries = query_indices_v1(&transcript, &aggregate_layout)?
        .into_iter()
        .map(|index| {
            aggregate::build_query_v1(
                AGGREGATE_PARAMETERS_V1,
                &shared_layout,
                index,
                &trace_materials,
                &compositions,
                &fri_masks,
                &fri_lanes,
            )
            .map_err(map_aggregate_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (trace_frontiers, composition_frontiers, fri_mask_frontiers, fri_round_frontiers) =
        aggregate::build_all_frontiers_v1(
            AGGREGATE_PARAMETERS_V1,
            &shared_layout,
            &queries,
            &trace_materials,
            &composition_trees,
            &fri_masks,
            &fri_lanes,
        )
        .map_err(map_aggregate_error_v1)?;
    for (group, (base_frontier, aux_frontier)) in trace_group_proofs.iter_mut().zip(trace_frontiers)
    {
        group.base_frontier = base_frontier;
        group.aux_frontier = aux_frontier;
    }
    let proof = ZkX509SegmentedStarkProofV1 {
        aggregate: aggregate::AggregateStarkProofV1 {
            version: ZK_X509_PROOF_VERSION_V1,
            trace_groups: trace_group_proofs,
            composition_roots,
            composition_frontiers,
            fri_mask_roots,
            fri_mask_frontiers,
            fri_lanes: fri_lanes
                .into_iter()
                .zip(fri_round_frontiers)
                .map(|(lane, round_frontiers)| FriLaneProofV1 {
                    roots: lane.roots,
                    terminal_values: lane
                        .terminal_values
                        .into_iter()
                        .map(|value| value.coefficients().map(F::value))
                        .collect(),
                    round_frontiers,
                })
                .collect(),
            queries,
            grinding_nonce,
        },
        deep,
    };
    let encoded = encode_zk_x509_segmented_stark_proof_v1(&proof, &aggregate_layout)?;
    verify_zk_x509_projection_segmented_stark_v1(statement, &encoded)?;
    Ok(encoded)
}

/// Prove the registered projection AIR with operating-system entropy.
#[allow(dead_code)]
pub(crate) fn prove_zk_x509_projection_segmented_stark_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    witness: &ZkX509ProjectionWitnessV1,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    prove_zk_x509_projection_segmented_stark_v1_with_rng(statement, witness, &mut rand::rngs::OsRng)
}

#[allow(clippy::too_many_lines)]
fn build_zk_x509_der_segmented_stark_proof_v1_with_rng<R: TryRngCore>(
    shape: &ZkX509DerStarkShapeV1,
    documents: &[&[u8]],
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    shape
        .validate()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let base =
        build_zk_x509_der_stark_base_v1(documents).map_err(|_| ZkX509StarkErrorV1::DerWitness)?;
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(shape.clone())
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let layout = SegmentLayoutV1::for_der(schedule.active_rows())?;
    layout.validate()?;
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&[layout])?;
    let shared_layout = aggregate_layout.as_shared()?;
    let public_digest = der_public_digest_v1(shape)?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:der-aggregate-layout:v1",
        &aggregate_layout,
    )?;

    let (base_commitment, base_polynomials) = aggregate::commit_masked_trace_polynomial_columns_v1(
        BASE_LEAF_DOMAIN,
        BASE_NODE_DOMAIN,
        0,
        layout.trace_log2,
        layout.lde_log2,
        ZK_X509_DER_STARK_BASE_WIDTH_V1,
        MASK_DEGREE,
        &[],
        rng,
        |column| {
            build_zk_x509_der_stark_native_base_column_v1(&base, column)
                .map_err(|_| AggregateStarkErrorV1::InternalInvariant)
        },
    )
    .map_err(map_aggregate_error_v1)?;
    let mut trace_group_proofs = vec![TraceGroupProofV1 {
        base_root: base_commitment.commitment.root,
        aux_root: [0; 32],
        base_frontier: Vec::new(),
        aux_frontier: Vec::new(),
    }];
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;

    let challenges = derive_der_challenges_v1(&mut transcript)?;
    let public = derive_zk_x509_der_stark_public_terminals_v1(shape, challenges)
        .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    let trace = build_zk_x509_der_stark_trace_v1(base, challenges)
        .map_err(|_| ZkX509StarkErrorV1::DerWitness)?;
    let claims =
        zk_x509_der_stark_terminal_claims_v1(&trace).map_err(|_| ZkX509StarkErrorV1::DerWitness)?;
    let (aux_commitment, aux_polynomials) = aggregate::commit_masked_trace_polynomial_columns_v1(
        AUX_LEAF_DOMAIN,
        AUX_NODE_DOMAIN,
        0,
        layout.trace_log2,
        layout.lde_log2,
        ZK_X509_DER_STARK_AUX_WIDTH_V1,
        MASK_DEGREE,
        &[],
        rng,
        |column| {
            build_zk_x509_der_stark_native_aux_column_v1(&trace, column)
                .map_err(|_| AggregateStarkErrorV1::InternalInvariant)
        },
    )
    .map_err(map_aggregate_error_v1)?;
    // The retained masked polynomials are the sole post-commitment source.
    // Release the native 76+196-column trace before allocating the larger
    // degree-seven quotient coset.
    drop(trace);
    trace_group_proofs[0].aux_root = aux_commitment.commitment.root;
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_group_proofs)
        .map_err(map_aggregate_error_v1)?;
    absorb_der_terminal_claims_v1(&mut transcript, claims)?;
    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;

    let composition_material = der_composition_material_from_polynomials_v1(
        layout,
        &schedule,
        &base_polynomials,
        &aux_polynomials,
        challenges,
        public,
        claims,
        &alphas[0],
    )?;
    let compositions = &composition_material.evaluations;

    let mut composition_roots = Vec::new();
    composition_roots
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for (lane, composition) in compositions.iter().enumerate() {
        composition_roots.push(
            aggregate::streaming_composition_commitment_v1(
                AGGREGATE_DOMAINS_V1,
                lane,
                composition,
                &[],
            )
            .map_err(map_aggregate_error_v1)?
            .root,
        );
    }
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let fri_masks =
        aggregate::build_fri_mask_oracles_v1(AGGREGATE_PARAMETERS_V1, &shared_layout, rng)
            .map_err(map_aggregate_error_v1)?;
    let fri_mask_roots = fri_masks
        .iter()
        .map(|mask| mask.tree.root())
        .collect::<Vec<_>>();
    aggregate::absorb_fri_mask_roots_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &fri_mask_roots)
        .map_err(map_aggregate_error_v1)?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    let (base_current, base_next) = aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(
        &base_polynomials,
        deep_point,
    )
    .map_err(map_aggregate_error_v1)?;
    let (aux_current, aux_next) = aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(
        &aux_polynomials,
        deep_point,
    )
    .map_err(map_aggregate_error_v1)?;
    let deep_composition_values = evaluate_retained_composition_coefficients_at_deep_v1(
        &composition_material.coefficient_chunks,
        deep_point,
    )?;
    let deep = aggregate::AggregateDeepProofV1 {
        trace_groups: vec![aggregate::AggregateDeepTraceGroupOpeningV1 {
            base_current: fp4_values_to_wire_v1(base_current),
            base_next: fp4_values_to_wire_v1(base_next),
            aux_current: fp4_values_to_wire_v1(aux_current),
            aux_next: fp4_values_to_wire_v1(aux_next),
        }],
        composition_values: deep_composition_values
            .into_iter()
            .map(fp4_values_to_wire_v1)
            .collect(),
    };
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let (deep_trace_groups, deep_compositions) =
        canonical_deep_values_v1(&deep, &aggregate_layout)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let mut fri_bases = der_fri_bases_from_polynomials_v1(
        layout,
        &base_polynomials,
        &aux_polynomials,
        &composition_material.coefficient_chunks,
        &mixes[0],
        deep_point,
        &deep_trace_groups[0],
        &deep_compositions,
    )?;
    for (base, mask) in fri_bases.iter_mut().zip(&fri_masks) {
        aggregate::add_fri_mask_oracle_v1(base, mask).map_err(map_aggregate_error_v1)?;
    }
    let mut fri_materials = Vec::new();
    fri_materials
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for lane in 0..SECURITY_LANES {
        let base_values = core::mem::take(
            fri_bases
                .get_mut(lane)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        );
        fri_materials.push(
            aggregate::build_streaming_fri_lane_v1(
                AGGREGATE_PARAMETERS_V1,
                AGGREGATE_DOMAINS_V1,
                &shared_layout,
                lane,
                base_values,
                &mut transcript,
            )
            .map_err(map_aggregate_error_v1)?,
        );
    }
    fri_bases = der_fri_bases_from_polynomials_v1(
        layout,
        &base_polynomials,
        &aux_polynomials,
        &composition_material.coefficient_chunks,
        &mixes[0],
        deep_point,
        &deep_trace_groups[0],
        &deep_compositions,
    )?;
    for (base, mask) in fri_bases.iter_mut().zip(&fri_masks) {
        aggregate::add_fri_mask_oracle_v1(base, mask).map_err(map_aggregate_error_v1)?;
    }

    let grinding_state = transcript.state();
    let grinding_nonce = grind_nonce_v1(&grinding_state, ZK_X509_GRINDING_BITS_V1)
        .map_err(map_transparent_error_v1)?;
    absorb_grinding_nonce_v1(&mut transcript, grinding_nonce)?;
    let query_indices = query_indices_v1(&transcript, &aggregate_layout)?;
    let query_skeleton = query_indices
        .iter()
        .map(|index| {
            Ok(aggregate::AggregateQueryProofV1 {
                index: u32::try_from(*index).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
                trace_groups: Vec::new(),
                composition_values: Vec::new(),
                fri_mask_values: Vec::new(),
                fri_lanes: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, ZkX509StarkErrorV1>>()?;
    let trace_opening_indices =
        aggregate::trace_group_opening_indices_v1(&query_skeleton, &shared_layout, 0)
            .map_err(map_aggregate_error_v1)?;
    let base_openings = aggregate::replay_masked_trace_polynomial_columns_v1(
        BASE_LEAF_DOMAIN,
        BASE_NODE_DOMAIN,
        0,
        &base_polynomials,
        &trace_opening_indices,
    )
    .map_err(map_aggregate_error_v1)?;
    let aux_openings = aggregate::replay_masked_trace_polynomial_columns_v1(
        AUX_LEAF_DOMAIN,
        AUX_NODE_DOMAIN,
        0,
        &aux_polynomials,
        &trace_opening_indices,
    )
    .map_err(map_aggregate_error_v1)?;
    if base_openings.commitment.root != trace_group_proofs[0].base_root
        || aux_openings.commitment.root != trace_group_proofs[0].aux_root
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    trace_group_proofs[0].base_frontier = base_openings.commitment.frontier;
    trace_group_proofs[0].aux_frontier = aux_openings.commitment.frontier;
    drop(base_polynomials);
    drop(aux_polynomials);

    let composition_opening_indices =
        aggregate::composition_opening_indices_v1(&query_skeleton, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    let mut composition_frontiers = Vec::new();
    composition_frontiers
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for (lane, composition) in compositions.iter().enumerate() {
        let commitment = aggregate::streaming_composition_commitment_v1(
            AGGREGATE_DOMAINS_V1,
            lane,
            composition,
            &composition_opening_indices,
        )
        .map_err(map_aggregate_error_v1)?;
        if commitment.root != composition_roots[lane] {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        composition_frontiers.push(commitment.frontier);
    }
    let fri_mask_frontiers = fri_masks
        .iter()
        .map(|mask| {
            aggregate::canonical_multiproof_frontier_v1(
                &mask.tree,
                aggregate_layout.common_lde_size(),
                &composition_opening_indices,
            )
            .map_err(map_aggregate_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut fri_openings = Vec::new();
    fri_openings
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for (lane, (base_values, material)) in fri_bases.into_iter().zip(&fri_materials).enumerate() {
        fri_openings.push(
            aggregate::open_streaming_fri_lane_v1(
                AGGREGATE_PARAMETERS_V1,
                AGGREGATE_DOMAINS_V1,
                &shared_layout,
                lane,
                base_values,
                material,
                &query_indices,
            )
            .map_err(map_aggregate_error_v1)?,
        );
    }

    let mut queries = Vec::new();
    queries
        .try_reserve_exact(query_indices.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    let next_stride = aggregate_layout
        .trace_groups
        .first()
        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
        .next_stride(aggregate_layout.common_lde_log2)?;
    for (query_position, index) in query_indices.iter().copied().enumerate() {
        let next = (index + next_stride) % aggregate_layout.common_lde_size();
        let base_current = base_openings
            .opened_rows
            .get(&index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
            .iter()
            .map(|value| value.0)
            .collect();
        let base_next = base_openings
            .opened_rows
            .get(&next)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
            .iter()
            .map(|value| value.0)
            .collect();
        let aux_current = aux_openings
            .opened_rows
            .get(&index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
            .iter()
            .map(|value| value.0)
            .collect();
        let aux_next = aux_openings
            .opened_rows
            .get(&next)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
            .iter()
            .map(|value| value.0)
            .collect();
        queries.push(aggregate::AggregateQueryProofV1 {
            index: u32::try_from(index).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            trace_groups: vec![aggregate::AggregateTraceGroupQueryV1 {
                base_current,
                base_next,
                aux_current,
                aux_next,
            }],
            composition_values: compositions
                .iter()
                .map(|lane| {
                    lane.iter()
                        .map(|chunk| chunk[index].coefficients().map(F::value))
                        .collect()
                })
                .collect(),
            fri_mask_values: fri_masks
                .iter()
                .map(|mask| mask.evaluations[index].coefficients().map(F::value))
                .collect(),
            fri_lanes: fri_openings
                .iter()
                .map(|lane| {
                    lane.queries
                        .get(query_position)
                        .cloned()
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }

    let fri_lanes = fri_materials
        .into_iter()
        .zip(fri_openings)
        .map(|(material, openings)| FriLaneProofV1 {
            roots: material.roots,
            terminal_values: material
                .terminal_values
                .into_iter()
                .map(|value| value.coefficients().map(F::value))
                .collect(),
            round_frontiers: openings.round_frontiers,
        })
        .collect();
    let aggregate_proof = ZkX509SegmentedStarkProofV1 {
        aggregate: aggregate::AggregateStarkProofV1 {
            version: ZK_X509_PROOF_VERSION_V1,
            trace_groups: trace_group_proofs,
            composition_roots,
            composition_frontiers,
            fri_mask_roots,
            fri_mask_frontiers,
            fri_lanes,
            queries,
            grinding_nonce,
        },
        deep,
    };
    let aggregate_bytes =
        encode_zk_x509_segmented_stark_proof_v1(&aggregate_proof, &aggregate_layout)?;
    encode_der_segmented_proof_envelope_v1(claims, &aggregate_bytes)
}

/// Construct and self-verify the canonical strict-DER aggregate proof.
///
/// Trace columns are committed one at a time and retained only as zeroizing
/// masked coefficients. Constraints are evaluated on the exact minimal
/// quotient coset; commitment replay and FRI batching never allocate or spill
/// a common-domain trace matrix.
#[allow(dead_code)]
pub(crate) fn prove_zk_x509_der_segmented_stark_v1_with_rng<R: TryRngCore>(
    shape: &ZkX509DerStarkShapeV1,
    documents: &[&[u8]],
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    let encoded = build_zk_x509_der_segmented_stark_proof_v1_with_rng(shape, documents, rng)?;
    verify_zk_x509_der_segmented_stark_v1(shape, &encoded)?;
    Ok(encoded)
}

/// Prove the registered strict-DER AIR with operating-system entropy.
#[allow(dead_code)]
pub(crate) fn prove_zk_x509_der_segmented_stark_v1(
    shape: &ZkX509DerStarkShapeV1,
    documents: &[&[u8]],
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    prove_zk_x509_der_segmented_stark_v1_with_rng(shape, documents, &mut rand::rngs::OsRng)
}

struct IoOpenedRowEvaluatorV1<'a> {
    aggregate_layout: &'a AggregateProofLayoutV1,
    layout: SegmentLayoutV1,
    logical_active_rows: usize,
    fixed_lde: &'a [Vec<F>],
    io_challenges: ZkX509IoChallengesV1,
    alphas: &'a [Vec<E>],
    mixes: &'a [FriMixV1],
    lde_root: F,
}

struct DerOpenedRowEvaluatorV1<'a> {
    aggregate_layout: &'a AggregateProofLayoutV1,
    layout: SegmentLayoutV1,
    fixed_openings: &'a BTreeMap<usize, [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1]>,
    challenges: ZkX509DerStarkChallengesV1,
    public: ZkX509DerStarkPublicTerminalsV1,
    claims: ZkX509DerStarkTerminalClaimsV1,
    alphas: &'a [Vec<E>],
    mixes: &'a [FriMixV1],
    lde_root: F,
}

#[derive(Clone, Copy)]
struct RegisteredOpenedRowsV1<'a> {
    base_current: &'a [F],
    base_next: &'a [F],
    aux_current: &'a [F],
    aux_next: &'a [F],
}

fn registered_opened_rows_v1<'a>(
    aggregate_layout: &AggregateProofLayoutV1,
    registration: RegisteredSegmentLayoutV1,
    trace_groups: &'a [aggregate::AggregateOpenedTraceGroupV1],
) -> Result<RegisteredOpenedRowsV1<'a>, AggregateStarkErrorV1> {
    if trace_groups.len() != aggregate_layout.trace_groups.len() {
        return Err(AggregateStarkErrorV1::ConstraintOpening);
    }
    let group = trace_groups
        .get(registration.trace_group)
        .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
    let expected = aggregate_layout
        .trace_groups
        .get(registration.trace_group)
        .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
    if group.base_current.len() != expected.base_width
        || group.base_next.len() != expected.base_width
        || group.aux_current.len() != expected.aux_width
        || group.aux_next.len() != expected.aux_width
    {
        return Err(AggregateStarkErrorV1::ConstraintOpening);
    }
    let base_end = registration
        .base_end()
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
    let aux_end = registration
        .aux_end()
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
    Ok(RegisteredOpenedRowsV1 {
        base_current: group
            .base_current
            .get(registration.base_start..base_end)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?,
        base_next: group
            .base_next
            .get(registration.base_start..base_end)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?,
        aux_current: group
            .aux_current
            .get(registration.aux_start..aux_end)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?,
        aux_next: group
            .aux_next
            .get(registration.aux_start..aux_end)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?,
    })
}

/// One native trace-column implementation behind the unified MAIN boundary.
///
/// The verifier-owned registration is passed into every operation. Providers
/// never supply adapter identities, instances, ranges, widths, or native
/// logarithms themselves.
trait MainTraceGroupSourceV1 {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1>;

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1>;
}

/// A copied witness column which is overwritten before its allocation is
/// released.
#[derive(Debug, PartialEq, Eq)]
struct ZeroizingMainTraceColumnV1(Vec<F>);

impl ZeroizingMainTraceColumnV1 {
    fn into_vec_v1(mut self) -> Vec<F> {
        core::mem::take(&mut self.0)
    }

    fn zeroize_private_v1(&mut self) {
        self.0.fill(F::ZERO);
        self.0.clear();
    }
}

impl core::ops::Deref for ZeroizingMainTraceColumnV1 {
    type Target = [F];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ZeroizingMainTraceColumnV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq<Vec<F>> for ZeroizingMainTraceColumnV1 {
    fn eq(&self, other: &Vec<F>) -> bool {
        self.0.as_slice() == other.as_slice()
    }
}

impl Drop for ZeroizingMainTraceColumnV1 {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MainP256RegistrationBindingV1 {
    main: RegisteredSegmentLayoutV1,
    p256: P256MainRegistrationV1,
}

fn canonical_p256_main_layout_bindings_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<MainP256RegistrationBindingV1>, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(
            P256_CERTIFICATE_REGISTRATION_COUNT_V1 * (P256_SIGNATURE_COUNT_V1 - 1)
                + P256_WALLET_REGISTRATION_COUNT_V1,
        )
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for main in layout
        .registered_segments
        .iter()
        .copied()
        .filter(|registration| {
            matches!(
                registration.segment.adapter,
                SegmentAdapterIdV1::P256Arithmetic
                    | SegmentAdapterIdV1::P256Reduction
                    | SegmentAdapterIdV1::P256LowS
                    | SegmentAdapterIdV1::P256Window
                    | SegmentAdapterIdV1::P256ValueBus
                    | SegmentAdapterIdV1::P256ScalarBitBus
            )
        })
    {
        bindings.push(MainP256RegistrationBindingV1 {
            main,
            p256: p256_main_registration_from_main_layout_v1(main)?,
        });
    }

    // Validate the complete central order independently of MAIN's
    // equal-native-log order, then prove a one-to-one association with all
    // MAIN P-256 slices.
    let mut central_order = Vec::new();
    central_order
        .try_reserve_exact(bindings.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        for (adapter, local) in [
            (P256MainAdapterV1::ValueBus, 0),
            (P256MainAdapterV1::ValueBus, 1),
            (P256MainAdapterV1::Arithmetic, 0),
            (P256MainAdapterV1::WindowBatch, 0),
            (P256MainAdapterV1::Reduction, 0),
            (P256MainAdapterV1::Reduction, 1),
        ] {
            central_order.push(
                P256MainRegistrationV1::new_v1(signature, adapter, local)
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
        if signature == P256_SIGNATURE_COUNT_V1 - 1 {
            central_order.push(
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::WalletLowS, 0)
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
        for adapter in [
            P256MainAdapterV1::BindingSink,
            P256MainAdapterV1::ScalarBitBus,
        ] {
            central_order.push(
                P256MainRegistrationV1::new_v1(signature, adapter, 0)
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
    }
    validate_p256_main_registration_order_v1(&central_order)
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    if bindings.len() != central_order.len()
        || central_order.iter().any(|expected| {
            bindings
                .iter()
                .filter(|binding| binding.p256 == *expected)
                .count()
                != 1
        })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(bindings)
}

const P256_MAIN_LOG5_REGISTRATION_COUNT_V1: usize = 11;
const P256_MAIN_LOG5_BASE_WIDTH_V1: usize =
    10 * P256_REDUCTION_BASE_WIDTH_V1 + P256_LOW_S_BASE_WIDTH_V1;
const P256_MAIN_LOG5_AUX_WIDTH_V1: usize =
    10 * P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1 + P256_LOW_S_AGGREGATE_AUX_WIDTH_V1;
const _: () = assert!(P256_MAIN_LOG5_BASE_WIDTH_V1 == 596);
const _: () = assert!(P256_MAIN_LOG5_AUX_WIDTH_V1 == 204);

fn canonical_p256_main_log5_bindings_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<MainP256RegistrationBindingV1>, ZkX509StarkErrorV1> {
    let bindings = canonical_p256_main_layout_bindings_v1(layout)?;
    let log5 = bindings
        .into_iter()
        .filter(|binding| binding.main.segment.trace_log2 == 5)
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(P256_MAIN_LOG5_REGISTRATION_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        for local in 0..2 {
            expected.push(
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Reduction, local)
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
    }
    expected.push(
        P256MainRegistrationV1::new_v1(
            P256_SIGNATURE_COUNT_V1 - 1,
            P256MainAdapterV1::WalletLowS,
            0,
        )
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
    );
    let group = log5
        .first()
        .map(|binding| binding.main.trace_group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_layout = layout
        .trace_groups
        .get(group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if log5.len() != P256_MAIN_LOG5_REGISTRATION_COUNT_V1
        || log5
            .iter()
            .map(|binding| binding.p256)
            .ne(expected.iter().copied())
        || log5.iter().any(|binding| binding.main.trace_group != group)
        || group_layout.native_trace_log2 != 5
        || group_layout.base_width != P256_MAIN_LOG5_BASE_WIDTH_V1
        || group_layout.aux_width != P256_MAIN_LOG5_AUX_WIDTH_V1
        || group_layout.column_chunks != P256_MAIN_LOG5_REGISTRATION_COUNT_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(log5)
}

const P256_MAIN_LOG16_REGISTRATION_COUNT_V1: usize = 10;
const P256_MAIN_LOG16_BASE_WIDTH_V1: usize =
    P256_SIGNATURE_COUNT_V1 * (P256_WINDOW_BASE_WIDTH_V1 + P256_BINDING_SINK_BASE_WIDTH_V1);
const P256_MAIN_LOG16_AUX_WIDTH_V1: usize = P256_SIGNATURE_COUNT_V1
    * (P256_WINDOW_AGGREGATE_AUX_WIDTH_V1
        + super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1);
const P256_MAIN_LOG16_PHYSICAL_CHUNKS_V1: usize = 10;
const P256_MAIN_LOG16_NEXT_STRIDE_V1: usize = 512;
const _: () = assert!(P256_MAIN_LOG16_BASE_WIDTH_V1 == 430);
const _: () = assert!(P256_MAIN_LOG16_AUX_WIDTH_V1 == 375);
const _: () = assert!(P256_MAIN_LOG16_PHYSICAL_CHUNKS_V1 == 10);
const _: () = assert!(
    P256_MAIN_LOG16_NEXT_STRIDE_V1
        == 1 << (ZK_X509_MAIN_COMMON_LDE_LOG2_V1 - P256_WINDOW_AGGREGATE_TRACE_LOG2_V1)
);

/// The sole native-log-sixteen P-256 registration order: all five vertical
/// window batches followed by all five external-binding sinks.
///
/// MAIN sorts equal-log registrations by adapter identity and global instance.
/// Reconstructing the expected central identities here prevents either a
/// caller-selected topology or an accidental interleaving of the two AIRs.
fn canonical_p256_main_log16_bindings_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<MainP256RegistrationBindingV1>, ZkX509StarkErrorV1> {
    let bindings = canonical_p256_main_layout_bindings_v1(layout)?;
    let log16 = bindings
        .into_iter()
        .filter(|binding| binding.main.segment.trace_log2 == 16)
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(P256_MAIN_LOG16_REGISTRATION_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        expected.push(
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::WindowBatch, 0)
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
        );
    }
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        expected.push(
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::BindingSink, 0)
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
        );
    }
    let group = log16
        .first()
        .map(|binding| binding.main.trace_group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_layout = layout
        .trace_groups
        .get(group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if log16.len() != P256_MAIN_LOG16_REGISTRATION_COUNT_V1
        || log16
            .iter()
            .map(|binding| binding.p256)
            .ne(expected.iter().copied())
        || log16
            .iter()
            .any(|binding| binding.main.trace_group != group)
        || log16.iter().any(|binding| {
            !matches!(
                (
                    binding.main.segment.adapter,
                    p256_instance_parts_v1(binding.main.segment.instance),
                    binding.p256.adapter_v1(),
                    binding.p256.local_instance_v1(),
                ),
                (
                    SegmentAdapterIdV1::P256Window,
                    Some((_, 0)),
                    P256MainAdapterV1::WindowBatch,
                    0,
                ) | (
                    SegmentAdapterIdV1::P256ValueBus,
                    Some((_, 2)),
                    P256MainAdapterV1::BindingSink,
                    0,
                )
            )
        })
        || group_layout.native_trace_log2 != P256_WINDOW_AGGREGATE_TRACE_LOG2_V1
        || P256_WINDOW_AGGREGATE_TRACE_LOG2_V1 != P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1
        || group_layout.base_width != P256_MAIN_LOG16_BASE_WIDTH_V1
        || group_layout.aux_width != P256_MAIN_LOG16_AUX_WIDTH_V1
        || group_layout.column_chunks != P256_MAIN_LOG16_PHYSICAL_CHUNKS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(log16)
}

const P256_MAIN_LOG19_REGISTRATION_COUNT_V1: usize = 15;
const MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1: usize = 6;
const MAIN_LOG19_REGISTRATION_COUNT_V1: usize = 21;
const MAIN_LOG19_BASE_WIDTH_V1: usize = 1_940;
const MAIN_LOG19_AUX_WIDTH_V1: usize = 1_772;
const MAIN_LOG19_PHYSICAL_CHUNKS_V1: usize = 52;
const P256_MAIN_LOG19_BASE_START_V1: usize = 545;
const P256_MAIN_LOG19_AUX_START_V1: usize = 772;
const P256_MAIN_LOG19_BASE_WIDTH_V1: usize = P256_SIGNATURE_COUNT_V1
    * (P256_ARITHMETIC_BASE_WIDTH_V1 + 2 * P256_VALUE_BUS_STARK_BASE_WIDTH_V1);
const P256_MAIN_LOG19_AUX_WIDTH_V1: usize = P256_SIGNATURE_COUNT_V1
    * (P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1
        + P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1
        + P256_VALUE_BUS_STARK_AUX_WIDTH_V1);
const P256_MAIN_LOG19_PHYSICAL_CHUNKS_V1: usize = 35;
const P256_MAIN_LOG19_NEXT_STRIDE_V1: usize = 64;
const P256_MAIN_LOG19_BASE_STARTS_V1: [usize; P256_MAIN_LOG19_REGISTRATION_COUNT_V1] = [
    545, 756, 967, 1_178, 1_389, 1_600, 1_634, 1_668, 1_702, 1_736, 1_770, 1_804, 1_838, 1_872,
    1_906,
];
const P256_MAIN_LOG19_AUX_STARTS_V1: [usize; P256_MAIN_LOG19_REGISTRATION_COUNT_V1] = [
    772, 844, 916, 988, 1_060, 1_132, 1_248, 1_260, 1_376, 1_388, 1_504, 1_516, 1_632, 1_644, 1_760,
];
const MAIN_LOG19_BASE_STARTS_V1: [usize; MAIN_LOG19_REGISTRATION_COUNT_V1] = [
    0, 76, 189, 278, 367, 456, 545, 756, 967, 1_178, 1_389, 1_600, 1_634, 1_668, 1_702, 1_736,
    1_770, 1_804, 1_838, 1_872, 1_906,
];
const MAIN_LOG19_AUX_STARTS_V1: [usize; MAIN_LOG19_REGISTRATION_COUNT_V1] = [
    0, 196, 460, 538, 616, 694, 772, 844, 916, 988, 1_060, 1_132, 1_248, 1_260, 1_376, 1_388,
    1_504, 1_516, 1_632, 1_644, 1_760,
];
const MAIN_LOG19_NON_P256_KEYS_V1: [(SegmentAdapterIdV1, u16);
    MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1] = [
    (SegmentAdapterIdV1::StrictDer, 0),
    (SegmentAdapterIdV1::Rfc5280, 0),
    (SegmentAdapterIdV1::Sha256CallBus, 0),
    (SegmentAdapterIdV1::Sha256CallBus, 1),
    (SegmentAdapterIdV1::Sha256CallBus, 2),
    (SegmentAdapterIdV1::Sha256CallBus, 3),
];
const _: () = assert!(P256_MAIN_LOG19_BASE_WIDTH_V1 == 1_395);
const _: () = assert!(P256_MAIN_LOG19_AUX_WIDTH_V1 == 1_000);
const _: () = assert!(
    MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1 + P256_MAIN_LOG19_REGISTRATION_COUNT_V1
        == MAIN_LOG19_REGISTRATION_COUNT_V1
);
const _: () = assert!(P256_MAIN_LOG19_BASE_START_V1 + P256_MAIN_LOG19_BASE_WIDTH_V1 == 1_940);
const _: () = assert!(P256_MAIN_LOG19_AUX_START_V1 + P256_MAIN_LOG19_AUX_WIDTH_V1 == 1_772);
const _: () = assert!(
    P256_MAIN_LOG19_NEXT_STRIDE_V1
        == 1 << (ZK_X509_MAIN_COMMON_LDE_LOG2_V1 - ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
);

/// Exact mixed log19 registration, including the six non-P-256 owners.
///
/// This validation is deliberately stronger than selecting every segment
/// whose logarithm happens to be 19. It fixes the registration identities,
/// offsets, group geometry, and physical chunk count before a production
/// verifier source can exist.
fn canonical_main_log19_registrations_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<RegisteredSegmentLayoutV1>, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    let registrations = layout
        .registered_segments
        .iter()
        .copied()
        .filter(|registration| registration.segment.trace_log2 == ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
        .collect::<Vec<_>>();
    let group = registrations
        .first()
        .map(|registration| registration.trace_group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_layout = layout
        .trace_groups
        .get(group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let non_p256_chunks = registrations
        .iter()
        .take(MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1)
        .try_fold(0_usize, |sum, registration| {
            sum.checked_add(registration.column_chunks)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
        })?;
    if registrations.len() != MAIN_LOG19_REGISTRATION_COUNT_V1
        || registrations
            .iter()
            .any(|registration| registration.trace_group != group)
        || registrations
            .iter()
            .map(|registration| registration.base_start)
            .ne(MAIN_LOG19_BASE_STARTS_V1)
        || registrations
            .iter()
            .map(|registration| registration.aux_start)
            .ne(MAIN_LOG19_AUX_STARTS_V1)
        || registrations
            .iter()
            .take(MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1)
            .map(|registration| (registration.segment.adapter, registration.segment.instance))
            .ne(MAIN_LOG19_NON_P256_KEYS_V1)
        || non_p256_chunks != MAIN_LOG19_PHYSICAL_CHUNKS_V1 - P256_MAIN_LOG19_PHYSICAL_CHUNKS_V1
        || group_layout.native_trace_log2 != ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
        || group_layout.base_width != MAIN_LOG19_BASE_WIDTH_V1
        || group_layout.aux_width != MAIN_LOG19_AUX_WIDTH_V1
        || group_layout.column_chunks != MAIN_LOG19_PHYSICAL_CHUNKS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    canonical_p256_main_log19_bindings_v1(layout)?;
    Ok(registrations)
}

/// Exact P-256 subset of MAIN's mixed native-log19 group.
///
/// The six DER/RFC/SHA registrations occupy the fixed prefix. P-256 then
/// appears as all five arithmetic registrations followed by execution/sorted
/// value-bus pairs in global signature order.
fn canonical_p256_main_log19_bindings_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<MainP256RegistrationBindingV1>, ZkX509StarkErrorV1> {
    let bindings = canonical_p256_main_layout_bindings_v1(layout)?;
    let log19 = bindings
        .into_iter()
        .filter(|binding| binding.main.segment.trace_log2 == ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(P256_MAIN_LOG19_REGISTRATION_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        expected.push(
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0)
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
        );
    }
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        for local in 0..2 {
            expected.push(
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::ValueBus, local)
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
    }
    let group = log19
        .first()
        .map(|binding| binding.main.trace_group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_layout = layout
        .trace_groups
        .get(group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_registration_count = layout
        .registered_segments
        .iter()
        .filter(|registration| registration.trace_group == group)
        .count();
    let p256_chunks = log19.iter().try_fold(0_usize, |sum, binding| {
        sum.checked_add(binding.main.column_chunks)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    })?;
    if log19.len() != P256_MAIN_LOG19_REGISTRATION_COUNT_V1
        || log19
            .iter()
            .map(|binding| binding.p256)
            .ne(expected.iter().copied())
        || log19
            .iter()
            .any(|binding| binding.main.trace_group != group)
        || log19
            .iter()
            .map(|binding| binding.main.base_start)
            .ne(P256_MAIN_LOG19_BASE_STARTS_V1)
        || log19
            .iter()
            .map(|binding| binding.main.aux_start)
            .ne(P256_MAIN_LOG19_AUX_STARTS_V1)
        || group_registration_count != MAIN_LOG19_REGISTRATION_COUNT_V1
        || group_layout.native_trace_log2 != ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
        || group_layout.base_width != MAIN_LOG19_BASE_WIDTH_V1
        || group_layout.aux_width != MAIN_LOG19_AUX_WIDTH_V1
        || group_layout.column_chunks != MAIN_LOG19_PHYSICAL_CHUNKS_V1
        || p256_chunks != P256_MAIN_LOG19_PHYSICAL_CHUNKS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(log19)
}

fn p256_aggregate_challenges_from_post_base_v1(
    post_base: ZkX509CredentialMainPostBaseChallengesV1,
) -> Result<P256AggregateChallengesV1, ZkX509StarkErrorV1> {
    let challenges = P256AggregateChallengesV1 {
        value: post_base.p256_value(),
        cross: post_base.p256_cross(),
        scalar: post_base.p256_scalar(),
        arithmetic_copy: post_base.p256_arithmetic_copy(),
    };
    challenges.validate()?;
    Ok(challenges)
}

#[derive(Clone, Copy)]
enum MainP256Log5TracePhaseV1<'a> {
    Base(&'a P256MainBaseSourceV1),
    Bound(&'a P256MainBoundSourceV1),
}

/// Borrowed trace replay for the ten reductions and wallet low-S adapter.
///
/// The central five-signature source remains uniquely owned by the caller and
/// can therefore be reused by the other native-log P-256 views without
/// recompiling or duplicating any private trace material.
struct MainP256Log5TraceGroupSourceV1<'a> {
    bindings: Vec<MainP256RegistrationBindingV1>,
    phase: MainP256Log5TracePhaseV1<'a>,
}

impl<'a> MainP256Log5TraceGroupSourceV1<'a> {
    fn for_base_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBaseSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        Ok(Self {
            bindings: canonical_p256_main_log5_bindings_v1(layout)?,
            phase: MainP256Log5TracePhaseV1::Base(source),
        })
    }

    fn for_bound_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        source.post_base_v1()?;
        Ok(Self {
            bindings: canonical_p256_main_log5_bindings_v1(layout)?,
            phase: MainP256Log5TracePhaseV1::Bound(source),
        })
    }

    fn binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }
}

impl MainTraceGroupSourceV1 for MainP256Log5TraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        match self.phase {
            MainP256Log5TracePhaseV1::Base(source) => {
                source.fill_base_column_v1(binding.p256, local_column, &mut output)?
            }
            MainP256Log5TracePhaseV1::Bound(source) => {
                source.fill_base_column_v1(binding.p256, local_column, &mut output)?
            }
        }
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if local_column >= registration.segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let source = match self.phase {
            MainP256Log5TracePhaseV1::Base(_) => {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
            MainP256Log5TracePhaseV1::Bound(source) => source,
        };
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        source.fill_aux_column_v1(binding.p256, local_column, &mut output)?;
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }
}

fn zeroize_p256_terminal_registration_v1(registration: &mut P256TerminalRegistrationV1) {
    registration.buses.value_execution.fill(F::ZERO);
    registration.buses.value_sorted.fill(F::ZERO);
    registration.buses.value_arithmetic_copy.fill(F::ZERO);
    registration.buses.arithmetic_value_copy.fill(F::ZERO);
    registration.buses.arithmetic_scalar.fill(F::ZERO);
    registration.buses.window_scalar.fill(F::ZERO);
    registration.buses.scalar_bus_arithmetic.fill(F::ZERO);
    registration.buses.scalar_bus_window.fill(F::ZERO);
    for claim in &mut registration.cross_sources {
        claim.start.fill(F::ZERO);
        claim.terminal.fill(F::ZERO);
    }
    registration.cross_sources.clear();
    registration.sink.fill(F::ZERO);
}

/// Fixed-polynomial streaming and composition evaluation for the log-five
/// prover, borrowing the one already-bound central P-256 capability.
struct MainP256Log5ProverConstraintSourceV1<'a> {
    source: &'a P256MainBoundSourceV1,
    bindings: Vec<MainP256RegistrationBindingV1>,
    challenges: P256AggregateChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
}

impl<'a> MainP256Log5ProverConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        Ok(Self {
            source,
            bindings: canonical_p256_main_log5_bindings_v1(layout)?,
            challenges: p256_aggregate_challenges_from_post_base_v1(source.post_base_v1()?)?,
            terminals: main_p256_terminal_registrations_v1(&source.terminal_claims_v1()?)?,
        })
    }

    fn binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(
            RegisteredSegmentLayoutV1,
            usize,
            &[F],
        ) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        for binding in self.bindings.iter().copied() {
            let trace_root = goldilocks_primitive_root_v1(binding.main.segment.trace_log2)
                .map_err(map_transparent_error_v1)?;
            for local_column in 0..binding.main.segment.fixed_width {
                let mut coefficients =
                    zeroed_main_trace_column_v1(binding.main.segment.trace_size())?;
                self.source
                    .fill_fixed_column_v1(binding.p256, local_column, &mut coefficients)?;
                goldilocks_ifft_v1(&mut coefficients, trace_root)
                    .map_err(map_transparent_error_v1)?;
                consume(binding.main, local_column, &coefficients)?;
            }
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F],
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if fixed.len() != registration.segment.fixed_width
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .chain(fixed)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        p256_opened_residues_v1(
            registration,
            opening,
            fixed,
            self.challenges,
            self.terminals
                .get(binding.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }

    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if F::canonical(x.0).is_none() || alphas.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let residues = self.constraint_residues_v1(registration, opening, fixed)?;
        accumulator_quotient_value_v1(registration.segment, x, &residues, alphas)
    }
}

/// Witness-free fixed preprocessing and opened-row evaluation for the eleven
/// reductions in MAIN's native log-five group.
///
/// One central verifier-fixed source is shared across all P-256 groups. This
/// view owns only its bounded, registration-local opening caches.
struct MainP256Log5VerifierConstraintSourceV1<'a> {
    bindings: Vec<MainP256RegistrationBindingV1>,
    common_lde_log2: u8,
    challenges: P256AggregateChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
    fixed: &'a P256MainVerifierFixedSourceV1,
    fixed_openings: Vec<BTreeMap<usize, Vec<F>>>,
}

impl<'a> MainP256Log5VerifierConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        fixed: &'a P256MainVerifierFixedSourceV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
        claims: ZkX509P256TerminalClaimsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let bindings = canonical_p256_main_log5_bindings_v1(layout)?;
        if bindings
            .iter()
            .any(|binding| layout.common_lde_log2 < binding.main.segment.lde_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut fixed_openings = Vec::new();
        fixed_openings
            .try_reserve_exact(P256_MAIN_LOG5_REGISTRATION_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        fixed_openings.resize_with(P256_MAIN_LOG5_REGISTRATION_COUNT_V1, BTreeMap::new);
        Ok(Self {
            bindings,
            common_lde_log2: layout.common_lde_log2,
            challenges: p256_aggregate_challenges_from_post_base_v1(post_base)?,
            terminals: main_p256_terminal_registrations_v1(&claims)?,
            fixed,
            fixed_openings,
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn binding_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.main == registration)
            .map(|(index, _)| index);
        let index = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(index)
    }

    fn next_query_index_v1(
        &self,
        registration_index: usize,
        query_index: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let registration = self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .main;
        let stride_log2 = self
            .common_lde_log2
            .checked_sub(registration.segment.trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let stride = 1_usize
            .checked_shl(u32::from(stride_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(query_index
            .checked_add(stride)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1()?)
    }

    fn ensure_fixed_openings_v1(
        &mut self,
        registration_index: usize,
        indices: [usize; 2],
    ) -> Result<(), ZkX509StarkErrorV1> {
        let binding = *self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let cache = self
            .fixed_openings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let mut missing = indices
            .into_iter()
            .filter(|index| !cache.contains_key(index))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        if cache
            .len()
            .checked_add(missing.len())
            .filter(|count| *count <= VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .is_none()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        // Sample into a temporary map. No rejected schedule or resource
        // request can partially consume the bounded cache.
        let sampled = match binding.p256.adapter_v1() {
            P256MainAdapterV1::Reduction => sampled_verifier_generated_fixed_openings_v1::<
                P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
            >(
                binding.main.segment,
                self.common_lde_log2,
                &missing,
                |row| {
                    self.fixed
                        .fixed_row_v1(binding.p256, row)?
                        .try_into()
                        .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)
                },
            )?,
            P256MainAdapterV1::WalletLowS => {
                sampled_verifier_generated_fixed_openings_v1::<P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1>(
                    binding.main.segment,
                    self.common_lde_log2,
                    &missing,
                    |row| {
                        self.fixed
                            .fixed_row_v1(binding.p256, row)?
                            .try_into()
                            .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)
                    },
                )?
            }
            _ => return Err(ZkX509StarkErrorV1::InternalInvariant),
        };
        let cache = self
            .fixed_openings
            .get_mut(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if sampled.keys().any(|index| cache.contains_key(index)) {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        cache.extend(sampled);
        Ok(())
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = self.binding_index_v1(registration)?;
        let binding = *self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let common_lde_size = self.common_lde_size_v1()?;
        if query_index >= common_lde_size
            || next_query_index >= common_lde_size
            || next_query_index != self.next_query_index_v1(registration_index, query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || F::canonical(x.0).is_none()
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        self.ensure_fixed_openings_v1(registration_index, [query_index, next_query_index])?;
        let cache = self
            .fixed_openings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let current = cache
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let next = cache
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if current.len() != registration.segment.fixed_width
            || next.len() != registration.segment.fixed_width
            || current
                .iter()
                .chain(next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        let terminals = self
            .terminals
            .get(binding.p256.signature_v1())
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        p256_opened_residues_v1(registration, opening, current, self.challenges, terminals)
    }

    #[cfg(test)]
    fn cached_openings_v1(&self, registration_index: usize) -> Option<usize> {
        self.fixed_openings
            .get(registration_index)
            .map(BTreeMap::len)
    }
}

#[derive(Clone, Copy)]
enum MainP256Log16TracePhaseV1<'a> {
    Base(&'a P256MainBaseSourceV1),
    Bound(&'a P256MainBoundSourceV1),
}

/// Borrowed trace replay for MAIN's five window batches and five binding
/// sinks. The central P-256 source remains the sole owner of private rows.
struct MainP256Log16TraceGroupSourceV1<'a> {
    bindings: Vec<MainP256RegistrationBindingV1>,
    phase: MainP256Log16TracePhaseV1<'a>,
}

impl<'a> MainP256Log16TraceGroupSourceV1<'a> {
    fn for_base_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBaseSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        Ok(Self {
            bindings: canonical_p256_main_log16_bindings_v1(layout)?,
            phase: MainP256Log16TracePhaseV1::Base(source),
        })
    }

    fn for_bound_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        source.post_base_v1()?;
        Ok(Self {
            bindings: canonical_p256_main_log16_bindings_v1(layout)?,
            phase: MainP256Log16TracePhaseV1::Bound(source),
        })
    }

    fn binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }
}

impl MainTraceGroupSourceV1 for MainP256Log16TraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        match self.phase {
            MainP256Log16TracePhaseV1::Base(source) => {
                source.fill_base_column_v1(binding.p256, local_column, &mut output)?
            }
            MainP256Log16TracePhaseV1::Bound(source) => {
                source.fill_base_column_v1(binding.p256, local_column, &mut output)?
            }
        }
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if local_column >= registration.segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let MainP256Log16TracePhaseV1::Bound(source) = self.phase else {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        };
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        source.fill_aux_column_v1(binding.p256, local_column, &mut output)?;
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }
}

/// Fixed-polynomial streaming and opened-row composition for the bound
/// log-sixteen prover.
struct MainP256Log16ProverConstraintSourceV1<'a> {
    source: &'a P256MainBoundSourceV1,
    bindings: Vec<MainP256RegistrationBindingV1>,
    challenges: P256AggregateChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
}

impl<'a> MainP256Log16ProverConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_p256_main_registration_order_v1(&source.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        Ok(Self {
            source,
            bindings: canonical_p256_main_log16_bindings_v1(layout)?,
            challenges: p256_aggregate_challenges_from_post_base_v1(source.post_base_v1()?)?,
            terminals: main_p256_terminal_registrations_v1(&source.terminal_claims_v1()?)?,
        })
    }

    fn binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(
            RegisteredSegmentLayoutV1,
            usize,
            &[F],
        ) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        for binding in self.bindings.iter().copied() {
            let trace_root = goldilocks_primitive_root_v1(binding.main.segment.trace_log2)
                .map_err(map_transparent_error_v1)?;
            for local_column in 0..binding.main.segment.fixed_width {
                let mut coefficients =
                    zeroed_main_trace_column_v1(binding.main.segment.trace_size())?;
                self.source
                    .fill_fixed_column_v1(binding.p256, local_column, &mut coefficients)?;
                goldilocks_ifft_v1(&mut coefficients, trace_root)
                    .map_err(map_transparent_error_v1)?;
                consume(binding.main, local_column, &coefficients)?;
            }
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F],
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        if fixed.len() != registration.segment.fixed_width
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .chain(fixed)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        p256_opened_residues_v1(
            registration,
            opening,
            fixed,
            self.challenges,
            self.terminals
                .get(binding.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }

    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if F::canonical(x.0).is_none() || alphas.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let residues = self.constraint_residues_v1(registration, opening, fixed)?;
        accumulator_quotient_value_v1(registration.segment, x, &residues, alphas)
    }
}

/// Witness-free fixed sampler and opened-row evaluator for all ten canonical
/// log-sixteen registrations.
///
/// Each registration owns an independent bounded cache. Missing rows are
/// sampled into a temporary map and committed only after every generated
/// opening succeeds.
struct MainP256Log16VerifierConstraintSourceV1<'a> {
    bindings: Vec<MainP256RegistrationBindingV1>,
    common_lde_log2: u8,
    fixed: &'a P256MainVerifierFixedSourceV1,
    challenges: P256AggregateChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
    fixed_openings: Vec<BTreeMap<usize, Vec<F>>>,
}

impl<'a> MainP256Log16VerifierConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        fixed: &'a P256MainVerifierFixedSourceV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
        claims: &ZkX509P256TerminalClaimsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let bindings = canonical_p256_main_log16_bindings_v1(layout)?;
        if bindings.iter().any(|binding| {
            layout.common_lde_log2 < binding.main.segment.lde_log2
                || binding.main.segment.trace_log2 != P256_WINDOW_AGGREGATE_TRACE_LOG2_V1
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut fixed_openings = Vec::new();
        fixed_openings
            .try_reserve_exact(P256_MAIN_LOG16_REGISTRATION_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        fixed_openings.resize_with(P256_MAIN_LOG16_REGISTRATION_COUNT_V1, BTreeMap::new);
        Ok(Self {
            bindings,
            common_lde_log2: layout.common_lde_log2,
            fixed,
            challenges: p256_aggregate_challenges_from_post_base_v1(post_base)?,
            terminals: main_p256_terminal_registrations_v1(claims)?,
            fixed_openings,
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn binding_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.main == registration)
            .map(|(index, _)| index);
        let index = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(index)
    }

    fn next_query_index_v1(
        &self,
        registration_index: usize,
        query_index: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let registration = self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .main;
        let stride_log2 = self
            .common_lde_log2
            .checked_sub(registration.segment.trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let stride = 1_usize
            .checked_shl(u32::from(stride_log2))
            .filter(|stride| *stride == P256_MAIN_LOG16_NEXT_STRIDE_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(query_index
            .checked_add(stride)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1()?)
    }

    fn ensure_fixed_openings_v1(
        &mut self,
        registration_index: usize,
        indices: [usize; 2],
    ) -> Result<(), ZkX509StarkErrorV1> {
        let binding = *self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let cache = self
            .fixed_openings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let mut missing = indices
            .into_iter()
            .filter(|index| !cache.contains_key(index))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        if cache
            .len()
            .checked_add(missing.len())
            .filter(|count| *count <= VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .is_none()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        let sampled = match binding.p256.adapter_v1() {
            P256MainAdapterV1::WindowBatch => {
                sampled_verifier_generated_fixed_openings_v1::<P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1>(
                    binding.main.segment,
                    self.common_lde_log2,
                    &missing,
                    |row| {
                        self.fixed
                            .fixed_row_v1(binding.p256, row)?
                            .try_into()
                            .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)
                    },
                )?
            }
            P256MainAdapterV1::BindingSink => {
                sampled_verifier_generated_fixed_openings_v1::<P256_BINDING_SINK_FIXED_WIDTH_V1>(
                    binding.main.segment,
                    self.common_lde_log2,
                    &missing,
                    |row| {
                        self.fixed
                            .fixed_row_v1(binding.p256, row)?
                            .try_into()
                            .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)
                    },
                )?
            }
            _ => return Err(ZkX509StarkErrorV1::InternalInvariant),
        };
        let cache = self
            .fixed_openings
            .get_mut(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if sampled.keys().any(|index| cache.contains_key(index)) {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        cache.extend(sampled);
        Ok(())
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = self.binding_index_v1(registration)?;
        let binding = *self
            .bindings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let common_lde_size = self.common_lde_size_v1()?;
        if query_index >= common_lde_size
            || next_query_index >= common_lde_size
            || next_query_index != self.next_query_index_v1(registration_index, query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || F::canonical(x.0).is_none()
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        self.ensure_fixed_openings_v1(registration_index, [query_index, next_query_index])?;
        let cache = self
            .fixed_openings
            .get(registration_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let current = cache
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let next = cache
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if current.len() != registration.segment.fixed_width
            || next.len() != registration.segment.fixed_width
            || current
                .iter()
                .chain(next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        p256_opened_residues_v1(
            registration,
            opening,
            current,
            self.challenges,
            self.terminals
                .get(binding.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }

    #[cfg(test)]
    fn cached_openings_v1(&self, registration_index: usize) -> Option<usize> {
        self.fixed_openings
            .get(registration_index)
            .map(BTreeMap::len)
    }
}

fn map_main_sha_source_error_v1(
    error: super::sha_call_bus_stark::ZkX509ShaCallBusStarkErrorV1,
) -> ZkX509StarkErrorV1 {
    use super::sha_call_bus_stark::ZkX509ShaCallBusStarkErrorV1 as Error;
    match error {
        Error::Resource => ZkX509StarkErrorV1::AllocationFailure,
        Error::Phase => ZkX509StarkErrorV1::TranscriptMismatch,
        Error::Challenge => ZkX509StarkErrorV1::TranscriptMismatch,
        Error::Topology
        | Error::LengthOrPadding
        | Error::InactiveCall
        | Error::Digest
        | Error::Event
        | Error::Terminal => ZkX509StarkErrorV1::AccumulatorWitness,
    }
}

fn map_main_rfc_source_error_v1(
    error: super::rfc5280_stark::ZkX509Rfc5280StarkErrorV1,
) -> ZkX509StarkErrorV1 {
    use super::rfc5280_stark::ZkX509Rfc5280StarkErrorV1 as Error;
    match error {
        Error::Resource => ZkX509StarkErrorV1::AllocationFailure,
        Error::Challenge => ZkX509StarkErrorV1::TranscriptMismatch,
        Error::Shape
        | Error::Grammar
        | Error::Semantic
        | Error::Source
        | Error::Output
        | Error::TerminalClaim => ZkX509StarkErrorV1::DerWitness,
    }
}

fn main_log19_sha_base_sources_v1<'a>(
    schedule: &'a ZkX509ShaCallScheduleV1,
    witnesses: &'a [ZkX509ShaCallWitnessV1; super::sha_call_bus_stark::ZK_X509_SHA_CALL_COUNT_V1],
) -> Result<[ZkX509ShaBatchSegmentBaseSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1], ZkX509StarkErrorV1>
{
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(ZK_X509_SHA_SEGMENT_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        sources.push(
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(schedule, witnesses, segment)
                .map_err(map_main_sha_source_error_v1)?,
        );
    }
    sources
        .try_into()
        .map_err(|_: Vec<ZkX509ShaBatchSegmentBaseSourceV1<'a>>| {
            ZkX509StarkErrorV1::InternalInvariant
        })
}

/// Challenge-independent owner of the complete mixed native-log19 MAIN group.
///
/// Strict DER, RFC 5280, the four SHA registrations, and all fifteen P-256
/// registrations are routed from the verifier-owned registration.  The type
/// deliberately has no auxiliary-column implementation: a successful X5B1
/// transition consumes it and returns [`MainLog19BoundTraceGroupSourceV1`].
struct MainLog19BaseTraceGroupSourceV1<'assembly, 'source> {
    registrations: Vec<RegisteredSegmentLayoutV1>,
    p256_bindings: Vec<MainP256RegistrationBindingV1>,
    der: &'assembly ZkX509DerStarkBaseV1,
    rfc: &'assembly ZkX509Rfc5280StarkBaseMaterialV1,
    sha: &'source [ZkX509ShaBatchSegmentBaseSourceV1<'assembly>; ZK_X509_SHA_SEGMENT_COUNT_V1],
    p256: &'source P256MainBaseSourceV1,
}

impl<'assembly, 'source> MainLog19BaseTraceGroupSourceV1<'assembly, 'source> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        assembly: &'assembly ZkX509MainTraceAssemblyV1,
        sha: &'source [ZkX509ShaBatchSegmentBaseSourceV1<'assembly>; ZK_X509_SHA_SEGMENT_COUNT_V1],
        p256: &'source P256MainBaseSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_zk_x509_main_verifier_profile_v1(assembly.verifier_profile)?;
        validate_p256_main_registration_order_v1(&p256.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
        Ok(Self {
            registrations: canonical_main_log19_registrations_v1(layout)?,
            p256_bindings: canonical_p256_main_log19_bindings_v1(layout)?,
            der: &assembly.der_base,
            rfc: &assembly.rfc_base,
            sha,
            p256,
        })
    }

    fn registration_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let mut matches = self
            .registrations
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == registration)
            .map(|(index, _)| index);
        let index = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(index)
    }

    fn p256_binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .p256_bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }
}

impl MainTraceGroupSourceV1 for MainLog19BaseTraceGroupSourceV1<'_, '_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let registration_index = self.registration_index_v1(registration)?;
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let column = match registration_index {
            0 => build_zk_x509_der_stark_native_base_column_v1(self.der, local_column)
                .map_err(ZkX509StarkErrorV1::from)?,
            1 => self
                .rfc
                .build_base_column(local_column)
                .map_err(map_main_rfc_source_error_v1)?,
            2..=5 => {
                let segment = registration_index - 2;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.sha[segment]
                    .fill_base_column_v1(segment, local_column, &mut column)
                    .map_err(map_main_sha_source_error_v1)?;
                return Ok(column);
            }
            _ => {
                let binding = self.p256_binding_v1(registration)?;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.p256
                    .fill_base_column_v1(binding.p256, local_column, &mut column)?;
                return Ok(column);
            }
        };
        Ok(ZeroizingMainTraceColumnV1(column))
    }

    fn native_aux_column_v1(
        &mut self,
        _registration: RegisteredSegmentLayoutV1,
        _local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    }
}

fn zeroize_main_der_trace_v1(trace: &mut ZkX509DerStarkTraceV1) {
    trace.base.zeroize_private_v1();
    for row in &mut trace.aux_rows {
        row.fill(F::ZERO);
    }
    trace.aux_rows.clear();
}

struct ZeroizingMainDerTraceGuardV1(Option<ZkX509DerStarkTraceV1>);

impl ZeroizingMainDerTraceGuardV1 {
    fn new_v1(trace: ZkX509DerStarkTraceV1) -> Self {
        Self(Some(trace))
    }

    fn trace_v1(&self) -> Result<&ZkX509DerStarkTraceV1, ZkX509StarkErrorV1> {
        self.0.as_ref().ok_or(ZkX509StarkErrorV1::InternalInvariant)
    }

    fn take_v1(&mut self) -> Result<ZkX509DerStarkTraceV1, ZkX509StarkErrorV1> {
        self.0.take().ok_or(ZkX509StarkErrorV1::InternalInvariant)
    }
}

impl Drop for ZeroizingMainDerTraceGuardV1 {
    fn drop(&mut self) {
        if let Some(trace) = self.0.as_mut() {
            zeroize_main_der_trace_v1(trace);
        }
        self.0 = None;
    }
}

/// Challenge-bound owner of the complete mixed native-log19 MAIN group.
///
/// Construction consumes the pre-X5B1 owner and requires both the outer
/// credential binding and the P-256 capability bound by that same token.
struct MainLog19BoundTraceGroupSourceV1<'a> {
    registrations: Vec<RegisteredSegmentLayoutV1>,
    p256_bindings: Vec<MainP256RegistrationBindingV1>,
    der: ZkX509DerStarkTraceV1,
    der_fixed: ZkX509DerStarkFixedScheduleV1,
    rfc: ZkX509Rfc5280StarkColumnProviderV1<'a>,
    sha_base: [ZkX509ShaBatchSegmentBaseSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1],
    sha_aux: [ZkX509ShaBatchSegmentAuxSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1],
    sha_fixed: ZkX509ShaBatchFixedProviderV1,
    p256: P256MainBoundSourceV1,
    post_base: ZkX509CredentialMainPostBaseChallengesV1,
    claims: ZkX509MainTerminalClaimsV1,
}

impl<'a> MainLog19BoundTraceGroupSourceV1<'a> {
    /// Consume every challenge-independent log19 child exactly once under the
    /// credential-derived X5B1 binding.
    ///
    /// This is deliberately the only transition into the bound mixed group.
    /// In particular, callers cannot provide a separately bound P-256 source:
    /// P-256 and all four SHA segments are transitioned here from the same
    /// opaque credential capability.
    fn bind_from_phase_v1(
        layout: &AggregateProofLayoutV1,
        assembly: &'a ZkX509MainTraceAssemblyV1,
        mut sha: [ZkX509ShaBatchSegmentBaseSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1],
        mut p256: P256MainBaseSourceV1,
        binding: ZkX509CredentialPreAuxBindingV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        validate_zk_x509_main_verifier_profile_v1(assembly.verifier_profile)?;
        let registrations = canonical_main_log19_registrations_v1(layout)?;
        let p256_bindings = canonical_p256_main_log19_bindings_v1(layout)?;
        let post_base = binding.main_post_base();
        let p256 = p256.bind_v1(post_base)?;
        if p256.post_base_v1()? != post_base {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }

        let mut sha_aux = Vec::new();
        sha_aux
            .try_reserve_exact(ZK_X509_SHA_SEGMENT_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for source in &mut sha {
            sha_aux.push(
                source
                    .bind_v1(binding)
                    .map_err(map_main_sha_source_error_v1)?,
            );
        }
        let sha_aux: [ZkX509ShaBatchSegmentAuxSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1] = sha_aux
            .try_into()
            .map_err(|_: Vec<ZkX509ShaBatchSegmentAuxSourceV1<'a>>| {
                ZkX509StarkErrorV1::InternalInvariant
            })?;

        let mut der = ZeroizingMainDerTraceGuardV1::new_v1(
            build_zk_x509_der_stark_trace_v1(assembly.der_base.clone(), post_base.der())
                .map_err(ZkX509StarkErrorV1::from)?,
        );
        let rfc = ZkX509Rfc5280StarkColumnProviderV1::new_v1(
            &assembly.rfc_base,
            post_base.der(),
            post_base.rfc5280(),
        )
        .map_err(map_main_rfc_source_error_v1)?;
        let mut sha_segments = Vec::new();
        let mut ca_calls = Vec::new();
        sha_segments
            .try_reserve_exact(ZK_X509_SHA_SEGMENT_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        ca_calls
            .try_reserve_exact(ZK_X509_SHA_CA_CALL_COUNT_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (segment, source) in sha_aux.iter().enumerate() {
            let mut column = zeroed_main_trace_column_v1(ZK_X509_DER_STARK_TRACE_SIZE_V1)?;
            let terminals = source
                .fill_aux_column_with_air_terminals_v1(segment, 0, &mut column)
                .map_err(map_main_sha_source_error_v1)?;
            sha_segments.push(terminals.segment);
            ca_calls.extend(terminals.ca_call_boundaries);
        }
        let sha_segments: [ZkX509ShaSegmentTerminalV1; ZK_X509_SHA_SEGMENT_COUNT_V1] = sha_segments
            .try_into()
            .map_err(|_: Vec<ZkX509ShaSegmentTerminalV1>| ZkX509StarkErrorV1::InternalInvariant)?;
        let ca_calls: [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1] = ca_calls
            .try_into()
            .map_err(|_: Vec<ZkX509ShaCallBoundaryTerminalV1>| {
                ZkX509StarkErrorV1::InternalInvariant
            })?;
        let claims = ZkX509MainTerminalClaimsV1 {
            der: zk_x509_der_stark_terminal_claims_v1(der.trace_v1()?)
                .map_err(ZkX509StarkErrorV1::from)?,
            rfc5280: rfc.terminal_claims_v1(),
            sha: ZkX509ShaSegmentTerminalClaimsV1::from_sha_air_terminals_v1(
                sha_segments,
                ca_calls,
            )
            .map_err(map_main_rfc_source_error_v1)?,
            p256: p256.terminal_claims_v1()?,
        };
        validate_zk_x509_der_rfc_terminal_equalities_v1(claims.der, claims.rfc5280)
            .map_err(map_main_rfc_source_error_v1)?;
        if !zk_x509_main_rfc_sha_terminal_products_match_v1(claims.rfc5280, claims.sha) {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        let der_fixed = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .map_err(ZkX509StarkErrorV1::from)?;
        let sha_base =
            main_log19_sha_base_sources_v1(&assembly.sha_schedule, &assembly.sha_witnesses)?;
        let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(assembly.sha_schedule.shape())
            .map_err(map_main_sha_source_error_v1)?;
        let der = der.take_v1()?;

        Ok(Self {
            registrations,
            p256_bindings,
            der,
            der_fixed,
            rfc,
            sha_base,
            sha_aux,
            sha_fixed,
            p256,
            post_base,
            claims,
        })
    }

    fn registration_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let mut matches = self
            .registrations
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == registration)
            .map(|(index, _)| index);
        let index = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(index)
    }

    fn p256_binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .p256_bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }

    const fn terminal_claims_v1(&self) -> ZkX509MainTerminalClaimsV1 {
        self.claims
    }

    fn zeroize_private_v1(&mut self) {
        zeroize_main_der_trace_v1(&mut self.der);
    }

    fn native_fixed_column_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let registration_index = self.registration_index_v1(registration)?;
        if local_column >= registration.segment.fixed_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let column = match registration_index {
            0 => build_zk_x509_der_stark_native_fixed_column_v1(&self.der_fixed, local_column)
                .map_err(ZkX509StarkErrorV1::from)?,
            1 => self
                .rfc
                .build_fixed_column_v1(local_column)
                .map_err(map_main_rfc_source_error_v1)?,
            2..=5 => {
                let segment = registration_index - 2;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                for (row, value) in column.iter_mut().enumerate() {
                    *value = self
                        .sha_fixed
                        .fixed_row_v1(segment, row)
                        .map_err(map_main_sha_source_error_v1)?[local_column];
                }
                return Ok(column);
            }
            _ => {
                let binding = self.p256_binding_v1(registration)?;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.p256
                    .fill_fixed_column_v1(binding.p256, local_column, &mut column)?;
                return Ok(column);
            }
        };
        Ok(ZeroizingMainTraceColumnV1(column))
    }
}

impl Drop for MainLog19BoundTraceGroupSourceV1<'_> {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}

impl MainTraceGroupSourceV1 for MainLog19BoundTraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let registration_index = self.registration_index_v1(registration)?;
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let column = match registration_index {
            0 => build_zk_x509_der_stark_native_base_column_v1(&self.der.base, local_column)
                .map_err(ZkX509StarkErrorV1::from)?,
            1 => self
                .rfc
                .build_base_column_v1(local_column)
                .map_err(map_main_rfc_source_error_v1)?,
            2..=5 => {
                let segment = registration_index - 2;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.sha_base[segment]
                    .fill_base_column_v1(segment, local_column, &mut column)
                    .map_err(map_main_sha_source_error_v1)?;
                return Ok(column);
            }
            _ => {
                let binding = self.p256_binding_v1(registration)?;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.p256
                    .fill_base_column_v1(binding.p256, local_column, &mut column)?;
                return Ok(column);
            }
        };
        Ok(ZeroizingMainTraceColumnV1(column))
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let registration_index = self.registration_index_v1(registration)?;
        if local_column >= registration.segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let column = match registration_index {
            0 => build_zk_x509_der_stark_native_aux_column_v1(&self.der, local_column)
                .map_err(ZkX509StarkErrorV1::from)?,
            1 => self
                .rfc
                .build_aux_column_v1(local_column)
                .map_err(map_main_rfc_source_error_v1)?,
            2..=5 => {
                let segment = registration_index - 2;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.sha_aux[segment]
                    .fill_aux_column_v1(segment, local_column, &mut column)
                    .map_err(map_main_sha_source_error_v1)?;
                return Ok(column);
            }
            _ => {
                let binding = self.p256_binding_v1(registration)?;
                let mut column = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
                self.p256
                    .fill_aux_column_v1(binding.p256, local_column, &mut column)?;
                return Ok(column);
            }
        };
        Ok(ZeroizingMainTraceColumnV1(column))
    }
}

/// Native fixed-polynomial and quotient owner for the complete mixed log19
/// prover group.
///
/// The source borrows the already-bound trace owner, so it is impossible to
/// evaluate a challenge-dependent residue against a pre-X5B1 trace.
struct MainLog19ProverConstraintSourceV1<'a, 'source> {
    source: &'source MainLog19BoundTraceGroupSourceV1<'a>,
    der_public: ZkX509DerStarkPublicTerminalsV1,
    p256_challenges: P256AggregateChallengesV1,
    p256_terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
}

impl<'a, 'source> MainLog19ProverConstraintSourceV1<'a, 'source> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        source: &'source MainLog19BoundTraceGroupSourceV1<'a>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if canonical_main_log19_registrations_v1(layout)? != source.registrations
            || source.p256.post_base_v1()? != source.post_base
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        Ok(Self {
            source,
            der_public: derive_zk_x509_der_stark_public_terminals_v1(
                &ZkX509DerStarkShapeV1,
                source.post_base.der(),
            )
            .map_err(ZkX509StarkErrorV1::from)?,
            p256_challenges: p256_aggregate_challenges_from_post_base_v1(source.post_base)?,
            p256_terminals: main_p256_terminal_registrations_v1(&source.claims.p256)?,
        })
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(
            RegisteredSegmentLayoutV1,
            usize,
            &[F],
        ) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        for registration in self.source.registrations.iter().copied() {
            let trace_root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
                .map_err(map_transparent_error_v1)?;
            for local_column in 0..registration.segment.fixed_width {
                let mut coefficients = self
                    .source
                    .native_fixed_column_v1(registration, local_column)?;
                goldilocks_ifft_v1(&mut coefficients, trace_root)
                    .map_err(map_transparent_error_v1)?;
                consume(registration, local_column, &coefficients)?;
            }
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed_current: &[F],
        fixed_next: &[F],
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = self.source.registration_index_v1(registration)?;
        if fixed_current.len() != registration.segment.fixed_width
            || fixed_next.len() != registration.segment.fixed_width
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .chain(fixed_current)
                .chain(fixed_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let residues = match registration_index {
            0 => evaluate_zk_x509_der_stark_residues_v1(
                opening
                    .base_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .base_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .aux_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .aux_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                fixed_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                fixed_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                self.source.post_base.der(),
                self.der_public,
                self.source.claims.der,
            )
            .map_err(ZkX509StarkErrorV1::from)?,
            1 => evaluate_zk_x509_rfc5280_stark_residues_v1(
                opening
                    .base_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .base_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .aux_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                opening
                    .aux_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                fixed_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                self.source.post_base.der(),
                self.source.post_base.rfc5280(),
                self.source.claims.rfc5280,
            )
            .map_err(map_main_rfc_source_error_v1)?,
            2..=5 => {
                let segment = registration_index - 2;
                let current = ZkX509ShaBatchRowV1 {
                    base: *<&[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]>::try_from(opening.base_current)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    aux: *<&[F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]>::try_from(opening.aux_current)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    fixed: *<&[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]>::try_from(fixed_current)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                };
                let next = ZkX509ShaBatchRowV1 {
                    base: *<&[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]>::try_from(opening.base_next)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    aux: *<&[F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]>::try_from(opening.aux_next)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    fixed: *<&[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]>::try_from(fixed_next)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                };
                evaluate_zk_x509_sha_batch_residues_v1(
                    &current,
                    &next,
                    self.source.post_base.sha_word(),
                    self.source.post_base.sha(),
                    self.source.post_base.rfc5280(),
                    self.source.claims.sha.segments[segment],
                    &self.source.claims.sha.ca_calls,
                )
                .map_err(map_main_sha_source_error_v1)?
            }
            _ => {
                let binding = self.source.p256_binding_v1(registration)?;
                p256_opened_residues_v1(
                    registration,
                    opening,
                    fixed_current,
                    self.p256_challenges,
                    self.p256_terminals
                        .get(binding.p256.signature_v1())
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
                )?
            }
        };
        if residues.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(residues)
    }

    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed_current: &[F],
        fixed_next: &[F],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if F::canonical(x.0).is_none() || alphas.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        accumulator_quotient_value_v1(
            registration.segment,
            x,
            &self.constraint_residues_v1(registration, opening, fixed_current, fixed_next)?,
            alphas,
        )
    }
}

/// Witness-free opened-row evaluation for the fifteen P-256 registrations in
/// MAIN's mixed native-log19 group.
///
/// The combined 404-column schedule is evaluated once at the verifier's
/// post-grinding current/next union. All registrations then borrow their exact
/// manifest-bound slice from that immutable result.
struct MainP256Log19VerifierConstraintSourceV1 {
    bindings: Vec<MainP256RegistrationBindingV1>,
    common_lde_log2: u8,
    challenges: P256AggregateChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
    fixed_openings: Option<ZkX509FixedAlgebraicOpeningsV1>,
}

impl MainP256Log19VerifierConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
        claims: ZkX509P256TerminalClaimsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let bindings = canonical_p256_main_log19_bindings_v1(layout)?;
        if layout.common_lde_log2 != ZK_X509_MAIN_COMMON_LDE_LOG2_V1
            || bindings
                .iter()
                .any(|binding| binding.main.segment.trace_log2 != ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            bindings,
            common_lde_log2: layout.common_lde_log2,
            challenges: p256_aggregate_challenges_from_post_base_v1(post_base)?,
            terminals: main_p256_terminal_registrations_v1(&claims)?,
            fixed_openings: None,
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn binding_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
        let mut matches = self
            .bindings
            .iter()
            .copied()
            .filter(|binding| binding.main == registration);
        let binding = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(binding)
    }

    fn next_query_index_v1(&self, query_index: usize) -> Result<usize, ZkX509StarkErrorV1> {
        Ok(query_index
            .checked_add(P256_MAIN_LOG19_NEXT_STRIDE_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1()?)
    }

    fn install_verifier_derived_fixed_openings_v1(
        &mut self,
        openings: ZkX509FixedAlgebraicOpeningsV1,
        expected_indices: &[u64],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.fixed_openings.is_some() {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        validate_verifier_derived_p256_log19_fixed_openings_v1(&openings, expected_indices)?;
        self.fixed_openings = Some(openings);
        Ok(())
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let binding = self.binding_v1(registration)?;
        let common_lde_size = self.common_lde_size_v1()?;
        if query_index >= common_lde_size
            || next_query_index >= common_lde_size
            || next_query_index != self.next_query_index_v1(query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || F::canonical(x.0).is_none()
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let fixed = self
            .fixed_openings
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        let current_combined = fixed
            .row_for_query_v1(
                u64::try_from(query_index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
            )
            .map_err(map_fixed_algebraic_error_v1)?
            .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
        let next_combined = fixed
            .row_for_query_v1(
                u64::try_from(next_query_index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
            )
            .map_err(map_fixed_algebraic_error_v1)?
            .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
        let current =
            zk_x509_p256_fixed_algebraic_row_for_registration_v1(current_combined, binding.p256)
                .map_err(map_p256_fixed_algebraic_error_v1)?;
        let next =
            zk_x509_p256_fixed_algebraic_row_for_registration_v1(next_combined, binding.p256)
                .map_err(map_p256_fixed_algebraic_error_v1)?;
        if current.len() != registration.segment.fixed_width
            || next.len() != registration.segment.fixed_width
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        p256_opened_residues_v1(
            registration,
            opening,
            current,
            self.challenges,
            self.terminals
                .get(binding.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }

    #[cfg(test)]
    fn cached_openings_v1(&self) -> usize {
        self.fixed_openings
            .as_ref()
            .map_or(0, ZkX509FixedAlgebraicOpeningsV1::len_v1)
    }
}

fn validate_verifier_derived_p256_log19_fixed_openings_v1(
    openings: &ZkX509FixedAlgebraicOpeningsV1,
    expected_indices: &[u64],
) -> Result<(), ZkX509StarkErrorV1> {
    let schedule =
        zk_x509_p256_fixed_algebraic_schedule_v1().map_err(map_p256_fixed_algebraic_error_v1)?;
    if openings.is_empty_v1()
        || openings.len_v1() > VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        || openings.query_indices_v1() != expected_indices
        || usize::from(openings.width_v1()) != ZK_X509_P256_FIXED_ALGEBRAIC_WIDTH_V1
        || openings.schedule_digest_v1() != schedule.descriptor_digest_v1()
    {
        return Err(ZkX509StarkErrorV1::TraceOpening);
    }
    Ok(())
}

const MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1: usize = ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1;
const MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1: usize =
    ZK_X509_SHA_BATCH_FIXED_WIDTH_V1 - MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1;
const MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1: usize = ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1
    + ZK_X509_SHA_SEGMENT_COUNT_V1 * MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1;
const MAIN_LOG19_AFFINE_SEGMENT_GROWTH_V1: usize = 4_096;
#[cfg(test)]
const MAIN_LOG19_CSR_GROWTH_V1: usize = 4_096;
const _: () = assert!(MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1 == 91);
const _: () = assert!(MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1 == 27);
const _: () = assert!(MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1 == 189);
const _: () = assert!(MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1 <= u8::MAX as usize);
const _: () = assert!(
    P256_MAIN_LOG19_NEXT_STRIDE_V1
        == 1_usize << (ZK_X509_MAIN_COMMON_LDE_LOG2_V1 - ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
);

/// Sparse verifier-generated log19 fixed schedule.
///
/// The algebraic SHA schedule owns the complete fixed row. This independent
/// public schedule retains RFC's complete fixed row and the four SHA public
/// suffixes so installation can differentially check the algebraic openings
/// at the exact transcript-derived points.
#[cfg(test)]
struct MainLog19PublicFixedCsrV1 {
    row_offsets: Vec<u32>,
    columns: Vec<u8>,
    values: Vec<F>,
}

#[cfg(test)]
impl MainLog19PublicFixedCsrV1 {
    fn push_v1(&mut self, column: usize, value: F) -> Result<(), ZkX509StarkErrorV1> {
        if value == F::ZERO {
            return Ok(());
        }
        if column >= MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1
            || F::canonical(value.0).is_none()
            || self.columns.len() != self.values.len()
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        if self.columns.len() == self.columns.capacity() {
            self.columns
                .try_reserve(MAIN_LOG19_CSR_GROWTH_V1)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        }
        if self.values.len() == self.values.capacity() {
            self.values
                .try_reserve(MAIN_LOG19_CSR_GROWTH_V1)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        }
        self.columns
            .push(u8::try_from(column).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?);
        self.values.push(value);
        Ok(())
    }

    fn compile_v1(
        rfc: &ZkX509Rfc5280StarkFixedScheduleV1,
        sha: &ZkX509ShaBatchFixedProviderV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let mut schedule = Self {
            row_offsets: Vec::new(),
            columns: Vec::new(),
            values: Vec::new(),
        };
        schedule
            .row_offsets
            .try_reserve_exact(ZK_X509_DER_STARK_TRACE_SIZE_V1 + 1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        schedule.row_offsets.push(0);
        for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
            let rfc_row = rfc
                .fixed_row(row)
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            for (column, value) in rfc_row.into_iter().enumerate() {
                schedule.push_v1(column, value)?;
            }
            for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
                let sha_row = sha
                    .fixed_row_v1(segment, row)
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                for (local, value) in sha_row[MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1..]
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let column = ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1
                        .checked_add(
                            segment
                                .checked_mul(MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1)
                                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
                        )
                        .and_then(|start| start.checked_add(local))
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                    schedule.push_v1(column, value)?;
                }
            }
            schedule.row_offsets.push(
                u32::try_from(schedule.values.len())
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?,
            );
        }
        schedule.validate_v1()?;
        Ok(schedule)
    }

    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        if self.row_offsets.len() != ZK_X509_DER_STARK_TRACE_SIZE_V1 + 1
            || self.row_offsets.first() != Some(&0)
            || self.columns.len() != self.values.len()
            || self
                .row_offsets
                .last()
                .copied()
                .map(|offset| offset as usize)
                != Some(self.values.len())
            || self.row_offsets.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .columns
                .iter()
                .any(|column| usize::from(*column) >= MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1)
            || self
                .values
                .iter()
                .any(|value| *value == F::ZERO || F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(())
    }

    fn opened_pair_v1(
        &self,
        weights: &[F],
    ) -> Result<
        (
            [F; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1],
            [F; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1],
        ),
        ZkX509StarkErrorV1,
    > {
        self.validate_v1()?;
        if weights.len() != ZK_X509_DER_STARK_TRACE_SIZE_V1
            || weights.iter().any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut current = [F::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1];
        let mut next = [F::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1];
        for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
            let start = usize::try_from(self.row_offsets[row])
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
            let end = usize::try_from(self.row_offsets[row + 1])
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
            let current_weight = weights[row];
            let next_weight = weights
                [(row + ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1) % ZK_X509_DER_STARK_TRACE_SIZE_V1];
            for entry in start..end {
                let column = usize::from(
                    *self
                        .columns
                        .get(entry)
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
                );
                let value = *self
                    .values
                    .get(entry)
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                current[column] = current[column].add(value.mul(current_weight));
                next[column] = next[column].add(value.mul(next_weight));
            }
        }
        Ok((current, next))
    }
}

/// One maximal affine range in a verifier-owned fixed column.
///
/// `value(row) = start_value + step * (row - start)` for `start..end`.
/// Zero ranges are implicit and never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MainLog19PublicFixedAffineSegmentV1 {
    column: u8,
    start: u32,
    end: u32,
    start_value: F,
    step: F,
}

#[derive(Clone, Copy)]
struct MainLog19AffineTrackerV1 {
    initialized: bool,
    start: u32,
    start_value: F,
    previous: F,
    step: Option<F>,
}

impl MainLog19AffineTrackerV1 {
    const EMPTY: Self = Self {
        initialized: false,
        start: 0,
        start_value: F::ZERO,
        previous: F::ZERO,
        step: None,
    };

    fn observe_v1(&mut self, row: u32, value: F) -> Option<(u32, u32, F, F)> {
        if !self.initialized {
            self.initialized = true;
            self.start = row;
            self.start_value = value;
            self.previous = value;
            self.step = None;
            return None;
        }
        let candidate_step = value.sub(self.previous);
        match self.step {
            None => {
                self.step = Some(candidate_step);
                self.previous = value;
                None
            }
            Some(step) if step == candidate_step => {
                self.previous = value;
                None
            }
            Some(step) => {
                let completed = (self.start, row, self.start_value, step);
                self.start = row;
                self.start_value = value;
                self.previous = value;
                self.step = None;
                Some(completed)
            }
        }
    }

    fn finish_v1(self, end: u32) -> Option<(u32, u32, F, F)> {
        self.initialized.then_some((
            self.start,
            end,
            self.start_value,
            self.step.unwrap_or(F::ZERO),
        ))
    }
}

/// Canonical piecewise-affine representation of all verifier-generated public
/// fixed columns in MAIN's native-log19 group.
///
/// Long selector runs, counters, offsets, and statement-derived event ranges
/// are retained structurally instead of materializing 3.5 million nonzero
/// cells. All transcript queries are evaluated as one batch over this schedule.
struct MainLog19PublicFixedAffineScheduleV1 {
    segments: Vec<MainLog19PublicFixedAffineSegmentV1>,
}

fn main_log19_public_fixed_row_v1(
    rfc: &ZkX509Rfc5280StarkFixedScheduleV1,
    sha: &ZkX509ShaBatchFixedProviderV1,
    row: usize,
) -> Result<[F; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1], ZkX509StarkErrorV1> {
    let mut combined = [F::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1];
    let rfc_row = rfc
        .fixed_row(row)
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    combined[..ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1].copy_from_slice(&rfc_row);
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        let sha_row = sha
            .fixed_row_v1(segment, row)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let start =
            ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1 + segment * MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1;
        combined[start..start + MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1]
            .copy_from_slice(&sha_row[MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1..]);
    }
    if combined.iter().any(|value| F::canonical(value.0).is_none()) {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(combined)
}

impl MainLog19PublicFixedAffineScheduleV1 {
    fn push_v1(
        &mut self,
        column: usize,
        start: u32,
        end: u32,
        start_value: F,
        step: F,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if start_value == F::ZERO && step == F::ZERO {
            return Ok(());
        }
        if column >= MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1
            || start >= end
            || usize::try_from(end).map_or(true, |end| end > ZK_X509_DER_STARK_TRACE_SIZE_V1)
            || F::canonical(start_value.0).is_none()
            || F::canonical(step.0).is_none()
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        if self.segments.len() == self.segments.capacity() {
            self.segments
                .try_reserve(MAIN_LOG19_AFFINE_SEGMENT_GROWTH_V1)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        }
        self.segments.push(MainLog19PublicFixedAffineSegmentV1 {
            column: u8::try_from(column).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            start,
            end,
            start_value,
            step,
        });
        Ok(())
    }

    fn compile_v1(
        rfc: &ZkX509Rfc5280StarkFixedScheduleV1,
        sha: &ZkX509ShaBatchFixedProviderV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let mut schedule = Self {
            segments: Vec::new(),
        };
        let mut trackers = [MainLog19AffineTrackerV1::EMPTY; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1];
        for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
            let row_u32 = u32::try_from(row).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
            let combined = main_log19_public_fixed_row_v1(rfc, sha, row)?;
            for (column, (tracker, value)) in trackers.iter_mut().zip(combined).enumerate() {
                if let Some((start, end, start_value, step)) = tracker.observe_v1(row_u32, value) {
                    schedule.push_v1(column, start, end, start_value, step)?;
                }
            }
        }
        let end = u32::try_from(ZK_X509_DER_STARK_TRACE_SIZE_V1)
            .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
        for (column, tracker) in trackers.into_iter().enumerate() {
            if let Some((start, end, start_value, step)) = tracker.finish_v1(end) {
                schedule.push_v1(column, start, end, start_value, step)?;
            }
        }
        schedule
            .segments
            .sort_unstable_by_key(|segment| (segment.column, segment.start));
        schedule.validate_v1()?;
        Ok(schedule)
    }

    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        if self.segments.is_empty()
            || self
                .segments
                .windows(2)
                .any(|pair| (pair[0].column, pair[0].start) >= (pair[1].column, pair[1].start))
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        let mut previous: [Option<MainLog19PublicFixedAffineSegmentV1>;
            MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1] = [None; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1];
        for segment in &self.segments {
            let column = usize::from(segment.column);
            if column >= MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1
                || segment.start >= segment.end
                || usize::try_from(segment.end)
                    .map_or(true, |end| end > ZK_X509_DER_STARK_TRACE_SIZE_V1)
                || (segment.start_value == F::ZERO && segment.step == F::ZERO)
                || F::canonical(segment.start_value.0).is_none()
                || F::canonical(segment.step.0).is_none()
                || previous[column].is_some_and(|prior| prior.end > segment.start)
            {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            previous[column] = Some(*segment);
        }
        Ok(())
    }

    fn opened_all_v1(
        &self,
        query_schedule: &MainLog19VerifierQueryScheduleV1,
    ) -> Result<BTreeMap<usize, MainLog19VerifierGeneratedFixedOpeningV1>, ZkX509StarkErrorV1> {
        self.validate_v1()?;
        query_schedule.validate_v1()?;
        let common_lde_size = 1_usize
            .checked_shl(u32::from(ZK_X509_MAIN_COMMON_LDE_LOG2_V1))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut groups = BTreeMap::<usize, Vec<(usize, usize)>>::new();
        for index in query_schedule.indices.iter().copied() {
            let index = usize::try_from(index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            if index >= common_lde_size {
                return Err(ZkX509StarkErrorV1::TraceOpening);
            }
            groups
                .entry(index % P256_MAIN_LOG19_NEXT_STRIDE_V1)
                .or_default()
                .push((index, index / P256_MAIN_LOG19_NEXT_STRIDE_V1));
        }

        let mut generated = BTreeMap::new();
        for (remainder, indices_and_shifts) in groups {
            let weights = main_log19_lagrange_weights_v1(remainder)?;
            let (prefix, linear_prefix) = main_log19_weight_prefixes_v1(&weights)?;
            let mut combined = Vec::new();
            combined
                .try_reserve_exact(indices_and_shifts.len())
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            combined.resize(
                indices_and_shifts.len(),
                [F::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1],
            );
            for segment in &self.segments {
                for ((_, shift), opening) in indices_and_shifts.iter().zip(&mut combined) {
                    let contribution = main_log19_shifted_affine_segment_sum_v1(
                        &prefix,
                        &linear_prefix,
                        *shift,
                        *segment,
                    )?;
                    let column = usize::from(segment.column);
                    opening[column] = opening[column].add(contribution);
                }
            }
            for ((index, shift), combined) in indices_and_shifts.into_iter().zip(combined) {
                let der = main_log19_der_fixed_opening_from_prefix_v1(&weights, &prefix, shift)?;
                if generated
                    .insert(index, main_log19_generated_fixed_opening_v1(der, combined))
                    .is_some()
                {
                    return Err(ZkX509StarkErrorV1::InternalInvariant);
                }
            }
        }
        if generated.len() != query_schedule.indices.len()
            || generated
                .keys()
                .copied()
                .zip(query_schedule.indices.iter().copied())
                .any(|(actual, expected)| u64::try_from(actual).ok() != Some(expected))
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(generated)
    }
}

fn main_log19_weight_prefixes_v1(weights: &[F]) -> Result<(Vec<F>, Vec<F>), ZkX509StarkErrorV1> {
    if weights.len() != ZK_X509_DER_STARK_TRACE_SIZE_V1
        || weights.iter().any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut prefix = Vec::new();
    let mut linear_prefix = Vec::new();
    prefix
        .try_reserve_exact(ZK_X509_DER_STARK_TRACE_SIZE_V1 + 1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    linear_prefix
        .try_reserve_exact(ZK_X509_DER_STARK_TRACE_SIZE_V1 + 1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    prefix.push(F::ZERO);
    linear_prefix.push(F::ZERO);
    for (row, weight) in weights.iter().copied().enumerate() {
        prefix.push(
            prefix
                .last()
                .copied()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                .add(weight),
        );
        linear_prefix.push(
            linear_prefix
                .last()
                .copied()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                .add(
                    F(u64::try_from(row).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?)
                        .mul(weight),
                ),
        );
    }
    if prefix.last() != Some(&F::ONE) {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok((prefix, linear_prefix))
}

fn main_log19_affine_prefix_sum_v1(
    prefix: &[F],
    linear_prefix: &[F],
    start: usize,
    end: usize,
    start_value: F,
    step: F,
) -> Result<F, ZkX509StarkErrorV1> {
    if start > end
        || end > ZK_X509_DER_STARK_TRACE_SIZE_V1
        || prefix.len() != ZK_X509_DER_STARK_TRACE_SIZE_V1 + 1
        || linear_prefix.len() != prefix.len()
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let weight_sum = prefix[end].sub(prefix[start]);
    let relative_linear_sum = linear_prefix[end].sub(linear_prefix[start]).sub(
        F(u64::try_from(start).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?).mul(weight_sum),
    );
    Ok(start_value
        .mul(weight_sum)
        .add(step.mul(relative_linear_sum)))
}

fn main_log19_shifted_affine_segment_sum_v1(
    prefix: &[F],
    linear_prefix: &[F],
    shift: usize,
    segment: MainLog19PublicFixedAffineSegmentV1,
) -> Result<F, ZkX509StarkErrorV1> {
    let rows = ZK_X509_DER_STARK_TRACE_SIZE_V1;
    if shift >= rows {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let start =
        usize::try_from(segment.start).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let end = usize::try_from(segment.end).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let mut result = F::ZERO;
    let before_end = end.min(shift);
    if start < before_end {
        let mapped_start = start
            .checked_add(rows - shift)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        result = result.add(main_log19_affine_prefix_sum_v1(
            prefix,
            linear_prefix,
            mapped_start,
            mapped_start + before_end - start,
            segment.start_value,
            segment.step,
        )?);
    }
    let after_start = start.max(shift);
    if after_start < end {
        let value_at_after = segment.start_value.add(
            segment.step.mul(F(u64::try_from(after_start - start)
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?)),
        );
        result = result.add(main_log19_affine_prefix_sum_v1(
            prefix,
            linear_prefix,
            after_start - shift,
            end - shift,
            value_at_after,
            segment.step,
        )?);
    }
    Ok(result)
}

fn main_log19_shifted_weight_v1(
    weights: &[F],
    row: usize,
    shift: usize,
) -> Result<F, ZkX509StarkErrorV1> {
    let rows = ZK_X509_DER_STARK_TRACE_SIZE_V1;
    if weights.len() != rows || row >= rows || shift >= rows {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(weights[(row + rows - shift) % rows])
}

fn main_log19_shifted_weight_sum_v1(
    prefix: &[F],
    start: usize,
    end: usize,
    shift: usize,
) -> Result<F, ZkX509StarkErrorV1> {
    let rows = ZK_X509_DER_STARK_TRACE_SIZE_V1;
    if prefix.len() != rows + 1 || start > end || end > rows || shift >= rows {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let mut result = F::ZERO;
    let before_end = end.min(shift);
    if start < before_end {
        let mapped_start = start + rows - shift;
        result = result.add(prefix[mapped_start + before_end - start].sub(prefix[mapped_start]));
    }
    let after_start = start.max(shift);
    if after_start < end {
        result = result.add(prefix[end - shift].sub(prefix[after_start - shift]));
    }
    Ok(result)
}

fn main_log19_der_fixed_opening_from_prefix_v1(
    weights: &[F],
    prefix: &[F],
    shift: usize,
) -> Result<[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1], ZkX509StarkErrorV1> {
    let mut fixed = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
    fixed[FIX_FIRST_AGGREGATE] = main_log19_shifted_weight_v1(weights, 0, shift)?;
    fixed[FIX_LAST_AGGREGATE] =
        main_log19_shifted_weight_v1(weights, ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1, shift)?;
    fixed[FIX_FIRST_ACTIVE] = fixed[FIX_FIRST_AGGREGATE];
    fixed[DER_FIX_LAST_ACTIVE] = main_log19_shifted_weight_v1(
        weights,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
        shift,
    )?;
    fixed[FIX_FIRST_PARSER] = fixed[FIX_FIRST_AGGREGATE];
    fixed[FIX_LAST_PARSER] = main_log19_shifted_weight_v1(
        weights,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
        shift,
    )?;
    fixed[FIX_FIRST_COMPARATOR] = main_log19_shifted_weight_v1(
        weights,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
        shift,
    )?;
    fixed[FIX_LAST_COMPARATOR] = fixed[DER_FIX_LAST_ACTIVE];
    fixed[DER_FIX_ACTIVE] = main_log19_shifted_weight_sum_v1(
        prefix,
        0,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        shift,
    )?;
    fixed[FIX_PARSER] = main_log19_shifted_weight_sum_v1(
        prefix,
        0,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
        shift,
    )?;
    fixed[FIX_PARSER_CONTINUE] = main_log19_shifted_weight_sum_v1(
        prefix,
        0,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
        shift,
    )?;
    fixed[FIX_COMPARATOR] = main_log19_shifted_weight_sum_v1(
        prefix,
        super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        shift,
    )?;
    fixed[FIX_PADDING] = main_log19_shifted_weight_sum_v1(
        prefix,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        ZK_X509_DER_STARK_TRACE_SIZE_V1,
        shift,
    )?;
    if fixed[DER_FIX_ACTIVE].add(fixed[FIX_PADDING]) != F::ONE
        || fixed[FIX_PARSER].add(fixed[FIX_COMPARATOR]) != fixed[DER_FIX_ACTIVE]
        || fixed[FIX_PARSER_CONTINUE].add(fixed[FIX_LAST_PARSER]) != fixed[FIX_PARSER]
        || fixed[FIX_FINAL_DOCUMENT] != F::ZERO
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(fixed)
}

fn main_log19_lagrange_weights_v1(query_index: usize) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    let common_lde_size = 1_usize
        .checked_shl(u32::from(ZK_X509_MAIN_COMMON_LDE_LOG2_V1))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if query_index >= common_lde_size {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let trace_root = goldilocks_primitive_root_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
        .map_err(map_transparent_error_v1)?;
    let common_root = goldilocks_primitive_root_v1(ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
        .map_err(map_transparent_error_v1)?;
    if common_root.pow(P256_MAIN_LOG19_NEXT_STRIDE_V1 as u128) != trace_root {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
    let numerator = x.pow(ZK_X509_DER_STARK_TRACE_SIZE_V1 as u128).sub(F::ONE);
    if numerator == F::ZERO {
        return Err(ZkX509StarkErrorV1::TranscriptMismatch);
    }
    let inverse_trace_size = F(u64::try_from(ZK_X509_DER_STARK_TRACE_SIZE_V1)
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?)
    .inv()
    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let common = numerator.mul(inverse_trace_size);
    let mut denominators = Vec::new();
    denominators
        .try_reserve_exact(ZK_X509_DER_STARK_TRACE_SIZE_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    let mut trace_point = F::ONE;
    for _ in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        denominators.push(x.sub(trace_point));
        trace_point = trace_point.mul(trace_root);
    }
    goldilocks_batch_invert_v1(&mut denominators).map_err(map_transparent_error_v1)?;
    trace_point = F::ONE;
    let mut sum = F::ZERO;
    for inverse in &mut denominators {
        *inverse = common.mul(trace_point).mul(*inverse);
        sum = sum.add(*inverse);
        trace_point = trace_point.mul(trace_root);
    }
    if trace_point != F::ONE || sum != F::ONE {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(denominators)
}

#[cfg(test)]
fn main_log19_der_fixed_opening_v1(
    weights: &[F],
    next: bool,
) -> Result<[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1], ZkX509StarkErrorV1> {
    if weights.len() != ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let weight = |row: usize| {
        let index = if next {
            (row + ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1) % ZK_X509_DER_STARK_TRACE_SIZE_V1
        } else {
            row
        };
        weights[index]
    };
    let mut fixed = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
    fixed[FIX_FIRST_AGGREGATE] = weight(0);
    fixed[FIX_LAST_AGGREGATE] = weight(ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1);
    fixed[FIX_FIRST_ACTIVE] = weight(0);
    fixed[DER_FIX_LAST_ACTIVE] = weight(ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1);
    fixed[FIX_FIRST_PARSER] = weight(0);
    fixed[FIX_LAST_PARSER] = weight(super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1);
    fixed[FIX_FIRST_COMPARATOR] = weight(super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1);
    fixed[FIX_LAST_COMPARATOR] = weight(ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1);
    for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        let value = weight(row);
        if row < ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 {
            fixed[DER_FIX_ACTIVE] = fixed[DER_FIX_ACTIVE].add(value);
            if row < super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
                fixed[FIX_PARSER] = fixed[FIX_PARSER].add(value);
                if row + 1 < super::der_stark::ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
                    fixed[FIX_PARSER_CONTINUE] = fixed[FIX_PARSER_CONTINUE].add(value);
                }
            } else {
                fixed[FIX_COMPARATOR] = fixed[FIX_COMPARATOR].add(value);
            }
        } else {
            fixed[FIX_PADDING] = fixed[FIX_PADDING].add(value);
        }
    }
    if fixed[DER_FIX_ACTIVE].add(fixed[FIX_PADDING]) != F::ONE
        || fixed[FIX_PARSER].add(fixed[FIX_COMPARATOR]) != fixed[DER_FIX_ACTIVE]
        || fixed[FIX_PARSER_CONTINUE].add(fixed[FIX_LAST_PARSER]) != fixed[FIX_PARSER]
        || fixed[FIX_FINAL_DOCUMENT] != F::ZERO
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(fixed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MainLog19VerifierGeneratedFixedOpeningV1 {
    der: [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    rfc: ZkX509Rfc5280StarkFixedRowV1,
    sha_public: [[F; MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1]; ZK_X509_SHA_SEGMENT_COUNT_V1],
}

fn main_log19_generated_fixed_opening_v1(
    der: [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    combined: [F; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1],
) -> MainLog19VerifierGeneratedFixedOpeningV1 {
    let mut rfc = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
    rfc.copy_from_slice(&combined[..ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1]);
    let mut sha_public =
        [[F::ZERO; MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1]; ZK_X509_SHA_SEGMENT_COUNT_V1];
    for (segment, target) in sha_public.iter_mut().enumerate() {
        let start =
            ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1 + segment * MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1;
        target.copy_from_slice(&combined[start..start + MAIN_LOG19_SHA_PUBLIC_FIXED_WIDTH_V1]);
    }
    MainLog19VerifierGeneratedFixedOpeningV1 {
        der,
        rfc,
        sha_public,
    }
}

fn validate_verifier_derived_sha_fixed_openings_v1(
    openings: &ZkX509FixedAlgebraicOpeningsV1,
    expected_indices: &[u64],
    shape: ZkX509ShaCallPublicShapeV1,
) -> Result<(), ZkX509StarkErrorV1> {
    let schedule =
        zk_x509_sha_fixed_algebraic_schedule_v1(shape).map_err(map_sha_fixed_algebraic_error_v1)?;
    if openings.is_empty_v1()
        || openings.len_v1() > VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        || openings.query_indices_v1() != expected_indices
        || usize::from(openings.width_v1()) != ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1
        || openings.schedule_digest_v1() != schedule.descriptor_digest_v1()
    {
        return Err(ZkX509StarkErrorV1::TraceOpening);
    }
    Ok(())
}

fn expand_main_log19_sha_fixed_opening_v1(
    combined: &[F],
    public: &MainLog19VerifierGeneratedFixedOpeningV1,
) -> Result<[[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]; ZK_X509_SHA_SEGMENT_COUNT_V1], ZkX509StarkErrorV1>
{
    if combined.len() != ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1
        || combined.iter().any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::TraceOpening);
    }
    let mut rows = [[F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]; ZK_X509_SHA_SEGMENT_COUNT_V1];
    for (segment, row) in rows.iter_mut().enumerate() {
        let start = segment * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1;
        row.copy_from_slice(&combined[start..start + ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]);
        if row[MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1..] != public.sha_public[segment] {
            return Err(ZkX509StarkErrorV1::TraceOpening);
        }
    }
    Ok(rows)
}

struct MainLog19InstalledFixedOpeningsV1 {
    query_schedule: MainLog19VerifierQueryScheduleV1,
    generated: BTreeMap<usize, MainLog19VerifierGeneratedFixedOpeningV1>,
    sha: BTreeMap<usize, [[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]; ZK_X509_SHA_SEGMENT_COUNT_V1]>,
}

/// Closed verifier owner for MAIN's complete mixed native-log19 group.
///
/// This is the only production route that can evaluate DER, RFC 5280, all
/// four SHA registrations, and the fifteen P-256 registrations. It owns the
/// exact 21-registration layout and cannot be initialized without the one
/// verifier-derived opening token bound to all 58 transcript-order queries.
struct MainLog19VerifierConstraintSourceV1 {
    registrations: Vec<RegisteredSegmentLayoutV1>,
    post_base: ZkX509CredentialMainPostBaseChallengesV1,
    claims: ZkX509MainTerminalClaimsV1,
    der_public: ZkX509DerStarkPublicTerminalsV1,
    rfc_fixed: ZkX509Rfc5280StarkFixedScheduleV1,
    sha_shape: ZkX509ShaCallPublicShapeV1,
    sha_fixed: ZkX509ShaBatchFixedProviderV1,
    public_fixed: Option<MainLog19PublicFixedAffineScheduleV1>,
    p256: MainP256Log19VerifierConstraintSourceV1,
    fixed_openings: Option<MainLog19InstalledFixedOpeningsV1>,
}

impl MainLog19VerifierConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        rfc_statement: &ZkX509Rfc5280StatementV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
        claims: ZkX509MainTerminalClaimsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let registrations = canonical_main_log19_registrations_v1(layout)?;
        validate_zk_x509_der_rfc_terminal_equalities_v1(claims.der, claims.rfc5280)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        if !zk_x509_main_rfc_sha_terminal_products_match_v1(claims.rfc5280, claims.sha) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        claims
            .rfc5280
            .encode_x5r1_v1()
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        claims
            .sha
            .encode_x5q1_v1()
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let der_public =
            derive_zk_x509_der_stark_public_terminals_v1(&ZkX509DerStarkShapeV1, post_base.der())?;
        let rfc_shape = ZkX509Rfc5280StarkShapeV1::from_statement(rfc_statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let rfc_fixed = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(rfc_shape)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let sha_shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: rfc_statement.disclosed_attribute_indices.len(),
        };
        let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(sha_shape)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let p256 =
            MainP256Log19VerifierConstraintSourceV1::for_main_v1(layout, post_base, claims.p256)?;
        Ok(Self {
            registrations,
            post_base,
            claims,
            der_public,
            rfc_fixed,
            sha_shape,
            sha_fixed,
            public_fixed: None,
            p256,
            fixed_openings: None,
        })
    }

    fn common_lde_size_v1(&self) -> usize {
        1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1
    }

    fn next_query_index_v1(&self, query_index: usize) -> Result<usize, ZkX509StarkErrorV1> {
        Ok(query_index
            .checked_add(P256_MAIN_LOG19_NEXT_STRIDE_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1())
    }

    fn registration_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let mut matches = self
            .registrations
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == registration)
            .map(|(index, _)| index);
        let index = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches.next().is_some() {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(index)
    }

    fn install_verifier_derived_fixed_openings_v1(
        &mut self,
        derived: ZkX509MainVerifierDerivedFixedOpeningsV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.public_fixed.is_some()
            || self.fixed_openings.is_some()
            || self.p256.fixed_openings.is_some()
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        let ZkX509MainVerifierDerivedFixedOpeningsV1 {
            query_schedule,
            sha,
            p256_log19,
        } = derived;
        query_schedule.validate_v1()?;
        validate_verifier_derived_sha_fixed_openings_v1(
            &sha,
            &query_schedule.indices,
            self.sha_shape,
        )?;
        validate_verifier_derived_p256_log19_fixed_openings_v1(
            &p256_log19,
            &query_schedule.indices,
        )?;

        // Compile and open the verifier-owned public schedule exactly once for
        // the complete transcript query set. Every allocation, interpolation,
        // and algebraic-SHA consistency check completes in temporary storage before
        // any cache becomes observable.
        let public_fixed =
            MainLog19PublicFixedAffineScheduleV1::compile_v1(&self.rfc_fixed, &self.sha_fixed)?;
        let generated = public_fixed.opened_all_v1(&query_schedule)?;
        let mut expanded_sha = BTreeMap::new();
        for index in query_schedule.indices.iter().copied() {
            let combined = sha
                .row_for_query_v1(index)
                .map_err(map_fixed_algebraic_error_v1)?
                .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
            let index = usize::try_from(index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let public = generated
                .get(&index)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            if expanded_sha
                .insert(
                    index,
                    expand_main_log19_sha_fixed_opening_v1(combined, public)?,
                )
                .is_some()
            {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
        }
        if generated.keys().copied().ne(expanded_sha.keys().copied())
            || expanded_sha.len() != query_schedule.indices.len()
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        // The P-256 opening was fully validated above. Its installation is the
        // only fallible mutation point; the remaining moves are infallible.
        self.p256
            .install_verifier_derived_fixed_openings_v1(p256_log19, &query_schedule.indices)?;
        let installed = MainLog19InstalledFixedOpeningsV1 {
            query_schedule,
            generated,
            sha: expanded_sha,
        };
        self.public_fixed = Some(public_fixed);
        self.fixed_openings = Some(installed);
        Ok(())
    }

    fn validate_opening_request_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let registration_index = self.registration_index_v1(registration)?;
        let fixed = self
            .fixed_openings
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        if !fixed
            .query_schedule
            .pairs
            .contains(&(query_index, next_query_index))
            || query_index >= self.common_lde_size_v1()
            || next_query_index != self.next_query_index_v1(query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || F::canonical(x.0).is_none()
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root = goldilocks_primitive_root_v1(ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
            .map_err(map_transparent_error_v1)?;
        if x != F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128))
            || fixed.generated.get(&query_index).is_none()
            || fixed.generated.get(&next_query_index).is_none()
            || fixed.sha.get(&query_index).is_none()
            || fixed.sha.get(&next_query_index).is_none()
        {
            return Err(ZkX509StarkErrorV1::TraceOpening);
        }
        Ok(registration_index)
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = self.validate_opening_request_v1(
            registration,
            query_index,
            next_query_index,
            x,
            opening,
        )?;
        if registration_index >= MAIN_LOG19_NON_P256_REGISTRATION_COUNT_V1 {
            return self.p256.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            );
        }
        let fixed = self
            .fixed_openings
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        let current_fixed = fixed
            .generated
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
        let next_fixed = fixed
            .generated
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
        let residues = match registration.segment.adapter {
            SegmentAdapterIdV1::StrictDer => {
                let current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1] = opening
                    .base_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1] = opening
                    .base_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1] = opening
                    .aux_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let next_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1] = opening
                    .aux_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                evaluate_zk_x509_der_stark_residues_v1(
                    current,
                    next,
                    current_aux,
                    next_aux,
                    &current_fixed.der,
                    &next_fixed.der,
                    self.post_base.der(),
                    self.der_public,
                    self.claims.der,
                )
                .map_err(|_| ZkX509StarkErrorV1::ConstraintOpening)?
            }
            SegmentAdapterIdV1::Rfc5280 => {
                let current: &ZkX509Rfc5280StarkBaseRowV1 = opening
                    .base_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let next: &ZkX509Rfc5280StarkBaseRowV1 = opening
                    .base_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let current_aux: &ZkX509Rfc5280StarkAuxRowV1 = opening
                    .aux_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                let next_aux: &ZkX509Rfc5280StarkAuxRowV1 = opening
                    .aux_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
                evaluate_zk_x509_rfc5280_stark_residues_v1(
                    current,
                    next,
                    current_aux,
                    next_aux,
                    &current_fixed.rfc,
                    self.post_base.der(),
                    self.post_base.rfc5280(),
                    self.claims.rfc5280,
                )
                .map_err(|_| ZkX509StarkErrorV1::ConstraintOpening)?
            }
            SegmentAdapterIdV1::Sha256CallBus => {
                let segment = usize::from(registration.segment.instance);
                let current_sha_fixed = fixed
                    .sha
                    .get(&query_index)
                    .and_then(|rows| rows.get(segment))
                    .copied()
                    .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
                let next_sha_fixed = fixed
                    .sha
                    .get(&next_query_index)
                    .and_then(|rows| rows.get(segment))
                    .copied()
                    .ok_or(ZkX509StarkErrorV1::TraceOpening)?;
                let current = ZkX509ShaBatchRowV1 {
                    base: *<&[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]>::try_from(opening.base_current)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    aux: *<&[F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]>::try_from(opening.aux_current)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    fixed: current_sha_fixed,
                };
                let next = ZkX509ShaBatchRowV1 {
                    base: *<&[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]>::try_from(opening.base_next)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    aux: *<&[F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]>::try_from(opening.aux_next)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                    fixed: next_sha_fixed,
                };
                evaluate_zk_x509_sha_batch_residues_v1(
                    &current,
                    &next,
                    self.post_base.sha_word(),
                    self.post_base.sha(),
                    self.post_base.rfc5280(),
                    *self
                        .claims
                        .sha
                        .segments
                        .get(segment)
                        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
                    &self.claims.sha.ca_calls,
                )
                .map_err(|_| ZkX509StarkErrorV1::ConstraintOpening)?
            }
            _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
        };
        if residues.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(residues)
    }

    #[cfg(test)]
    fn cached_openings_v1(&self) -> usize {
        self.fixed_openings
            .as_ref()
            .map_or(0, |fixed| fixed.generated.len())
    }
}

/// Test-only opened-row adversary interface.
///
/// Production MAIN verification never accepts a dynamic implementation.
/// Concrete verifier variants below are the only production constructors.
#[cfg(test)]
trait MainOpenedConstraintTestSourceV1 {
    fn fixed_opened_rows_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
    ) -> Result<MainFixedOpenedRowsV1, ZkX509StarkErrorV1>;

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &MainFixedOpenedRowsV1,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1>;
}

/// Test-only fixed rows used to exercise malformed-provider rejection.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MainFixedOpenedRowsV1 {
    current: Vec<F>,
    next: Vec<F>,
}

fn validate_main_projection_registration_v1(
    registration: RegisteredSegmentLayoutV1,
) -> Result<(), ZkX509StarkErrorV1> {
    if registration.segment != SegmentLayoutV1::for_projection()? {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

fn copied_array_column_v1<const WIDTH: usize>(
    rows: &[[F; WIDTH]],
    local_column: usize,
) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
    if local_column >= WIDTH {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut column = Vec::new();
    column
        .try_reserve_exact(rows.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    column.extend(rows.iter().map(|row| row[local_column]));
    Ok(ZeroizingMainTraceColumnV1(column))
}

fn copied_matrix_column_v1(
    columns: &[Vec<F>],
    expected_width: usize,
    expected_rows: usize,
    local_column: usize,
) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
    if columns.len() != expected_width || local_column >= expected_width {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let source = columns
        .get(local_column)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if source.len() != expected_rows || source.iter().any(|value| F::canonical(value.0).is_none()) {
        return Err(ZkX509StarkErrorV1::IoWitness);
    }
    let mut column = Vec::new();
    column
        .try_reserve_exact(source.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    column.extend_from_slice(source);
    Ok(ZeroizingMainTraceColumnV1(column))
}

fn main_p256_scalar_registrations_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<[MainP256RegistrationBindingV1; P256_SIGNATURE_COUNT_V1], ZkX509StarkErrorV1> {
    let registrations = canonical_p256_main_layout_bindings_v1(layout)?
        .into_iter()
        .filter(|binding| {
            binding.main.segment.trace_log2 == P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1
                && binding.main.segment.adapter == SegmentAdapterIdV1::P256ScalarBitBus
                && binding.p256.adapter_v1() == P256MainAdapterV1::ScalarBitBus
        })
        .collect::<Vec<_>>();
    let group = registrations
        .first()
        .map(|binding| binding.main.trace_group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let group_layout = layout
        .trace_groups
        .get(group)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if registrations.len() != P256_SIGNATURE_COUNT_V1
        || registrations
            .iter()
            .enumerate()
            .any(|(signature, binding)| {
                binding.p256.signature_v1() != signature
                    || binding.p256.local_instance_v1() != 0
                    || p256_instance_parts_v1(binding.main.segment.instance) != Some((signature, 0))
                    || binding.main.segment.constraint_count
                        != P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
            })
        || registrations
            .iter()
            .any(|binding| binding.main.trace_group != group)
        || group_layout.native_trace_log2 != P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1
        || group_layout.base_width
            != P256_SIGNATURE_COUNT_V1 * P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1
        || group_layout.aux_width
            != P256_SIGNATURE_COUNT_V1 * P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1
        || group_layout.column_chunks != P256_SIGNATURE_COUNT_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    registrations
        .try_into()
        .map_err(|_: Vec<MainP256RegistrationBindingV1>| ZkX509StarkErrorV1::InternalInvariant)
}

fn main_p256_scalar_registration_v1(
    registrations: &[MainP256RegistrationBindingV1; P256_SIGNATURE_COUNT_V1],
    registration: RegisteredSegmentLayoutV1,
) -> Result<MainP256RegistrationBindingV1, ZkX509StarkErrorV1> {
    let mut matches = registrations
        .iter()
        .copied()
        .filter(|candidate| candidate.main == registration);
    let matched = matches.next().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if matches.next().is_some() {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(matched)
}

fn main_p256_terminal_registration_v1(
    claims: &ZkX509P256TerminalClaimsV1,
    signature: usize,
) -> Result<P256TerminalRegistrationV1, ZkX509StarkErrorV1> {
    let (buses, cross_sources, sink, role) =
        if let Some(claims) = claims.certificate_or_crl.get(signature) {
            (
                claims.buses,
                claims.cross_sources.as_slice(),
                claims.sink,
                P256EcdsaRoleV1::CertificateOrCrl,
            )
        } else if signature == P256_SIGNATURE_COUNT_V1 - 1 {
            (
                claims.wallet.buses,
                claims.wallet.cross_sources.as_slice(),
                claims.wallet.sink,
                P256EcdsaRoleV1::WalletOwnership,
            )
        } else {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        };
    let mut owned_cross_sources = Vec::new();
    owned_cross_sources
        .try_reserve_exact(cross_sources.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    owned_cross_sources.extend_from_slice(cross_sources);
    let terminals = P256TerminalRegistrationV1 {
        buses,
        cross_sources: owned_cross_sources,
        sink,
    };
    terminals.validate(role)?;
    Ok(terminals)
}

fn main_p256_terminal_registrations_v1(
    claims: &ZkX509P256TerminalClaimsV1,
) -> Result<[P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1], ZkX509StarkErrorV1> {
    let mut registrations = Vec::new();
    registrations
        .try_reserve_exact(P256_SIGNATURE_COUNT_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        registrations.push(main_p256_terminal_registration_v1(claims, signature)?);
    }
    registrations
        .try_into()
        .map_err(|_: Vec<P256TerminalRegistrationV1>| ZkX509StarkErrorV1::InternalInvariant)
}

fn zeroed_main_trace_column_v1(
    rows: usize,
) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(rows)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    values.resize(rows, F::ZERO);
    Ok(ZeroizingMainTraceColumnV1(values))
}

/// Exact five-signature scalar-bit trace source for the canonical MAIN log-8
/// group.
///
/// This is a non-owning projection of the one central P-256 provider. The base
/// view is constructed before X5B1 and cannot expose auxiliary columns. After
/// the central provider consumes the opaque post-base token, callers drop that
/// view and construct a bound view over the resulting capability.
#[derive(Clone, Copy)]
enum MainP256ScalarTraceViewV1<'a> {
    Base(&'a P256MainBaseSourceV1),
    Bound(&'a P256MainBoundSourceV1),
}

struct MainP256ScalarTraceGroupSourceV1<'a> {
    registrations: [MainP256RegistrationBindingV1; P256_SIGNATURE_COUNT_V1],
    view: MainP256ScalarTraceViewV1<'a>,
}

impl<'a> MainP256ScalarTraceGroupSourceV1<'a> {
    fn for_base_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBaseSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let registrations = main_p256_scalar_registrations_v1(layout)?;
        let canonical = source.canonical_registrations_v1()?;
        validate_p256_main_registration_order_v1(&canonical)?;
        if registrations.iter().any(|registration| {
            !canonical
                .iter()
                .any(|candidate| *candidate == registration.p256)
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            registrations,
            view: MainP256ScalarTraceViewV1::Base(source),
        })
    }

    fn for_bound_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let registrations = main_p256_scalar_registrations_v1(layout)?;
        let canonical = source.canonical_registrations_v1()?;
        validate_p256_main_registration_order_v1(&canonical)?;
        if registrations.iter().any(|registration| {
            !canonical
                .iter()
                .any(|candidate| *candidate == registration.p256)
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            registrations,
            view: MainP256ScalarTraceViewV1::Bound(source),
        })
    }

    fn p256_registration_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<P256MainRegistrationV1, ZkX509StarkErrorV1> {
        Ok(main_p256_scalar_registration_v1(&self.registrations, registration)?.p256)
    }
}

impl MainTraceGroupSourceV1 for MainP256ScalarTraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let p256 = self.p256_registration_v1(registration)?;
        if local_column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        match self.view {
            MainP256ScalarTraceViewV1::Base(source) => {
                source.fill_base_column_v1(p256, local_column, &mut output.0)?;
            }
            MainP256ScalarTraceViewV1::Bound(source) => {
                source.fill_base_column_v1(p256, local_column, &mut output.0)?;
            }
        }
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let p256 = self.p256_registration_v1(registration)?;
        if local_column >= registration.segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = zeroed_main_trace_column_v1(registration.segment.trace_size())?;
        let MainP256ScalarTraceViewV1::Bound(source) = self.view else {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        };
        source.fill_aux_column_v1(p256, local_column, &mut output.0)?;
        if output.iter().any(|value| F::canonical(value.0).is_none()) {
            return Err(ZkX509StarkErrorV1::P256Witness);
        }
        Ok(output)
    }
}

/// Fixed-polynomial and opened-row source for the five log-8 scalar buses on
/// the prover side.
struct MainP256ScalarProverConstraintSourceV1<'a> {
    registrations: [MainP256RegistrationBindingV1; P256_SIGNATURE_COUNT_V1],
    source: &'a P256MainBoundSourceV1,
    challenges: P256ScalarBitBusChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
}

impl<'a> MainP256ScalarProverConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        source: &'a P256MainBoundSourceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let registrations = main_p256_scalar_registrations_v1(layout)?;
        let canonical = source.canonical_registrations_v1()?;
        validate_p256_main_registration_order_v1(&canonical)?;
        let challenges = source.post_base_v1()?.p256_scalar();
        challenges
            .validate_v1()
            .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
        let terminals = main_p256_terminal_registrations_v1(&source.terminal_claims_v1()?)?;
        Ok(Self {
            registrations,
            source,
            challenges,
            terminals,
        })
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(
            RegisteredSegmentLayoutV1,
            usize,
            &[F],
        ) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        for matched in self.registrations.iter().copied() {
            let trace_root = goldilocks_primitive_root_v1(matched.main.segment.trace_log2)
                .map_err(map_transparent_error_v1)?;
            for local_column in 0..matched.main.segment.fixed_width {
                let mut column = zeroed_main_trace_column_v1(matched.main.segment.trace_size())?;
                self.source
                    .fill_fixed_column_v1(matched.p256, local_column, &mut column.0)?;
                goldilocks_ifft_v1(&mut column, trace_root).map_err(map_transparent_error_v1)?;
                consume(matched.main, local_column, &column)?;
            }
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1],
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let matched = main_p256_scalar_registration_v1(&self.registrations, registration)?;
        p256_scalar_opened_residues_v1(
            registration,
            opening,
            fixed,
            self.challenges,
            self.terminals
                .get(matched.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }

    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if F::canonical(x.0).is_none() || alphas.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let residues = self.constraint_residues_v1(registration, opening, fixed)?;
        accumulator_quotient_value_v1(registration.segment, x, &residues, alphas)
    }
}

/// Witness-free fixed sampler and opened-row evaluator for all five scalar
/// buses in the production MAIN verifier.
struct MainP256ScalarVerifierConstraintSourceV1<'a> {
    registrations: [MainP256RegistrationBindingV1; P256_SIGNATURE_COUNT_V1],
    common_lde_log2: u8,
    fixed: &'a P256MainVerifierFixedSourceV1,
    challenges: P256ScalarBitBusChallengesV1,
    terminals: [P256TerminalRegistrationV1; P256_SIGNATURE_COUNT_V1],
    fixed_openings:
        [BTreeMap<usize, [F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1]>; P256_SIGNATURE_COUNT_V1],
}

impl<'a> MainP256ScalarVerifierConstraintSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        fixed: &'a P256MainVerifierFixedSourceV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
        claims: &ZkX509P256TerminalClaimsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let registrations = main_p256_scalar_registrations_v1(layout)?;
        if registrations
            .iter()
            .any(|registration| layout.common_lde_log2 < registration.main.segment.lde_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let challenges = post_base.p256_scalar();
        challenges
            .validate_v1()
            .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
        Ok(Self {
            registrations,
            common_lde_log2: layout.common_lde_log2,
            fixed,
            challenges,
            terminals: main_p256_terminal_registrations_v1(claims)?,
            fixed_openings: core::array::from_fn(|_| BTreeMap::new()),
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn next_query_index_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let stride_log2 = self
            .common_lde_log2
            .checked_sub(registration.segment.trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let stride = 1_usize
            .checked_shl(u32::from(stride_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(query_index
            .checked_add(stride)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1()?)
    }

    fn ensure_fixed_openings_v1(
        &mut self,
        matched: MainP256RegistrationBindingV1,
        indices: [usize; 2],
    ) -> Result<(), ZkX509StarkErrorV1> {
        let signature = matched.p256.signature_v1();
        let cache = self
            .fixed_openings
            .get(signature)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let mut missing = indices
            .into_iter()
            .filter(|index| !cache.contains_key(index))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        if cache
            .len()
            .checked_add(missing.len())
            .filter(|count| *count <= VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .is_none()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let sampled = sampled_verifier_generated_fixed_openings_v1::<
            P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1,
        >(
            matched.main.segment,
            self.common_lde_log2,
            &missing,
            |row| {
                let fixed_row: [F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1] = self
                    .fixed
                    .fixed_row_v1(matched.p256, row)?
                    .try_into()
                    .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)?;
                Ok(fixed_row)
            },
        )?;
        let cache = self
            .fixed_openings
            .get_mut(signature)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let mut converted = BTreeMap::new();
        for (index, row) in sampled {
            converted.insert(
                index,
                row.try_into()
                    .map_err(|_: Vec<F>| ZkX509StarkErrorV1::InternalInvariant)?,
            );
        }
        if converted.keys().any(|index| cache.contains_key(index)) {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        cache.extend(converted);
        Ok(())
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let matched = main_p256_scalar_registration_v1(&self.registrations, registration)?;
        if query_index >= self.common_lde_size_v1()?
            || next_query_index >= self.common_lde_size_v1()?
            || next_query_index != self.next_query_index_v1(registration, query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        // Validate every caller-controlled coordinate and opening before the
        // bounded verifier-owned cache is allowed to change.
        self.ensure_fixed_openings_v1(matched, [query_index, next_query_index])?;
        let cache = self
            .fixed_openings
            .get(matched.p256.signature_v1())
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let fixed = cache
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let next_fixed = cache
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if fixed
            .iter()
            .chain(next_fixed)
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        p256_scalar_opened_residues_v1(
            registration,
            opening,
            fixed,
            self.challenges,
            self.terminals
                .get(matched.p256.signature_v1())
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        )
    }
}

fn validate_main_io_registration_v1(
    registration: RegisteredSegmentLayoutV1,
) -> Result<(), ZkX509StarkErrorV1> {
    if registration.segment != SegmentLayoutV1::for_full_io()? {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

fn compile_main_io_public_statement_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<(ZkX509IoStarkStatementV1, usize), ZkX509StarkErrorV1> {
    let plan = compile_zk_x509_main_io_declarations_v1(statement)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let logical_active_rows = plan.logical_active_rows;
    let io_statement = ZkX509IoStarkStatementV1::new(plan.declarations)?;
    if io_active_rows_v1(io_statement.declarations())? != logical_active_rows {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok((io_statement, logical_active_rows))
}

/// Compile the canonical log-18 MAIN I/O fixed schedule exclusively from the
/// verifier's typed public statement.
fn compile_main_io_fixed_schedule_v1(
    layout: SegmentLayoutV1,
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<(ZkX509IoStarkStatementV1, MainIoFixedScheduleV1), ZkX509StarkErrorV1> {
    if layout != SegmentLayoutV1::for_full_io()? {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let (io_statement, logical_active_rows) = compile_main_io_public_statement_v1(statement)?;
    let fixed_schedule =
        MainIoFixedScheduleV1::compile_v1(layout, &io_statement, logical_active_rows)?;
    Ok((io_statement, fixed_schedule))
}

fn compile_main_io_statement_from_source_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    source: &ZkX509MainIoBaseMaterialV1,
    layout: SegmentLayoutV1,
) -> Result<(ZkX509IoStarkStatementV1, MainIoFixedScheduleV1), ZkX509StarkErrorV1> {
    let (io_statement, fixed_schedule) = compile_main_io_fixed_schedule_v1(layout, statement)?;
    if source.declarations.as_slice() != io_statement.declarations()
        || source.logical_active_rows != fixed_schedule.logical_active_rows
        || source.execution.len() != fixed_schedule.logical_active_rows
        || source.sorted.len() != fixed_schedule.logical_active_rows
        || source.witnesses.len() != io_statement.declarations.len()
        || source
            .witnesses
            .iter()
            .zip(io_statement.declarations())
            .any(|(witness, declaration)| witness.declaration != *declaration)
    {
        return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
    }
    let (declarations, execution, sorted) = build_zk_x509_io_base_tables_v1(&source.witnesses)?;
    if declarations != source.declarations
        || execution != source.execution
        || sorted != source.sorted
    {
        return Err(ZkX509StarkErrorV1::IoWitness);
    }
    fixed_schedule.validate_witness_topology_v1(&execution, &sorted)?;
    Ok((io_statement, fixed_schedule))
}

/// Phased byte-memory source for the canonical MAIN log-18 group.
///
/// Construction validates and materializes only challenge-independent columns.
/// Auxiliary columns remain unavailable until the opaque joint post-base token
/// is bound. No API accepts raw I/O challenges.
struct MainIoTraceGroupSourceV1<'a> {
    registration: RegisteredSegmentLayoutV1,
    statement: ZkX509IoStarkStatementV1,
    source: &'a ZkX509MainIoBaseMaterialV1,
    base_columns: Vec<Vec<F>>,
    fixed_columns: Vec<Vec<F>>,
    aux_columns: Option<Vec<Vec<F>>>,
    post_base: Option<ZkX509CredentialMainPostBaseChallengesV1>,
    bind_attempted: bool,
}

impl<'a> MainIoTraceGroupSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        source: &'a ZkX509MainIoBaseMaterialV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::ByteMemory, 0)?;
        validate_main_io_registration_v1(registration)?;
        let (io_statement, fixed_schedule) =
            compile_main_io_statement_from_source_v1(statement, source, registration.segment)?;
        let (base_columns, fixed_columns, execution, sorted) =
            build_io_base_and_fixed_columns_from_schedule_v1(
                &io_statement,
                &source.witnesses,
                &fixed_schedule,
            )?;
        if execution != source.execution || sorted != source.sorted {
            return Err(ZkX509StarkErrorV1::IoWitness);
        }
        validate_io_base_phase_shape_v1(
            registration.segment,
            source.logical_active_rows,
            &base_columns,
            &fixed_columns,
        )?;
        Ok(Self {
            registration,
            statement: io_statement,
            source,
            base_columns,
            fixed_columns,
            aux_columns: None,
            post_base: None,
            bind_attempted: false,
        })
    }

    fn bind_challenges_v1(
        &mut self,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.bind_attempted || self.aux_columns.is_some() || self.post_base.is_some() {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        // Consume the sole phase transition before any fallible work. A failed
        // attempt cannot be retried with another token.
        self.bind_attempted = true;
        self.validate_base_phase_v1()?;
        let challenges = post_base.io();
        let aux_columns = build_io_aux_columns_v1(
            &self.statement,
            &self.source.witnesses,
            challenges,
            self.registration.segment,
            self.source.logical_active_rows,
            &self.source.execution,
            &self.source.sorted,
        )?;
        validate_io_bound_constraints_v1(
            self.registration.segment,
            self.source.logical_active_rows,
            &self.base_columns,
            &aux_columns,
            &self.fixed_columns,
            challenges,
        )?;
        self.aux_columns = Some(aux_columns);
        self.post_base = Some(post_base);
        Ok(())
    }

    fn validate_base_phase_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        validate_io_base_phase_shape_v1(
            self.registration.segment,
            self.source.logical_active_rows,
            &self.base_columns,
            &self.fixed_columns,
        )
    }

    fn validate_bound_phase_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        let aux_columns = self
            .aux_columns
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        let post_base = self
            .post_base
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        validate_io_bound_constraints_v1(
            self.registration.segment,
            self.source.logical_active_rows,
            &self.base_columns,
            aux_columns,
            &self.fixed_columns,
            post_base.io(),
        )
    }

    fn zeroize_private_buffers_v1(&mut self) {
        for column in &mut self.base_columns {
            column.fill(F::ZERO);
            column.clear();
        }
        self.base_columns.clear();
        if let Some(aux_columns) = &mut self.aux_columns {
            for column in aux_columns.iter_mut() {
                column.fill(F::ZERO);
                column.clear();
            }
            aux_columns.clear();
        }
        self.aux_columns = None;
        for column in &mut self.fixed_columns {
            column.clear();
        }
        self.fixed_columns.clear();
        self.post_base = None;
    }
}

impl Drop for MainIoTraceGroupSourceV1<'_> {
    fn drop(&mut self) {
        self.zeroize_private_buffers_v1();
    }
}

impl MainTraceGroupSourceV1 for MainIoTraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if registration != self.registration {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        copied_matrix_column_v1(
            &self.base_columns,
            IO_BASE_WIDTH,
            self.registration.segment.trace_size(),
            local_column,
        )
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if registration != self.registration {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let aux_columns = self
            .aux_columns
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        copied_matrix_column_v1(
            aux_columns,
            IO_AUX_WIDTH,
            self.registration.segment.trace_size(),
            local_column,
        )
    }
}

/// MAIN I/O fixed-polynomial and composition source for the log-18 prover.
///
/// It uses the same statement-only fixed compiler as the verifier and
/// independently revalidates the prover's witness topology against that
/// schedule. `MainIoTraceGroupSourceV1` performs the same check before any base
/// column can enter the MAIN commitment session.
struct MainIoProverConstraintSourceV1 {
    registration: RegisteredSegmentLayoutV1,
    challenges: ZkX509IoChallengesV1,
    fixed_schedule: MainIoFixedScheduleV1,
}

impl MainIoProverConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        source: &ZkX509MainIoBaseMaterialV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::ByteMemory, 0)?;
        validate_main_io_registration_v1(registration)?;
        let (_, fixed_schedule) =
            compile_main_io_statement_from_source_v1(statement, source, registration.segment)?;
        let challenges = post_base.io();
        challenges
            .validate()
            .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
        Ok(Self {
            registration,
            challenges,
            fixed_schedule,
        })
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(usize, &[F]) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let trace_root = goldilocks_primitive_root_v1(self.registration.segment.trace_log2)
            .map_err(map_transparent_error_v1)?;
        for local_column in 0..IO_FIXED_WIDTH {
            let mut column = Vec::new();
            column
                .try_reserve_exact(self.registration.segment.trace_size())
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            for row in 0..self.registration.segment.trace_size() {
                column.push(self.fixed_schedule.fixed_row_v1(row)?[local_column]);
            }
            let mut coefficients = ZeroizingMainTraceColumnV1(column);
            goldilocks_ifft_v1(&mut coefficients, trace_root).map_err(map_transparent_error_v1)?;
            consume(local_column, &coefficients)?;
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F; IO_FIXED_WIDTH],
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        if registration != self.registration
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .chain(fixed)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        io_constraint_residues_v1(
            registration.segment,
            self.fixed_schedule.logical_active_rows,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            fixed,
            self.challenges,
        )
    }

    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F; IO_FIXED_WIDTH],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if F::canonical(x.0).is_none() || alphas.len() != registration.segment.constraint_count {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let residues = self.constraint_residues_v1(registration, opening, fixed)?;
        accumulator_quotient_value_v1(registration.segment, x, &residues, alphas)
    }
}

/// Verifier-generated MAIN I/O fixed openings and opened-row evaluator.
///
/// Fixed rows come only from the typed public statement. The cache is bounded
/// to the two coordinates needed for each canonical query, and all
/// caller-controlled coordinates and opened values are checked before that
/// cache can change.
struct MainIoVerifierConstraintSourceV1 {
    registration: RegisteredSegmentLayoutV1,
    common_lde_log2: u8,
    challenges: ZkX509IoChallengesV1,
    fixed_schedule: MainIoFixedScheduleV1,
    fixed_openings: BTreeMap<usize, [F; IO_FIXED_WIDTH]>,
}

impl MainIoVerifierConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::ByteMemory, 0)?;
        validate_main_io_registration_v1(registration)?;
        if layout.common_lde_log2 < registration.segment.lde_log2 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let (_, fixed_schedule) =
            compile_main_io_fixed_schedule_v1(registration.segment, statement)?;
        let challenges = post_base.io();
        challenges
            .validate()
            .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
        Ok(Self {
            registration,
            common_lde_log2: layout.common_lde_log2,
            challenges,
            fixed_schedule,
            fixed_openings: BTreeMap::new(),
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn next_query_index_v1(&self, query_index: usize) -> Result<usize, ZkX509StarkErrorV1> {
        let stride_log2 = self
            .common_lde_log2
            .checked_sub(self.registration.segment.trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let stride = 1_usize
            .checked_shl(u32::from(stride_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(query_index
            .checked_add(stride)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            % self.common_lde_size_v1()?)
    }

    fn ensure_fixed_openings_v1(&mut self, indices: [usize; 2]) -> Result<(), ZkX509StarkErrorV1> {
        let mut missing = indices
            .into_iter()
            .filter(|index| !self.fixed_openings.contains_key(index))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        if self
            .fixed_openings
            .len()
            .checked_add(missing.len())
            .filter(|count| *count <= VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .is_none()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let sampled = sampled_verifier_generated_fixed_openings_v1(
            self.registration.segment,
            self.common_lde_log2,
            &missing,
            |row| self.fixed_schedule.fixed_row_v1(row),
        )?;
        for (index, row) in sampled {
            let row: [F; IO_FIXED_WIDTH] = row
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
            if self.fixed_openings.insert(index, row).is_some() {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
        }
        Ok(())
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        if registration != self.registration
            || query_index >= self.common_lde_size_v1()?
            || next_query_index >= self.common_lde_size_v1()?
            || next_query_index != self.next_query_index_v1(query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || F::canonical(x.0).is_none()
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        self.ensure_fixed_openings_v1([query_index, next_query_index])?;
        let current = self
            .fixed_openings
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let next = self
            .fixed_openings
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if current
            .iter()
            .chain(next)
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        io_constraint_residues_v1(
            registration.segment,
            self.fixed_schedule.logical_active_rows,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            current,
            self.challenges,
        )
    }
}

/// Challenge-independent and challenge-bound native projection columns for
/// the canonical MAIN log-15 group.
///
/// Base columns are available immediately. Auxiliary columns remain
/// inaccessible until the transcript-derived challenges are bound.
struct MainProjectionTraceGroupSourceV1<'a> {
    registration: RegisteredSegmentLayoutV1,
    trace: &'a ZkX509ProjectionTraceV1,
    aux: Option<ZkX509ProjectionAuxTraceV1>,
}

impl<'a> MainProjectionTraceGroupSourceV1<'a> {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        trace: &'a ZkX509ProjectionTraceV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::Projection, 0)?;
        validate_main_projection_registration_v1(registration)?;
        let fixed = compile_zk_x509_projection_fixed_trace_v1(statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        if trace.base.rows.len() != registration.segment.trace_size()
            || trace.fixed != fixed
            || trace
                .base
                .rows
                .iter()
                .flatten()
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProjectionWitness);
        }
        Ok(Self {
            registration,
            trace,
            aux: None,
        })
    }

    fn bind_challenges_v1(
        &mut self,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.aux.is_some() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let aux = build_zk_x509_projection_aux_trace_v1(
            &self.trace.base,
            &self.trace.fixed,
            post_base.projection(),
        )?;
        if aux.rows.len() != self.registration.segment.trace_size() {
            return Err(ZkX509StarkErrorV1::ProjectionWitness);
        }
        self.aux = Some(aux);
        Ok(())
    }

    fn zeroize_private_buffers_v1(&mut self) {
        if let Some(aux) = &mut self.aux {
            for row in &mut aux.rows {
                row.fill(F::ZERO);
            }
            aux.rows.clear();
        }
        self.aux = None;
    }
}

impl Drop for MainProjectionTraceGroupSourceV1<'_> {
    fn drop(&mut self) {
        self.zeroize_private_buffers_v1();
    }
}

impl MainTraceGroupSourceV1 for MainProjectionTraceGroupSourceV1<'_> {
    fn native_base_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if registration != self.registration {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        copied_array_column_v1(&self.trace.base.rows, local_column)
    }

    fn native_aux_column_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        if registration != self.registration {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let aux = self
            .aux
            .as_ref()
            .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
        copied_array_column_v1(&aux.rows, local_column)
    }
}

/// Projection fixed-polynomial and composition source for the MAIN prover.
///
/// Fixed columns are interpolated one at a time and handed to the caller
/// through a scoped zeroizing buffer. This is deliberately separate from the
/// bounded sampled-opening verifier below: a full prover traversal cannot
/// consume, or exhaust, the verifier's 116-opening cache.
struct MainProjectionProverConstraintSourceV1 {
    registration: RegisteredSegmentLayoutV1,
    challenges: ZkX509ProjectionChallengesV1,
    fixed_rows: Vec<[F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1]>,
}

impl MainProjectionProverConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::Projection, 0)?;
        validate_main_projection_registration_v1(registration)?;
        let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        if fixed_rows.len() != registration.segment.trace_size() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            registration,
            challenges: post_base.projection(),
            fixed_rows,
        })
    }

    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(usize, &[F]) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        let trace_root = goldilocks_primitive_root_v1(self.registration.segment.trace_log2)
            .map_err(map_transparent_error_v1)?;
        for local_column in 0..ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1 {
            let mut coefficients = copied_array_column_v1(&self.fixed_rows, local_column)?;
            goldilocks_ifft_v1(&mut coefficients, trace_root).map_err(map_transparent_error_v1)?;
            consume(local_column, &coefficients)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &[F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if registration != self.registration
            || alphas.len() != registration.segment.constraint_count
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        projection_quotient_value_v1(
            registration.segment,
            x,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            fixed,
            self.challenges,
            alphas,
        )
    }
}

/// Test-only verifier-minted projection fixed opening.
///
/// The registration and both query coordinates travel with the sampled rows.
/// Constraint evaluation revalidates all four fields against the source cache,
/// allowing tests to prove that caller-provided rows cannot be substituted for
/// verifier reconstruction. Production evaluation never manufactures or
/// exports this capability.
#[cfg(test)]
#[derive(Debug)]
struct MainProjectionVerifierFixedOpeningV1 {
    registration: RegisteredSegmentLayoutV1,
    query_index: usize,
    next_query_index: usize,
    current: [F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
    next: [F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
}

/// Verifier-generated projection fixed rows and opened constraint evaluator.
///
/// The fixed trace is compiled exclusively from the public statement and
/// sampled directly on MAIN's common coset. It never reads prover-native
/// projection material. This source is verifier-only and bounded to the exact
/// number of fixed openings required by the canonical query schedule.
struct MainProjectionVerifierConstraintSourceV1 {
    registration: RegisteredSegmentLayoutV1,
    common_lde_log2: u8,
    challenges: ZkX509ProjectionChallengesV1,
    fixed_rows: Vec<[F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1]>,
    fixed_openings: BTreeMap<usize, [F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1]>,
}

impl MainProjectionVerifierConstraintSourceV1 {
    fn for_main_v1(
        layout: &AggregateProofLayoutV1,
        statement: &IrohaZkX509StarkP256StatementV1,
        post_base: ZkX509CredentialMainPostBaseChallengesV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::Projection, 0)?;
        validate_main_projection_registration_v1(registration)?;
        let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        if fixed_rows.len() != registration.segment.trace_size()
            || layout.common_lde_log2 < registration.segment.lde_log2
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            registration,
            common_lde_log2: layout.common_lde_log2,
            challenges: post_base.projection(),
            fixed_rows,
            fixed_openings: BTreeMap::new(),
        })
    }

    fn common_lde_size_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    fn next_query_index_v1(&self, query_index: usize) -> Result<usize, ZkX509StarkErrorV1> {
        let stride_log2 = self
            .common_lde_log2
            .checked_sub(self.registration.segment.trace_log2)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let stride = 1_usize
            .checked_shl(u32::from(stride_log2))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let next = query_index
            .checked_add(stride)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        Ok(next % self.common_lde_size_v1()?)
    }

    fn ensure_fixed_openings_v1(&mut self, indices: [usize; 2]) -> Result<(), ZkX509StarkErrorV1> {
        let mut missing = indices
            .into_iter()
            .filter(|index| !self.fixed_openings.contains_key(index))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        if self
            .fixed_openings
            .len()
            .checked_add(missing.len())
            .filter(|count| *count <= VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
            .is_none()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let sampled = sampled_verifier_generated_fixed_openings_v1(
            self.registration.segment,
            self.common_lde_log2,
            &missing,
            |row| {
                self.fixed_rows
                    .get(row)
                    .copied()
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)
            },
        )?;
        for (index, row) in sampled {
            let row: [F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1] = row
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
            if self.fixed_openings.insert(index, row).is_some() {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn verifier_fixed_opening_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
    ) -> Result<MainProjectionVerifierFixedOpeningV1, ZkX509StarkErrorV1> {
        if registration != self.registration
            || query_index >= self.common_lde_size_v1()?
            || next_query_index >= self.common_lde_size_v1()?
            || next_query_index != self.next_query_index_v1(query_index)?
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        self.ensure_fixed_openings_v1([query_index, next_query_index])?;
        Ok(MainProjectionVerifierFixedOpeningV1 {
            registration,
            query_index,
            next_query_index,
            current: self
                .fixed_openings
                .get(&query_index)
                .copied()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
            next: self
                .fixed_openings
                .get(&next_query_index)
                .copied()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
        })
    }

    #[cfg(test)]
    fn constraint_residues_from_fixed_opening_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &MainProjectionVerifierFixedOpeningV1,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        if registration != self.registration
            || query_index >= self.common_lde_size_v1()?
            || next_query_index != self.next_query_index_v1(query_index)?
            || fixed.registration != registration
            || fixed.query_index != query_index
            || fixed.next_query_index != next_query_index
            || self.fixed_openings.get(&query_index) != Some(&fixed.current)
            || self.fixed_openings.get(&next_query_index) != Some(&fixed.next)
            || fixed
                .current
                .iter()
                .chain(&fixed.next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        projection_constraint_residues_v1(
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            &fixed.current,
            self.challenges,
        )
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        if registration != self.registration
            || query_index >= self.common_lde_size_v1()?
            || next_query_index >= self.common_lde_size_v1()?
            || next_query_index != self.next_query_index_v1(query_index)?
            || opening.base_current.len() != registration.segment.base_width
            || opening.base_next.len() != registration.segment.base_width
            || opening.aux_current.len() != registration.segment.aux_width
            || opening.aux_next.len() != registration.segment.aux_width
            || opening
                .base_current
                .iter()
                .chain(opening.base_next)
                .chain(opening.aux_current)
                .chain(opening.aux_next)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let root =
            goldilocks_primitive_root_v1(self.common_lde_log2).map_err(map_transparent_error_v1)?;
        let expected_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        if x != expected_x {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }

        // Only the concrete verifier source can populate or read this cache.
        // All caller-controlled coordinates have already been validated, so a
        // rejected query cannot consume one of the bounded sampled openings.
        self.ensure_fixed_openings_v1([query_index, next_query_index])?;
        let current = self
            .fixed_openings
            .get(&query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let next = self
            .fixed_openings
            .get(&next_query_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        if current
            .iter()
            .chain(next)
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        projection_constraint_residues_v1(
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            current,
            self.challenges,
        )
    }
}

/// Closed association between one implementation and one canonical MAIN
/// native-log group. No proof or caller-provided integer selects a group.
enum MainTraceGroupProviderV1<'a> {
    Log5(&'a mut MainP256Log5TraceGroupSourceV1<'a>),
    P256Scalar(&'a mut MainP256ScalarTraceGroupSourceV1<'a>),
    Log15(&'a mut dyn MainTraceGroupSourceV1),
    Log16(&'a mut MainP256Log16TraceGroupSourceV1<'a>),
    Log18(&'a mut dyn MainTraceGroupSourceV1),
    Log19(&'a mut dyn MainTraceGroupSourceV1),
    #[cfg(test)]
    TestLog5(&'a mut dyn MainTraceGroupSourceV1),
    #[cfg(test)]
    TestLog8(&'a mut dyn MainTraceGroupSourceV1),
    #[cfg(test)]
    TestLog16(&'a mut dyn MainTraceGroupSourceV1),
}

impl MainTraceGroupProviderV1<'_> {
    fn native_trace_log2_v1(&self) -> u8 {
        match self {
            Self::Log5(_) => 5,
            Self::P256Scalar(_) => 8,
            Self::Log15(_) => 15,
            Self::Log16(_) => 16,
            Self::Log18(_) => 18,
            Self::Log19(_) => 19,
            #[cfg(test)]
            Self::TestLog5(_) => 5,
            #[cfg(test)]
            Self::TestLog8(_) => 8,
            #[cfg(test)]
            Self::TestLog16(_) => 16,
        }
    }

    fn source_mut_v1(&mut self) -> &mut dyn MainTraceGroupSourceV1 {
        match self {
            Self::Log5(source) => *source,
            Self::P256Scalar(source) => *source,
            Self::Log16(source) => *source,
            Self::Log15(source) | Self::Log18(source) | Self::Log19(source) => *source,
            #[cfg(test)]
            Self::TestLog5(source) => *source,
            #[cfg(test)]
            Self::TestLog8(source) => *source,
            #[cfg(test)]
            Self::TestLog16(source) => *source,
        }
    }
}

/// Closed association between one verifier-safe opened-row implementation and
/// one canonical MAIN native-log group.
///
/// Production variants always name a concrete verifier implementation. The
/// dynamic variants only exist in unit tests so malformed-provider behavior
/// remains testable without creating a production extension point capable of
/// fabricating fixed openings.
enum MainOpenedGroupProviderV1<'a> {
    Log5(&'a mut MainP256Log5VerifierConstraintSourceV1<'a>),
    Log16(&'a mut MainP256Log16VerifierConstraintSourceV1<'a>),
    Log19(&'a mut MainLog19VerifierConstraintSourceV1),
    Io(&'a mut MainIoVerifierConstraintSourceV1),
    Projection(&'a mut MainProjectionVerifierConstraintSourceV1),
    P256Scalar(&'a mut MainP256ScalarVerifierConstraintSourceV1<'a>),
    #[cfg(test)]
    TestLog5(&'a mut dyn MainOpenedConstraintTestSourceV1),
    #[cfg(test)]
    TestLog8(&'a mut dyn MainOpenedConstraintTestSourceV1),
    #[cfg(test)]
    TestLog15(&'a mut dyn MainOpenedConstraintTestSourceV1),
    #[cfg(test)]
    TestLog16(&'a mut dyn MainOpenedConstraintTestSourceV1),
    #[cfg(test)]
    TestLog18(&'a mut dyn MainOpenedConstraintTestSourceV1),
    #[cfg(test)]
    TestLog19(&'a mut dyn MainOpenedConstraintTestSourceV1),
}

impl MainOpenedGroupProviderV1<'_> {
    fn native_trace_log2_v1(&self) -> u8 {
        match self {
            Self::Log5(_) => 5,
            Self::Log16(_) => 16,
            Self::Log19(_) => 19,
            Self::Io(_) => 18,
            Self::Projection(_) => 15,
            Self::P256Scalar(_) => 8,
            #[cfg(test)]
            Self::TestLog5(_) => 5,
            #[cfg(test)]
            Self::TestLog8(_) => 8,
            #[cfg(test)]
            Self::TestLog15(_) => 15,
            #[cfg(test)]
            Self::TestLog16(_) => 16,
            #[cfg(test)]
            Self::TestLog18(_) => 18,
            #[cfg(test)]
            Self::TestLog19(_) => 19,
        }
    }

    fn constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        match self {
            Self::Log5(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            Self::Log16(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            Self::Log19(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            Self::Io(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            Self::Projection(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            Self::P256Scalar(source) => source.constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            ),
            #[cfg(test)]
            Self::TestLog5(source)
            | Self::TestLog8(source)
            | Self::TestLog15(source)
            | Self::TestLog16(source)
            | Self::TestLog18(source)
            | Self::TestLog19(source) => {
                let fixed =
                    source.fixed_opened_rows_v1(registration, query_index, next_query_index)?;
                if fixed.current.len() != registration.segment.fixed_width
                    || fixed.next.len() != registration.segment.fixed_width
                    || fixed
                        .current
                        .iter()
                        .chain(&fixed.next)
                        .any(|value| F::canonical(value.0).is_none())
                {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                source.constraint_residues_v1(registration, query_index, x, opening, &fixed)
            }
        }
    }
}

/// Verify the exact canonical byte-memory segmented proof.
pub(crate) fn verify_zk_x509_io_segmented_stark_v1(
    statement: &ZkX509IoStarkStatementV1,
    proof_bytes: &[u8],
) -> Result<(), ZkX509StarkErrorV1> {
    validate_declarations_v1(&statement.declarations)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let active_rows = io_active_rows_v1(&statement.declarations)?;
    if active_rows > byte_memory_capacity_v1().map_err(|_| ZkX509StarkErrorV1::InvalidStatement)? {
        return Err(ZkX509StarkErrorV1::InvalidStatement);
    }
    let layout = SegmentLayoutV1::for_io(active_rows)?;
    layout.validate()?;
    let layouts = [layout];
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&layouts)?;
    let proof = decode_zk_x509_segmented_stark_proof_v1(proof_bytes, &aggregate_layout)?;
    if proof.trace_groups.is_empty() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let public_digest = io_public_digest_v1(statement)?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:io-aggregate-layout:v1",
        &aggregate_layout,
    )?;
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let io_challenges =
        derive_zk_x509_io_challenges_v1(&mut transcript).map_err(map_transparent_error_v1)?;
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &proof.composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        &proof.fri_mask_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let shared_layout = aggregate_layout.as_shared()?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &proof.deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let deep_mixes = aggregate_deep_lane_mixes_v1(&mixes, &aggregate_layout)?;
    let (fri_betas, terminal_fields) = aggregate::verify_fri_commitments_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &mut transcript,
    )
    .map_err(map_aggregate_error_v1)?;
    let grinding_state = transcript.state();
    verify_grinding_nonce_v1(
        &grinding_state,
        ZK_X509_GRINDING_BITS_V1,
        proof.grinding_nonce,
    )
    .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    absorb_grinding_nonce_v1(&mut transcript, proof.grinding_nonce)?;
    let expected_indices = query_indices_v1(&transcript, &aggregate_layout)?;

    aggregate::verify_all_merkle_openings_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &expected_indices,
    )
    .map_err(map_aggregate_error_v1)?;

    let fixed_schedule = MainIoFixedScheduleV1::compile_v1(layout, statement, active_rows)?;
    let fixed_columns = fixed_schedule.fixed_columns_v1()?;
    let fixed_lde = fixed_lde_columns_v1(&fixed_columns, layout)?;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut evaluator = IoOpenedRowEvaluatorV1 {
        aggregate_layout: &aggregate_layout,
        layout,
        logical_active_rows: active_rows,
        fixed_lde: &fixed_lde,
        io_challenges,
        alphas: &alphas[0],
        mixes: &mixes[0],
        lde_root,
    };
    aggregate::verify_opened_query_relations_with_deep_v1(
        &proof.aggregate,
        &proof.deep,
        deep_point,
        &deep_mixes,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        &expected_indices,
        &fri_betas,
        &terminal_fields,
        &mut evaluator,
    )
    .map_err(map_aggregate_error_v1)
}

/// Verify the exact canonical registered strict-DER proof.
#[allow(dead_code)]
pub(crate) fn verify_zk_x509_der_segmented_stark_v1(
    shape: &ZkX509DerStarkShapeV1,
    proof_bytes: &[u8],
) -> Result<(), ZkX509StarkErrorV1> {
    shape
        .validate()
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(shape.clone())
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    let layout = SegmentLayoutV1::for_der(schedule.active_rows())?;
    layout.validate()?;
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&[layout])?;
    let (claims, aggregate_proof_bytes) = decode_der_segmented_proof_envelope_v1(proof_bytes)?;
    let proof = decode_zk_x509_segmented_stark_proof_v1(aggregate_proof_bytes, &aggregate_layout)?;
    if proof.trace_groups.len() != 1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }

    let public_digest = der_public_digest_v1(shape)?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:der-aggregate-layout:v1",
        &aggregate_layout,
    )?;
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let challenges = derive_der_challenges_v1(&mut transcript)?;
    let public = derive_zk_x509_der_stark_public_terminals_v1(shape, challenges)
        .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    absorb_der_terminal_claims_v1(&mut transcript, claims)?;
    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &proof.composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        &proof.fri_mask_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let shared_layout = aggregate_layout.as_shared()?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &proof.deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let deep_mixes = aggregate_deep_lane_mixes_v1(&mixes, &aggregate_layout)?;
    let (fri_betas, terminal_fields) = aggregate::verify_fri_commitments_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &mut transcript,
    )
    .map_err(map_aggregate_error_v1)?;
    let grinding_state = transcript.state();
    verify_grinding_nonce_v1(
        &grinding_state,
        ZK_X509_GRINDING_BITS_V1,
        proof.grinding_nonce,
    )
    .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    absorb_grinding_nonce_v1(&mut transcript, proof.grinding_nonce)?;
    let expected_indices = query_indices_v1(&transcript, &aggregate_layout)?;
    aggregate::verify_all_merkle_openings_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &expected_indices,
    )
    .map_err(map_aggregate_error_v1)?;

    let next_stride = aggregate_layout
        .trace_groups
        .first()
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
        .next_stride(aggregate_layout.common_lde_log2)?;
    let fixed_indices = expected_indices
        .iter()
        .flat_map(|index| {
            [
                *index,
                (*index + next_stride) % aggregate_layout.common_lde_size(),
            ]
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let fixed_openings = der_fixed_openings_v1(&schedule, layout, &fixed_indices)?;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut evaluator = DerOpenedRowEvaluatorV1 {
        aggregate_layout: &aggregate_layout,
        layout,
        fixed_openings: &fixed_openings,
        challenges,
        public,
        claims,
        alphas: &alphas[0],
        mixes: &mixes[0],
        lde_root,
    };
    aggregate::verify_opened_query_relations_with_deep_v1(
        &proof.aggregate,
        &proof.deep,
        deep_point,
        &deep_mixes,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        &expected_indices,
        &fri_betas,
        &terminal_fields,
        &mut evaluator,
    )
    .map_err(map_aggregate_error_v1)
}

/// Verify the exact canonical registered projection proof.
pub(crate) fn verify_zk_x509_projection_segmented_stark_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    proof_bytes: &[u8],
) -> Result<(), ZkX509StarkErrorV1> {
    let layout = SegmentLayoutV1::for_projection()?;
    layout.validate()?;
    let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(statement)
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
    if fixed_rows.len() != layout.trace_size() {
        return Err(ZkX509StarkErrorV1::InvalidStatement);
    }
    let aggregate_layout = AggregateProofLayoutV1::for_segments(&[layout])?;
    let proof = decode_zk_x509_segmented_stark_proof_v1(proof_bytes, &aggregate_layout)?;
    if proof.trace_groups.len() != 1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let public_digest = projection_public_digest_v1(statement)?;
    let mut transcript = new_transcript_v1(&public_digest)?;
    absorb_aggregate_layout_v1(
        &mut transcript,
        b"iroha:privacy:zk-x509:projection-aggregate-layout:v1",
        &aggregate_layout,
    )?;
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let projection_challenges = derive_projection_challenges_v1(&mut transcript)?;
    aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &proof.trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let alphas = derive_constraint_alphas_v1(&mut transcript, &aggregate_layout)?;
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &proof.composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        &proof.fri_mask_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let shared_layout = aggregate_layout.as_shared()?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &proof.deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &aggregate_layout)?;
    let deep_mixes = aggregate_deep_lane_mixes_v1(&mixes, &aggregate_layout)?;
    let (fri_betas, terminal_fields) = aggregate::verify_fri_commitments_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &mut transcript,
    )
    .map_err(map_aggregate_error_v1)?;
    let grinding_state = transcript.state();
    verify_grinding_nonce_v1(
        &grinding_state,
        ZK_X509_GRINDING_BITS_V1,
        proof.grinding_nonce,
    )
    .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    absorb_grinding_nonce_v1(&mut transcript, proof.grinding_nonce)?;
    let expected_indices = query_indices_v1(&transcript, &aggregate_layout)?;
    aggregate::verify_all_merkle_openings_v1(
        &proof,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &expected_indices,
    )
    .map_err(map_aggregate_error_v1)?;

    let fixed_columns = transpose_array_rows_v1(&fixed_rows)?;
    let fixed_lde = fixed_lde_columns_v1(&fixed_columns, layout)?;
    let lde_root =
        goldilocks_primitive_root_v1(layout.lde_log2).map_err(map_transparent_error_v1)?;
    let mut evaluator = ProjectionOpenedRowEvaluatorV1 {
        aggregate_layout: &aggregate_layout,
        layout,
        fixed_lde: &fixed_lde,
        challenges: projection_challenges,
        alphas: &alphas[0],
        mixes: &mixes[0],
        lde_root,
    };
    aggregate::verify_opened_query_relations_with_deep_v1(
        &proof.aggregate,
        &proof.deep,
        deep_point,
        &deep_mixes,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        &expected_indices,
        &fri_betas,
        &terminal_fields,
        &mut evaluator,
    )
    .map_err(map_aggregate_error_v1)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{Mutex, OnceLock},
    };

    use rand::{RngCore, SeedableRng as _, rngs::StdRng};
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::privacy_engines::zk_x509::{
        accumulator_air::{
            ZkX509CaAccumulatorStatementV1, ZkX509CaAccumulatorWitnessV1,
            build_ca_accumulator_trace_v1,
        },
        der_air::ZkX509DerEkuV1,
        der_stark::ZkX509DerStarkPrivateShapeV1,
        io_air::ZkX509IoEndpointV1,
        merkle::{
            ZK_X509_CA_SPKI_DER_BYTES_V1, ca_membership_path_from_complete_spkis_v1,
            ca_root_from_complete_spkis_v1,
        },
        p256_aggregate_adapter::p256_main_base_source_fixture_for_test_v1,
        sha_call_bus_stark::ZkX509ShaCallPublicShapeV1,
    };
    use iroha_data_model::{
        ChainId,
        privacy::{
            PrivacyAttributeDigestV1, PrivacyCertificateKeyDigestV1, PrivacyChallengeV1,
            PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyNullifierV1,
            PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyRootV1,
            PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
            PrivacyVerifierDigestV1, PrivacyX509ExtendedKeyUsageV1,
            PrivacyZkX509CertificatePolicyRecordDigestV1, PrivacyZkX509CrlRecordDigestV1,
            PrivacyZkX509TrustAnchorRecordDigestV1,
        },
    };

    static PROOF_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn proof_guard() -> std::sync::MutexGuard<'static, ()> {
        PROOF_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("zk-X509 STARK proof mutex")
    }

    fn extension_v1(value: F) -> E {
        E::from_base(value)
    }

    const TERMINAL_TEST_HEADER_BYTES_V1: usize = 12;
    const TERMINAL_TEST_RECORD_BYTES_V1: usize = 16;
    const TERMINAL_TEST_VALUE_OFFSET_V1: usize = 8;
    const TEST_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [0x93; 32];

    fn overwrite_main_terminal_record_value_v1(
        encoded: &mut [u8],
        frame_offset: usize,
        record: usize,
        value: F,
    ) {
        let start = frame_offset
            + TERMINAL_TEST_HEADER_BYTES_V1
            + record * TERMINAL_TEST_RECORD_BYTES_V1
            + TERMINAL_TEST_VALUE_OFFSET_V1;
        encoded[start..start + 8].copy_from_slice(&value.0.to_be_bytes());
    }

    #[test]
    fn main_transcript_is_release_only_and_domain_separated() {
        let public_digest = [0x71; 32];
        assert!(matches!(
            new_main_transcript_after_profile_validation_v1(&public_digest, [0_u8; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let first_release_digest = [0xa5; 32];
        let second_release_digest = [0x5a; 32];
        let first =
            new_main_transcript_after_profile_validation_v1(&public_digest, first_release_digest)
                .expect("candidate release transcript");
        let second =
            new_main_transcript_after_profile_validation_v1(&public_digest, second_release_digest)
                .expect("distinct candidate release transcript");
        let changed_public =
            new_main_transcript_after_profile_validation_v1(&[0x72; 32], first_release_digest)
                .expect("public-bound release transcript");
        let focused = new_transcript_v1(&public_digest).expect("focused transcript");

        assert_ne!(first.state(), second.state());
        assert_ne!(first.state(), changed_public.state());
        assert_ne!(first.state(), focused.state());
    }

    #[derive(Default)]
    struct MockMainTraceGroupSourceV1 {
        short_base_column: bool,
        noncanonical_aux_column: bool,
        short_fixed_row: bool,
        noncanonical_fixed_row: bool,
        short_residues: bool,
        noncanonical_residues: bool,
    }

    fn mock_main_column_value_v1(
        registration: RegisteredSegmentLayoutV1,
        local_column: usize,
        row: usize,
        aux: bool,
    ) -> F {
        F(u64::from(registration.segment.adapter.wire()) * 1_000_000
            + u64::from(registration.segment.instance) * 10_000
            + u64::try_from(local_column).expect("registered width fits u64") * 101
            + u64::try_from(row % 97).expect("small row residue")
            + u64::from(u8::from(aux)))
    }

    fn mock_main_residue_value_v1(
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed: &MainFixedOpenedRowsV1,
    ) -> F {
        let opened = opening
            .base_current
            .first()
            .copied()
            .unwrap_or(F::ZERO)
            .add(opening.base_next.first().copied().unwrap_or(F::ZERO))
            .add(opening.aux_current.first().copied().unwrap_or(F::ZERO))
            .add(opening.aux_next.first().copied().unwrap_or(F::ZERO))
            .add(fixed.current.first().copied().unwrap_or(F::ZERO))
            .add(fixed.next.first().copied().unwrap_or(F::ZERO));
        opened
            .add(F(u64::from(registration.segment.adapter.wire())))
            .add(F(u64::from(registration.segment.instance)))
            .add(F(u64::try_from(query_index).expect("log25 query fits u64")))
            .add(F(x.0 % 257))
    }

    fn mock_main_fixed_row_v1(
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
    ) -> Vec<F> {
        (0..registration.segment.fixed_width)
            .map(|column| {
                F(u64::from(registration.segment.adapter.wire()) * 1_000_000
                    + u64::from(registration.segment.instance) * 10_000
                    + u64::try_from(column).expect("registered width fits u64") * 101
                    + u64::try_from(query_index % 97).expect("small query residue"))
            })
            .collect()
    }

    impl MainTraceGroupSourceV1 for MockMainTraceGroupSourceV1 {
        fn native_base_column_v1(
            &mut self,
            registration: RegisteredSegmentLayoutV1,
            local_column: usize,
        ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
            if local_column >= registration.segment.base_width {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let length = registration
                .segment
                .trace_size()
                .checked_sub(usize::from(u8::from(self.short_base_column)))
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            Ok(ZeroizingMainTraceColumnV1(
                (0..length)
                    .map(|row| mock_main_column_value_v1(registration, local_column, row, false))
                    .collect(),
            ))
        }

        fn native_aux_column_v1(
            &mut self,
            registration: RegisteredSegmentLayoutV1,
            local_column: usize,
        ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
            if local_column >= registration.segment.aux_width {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let mut column = (0..registration.segment.trace_size())
                .map(|row| mock_main_column_value_v1(registration, local_column, row, true))
                .collect::<Vec<_>>();
            if self.noncanonical_aux_column {
                column[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
            }
            Ok(ZeroizingMainTraceColumnV1(column))
        }
    }

    impl MainOpenedConstraintTestSourceV1 for MockMainTraceGroupSourceV1 {
        fn fixed_opened_rows_v1(
            &mut self,
            registration: RegisteredSegmentLayoutV1,
            query_index: usize,
            next_query_index: usize,
        ) -> Result<MainFixedOpenedRowsV1, ZkX509StarkErrorV1> {
            let mut fixed = MainFixedOpenedRowsV1 {
                current: mock_main_fixed_row_v1(registration, query_index),
                next: mock_main_fixed_row_v1(registration, next_query_index),
            };
            if self.short_fixed_row {
                fixed.current.pop();
            }
            if self.noncanonical_fixed_row && !fixed.next.is_empty() {
                fixed.next[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
            }
            Ok(fixed)
        }

        fn constraint_residues_v1(
            &mut self,
            registration: RegisteredSegmentLayoutV1,
            query_index: usize,
            x: F,
            opening: RegisteredOpenedRowsV1<'_>,
            fixed: &MainFixedOpenedRowsV1,
        ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
            let count = registration
                .segment
                .constraint_count
                .checked_sub(usize::from(u8::from(self.short_residues)))
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues =
                vec![
                    mock_main_residue_value_v1(registration, query_index, x, opening, fixed);
                    count
                ];
            if self.noncanonical_residues && !residues.is_empty() {
                residues[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
            }
            Ok(residues)
        }
    }

    fn mock_main_group_providers_v1<'a>(
        sources: [&'a mut MockMainTraceGroupSourceV1; FULL_PROFILE_TRACE_GROUPS_V1],
    ) -> Vec<MainTraceGroupProviderV1<'a>> {
        let [log5, log8, log15, log16, log18, log19] = sources;
        vec![
            MainTraceGroupProviderV1::TestLog5(log5),
            MainTraceGroupProviderV1::TestLog8(log8),
            MainTraceGroupProviderV1::Log15(log15),
            MainTraceGroupProviderV1::TestLog16(log16),
            MainTraceGroupProviderV1::Log18(log18),
            MainTraceGroupProviderV1::Log19(log19),
        ]
    }

    fn mock_main_opened_group_providers_v1<'a>(
        sources: [&'a mut MockMainTraceGroupSourceV1; FULL_PROFILE_TRACE_GROUPS_V1],
    ) -> Vec<MainOpenedGroupProviderV1<'a>> {
        let [log5, log8, log15, log16, log18, log19] = sources;
        vec![
            MainOpenedGroupProviderV1::TestLog5(log5),
            MainOpenedGroupProviderV1::TestLog8(log8),
            MainOpenedGroupProviderV1::TestLog15(log15),
            MainOpenedGroupProviderV1::TestLog16(log16),
            MainOpenedGroupProviderV1::TestLog18(log18),
            MainOpenedGroupProviderV1::TestLog19(log19),
        ]
    }

    fn mock_main_opened_groups_v1(
        layout: &AggregateProofLayoutV1,
    ) -> Vec<aggregate::AggregateOpenedTraceGroupV1> {
        layout
            .trace_groups
            .iter()
            .enumerate()
            .map(|(group, descriptor)| {
                let row = |width: usize, offset: u64| {
                    (0..width)
                        .map(|column| {
                            F(u64::try_from(group + 1).expect("small group") * 10_000
                                + offset
                                + u64::try_from(column).expect("registered width fits u64"))
                        })
                        .collect()
                };
                aggregate::AggregateOpenedTraceGroupV1 {
                    base_current: row(descriptor.base_width, 100),
                    base_next: row(descriptor.base_width, 200),
                    aux_current: row(descriptor.aux_width, 300),
                    aux_next: row(descriptor.aux_width, 400),
                }
            })
            .collect()
    }

    fn mock_main_alphas_v1(layout: &AggregateProofLayoutV1) -> Vec<Vec<Vec<E>>> {
        layout
            .registered_segments
            .iter()
            .enumerate()
            .map(|(registration, descriptor)| {
                vec![
                    vec![
                        E::from_base(F(
                            u64::try_from(registration + 2).expect("small registration")
                        ));
                        descriptor.segment.constraint_count
                    ];
                    SECURITY_LANES
                ]
            })
            .collect()
    }

    fn mock_main_mixes_v1(layout: &AggregateProofLayoutV1) -> Vec<Vec<FriMixV1>> {
        let composition = (0..COMPOSITION_DEGREE_CHUNKS)
            .map(|index| E::from_base(F(u64::try_from(index + 31).expect("small mix"))))
            .collect::<Vec<_>>();
        layout
            .trace_groups
            .iter()
            .enumerate()
            .map(|(group, descriptor)| {
                (0..SECURITY_LANES)
                    .map(|_| {
                        let coefficient =
                            E::from_base(F(u64::try_from(group + 3).expect("small group")));
                        FriMixV1 {
                            base: vec![coefficient; descriptor.base_width],
                            base_next: vec![E::ZERO; descriptor.base_width],
                            aux: vec![coefficient; descriptor.aux_width],
                            aux_next: vec![E::ZERO; descriptor.aux_width],
                            composition: composition.clone(),
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn endpoint(role: ZkX509IoSegmentRoleV1, instance: u16) -> ZkX509IoEndpointV1 {
        ZkX509IoEndpointV1 { role, instance }
    }

    fn channel(
        channel: u32,
        producer: ZkX509IoEndpointV1,
        consumers: Vec<ZkX509IoEndpointV1>,
        value: &[u8],
        public: bool,
    ) -> ZkX509IoChannelWitnessV1 {
        ZkX509IoChannelWitnessV1 {
            declaration: ZkX509IoChannelDeclarationV1 {
                channel,
                producer,
                consumers: consumers.clone(),
                byte_len: value.len() as u32,
                public_value: public.then(|| value.to_vec()),
            },
            producer_value: value.to_vec(),
            consumer_values: vec![value.to_vec(); consumers.len()],
        }
    }

    fn fixture_witnesses() -> Vec<ZkX509IoChannelWitnessV1> {
        vec![
            channel(
                0,
                endpoint(ZkX509IoSegmentRoleV1::StrictDer, 0),
                vec![
                    endpoint(ZkX509IoSegmentRoleV1::Sha256, 0),
                    endpoint(ZkX509IoSegmentRoleV1::P256, 0),
                ],
                &[0x30, 0x03, 0x02, 0x01],
                false,
            ),
            channel(
                1,
                endpoint(ZkX509IoSegmentRoleV1::Projection, 0),
                vec![endpoint(ZkX509IoSegmentRoleV1::PublicInput, 0)],
                &[0xA1, 0xB2, 0xC3, 0xD4],
                true,
            ),
        ]
    }

    fn fixture_statement() -> ZkX509IoStarkStatementV1 {
        ZkX509IoStarkStatementV1::new(
            fixture_witnesses()
                .into_iter()
                .map(|witness| witness.declaration)
                .collect(),
        )
        .expect("valid statement")
    }

    fn io_challenges_fixture_v1() -> ZkX509IoChallengesV1 {
        let mut transcript = TransparentTranscriptV1::new(
            b"zk-x509-io-logical-active-tests-v1",
            &[0x63; 32],
            &[0xA7; 32],
        )
        .expect("I/O test transcript");
        derive_zk_x509_io_challenges_v1(&mut transcript).expect("I/O test challenges")
    }

    fn focused_io_material_fixture_v1() -> (IoTraceMaterialV1, ZkX509IoChallengesV1) {
        let statement = fixture_statement();
        let witnesses = fixture_witnesses();
        let logical_active_rows =
            io_active_rows_v1(statement.declarations()).expect("logical rows");
        let (layout, base_columns, fixed_columns, execution, sorted) =
            build_io_base_and_fixed_columns_v1(&statement, &witnesses)
                .expect("focused base and fixed columns");
        let challenges = io_challenges_fixture_v1();
        let aux_columns = build_io_aux_columns_v1(
            &statement,
            &witnesses,
            challenges,
            layout,
            logical_active_rows,
            &execution,
            &sorted,
        )
        .expect("focused auxiliary columns");
        (
            IoTraceMaterialV1 {
                layout,
                logical_active_rows,
                base_columns,
                aux_columns,
                fixed_columns,
            },
            challenges,
        )
    }

    fn main_io_topology_source_fixture_v1(
        disclosures: usize,
    ) -> (IrohaZkX509StarkP256StatementV1, ZkX509MainIoBaseMaterialV1) {
        let statement =
            crate::privacy_engines::zk_x509::main_io::tests::statement_with_disclosures_v1(
                disclosures,
            );
        let plan = compile_zk_x509_main_io_declarations_v1(&statement).expect("MAIN I/O plan");
        let witnesses =
            topology_witnesses_v1(&plan.declarations).expect("topology-only MAIN witnesses");
        let (declarations, execution, sorted) =
            build_zk_x509_io_base_tables_v1(&witnesses).expect("topology-only MAIN base tables");
        assert_eq!(declarations, plan.declarations);
        assert_eq!(execution.len(), plan.logical_active_rows);
        assert_eq!(sorted.len(), plan.logical_active_rows);
        (
            statement,
            ZkX509MainIoBaseMaterialV1 {
                witnesses,
                declarations,
                logical_active_rows: plan.logical_active_rows,
                execution,
                sorted,
            },
        )
    }

    fn legacy_direct_io_fixed_row_v1(
        statement: &ZkX509IoStarkStatementV1,
        topology_execution: &[IoAccessV1],
        topology_sorted: &[IoAccessV1],
        witness_sorted: &[IoAccessV1],
        layout: SegmentLayoutV1,
        logical_active_rows: usize,
        index: usize,
    ) -> [F; IO_FIXED_WIDTH] {
        let mut fixed = [F::ZERO; IO_FIXED_WIDTH];
        if index < logical_active_rows {
            access_fixed_fields_v1(topology_execution[index], &mut fixed, FIX_EXEC_CHANNEL);
            access_fixed_fields_v1(topology_sorted[index], &mut fixed, FIX_SORT_CHANNEL);
            if let Some(value) =
                public_value_for_access_v1(statement.declarations(), topology_execution[index])
                    .expect("legacy public lookup")
            {
                fixed[FIX_PUBLIC_SELECTOR] = F::ONE;
                fixed[FIX_PUBLIC_VALUE] = F(u64::from(value));
            }
            if index + 1 < logical_active_rows
                && witness_sorted[index].channel == witness_sorted[index + 1].channel
                && witness_sorted[index].offset == witness_sorted[index + 1].offset
            {
                fixed[FIX_SORT_SAME_ADDRESS_NEXT] = F::ONE;
            }
        }
        let [active, first, last_active, transition] =
            io_fixed_selector_fields_v1(index, logical_active_rows, layout.trace_size())
                .expect("legacy selectors");
        fixed[FIX_ACTIVE] = active;
        fixed[FIX_FIRST] = first;
        fixed[FIX_LAST_ACTIVE] = last_active;
        fixed[FIX_TRANSITION] = transition;
        fixed
    }

    #[test]
    fn main_io_statement_only_compiler_matches_every_honest_legacy_fixed_row_and_column() {
        let (statement, source) = main_io_topology_source_fixture_v1(0);
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("MAIN I/O registration");
        let (io_statement, schedule) =
            compile_main_io_fixed_schedule_v1(registration.segment, &statement)
                .expect("statement-only fixed schedule");
        let synthetic =
            topology_witnesses_v1(io_statement.declarations()).expect("legacy topology witnesses");
        let (_, topology_execution, topology_sorted) =
            build_zk_x509_io_base_tables_v1(&synthetic).expect("legacy direct topology tables");
        assert_eq!(topology_execution.len(), source.logical_active_rows);
        assert_eq!(topology_sorted.len(), source.logical_active_rows);

        for index in 0..registration.segment.trace_size() {
            let compiled = schedule.fixed_row_v1(index).expect("compiled fixed row");
            let legacy = legacy_direct_io_fixed_row_v1(
                &io_statement,
                &topology_execution,
                &topology_sorted,
                &source.sorted,
                registration.segment,
                source.logical_active_rows,
                index,
            );
            assert_eq!(
                compiled, legacy,
                "all {IO_FIXED_WIDTH} fixed columns must match at MAIN row {index}"
            );
        }

        let post_base = projection_provider_post_base_v1(&statement);
        let prover =
            MainIoProverConstraintSourceV1::for_main_v1(&layout, &statement, &source, post_base)
                .expect("MAIN I/O prover constraint source");
        let mut streamed_columns = 0_usize;
        assert!(matches!(
            prover.stream_fixed_polynomials_v1(|column, coefficients| {
                assert_eq!(column, 0);
                assert_eq!(coefficients.len(), registration.segment.trace_size());
                streamed_columns += 1;
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            }),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert_eq!(streamed_columns, 1);
        let mut forged_source = source.clone();
        forged_source.execution[0].channel = forged_source.execution[0].channel.add(F::ONE);
        assert!(
            MainIoProverConstraintSourceV1::for_main_v1(
                &layout,
                &statement,
                &forged_source,
                post_base,
            )
            .is_err(),
            "the prover fixed source must validate witness topology against the public compiler"
        );
        let verifier =
            MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("MAIN I/O verifier constraint source");
        assert_eq!(prover.fixed_schedule, schedule);
        assert_eq!(verifier.fixed_schedule, schedule);
        assert!(verifier.fixed_openings.is_empty());
    }

    #[test]
    fn main_io_fixed_same_address_schedule_and_topology_are_independent_of_private_bytes() {
        let statement = fixture_statement();
        let witnesses = fixture_witnesses();
        let logical_active_rows =
            io_active_rows_v1(statement.declarations()).expect("logical rows");
        let layout = SegmentLayoutV1::for_io(logical_active_rows).expect("focused I/O layout");
        let (_, honest_fixed, honest_execution, honest_sorted) =
            build_io_base_and_fixed_columns_for_layout_v1(
                &statement,
                &witnesses,
                layout,
                logical_active_rows,
            )
            .expect("honest fixed schedule");

        let mut changed_witnesses = witnesses.clone();
        let private = changed_witnesses
            .iter_mut()
            .find(|witness| witness.declaration.public_value.is_none())
            .expect("private fixture channel");
        for (offset, byte) in private.producer_value.iter_mut().enumerate() {
            *byte = u8::try_from(0xE0 + offset).expect("small private fixture");
        }
        for consumer in &mut private.consumer_values {
            consumer.copy_from_slice(&private.producer_value);
        }
        let (_, changed_fixed, changed_execution, changed_sorted) =
            build_io_base_and_fixed_columns_for_layout_v1(
                &statement,
                &changed_witnesses,
                layout,
                logical_active_rows,
            )
            .expect("changed private bytes");
        assert_eq!(honest_fixed, changed_fixed);
        assert_ne!(honest_execution, changed_execution);
        assert_ne!(honest_sorted, changed_sorted);
        assert!(
            honest_fixed[FIX_SORT_SAME_ADDRESS_NEXT]
                .iter()
                .any(|value| *value == F::ONE)
        );

        let schedule = MainIoFixedScheduleV1::compile_v1(layout, &statement, logical_active_rows)
            .expect("statement-only schedule");
        schedule
            .validate_witness_topology_v1(&honest_execution, &honest_sorted)
            .expect("honest topology");
        schedule
            .validate_witness_topology_v1(&changed_execution, &changed_sorted)
            .expect("private values are not topology");
        let mut forged_execution = changed_execution;
        forged_execution[0].offset = forged_execution[0].offset.add(F::ONE);
        assert!(
            schedule
                .validate_witness_topology_v1(&forged_execution, &changed_sorted)
                .is_err()
        );
        let mut forged_sorted = changed_sorted;
        forged_sorted[0].is_write = F::ZERO;
        assert!(
            schedule
                .validate_witness_topology_v1(&honest_execution, &forged_sorted)
                .is_err()
        );
    }

    #[test]
    fn full_main_io_registration_is_capacity_but_d0_through_d4_use_logical_selectors_and_endpoints()
    {
        let layout = SegmentLayoutV1::for_full_io().expect("full I/O layout");
        assert_eq!(layout.trace_log2, 18);
        assert_eq!(layout.trace_size(), ZK_X509_IO_FIXED_CAPACITY_ROWS_V1);
        assert_eq!(layout.active_rows, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1);

        for disclosures in 0..=4 {
            let statement =
                crate::privacy_engines::zk_x509::main_io::tests::statement_with_disclosures_v1(
                    disclosures,
                );
            let plan = compile_zk_x509_main_io_declarations_v1(&statement).expect("MAIN I/O plan");
            validate_io_logical_geometry_v1(layout, plan.logical_active_rows)
                .expect("full capacity with logical prefix");
            assert!(plan.logical_active_rows < layout.trace_size());
            assert_eq!(
                io_fixed_selector_fields_v1(0, plan.logical_active_rows, layout.trace_size())
                    .expect("first row"),
                [F::ONE, F::ONE, F::ZERO, F::ONE]
            );
            assert_eq!(
                io_fixed_selector_fields_v1(
                    plan.logical_active_rows - 1,
                    plan.logical_active_rows,
                    layout.trace_size(),
                )
                .expect("last logical row"),
                [F::ONE, F::ZERO, F::ONE, F::ONE]
            );
            assert_eq!(
                io_fixed_selector_fields_v1(
                    plan.logical_active_rows,
                    plan.logical_active_rows,
                    layout.trace_size(),
                )
                .expect("first padding row"),
                [F::ZERO, F::ZERO, F::ZERO, F::ONE]
            );
            assert_eq!(
                io_fixed_selector_fields_v1(
                    layout.trace_size() - 1,
                    plan.logical_active_rows,
                    layout.trace_size(),
                )
                .expect("last capacity row"),
                [F::ZERO, F::ZERO, F::ZERO, F::ZERO]
            );

            let current_base = vec![F::ZERO; IO_BASE_WIDTH];
            let next_base = current_base.clone();
            let mut current_aux = vec![F::ZERO; IO_AUX_WIDTH];
            let mut next_aux = vec![F::ZERO; IO_AUX_WIDTH];
            let logical_field =
                F(u64::try_from(plan.logical_active_rows).expect("logical rows fit u64"));
            for row in [&mut current_aux, &mut next_aux] {
                row[AUX_CONT_GLOBAL_END] = logical_field;
                row[AUX_CONT_MEMORY_END] = logical_field;
                row[AUX_CONT_EXEC_START..AUX_CONT_EXEC_START + IO_LANES].fill(F::ONE);
                row[AUX_CONT_SORT_START..AUX_CONT_SORT_START + IO_LANES].fill(F::ONE);
            }
            let mut fixed = vec![F::ZERO; IO_FIXED_WIDTH];
            fixed[FIX_TRANSITION] = F::ONE;
            let residues = io_constraint_residues_v1(
                layout,
                plan.logical_active_rows,
                &current_base,
                &next_base,
                &current_aux,
                &next_aux,
                &fixed,
                io_challenges_fixture_v1(),
            )
            .expect("logical continuation residues");
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "d={disclosures} logical padding row must satisfy every I/O constraint"
            );
            let capacity_residues = io_constraint_residues_v1(
                layout,
                layout.active_rows,
                &current_base,
                &next_base,
                &current_aux,
                &next_aux,
                &fixed,
                io_challenges_fixture_v1(),
            )
            .expect("capacity-sized comparison residues");
            assert!(
                capacity_residues.iter().any(|residue| *residue != F::ZERO),
                "d={disclosures} continuation endpoints must not use registration capacity"
            );
        }
    }

    #[test]
    fn io_logical_geometry_rejects_zero_over_capacity_and_noncanonical_two_count_layouts() {
        let full = SegmentLayoutV1::for_full_io().expect("full I/O");
        assert!(validate_io_logical_geometry_v1(full, 0).is_err());
        assert!(
            validate_io_logical_geometry_v1(full, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1).is_err()
        );
        assert!(io_fixed_selector_fields_v1(0, 0, ZK_X509_IO_FIXED_CAPACITY_ROWS_V1).is_err());
        assert!(
            io_fixed_selector_fields_v1(
                0,
                ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1,
                ZK_X509_IO_FIXED_CAPACITY_ROWS_V1,
            )
            .is_err()
        );

        let logical_active_rows =
            io_active_rows_v1(fixture_statement().declarations()).expect("focused logical rows");
        let mut noncanonical = SegmentLayoutV1::for_io(logical_active_rows).expect("focused I/O");
        noncanonical.active_rows += 1;
        assert!(validate_io_logical_geometry_v1(noncanonical, logical_active_rows).is_err());

        let base = vec![F::ZERO; IO_BASE_WIDTH];
        let aux = vec![F::ZERO; IO_AUX_WIDTH];
        let fixed = vec![F::ZERO; IO_FIXED_WIDTH];
        assert!(matches!(
            io_constraint_residues_v1(
                full,
                0,
                &base,
                &base,
                &aux,
                &aux,
                &fixed,
                io_challenges_fixture_v1(),
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            io_constraint_residues_v1(
                full,
                ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1,
                &base,
                &base,
                &aux,
                &aux,
                &fixed,
                io_challenges_fixture_v1(),
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }

    #[test]
    fn io_material_rejects_off_by_one_selectors_padding_forgery_and_continuation_mismatch() {
        let (material, challenges) = focused_io_material_fixture_v1();
        validate_io_trace_material_shape_v1(&material).expect("canonical material shape");
        validate_io_base_constraints_v1(&material, challenges)
            .expect("canonical focused I/O constraints");

        let logical = material.logical_active_rows;
        let mut moved_last = material.clone();
        moved_last.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ZERO;
        moved_last.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ONE;
        assert!(matches!(
            validate_io_trace_material_shape_v1(&moved_last),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));

        let mut active_padding = material.clone();
        active_padding.fixed_columns[FIX_ACTIVE][logical] = F::ONE;
        assert!(matches!(
            validate_io_trace_material_shape_v1(&active_padding),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));

        let mut base_padding = material.clone();
        base_padding.base_columns[EXEC_VALUE][logical] = F::ONE;
        assert!(matches!(
            validate_io_trace_material_shape_v1(&base_padding),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));

        let mut wrong_continuation = material.clone();
        wrong_continuation.aux_columns[AUX_CONT_GLOBAL_END][logical] =
            wrong_continuation.aux_columns[AUX_CONT_GLOBAL_END][logical].add(F::ONE);
        assert!(matches!(
            validate_io_trace_material_shape_v1(&wrong_continuation),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));

        let mut wrong_logical_count = material;
        wrong_logical_count.logical_active_rows += 1;
        assert!(matches!(
            validate_io_trace_material_shape_v1(&wrong_logical_count),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }

    #[test]
    fn phased_full_main_io_provider_enforces_base_then_token_then_aux_and_rejects_tampering() {
        let _guard = proof_guard();
        let (statement, source) = main_io_topology_source_fixture_v1(0);
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("MAIN I/O registration");

        let assert_source_rejected = |changed: &ZkX509MainIoBaseMaterialV1| {
            assert!(
                MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, changed).is_err(),
                "altered MAIN source must reject before entering the base phase"
            );
        };
        let mut changed = source.clone();
        changed.logical_active_rows -= 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.logical_active_rows += 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.logical_active_rows = 0;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.logical_active_rows = ZK_X509_IO_FIXED_CAPACITY_ROWS_V1 + 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.declarations.swap(0, 1);
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.declarations[0].channel += 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.declarations[0].producer.instance =
            changed.declarations[0].producer.instance.wrapping_add(1);
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.declarations[0].consumers[0].instance = changed.declarations[0].consumers[0]
            .instance
            .wrapping_add(1);
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.declarations[0].byte_len += 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        let private_declaration = changed
            .declarations
            .iter_mut()
            .find(|declaration| declaration.public_value.is_none())
            .expect("MAIN has private channels");
        private_declaration.public_value = Some(vec![0_u8; private_declaration.byte_len as usize]);
        assert_source_rejected(&changed);
        changed = source.clone();
        changed
            .declarations
            .iter_mut()
            .find(|declaration| declaration.public_value.is_some())
            .expect("MAIN has public channels")
            .public_value = None;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.execution.pop();
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.execution[0].value = changed.execution[0].value.add(F::ONE);
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.sorted.pop();
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.sorted.swap(0, 1);
        assert_source_rejected(&changed);
        changed = source.clone();
        let public = changed
            .declarations
            .iter_mut()
            .find_map(|declaration| declaration.public_value.as_mut())
            .expect("MAIN has verifier-fixed public digest channels");
        public[0] ^= 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.witnesses[0].declaration.channel += 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.witnesses.pop();
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.witnesses[0].producer_value[0] ^= 1;
        assert_source_rejected(&changed);
        changed = source.clone();
        changed.witnesses[0].consumer_values.pop();
        assert_source_rejected(&changed);
        drop(changed);

        let (other_statement, _) = main_io_topology_source_fixture_v1(1);
        assert!(
            MainIoTraceGroupSourceV1::for_main_v1(&layout, &other_statement, &source).is_err(),
            "statement/source replay across disclosure counts must reject"
        );
        let isolated_io_layout = AggregateProofLayoutV1::for_segments(&[
            SegmentLayoutV1::for_full_io().expect("full I/O segment"),
        ])
        .expect("isolated I/O aggregate");
        assert!(
            MainIoTraceGroupSourceV1::for_main_v1(&isolated_io_layout, &statement, &source)
                .is_err(),
            "production provider requires the exact 49-registration MAIN layout"
        );

        let post_base = projection_provider_post_base_v1(&statement);
        let mut failed_bind = MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, &source)
            .expect("failed-bind adversary base phase");
        failed_bind.base_columns[EXEC_VALUE][source.logical_active_rows] = F::ONE;
        assert!(matches!(
            failed_bind.bind_challenges_v1(post_base),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));
        failed_bind.base_columns[EXEC_VALUE][source.logical_active_rows] = F::ZERO;
        assert!(matches!(
            failed_bind.bind_challenges_v1(post_base),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert!(failed_bind.aux_columns.is_none());
        drop(failed_bind);

        let mut provider = MainIoTraceGroupSourceV1::for_main_v1(&layout, &statement, &source)
            .expect("MAIN I/O base phase");
        let logical = source.logical_active_rows;
        let capacity = ZK_X509_IO_FIXED_CAPACITY_ROWS_V1;
        assert_eq!(provider.registration, registration);
        assert_eq!(provider.registration.segment.active_rows, capacity);
        assert_eq!(provider.registration.segment.trace_size(), capacity);
        assert!(!provider.bind_attempted);
        assert!(provider.aux_columns.is_none());
        assert!(provider.post_base.is_none());
        provider
            .validate_base_phase_v1()
            .expect("validated pre-challenge base phase");
        assert!(provider.validate_bound_phase_v1().is_err());
        assert!(
            provider.native_aux_column_v1(registration, 0).is_err(),
            "auxiliary columns must not exist before the post-base token"
        );
        let base_column = provider
            .native_base_column_v1(registration, EXEC_VALUE)
            .expect("base column available before token");
        assert_eq!(base_column.len(), capacity);
        drop(base_column);
        assert!(
            provider
                .native_base_column_v1(registration, IO_BASE_WIDTH)
                .is_err()
        );
        let wrong_registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        assert!(
            provider
                .native_base_column_v1(wrong_registration, 0)
                .is_err()
        );

        provider.base_columns[EXEC_VALUE][logical] = F::ONE;
        assert!(provider.validate_base_phase_v1().is_err());
        provider.base_columns[EXEC_VALUE][logical] = F::ZERO;
        provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ZERO;
        provider.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ONE;
        assert!(provider.validate_base_phase_v1().is_err());
        provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1] = F::ONE;
        provider.fixed_columns[FIX_LAST_ACTIVE][logical] = F::ZERO;
        provider
            .validate_base_phase_v1()
            .expect("restored base phase");

        provider
            .bind_challenges_v1(post_base)
            .expect("opaque post-base token binds I/O auxiliary phase");
        assert!(provider.bind_attempted);
        assert_eq!(provider.post_base, Some(post_base));
        assert!(provider.aux_columns.is_some());
        provider
            .validate_bound_phase_v1()
            .expect("validated post-token auxiliary phase");
        assert!(matches!(
            provider.bind_challenges_v1(post_base),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        let aux_column = provider
            .native_aux_column_v1(registration, AUX_CONT_GLOBAL_END)
            .expect("aux column available after token");
        assert_eq!(aux_column.len(), capacity);
        drop(aux_column);
        assert!(
            provider
                .native_aux_column_v1(registration, IO_AUX_WIDTH)
                .is_err()
        );
        assert!(
            provider
                .native_aux_column_v1(wrong_registration, 0)
                .is_err()
        );

        let aux_columns = provider.aux_columns.as_ref().expect("bound aux");
        assert!(
            provider
                .base_columns
                .iter()
                .chain(aux_columns)
                .chain(&provider.fixed_columns)
                .all(|column| column.len() == capacity)
        );
        assert_eq!(provider.fixed_columns[FIX_ACTIVE][logical - 1], F::ONE);
        assert_eq!(provider.fixed_columns[FIX_LAST_ACTIVE][logical - 1], F::ONE);
        assert_eq!(provider.fixed_columns[FIX_ACTIVE][logical], F::ZERO);
        assert_eq!(provider.fixed_columns[FIX_LAST_ACTIVE][logical], F::ZERO);
        assert_eq!(provider.fixed_columns[FIX_TRANSITION][logical], F::ONE);
        let logical_field = F(u64::try_from(logical).expect("logical rows fit u64"));
        for index in [0, logical - 1, logical, capacity - 1] {
            assert_eq!(aux_columns[AUX_CONT_GLOBAL_END][index], logical_field);
            assert_eq!(aux_columns[AUX_CONT_MEMORY_END][index], logical_field);
        }
        for lane in 0..IO_LANES {
            let final_exec = aux_columns[AUX_EXEC_AFTER + lane][logical - 1];
            let final_sort = aux_columns[AUX_SORT_AFTER + lane][logical - 1];
            assert_eq!(aux_columns[AUX_EXEC_BEFORE + lane][logical], final_exec);
            assert_eq!(aux_columns[AUX_EXEC_AFTER + lane][capacity - 1], final_exec);
            assert_eq!(aux_columns[AUX_SORT_BEFORE + lane][logical], final_sort);
            assert_eq!(aux_columns[AUX_SORT_AFTER + lane][capacity - 1], final_sort);
        }

        provider.aux_columns.as_mut().expect("bound aux")[AUX_CONT_MEMORY_END][logical] =
            logical_field.add(F::ONE);
        assert!(provider.validate_bound_phase_v1().is_err());
        provider.aux_columns.as_mut().expect("bound aux")[AUX_CONT_MEMORY_END][logical] =
            logical_field;
        let first_padding_product =
            provider.aux_columns.as_ref().expect("bound aux")[AUX_EXEC_BEFORE][logical];
        provider.aux_columns.as_mut().expect("bound aux")[AUX_EXEC_BEFORE][logical] =
            first_padding_product.add(F::ONE);
        assert!(matches!(
            provider.validate_bound_phase_v1(),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));
        provider.zeroize_private_buffers_v1();
        assert!(provider.base_columns.is_empty());
        assert!(provider.fixed_columns.is_empty());
        assert!(provider.aux_columns.is_none());
        assert!(provider.post_base.is_none());
        assert!(provider.native_base_column_v1(registration, 0).is_err());
        assert!(provider.native_aux_column_v1(registration, 0).is_err());
        assert!(matches!(
            provider.bind_challenges_v1(post_base),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
    }

    #[test]
    fn main_io_verifier_rejects_registration_query_next_x_width_and_noncanonical_without_mutation()
    {
        let (statement, _) = main_io_topology_source_fixture_v1(0);
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("MAIN I/O registration");
        let wrong_registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("wrong registration");
        let post_base = projection_provider_post_base_v1(&statement);
        let mut source =
            MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("MAIN I/O verifier source");
        let original_schedule = source.fixed_schedule.clone();
        let original_challenges = source.challenges;
        let query_index = 17;
        let next_query_index = source
            .next_query_index_v1(query_index)
            .expect("canonical next query");
        let common_lde_size = source.common_lde_size_v1().expect("common size");
        let wrong_next_query_index = (next_query_index + 1) % common_lde_size;
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };

        assert!(
            source
                .constraint_residues_v1(
                    wrong_registration,
                    query_index,
                    next_query_index,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    common_lde_size,
                    next_query_index,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    wrong_next_query_index,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x.add(F::ONE),
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1),
                    opening,
                )
                .is_err()
        );

        let short_base = vec![F::ZERO; registration.segment.base_width - 1];
        let short_opening = RegisteredOpenedRowsV1 {
            base_current: &short_base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    short_opening,
                )
                .is_err()
        );
        let short_aux = vec![F::ZERO; registration.segment.aux_width - 1];
        let short_aux_opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &short_aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    short_aux_opening,
                )
                .is_err()
        );

        let mut noncanonical_base = base.clone();
        noncanonical_base[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        let noncanonical_opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &noncanonical_base,
            aux_current: &aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    noncanonical_opening,
                )
                .is_err()
        );
        let mut noncanonical_aux = aux.clone();
        noncanonical_aux[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        let noncanonical_aux_opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &noncanonical_aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    noncanonical_aux_opening,
                )
                .is_err()
        );

        assert!(
            source.fixed_openings.is_empty(),
            "every invalid request must reject before sampling"
        );
        assert_eq!(source.fixed_schedule, original_schedule);
        assert_eq!(source.challenges, original_challenges);
    }

    #[test]
    fn main_io_closed_log18_verifier_matches_prover_residues_and_reuses_bounded_cache() {
        let (statement, source) = main_io_topology_source_fixture_v1(0);
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("MAIN I/O registration");
        let post_base = projection_provider_post_base_v1(&statement);
        let prover =
            MainIoProverConstraintSourceV1::for_main_v1(&layout, &statement, &source, post_base)
                .expect("MAIN I/O prover constraint source");
        let mut verifier =
            MainIoVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("MAIN I/O verifier constraint source");
        let query_index = 17;
        let next_query_index = verifier
            .next_query_index_v1(query_index)
            .expect("canonical next query");
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };

        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log8 = MockMainTraceGroupSourceV1::default();
        let mut log15 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            vec![
                MainOpenedGroupProviderV1::TestLog5(&mut log5),
                MainOpenedGroupProviderV1::TestLog8(&mut log8),
                MainOpenedGroupProviderV1::TestLog15(&mut log15),
                MainOpenedGroupProviderV1::TestLog16(&mut log16),
                MainOpenedGroupProviderV1::Io(&mut verifier),
                MainOpenedGroupProviderV1::TestLog19(&mut log19),
            ],
        )
        .expect("closed MAIN provider set");
        let opened_residues = providers
            .registered_constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            )
            .expect("closed log-18 I/O verifier route");
        drop(providers);
        assert_eq!(verifier.fixed_openings.len(), 2);

        let verifier_fixed = *verifier
            .fixed_openings
            .get(&query_index)
            .expect("verifier-generated current fixed opening");
        let prover_residues = prover
            .constraint_residues_v1(registration, opening, &verifier_fixed)
            .expect("direct prover residues");
        assert_eq!(opened_residues, prover_residues);
        assert_eq!(opened_residues.len(), registration.segment.constraint_count);
        let alphas = vec![E::ONE; registration.segment.constraint_count];
        let composition = prover
            .composition_value_v1(registration, x, opening, &verifier_fixed, &alphas)
            .expect("prover quotient");
        assert_eq!(
            composition,
            accumulator_quotient_value_v1(registration.segment, x, &opened_residues, &alphas,)
                .expect("residue quotient")
        );

        let repeated = verifier
            .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
            .expect("cache reuse");
        assert_eq!(repeated, opened_residues);
        assert_eq!(verifier.fixed_openings.len(), 2);

        let cap_query = 1_000;
        let cap_next = verifier
            .next_query_index_v1(cap_query)
            .expect("cap-test next query");
        verifier
            .fixed_openings
            .entry(cap_query)
            .or_insert([F::ZERO; IO_FIXED_WIDTH]);
        let mut candidate = 0_usize;
        while verifier.fixed_openings.len() < VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
            if candidate != cap_next {
                verifier
                    .fixed_openings
                    .entry(candidate)
                    .or_insert([F::ZERO; IO_FIXED_WIDTH]);
            }
            candidate += 1;
        }
        assert!(!verifier.fixed_openings.contains_key(&cap_next));
        let full_cache = verifier.fixed_openings.clone();
        let cap_x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(cap_query as u128));
        assert!(
            verifier
                .constraint_residues_v1(registration, cap_query, cap_next, cap_x, opening,)
                .is_err(),
            "the 117th distinct fixed opening must reject before sampling"
        );
        assert_eq!(verifier.fixed_openings, full_cache);
        assert_eq!(
            verifier.fixed_openings.len(),
            VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        );
    }

    fn fixture() -> &'static (ZkX509IoStarkStatementV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkX509IoStarkStatementV1, Vec<u8>)> = OnceLock::new();
        let _guard = proof_guard();
        FIXTURE.get_or_init(|| {
            let statement = fixture_statement();
            let mut rng = StdRng::from_seed([0x5A; 32]);
            let proof = prove_zk_x509_io_segmented_stark_v1_with_rng(
                &statement,
                &fixture_witnesses(),
                &mut rng,
            )
            .expect("deterministic proof");
            (statement, proof)
        })
    }

    fn fixture_layout() -> SegmentLayoutV1 {
        SegmentLayoutV1::for_io(io_active_rows_v1(fixture().0.declarations()).expect("rows"))
            .expect("layout")
    }

    fn fixture_aggregate_layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::for_segments(&[fixture_layout()]).expect("aggregate layout")
    }

    fn decode_fixture() -> ZkX509SegmentedStarkProofV1 {
        decode_zk_x509_segmented_stark_proof_v1(&fixture().1, &fixture_aggregate_layout())
            .expect("decode fixture")
    }

    fn assert_rejected(proof: &ZkX509SegmentedStarkProofV1) {
        match encode_zk_x509_segmented_stark_proof_v1(proof, &fixture_aggregate_layout()) {
            Ok(bytes) => assert!(
                verify_zk_x509_io_segmented_stark_v1(&fixture().0, &bytes).is_err(),
                "adversarial proof must reject"
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch) => {}
            Err(error) => panic!("unexpected adversarial encode failure: {error}"),
        }
    }

    fn projection_fixture() -> &'static (
        IrohaZkX509StarkP256StatementV1,
        ZkX509ProjectionWitnessV1,
        Vec<u8>,
    ) {
        static FIXTURE: OnceLock<(
            IrohaZkX509StarkP256StatementV1,
            ZkX509ProjectionWitnessV1,
            Vec<u8>,
        )> = OnceLock::new();
        let _guard = proof_guard();
        FIXTURE.get_or_init(|| {
            let (statement, witness) =
                crate::privacy_engines::zk_x509::projection_air::tests::fixture();
            let mut rng = StdRng::from_seed([0xA7; 32]);
            let proof = prove_zk_x509_projection_segmented_stark_v1_with_rng(
                &statement, &witness, &mut rng,
            )
            .expect("deterministic projection proof");
            (statement, witness, proof)
        })
    }

    fn projection_layout() -> SegmentLayoutV1 {
        SegmentLayoutV1::for_projection().expect("projection layout")
    }

    fn projection_provider_post_base_v1(
        statement: &IrohaZkX509StarkP256StatementV1,
    ) -> ZkX509CredentialMainPostBaseChallengesV1 {
        let digest = projection_public_digest_v1(statement).expect("projection public digest");
        derive_zk_x509_credential_pre_aux_binding_v1(
            ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
                digest,
                [0x31; 32],
                core::array::from_fn(|index| [u8::try_from(index + 1).expect("six roots"); 32]),
            ),
            [0x41; 32],
            [0x51; 32],
            [0x61; 32],
        )
        .expect("joint post-base challenge derivation")
        .main_post_base()
    }

    fn main_base_commitment_session_fixture_v1() -> ZkX509MainBaseCommitmentSessionV1 {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
            &layout,
            [0xB1; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
        )
        .expect("canonical MAIN base session after injected profile validation")
    }

    fn unpinned_main_verifier_profile_fixture_v1() -> ZkX509MainVerifierProfileV1 {
        ZkX509MainVerifierProfileV1 {
            registration: validate_zk_x509_main_registration_shape_v1()
                .expect("canonical registration shape"),
            compiled_profile_digest: TEST_COMPILED_PROFILE_DIGEST_V1,
        }
    }

    fn der_layout() -> SegmentLayoutV1 {
        SegmentLayoutV1::for_der(ZkX509DerStarkShapeV1.active_rows()).expect("DER layout")
    }

    fn der_aggregate_layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::for_segments(&[der_layout()]).expect("DER aggregate layout")
    }

    fn sha_word_layout() -> SegmentLayoutV1 {
        SegmentLayoutV1::for_sha_segment(0, ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0])
            .expect("SHA batch layout")
    }

    fn sha_word_aggregate_layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::for_segments(&[sha_word_layout()])
            .expect("SHA-word aggregate layout")
    }

    fn projection_aggregate_layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::for_segments(&[projection_layout()])
            .expect("projection aggregate layout")
    }

    fn accumulator_aggregate_layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::for_accumulators_v1().expect("accumulator aggregate layout")
    }

    fn accumulator_public_fixture() -> AccumulatorRegistrationPublicV1 {
        AccumulatorRegistrationPublicV1 {
            ca_membership: ZkX509CaAccumulatorStarkPublicV1 {
                governed_root: [F(7); 32],
                root_spki_channel: F(36),
            },
        }
    }

    fn accumulator_trace() -> &'static (ZkX509CaAccumulatorTraceV1, ZkX509ShaCallScheduleV1) {
        static TRACE: OnceLock<(ZkX509CaAccumulatorTraceV1, ZkX509ShaCallScheduleV1)> =
            OnceLock::new();
        TRACE.get_or_init(|| {
            fn spki(index: u16) -> [u8; ZK_X509_CA_SPKI_DER_BYTES_V1] {
                let mut spki = [0x42; ZK_X509_CA_SPKI_DER_BYTES_V1];
                spki[..2].copy_from_slice(&index.to_be_bytes());
                spki
            }
            let members = [spki(7), spki(1), spki(9), spki(3)];
            let references = members
                .iter()
                .map(|member: &[u8; ZK_X509_CA_SPKI_DER_BYTES_V1]| member.as_slice())
                .collect::<Vec<_>>();
            let root = ca_root_from_complete_spkis_v1(&references).expect("compact CA root");
            let path =
                ca_membership_path_from_complete_spkis_v1(&references, &members[0]).expect("path");
            let trace = build_ca_accumulator_trace_v1(
                ZkX509CaAccumulatorStatementV1 {
                    governed_root: root,
                },
                ZkX509CaAccumulatorWitnessV1 {
                    root_spki_der: members[0],
                    path,
                },
            )
            .expect("compact CA trace");
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: 4,
            })
            .expect("canonical SHA schedule");
            (trace, schedule)
        })
    }

    fn accumulator_trace_registration() -> &'static AccumulatorTraceRegistrationV1 {
        static REGISTRATION: OnceLock<AccumulatorTraceRegistrationV1> = OnceLock::new();
        REGISTRATION.get_or_init(|| {
            let (ca, schedule) = accumulator_trace();
            compile_accumulator_trace_registration_v1(ca, schedule)
                .expect("canonical accumulator registration")
        })
    }

    fn p256_aggregate_challenges_fixture() -> P256AggregateChallengesV1 {
        let mut transcript =
            TransparentTranscriptV1::new(b"p256-aggregate-test", &[0x31; 32], &[0x57; 32])
                .expect("P-256 aggregate transcript");
        derive_p256_aggregate_challenges_v1(&mut transcript)
            .expect("canonical P-256 aggregate challenges")
    }

    fn p256_terminal_fixture(role: P256EcdsaRoleV1) -> P256TerminalRegistrationV1 {
        let buses = P256BusTerminalClaimsV1 {
            value_execution: [F(11), F(12), F(13), F(14)],
            value_sorted: [F(11), F(12), F(13), F(14)],
            value_arithmetic_copy: [F(21), F(22), F(23), F(24)],
            arithmetic_value_copy: [F(21), F(22), F(23), F(24)],
            arithmetic_scalar: [F(31), F(32), F(33), F(34)],
            window_scalar: [F(41), F(42), F(43), F(44)],
            scalar_bus_arithmetic: [F(31), F(32), F(33), F(34)],
            scalar_bus_window: [F(41), F(42), F(43), F(44)],
        };
        let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
        let cross_sources = p256_cross_trace_terminal_roles_v1(role)
            .iter()
            .copied()
            .enumerate()
            .map(|(index, source_role)| {
                let terminal = core::array::from_fn(|lane| F((100 + index * 7 + lane) as u64));
                let claim = P256CrossTraceTerminalClaimV1 {
                    role: source_role,
                    start: running,
                    terminal,
                };
                running = terminal;
                claim
            })
            .collect::<Vec<_>>();
        let terminals = P256TerminalRegistrationV1 {
            buses,
            cross_sources,
            sink: running,
        };
        terminals.validate(role).expect("canonical P-256 terminals");
        terminals
    }

    fn zero_p256_terminal_fixture(role: P256EcdsaRoleV1) -> P256TerminalRegistrationV1 {
        let zero = [F::ZERO; P256_CROSS_TRACE_LANES_V1];
        let buses = P256BusTerminalClaimsV1 {
            value_execution: zero,
            value_sorted: zero,
            value_arithmetic_copy: zero,
            arithmetic_value_copy: zero,
            arithmetic_scalar: zero,
            window_scalar: zero,
            scalar_bus_arithmetic: zero,
            scalar_bus_window: zero,
        };
        let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
        let cross_sources = p256_cross_trace_terminal_roles_v1(role)
            .iter()
            .copied()
            .map(|source_role| {
                let claim = P256CrossTraceTerminalClaimV1 {
                    role: source_role,
                    start: running,
                    terminal: zero,
                };
                running = zero;
                claim
            })
            .collect::<Vec<_>>();
        P256TerminalRegistrationV1 {
            buses,
            cross_sources,
            sink: zero,
        }
    }

    fn p256_main_provider_post_base_fixture_v1() -> ZkX509CredentialMainPostBaseChallengesV1 {
        derive_zk_x509_credential_pre_aux_binding_v1(
            ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
                [0x71; 32],
                [0x72; 32],
                core::array::from_fn(|index| [u8::try_from(index + 0x31).expect("six roots"); 32]),
            ),
            [0x73; 32],
            [0x74; 32],
            [0x75; 32],
        )
        .expect("canonical joint post-base challenges")
        .main_post_base()
    }

    fn p256_main_terminal_claims_fixture_v1() -> ZkX509P256TerminalClaimsV1 {
        fn scalar_buses(signature: usize) -> P256BusTerminalClaimsV1 {
            let mut buses = zero_p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl).buses;
            buses.arithmetic_scalar = core::array::from_fn(|lane| {
                F(u64::try_from(100 + signature * 16 + lane).expect("small fixture"))
            });
            buses.scalar_bus_arithmetic = buses.arithmetic_scalar;
            buses.window_scalar = core::array::from_fn(|lane| {
                F(u64::try_from(200 + signature * 16 + lane).expect("small fixture"))
            });
            buses.scalar_bus_window = buses.window_scalar;
            buses
        }

        let certificate_or_crl = core::array::from_fn(|signature| {
            let mut terminals = zero_p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl);
            terminals.buses = scalar_buses(signature);
            ZkX509P256CertificateTerminalClaimsV1 {
                buses: terminals.buses,
                cross_sources: terminals
                    .cross_sources
                    .as_slice()
                    .try_into()
                    .expect("four certificate cross-source claims"),
                sink: terminals.sink,
            }
        });
        let mut wallet = zero_p256_terminal_fixture(P256EcdsaRoleV1::WalletOwnership);
        wallet.buses = scalar_buses(P256_SIGNATURE_COUNT_V1 - 1);
        ZkX509P256TerminalClaimsV1 {
            certificate_or_crl,
            wallet: ZkX509P256WalletTerminalClaimsV1 {
                buses: wallet.buses,
                cross_sources: wallet
                    .cross_sources
                    .as_slice()
                    .try_into()
                    .expect("five wallet cross-source claims"),
                sink: wallet.sink,
            },
        }
    }

    fn main_log19_statement_fixture_v1() -> ZkX509Rfc5280StatementV1 {
        ZkX509Rfc5280StatementV1 {
            presentation_not_before_unix_seconds: 1,
            presentation_not_after_unix_seconds: 2,
            leaf_key_usage: 1,
            leaf_extended_key_usages: vec![ZkX509DerEkuV1::ClientAuthentication],
            crl_number: 1,
            disclosed_attribute_indices: Vec::new(),
        }
    }

    fn main_log19_terminal_claims_fixture_v1() -> ZkX509MainTerminalClaimsV1 {
        let der = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(3), F(5), F(7), F(11)],
            node: [F(13), F(17), F(19), F(23)],
        };
        let mut sha = ZkX509ShaSegmentTerminalClaimsV1::canonical_zero_for_test_v1();
        for segment in &mut sha.segments {
            for stream in &mut segment.rfc_stream_products {
                stream.fill(F::ONE);
            }
        }
        ZkX509MainTerminalClaimsV1 {
            der,
            rfc5280: ZkX509Rfc5280StarkTerminalClaimsV1::canonical_for_der_test_v1(der)
                .expect("canonical DER/RFC test claims"),
            sha,
            p256: p256_main_terminal_claims_fixture_v1(),
        }
    }

    fn main_log19_source_fixture_v1(
        layout: &AggregateProofLayoutV1,
    ) -> MainLog19VerifierConstraintSourceV1 {
        MainLog19VerifierConstraintSourceV1::for_main_v1(
            layout,
            &main_log19_statement_fixture_v1(),
            p256_main_provider_post_base_fixture_v1(),
            main_log19_terminal_claims_fixture_v1(),
        )
        .expect("closed mixed log19 verifier source")
    }

    fn main_log19_query_coordinates_fixture_v1() -> [usize; QUERY_COUNT] {
        core::array::from_fn(|index| index * P256_MAIN_LOG19_NEXT_STRIDE_V1)
    }

    fn main_log19_adversarial_query_coordinates_fixture_v1() -> [usize; QUERY_COUNT] {
        core::array::from_fn(|index| {
            let shift = match index {
                0 => 0,
                1 => ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
                _ => (index * 7_919 + 17) % ZK_X509_DER_STARK_TRACE_SIZE_V1,
            };
            shift * P256_MAIN_LOG19_NEXT_STRIDE_V1 + index
        })
    }

    fn p256_main_base_source_fixture_v1() -> P256MainBaseSourceV1 {
        p256_main_base_source_fixture_for_test_v1().expect("canonical central P-256 base source")
    }

    #[test]
    fn main_log19_query_union_is_exact_sorted_unique_and_bounded() {
        let query_coordinates = main_log19_query_coordinates_fixture_v1();
        let schedule =
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&query_coordinates)
                .expect("canonical post-grinding query schedule");

        assert_eq!(schedule.pairs.len(), QUERY_COUNT);
        assert_eq!(schedule.indices.len(), QUERY_COUNT + 1);
        assert_eq!(schedule.indices.first(), Some(&0));
        assert_eq!(
            schedule.indices.last(),
            Some(
                &u64::try_from(QUERY_COUNT * P256_MAIN_LOG19_NEXT_STRIDE_V1)
                    .expect("bounded last query"),
            )
        );
        assert!(schedule.indices.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            !schedule.indices.is_empty()
                && schedule.indices.len() <= ZK_X509_FIXED_ALGEBRAIC_MAX_QUERIES_V1
        );
        for (slot, current) in query_coordinates.into_iter().enumerate() {
            assert_eq!(
                schedule.pairs[slot],
                (current, current + P256_MAIN_LOG19_NEXT_STRIDE_V1)
            );
        }
        assert_eq!(
            schedule.order_digest,
            main_log19_query_schedule_digest_v1(&schedule.pairs).expect("independent order digest"),
        );
        schedule.validate_v1().expect("self-validating schedule");
    }

    #[test]
    fn main_log19_query_union_handles_wraparound_and_adversarial_residues() {
        let common_lde_size = 1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1;
        let wraparound = core::array::from_fn::<_, QUERY_COUNT, _>(|index| {
            common_lde_size - 1 - index * P256_MAIN_LOG19_NEXT_STRIDE_V1
        });
        let wrapped = MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&wraparound)
            .expect("wraparound query schedule");
        assert!(
            wrapped.pairs.iter().any(|(current, next)| next < current),
            "at least one next query must wrap modulo the common LDE",
        );
        assert!(wrapped.indices.windows(2).all(|pair| pair[0] < pair[1]));

        let adversarial = main_log19_adversarial_query_coordinates_fixture_v1();
        let schedule = MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&adversarial)
            .expect("adversarial residue schedule");
        assert_eq!(schedule.indices.len(), QUERY_COUNT * 2);
        assert!(schedule.indices.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            schedule
                .indices
                .iter()
                .all(|index| *index < u64::try_from(common_lde_size).expect("log25 fits u64"))
        );
    }

    #[test]
    fn main_log19_query_union_rejects_wrong_count_duplicate_range_and_tampering() {
        let canonical = main_log19_adversarial_query_coordinates_fixture_v1();
        assert!(matches!(
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(
                &canonical[..QUERY_COUNT - 1],
            ),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));
        let mut too_many = canonical.to_vec();
        too_many.push(7);
        assert!(matches!(
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&too_many),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut duplicate = canonical;
        duplicate[1] = duplicate[0];
        assert!(matches!(
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&duplicate),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut out_of_range = canonical;
        out_of_range[0] = 1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1;
        assert!(matches!(
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&out_of_range),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));
        out_of_range[0] = usize::MAX;
        assert!(matches!(
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&out_of_range),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut wrong_next =
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&canonical)
                .expect("canonical schedule");
        wrong_next.pairs[0].1 ^= 1;
        assert!(matches!(
            wrong_next.validate_v1(),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut reordered = MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&canonical)
            .expect("canonical schedule");
        reordered.pairs.swap(0, 1);
        assert!(matches!(
            reordered.validate_v1(),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut wrong_digest =
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&canonical)
                .expect("canonical schedule");
        wrong_digest.order_digest[0] ^= 1;
        assert!(matches!(
            wrong_digest.validate_v1(),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut noncanonical_union =
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&canonical)
                .expect("canonical schedule");
        noncanonical_union.indices.swap(0, 1);
        assert!(matches!(
            noncanonical_union.validate_v1(),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));

        let mut missing_union =
            MainLog19VerifierQueryScheduleV1::from_query_coordinates_v1(&canonical)
                .expect("canonical schedule");
        missing_union.indices.pop();
        assert!(matches!(
            missing_union.validate_v1(),
            Err(ZkX509StarkErrorV1::TraceOpening)
        ));
    }

    #[test]
    fn main_log19_algebraic_openings_use_both_success_only_schedule_caches() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let query_coordinates = main_log19_query_coordinates_fixture_v1();
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 0,
        };
        let derived = derive_zk_x509_main_fixed_openings_after_profile_validation_v1(
            shape,
            &query_coordinates,
        )
        .expect("verifier-derived SHA and P-256 openings");
        let duplicate = derived.clone();
        let expected_indices = derived.query_schedule.indices.len();
        assert_eq!(expected_indices, QUERY_COUNT + 1);
        assert_eq!(
            derived.sha.query_indices_v1(),
            derived.query_schedule.indices.as_slice()
        );
        assert_eq!(
            derived.p256_log19.query_indices_v1(),
            derived.query_schedule.indices.as_slice()
        );
        assert_eq!(
            usize::from(derived.sha.width_v1()),
            ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1
        );
        assert_eq!(
            usize::from(derived.p256_log19.width_v1()),
            ZK_X509_P256_FIXED_ALGEBRAIC_WIDTH_V1
        );
        assert_eq!(
            derived.sha.schedule_digest_v1(),
            zk_x509_sha_fixed_algebraic_schedule_v1(shape)
                .expect("cached SHA schedule")
                .descriptor_digest_v1()
        );
        assert_eq!(
            derived.p256_log19.schedule_digest_v1(),
            zk_x509_p256_fixed_algebraic_schedule_v1()
                .expect("cached P-256 schedule")
                .descriptor_digest_v1()
        );

        let mut source = main_log19_source_fixture_v1(&layout);
        source
            .install_verifier_derived_fixed_openings_v1(derived)
            .expect("atomic verifier-derived installation");
        assert_eq!(source.cached_openings_v1(), expected_indices);
        assert_eq!(source.p256.cached_openings_v1(), expected_indices);
        assert!(
            source
                .public_fixed
                .as_ref()
                .is_some_and(|schedule| !schedule.segments.is_empty())
        );
        assert!(matches!(
            source.install_verifier_derived_fixed_openings_v1(duplicate),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(source.cached_openings_v1(), expected_indices);
        assert_eq!(source.p256.cached_openings_v1(), expected_indices);
    }

    #[test]
    fn p256_terminal_registration_scrub_is_recursive_and_idempotent() {
        let mut terminal = p256_terminal_fixture(P256EcdsaRoleV1::WalletOwnership);
        zeroize_p256_terminal_registration_v1(&mut terminal);
        zeroize_p256_terminal_registration_v1(&mut terminal);
        assert!(terminal.cross_sources.is_empty());
        assert_eq!(terminal.sink, [F::ZERO; P256_CROSS_TRACE_LANES_V1]);
        assert!(
            [
                terminal.buses.value_execution,
                terminal.buses.value_sorted,
                terminal.buses.value_arithmetic_copy,
                terminal.buses.arithmetic_value_copy,
                terminal.buses.arithmetic_scalar,
                terminal.buses.window_scalar,
                terminal.buses.scalar_bus_arithmetic,
                terminal.buses.scalar_bus_window,
            ]
            .into_iter()
            .flatten()
            .all(|value| value == F::ZERO)
        );
    }

    #[test]
    fn main_p256_log8_verifier_covers_five_buses_and_exact_eight_terminal_bindings() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("shared verifier-fixed P-256 source");
        let claims = p256_main_terminal_claims_fixture_v1();
        let mut source = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            p256_main_provider_post_base_fixture_v1(),
            &claims,
        )
        .expect("closed log8 verifier source");
        assert!(core::ptr::eq(source.fixed, &fixed));
        assert_eq!(source.registrations.len(), P256_SIGNATURE_COUNT_V1);

        let base = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1];
        let aux = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let common_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        for signature in 0..P256_SIGNATURE_COUNT_V1 {
            let registration = source.registrations[signature].main;
            let query_index = 17 + signature;
            let next_query_index = source
                .next_query_index_v1(registration, query_index)
                .expect("canonical scalar next query");
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
            let residues = source
                .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
                .expect("scalar opened residues");
            assert_eq!(
                residues.len(),
                P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_COUNT_V1 + 2 * P256_SCALAR_BIT_BUS_LANES_V1
            );
            let fixed_opening = source.fixed_openings[signature]
                .get(&query_index)
                .expect("verifier-generated fixed opening");
            let expected = evaluate_p256_scalar_source_terminal_openings_v1(
                p256_scalar_bit_bus_stark_last_active_selector_v1(fixed_opening),
                source.terminals[signature].buses.arithmetic_scalar,
                source.terminals[signature].buses.window_scalar,
                p256_scalar_bit_bus_opened_terminals_v1(&aux),
            );
            assert_eq!(
                &residues[P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_COUNT_V1..],
                expected.as_slice(),
                "signature {signature} must expose exactly the fixed-selector terminal bindings"
            );
            assert_eq!(source.fixed_openings[signature].len(), 2);
        }
        assert_eq!(
            source
                .fixed_openings
                .iter()
                .map(BTreeMap::len)
                .sum::<usize>(),
            P256_SIGNATURE_COUNT_V1 * 2
        );
    }

    #[test]
    fn main_p256_log8_closed_provider_routes_only_the_concrete_verifier() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("shared verifier-fixed P-256 source");
        let claims = p256_main_terminal_claims_fixture_v1();
        let mut scalar = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            p256_main_provider_post_base_fixture_v1(),
            &claims,
        )
        .expect("closed log8 verifier source");
        let registration = scalar.registrations[2].main;
        let query_index = 31;
        let next_query_index = scalar
            .next_query_index_v1(registration, query_index)
            .expect("canonical scalar next query");
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1];
        let aux = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log15 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log18 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            vec![
                MainOpenedGroupProviderV1::TestLog5(&mut log5),
                MainOpenedGroupProviderV1::P256Scalar(&mut scalar),
                MainOpenedGroupProviderV1::TestLog15(&mut log15),
                MainOpenedGroupProviderV1::TestLog16(&mut log16),
                MainOpenedGroupProviderV1::TestLog18(&mut log18),
                MainOpenedGroupProviderV1::TestLog19(&mut log19),
            ],
        )
        .expect("exact closed MAIN providers");
        let residues = providers
            .registered_constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            )
            .expect("production concrete log8 route");
        assert_eq!(
            residues.len(),
            P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
        );
    }

    #[test]
    fn main_p256_log8_verifier_rejects_adversarial_inputs_before_sampling() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("shared verifier-fixed P-256 source");
        let claims = p256_main_terminal_claims_fixture_v1();
        let mut source = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            p256_main_provider_post_base_fixture_v1(),
            &claims,
        )
        .expect("closed log8 verifier source");
        let registration = source.registrations[0].main;
        let query_index = 23;
        let next_query_index = source
            .next_query_index_v1(registration, query_index)
            .expect("canonical scalar next query");
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1];
        let aux = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };

        let wrong_registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        assert!(
            source
                .constraint_residues_v1(
                    wrong_registration,
                    query_index,
                    next_query_index,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index ^ 1,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x.add(F::ONE),
                    opening,
                )
                .is_err()
        );
        let short_base = [F::ZERO; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1 - 1];
        let short_opening = RegisteredOpenedRowsV1 {
            base_current: &short_base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    short_opening,
                )
                .is_err()
        );
        let mut noncanonical_aux = aux;
        noncanonical_aux[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        let noncanonical_opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &noncanonical_aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x,
                    noncanonical_opening,
                )
                .is_err()
        );
        assert!(
            source.fixed_openings.iter().all(BTreeMap::is_empty),
            "rejected requests must not consume verifier fixed-opening capacity"
        );

        let mut inconsistent = claims;
        inconsistent.certificate_or_crl[0]
            .buses
            .scalar_bus_arithmetic[0] = inconsistent.certificate_or_crl[0]
            .buses
            .scalar_bus_arithmetic[0]
            .add(F::ONE);
        assert!(
            MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
                &layout,
                &fixed,
                p256_main_provider_post_base_fixture_v1(),
                &inconsistent,
            )
            .is_err(),
            "inconsistent proof terminals must reject at construction"
        );
    }

    #[test]
    fn main_p256_log8_verifier_fixed_cache_is_bounded_per_signature() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("shared verifier-fixed P-256 source");
        let claims = p256_main_terminal_claims_fixture_v1();
        let mut source = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            p256_main_provider_post_base_fixture_v1(),
            &claims,
        )
        .expect("closed log8 verifier source");
        let first = source.registrations[0];
        for pair in 0..(VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 / 2) {
            source
                .ensure_fixed_openings_v1(first, [pair * 2, pair * 2 + 1])
                .expect("bounded unique scalar fixed openings");
        }
        assert_eq!(
            source.fixed_openings[0].len(),
            VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        );
        let snapshot = source.fixed_openings[0].clone();
        assert!(
            source
                .ensure_fixed_openings_v1(
                    first,
                    [
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1,
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 + 1,
                    ],
                )
                .is_err()
        );
        assert_eq!(source.fixed_openings[0], snapshot);
        let second = source.registrations[1];
        source
            .ensure_fixed_openings_v1(second, [0, 1])
            .expect("separate signature owns a separate bounded cache");
        assert_eq!(source.fixed_openings[1].len(), 2);
    }

    #[test]
    fn main_p256_log8_shared_central_source_matches_prover_and_verifier() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let mut central = p256_main_base_source_fixture_v1();
        let first_registration =
            main_p256_scalar_registrations_v1(&layout).expect("five scalar registrations")[0];
        {
            let mut base_view = MainP256ScalarTraceGroupSourceV1::for_base_v1(&layout, &central)
                .expect("borrowed pre-X5B1 scalar view");
            let column = base_view
                .native_base_column_v1(first_registration.main, 0)
                .expect("challenge-independent scalar base column");
            let mut direct = vec![F::ZERO; first_registration.main.segment.trace_size()];
            central
                .fill_base_column_v1(first_registration.p256, 0, &mut direct)
                .expect("central scalar base column");
            assert_eq!(&*column, direct.as_slice());
            assert!(matches!(
                base_view.native_aux_column_v1(first_registration.main, 0),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            ));
        }

        let post_base = p256_main_provider_post_base_fixture_v1();
        let bound = central
            .bind_v1(post_base)
            .expect("single opaque central P-256 phase transition");
        let mut trace = MainP256ScalarTraceGroupSourceV1::for_bound_v1(&layout, &bound)
            .expect("borrowed bound scalar view");
        let prover = MainP256ScalarProverConstraintSourceV1::for_main_v1(&layout, &bound)
            .expect("bound scalar prover source");
        let claims = bound
            .terminal_claims_v1()
            .expect("central five-signature terminal claims");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("shared verifier-fixed P-256 source");
        let mut verifier = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
            &layout, &fixed, post_base, &claims,
        )
        .expect("witness-free scalar verifier");

        let mut streamed_fixed = 0_usize;
        prover
            .stream_fixed_polynomials_v1(|registration, column, coefficients| {
                assert_eq!(
                    registration.segment.adapter,
                    SegmentAdapterIdV1::P256ScalarBitBus
                );
                assert!(column < P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1);
                assert_eq!(coefficients.len(), registration.segment.trace_size());
                streamed_fixed += 1;
                Ok(())
            })
            .expect("all scalar fixed polynomials");
        assert_eq!(
            streamed_fixed,
            P256_SIGNATURE_COUNT_V1 * P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1
        );

        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        for signature in 0..P256_SIGNATURE_COUNT_V1 {
            let registration = verifier.registrations[signature].main;
            let query_index = 43 + signature;
            let next_query_index = verifier
                .next_query_index_v1(registration, query_index)
                .expect("canonical scalar next query");
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
            let base_columns = (0..registration.segment.base_width)
                .map(|column| trace.native_base_column_v1(registration, column))
                .collect::<Result<Vec<_>, _>>()
                .expect("bound scalar base columns");
            let aux_columns = (0..registration.segment.aux_width)
                .map(|column| trace.native_aux_column_v1(registration, column))
                .collect::<Result<Vec<_>, _>>()
                .expect("bound scalar auxiliary columns");
            let evaluate = |columns: &[ZeroizingMainTraceColumnV1], index: usize| {
                columns
                    .iter()
                    .map(|column| {
                        evaluate_native_fixed_column_v1(
                            registration.segment,
                            layout.common_lde_log2,
                            index,
                            |row| column[row],
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let base_current = evaluate(&base_columns, query_index);
            let base_next = evaluate(&base_columns, next_query_index);
            let aux_current = evaluate(&aux_columns, query_index);
            let aux_next = evaluate(&aux_columns, next_query_index);
            let opening = RegisteredOpenedRowsV1 {
                base_current: &base_current,
                base_next: &base_next,
                aux_current: &aux_current,
                aux_next: &aux_next,
            };
            let verifier_residues = verifier
                .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
                .expect("witness-free verifier residues");
            let verifier_fixed = verifier.fixed_openings[signature]
                .get(&query_index)
                .expect("verifier-generated fixed opening");
            let prover_residues = prover
                .constraint_residues_v1(registration, opening, verifier_fixed)
                .expect("prover residues over identical openings");
            assert_eq!(
                verifier_residues, prover_residues,
                "signature {signature} prover/verifier scalar residues"
            );
            assert_eq!(
                verifier_residues.len(),
                P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
            );
            let alphas = vec![E::ONE; registration.segment.constraint_count];
            assert_eq!(
                prover
                    .composition_value_v1(registration, x, opening, verifier_fixed, &alphas,)
                    .expect("scalar prover composition"),
                accumulator_quotient_value_v1(registration.segment, x, &prover_residues, &alphas,)
                    .expect("direct scalar quotient")
            );
        }
        let mut routed = MainTraceGroupProviderV1::P256Scalar(&mut trace);
        assert_eq!(routed.native_trace_log2_v1(), 8);
        assert_eq!(
            routed
                .source_mut_v1()
                .native_aux_column_v1(first_registration.main, 0)
                .expect("closed production log8 trace route")
                .len(),
            first_registration.main.segment.trace_size()
        );
    }

    fn evaluate_native_fixed_column_v1(
        segment: SegmentLayoutV1,
        common_lde_log2: u8,
        index: usize,
        mut native_value: impl FnMut(usize) -> F,
    ) -> F {
        let trace_root =
            goldilocks_primitive_root_v1(segment.trace_log2).expect("native trace root");
        let common_root = goldilocks_primitive_root_v1(common_lde_log2).expect("common LDE root");
        let mut coefficients = (0..segment.trace_size())
            .map(|row| native_value(row))
            .collect::<Vec<_>>();
        goldilocks_ifft_v1(&mut coefficients, trace_root).expect("fixed-column interpolation");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(index as u128));
        coefficients
            .iter()
            .rev()
            .fold(F::ZERO, |value, coefficient| value.mul(x).add(*coefficient))
    }

    fn evaluate_main_p256_log5_zero_opening_v1(
        source: &mut MainP256Log5VerifierConstraintSourceV1<'_>,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = source.binding_index_v1(registration)?;
        let next_query_index = source.next_query_index_v1(registration_index, query_index)?;
        let root =
            goldilocks_primitive_root_v1(source.common_lde_log2).expect("common MAIN LDE root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base_current = vec![F::ZERO; registration.segment.base_width];
        let base_next = vec![F::ZERO; registration.segment.base_width];
        let aux_current = vec![F::ZERO; registration.segment.aux_width];
        let aux_next = vec![F::ZERO; registration.segment.aux_width];
        source.constraint_residues_v1(
            registration,
            query_index,
            next_query_index,
            x,
            RegisteredOpenedRowsV1 {
                base_current: &base_current,
                base_next: &base_next,
                aux_current: &aux_current,
                aux_next: &aux_next,
            },
        )
    }

    #[test]
    fn main_p256_log5_registration_order_widths_and_fixed_ranges_are_exact() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let bindings =
            canonical_p256_main_log5_bindings_v1(&layout).expect("canonical log-five bindings");
        assert_eq!(bindings.len(), P256_MAIN_LOG5_REGISTRATION_COUNT_V1);
        assert_eq!(
            bindings
                .iter()
                .map(|binding| (
                    binding.p256.signature_v1(),
                    binding.p256.adapter_v1(),
                    binding.p256.local_instance_v1(),
                ))
                .collect::<Vec<_>>(),
            [
                (0, P256MainAdapterV1::Reduction, 0),
                (0, P256MainAdapterV1::Reduction, 1),
                (1, P256MainAdapterV1::Reduction, 0),
                (1, P256MainAdapterV1::Reduction, 1),
                (2, P256MainAdapterV1::Reduction, 0),
                (2, P256MainAdapterV1::Reduction, 1),
                (3, P256MainAdapterV1::Reduction, 0),
                (3, P256MainAdapterV1::Reduction, 1),
                (4, P256MainAdapterV1::Reduction, 0),
                (4, P256MainAdapterV1::Reduction, 1),
                (4, P256MainAdapterV1::WalletLowS, 0),
            ]
        );
        let mut next_base = 0;
        let mut next_aux = 0;
        for binding in bindings.iter().copied() {
            assert_eq!(binding.main.base_start, next_base);
            assert_eq!(binding.main.aux_start, next_aux);
            assert_eq!(
                p256_main_registration_from_main_layout_v1(binding.main)
                    .expect("shared MAIN-to-P256 mapping"),
                binding.p256
            );
            next_base = binding.main.base_end().expect("base end");
            next_aux = binding.main.aux_end().expect("aux end");
        }
        assert_eq!(next_base, P256_MAIN_LOG5_BASE_WIDTH_V1);
        assert_eq!(next_aux, P256_MAIN_LOG5_AUX_WIDTH_V1);

        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        for binding in bindings.iter().copied() {
            let shape = binding.p256.shape_v1().expect("canonical P-256 shape");
            let mut output = vec![F::ZERO; shape.trace_size];
            fixed
                .fill_fixed_column_v1(binding.p256, shape.fixed_width - 1, &mut output)
                .expect("last fixed column");
            assert!(
                fixed
                    .fill_fixed_column_v1(binding.p256, shape.fixed_width, &mut output)
                    .is_err(),
                "registration {:?} accepted fixed column at its exclusive end",
                binding.p256
            );
            output.pop();
            assert!(
                fixed
                    .fill_fixed_column_v1(binding.p256, 0, &mut output)
                    .is_err(),
                "registration {:?} accepted a short fixed column",
                binding.p256
            );
        }

        let log5_index = layout
            .registered_segments
            .iter()
            .position(|registration| *registration == bindings[0].main)
            .expect("log-five registration position");
        let mutations: [fn(&mut RegisteredSegmentLayoutV1); 5] = [
            |registration: &mut RegisteredSegmentLayoutV1| {
                registration.base_start = registration.base_start.saturating_add(1);
            },
            |registration: &mut RegisteredSegmentLayoutV1| {
                registration.aux_start = registration.aux_start.saturating_add(1);
            },
            |registration: &mut RegisteredSegmentLayoutV1| {
                registration.segment.instance = registration.segment.instance.saturating_add(1);
            },
            |registration: &mut RegisteredSegmentLayoutV1| {
                registration.segment.base_width = registration.segment.base_width.saturating_add(1);
            },
            |registration: &mut RegisteredSegmentLayoutV1| {
                registration.segment.fixed_width =
                    registration.segment.fixed_width.saturating_add(1);
            },
        ];
        for mutate in mutations {
            let mut forged = layout.clone();
            mutate(&mut forged.registered_segments[log5_index]);
            assert!(canonical_p256_main_log5_bindings_v1(&forged).is_err());
        }
    }

    #[test]
    fn main_p256_log5_central_trace_views_and_prover_are_end_to_end_closed() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let bindings =
            canonical_p256_main_log5_bindings_v1(&layout).expect("canonical log-five bindings");
        let mut base_source = p256_main_base_source_fixture_v1();
        {
            let mut routed_view =
                MainP256Log5TraceGroupSourceV1::for_base_v1(&layout, &base_source)
                    .expect("routed pre-X5B1 log-five view");
            let mut provider = MainTraceGroupProviderV1::Log5(&mut routed_view);
            assert_eq!(provider.native_trace_log2_v1(), 5);
            let routed = provider
                .source_mut_v1()
                .native_base_column_v1(bindings[0].main, 0)
                .expect("closed log-five trace-provider route");
            assert_eq!(routed.len(), bindings[0].main.segment.trace_size());
        }
        let mut base_parity = Vec::new();
        base_parity
            .try_reserve_exact(P256_MAIN_LOG5_REGISTRATION_COUNT_V1)
            .expect("bounded base-parity fixture");
        {
            let mut base_view = MainP256Log5TraceGroupSourceV1::for_base_v1(&layout, &base_source)
                .expect("pre-X5B1 log-five view");
            for binding in bindings.iter().copied() {
                let mut column = base_view
                    .native_base_column_v1(binding.main, binding.main.segment.base_width - 1)
                    .expect("last challenge-independent column");
                assert_eq!(column.len(), binding.main.segment.trace_size());
                column.zeroize_private_v1();
                assert!(column.is_empty());
                base_parity.push((
                    binding.main,
                    base_view
                        .native_base_column_v1(binding.main, binding.main.segment.base_width - 1)
                        .expect("retained zeroizing base-parity column"),
                ));
                assert!(
                    base_view
                        .native_base_column_v1(binding.main, binding.main.segment.base_width)
                        .is_err()
                );
                assert!(matches!(
                    base_view.native_aux_column_v1(binding.main, 0),
                    Err(ZkX509StarkErrorV1::TranscriptMismatch)
                ));
                assert!(
                    base_view
                        .native_aux_column_v1(binding.main, binding.main.segment.aux_width)
                        .is_err()
                );
            }
        }

        let post_base = p256_main_provider_post_base_fixture_v1();
        let mut bound = base_source
            .bind_v1(post_base)
            .expect("single central P-256 phase transition");
        {
            let mut bound_view = MainP256Log5TraceGroupSourceV1::for_bound_v1(&layout, &bound)
                .expect("post-X5B1 log-five view");
            let prover = MainP256Log5ProverConstraintSourceV1::for_main_v1(&layout, &bound)
                .expect("bound log-five prover");
            let mut streamed = 0;
            prover
                .stream_fixed_polynomials_v1(|registration, local_column, coefficients| {
                    assert!(bindings.iter().any(|binding| binding.main == registration));
                    assert!(local_column < registration.segment.fixed_width);
                    assert_eq!(coefficients.len(), registration.segment.trace_size());
                    streamed += 1;
                    Ok(())
                })
                .expect("all verifier-owned log-five fixed polynomials");
            assert_eq!(
                streamed,
                10 * P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1 + P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1
            );

            for binding in bindings.iter().copied() {
                let before_binding = base_parity
                    .iter()
                    .find(|(registration, _)| *registration == binding.main)
                    .expect("pre-bind base parity column");
                let after_binding = bound_view
                    .native_base_column_v1(binding.main, binding.main.segment.base_width - 1)
                    .expect("post-bind base parity column");
                assert_eq!(&*after_binding, &*before_binding.1);
                assert!(
                    bound_view
                        .native_base_column_v1(binding.main, binding.main.segment.base_width)
                        .is_err()
                );
                assert!(
                    bound_view
                        .native_aux_column_v1(binding.main, binding.main.segment.aux_width)
                        .is_err()
                );
                let mut invalid_fixed = vec![F::ZERO; binding.main.segment.trace_size()];
                assert!(
                    bound
                        .fill_fixed_column_v1(
                            binding.p256,
                            binding.main.segment.fixed_width,
                            &mut invalid_fixed,
                        )
                        .is_err()
                );

                let mut base_columns = Vec::new();
                base_columns
                    .try_reserve_exact(binding.main.segment.base_width)
                    .expect("bounded test base columns");
                for column in 0..binding.main.segment.base_width {
                    base_columns.push(
                        bound_view
                            .native_base_column_v1(binding.main, column)
                            .expect("bound base replay"),
                    );
                }
                let mut aux_columns = Vec::new();
                aux_columns
                    .try_reserve_exact(binding.main.segment.aux_width)
                    .expect("bounded test auxiliary columns");
                for column in 0..binding.main.segment.aux_width {
                    aux_columns.push(
                        bound_view
                            .native_aux_column_v1(binding.main, column)
                            .expect("bound auxiliary replay"),
                    );
                }
                let mut fixed_columns = Vec::new();
                fixed_columns
                    .try_reserve_exact(binding.main.segment.fixed_width)
                    .expect("bounded test fixed columns");
                for column in 0..binding.main.segment.fixed_width {
                    let mut values = vec![F::ZERO; binding.main.segment.trace_size()];
                    bound
                        .fill_fixed_column_v1(binding.p256, column, &mut values)
                        .expect("verifier-owned fixed replay");
                    fixed_columns.push(values);
                }

                for row in 0..binding.main.segment.trace_size() {
                    let next = (row + 1) % binding.main.segment.trace_size();
                    let base_current = base_columns
                        .iter()
                        .map(|column| column[row])
                        .collect::<Vec<_>>();
                    let base_next = base_columns
                        .iter()
                        .map(|column| column[next])
                        .collect::<Vec<_>>();
                    let aux_current = aux_columns
                        .iter()
                        .map(|column| column[row])
                        .collect::<Vec<_>>();
                    let aux_next = aux_columns
                        .iter()
                        .map(|column| column[next])
                        .collect::<Vec<_>>();
                    let fixed_row = fixed_columns
                        .iter()
                        .map(|column| column[row])
                        .collect::<Vec<_>>();
                    let opening = RegisteredOpenedRowsV1 {
                        base_current: &base_current,
                        base_next: &base_next,
                        aux_current: &aux_current,
                        aux_next: &aux_next,
                    };
                    let residues = prover
                        .constraint_residues_v1(binding.main, opening, &fixed_row)
                        .expect("native log-five residues");
                    assert_eq!(residues.len(), binding.main.segment.constraint_count);
                    assert!(
                        residues.iter().all(|residue| *residue == F::ZERO),
                        "registration {:?}, native row {row}",
                        binding.p256,
                    );
                    let alphas = vec![E::ONE; binding.main.segment.constraint_count];
                    assert_eq!(
                        prover
                            .composition_value_v1(
                                binding.main,
                                F(GOLDILOCKS_GENERATOR_V1),
                                opening,
                                &fixed_row,
                                &alphas,
                            )
                            .expect("zero native composition"),
                        E::ZERO
                    );
                }
            }
        }
        for (_, column) in &mut base_parity {
            column.zeroize_private_v1();
        }
        base_parity.clear();
        bound.zeroize_private_v1();
        assert!(MainP256Log5TraceGroupSourceV1::for_bound_v1(&layout, &bound).is_err());
    }

    #[test]
    fn main_p256_log5_verifier_fixed_sampling_is_exact_bounded_and_transactional() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        let post_base = p256_main_provider_post_base_fixture_v1();
        let claims = p256_main_terminal_claims_fixture_v1();
        {
            let mut routed = MainP256Log5VerifierConstraintSourceV1::for_main_v1(
                &layout, &fixed, post_base, claims,
            )
            .expect("routed log-five verifier source");
            let registration = routed.bindings[0].main;
            let query_index = 5;
            let next_query_index = routed
                .next_query_index_v1(0, query_index)
                .expect("routed next coordinate");
            let root =
                goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common MAIN root");
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
            let base = vec![F::ZERO; registration.segment.base_width];
            let aux = vec![F::ZERO; registration.segment.aux_width];
            let opening = RegisteredOpenedRowsV1 {
                base_current: &base,
                base_next: &base,
                aux_current: &aux,
                aux_next: &aux,
            };
            let mut provider = MainOpenedGroupProviderV1::Log5(&mut routed);
            assert_eq!(provider.native_trace_log2_v1(), 5);
            assert_eq!(
                provider
                    .constraint_residues_v1(
                        registration,
                        query_index,
                        next_query_index,
                        x,
                        opening,
                    )
                    .expect("closed log-five verifier-provider route")
                    .len(),
                registration.segment.constraint_count
            );
        }
        let mut source =
            MainP256Log5VerifierConstraintSourceV1::for_main_v1(&layout, &fixed, post_base, claims)
                .expect("canonical log-five verifier source");
        let bindings = source.bindings.clone();
        for (registration_index, binding) in bindings.iter().copied().enumerate() {
            let query_index = registration_index * 13 + 7;
            let next_query_index = source
                .next_query_index_v1(registration_index, query_index)
                .expect("canonical next coordinate");
            let residues =
                evaluate_main_p256_log5_zero_opening_v1(&mut source, binding.main, query_index)
                    .expect("closed log-five residue evaluation");
            assert_eq!(residues.len(), binding.main.segment.constraint_count);
            assert_eq!(source.cached_openings_v1(registration_index), Some(2));

            let cache = &source.fixed_openings[registration_index];
            for index in [query_index, next_query_index] {
                let opened = cache.get(&index).expect("verifier-minted fixed opening");
                assert_eq!(opened.len(), binding.main.segment.fixed_width);
                for column in 0..binding.main.segment.fixed_width {
                    assert_eq!(
                        opened[column],
                        evaluate_native_fixed_column_v1(
                            binding.main.segment,
                            layout.common_lde_log2,
                            index,
                            |row| {
                                fixed
                                    .fixed_cell_v1(binding.p256, row, column)
                                    .expect("verifier-owned native fixed cell")
                            },
                        ),
                        "registration {registration_index}, query {index}, fixed column {column}",
                    );
                }
            }
        }

        let mut rejected =
            MainP256Log5VerifierConstraintSourceV1::for_main_v1(&layout, &fixed, post_base, claims)
                .expect("fresh log-five verifier source");
        let canonical = rejected.bindings[0].main;
        let mut forged_slice = canonical;
        forged_slice.base_start = forged_slice.base_start.saturating_add(1);
        let mut forged_instance = canonical;
        forged_instance.segment.instance = forged_instance.segment.instance.saturating_add(1);
        let mut forged_adapter = canonical;
        forged_adapter.segment.adapter = SegmentAdapterIdV1::P256LowS;
        for forged in [forged_slice, forged_instance, forged_adapter] {
            assert!(evaluate_main_p256_log5_zero_opening_v1(&mut rejected, forged, 0).is_err());
            assert_eq!(rejected.cached_openings_v1(0), Some(0));
        }

        let registration_index = rejected
            .binding_index_v1(canonical)
            .expect("canonical registration");
        let query_index = 3;
        let next_query_index = rejected
            .next_query_index_v1(registration_index, query_index)
            .expect("canonical next coordinate");
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common MAIN root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = vec![F::ZERO; canonical.segment.base_width];
        let aux = vec![F::ZERO; canonical.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    (next_query_index + 1) % layout.common_lde_size(),
                    x,
                    opening,
                )
                .is_err()
        );
        assert_eq!(rejected.cached_openings_v1(0), Some(0));
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    next_query_index,
                    x.add(F::ONE),
                    opening,
                )
                .is_err()
        );
        assert_eq!(rejected.cached_openings_v1(0), Some(0));
        let mut noncanonical = base.clone();
        noncanonical[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    next_query_index,
                    x,
                    RegisteredOpenedRowsV1 {
                        base_current: &noncanonical,
                        base_next: &base,
                        aux_current: &aux,
                        aux_next: &aux,
                    },
                )
                .is_err()
        );
        assert_eq!(rejected.cached_openings_v1(0), Some(0));

        let mut bounded =
            MainP256Log5VerifierConstraintSourceV1::for_main_v1(&layout, &fixed, post_base, claims)
                .expect("fresh bounded log-five verifier source");
        let stride = 1_usize << (layout.common_lde_log2 - 5);
        for query in 0..(VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 / 2) {
            bounded
                .ensure_fixed_openings_v1(0, [query, query + stride])
                .expect("opening within the exact verifier bound");
        }
        assert_eq!(
            bounded.cached_openings_v1(0),
            Some(VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
        );
        assert!(
            bounded
                .ensure_fixed_openings_v1(
                    0,
                    [
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 / 2,
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 / 2 + stride,
                    ],
                )
                .is_err()
        );
        assert_eq!(
            bounded.cached_openings_v1(0),
            Some(VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1)
        );
    }

    #[test]
    fn main_p256_log5_terminal_selectors_reject_coordinated_chain_forgeries() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        let post_base = p256_main_provider_post_base_fixture_v1();
        let canonical_claims = p256_main_terminal_claims_fixture_v1();

        for (registration_index, mut forged_claims) in [
            (0, canonical_claims),
            (P256_MAIN_LOG5_REGISTRATION_COUNT_V1 - 1, canonical_claims),
        ] {
            if registration_index == 0 {
                forged_claims.certificate_or_crl[0].cross_sources[2].terminal[0] = F::ONE;
                forged_claims.certificate_or_crl[0].cross_sources[3].start[0] = F::ONE;
            } else {
                forged_claims.wallet.cross_sources[4].terminal[0] = F::ONE;
                forged_claims.wallet.sink[0] = F::ONE;
            }
            main_p256_terminal_registrations_v1(&forged_claims)
                .expect("coordinated host-side chain remains equal");

            let mut canonical = MainP256Log5VerifierConstraintSourceV1::for_main_v1(
                &layout,
                &fixed,
                post_base,
                canonical_claims,
            )
            .expect("canonical log-five verifier");
            let mut forged = MainP256Log5VerifierConstraintSourceV1::for_main_v1(
                &layout,
                &fixed,
                post_base,
                forged_claims,
            )
            .expect("coherently forged host claims");
            let registration = canonical.bindings[registration_index].main;
            let canonical_residues =
                evaluate_main_p256_log5_zero_opening_v1(&mut canonical, registration, 0)
                    .expect("canonical terminal residues");
            let forged_residues =
                evaluate_main_p256_log5_zero_opening_v1(&mut forged, registration, 0)
                    .expect("forged terminal residues");
            assert_eq!(
                canonical_residues.len(),
                registration.segment.constraint_count
            );
            assert!(
                canonical_residues[canonical_residues.len() - P256_CROSS_TRACE_LANES_V1..]
                    .iter()
                    .all(|residue| *residue == F::ZERO)
            );
            assert!(
                forged_residues[forged_residues.len() - P256_CROSS_TRACE_LANES_V1..]
                    .iter()
                    .any(|residue| *residue != F::ZERO),
                "registration {registration_index} coordinated terminal forgery escaped its fixed selector",
            );
        }
    }

    fn evaluate_main_p256_log16_zero_opening_v1(
        source: &mut MainP256Log16VerifierConstraintSourceV1<'_>,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        let registration_index = source.binding_index_v1(registration)?;
        let next_query_index = source.next_query_index_v1(registration_index, query_index)?;
        let root =
            goldilocks_primitive_root_v1(source.common_lde_log2).expect("common MAIN LDE root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base_current = vec![F::ZERO; registration.segment.base_width];
        let base_next = vec![F::ZERO; registration.segment.base_width];
        let aux_current = vec![F::ZERO; registration.segment.aux_width];
        let aux_next = vec![F::ZERO; registration.segment.aux_width];
        source.constraint_residues_v1(
            registration,
            query_index,
            next_query_index,
            x,
            RegisteredOpenedRowsV1 {
                base_current: &base_current,
                base_next: &base_next,
                aux_current: &aux_current,
                aux_next: &aux_next,
            },
        )
    }

    #[test]
    fn main_p256_log16_registration_order_ranges_and_stride_are_exact() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let bindings =
            canonical_p256_main_log16_bindings_v1(&layout).expect("canonical log-sixteen bindings");
        assert_eq!(bindings.len(), P256_MAIN_LOG16_REGISTRATION_COUNT_V1);
        assert_eq!(
            bindings
                .iter()
                .map(|binding| (
                    binding.p256.signature_v1(),
                    binding.p256.adapter_v1(),
                    binding.p256.local_instance_v1(),
                ))
                .collect::<Vec<_>>(),
            [
                (0, P256MainAdapterV1::WindowBatch, 0),
                (1, P256MainAdapterV1::WindowBatch, 0),
                (2, P256MainAdapterV1::WindowBatch, 0),
                (3, P256MainAdapterV1::WindowBatch, 0),
                (4, P256MainAdapterV1::WindowBatch, 0),
                (0, P256MainAdapterV1::BindingSink, 0),
                (1, P256MainAdapterV1::BindingSink, 0),
                (2, P256MainAdapterV1::BindingSink, 0),
                (3, P256MainAdapterV1::BindingSink, 0),
                (4, P256MainAdapterV1::BindingSink, 0),
            ]
        );
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.main.base_start)
                .collect::<Vec<_>>(),
            [0, 61, 122, 183, 244, 305, 330, 355, 380, 405]
        );
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.main.aux_start)
                .collect::<Vec<_>>(),
            [0, 37, 74, 111, 148, 185, 223, 261, 299, 337]
        );
        let mut next_base = 0;
        let mut next_aux = 0;
        for binding in bindings.iter().copied() {
            assert_eq!(binding.main.base_start, next_base);
            assert_eq!(binding.main.aux_start, next_aux);
            assert_eq!(
                p256_main_registration_from_main_layout_v1(binding.main)
                    .expect("shared MAIN-to-P256 mapping"),
                binding.p256
            );
            next_base = binding.main.base_end().expect("base end");
            next_aux = binding.main.aux_end().expect("aux end");
        }
        assert_eq!(next_base, P256_MAIN_LOG16_BASE_WIDTH_V1);
        assert_eq!(next_aux, P256_MAIN_LOG16_AUX_WIDTH_V1);
        let group = layout.trace_groups[bindings[0].main.trace_group];
        assert_eq!(group.base_width, P256_MAIN_LOG16_BASE_WIDTH_V1);
        assert_eq!(group.aux_width, P256_MAIN_LOG16_AUX_WIDTH_V1);
        assert_eq!(group.column_chunks, P256_MAIN_LOG16_PHYSICAL_CHUNKS_V1);

        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        for binding in bindings.iter().copied() {
            let shape = binding.p256.shape_v1().expect("canonical P-256 shape");
            assert!(
                fixed
                    .fixed_cell_v1(binding.p256, shape.trace_size - 1, shape.fixed_width - 1)
                    .is_ok()
            );
            assert!(
                fixed
                    .fixed_cell_v1(binding.p256, shape.trace_size, 0)
                    .is_err()
            );
            assert!(
                fixed
                    .fixed_cell_v1(binding.p256, 0, shape.fixed_width)
                    .is_err()
            );
        }

        let claims = p256_main_terminal_claims_fixture_v1();
        let source = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            p256_main_provider_post_base_fixture_v1(),
            &claims,
        )
        .expect("canonical log-sixteen verifier");
        assert_eq!(
            source
                .next_query_index_v1(0, 7)
                .expect("exact non-wrapping stride"),
            7 + P256_MAIN_LOG16_NEXT_STRIDE_V1
        );
        assert_eq!(
            source
                .next_query_index_v1(9, layout.common_lde_size() - 256)
                .expect("exact wrapping stride"),
            256
        );

        let window_position = layout
            .registered_segments
            .iter()
            .position(|registration| *registration == bindings[0].main)
            .expect("first window position");
        let sink_position = layout
            .registered_segments
            .iter()
            .position(|registration| *registration == bindings[5].main)
            .expect("first sink position");
        let mut forgeries = Vec::new();
        let mut forged = layout.clone();
        forged.registered_segments[window_position].base_start += 1;
        forgeries.push(forged);
        let mut forged = layout.clone();
        forged.registered_segments[window_position].aux_start += 1;
        forgeries.push(forged);
        let mut forged = layout.clone();
        forged.registered_segments[window_position]
            .segment
            .fixed_width += 1;
        forgeries.push(forged);
        let mut forged = layout.clone();
        forged.registered_segments[sink_position].segment.instance -= 1;
        forgeries.push(forged);
        let mut forged = layout.clone();
        forged
            .registered_segments
            .swap(window_position, sink_position);
        forgeries.push(forged);
        let mut forged = layout.clone();
        forged.trace_groups[bindings[0].main.trace_group].column_chunks += 1;
        forgeries.push(forged);
        for forged in &forgeries {
            assert!(
                canonical_p256_main_log16_bindings_v1(forged).is_err(),
                "every caller-shaped range or topology mutation must reject"
            );
        }
    }

    #[test]
    fn main_p256_log16_borrowed_phases_prover_and_verifier_match() {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("main-p256-log16".to_owned())
                .stack_size(32 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    let layout = AggregateProofLayoutV1::for_full_profile_v1()
                        .expect("canonical MAIN layout");
                    let bindings = canonical_p256_main_log16_bindings_v1(&layout)
                        .expect("canonical log-sixteen bindings");
                    let mut central = p256_main_base_source_fixture_v1();
                    let selected = [0_usize, 7, 9];
                    let mut base_parity = Vec::new();
                    {
                        let mut routed_view =
                            MainP256Log16TraceGroupSourceV1::for_base_v1(&layout, &central)
                                .expect("routed pre-X5B1 log-sixteen view");
                        let mut routed = MainTraceGroupProviderV1::Log16(&mut routed_view);
                        assert_eq!(routed.native_trace_log2_v1(), 16);
                        assert_eq!(
                            routed
                                .source_mut_v1()
                                .native_base_column_v1(bindings[0].main, 0)
                                .expect("closed production log-sixteen trace route")
                                .len(),
                            bindings[0].main.segment.trace_size()
                        );
                    }
                    {
                        let mut base_view =
                            MainP256Log16TraceGroupSourceV1::for_base_v1(&layout, &central)
                                .expect("borrowed pre-X5B1 log-sixteen view");
                        for index in selected {
                            let binding = bindings[index];
                            base_parity.push((
                                binding.main,
                                base_view
                                    .native_base_column_v1(
                                        binding.main,
                                        binding.main.segment.base_width - 1,
                                    )
                                    .expect("challenge-independent base column"),
                            ));
                            assert!(matches!(
                                base_view.native_aux_column_v1(binding.main, 0),
                                Err(ZkX509StarkErrorV1::TranscriptMismatch)
                            ));
                            assert!(
                                base_view
                                    .native_base_column_v1(
                                        binding.main,
                                        binding.main.segment.base_width,
                                    )
                                    .is_err()
                            );
                            assert!(
                                base_view
                                    .native_aux_column_v1(
                                        binding.main,
                                        binding.main.segment.aux_width,
                                    )
                                    .is_err()
                            );
                        }
                    }

                    let post_base = p256_main_provider_post_base_fixture_v1();
                    let mut bound = central
                        .bind_v1(post_base)
                        .expect("single opaque central P-256 phase transition");
                    {
                        let mut bound_view =
                            MainP256Log16TraceGroupSourceV1::for_bound_v1(&layout, &bound)
                                .expect("borrowed bound log-sixteen view");
                        for (registration, before) in &base_parity {
                            assert_eq!(
                                &*bound_view
                                    .native_base_column_v1(
                                        *registration,
                                        registration.segment.base_width - 1,
                                    )
                                    .expect("post-bind base replay"),
                                &**before
                            );
                            assert_eq!(
                                bound_view
                                    .native_aux_column_v1(*registration, 0)
                                    .expect("post-bind auxiliary replay")
                                    .len(),
                                registration.segment.trace_size()
                            );
                        }

                        let prover =
                            MainP256Log16ProverConstraintSourceV1::for_main_v1(&layout, &bound)
                                .expect("bound log-sixteen prover source");
                        let mut streamed = 0_usize;
                        assert!(matches!(
                            prover.stream_fixed_polynomials_v1(
                                |registration, local_column, coefficients| {
                                    assert_eq!(registration, bindings[0].main);
                                    assert_eq!(local_column, 0);
                                    assert_eq!(
                                        coefficients.len(),
                                        registration.segment.trace_size()
                                    );
                                    streamed += 1;
                                    Err(ZkX509StarkErrorV1::ProfileMismatch)
                                },
                            ),
                            Err(ZkX509StarkErrorV1::ProfileMismatch)
                        ));
                        assert_eq!(streamed, 1);

                        let claims = bound
                            .terminal_claims_v1()
                            .expect("central five-signature terminal claims");
                        let fixed = P256MainVerifierFixedSourceV1::new_v1()
                            .expect("independent witness-free fixed source");
                        let mut verifier = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
                            &layout, &fixed, post_base, &claims,
                        )
                        .expect("witness-free log-sixteen verifier source");
                        let root = goldilocks_primitive_root_v1(layout.common_lde_log2)
                            .expect("common MAIN root");
                        for registration_index in [0_usize, 7] {
                            let registration = bindings[registration_index].main;
                            let query_index = 29 + registration_index;
                            let next_query_index = verifier
                                .next_query_index_v1(registration_index, query_index)
                                .expect("canonical +512 next coordinate");
                            let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
                            let base = vec![F::ZERO; registration.segment.base_width];
                            let aux = vec![F::ZERO; registration.segment.aux_width];
                            let opening = RegisteredOpenedRowsV1 {
                                base_current: &base,
                                base_next: &base,
                                aux_current: &aux,
                                aux_next: &aux,
                            };
                            let verifier_residues = verifier
                                .constraint_residues_v1(
                                    registration,
                                    query_index,
                                    next_query_index,
                                    x,
                                    opening,
                                )
                                .expect("witness-free opened residues");
                            let verifier_fixed = verifier.fixed_openings[registration_index]
                                .get(&query_index)
                                .expect("verifier-generated current fixed opening");
                            let prover_residues = prover
                                .constraint_residues_v1(registration, opening, verifier_fixed)
                                .expect("bound prover residues");
                            assert_eq!(verifier_residues, prover_residues);
                            assert_eq!(
                                verifier_residues.len(),
                                registration.segment.constraint_count
                            );
                            let alphas = vec![E::ONE; registration.segment.constraint_count];
                            assert_eq!(
                                prover
                                    .composition_value_v1(
                                        registration,
                                        x,
                                        opening,
                                        verifier_fixed,
                                        &alphas,
                                    )
                                    .expect("bound prover composition"),
                                accumulator_quotient_value_v1(
                                    registration.segment,
                                    x,
                                    &verifier_residues,
                                    &alphas,
                                )
                                .expect("direct residue quotient")
                            );
                        }

                        let registration = bindings[0].main;
                        let query_index = 29;
                        let next_query_index = verifier
                            .next_query_index_v1(0, query_index)
                            .expect("cached +512 next coordinate");
                        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
                        let base = vec![F::ZERO; registration.segment.base_width];
                        let aux = vec![F::ZERO; registration.segment.aux_width];
                        let opening = RegisteredOpenedRowsV1 {
                            base_current: &base,
                            base_next: &base,
                            aux_current: &aux,
                            aux_next: &aux,
                        };
                        let mut routed = MainOpenedGroupProviderV1::Log16(&mut verifier);
                        assert_eq!(routed.native_trace_log2_v1(), 16);
                        assert_eq!(
                            routed
                                .constraint_residues_v1(
                                    registration,
                                    query_index,
                                    next_query_index,
                                    x,
                                    opening,
                                )
                                .expect("closed production log-sixteen verifier route")
                                .len(),
                            registration.segment.constraint_count
                        );
                    }
                    for (_, column) in &mut base_parity {
                        column.zeroize_private_v1();
                    }
                    bound.zeroize_private_v1();
                    assert!(
                        MainP256Log16TraceGroupSourceV1::for_bound_v1(&layout, &bound).is_err()
                    );
                })
                .expect("spawn large-stack log-sixteen test")
                .join()
                .expect("large-stack log-sixteen test");
        });
    }

    #[test]
    fn main_p256_log16_verifier_sampling_is_fixed_exact_and_transactional() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        let post_base = p256_main_provider_post_base_fixture_v1();
        let claims = p256_main_terminal_claims_fixture_v1();
        let mut rejected = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
            &layout, &fixed, post_base, &claims,
        )
        .expect("fresh log-sixteen verifier");
        let canonical = rejected.bindings[0].main;
        let query_index = 7;
        let next_query_index = rejected
            .next_query_index_v1(0, query_index)
            .expect("canonical +512 next coordinate");
        assert_eq!(
            next_query_index - query_index,
            P256_MAIN_LOG16_NEXT_STRIDE_V1
        );
        let root = goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common MAIN root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(query_index as u128));
        let base = vec![F::ZERO; canonical.segment.base_width];
        let aux = vec![F::ZERO; canonical.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let mut forged_slice = canonical;
        forged_slice.base_start += 1;
        let wrong_registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("wrong-log registration");
        for registration in [forged_slice, wrong_registration] {
            assert!(
                rejected
                    .constraint_residues_v1(
                        registration,
                        query_index,
                        next_query_index,
                        x,
                        opening,
                    )
                    .is_err()
            );
        }
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    layout.common_lde_size(),
                    next_query_index,
                    x,
                    opening,
                )
                .is_err()
        );
        assert!(
            rejected
                .constraint_residues_v1(canonical, query_index, next_query_index + 1, x, opening,)
                .is_err()
        );
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    next_query_index,
                    x.add(F::ONE),
                    opening,
                )
                .is_err()
        );
        let short_base = vec![F::ZERO; canonical.segment.base_width - 1];
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    next_query_index,
                    x,
                    RegisteredOpenedRowsV1 {
                        base_current: &short_base,
                        base_next: &base,
                        aux_current: &aux,
                        aux_next: &aux,
                    },
                )
                .is_err()
        );
        let mut noncanonical_aux = aux.clone();
        noncanonical_aux[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert!(
            rejected
                .constraint_residues_v1(
                    canonical,
                    query_index,
                    next_query_index,
                    x,
                    RegisteredOpenedRowsV1 {
                        base_current: &base,
                        base_next: &base,
                        aux_current: &noncanonical_aux,
                        aux_next: &aux,
                    },
                )
                .is_err()
        );
        assert!(
            rejected.fixed_openings.iter().all(BTreeMap::is_empty),
            "all malformed coordinates and rows must reject before sampling"
        );

        let bindings = rejected.bindings.clone();
        for registration_index in [0_usize, 7] {
            let binding = bindings[registration_index];
            let query_index = 17 + registration_index;
            let next_query_index = rejected
                .next_query_index_v1(registration_index, query_index)
                .expect("canonical +512 next coordinate");
            evaluate_main_p256_log16_zero_opening_v1(&mut rejected, binding.main, query_index)
                .expect("valid witness-free fixed sampling");
            assert_eq!(rejected.cached_openings_v1(registration_index), Some(2));
            let snapshot = rejected.fixed_openings[registration_index].clone();
            evaluate_main_p256_log16_zero_opening_v1(&mut rejected, binding.main, query_index)
                .expect("fixed-opening cache reuse");
            assert_eq!(rejected.fixed_openings[registration_index], snapshot);

            for local_column in [0, binding.main.segment.fixed_width - 1] {
                let mut native = vec![F::ZERO; binding.main.segment.trace_size()];
                fixed
                    .fill_fixed_column_v1(binding.p256, local_column, &mut native)
                    .expect("direct verifier-owned fixed column");
                for index in [query_index, next_query_index] {
                    assert_eq!(
                        rejected.fixed_openings[registration_index][&index][local_column],
                        evaluate_native_fixed_column_v1(
                            binding.main.segment,
                            layout.common_lde_log2,
                            index,
                            |row| native[row],
                        ),
                        "registration {registration_index}, query {index}, fixed column \
                         {local_column}",
                    );
                }
            }
        }
        assert_eq!(rejected.cached_openings_v1(0), Some(2));
        assert_eq!(rejected.cached_openings_v1(7), Some(2));
        assert!(
            rejected
                .fixed_openings
                .iter()
                .enumerate()
                .all(|(index, cache)| matches!(index, 0 | 7) || cache.is_empty())
        );

        let mut transactional = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
            &layout, &fixed, post_base, &claims,
        )
        .expect("fresh transactional log-sixteen verifier");
        transactional.bindings[0].main.segment.fixed_width += 1;
        assert!(
            transactional
                .ensure_fixed_openings_v1(0, [0, P256_MAIN_LOG16_NEXT_STRIDE_V1])
                .is_err()
        );
        assert_eq!(transactional.cached_openings_v1(0), Some(0));

        let mut bounded = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
            &layout, &fixed, post_base, &claims,
        )
        .expect("fresh bounded log-sixteen verifier");
        let width = bounded.bindings[0].main.segment.fixed_width;
        for index in 0..VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
            bounded.fixed_openings[0].insert(index, vec![F::ZERO; width]);
        }
        let full_cache = bounded.fixed_openings[0].clone();
        assert!(
            bounded
                .ensure_fixed_openings_v1(
                    0,
                    [
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1,
                        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 + 1,
                    ],
                )
                .is_err()
        );
        assert_eq!(bounded.fixed_openings[0], full_cache);
        assert!(bounded.fixed_openings[1].is_empty());
    }

    #[test]
    fn main_p256_log16_terminal_selectors_reject_coordinated_forgeries() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let fixed =
            P256MainVerifierFixedSourceV1::new_v1().expect("verifier-owned P-256 fixed source");
        let post_base = p256_main_provider_post_base_fixture_v1();
        let mut canonical_claims = p256_main_terminal_claims_fixture_v1();
        canonical_claims.certificate_or_crl[0].buses.window_scalar =
            [F::ZERO; P256_SCALAR_BIT_BUS_LANES_V1];
        canonical_claims.certificate_or_crl[0]
            .buses
            .scalar_bus_window = [F::ZERO; P256_SCALAR_BIT_BUS_LANES_V1];
        let mut verifier = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
            &layout,
            &fixed,
            post_base,
            &canonical_claims,
        )
        .expect("canonical log-sixteen verifier");
        let bindings = verifier.bindings.clone();
        let challenges = verifier.challenges;

        let window = bindings[0];
        let window_residues =
            evaluate_main_p256_log16_zero_opening_v1(&mut verifier, window.main, 0)
                .expect("canonical window terminal residues");
        let window_fixed = verifier.fixed_openings[0][&0].clone();
        let window_base = vec![F::ZERO; window.main.segment.base_width];
        let window_aux = vec![F::ZERO; window.main.segment.aux_width];
        let window_opening = RegisteredOpenedRowsV1 {
            base_current: &window_base,
            base_next: &window_base,
            aux_current: &window_aux,
            aux_next: &window_aux,
        };
        let window_tail = P256_CROSS_TRACE_LANES_V1 + P256_SCALAR_BIT_BUS_LANES_V1;
        assert!(
            window_residues[window_residues.len() - window_tail..]
                .iter()
                .all(|residue| *residue == F::ZERO)
        );

        let mut forged_cross = canonical_claims;
        forged_cross.certificate_or_crl[0].cross_sources[1].terminal[0] = F::ONE;
        forged_cross.certificate_or_crl[0].cross_sources[2].start[0] = F::ONE;
        let forged_cross_terminal = main_p256_terminal_registration_v1(&forged_cross, 0)
            .expect("coordinated host-side window chain");
        let forged_cross_residues = p256_opened_residues_v1(
            window.main,
            window_opening,
            &window_fixed,
            challenges,
            &forged_cross_terminal,
        )
        .expect("forged window-cross residues");
        assert!(
            forged_cross_residues[forged_cross_residues.len() - window_tail
                ..forged_cross_residues.len() - P256_SCALAR_BIT_BUS_LANES_V1]
                .iter()
                .any(|residue| *residue != F::ZERO)
        );

        let mut forged_scalar = canonical_claims;
        forged_scalar.certificate_or_crl[0].buses.window_scalar[0] = F::ONE;
        forged_scalar.certificate_or_crl[0].buses.scalar_bus_window[0] = F::ONE;
        let forged_scalar_terminal = main_p256_terminal_registration_v1(&forged_scalar, 0)
            .expect("coordinated host-side scalar bus");
        let forged_scalar_residues = p256_opened_residues_v1(
            window.main,
            window_opening,
            &window_fixed,
            challenges,
            &forged_scalar_terminal,
        )
        .expect("forged window-scalar residues");
        assert!(
            forged_scalar_residues[forged_scalar_residues.len() - P256_SCALAR_BIT_BUS_LANES_V1..]
                .iter()
                .any(|residue| *residue != F::ZERO)
        );

        let sink = bindings[P256_SIGNATURE_COUNT_V1];
        let sink_residues = evaluate_main_p256_log16_zero_opening_v1(&mut verifier, sink.main, 1)
            .expect("canonical sink terminal residues");
        let sink_fixed = verifier.fixed_openings[P256_SIGNATURE_COUNT_V1][&1].clone();
        let sink_base = vec![F::ZERO; sink.main.segment.base_width];
        let sink_aux = vec![F::ZERO; sink.main.segment.aux_width];
        let sink_opening = RegisteredOpenedRowsV1 {
            base_current: &sink_base,
            base_next: &sink_base,
            aux_current: &sink_aux,
            aux_next: &sink_aux,
        };
        assert!(
            sink_residues[sink_residues.len() - P256_CROSS_TRACE_LANES_V1..]
                .iter()
                .all(|residue| *residue == F::ZERO)
        );
        let mut forged_sink = canonical_claims;
        forged_sink.certificate_or_crl[0].cross_sources[3].terminal[0] = F::ONE;
        forged_sink.certificate_or_crl[0].sink[0] = F::ONE;
        let forged_sink_terminal = main_p256_terminal_registration_v1(&forged_sink, 0)
            .expect("coordinated host-side sink chain");
        let forged_sink_residues = p256_opened_residues_v1(
            sink.main,
            sink_opening,
            &sink_fixed,
            challenges,
            &forged_sink_terminal,
        )
        .expect("forged sink residues");
        assert!(
            forged_sink_residues[forged_sink_residues.len() - P256_CROSS_TRACE_LANES_V1..]
                .iter()
                .any(|residue| *residue != F::ZERO)
        );
    }

    fn decode_projection_fixture() -> ZkX509SegmentedStarkProofV1 {
        decode_zk_x509_segmented_stark_proof_v1(
            &projection_fixture().2,
            &projection_aggregate_layout(),
        )
        .expect("decode projection fixture")
    }

    fn assert_projection_rejected(proof: &ZkX509SegmentedStarkProofV1) {
        match encode_zk_x509_segmented_stark_proof_v1(proof, &projection_aggregate_layout()) {
            Ok(bytes) => assert!(
                verify_zk_x509_projection_segmented_stark_v1(&projection_fixture().0, &bytes)
                    .is_err(),
                "adversarial projection proof must reject"
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch) => {}
            Err(error) => panic!("unexpected adversarial projection encode failure: {error}"),
        }
    }

    #[derive(Debug)]
    struct MaxValueRng;

    impl RngCore for MaxValueRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }

        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0xFF);
        }
    }

    #[test]
    fn canonical_multiproof_matches_small_trees_and_rejects_every_frontier_mutation() {
        let leaves = (0_u8..16)
            .map(|value| {
                sha256_frame_v1(b"aggregate-stark-multiproof-test-leaf-v1", &[&[value]])
                    .expect("leaf")
            })
            .collect::<Vec<_>>();
        let tree = Sha256MerkleTreeV1::from_leaves(
            leaves.clone(),
            b"aggregate-stark-multiproof-test-node-v1",
        )
        .expect("tree");
        for indices in [
            vec![0_usize],
            vec![1, 2, 7, 8, 15],
            (0_usize..16).collect::<Vec<_>>(),
        ] {
            let frontier = canonical_multiproof_frontier_v1(&tree, leaves.len(), &indices)
                .expect("canonical frontier");
            let opened = indices
                .iter()
                .copied()
                .map(|index| (index, leaves[index]))
                .collect::<BTreeMap<_, _>>();
            verify_canonical_multiproof_v1(
                b"aggregate-stark-multiproof-test-node-v1",
                &tree.root(),
                leaves.len(),
                &opened,
                &frontier,
            )
            .expect("valid multiproof");
            for position in 0..frontier.len() {
                let mut changed = frontier.clone();
                changed[position][0] ^= 1;
                assert!(
                    verify_canonical_multiproof_v1(
                        b"aggregate-stark-multiproof-test-node-v1",
                        &tree.root(),
                        leaves.len(),
                        &opened,
                        &changed,
                    )
                    .is_err(),
                    "frontier hash {position} must be bound"
                );
            }
            if indices.len() == leaves.len() {
                assert!(frontier.is_empty());
            }
        }
        assert!(canonical_multiproof_frontier_v1(&tree, leaves.len(), &[2, 2]).is_err());
        assert!(canonical_multiproof_frontier_v1(&tree, leaves.len(), &[2, 1]).is_err());
        assert!(canonical_multiproof_frontier_v1(&tree, leaves.len(), &[16]).is_err());
    }

    #[test]
    fn registered_degree_and_fri_capacity_boundaries_fail_closed() {
        let minimum_trace_size = 1_usize << MIN_TRACE_LOG2;
        let minimum_masked_degree = minimum_trace_size + MASK_DEGREE;
        let minimum_maximum_degree_quotient = usize::from(ZK_X509_MAX_CONSTRAINT_DEGREE_V1)
            * minimum_masked_degree
            - minimum_trace_size;
        let minimum_valid_lde_log2 = 16;
        let minimum_fri_rounds = minimum_valid_lde_log2 - TERMINAL_LOG2;
        let minimum_fri_degree = (TERMINAL_DEGREE_BOUND + 1) * (1_usize << minimum_fri_rounds) - 1;
        assert_eq!(
            checked_segment_degree_capacity_v1(
                MIN_TRACE_LOG2,
                minimum_valid_lde_log2,
                ZK_X509_MAX_CONSTRAINT_DEGREE_V1,
            )
            .expect("minimum secure maximum-degree domain"),
            (minimum_maximum_degree_quotient, minimum_fri_degree)
        );
        assert_eq!(minimum_maximum_degree_quotient, 5_703);
        assert_eq!(minimum_fri_degree, 2_047);
        assert!(
            checked_segment_degree_capacity_v1(
                MIN_TRACE_LOG2,
                minimum_valid_lde_log2 - 1,
                ZK_X509_MAX_CONSTRAINT_DEGREE_V1,
            )
            .is_err(),
            "the adjacent smaller LDE cannot hold the secure mask and quotient"
        );
        assert!(
            checked_segment_degree_capacity_v1(
                MIN_TRACE_LOG2,
                minimum_valid_lde_log2,
                ZK_X509_MAX_CONSTRAINT_DEGREE_V1 + 1,
            )
            .is_err()
        );
        assert!(
            checked_segment_degree_capacity_v1(MIN_TRACE_LOG2, minimum_valid_lde_log2, 1).is_err()
        );
        assert!(
            checked_segment_degree_capacity_v1(
                MIN_TRACE_LOG2 - 1,
                minimum_valid_lde_log2,
                ZK_X509_MAX_CONSTRAINT_DEGREE_V1,
            )
            .is_err()
        );
        assert!(
            checked_segment_degree_capacity_v1(
                MIN_TRACE_LOG2,
                TERMINAL_LOG2 - 1,
                ZK_X509_MAX_CONSTRAINT_DEGREE_V1,
            )
            .is_err()
        );
        assert!(checked_segment_degree_capacity_v1(u8::MAX, u8::MAX, 4).is_err());

        let focused_io = SegmentLayoutV1::for_io(1).expect("minimum focused I/O layout");
        assert_eq!(focused_io.trace_log2, IO_MIN_SECURE_TRACE_LOG2_V1);
        assert_eq!(
            focused_io.lde_log2,
            IO_MIN_SECURE_TRACE_LOG2_V1 + BLOWUP_LOG2
        );
        assert_eq!(
            checked_segment_degree_capacity_v1(
                focused_io.trace_log2,
                focused_io.lde_log2,
                focused_io.constraint_degree,
            )
            .expect("focused I/O degree capacity"),
            (6_276, 2_047)
        );
        focused_io.validate().expect("valid focused I/O layout");
        let mut stale_io = focused_io;
        stale_io.trace_log2 -= 1;
        stale_io.lde_log2 -= 1;
        assert!(
            stale_io.validate().is_err(),
            "the former log9/log15 focused-I/O layout cannot carry the release mask"
        );

        assert!(
            checked_segment_degree_capacity_v1(
                ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
                ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
                ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1,
            )
            .is_ok(),
            "the released log19 SHA registration requires the log25 MAIN domain"
        );
        assert!(
            checked_segment_degree_capacity_v1(
                ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
                22,
                ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1,
            )
            .is_err(),
            "the stale log22 SHA domain must fail closed"
        );
        assert!(
            checked_segment_degree_capacity_v1(
                ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
                ZK_X509_MAIN_COMMON_LDE_LOG2_V1 + 1,
                ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1,
            )
            .is_err(),
            "unregistered domains larger than MAIN must fail closed"
        );

        let trace_size = ZK_X509_DER_STARK_TRACE_SIZE_V1;
        let expected_quotient = usize::from(ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1)
            * (trace_size + MASK_DEGREE)
            - trace_size;
        let expected_chunk_capacity = 2 * trace_size - 1;
        assert_eq!(
            checked_segment_degree_capacity_v1(
                ZK_X509_DER_STARK_TRACE_LOG2_V1,
                ZK_X509_DER_STARK_TRACE_LOG2_V1 + BLOWUP_LOG2,
                ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1,
            )
            .expect("DER degree capacity"),
            (expected_quotient, expected_chunk_capacity)
        );
        assert!(
            expected_quotient > expected_chunk_capacity,
            "the exact DER quotient must exercise multiple FRI chunks"
        );
        assert!(
            expected_quotient <= (expected_chunk_capacity + 1) * COMPOSITION_DEGREE_CHUNKS - 1,
            "all four authenticated chunks must cover the exact DER quotient"
        );

        let mut invalid = der_layout();
        invalid.constraint_degree = ZK_X509_MAX_CONSTRAINT_DEGREE_V1 + 1;
        assert!(invalid.validate().is_err());
        invalid = der_layout();
        invalid.trace_log2 = u8::MAX;
        invalid.lde_log2 = u8::MAX;
        assert!(invalid.validate().is_err());
        invalid = der_layout();
        invalid.lde_log2 = TERMINAL_LOG2 - 1;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn interpolated_masked_degree_seven_quotient_attains_registered_bound() {
        const TRACE_LOG2: u8 = MIN_TRACE_LOG2;
        const TRACE_SIZE: usize = 1 << TRACE_LOG2;
        const TRACE_DEGREE: usize = TRACE_SIZE + MASK_DEGREE;
        const INTERPOLATION_LOG2: u8 = 13;
        let mut trace = vec![F::ZERO; TRACE_DEGREE + 1];
        trace[0] = F(7);
        trace[1] = F(11);
        trace[TRACE_DEGREE] = F::ONE;
        let multiply = |left: &[F], right: &[F]| {
            let mut product = vec![F::ZERO; left.len() + right.len() - 1];
            for (left_degree, left_coefficient) in left.iter().copied().enumerate() {
                for (right_degree, right_coefficient) in right.iter().copied().enumerate() {
                    product[left_degree + right_degree] = product[left_degree + right_degree]
                        .add(left_coefficient.mul(right_coefficient));
                }
            }
            product
        };
        let mut constraint = trace.clone();
        for _ in 1..usize::from(ZK_X509_MAX_CONSTRAINT_DEGREE_V1) {
            constraint = multiply(&constraint, &trace);
        }

        // Subtract the exact remainder modulo X^N - 1 so this synthetic
        // degree-seven constraint vanishes on the native subgroup.
        let mut numerator = constraint.clone();
        let mut remainder = vec![F::ZERO; TRACE_SIZE];
        for (degree, coefficient) in constraint.iter().copied().enumerate() {
            remainder[degree % TRACE_SIZE] = remainder[degree % TRACE_SIZE].add(coefficient);
        }
        for (coefficient, remainder) in numerator.iter_mut().zip(remainder) {
            *coefficient = coefficient.sub(remainder);
        }

        let mut quotient = vec![F::ZERO; numerator.len() - TRACE_SIZE];
        for degree in (TRACE_SIZE..numerator.len()).rev() {
            let coefficient = numerator[degree];
            quotient[degree - TRACE_SIZE] = coefficient;
            numerator[degree] = numerator[degree].sub(coefficient);
            numerator[degree - TRACE_SIZE] = numerator[degree - TRACE_SIZE].add(coefficient);
        }
        assert!(numerator.iter().all(|coefficient| *coefficient == F::ZERO));

        let expected_degree =
            usize::from(ZK_X509_MAX_CONSTRAINT_DEGREE_V1) * TRACE_DEGREE - TRACE_SIZE;
        assert_eq!(
            quotient
                .iter()
                .rposition(|coefficient| *coefficient != F::ZERO),
            Some(expected_degree)
        );
        quotient.resize(1 << INTERPOLATION_LOG2, F::ZERO);
        let interpolation_root =
            goldilocks_primitive_root_v1(INTERPOLATION_LOG2).expect("interpolation root");
        let mut evaluations = quotient.clone();
        crate::privacy_engines::transparent_stark::goldilocks_fft_v1(
            &mut evaluations,
            interpolation_root,
        )
        .expect("quotient evaluations");
        goldilocks_ifft_v1(&mut evaluations, interpolation_root).expect("quotient interpolation");
        let measured_degree = evaluations
            .iter()
            .rposition(|coefficient| *coefficient != F::ZERO)
            .expect("nonzero quotient");
        let (registered_quotient_degree, fri_input_degree) =
            checked_segment_degree_capacity_v1(TRACE_LOG2, 16, ZK_X509_MAX_CONSTRAINT_DEGREE_V1)
                .expect("degree capacity");
        assert_eq!(measured_degree, expected_degree);
        assert_eq!(measured_degree, registered_quotient_degree);
        assert!(
            measured_degree > fri_input_degree,
            "the exact quotient must exercise more than one composition chunk"
        );
        assert!(
            measured_degree <= (fri_input_degree + 1) * COMPOSITION_DEGREE_CHUNKS - 1,
            "the registered chunks must cover the exact quotient bound"
        );
    }

    #[test]
    fn every_segment_constructor_binds_the_correct_degree_capacity_profile() {
        let main_segments = [
            SegmentLayoutV1::for_io(1).expect("focused I/O"),
            SegmentLayoutV1::for_full_io().expect("full I/O"),
            SegmentLayoutV1::for_der(1).expect("strict DER"),
            SegmentLayoutV1::for_rfc5280(1).expect("RFC 5280"),
            SegmentLayoutV1::for_projection().expect("projection"),
            SegmentLayoutV1::for_sha_segment(0, ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0])
                .expect("SHA segment"),
        ];
        for segment in main_segments {
            segment.validate().expect("valid MAIN segment");
            assert!(
                checked_segment_degree_capacity_v1(
                    segment.trace_log2,
                    segment.main_capacity_lde_log2_v1(),
                    segment.constraint_degree,
                )
                .is_ok()
            );
        }

        for role in [
            P256EcdsaRoleV1::CertificateOrCrl,
            P256EcdsaRoleV1::WalletOwnership,
        ] {
            let segments = canonical_p256_segment_layouts_v1(role).expect("P-256 segments");
            assert!(
                segments
                    .iter()
                    .any(|segment| segment.trace_log2 == ZK_X509_MAX_NATIVE_TRACE_LOG2_V1),
                "each P-256 role must carry the log19 host that justifies log25 capacity"
            );
            for segment in segments {
                segment.validate().expect("valid P-256 segment");
                assert_eq!(
                    segment.main_capacity_lde_log2_v1(),
                    ZK_X509_MAIN_COMMON_LDE_LOG2_V1
                );
                assert!(
                    checked_segment_degree_capacity_v1(
                        segment.trace_log2,
                        segment.main_capacity_lde_log2_v1(),
                        segment.constraint_degree,
                    )
                    .is_ok()
                );
            }
        }

        let compact_ca = SegmentLayoutV1::for_ca_accumulator().expect("compact CA");
        compact_ca.validate().expect("valid compact-CA segment");
        assert_eq!(compact_ca.lde_log2, ZK_X509_CA_FRI_LDE_LOG2_V1);
        assert!(
            checked_compact_ca_degree_capacity_v1(
                compact_ca.trace_log2,
                compact_ca.lde_log2,
                compact_ca.constraint_degree,
            )
            .is_ok()
        );
        assert!(
            checked_segment_degree_capacity_v1(
                compact_ca.trace_log2,
                compact_ca.lde_log2,
                compact_ca.constraint_degree,
            )
            .is_err(),
            "the compact-CA constructor must never silently inherit MAIN parameters"
        );
    }

    #[test]
    fn verifier_owned_segment_registration_is_exact_and_full_profile_stays_closed() {
        let layout = fixture_aggregate_layout();
        layout.validate().expect("canonical registered layout");
        assert!(layout.validate_full_profile_registration().is_err());
        assert_eq!(layout.trace_groups.len(), 1);
        assert_eq!(layout.trace_groups[0].column_chunks, 1);
        assert_eq!(layout.trace_groups[0].base_width, IO_BASE_WIDTH);
        assert_eq!(layout.trace_groups[0].aux_width, IO_AUX_WIDTH);
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("byte-memory registration");
        assert_eq!(registration.trace_group, 0);
        assert_eq!(registration.base_start, 0);
        assert_eq!(registration.base_end().expect("base end"), IO_BASE_WIDTH);
        assert_eq!(registration.aux_start, 0);
        assert_eq!(registration.aux_end().expect("aux end"), IO_AUX_WIDTH);
        assert_eq!(registration.segment.fixed_width, IO_FIXED_WIDTH);
        assert_eq!(registration.segment.constraint_count, IO_CONSTRAINT_COUNT);
        assert_eq!(registration.segment.constraint_degree, IO_CONSTRAINT_DEGREE);

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments.clear();
        mutations.push(changed);
        changed = layout.clone();
        changed
            .registered_segments
            .push(changed.registered_segments[0]);
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::StrictDer;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.instance = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.active_rows = 0;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.fixed_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_count -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_degree -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].trace_group = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].base_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].aux_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation.validate().is_err(),
                "registration mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn main_verifier_profile_is_pinned_and_rejects_substitution() {
        let mut profile =
            construct_zk_x509_main_verifier_profile_v1().expect("release-pinned MAIN profile");
        profile.compiled_profile_digest[0] ^= 1;
        assert!(validate_zk_x509_main_verifier_profile_v1(profile).is_err());
    }

    #[test]
    fn der_registration_claim_order_and_every_shape_field_are_bound() {
        let layout = der_aggregate_layout();
        layout.validate().expect("canonical DER registration");
        assert!(layout.validate_full_profile_registration().is_err());
        assert_eq!(layout.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
        assert_eq!(layout.trace_groups.len(), 1);
        assert_eq!(
            layout.trace_groups[0].native_trace_log2,
            ZK_X509_DER_STARK_TRACE_LOG2_V1
        );
        assert_eq!(
            layout.trace_groups[0].column_chunks,
            ZK_X509_DER_STARK_BASE_WIDTH_V1
                .max(ZK_X509_DER_STARK_AUX_WIDTH_V1)
                .div_ceil(usize::from(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1))
        );
        assert_eq!(
            layout.trace_groups[0].base_width,
            ZK_X509_DER_STARK_BASE_WIDTH_V1
        );
        assert_eq!(
            layout.trace_groups[0].aux_width,
            ZK_X509_DER_STARK_AUX_WIDTH_V1
        );
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::StrictDer, 0)
            .expect("DER registration");
        assert_eq!(registration.trace_group, 0);
        assert_eq!(registration.base_start, 0);
        assert_eq!(registration.aux_start, 0);
        assert_eq!(
            registration.segment.active_rows,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
        );
        assert_eq!(
            registration.segment.trace_log2,
            ZK_X509_DER_STARK_TRACE_LOG2_V1
        );
        assert_eq!(
            registration.segment.lde_log2,
            ZK_X509_DER_STARK_TRACE_LOG2_V1 + BLOWUP_LOG2
        );
        assert_eq!(
            registration.segment.fixed_width,
            ZK_X509_DER_STARK_FIXED_WIDTH_V1
        );
        assert_eq!(
            registration.segment.constraint_count,
            ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1
        );
        assert_eq!(
            registration.segment.constraint_degree,
            ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1
        );

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments.clear();
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::Rfc5280;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.instance = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.active_rows = 0;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.trace_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.fixed_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_count -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_degree -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].trace_group = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].base_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].aux_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].native_trace_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation.validate().is_err(),
                "DER registration mutation {index} must fail closed"
            );
        }

        let sha = sha_word_layout();
        let ordered = AggregateProofLayoutV1::for_segments(&[der_layout(), sha])
            .expect("equal-log groups preserve adapter order");
        assert_eq!(ordered.trace_groups.len(), 2);
        assert_eq!(ordered.registered_segments[0].trace_group, 0);
        assert_eq!(ordered.registered_segments[1].trace_group, 1);
        assert_eq!(ordered.registered_segments[0].base_start, 0);
        assert_eq!(ordered.registered_segments[1].base_start, 0);
        assert!(AggregateProofLayoutV1::for_segments(&[sha, der_layout()]).is_err());
        assert!(AggregateProofLayoutV1::for_segments(&[der_layout(), der_layout()]).is_err());

        let claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(101), F(103), F(107), F(109)],
            node: [F(113), F(127), F(131), F(137)],
        };
        let roots = vec![TraceGroupProofV1 {
            base_root: [0x31; 32],
            aux_root: [0x53; 32],
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        }];
        let challenge_state = |derive_after_aux| {
            let mut transcript = new_transcript_v1(&[0x71; 32]).expect("transcript");
            absorb_aggregate_layout_v1(
                &mut transcript,
                b"iroha:privacy:zk-x509:der-aggregate-layout:test:v1",
                &layout,
            )
            .expect("layout");
            aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &roots)
                .expect("base roots");
            if derive_after_aux {
                aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &roots)
                    .expect("aux roots");
            }
            derive_der_challenges_v1(&mut transcript)
                .expect("DER challenges")
                .tuple[0][0]
        };
        assert_ne!(challenge_state(false), challenge_state(true));

        let transcript_state = |claims, claims_before_aux| {
            let mut transcript = new_transcript_v1(&[0x71; 32]).expect("transcript");
            absorb_aggregate_layout_v1(
                &mut transcript,
                b"iroha:privacy:zk-x509:der-aggregate-layout:test:v1",
                &layout,
            )
            .expect("layout");
            aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &roots)
                .expect("base roots");
            derive_der_challenges_v1(&mut transcript).expect("DER challenges");
            if claims_before_aux {
                absorb_der_terminal_claims_v1(&mut transcript, claims).expect("claims");
            }
            aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &roots)
                .expect("aux roots");
            if !claims_before_aux {
                absorb_der_terminal_claims_v1(&mut transcript, claims).expect("claims");
            }
            derive_constraint_alphas_v1(&mut transcript, &layout).expect("alphas")[0][0][0]
        };
        let canonical_alpha = transcript_state(claims, false);
        assert_ne!(canonical_alpha, transcript_state(claims, true));
        for index in 0..DER_PROOF_CLAIM_COUNT_V1 {
            let mut changed = claims;
            if index < ZK_X509_DER_STARK_BUS_LANES_V1 {
                changed.input_byte[index] = changed.input_byte[index].add(F::ONE);
            } else {
                changed.node[index - ZK_X509_DER_STARK_BUS_LANES_V1] =
                    changed.node[index - ZK_X509_DER_STARK_BUS_LANES_V1].add(F::ONE);
            }
            assert_ne!(
                canonical_alpha,
                transcript_state(changed, false),
                "DER claim field {index} must alter composition challenges"
            );
        }
        assert_ne!(
            canonical_alpha,
            transcript_state(
                ZkX509DerStarkTerminalClaimsV1 {
                    input_byte: claims.node,
                    node: claims.input_byte,
                },
                false
            )
        );
        let mut noncanonical = claims;
        noncanonical.node[0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        let mut transcript = new_transcript_v1(&[0x71; 32]).expect("transcript");
        assert!(absorb_der_terminal_claims_v1(&mut transcript, noncanonical).is_err());

        let aux = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
        let zero_claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1],
            node: [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1],
        };
        assert_eq!(
            evaluate_der_terminal_claim_opening_v1(F::ONE, &aux, zero_claims)
                .expect("claim opening"),
            [F::ZERO; 2 * ZK_X509_DER_STARK_BUS_LANES_V1]
        );
        let mut changed_claims = zero_claims;
        changed_claims.input_byte[2] = F::ONE;
        assert!(
            evaluate_der_terminal_claim_opening_v1(F::ONE, &aux, changed_claims)
                .expect("claim opening")
                .iter()
                .any(|residue| *residue != F::ZERO)
        );
    }

    #[test]
    fn x5m1_main_envelope_is_canonical_bounded_and_adversarially_strict() {
        let der = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(3), F(5), F(7), F(11)],
            node: [F(13), F(17), F(19), F(23)],
        };
        let mut sha = ZkX509ShaSegmentTerminalClaimsV1::canonical_zero_for_test_v1();
        for segment in &mut sha.segments {
            for stream in &mut segment.rfc_stream_products {
                stream.fill(F::ONE);
            }
        }
        let claims = ZkX509MainTerminalClaimsV1 {
            der,
            rfc5280: ZkX509Rfc5280StarkTerminalClaimsV1::canonical_for_der_test_v1(der)
                .expect("canonical DER/RFC test claims"),
            sha,
            p256: p256_main_terminal_claims_fixture_v1(),
        };
        let aggregate = b"X5S1aggregate";
        let encoded =
            encode_zk_x509_main_proof_envelope_v1(claims, aggregate).expect("canonical X5M1");
        assert_eq!(&encoded[..4], b"X5M1");
        assert_eq!(
            encoded.len(),
            ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1 + aggregate.len()
        );
        let decoded =
            decode_zk_x509_main_proof_envelope_v1(&encoded).expect("decode canonical X5M1");
        assert_eq!(decoded.claims, claims);
        assert_eq!(decoded.aggregate_proof, aggregate);
        assert_eq!(
            encode_zk_x509_main_proof_envelope_v1(decoded.claims, decoded.aggregate_proof)
                .expect("canonical re-encode"),
            encoded
        );
        assert!(
            ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1
                < usize::try_from(ZK_X509_MAX_PROOF_BYTES_V1).expect("global proof cap fits usize")
        );
        let exact_aggregate_len = ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1
            .checked_sub(ZK_X509_MAIN_PROOF_ENVELOPE_FIXED_BYTES_V1)
            .expect("MAIN partition accommodates its fixed envelope");
        let mut exact_aggregate = vec![0_u8; exact_aggregate_len];
        exact_aggregate[..PROOF_MAGIC_V1.len()].copy_from_slice(&PROOF_MAGIC_V1);
        let exact_cap = encode_zk_x509_main_proof_envelope_v1(claims, &exact_aggregate)
            .expect("exact MAIN aggregate partition boundary");
        assert_eq!(exact_cap.len(), ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1);
        assert!(
            decode_zk_x509_main_proof_envelope_v1(&exact_cap).is_ok(),
            "decoder must accept the exact partition boundary"
        );
        drop(exact_cap);
        exact_aggregate.push(0);
        assert!(matches!(
            encode_zk_x509_main_proof_envelope_v1(claims, &exact_aggregate),
            Err(ZkX509StarkErrorV1::ProofTooLarge)
        ));
        let oversized_wire = vec![0_u8; ZK_X509_MAIN_AGGREGATE_MAX_PROOF_BYTES_V1 + 1];
        assert!(matches!(
            decode_zk_x509_main_proof_envelope_v1(&oversized_wire),
            Err(ZkX509StarkErrorV1::ProofTooLarge)
        ));

        let mut internally_unequal = claims;
        internally_unequal.p256.certificate_or_crl[0]
            .buses
            .value_execution[0] = F::ONE;
        assert!(
            internally_unequal.p256.encode_x5v1_v1().is_ok(),
            "the standalone canonical codec intentionally checks shape, not AIR equality"
        );
        assert!(matches!(
            encode_zk_x509_main_proof_envelope_v1(internally_unequal, aggregate),
            Err(ZkX509StarkErrorV1::InternalInvariant)
        ));
        let mut transcript =
            new_transcript_v1(&[0x92; 32]).expect("MAIN terminal transcript fixture");
        let transcript_before = transcript;
        assert!(matches!(
            absorb_zk_x509_main_terminal_claims_v1(&mut transcript, internally_unequal),
            Err(ZkX509StarkErrorV1::InvalidStatement)
        ));
        assert_eq!(
            transcript, transcript_before,
            "internally unequal X5V1 claims must fail before transcript mutation"
        );
        let mut unequal_encoded = encoded.clone();
        overwrite_main_terminal_record_value_v1(
            &mut unequal_encoded,
            MAIN_PROOF_P256_OFFSET_V1,
            0,
            F::ONE,
        );
        assert!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(
                &unequal_encoded[MAIN_PROOF_P256_OFFSET_V1
                    ..MAIN_PROOF_P256_OFFSET_V1 + ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1],
            )
            .is_ok(),
            "the adversary must remain canonically encoded"
        );
        assert!(matches!(
            decode_zk_x509_main_proof_envelope_v1(&unequal_encoded),
            Err(ZkX509StarkErrorV1::MalformedProof)
        ));

        let terminal_challenge = |claims| {
            let mut transcript = new_transcript_v1(&[0x91; 32]).expect("MAIN transcript");
            absorb_zk_x509_main_terminal_claims_v1(&mut transcript, claims)
                .expect("terminal frame");
            transcript
                .challenge_fp4(b"main-terminal-test-alpha-v1")
                .expect("post-terminal challenge")
        };
        let canonical_terminal_challenge = terminal_challenge(claims);
        for offset in [MAIN_PROOF_SHA_OFFSET_V1 + 27] {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            let changed_claims = decode_zk_x509_main_proof_envelope_v1(&changed)
                .expect("canonical changed claim")
                .claims;
            assert_ne!(
                terminal_challenge(changed_claims),
                canonical_terminal_challenge,
                "claim value at {offset} was not transcript-bound"
            );
        }
        let mut changed_p256_claims = claims;
        changed_p256_claims.p256.certificate_or_crl[0]
            .buses
            .value_execution[0] = F::ONE;
        changed_p256_claims.p256.certificate_or_crl[0]
            .buses
            .value_sorted[0] = F::ONE;
        let changed_p256 = encode_zk_x509_main_proof_envelope_v1(changed_p256_claims, aggregate)
            .expect("internally equal changed P-256 claim");
        let changed_p256_claims = decode_zk_x509_main_proof_envelope_v1(&changed_p256)
            .expect("canonical changed P-256 claim")
            .claims;
        assert_ne!(
            terminal_challenge(changed_p256_claims),
            canonical_terminal_challenge,
            "internally equal P-256 terminal mutation was not transcript-bound"
        );
        for offset in [
            MAIN_PROOF_HEADER_BYTES_V1 + 7,
            MAIN_PROOF_RFC_OFFSET_V1 + 23,
        ] {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_zk_x509_main_proof_envelope_v1(&changed).is_err(),
                "DER/RFC equality mutation at {offset} must fail before MAIN alphas"
            );
        }

        // X5R1 encodes four aggregate relation products first. Keep its
        // aggregate consumer relation consistent while corrupting each of the
        // four RFC roles consumed by SHA. The nested frame must remain valid;
        // only the cross-adapter MAIN equality may reject it.
        const RFC_AGGREGATE_RECORDS_V1: usize = 4 * ZK_X509_DER_STARK_BUS_LANES_V1;
        const RFC_AGGREGATE_CONSUMER_RELATION_V1: usize = 3;
        const RFC_OUTPUT_ENDPOINTS_V1: usize = 2;
        const RFC_CONSUMER_ENDPOINT_V1: usize = 1;
        for role in MAIN_RFC_SHA_CONSUMER_ROLES_V1 {
            let role_index = role as usize - 1;
            for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
                let mut changed = encoded.clone();
                let aggregate_record =
                    RFC_AGGREGATE_CONSUMER_RELATION_V1 * ZK_X509_DER_STARK_BUS_LANES_V1 + lane;
                let role_consumer_record = RFC_AGGREGATE_RECORDS_V1
                    + role_index * RFC_OUTPUT_ENDPOINTS_V1 * ZK_X509_DER_STARK_BUS_LANES_V1
                    + RFC_CONSUMER_ENDPOINT_V1 * ZK_X509_DER_STARK_BUS_LANES_V1
                    + lane;
                overwrite_main_terminal_record_value_v1(
                    &mut changed,
                    MAIN_PROOF_RFC_OFFSET_V1,
                    aggregate_record,
                    F(2),
                );
                overwrite_main_terminal_record_value_v1(
                    &mut changed,
                    MAIN_PROOF_RFC_OFFSET_V1,
                    role_consumer_record,
                    F(2),
                );
                let rfc_end = MAIN_PROOF_RFC_OFFSET_V1 + ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1;
                let changed_rfc = ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(
                    &changed[MAIN_PROOF_RFC_OFFSET_V1..rfc_end],
                )
                .expect("internally consistent changed X5R1");
                let mut changed_claims = claims;
                changed_claims.rfc5280 = changed_rfc;
                assert!(
                    matches!(
                        encode_zk_x509_main_proof_envelope_v1(changed_claims, aggregate),
                        Err(ZkX509StarkErrorV1::InternalInvariant)
                    ),
                    "RFC role {role:?} lane {lane} escaped MAIN encoding"
                );
                let mut transcript =
                    new_transcript_v1(&[0x91; 32]).expect("MAIN transcript fixture");
                let transcript_before = transcript;
                assert!(
                    matches!(
                        absorb_zk_x509_main_terminal_claims_v1(&mut transcript, changed_claims,),
                        Err(ZkX509StarkErrorV1::InvalidStatement)
                    ),
                    "RFC role {role:?} lane {lane} escaped pre-alpha validation"
                );
                assert_eq!(
                    transcript, transcript_before,
                    "failed RFC/SHA equality must not mutate the transcript"
                );
                assert!(
                    matches!(
                        decode_zk_x509_main_proof_envelope_v1(&changed),
                        Err(ZkX509StarkErrorV1::MalformedProof)
                    ),
                    "RFC role {role:?} lane {lane} escaped MAIN decoding"
                );
            }
        }

        // X5Q1 orders two call-bus products followed by four RFC streams for
        // each of four physical segments. Exercise every one of those 64
        // stream terminals while preserving a canonical standalone X5Q1.
        let sha_streams = claims.sha.segments[0].rfc_stream_products.len();
        let sha_lanes = claims.sha.segments[0].rfc_stream_products[0].len();
        let sha_records_per_segment = (2 + sha_streams) * sha_lanes;
        for segment in 0..claims.sha.segments.len() {
            for stream in 0..sha_streams {
                for lane in 0..sha_lanes {
                    let mut changed = encoded.clone();
                    let record =
                        segment * sha_records_per_segment + (2 + stream) * sha_lanes + lane;
                    overwrite_main_terminal_record_value_v1(
                        &mut changed,
                        MAIN_PROOF_SHA_OFFSET_V1,
                        record,
                        F(2),
                    );
                    let sha_end =
                        MAIN_PROOF_SHA_OFFSET_V1 + ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1;
                    let changed_sha = ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(
                        &changed[MAIN_PROOF_SHA_OFFSET_V1..sha_end],
                    )
                    .expect("canonical changed X5Q1");
                    let mut changed_claims = claims;
                    changed_claims.sha = changed_sha;
                    assert!(
                        matches!(
                            encode_zk_x509_main_proof_envelope_v1(changed_claims, aggregate),
                            Err(ZkX509StarkErrorV1::InternalInvariant)
                        ),
                        "SHA segment {segment} stream {stream} lane {lane} escaped MAIN encoding"
                    );
                    let mut transcript =
                        new_transcript_v1(&[0x91; 32]).expect("MAIN transcript fixture");
                    let transcript_before = transcript;
                    assert!(
                        matches!(
                            absorb_zk_x509_main_terminal_claims_v1(&mut transcript, changed_claims,),
                            Err(ZkX509StarkErrorV1::InvalidStatement)
                        ),
                        "SHA segment {segment} stream {stream} lane {lane} escaped pre-alpha validation"
                    );
                    assert_eq!(
                        transcript, transcript_before,
                        "failed RFC/SHA equality must not mutate the transcript"
                    );
                    assert!(
                        matches!(
                            decode_zk_x509_main_proof_envelope_v1(&changed),
                            Err(ZkX509StarkErrorV1::MalformedProof)
                        ),
                        "SHA segment {segment} stream {stream} lane {lane} escaped MAIN decoding"
                    );
                }
            }
        }

        // Omitting a complete stream contribution by replacing its four
        // terminals with zero remains a canonical X5Q1 frame, but cannot
        // satisfy the RFC union equality.
        let mut omitted_stream = encoded.clone();
        for lane in 0..sha_lanes {
            overwrite_main_terminal_record_value_v1(
                &mut omitted_stream,
                MAIN_PROOF_SHA_OFFSET_V1,
                2 * sha_lanes + lane,
                F::ZERO,
            );
        }
        let sha_end = MAIN_PROOF_SHA_OFFSET_V1 + ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1;
        ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(
            &omitted_stream[MAIN_PROOF_SHA_OFFSET_V1..sha_end],
        )
        .expect("zeroed stream is a canonical standalone X5Q1");
        assert!(matches!(
            decode_zk_x509_main_proof_envelope_v1(&omitted_stream),
            Err(ZkX509StarkErrorV1::MalformedProof)
        ));

        // Segment records are address-ordered. Moving a complete segment
        // block, including its addresses, must fail the nested canonical
        // decoder before MAIN can consider product equality.
        let mut swapped_segments = encoded.clone();
        let sha_segment_record_bytes = sha_records_per_segment * TERMINAL_TEST_RECORD_BYTES_V1;
        let sha_records_start = MAIN_PROOF_SHA_OFFSET_V1 + TERMINAL_TEST_HEADER_BYTES_V1;
        for offset in 0..sha_segment_record_bytes {
            swapped_segments.swap(
                sha_records_start + offset,
                sha_records_start + sha_segment_record_bytes + offset,
            );
        }
        assert!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(
                &swapped_segments[MAIN_PROOF_SHA_OFFSET_V1..sha_end],
            )
            .is_err()
        );
        assert!(decode_zk_x509_main_proof_envelope_v1(&swapped_segments).is_err());

        let mut noncanonical_sha = encoded.clone();
        overwrite_main_terminal_record_value_v1(
            &mut noncanonical_sha,
            MAIN_PROOF_SHA_OFFSET_V1,
            2 * sha_lanes,
            F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1),
        );
        assert!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(
                &noncanonical_sha[MAIN_PROOF_SHA_OFFSET_V1..sha_end],
            )
            .is_err()
        );
        assert!(decode_zk_x509_main_proof_envelope_v1(&noncanonical_sha).is_err());

        for prefix_len in 0..encoded.len() {
            assert!(
                decode_zk_x509_main_proof_envelope_v1(&encoded[..prefix_len]).is_err(),
                "truncated prefix {prefix_len} accepted"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(
            decode_zk_x509_main_proof_envelope_v1(&trailing).is_err(),
            "trailing byte accepted"
        );
        for offset in 0..MAIN_PROOF_HEADER_BYTES_V1 {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_zk_x509_main_proof_envelope_v1(&changed).is_err(),
                "header byte {offset} accepted"
            );
        }
        for offset in [
            MAIN_PROOF_RFC_OFFSET_V1,
            MAIN_PROOF_SHA_OFFSET_V1,
            MAIN_PROOF_P256_OFFSET_V1,
        ] {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_zk_x509_main_proof_envelope_v1(&changed).is_err(),
                "nested terminal-frame identity at {offset} accepted"
            );
        }
        let mut noncanonical = encoded.clone();
        noncanonical[MAIN_PROOF_HEADER_BYTES_V1..MAIN_PROOF_HEADER_BYTES_V1 + 8].copy_from_slice(
            &crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1.to_be_bytes(),
        );
        assert!(matches!(
            decode_zk_x509_main_proof_envelope_v1(&noncanonical),
            Err(ZkX509StarkErrorV1::NonCanonicalField)
        ));
        for offset in
            MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1..MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1 + 4
        {
            let mut changed = encoded.clone();
            changed[offset] ^= 0x80;
            assert!(
                decode_zk_x509_main_proof_envelope_v1(&changed).is_err(),
                "aggregate length byte {offset} accepted"
            );
        }
        let aggregate_start = MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1 + 4;
        let mut changed_aggregate_magic = encoded.clone();
        changed_aggregate_magic[aggregate_start] ^= 1;
        assert!(decode_zk_x509_main_proof_envelope_v1(&changed_aggregate_magic).is_err());

        let mut legacy_sidecar_wire = encoded[..MAIN_PROOF_AGGREGATE_LENGTH_OFFSET_V1].to_vec();
        append_u32_v1(&mut legacy_sidecar_wire, 4);
        legacy_sidecar_wire.extend_from_slice(b"X5F1");
        append_u32_v1(
            &mut legacy_sidecar_wire,
            u32::try_from(aggregate.len()).expect("small aggregate"),
        );
        legacy_sidecar_wire.extend_from_slice(aggregate);
        assert!(
            decode_zk_x509_main_proof_envelope_v1(&legacy_sidecar_wire).is_err(),
            "legacy X5F1 sidecar wire accepted"
        );
        assert!(encode_zk_x509_main_proof_envelope_v1(claims, b"X5F1").is_err());
    }

    #[test]
    fn der_statement_digest_and_x5p1_envelope_are_exact_and_fail_closed() {
        let shape = ZkX509DerStarkShapeV1;
        let digest = der_public_digest_v1(&shape).expect("DER public digest");
        assert_eq!(
            hex::encode(digest),
            "b4837637f1bf0678fa78729a4fb2d9ae62da60c7768cf5bdf061abfe96a7443d"
        );

        let claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(3), F(5), F(7), F(11)],
            node: [F(13), F(17), F(19), F(23)],
        };
        let aggregate = b"X5S1payload";
        let encoded =
            encode_der_segmented_proof_envelope_v1(claims, aggregate).expect("X5P1 envelope");
        assert_eq!(encoded.len(), DER_PROOF_ENVELOPE_BYTES_V1 + aggregate.len());
        assert_eq!(&encoded[..4], b"X5P1");
        assert_eq!(
            &encoded[DER_PROOF_ENVELOPE_BYTES_V1..],
            aggregate.as_slice()
        );
        let (decoded_claims, decoded_aggregate) =
            decode_der_segmented_proof_envelope_v1(&encoded).expect("decode X5P1");
        assert_eq!(decoded_claims, claims);
        assert_eq!(decoded_aggregate, aggregate);
        assert_eq!(
            encode_der_segmented_proof_envelope_v1(decoded_claims, decoded_aggregate)
                .expect("canonical re-encode"),
            encoded
        );

        for prefix_len in 0..encoded.len() {
            assert!(
                decode_der_segmented_proof_envelope_v1(&encoded[..prefix_len]).is_err(),
                "truncated prefix {prefix_len} must reject"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_der_segmented_proof_envelope_v1(&trailing).is_err());
        for offset in 0..12 {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_der_segmented_proof_envelope_v1(&changed).is_err(),
                "header byte {offset} must be exact"
            );
        }
        for claim_index in 0..DER_PROOF_CLAIM_COUNT_V1 {
            let start = 12 + claim_index * DER_PROOF_CLAIM_RECORD_BYTES_V1;
            for offset in start..start + 4 {
                let mut changed = encoded.clone();
                changed[offset] ^= 1;
                assert!(
                    decode_der_segmented_proof_envelope_v1(&changed).is_err(),
                    "claim type/lane byte {offset} must be exact"
                );
            }
        }
        let mut reordered = encoded.clone();
        let first: [u8; DER_PROOF_CLAIM_RECORD_BYTES_V1] = reordered
            [12..12 + DER_PROOF_CLAIM_RECORD_BYTES_V1]
            .try_into()
            .expect("first claim");
        let second: [u8; DER_PROOF_CLAIM_RECORD_BYTES_V1] = reordered
            [12 + DER_PROOF_CLAIM_RECORD_BYTES_V1..12 + 2 * DER_PROOF_CLAIM_RECORD_BYTES_V1]
            .try_into()
            .expect("second claim");
        reordered[12..12 + DER_PROOF_CLAIM_RECORD_BYTES_V1].copy_from_slice(&second);
        reordered[12 + DER_PROOF_CLAIM_RECORD_BYTES_V1..12 + 2 * DER_PROOF_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first);
        assert!(decode_der_segmented_proof_envelope_v1(&reordered).is_err());
        let mut duplicated = encoded.clone();
        duplicated[12 + DER_PROOF_CLAIM_RECORD_BYTES_V1..12 + 2 * DER_PROOF_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first);
        assert!(decode_der_segmented_proof_envelope_v1(&duplicated).is_err());

        for offset in DER_PROOF_LENGTH_OFFSET_V1..DER_PROOF_ENVELOPE_BYTES_V1 {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                decode_der_segmented_proof_envelope_v1(&changed).is_err(),
                "length byte {offset} must be exact"
            );
        }
        let mut noncanonical = encoded.clone();
        noncanonical[16..24].copy_from_slice(
            &crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1.to_be_bytes(),
        );
        assert!(matches!(
            decode_der_segmented_proof_envelope_v1(&noncanonical),
            Err(ZkX509StarkErrorV1::NonCanonicalField)
        ));
        assert!(encode_der_segmented_proof_envelope_v1(claims, &[]).is_err());
        let mut rejected_draft_magic = encoded.clone();
        rejected_draft_magic[..4].copy_from_slice(b"X5D2");
        assert!(decode_der_segmented_proof_envelope_v1(&rejected_draft_magic).is_err());

        // A valid envelope cannot make an invalid inner aggregate proof
        // verifier-visible as a different wire format.
        assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &encoded).is_err());
    }

    #[test]
    fn der_sampled_fixed_rows_match_reduced_full_lde_for_every_column_family() {
        let _guard = proof_guard();
        fn native_row(
            document_count: usize,
            parser_rows: usize,
            active_rows: usize,
            index: usize,
            trace_size: usize,
        ) -> [F; ZK_X509_DER_STARK_FIXED_WIDTH_V1] {
            assert!(index < trace_size);
            let mut row = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
            row[FIX_FINAL_DOCUMENT] = F(u64::try_from(document_count - 1).expect("document"));
            row[FIX_FIRST_AGGREGATE] = F(u64::from(index == 0));
            row[FIX_LAST_AGGREGATE] = F(u64::from(index + 1 == trace_size));
            if index >= active_rows {
                row[FIX_PADDING] = F::ONE;
                return row;
            }
            row[DER_FIX_ACTIVE] = F::ONE;
            row[FIX_FIRST_ACTIVE] = F(u64::from(index == 0));
            row[DER_FIX_LAST_ACTIVE] = F(u64::from(index + 1 == active_rows));
            if index < parser_rows {
                row[FIX_PARSER] = F::ONE;
                row[FIX_FIRST_PARSER] = F(u64::from(index == 0));
                row[FIX_LAST_PARSER] = F(u64::from(index + 1 == parser_rows));
                row[FIX_PARSER_CONTINUE] = F(u64::from(index + 1 < parser_rows));
            } else {
                row[FIX_COMPARATOR] = F::ONE;
                row[FIX_FIRST_COMPARATOR] = F(u64::from(index == parser_rows));
                row[FIX_LAST_COMPARATOR] = F(u64::from(index + 1 == active_rows));
            }
            row
        }

        let trace_log2 = 5;
        let lde_log2 = trace_log2 + BLOWUP_LOG2;
        let trace_size = 1_usize << trace_log2;
        let lde_size = 1_usize << lde_log2;
        let lde_root = goldilocks_primitive_root_v1(lde_log2).expect("LDE root");
        for private_shape in [
            ZkX509DerStarkPrivateShapeV1 {
                document_lengths: vec![5],
                parser_rows: 7,
                comparator_rows: 4,
            },
            ZkX509DerStarkPrivateShapeV1 {
                document_lengths: vec![5],
                parser_rows: 7,
                comparator_rows: 0,
            },
            ZkX509DerStarkPrivateShapeV1 {
                document_lengths: vec![5, 17],
                parser_rows: 26,
                comparator_rows: 6,
            },
        ] {
            private_shape.validate().expect("private shape");
            let document_count = private_shape.document_lengths.len();
            let parser_rows = private_shape.parser_rows;
            let active_rows = private_shape.active_rows().expect("active rows");
            let columns = (0..ZK_X509_DER_STARK_FIXED_WIDTH_V1)
                .map(|column| {
                    (0..trace_size)
                        .map(|index| {
                            native_row(document_count, parser_rows, active_rows, index, trace_size)
                                [column]
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut layout = der_layout();
            layout.active_rows = active_rows;
            layout.trace_log2 = trace_log2;
            layout.lde_log2 = lde_log2;
            let full_lde = fixed_lde_columns_v1(&columns, layout).expect("full fixed LDE");
            for index in 0..lde_size {
                let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
                let sampled = der_fixed_row_at_point_for_shape_v1(
                    document_count,
                    parser_rows,
                    active_rows,
                    trace_log2,
                    x,
                )
                .expect("sampled fixed row");
                for column in 0..ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
                    assert_eq!(
                        sampled[column], full_lde[column][index],
                        "shape={private_shape:?}, column={column}, LDE row={index}"
                    );
                }
            }
            assert_eq!(
                checked_der_fixed_sampled_work_v1(active_rows, trace_size, QUERY_COUNT)
                    .expect("sampled work"),
                (active_rows + usize::from(active_rows < trace_size)) * QUERY_COUNT
            );
        }
        for (document_count, parser_rows, active_rows) in
            [(1_usize, 1_usize, 1_usize), (1, 1, 5), (2, 31, 32)]
        {
            let columns = (0..ZK_X509_DER_STARK_FIXED_WIDTH_V1)
                .map(|column| {
                    (0..trace_size)
                        .map(|index| {
                            native_row(document_count, parser_rows, active_rows, index, trace_size)
                                [column]
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut layout = der_layout();
            layout.active_rows = active_rows;
            layout.trace_log2 = trace_log2;
            layout.lde_log2 = lde_log2;
            let full_lde = fixed_lde_columns_v1(&columns, layout).expect("edge full fixed LDE");
            for index in 0..lde_size {
                let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
                let sampled = der_fixed_row_at_point_for_shape_v1(
                    document_count,
                    parser_rows,
                    active_rows,
                    trace_log2,
                    x,
                )
                .expect("edge sampled fixed row");
                for column in 0..ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
                    assert_eq!(
                        sampled[column], full_lde[column][index],
                        "documents={document_count}, parser={parser_rows}, active={active_rows}, \
                         column={column}, LDE row={index}"
                    );
                }
            }
        }
        assert!(checked_der_fixed_sampled_work_v1(0, trace_size, 1).is_err());
        assert!(checked_der_fixed_sampled_work_v1(trace_size + 1, trace_size, 1).is_err());
        assert!(checked_der_fixed_sampled_work_v1(
            1,
            trace_size,
            DER_FIXED_MAX_SAMPLED_OPENINGS_V1 + 1,
        )
        .is_err());
        assert!(
            checked_der_fixed_sampled_work_v1(
                usize::MAX,
                usize::MAX,
                DER_FIXED_MAX_SAMPLED_OPENINGS_V1,
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_der_wires_reject_before_sampled_fixed_evaluation() {
        let _guard = proof_guard();
        DER_FIXED_OPENING_EVALUATIONS_V1.store(0, std::sync::atomic::Ordering::SeqCst);
        let document = [0x30, 0x03, 0x02, 0x01, 0x01];
        let private_shape = build_zk_x509_der_stark_base_v1(&[&document])
            .expect("DER base")
            .private_shape;
        private_shape.validate().expect("private DER shape");
        let shape = ZkX509DerStarkShapeV1;
        let claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(3), F(5), F(7), F(11)],
            node: [F(13), F(17), F(19), F(23)],
        };
        let malformed_inner =
            encode_der_segmented_proof_envelope_v1(claims, b"X5S1malformed-inner")
                .expect("outer envelope");
        for malformed in [
            Vec::new(),
            b"X5D2draft".to_vec(),
            malformed_inner[..malformed_inner.len() - 1].to_vec(),
            malformed_inner,
        ] {
            assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &malformed).is_err());
        }
        assert_eq!(
            DER_FIXED_OPENING_EVALUATIONS_V1.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "malformed proof parsing, shape, and commitment failures must not enter fixed evaluation"
        );
    }

    #[test]
    fn der_opened_row_evaluator_binds_both_rows_fixed_schedule_claims_and_lane_mixes() {
        let layout = der_layout();
        let aggregate_layout = der_aggregate_layout();
        let query_index = 123_usize;
        let next_index = query_index + BLOWUP;
        let challenges = ZkX509DerStarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|slot| {
                    F(u64::try_from(100 + lane * 20 + slot).expect("challenge"))
                })
            }),
            byte_lookup: [F(701), F(709), F(719), F(727)],
        };
        let public = ZkX509DerStarkPublicTerminalsV1;
        let claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(743), F(751), F(757), F(761)],
            node: [F(769), F(773), F(787), F(797)],
        };
        let trace_groups = vec![aggregate::AggregateOpenedTraceGroupV1 {
            base_current: (0..ZK_X509_DER_STARK_BASE_WIDTH_V1)
                .map(|index| F(u64::try_from(index + 1).expect("base")))
                .collect(),
            base_next: (0..ZK_X509_DER_STARK_BASE_WIDTH_V1)
                .map(|index| F(u64::try_from(index + 211).expect("next base")))
                .collect(),
            aux_current: (0..ZK_X509_DER_STARK_AUX_WIDTH_V1)
                .map(|index| F(u64::try_from(index + 419).expect("aux")))
                .collect(),
            aux_next: (0..ZK_X509_DER_STARK_AUX_WIDTH_V1)
                .map(|index| F(u64::try_from(index + 911).expect("next aux")))
                .collect(),
        }];
        let fixed_current =
            core::array::from_fn(|index| F(u64::try_from(index + 1_301).expect("fixed")));
        let fixed_next =
            core::array::from_fn(|index| F(u64::try_from(index + 1_401).expect("next fixed")));
        let fixed_openings =
            BTreeMap::from([(query_index, fixed_current), (next_index, fixed_next)]);
        let alphas = (0..SECURITY_LANES)
            .map(|lane| {
                (0..ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1)
                    .map(|index| {
                        extension_v1(F(
                            u64::try_from(2_003 + lane * 1_000 + index).expect("alpha")
                        ))
                    })
                    .collect()
            })
            .collect::<Vec<_>>();
        let mixes = (0..SECURITY_LANES)
            .map(|lane| FriMixV1 {
                base: (0..ZK_X509_DER_STARK_BASE_WIDTH_V1)
                    .map(|index| {
                        extension_v1(F(
                            u64::try_from(5_003 + lane * 500 + index).expect("base mix")
                        ))
                    })
                    .collect(),
                base_next: (0..ZK_X509_DER_STARK_BASE_WIDTH_V1)
                    .map(|index| {
                        extension_v1(F(
                            u64::try_from(6_003 + lane * 500 + index).expect("next base mix")
                        ))
                    })
                    .collect(),
                aux: (0..ZK_X509_DER_STARK_AUX_WIDTH_V1)
                    .map(|index| {
                        extension_v1(F(
                            u64::try_from(7_003 + lane * 500 + index).expect("aux mix")
                        ))
                    })
                    .collect(),
                aux_next: (0..ZK_X509_DER_STARK_AUX_WIDTH_V1)
                    .map(|index| {
                        extension_v1(F(
                            u64::try_from(8_003 + lane * 500 + index).expect("next aux mix")
                        ))
                    })
                    .collect(),
                composition: (0..COMPOSITION_DEGREE_CHUNKS)
                    .map(|chunk| {
                        extension_v1(F(
                            u64::try_from(9_001 + lane * 10 + chunk).expect("composition mix")
                        ))
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let lde_root = goldilocks_primitive_root_v1(layout.lde_log2).expect("LDE root");
        let mut evaluator = DerOpenedRowEvaluatorV1 {
            aggregate_layout: &aggregate_layout,
            layout,
            fixed_openings: &fixed_openings,
            challenges,
            public,
            claims,
            alphas: &alphas,
            mixes: &mixes,
            lde_root,
        };
        for lane in 0..SECURITY_LANES {
            let composition_chunks = (0..COMPOSITION_DEGREE_CHUNKS)
                .map(|chunk| {
                    extension_v1(F(u64::try_from(11_001 + lane * 10 + chunk).expect("chunk")))
                })
                .collect::<Vec<_>>();
            let evaluated = aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks,
            )
            .expect("opened DER row");
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(query_index as u128));
            let composition = der_quotient_value_v1(
                layout,
                x,
                &trace_groups[0].base_current,
                &trace_groups[0].base_next,
                &trace_groups[0].aux_current,
                &trace_groups[0].aux_next,
                &fixed_current,
                &fixed_next,
                challenges,
                public,
                claims,
                &alphas[lane],
            )
            .expect("direct quotient");
            let mixed_base = trace_groups[0]
                .base_current
                .iter()
                .zip(&mixes[lane].base)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                });
            let mixed_aux = trace_groups[0]
                .aux_current
                .iter()
                .zip(&mixes[lane].aux)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                });
            assert_eq!(evaluated.composition, composition);
            assert_eq!(
                evaluated.fri_base,
                mixed_base.add(mixed_aux).add(
                    mix_opened_composition_chunks_v1(&composition_chunks, &mixes[lane])
                        .expect("composition mix"),
                )
            );
        }

        let missing_next = BTreeMap::from([(query_index, fixed_current)]);
        evaluator.fixed_openings = &missing_next;
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                0,
                &trace_groups,
                &[E::ZERO; COMPOSITION_DEGREE_CHUNKS],
            )
            .is_err()
        );
        evaluator.fixed_openings = &fixed_openings;
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                SECURITY_LANES,
                &trace_groups,
                &[E::ZERO; COMPOSITION_DEGREE_CHUNKS],
            )
            .is_err()
        );
        let mut wrong_width = trace_groups.clone();
        wrong_width[0].aux_next.pop();
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                0,
                &wrong_width,
                &[E::ZERO; COMPOSITION_DEGREE_CHUNKS],
            )
            .is_err()
        );
    }

    #[test]
    fn der_retained_prover_resource_plan_and_production_source_exclude_trace_scratch() {
        let plan = der_retained_prover_allocation_plan_v1(der_layout()).expect("retained DER plan");
        assert_eq!(plan.quotient_coset_log2, 22);
        assert_eq!(plan.quotient_coset_rows, 1 << 22);
        assert_eq!(plan.quotient_next_stride, 8);
        assert_eq!(plan.maximum_quotient_degree, 3_151_335);
        assert_eq!(plan.retained_masked_coefficient_bytes, 1_142_595_840);
        assert_eq!(plan.quotient_trace_matrix_bytes, 9_596_567_552);
        assert_eq!(plan.encrypted_trace_scratch_bytes, 0);
        assert_eq!(plan.common_domain_trace_matrix_bytes, 0);

        let mut changed = der_layout();
        changed.lde_log2 -= 1;
        assert!(der_retained_prover_allocation_plan_v1(changed).is_err());
        changed = der_layout();
        changed.constraint_degree -= 1;
        assert!(der_retained_prover_allocation_plan_v1(changed).is_err());
        changed = der_layout();
        changed.trace_log2 -= 1;
        assert!(der_retained_prover_allocation_plan_v1(changed).is_err());

        let source = include_str!("stark.rs");
        let start = source
            .find("fn build_zk_x509_der_segmented_stark_proof_v1_with_rng")
            .expect("DER builder source");
        let end = source[start..]
            .find("/// Construct and self-verify the canonical strict-DER aggregate proof.")
            .map(|offset| start + offset)
            .expect("DER builder end");
        let builder = &source[start..end];
        for forbidden in [
            "EncryptedFieldMatrixScratchV1",
            "EncryptedFieldMatrixScratchWriterV1",
            "commit_masked_trace_columns_retaining_encrypted_scratch_v1",
            "commit_encrypted_field_scratch_rows_v1",
            "der_fixed_lde_scratch_v1",
            "der_composition_lanes_from_scratches_v1",
            "der_fri_bases_from_scratches_v1",
            "evaluate_composition_chunks_at_deep_v1",
        ] {
            assert!(
                !builder.contains(forbidden),
                "production DER builder must not contain `{forbidden}`"
            );
        }
        for required in [
            "commit_masked_trace_polynomial_columns_v1",
            "der_composition_material_from_polynomials_v1",
            "evaluate_masked_trace_polynomial_columns_at_deep_v1",
            "evaluate_retained_composition_coefficients_at_deep_v1",
            "der_fri_bases_from_polynomials_v1",
            "replay_masked_trace_polynomial_columns_v1",
        ] {
            assert!(
                builder.contains(required),
                "production DER builder must contain `{required}`"
            );
        }
    }

    #[test]
    fn der_reduced_retained_commitment_openings_and_deep_are_byte_exact() {
        let native_columns = (0_u64..3)
            .map(|column| {
                (0_u64..8)
                    .map(|row| F::reduce(u128::from(column + 13) * u128::from(row + 29) + 7))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let initial_indices = [1, 9, 31];
        let mut replayable_rng = StdRng::from_seed([0xE1; 32]);
        let (replayable_commitment, masks) = aggregate::commit_masked_trace_columns_v1(
            b"zk-x509-der-reduced-retained-leaf",
            b"zk-x509-der-reduced-retained-node",
            0,
            3,
            6,
            native_columns.len(),
            3,
            &initial_indices,
            &mut replayable_rng,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("replayable reduced commitment");
        let mut retained_rng = StdRng::from_seed([0xE1; 32]);
        let (retained_commitment, polynomials) =
            aggregate::commit_masked_trace_polynomial_columns_v1(
                b"zk-x509-der-reduced-retained-leaf",
                b"zk-x509-der-reduced-retained-node",
                0,
                3,
                6,
                native_columns.len(),
                3,
                &initial_indices,
                &mut retained_rng,
                |column| Ok(native_columns[column].clone()),
            )
            .expect("retained reduced commitment");
        assert_eq!(retained_commitment, replayable_commitment);

        let replay_indices = [0, 7, 17, 63];
        let replayable_openings = aggregate::replay_masked_trace_columns_v1(
            b"zk-x509-der-reduced-retained-leaf",
            b"zk-x509-der-reduced-retained-node",
            0,
            &masks,
            &replay_indices,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("replayable reduced openings");
        let retained_openings = aggregate::replay_masked_trace_polynomial_columns_v1(
            b"zk-x509-der-reduced-retained-leaf",
            b"zk-x509-der-reduced-retained-node",
            0,
            &polynomials,
            &replay_indices,
        )
        .expect("retained reduced openings");
        assert_eq!(retained_openings, replayable_openings);

        let deep_point = E::canonical([101, 3, 5, 7]).expect("DEEP point");
        let replayable_deep =
            aggregate::evaluate_masked_native_columns_at_deep_v1(&masks, deep_point, |column| {
                Ok(native_columns[column].clone())
            })
            .expect("replayable DEEP");
        let retained_deep = aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(
            &polynomials,
            deep_point,
        )
        .expect("retained DEEP");
        assert_eq!(retained_deep, replayable_deep);

        let mut mutated_columns = native_columns.clone();
        mutated_columns[0][0] = mutated_columns[0][0].add(F::ONE);
        let mut mutation_rng = StdRng::from_seed([0xE1; 32]);
        let (mutated_commitment, _) = aggregate::commit_masked_trace_polynomial_columns_v1(
            b"zk-x509-der-reduced-retained-leaf",
            b"zk-x509-der-reduced-retained-node",
            0,
            3,
            6,
            mutated_columns.len(),
            3,
            &initial_indices,
            &mut mutation_rng,
            |column| Ok(mutated_columns[column].clone()),
        )
        .expect("mutated retained commitment");
        assert_ne!(
            mutated_commitment.commitment.root,
            retained_commitment.commitment.root
        );
    }

    #[test]
    fn der_reduced_deep_coefficient_accumulator_matches_pointwise_oracle_and_rejects_mutations() {
        const REDUCED_NATIVE_LOG2: u8 = 3;
        const REDUCED_LDE_LOG2: u8 = 6;
        const COEFFICIENT_CAP: usize = 16;

        fn evaluate_base(coefficients: &[F], point: E) -> E {
            coefficients
                .iter()
                .rev()
                .copied()
                .fold(E::ZERO, |value, coefficient| {
                    value.mul(point).add(E::from_base(coefficient))
                })
        }

        fn evaluate_extension(coefficients: &[E], point: E) -> E {
            coefficients
                .iter()
                .rev()
                .copied()
                .fold(E::ZERO, |value, coefficient| {
                    value.mul(point).add(coefficient)
                })
        }

        let base_coefficients = (0..3)
            .map(|column| {
                (0..12)
                    .map(|degree| {
                        F(u64::try_from(101 + column * 29 + degree).expect("base coefficient"))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let aux_coefficients = (0..2)
            .map(|column| {
                (0..10)
                    .map(|degree| {
                        F(u64::try_from(401 + column * 31 + degree).expect("aux coefficient"))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let composition_coefficients = (0..SECURITY_LANES)
            .map(|lane| {
                (0..COMPOSITION_DEGREE_CHUNKS)
                    .map(|chunk| {
                        let length = 9 - chunk;
                        (0..length)
                            .map(|degree| {
                                let value = u64::try_from(701 + lane * 101 + chunk * 17 + degree)
                                    .expect("composition coefficient");
                                E::canonical([value, value + 1, value + 2, value + 3])
                                    .expect("canonical composition coefficient")
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(
            base_coefficients
                .iter()
                .chain(&aux_coefficients)
                .all(|coefficients| coefficients.len() - 1 <= COEFFICIENT_CAP)
        );
        assert!(
            composition_coefficients
                .iter()
                .flatten()
                .all(|coefficients| { coefficients.len().saturating_sub(1) <= COEFFICIENT_CAP })
        );

        let deep_point = E::canonical([1_001, 7, 11, 13]).expect("canonical DEEP point");
        let native_root = goldilocks_primitive_root_v1(REDUCED_NATIVE_LOG2).expect("native root");
        let deep_next_point = deep_point.mul_base(native_root);
        let deep_trace = aggregate::AggregateOpenedDeepTraceGroupV1 {
            base_current: base_coefficients
                .iter()
                .map(|coefficients| evaluate_base(coefficients, deep_point))
                .collect(),
            base_next: base_coefficients
                .iter()
                .map(|coefficients| evaluate_base(coefficients, deep_next_point))
                .collect(),
            aux_current: aux_coefficients
                .iter()
                .map(|coefficients| evaluate_base(coefficients, deep_point))
                .collect(),
            aux_next: aux_coefficients
                .iter()
                .map(|coefficients| evaluate_base(coefficients, deep_next_point))
                .collect(),
        };
        let deep_compositions = composition_coefficients
            .iter()
            .map(|chunks| {
                chunks
                    .iter()
                    .map(|coefficients| evaluate_extension(coefficients, deep_point))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            evaluate_retained_composition_coefficients_at_deep_v1(
                &composition_coefficients,
                deep_point,
            )
            .expect("retained composition DEEP values"),
            deep_compositions
        );
        let mut missing_chunk = composition_coefficients.clone();
        missing_chunk[0].pop();
        assert!(
            evaluate_retained_composition_coefficients_at_deep_v1(&missing_chunk, deep_point)
                .is_err()
        );
        assert!(evaluate_retained_composition_coefficients_at_deep_v1(&[], deep_point).is_err());
        let mixes = (0..SECURITY_LANES)
            .map(|lane| FriMixV1 {
                base: (0..base_coefficients.len())
                    .map(|column| {
                        extension_v1(F(
                            u64::try_from(2_001 + lane * 101 + column).expect("base mix")
                        ))
                    })
                    .collect(),
                base_next: (0..base_coefficients.len())
                    .map(|column| {
                        extension_v1(F(
                            u64::try_from(2_201 + lane * 101 + column).expect("next base mix")
                        ))
                    })
                    .collect(),
                aux: (0..aux_coefficients.len())
                    .map(|column| {
                        extension_v1(F(
                            u64::try_from(2_401 + lane * 101 + column).expect("aux mix")
                        ))
                    })
                    .collect(),
                aux_next: (0..aux_coefficients.len())
                    .map(|column| {
                        extension_v1(F(
                            u64::try_from(2_601 + lane * 101 + column).expect("next aux mix")
                        ))
                    })
                    .collect(),
                composition: (0..COMPOSITION_DEGREE_CHUNKS)
                    .map(|chunk| {
                        extension_v1(F(
                            u64::try_from(2_801 + lane * 101 + chunk).expect("composition mix")
                        ))
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();

        let accumulated_coefficients = (0..SECURITY_LANES)
            .map(|lane| {
                let mut accumulator = vec![E::ZERO; COEFFICIENT_CAP];
                for (column, coefficients) in base_coefficients.iter().enumerate() {
                    accumulate_base_deep_quotient_v1(
                        coefficients,
                        deep_point,
                        deep_trace.base_current[column],
                        mixes[lane].base[column],
                        &mut accumulator,
                    )
                    .expect("base current coefficient quotient");
                    accumulate_base_deep_quotient_v1(
                        coefficients,
                        deep_next_point,
                        deep_trace.base_next[column],
                        mixes[lane].base_next[column],
                        &mut accumulator,
                    )
                    .expect("base next coefficient quotient");
                }
                for (column, coefficients) in aux_coefficients.iter().enumerate() {
                    accumulate_base_deep_quotient_v1(
                        coefficients,
                        deep_point,
                        deep_trace.aux_current[column],
                        mixes[lane].aux[column],
                        &mut accumulator,
                    )
                    .expect("aux current coefficient quotient");
                    accumulate_base_deep_quotient_v1(
                        coefficients,
                        deep_next_point,
                        deep_trace.aux_next[column],
                        mixes[lane].aux_next[column],
                        &mut accumulator,
                    )
                    .expect("aux next coefficient quotient");
                }
                for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
                    accumulate_extension_deep_quotient_v1(
                        &composition_coefficients[lane][chunk],
                        deep_point,
                        deep_compositions[lane][chunk],
                        mixes[lane].composition[chunk],
                        &mut accumulator,
                    )
                    .expect("composition coefficient quotient");
                }
                accumulator
            })
            .collect::<Vec<_>>();

        let lde_size = 1_usize << REDUCED_LDE_LOG2;
        let lde_root = goldilocks_primitive_root_v1(REDUCED_LDE_LOG2).expect("LDE root");
        let accumulated_evaluations = accumulated_coefficients
            .iter()
            .map(|coefficients| {
                goldilocks_fp4_evaluate_coset_v1(
                    coefficients,
                    lde_size,
                    lde_root,
                    F(GOLDILOCKS_GENERATOR_V1),
                )
                .expect("coefficient-accumulated LDE")
            })
            .collect::<Vec<_>>();

        for index in 0..lde_size {
            let x = F(GOLDILOCKS_GENERATOR_V1)
                .mul(lde_root.pow(u128::try_from(index).expect("LDE index")));
            let query_point = E::from_base(x);
            let current_inverse = query_point
                .sub(deep_point)
                .inv()
                .expect("current DEEP denominator");
            let next_inverse = query_point
                .sub(deep_next_point)
                .inv()
                .expect("next DEEP denominator");
            for lane in 0..SECURITY_LANES {
                let mut expected = E::ZERO;
                for (column, coefficients) in base_coefficients.iter().enumerate() {
                    let value = evaluate_base(coefficients, query_point);
                    expected = expected.add(
                        value
                            .sub(deep_trace.base_current[column])
                            .mul(current_inverse)
                            .mul(mixes[lane].base[column]),
                    );
                    expected = expected.add(
                        value
                            .sub(deep_trace.base_next[column])
                            .mul(next_inverse)
                            .mul(mixes[lane].base_next[column]),
                    );
                }
                for (column, coefficients) in aux_coefficients.iter().enumerate() {
                    let value = evaluate_base(coefficients, query_point);
                    expected = expected.add(
                        value
                            .sub(deep_trace.aux_current[column])
                            .mul(current_inverse)
                            .mul(mixes[lane].aux[column]),
                    );
                    expected = expected.add(
                        value
                            .sub(deep_trace.aux_next[column])
                            .mul(next_inverse)
                            .mul(mixes[lane].aux_next[column]),
                    );
                }
                for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
                    expected = expected.add(
                        evaluate_extension(&composition_coefficients[lane][chunk], query_point)
                            .sub(deep_compositions[lane][chunk])
                            .mul(current_inverse)
                            .mul(mixes[lane].composition[chunk]),
                    );
                }
                assert_eq!(
                    accumulated_evaluations[lane][index], expected,
                    "coefficient and pointwise DEEP-ALI paths differ at lane {lane}, row {index}"
                );
            }
        }

        let mut wrong_deep_accumulator = vec![E::ZERO; COEFFICIENT_CAP];
        assert!(matches!(
            accumulate_base_deep_quotient_v1(
                &base_coefficients[0],
                deep_point,
                deep_trace.base_current[0].add(E::ONE),
                E::ONE,
                &mut wrong_deep_accumulator,
            ),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));

        let mut mutated = base_coefficients[0].clone();
        mutated[0] = mutated[0].add(F::ONE);
        let mut mutation_accumulator = vec![E::ZERO; COEFFICIENT_CAP];
        assert!(matches!(
            accumulate_base_deep_quotient_v1(
                &mutated,
                deep_point,
                deep_trace.base_current[0],
                E::ONE,
                &mut mutation_accumulator,
            ),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));

        let mut undersized = vec![E::ZERO; base_coefficients[0].len() - 2];
        assert!(matches!(
            accumulate_base_deep_quotient_v1(
                &base_coefficients[0],
                deep_point,
                deep_trace.base_current[0],
                E::ONE,
                &mut undersized,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));

        let oversized_extension = vec![E::ONE; COEFFICIENT_CAP + 2];
        let mut bounded = vec![E::ZERO; COEFFICIENT_CAP];
        assert!(matches!(
            accumulate_extension_deep_quotient_v1(
                &oversized_extension,
                deep_point,
                evaluate_extension(&oversized_extension, deep_point),
                E::ONE,
                &mut bounded,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            accumulate_extension_deep_quotient_v1(&[], deep_point, E::ONE, E::ONE, &mut bounded,),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));
        let before = bounded.clone();
        accumulate_extension_deep_quotient_v1(&[], deep_point, E::ZERO, E::ONE, &mut bounded)
            .expect("the zero polynomial has the zero DEEP opening");
        assert_eq!(bounded, before);
    }
    #[test]
    #[ignore = "native log19/log22/log25 retained-coefficient proof, FRI, and RSS gate"]
    fn native_log19_der_proof_roundtrips_and_rejects_cross_layer_mutations() {
        let _guard = proof_guard();
        DER_FIXED_OPENING_EVALUATIONS_V1.store(0, std::sync::atomic::Ordering::SeqCst);
        let document = [0x30, 0x03, 0x02, 0x01, 0x01];
        let private_shape = build_zk_x509_der_stark_base_v1(&[&document])
            .expect("DER base")
            .private_shape;
        private_shape.validate().expect("private DER shape");
        let shape = ZkX509DerStarkShapeV1;
        let mut rng = StdRng::from_seed([0xD3; 32]);
        let prove_started = std::time::Instant::now();
        let proof =
            build_zk_x509_der_segmented_stark_proof_v1_with_rng(&shape, &[&document], &mut rng)
                .expect("native log19 DER proof");
        let prove_elapsed = prove_started.elapsed();
        assert!(
            prove_elapsed <= std::time::Duration::from_secs(ZK_X509_PROVER_TARGET_SECONDS_V1),
            "reference-host DER proof exceeded the release prover target: {prove_elapsed:?}"
        );
        let digest: [u8; 32] = Sha256::digest(&proof).into();
        assert!(
            proof.len() <= 1_800_000,
            "reference log19 DER proof exceeded its 1.8 MB release ceiling"
        );
        assert_eq!(
            proof.len(),
            1_752_584,
            "update only when the canonical log19 DER proof wire intentionally changes"
        );
        assert_eq!(
            hex::encode(digest),
            "954900070c22680460d07bc35969e1e71c8cf9f7e089d7b3abf97406b3e3df15",
            "update only when the canonical log19 DER proof protocol intentionally changes"
        );
        let first_verify_started = std::time::Instant::now();
        verify_zk_x509_der_segmented_stark_v1(&shape, &proof)
            .expect("native log19 DER first verification");
        let first_verify_elapsed = first_verify_started.elapsed();
        let repeat_verify_started = std::time::Instant::now();
        verify_zk_x509_der_segmented_stark_v1(&shape.clone(), &proof)
            .expect("native log19 DER repeat verification");
        let repeat_verify_elapsed = repeat_verify_started.elapsed();
        assert!(
            first_verify_elapsed <= std::time::Duration::from_secs(15),
            "reference-host first verification exceeded the 15-second release ceiling: \
             {first_verify_elapsed:?}"
        );
        assert!(
            repeat_verify_elapsed <= std::time::Duration::from_secs(15),
            "reference-host repeat verification exceeded the 15-second release ceiling: \
             {repeat_verify_elapsed:?}"
        );
        let fixed_evaluations_after_valid =
            DER_FIXED_OPENING_EVALUATIONS_V1.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(fixed_evaluations_after_valid, 2);
        eprintln!(
            "native log19 DER proof bytes={}, sha256={}, prove={prove_elapsed:?}, \
             first_verify={first_verify_elapsed:?}, repeat_verify={repeat_verify_elapsed:?}",
            proof.len(),
            hex::encode(digest)
        );
        assert_eq!(&proof[..4], b"X5P1");
        let (claims, aggregate_bytes) =
            decode_der_segmented_proof_envelope_v1(&proof).expect("DER envelope");
        let layout = SegmentLayoutV1::for_der(shape.active_rows()).expect("DER layout");
        let aggregate_layout =
            AggregateProofLayoutV1::for_segments(&[layout]).expect("aggregate layout");
        let aggregate = decode_zk_x509_segmented_stark_proof_v1(aggregate_bytes, &aggregate_layout)
            .expect("inner aggregate proof");

        let mut trailing = proof.clone();
        trailing.push(0);
        assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &trailing).is_err());
        let mut draft_magic = proof.clone();
        draft_magic[..4].copy_from_slice(b"X5D2");
        assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &draft_magic).is_err());
        let mut mistyped_claim = proof.clone();
        mistyped_claim[12] ^= 1;
        assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &mistyped_claim).is_err());
        let mut wrong_lane = proof.clone();
        wrong_lane[14] ^= 1;
        assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &wrong_lane).is_err());

        for claim_index in 0..DER_PROOF_CLAIM_COUNT_V1 {
            let mut changed_claims = claims;
            if claim_index < ZK_X509_DER_STARK_BUS_LANES_V1 {
                changed_claims.input_byte[claim_index] =
                    changed_claims.input_byte[claim_index].add(F::ONE);
            } else {
                changed_claims.node[claim_index - ZK_X509_DER_STARK_BUS_LANES_V1] =
                    changed_claims.node[claim_index - ZK_X509_DER_STARK_BUS_LANES_V1].add(F::ONE);
            }
            let changed = encode_der_segmented_proof_envelope_v1(changed_claims, aggregate_bytes)
                .expect("mutated claim envelope");
            assert!(
                verify_zk_x509_der_segmented_stark_v1(&shape, &changed).is_err(),
                "terminal claim {claim_index} must be transcript- and AIR-bound"
            );
        }

        let mut changed_private_shape = private_shape.clone();
        changed_private_shape.parser_rows += 1;
        assert_ne!(changed_private_shape, private_shape);
        assert_eq!(
            der_public_digest_v1(&shape).expect("fixed public digest"),
            der_public_digest_v1(&ZkX509DerStarkShapeV1).expect("fixed public digest")
        );

        let mut mutations = Vec::new();
        let mut changed = aggregate.clone();
        changed.trace_groups[0].base_root[0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.trace_groups[0].aux_root[0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.composition_roots[0][0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.fri_lanes[0].roots[0][0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.fri_lanes[0].terminal_values[0][0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.queries[0].trace_groups[0].base_current[0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.queries[0].trace_groups[0].aux_next[0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.queries[0].composition_values[0][0][0] ^= 1;
        mutations.push(changed);
        changed = aggregate.clone();
        changed.queries[0].fri_lanes[0].rounds[0].low[0] ^= 1;
        mutations.push(changed);
        changed = aggregate;
        changed.grinding_nonce ^= 1;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            let inner = encode_zk_x509_segmented_stark_proof_v1(mutation, &aggregate_layout)
                .expect("mutated inner proof");
            let outer = encode_der_segmented_proof_envelope_v1(claims, &inner)
                .expect("mutated outer proof");
            assert!(
                verify_zk_x509_der_segmented_stark_v1(&shape, &outer).is_err(),
                "aggregate layer mutation {index} must reject"
            );
        }
        assert_eq!(
            DER_FIXED_OPENING_EVALUATIONS_V1.load(std::sync::atomic::Ordering::SeqCst),
            fixed_evaluations_after_valid,
            "all malformed/transcript/Merkle mutations must reject before sampled fixed evaluation"
        );
    }

    #[test]
    fn projection_registration_is_exact_and_every_range_mutation_fails_closed() {
        let layout = projection_aggregate_layout();
        layout
            .validate()
            .expect("canonical projection registration");
        assert!(layout.validate_full_profile_registration().is_err());
        assert_eq!(layout.common_lde_log2, 15 + BLOWUP_LOG2);
        assert_eq!(layout.trace_groups.len(), 1);
        assert_eq!(layout.trace_groups[0].native_trace_log2, 15);
        assert_eq!(layout.trace_groups[0].column_chunks, 1);
        assert_eq!(
            layout.trace_groups[0].base_width,
            ZK_X509_PROJECTION_BASE_WIDTH_V1
        );
        assert_eq!(
            layout.trace_groups[0].aux_width,
            ZK_X509_PROJECTION_AUX_WIDTH_V1
        );
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        assert_eq!(registration.trace_group, 0);
        assert_eq!(registration.base_start, 0);
        assert_eq!(
            registration.base_end().expect("base end"),
            ZK_X509_PROJECTION_BASE_WIDTH_V1
        );
        assert_eq!(registration.aux_start, 0);
        assert_eq!(
            registration.aux_end().expect("aux end"),
            ZK_X509_PROJECTION_AUX_WIDTH_V1
        );
        assert_eq!(
            registration.segment.active_rows,
            ZK_X509_PROJECTION_TRACE_SIZE_V1
        );
        assert_eq!(registration.segment.trace_log2, 15);
        assert_eq!(registration.segment.lde_log2, 15 + BLOWUP_LOG2);
        assert_eq!(
            registration.segment.fixed_width,
            ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1
        );
        assert_eq!(
            registration.segment.constraint_count,
            ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1
        );
        assert_eq!(
            registration.segment.constraint_degree,
            ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1
        );

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments.clear();
        mutations.push(changed);
        changed = layout.clone();
        changed
            .registered_segments
            .push(changed.registered_segments[0]);
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::ByteMemory;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.instance = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.trace_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.fixed_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_count -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_degree += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].trace_group = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].base_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].aux_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].native_trace_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation.validate().is_err(),
                "projection registration mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn sha_batch_registration_is_exact_and_every_range_mutation_fails_closed() {
        let layout = sha_word_aggregate_layout();
        layout.validate().expect("canonical SHA-word registration");
        assert!(layout.validate_full_profile_registration().is_err());
        assert_eq!(layout.common_lde_log2, 25);
        assert_eq!(layout.trace_groups.len(), 1);
        assert_eq!(layout.trace_groups[0].native_trace_log2, 19);
        assert_eq!(
            layout.trace_groups[0].column_chunks,
            ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1
        );
        assert_eq!(
            layout.trace_groups[0].base_width,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1
        );
        assert_eq!(
            layout.trace_groups[0].aux_width,
            ZK_X509_SHA_BATCH_AUX_WIDTH_V1
        );
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Sha256CallBus, 0)
            .expect("SHA batch registration");
        assert_eq!(registration.trace_group, 0);
        assert_eq!(registration.base_start, 0);
        assert_eq!(
            registration.base_end().expect("base end"),
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1
        );
        assert_eq!(registration.aux_start, 0);
        assert_eq!(
            registration.aux_end().expect("aux end"),
            ZK_X509_SHA_BATCH_AUX_WIDTH_V1
        );
        assert_eq!(
            registration.segment.active_rows,
            ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0]
        );
        assert_eq!(
            registration.segment.trace_log2,
            ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
        );
        assert_eq!(
            registration.segment.lde_log2,
            ZK_X509_MAIN_COMMON_LDE_LOG2_V1
        );
        assert_eq!(
            registration.segment.fixed_width,
            ZK_X509_SHA_BATCH_FIXED_WIDTH_V1
        );
        assert_eq!(
            registration.segment.constraint_count,
            ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
        );
        assert_eq!(
            registration.segment.constraint_degree,
            ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1
        );

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments.clear();
        mutations.push(changed);
        changed = layout.clone();
        changed
            .registered_segments
            .push(changed.registered_segments[0]);
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::StrictDer;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.instance =
            u16::try_from(ZK_X509_SHA_SEGMENT_COUNT_V1).expect("small segment count");
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.trace_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 = 22;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.fixed_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_count -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].segment.constraint_degree -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].trace_group = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].base_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].aux_start = 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.registered_segments[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].base_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].aux_width -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.trace_groups[0].column_chunks += 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        changed = layout.clone();
        changed.common_lde_log2 = 22;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation.validate().is_err(),
                "SHA-word registration mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn p256_role_registrations_are_exact_minimal_and_bucketed_by_native_log() {
        let wallet = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::WalletOwnership)
            .expect("wallet P-256 registration");
        let certificate = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::CertificateOrCrl)
            .expect("certificate P-256 registration");
        wallet
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .expect("canonical wallet registration");
        certificate
            .validate_p256_registration_v1(P256EcdsaRoleV1::CertificateOrCrl)
            .expect("canonical certificate registration");
        assert_eq!(
            wallet.registered_segments.len(),
            P256_WALLET_REGISTRATION_COUNT_V1
        );
        assert_eq!(
            certificate.registered_segments.len(),
            P256_CERTIFICATE_REGISTRATION_COUNT_V1
        );
        assert_eq!(wallet.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
        assert_eq!(certificate.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
        assert_eq!(wallet.trace_groups.len(), 4);
        assert_eq!(certificate.trace_groups.len(), 4);
        assert!(
            wallet
                .registered_segments
                .iter()
                .any(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256LowS)
        );
        assert!(
            !certificate
                .registered_segments
                .iter()
                .any(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256LowS)
        );

        let wallet_instance =
            |local| p256_instance_v1(P256_SIGNATURE_COUNT_V1 - 1, local).expect("wallet instance");
        let expected = [
            (
                SegmentAdapterIdV1::P256Reduction,
                wallet_instance(0),
                P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1,
                P256_REDUCTION_BASE_WIDTH_V1,
                P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1,
                P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
                P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
                4,
                1,
            ),
            (
                SegmentAdapterIdV1::P256Reduction,
                wallet_instance(1),
                P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1,
                P256_REDUCTION_BASE_WIDTH_V1,
                P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1,
                P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
                P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
                4,
                1,
            ),
            (
                SegmentAdapterIdV1::P256LowS,
                wallet_instance(0),
                P256_LOW_S_AGGREGATE_TRACE_LOG2_V1,
                P256_LOW_S_BASE_WIDTH_V1,
                P256_LOW_S_AGGREGATE_AUX_WIDTH_V1,
                P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1,
                P256_LOW_S_REGISTERED_CONSTRAINT_COUNT_V1,
                3,
                1,
            ),
            (
                SegmentAdapterIdV1::P256ScalarBitBus,
                wallet_instance(0),
                P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1,
                P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1,
                P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
                P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1,
                P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1,
                3,
                1,
            ),
            (
                SegmentAdapterIdV1::P256Window,
                wallet_instance(0),
                P256_WINDOW_AGGREGATE_TRACE_LOG2_V1,
                P256_WINDOW_BASE_WIDTH_V1,
                P256_WINDOW_AGGREGATE_AUX_WIDTH_V1,
                P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1,
                P256_WINDOW_REGISTERED_CONSTRAINT_COUNT_V1,
                4,
                1,
            ),
            (
                SegmentAdapterIdV1::P256ValueBus,
                wallet_instance(2),
                P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1,
                P256_BINDING_SINK_BASE_WIDTH_V1,
                super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
                P256_BINDING_SINK_FIXED_WIDTH_V1,
                P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1,
                2,
                1,
            ),
            (
                SegmentAdapterIdV1::P256Arithmetic,
                wallet_instance(0),
                P256_ARITHMETIC_AGGREGATE_TRACE_LOG2_V1,
                P256_ARITHMETIC_BASE_WIDTH_V1,
                P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1,
                P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
                P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1,
                4,
                4,
            ),
            (
                SegmentAdapterIdV1::P256ValueBus,
                wallet_instance(0),
                P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
                P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
                P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1,
                P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
                P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1,
                3,
                2,
            ),
            (
                SegmentAdapterIdV1::P256ValueBus,
                wallet_instance(1),
                P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
                P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
                P256_VALUE_BUS_STARK_AUX_WIDTH_V1,
                P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
                P256_VALUE_SORTED_REGISTERED_CONSTRAINT_COUNT_V1,
                2,
                1,
            ),
        ];
        let expected_group = [0, 0, 0, 1, 2, 2, 3, 3, 3];
        let expected_base_start = [0, 56, 112, 0, 0, 61, 0, 211, 245];
        let expected_aux_start = [0, 19, 38, 0, 0, 37, 0, 72, 188];
        for (index, expected) in expected.into_iter().enumerate() {
            let registration = wallet.registered_segments[index];
            assert_eq!(
                (
                    registration.segment.adapter,
                    registration.segment.instance,
                    registration.segment.trace_log2,
                    registration.segment.base_width,
                    registration.segment.aux_width,
                    registration.segment.fixed_width,
                    registration.segment.constraint_count,
                    registration.segment.constraint_degree,
                    registration.column_chunks,
                ),
                expected
            );
            assert_eq!(registration.trace_group, expected_group[index]);
            assert_eq!(registration.base_start, expected_base_start[index]);
            assert_eq!(registration.aux_start, expected_aux_start[index]);
        }
        assert_eq!(
            wallet.trace_groups,
            vec![
                TraceGroupLayoutV1 {
                    native_trace_log2: 5,
                    column_chunks: 3,
                    base_width: 148,
                    aux_width: 52,
                },
                TraceGroupLayoutV1 {
                    native_trace_log2: 8,
                    column_chunks: 1,
                    base_width: 6,
                    aux_width: 32,
                },
                TraceGroupLayoutV1 {
                    native_trace_log2: 16,
                    column_chunks: 2,
                    base_width: P256_WINDOW_BASE_WIDTH_V1 + P256_BINDING_SINK_BASE_WIDTH_V1,
                    aux_width: P256_WINDOW_AGGREGATE_AUX_WIDTH_V1
                        + super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
                },
                TraceGroupLayoutV1 {
                    native_trace_log2: 19,
                    column_chunks: 7,
                    base_width: 279,
                    aux_width: 200,
                },
            ]
        );
        assert_eq!(
            certificate.trace_groups[0],
            TraceGroupLayoutV1 {
                native_trace_log2: 5,
                column_chunks: 2,
                base_width: 112,
                aux_width: 38,
            }
        );
        assert_eq!(
            P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1, 222,
            "value execution binds value, arithmetic-copy, and writer terminals"
        );
        assert!(
            wallet
                .validate_p256_registration_v1(P256EcdsaRoleV1::CertificateOrCrl)
                .is_err()
        );
        assert!(
            certificate
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );
    }

    #[test]
    fn p256_registration_rejects_omission_reorder_duplicate_reuse_and_splice() {
        let segments =
            canonical_p256_segment_layouts_v1(P256EcdsaRoleV1::WalletOwnership).expect("segments");
        let layout = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::WalletOwnership)
            .expect("wallet layout");
        for omitted in 0..segments.len() {
            let subset = segments
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, segment)| (index != omitted).then_some(segment))
                .collect::<Vec<_>>();
            let missing = AggregateProofLayoutV1::for_equal_log_buckets_v1(&subset)
                .expect("structural equal-log P-256 subset");
            assert!(
                missing
                    .validate_p256_instance_set_v1(P256EcdsaRoleV1::WalletOwnership)
                    .is_err()
            );
        }

        let mut duplicate = layout.clone();
        duplicate
            .registered_segments
            .push(duplicate.registered_segments[0]);
        assert!(
            duplicate
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );

        let mut reordered = layout.clone();
        reordered.registered_segments.swap(0, 1);
        assert!(
            reordered
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );

        let mut equal_shape_group_reuse = layout.clone();
        equal_shape_group_reuse.registered_segments[0].base_start =
            equal_shape_group_reuse.registered_segments[1].base_start;
        equal_shape_group_reuse.registered_segments[0].aux_start =
            equal_shape_group_reuse.registered_segments[1].aux_start;
        assert!(equal_shape_group_reuse.validate().is_err());
        assert!(
            equal_shape_group_reuse
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );

        let mut cross_identity_splice = layout.clone();
        cross_identity_splice.registered_segments.swap(4, 5);
        assert!(cross_identity_splice.validate().is_err());
        assert!(
            cross_identity_splice
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );

        let mut mutations = Vec::new();
        for registration_index in 0..layout.registered_segments.len() {
            for field in 0..10 {
                let mut changed = layout.clone();
                let segment = &mut changed.registered_segments[registration_index].segment;
                match field {
                    0 => segment.adapter = SegmentAdapterIdV1::Projection,
                    1 => segment.instance = segment.instance.saturating_add(7),
                    2 => segment.active_rows -= 1,
                    3 => segment.trace_log2 -= 1,
                    4 => segment.lde_log2 -= 1,
                    5 => segment.base_width -= 1,
                    6 => segment.aux_width -= 1,
                    7 => segment.fixed_width -= 1,
                    8 => segment.constraint_count -= 1,
                    9 => segment.constraint_degree = segment.constraint_degree.saturating_sub(1),
                    _ => unreachable!(),
                }
                mutations.push(changed);
            }
            let mut changed = layout.clone();
            changed.registered_segments[registration_index].base_start = 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            changed.registered_segments[registration_index].aux_start = 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            changed.registered_segments[registration_index].column_chunks += 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            let group_index = changed.registered_segments[registration_index].trace_group;
            changed.trace_groups[group_index].native_trace_log2 -= 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            let group_index = changed.registered_segments[registration_index].trace_group;
            changed.trace_groups[group_index].base_width -= 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            let group_index = changed.registered_segments[registration_index].trace_group;
            changed.trace_groups[group_index].aux_width -= 1;
            mutations.push(changed);
            let mut changed = layout.clone();
            let group_index = changed.registered_segments[registration_index].trace_group;
            changed.trace_groups[group_index].column_chunks += 1;
            mutations.push(changed);
        }
        let mut changed = layout;
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation
                    .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                    .is_err(),
                "P-256 layout mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn p256_terminal_registration_and_transcript_reject_all_role_mutations() {
        let role = P256EcdsaRoleV1::WalletOwnership;
        let registration = P256TraceRegistrationV1::new_v1(role).expect("P-256 registration");
        let terminals = p256_terminal_fixture(role);
        let challenge = |registration: &P256TraceRegistrationV1,
                         terminals: &P256TerminalRegistrationV1| {
            let mut transcript =
                TransparentTranscriptV1::new(b"p256-registration-test", &[0x91; 32], &[0x37; 32])
                    .expect("registration transcript");
            absorb_p256_registration_v1(&mut transcript, registration)
                .expect("static P-256 registration");
            absorb_p256_terminal_registration_v1(&mut transcript, registration.role, terminals)
                .expect("terminal registration");
            transcript
                .challenge_field(b"p256-registration-test-challenge-v1")
                .expect("registration challenge")
        };
        let canonical = challenge(&registration, &terminals);
        assert_eq!(
            challenge(&registration, &terminals),
            canonical,
            "registration transcript must replay exactly"
        );

        for index in 0..terminals.cross_sources.len() {
            for coordinate in 0..2 {
                let mut forged = terminals.clone();
                if coordinate == 0 {
                    forged.cross_sources[index].start[0] =
                        forged.cross_sources[index].start[0].add(F::ONE);
                } else {
                    forged.cross_sources[index].terminal[1] =
                        forged.cross_sources[index].terminal[1].add(F::ONE);
                }
                assert!(forged.validate(role).is_err());
                let mut transcript = TransparentTranscriptV1::new(
                    b"p256-registration-test",
                    &[0x91; 32],
                    &[0x37; 32],
                )
                .expect("registration transcript");
                absorb_p256_registration_v1(&mut transcript, &registration)
                    .expect("static registration");
                assert!(
                    absorb_p256_terminal_registration_v1(&mut transcript, role, &forged).is_err()
                );
            }
        }

        let mut omitted = terminals.clone();
        omitted.cross_sources.pop();
        assert!(omitted.validate(role).is_err());
        let mut duplicated = terminals.clone();
        duplicated.cross_sources.push(duplicated.cross_sources[0]);
        assert!(duplicated.validate(role).is_err());
        let mut reordered = terminals.clone();
        reordered.cross_sources.swap(0, 1);
        assert!(reordered.validate(role).is_err());
        let mut zero_start = terminals.clone();
        zero_start.cross_sources[0].start = [F::ZERO; P256_CROSS_TRACE_LANES_V1];
        assert!(zero_start.validate(role).is_err());

        for bus in 0..4 {
            let mut forged = terminals.clone();
            match bus {
                0 => forged.buses.value_sorted[0] = forged.buses.value_sorted[0].add(F::ONE),
                1 => {
                    forged.buses.arithmetic_value_copy[0] =
                        forged.buses.arithmetic_value_copy[0].add(F::ONE)
                }
                2 => {
                    forged.buses.scalar_bus_arithmetic[0] =
                        forged.buses.scalar_bus_arithmetic[0].add(F::ONE)
                }
                3 => {
                    forged.buses.scalar_bus_window[0] =
                        forged.buses.scalar_bus_window[0].add(F::ONE)
                }
                _ => unreachable!(),
            }
            assert!(forged.validate(role).is_err());
        }

        let certificate = P256TraceRegistrationV1::new_v1(P256EcdsaRoleV1::CertificateOrCrl)
            .expect("certificate registration");
        let certificate_terminals = p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl);
        assert_ne!(
            canonical,
            challenge(&certificate, &certificate_terminals),
            "role, segment multiplicity, and terminal-role count are transcript-bound"
        );
        assert_ne!(p256_aggregate_challenges_fixture().value, {
            let mut changed =
                TransparentTranscriptV1::new(b"p256-aggregate-test", &[0x31; 32], &[0x57; 32])
                    .expect("P-256 aggregate transcript");
            changed
                .absorb(b"adversarial-prefix", &[b"role-splice"])
                .expect("prefix mutation");
            derive_p256_aggregate_challenges_v1(&mut changed)
                .expect("mutated challenges")
                .value
        });
    }

    #[test]
    fn value_execution_writer_terminal_is_now_bound_under_coordinated_forgery() {
        let role = P256EcdsaRoleV1::WalletOwnership;
        let terminals = zero_p256_terminal_fixture(role);
        terminals
            .validate(role)
            .expect("zero-valued but consistently chained terminal fixture");
        let mut forged = terminals.clone();
        forged.cross_sources[0].terminal[0] = F::ONE;
        forged.cross_sources[1].start[0] = F::ONE;
        forged
            .validate(role)
            .expect("coordinated host-side chain remains equal");

        let layout = AggregateProofLayoutV1::for_p256_v1(role).expect("wallet layout");
        let registration = layout
            .registered_segment(
                SegmentAdapterIdV1::P256ValueBus,
                p256_instance_v1(P256_SIGNATURE_COUNT_V1 - 1, 0).expect("wallet instance"),
            )
            .expect("value-execution registration");
        let base_current = [F::ZERO; P256_VALUE_BUS_STARK_BASE_WIDTH_V1];
        let base_next = [F::ZERO; P256_VALUE_BUS_STARK_BASE_WIDTH_V1];
        let aux_current = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1];
        let aux_next = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base_current,
            base_next: &base_next,
            aux_current: &aux_current,
            aux_next: &aux_next,
        };
        let mut fixed = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1];
        let last_selector_column = (0..fixed.len())
            .find(|column| {
                fixed[*column] = F::ONE;
                let selected = p256_value_execution_last_selector_v1(&fixed) == F::ONE;
                fixed[*column] = F::ZERO;
                selected
            })
            .expect("verifier-derived value-execution last selector");
        fixed[last_selector_column] = F::ONE;
        let canonical = p256_opened_residues_v1(
            registration,
            opening,
            &fixed,
            p256_aggregate_challenges_fixture(),
            &terminals,
        )
        .expect("canonical value-execution residues");
        let forged_residues = p256_opened_residues_v1(
            registration,
            opening,
            &fixed,
            p256_aggregate_challenges_fixture(),
            &forged,
        )
        .expect("forged value-execution residues");
        assert_eq!(
            canonical.len(),
            P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1
        );
        assert!(
            canonical[P256_VALUE_EXECUTION_AGGREGATE_CONSTRAINT_COUNT_V1..]
                .iter()
                .all(|residue| *residue == F::ZERO)
        );
        let writer_terminal = &forged_residues[forged_residues.len() - P256_CROSS_TRACE_LANES_V1..];
        assert_ne!(writer_terminal[0], F::ZERO);
        assert!(
            writer_terminal[1..]
                .iter()
                .all(|residue| *residue == F::ZERO)
        );

        fixed[last_selector_column] = F::ZERO;
        let nongated = p256_opened_residues_v1(
            registration,
            opening,
            &fixed,
            p256_aggregate_challenges_fixture(),
            &forged,
        )
        .expect("nonterminal value-execution residues");
        assert!(
            nongated[P256_VALUE_EXECUTION_AGGREGATE_CONSTRAINT_COUNT_V1..]
                .iter()
                .all(|residue| *residue == F::ZERO)
        );
    }

    #[test]
    fn p256_opened_material_and_evaluator_reject_every_shape_boundary() {
        fn evaluate(
            material: &P256OpenedMaterialV1,
            challenges: P256AggregateChallengesV1,
            alphas: &[Vec<Vec<E>>],
            mixes: &[Vec<FriMixV1>],
            lde_root: F,
            query_index: usize,
            lane: usize,
            trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
            let mut evaluator = P256OpenedRowEvaluatorV1 {
                material,
                challenges,
                alphas,
                mixes,
                lde_root,
            };
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                trace_groups,
                &[E::ZERO; COMPOSITION_DEGREE_CHUNKS],
            )
        }

        let role = P256EcdsaRoleV1::WalletOwnership;
        let registration = P256TraceRegistrationV1::new_v1(role).expect("P-256 registration");
        let query_index = 17;
        let fixed_openings = registration
            .layout
            .registered_segments
            .iter()
            .map(|registered| {
                BTreeMap::from([(query_index, vec![F::ZERO; registered.segment.fixed_width])])
            })
            .collect::<Vec<_>>();
        let material = P256OpenedMaterialV1 {
            registration,
            terminals: zero_p256_terminal_fixture(role),
            fixed_openings,
        };
        material.validate().expect("canonical opened material");
        let trace_groups = material
            .registration
            .layout
            .trace_groups
            .iter()
            .map(|group| aggregate::AggregateOpenedTraceGroupV1 {
                base_current: vec![F::ZERO; group.base_width],
                base_next: vec![F::ZERO; group.base_width],
                aux_current: vec![F::ZERO; group.aux_width],
                aux_next: vec![F::ZERO; group.aux_width],
            })
            .collect::<Vec<_>>();
        let alphas = material
            .registration
            .layout
            .registered_segments
            .iter()
            .map(|registered| {
                (0..SECURITY_LANES)
                    .map(|_| vec![E::ONE; registered.segment.constraint_count])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mixes = material
            .registration
            .layout
            .trace_groups
            .iter()
            .map(|group| {
                (0..SECURITY_LANES)
                    .map(|_| FriMixV1 {
                        base: vec![E::ONE; group.base_width],
                        base_next: vec![E::ONE; group.base_width],
                        aux: vec![E::ONE; group.aux_width],
                        aux_next: vec![E::ONE; group.aux_width],
                        composition: vec![E::ONE; COMPOSITION_DEGREE_CHUNKS],
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let challenges = p256_aggregate_challenges_fixture();
        let lde_root = goldilocks_primitive_root_v1(material.registration.layout.common_lde_log2)
            .expect("common LDE root");
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .expect("well-shaped opened evaluation");

        let mut missing_fixed = material.clone();
        missing_fixed.fixed_openings[0].clear();
        assert!(missing_fixed.validate().is_err());
        let mut short_fixed = material.clone();
        short_fixed.fixed_openings[0]
            .get_mut(&query_index)
            .expect("fixed opening")
            .pop();
        assert!(short_fixed.validate().is_err());
        let mut missing_registration = material.clone();
        missing_registration.fixed_openings.pop();
        assert!(missing_registration.validate().is_err());
        let mut forged_terminal = material.clone();
        forged_terminal.terminals.buses.value_sorted[0] =
            forged_terminal.terminals.buses.value_sorted[0].add(F::ONE);
        assert!(forged_terminal.validate().is_err());

        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index + 1,
                0,
                &trace_groups,
            )
            .is_err(),
            "an uncompiled verifier-fixed opening must be rejected"
        );
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index,
                SECURITY_LANES,
                &trace_groups,
            )
            .is_err()
        );

        let mut invalid_challenges = challenges;
        invalid_challenges.cross.lanes[0].terms[0] = F::ZERO;
        assert!(
            evaluate(
                &material,
                invalid_challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &trace_groups,
            )
            .is_err()
        );

        let mut short_groups = trace_groups.clone();
        short_groups.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &short_groups,
            )
            .is_err()
        );
        let mut short_base = trace_groups.clone();
        short_base[0].base_current.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &short_base,
            )
            .is_err()
        );
        let mut short_aux = trace_groups.clone();
        short_aux[0].aux_next.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &short_aux,
            )
            .is_err()
        );

        let mut short_alphas = alphas.clone();
        short_alphas.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &short_alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &trace_groups,
            )
            .is_err()
        );
        let mut short_residue_alphas = alphas.clone();
        short_residue_alphas[0][0].pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &short_residue_alphas,
                &mixes,
                lde_root,
                query_index,
                0,
                &trace_groups,
            )
            .is_err()
        );

        let mut short_mixes = mixes.clone();
        short_mixes.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &short_mixes,
                lde_root,
                query_index,
                0,
                &trace_groups,
            )
            .is_err()
        );
        let mut short_base_mix = mixes;
        short_base_mix[0][0].base.pop();
        assert!(
            evaluate(
                &material,
                challenges,
                &alphas,
                &short_base_mix,
                lde_root,
                query_index,
                0,
                &trace_groups,
            )
            .is_err()
        );
    }

    #[test]
    fn deterministic_projection_proof_roundtrips_and_has_a_protocol_kat() {
        let (statement, _, proof) = projection_fixture();
        verify_zk_x509_projection_segmented_stark_v1(statement, proof)
            .expect("valid projection proof");
        assert!(proof.len() <= ZK_X509_MAX_PROOF_BYTES_V1 as usize);
        assert_eq!(&proof[..4], &PROOF_MAGIC_V1);
        let decoded = decode_projection_fixture();
        assert_eq!(
            exact_encoded_aggregate_proof_bytes_v1(&decoded, &projection_aggregate_layout())
                .expect("exact wire size"),
            proof.len()
        );
        assert!(
            proof.len()
                <= maximum_encoded_aggregate_proof_bytes_v1(&projection_aggregate_layout())
                    .expect("maximum wire size")
        );
        assert_eq!(
            encode_zk_x509_segmented_stark_proof_v1(&decoded, &projection_aggregate_layout())
                .expect("canonical re-encode"),
            *proof
        );
        let indices = decoded
            .queries
            .iter()
            .map(|query| query.index)
            .collect::<BTreeSet<_>>();
        assert_eq!(indices.len(), QUERY_COUNT);
        let digest: [u8; 32] = Sha256::digest(proof).into();
        assert_eq!(
            hex::encode(digest),
            "bd7e1827553dcaed27a5b18a4e29a9d5db948aa3a8ad7ace3427e98ee87d279b",
            "update only when the canonical projection proof protocol intentionally changes"
        );
    }

    #[test]
    fn aggregate_proof_shape_rejects_empty_duplicate_and_reordered_group_material() {
        let mut changed = decode_fixture();
        changed.trace_groups.clear();
        assert_rejected(&changed);

        changed = decode_fixture();
        let duplicate_group = changed.trace_groups[0].clone();
        changed.trace_groups.push(duplicate_group);
        assert_rejected(&changed);

        let mut layout = fixture_aggregate_layout();
        layout.registered_segments.clear();
        assert!(layout.validate().is_err());
    }

    #[test]
    fn deterministic_proof_roundtrips_and_has_unique_post_grinding_queries() {
        let (statement, proof) = fixture();
        verify_zk_x509_io_segmented_stark_v1(statement, proof).expect("valid proof");
        assert!(
            ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1
                .windows(b"sha256".len())
                .any(|window| window == b"sha256")
        );
        assert!(
            !ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1
                .windows(b"poseidon".len())
                .any(|window| window == b"poseidon")
        );
        assert!(proof.len() <= ZK_X509_MAX_PROOF_BYTES_V1 as usize);
        assert_eq!(&proof[..4], &PROOF_MAGIC_V1);
        let decoded = decode_fixture();
        assert_eq!(
            exact_encoded_aggregate_proof_bytes_v1(&decoded, &fixture_aggregate_layout())
                .expect("exact wire size"),
            proof.len()
        );
        assert!(
            proof.len()
                <= maximum_encoded_aggregate_proof_bytes_v1(&fixture_aggregate_layout())
                    .expect("maximum wire size")
        );
        assert_eq!(
            encode_zk_x509_segmented_stark_proof_v1(&decoded, &fixture_aggregate_layout())
                .expect("canonical re-encode"),
            *proof
        );
        let indices = decoded
            .queries
            .iter()
            .map(|query| query.index)
            .collect::<BTreeSet<_>>();
        assert_eq!(indices.len(), QUERY_COUNT);
        let digest: [u8; 32] = Sha256::digest(proof).into();
        assert_eq!(
            hex::encode(digest),
            "edb5e8a1e839059f449a818f53eaac0598f6c634ca2e0767a211ad70c4f08bce",
            "update only when the canonical proof protocol intentionally changes"
        );
    }

    #[test]
    fn witness_and_entropy_failures_are_rejected_before_emission() {
        let statement = fixture_statement();
        let mut changed = fixture_witnesses();
        changed[0].consumer_values[0][0] ^= 1;
        let mut rng = StdRng::from_seed([7; 32]);
        assert!(matches!(
            prove_zk_x509_io_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::IoWitness)
        ));

        let mut changed = fixture_witnesses();
        changed[1]
            .declaration
            .public_value
            .as_mut()
            .expect("public")[0] ^= 1;
        assert!(matches!(
            prove_zk_x509_io_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::WitnessStatementMismatch)
        ));

        assert!(matches!(
            prove_zk_x509_io_segmented_stark_v1_with_rng(
                &statement,
                &fixture_witnesses(),
                &mut MaxValueRng
            ),
            Err(ZkX509StarkErrorV1::RandomnessUnavailable)
        ));
    }

    #[test]
    fn exact_wire_rejects_truncation_trailing_magic_version_and_counts() {
        let (statement, proof) = fixture();
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(statement, &[]),
            Err(ZkX509StarkErrorV1::MalformedProof)
        ));
        for length in [1, 4, 7, proof.len() / 4, proof.len() / 2, proof.len() - 1] {
            assert!(
                verify_zk_x509_io_segmented_stark_v1(statement, &proof[..length]).is_err(),
                "prefix length {length} must reject"
            );
        }
        let mut trailing = proof.clone();
        trailing.push(0);
        assert!(verify_zk_x509_io_segmented_stark_v1(statement, &trailing).is_err());
        let mut wrong_magic = proof.clone();
        wrong_magic[0] ^= 1;
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(statement, &wrong_magic),
            Err(ZkX509StarkErrorV1::MalformedProof)
        ));
        let mut wrong_version = proof.clone();
        wrong_version[5] ^= 1;
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(statement, &wrong_version),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut wrong_segments = proof.clone();
        wrong_segments[7] ^= 1;
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(statement, &wrong_segments),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(
                statement,
                &vec![0_u8; ZK_X509_MAX_PROOF_BYTES_V1 as usize + 1]
            ),
            Err(ZkX509StarkErrorV1::ProofTooLarge)
        ));
    }

    #[test]
    fn every_committed_column_family_and_opening_family_is_bound() {
        let mut changed = decode_fixture();
        changed.trace_groups[0].base_root[0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.trace_groups[0].aux_root[0] ^= 1;
        assert_rejected(&changed);
        for lane in 0..SECURITY_LANES {
            changed = decode_fixture();
            changed.composition_roots[lane][0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.fri_lanes[lane].roots[0][0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.queries[0].composition_values[lane][0][0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.composition_frontiers[lane][0][0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.queries[0].fri_lanes[lane].rounds[0].low[0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.queries[0].fri_lanes[lane].rounds[0].high[0] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.fri_lanes[lane].round_frontiers[0][0][0] ^= 1;
            assert_rejected(&changed);
        }
        for column in 0..IO_BASE_WIDTH {
            changed = decode_fixture();
            changed.queries[0].trace_groups[0].base_current[column] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.queries[0].trace_groups[0].base_next[column] ^= 1;
            assert_rejected(&changed);
        }
        for column in 0..IO_AUX_WIDTH {
            changed = decode_fixture();
            changed.queries[0].trace_groups[0].aux_current[column] ^= 1;
            assert_rejected(&changed);
            changed = decode_fixture();
            changed.queries[0].trace_groups[0].aux_next[column] ^= 1;
            assert_rejected(&changed);
        }
        changed = decode_fixture();
        changed.trace_groups[0].base_frontier[0][0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.trace_groups[0].aux_frontier[0][0] ^= 1;
        assert_rejected(&changed);
    }

    #[test]
    fn every_deep_value_order_omission_and_replay_is_rejected() {
        let canonical = decode_fixture();
        for column in 0..canonical.deep.trace_groups[0].base_current.len() {
            let mut changed = canonical.clone();
            changed.deep.trace_groups[0].base_current[column][0] ^= 1;
            assert_rejected(&changed);
            let mut changed = canonical.clone();
            changed.deep.trace_groups[0].base_next[column][0] ^= 1;
            assert_rejected(&changed);
        }
        for column in 0..canonical.deep.trace_groups[0].aux_current.len() {
            let mut changed = canonical.clone();
            changed.deep.trace_groups[0].aux_current[column][0] ^= 1;
            assert_rejected(&changed);
            let mut changed = canonical.clone();
            changed.deep.trace_groups[0].aux_next[column][0] ^= 1;
            assert_rejected(&changed);
        }
        for lane in 0..canonical.deep.composition_values.len() {
            for chunk in 0..canonical.deep.composition_values[lane].len() {
                let mut changed = canonical.clone();
                changed.deep.composition_values[lane][chunk][0] ^= 1;
                assert_rejected(&changed);
            }
        }

        let mut reordered = canonical.clone();
        let group = &mut reordered.deep.trace_groups[0];
        core::mem::swap(&mut group.base_current, &mut group.base_next);
        assert_rejected(&reordered);
        let mut reordered = canonical.clone();
        let group = &mut reordered.deep.trace_groups[0];
        core::mem::swap(&mut group.aux_current, &mut group.aux_next);
        assert_rejected(&reordered);

        let mut omitted = canonical.clone();
        omitted.deep.trace_groups[0].base_current.pop();
        assert!(matches!(
            encode_zk_x509_segmented_stark_proof_v1(&omitted, &fixture_aggregate_layout()),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut duplicated = canonical.clone();
        let duplicate = duplicated.deep.trace_groups[0].aux_next[0];
        duplicated.deep.trace_groups[0].aux_next.push(duplicate);
        assert!(matches!(
            encode_zk_x509_segmented_stark_proof_v1(&duplicated, &fixture_aggregate_layout()),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));

        let mut replay_rng = StdRng::from_seed([0xC7; 32]);
        let replay_bytes = prove_zk_x509_io_segmented_stark_v1_with_rng(
            &fixture().0,
            &fixture_witnesses(),
            &mut replay_rng,
        )
        .expect("independent masked proof");
        let replay =
            decode_zk_x509_segmented_stark_proof_v1(&replay_bytes, &fixture_aggregate_layout())
                .expect("independent proof decoding");
        let mut spliced = canonical;
        spliced.deep = replay.deep;
        assert_rejected(&spliced);
    }

    #[test]
    fn every_projection_committed_column_and_opening_family_is_bound() {
        let mut changed = decode_projection_fixture();
        changed.trace_groups[0].base_root[0] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.trace_groups[0].aux_root[0] ^= 1;
        assert_projection_rejected(&changed);
        for lane in 0..SECURITY_LANES {
            changed = decode_projection_fixture();
            changed.composition_roots[lane][0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.fri_lanes[lane].roots[0][0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.queries[0].composition_values[lane][0][0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.composition_frontiers[lane][0][0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.queries[0].fri_lanes[lane].rounds[0].low[0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.queries[0].fri_lanes[lane].rounds[0].high[0] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.fri_lanes[lane].round_frontiers[0][0][0] ^= 1;
            assert_projection_rejected(&changed);
        }
        for column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
            changed = decode_projection_fixture();
            changed.queries[0].trace_groups[0].base_current[column] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.queries[0].trace_groups[0].base_next[column] ^= 1;
            assert_projection_rejected(&changed);
        }
        for column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
            changed = decode_projection_fixture();
            changed.queries[0].trace_groups[0].aux_current[column] ^= 1;
            assert_projection_rejected(&changed);
            changed = decode_projection_fixture();
            changed.queries[0].trace_groups[0].aux_next[column] ^= 1;
            assert_projection_rejected(&changed);
        }
        changed = decode_projection_fixture();
        changed.trace_groups[0].base_frontier[0][0] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.trace_groups[0].aux_frontier[0][0] ^= 1;
        assert_projection_rejected(&changed);
    }

    #[test]
    fn invalid_projection_witnesses_and_entropy_fail_before_emission() {
        let (statement, witness) =
            crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let mut rng = StdRng::from_seed([0x31; 32]);

        let mut changed = witness.clone();
        changed.chain_spki_der[0][0] ^= 1;
        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::ProjectionWitness)
        ));

        let mut changed = witness.clone();
        changed.leaf_serial[0] ^= 1;
        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::ProjectionWitness)
        ));

        let mut changed = witness.clone();
        changed.disclosed_attribute_values[0][0] ^= 1;
        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::ProjectionWitness)
        ));

        let mut changed = witness.clone();
        changed.attribute_salts[0][0] ^= 1;
        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::ProjectionWitness)
        ));

        let mut changed = witness.clone();
        changed.chain_spki_der.pop();
        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
            Err(ZkX509StarkErrorV1::ProjectionWitness)
        ));

        assert!(matches!(
            prove_zk_x509_projection_segmented_stark_v1_with_rng(
                &statement,
                &witness,
                &mut MaxValueRng
            ),
            Err(ZkX509StarkErrorV1::RandomnessUnavailable)
        ));
    }

    #[test]
    fn multiproof_frontiers_reject_nonminimal_duplicate_reordered_and_superfluous_siblings() {
        let mut changed = decode_fixture();
        changed.trace_groups[0].base_frontier.pop();
        assert_rejected(&changed);

        changed = decode_fixture();
        let duplicate = changed.trace_groups[0].base_frontier[0];
        changed.trace_groups[0].base_frontier.push(duplicate);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.trace_groups[0].base_frontier[1] = changed.trace_groups[0].base_frontier[0];
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.trace_groups[0].base_frontier.swap(0, 1);
        assert_rejected(&changed);

        changed = decode_fixture();
        let superfluous = changed.trace_groups[0].aux_frontier[0];
        changed.trace_groups[0].aux_frontier.insert(0, superfluous);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.composition_frontiers[0].swap(0, 1);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.fri_lanes[0].round_frontiers[0].swap(0, 1);
        assert_rejected(&changed);
    }

    #[test]
    fn query_reordering_duplication_grinding_and_composition_mutations_reject() {
        let mut changed = decode_fixture();
        changed.queries.swap(0, 1);
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[1] = changed.queries[0].clone();
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.grinding_nonce ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.composition_roots[0][0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.fri_lanes[0].roots[0][0] ^= 1;
        assert_rejected(&changed);
    }

    #[test]
    fn noncanonical_fields_and_terminal_high_degree_reject() {
        let mut changed = decode_fixture();
        changed.queries[0].trace_groups[0].base_current[0] =
            crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        assert!(matches!(
            encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout()),
            Err(ZkX509StarkErrorV1::NonCanonicalField)
        ));

        changed = decode_fixture();
        changed.deep.trace_groups[0].base_current[0][0] =
            crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
        assert!(matches!(
            encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout()),
            Err(ZkX509StarkErrorV1::NonCanonicalField)
        ));

        changed = decode_fixture();
        changed.fri_lanes[0].terminal_values[TERMINAL_SIZE - 1][0] ^= 1;
        let terminal = aggregate::canonical_fp4_fields_v1(
            &changed.fri_lanes[0].terminal_values,
            TERMINAL_SIZE,
        )
        .expect("canonical mutation");
        let tree = fri_tree_v1(0, fixture_aggregate_layout().fri_rounds(), &terminal)
            .expect("terminal tree");
        changed.fri_lanes[0].roots[fixture_aggregate_layout().fri_rounds()] = tree.root();
        let bytes = encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout())
            .expect("encode high-degree terminal");
        assert!(matches!(
            verify_zk_x509_io_segmented_stark_v1(&fixture().0, &bytes),
            Err(ZkX509StarkErrorV1::FriDegree)
        ));
    }

    #[test]
    fn every_public_topology_and_public_byte_change_rejects_replay() {
        let proof = &fixture().1;
        let mut declarations = fixture().0.declarations.clone();
        declarations[1].public_value.as_mut().expect("public")[0] ^= 1;
        let statement = ZkX509IoStarkStatementV1::new(declarations).expect("valid changed public");
        assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());

        let mut declarations = fixture().0.declarations.clone();
        declarations[0].producer.instance += 1;
        let statement =
            ZkX509IoStarkStatementV1::new(declarations).expect("valid changed producer");
        assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());

        let mut declarations = fixture().0.declarations.clone();
        declarations[0].consumers[0].instance += 1;
        let statement =
            ZkX509IoStarkStatementV1::new(declarations).expect("valid changed consumer");
        assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());
    }

    #[test]
    fn every_projection_public_field_and_output_rejects_proof_replay() {
        let (baseline, _, proof) = projection_fixture();
        let mut mutations = Vec::<(&str, IrohaZkX509StarkP256StatementV1)>::new();

        let mut changed = baseline.clone();
        changed.context.chain_id = ChainId::from("projection-air-replay");
        mutations.push(("context.chain_id", changed));
        let mut changed = baseline.clone();
        changed.context.action_index += 1;
        mutations.push(("context.action_index", changed));
        let mut changed = baseline.clone();
        changed.context.transaction_intent_digest =
            PrivacyTransactionIntentDigestV1::new([0x81; 32]);
        mutations.push(("context.transaction_intent_digest", changed));
        let mut changed = baseline.clone();
        changed.context.parameter_id = PrivacyParameterIdV1::new([0x82; 32]);
        mutations.push(("context.parameter_id", changed));
        let mut changed = baseline.clone();
        changed.context.parameter_digest = PrivacyParameterDigestV1::new([0x83; 32]);
        mutations.push(("context.parameter_digest", changed));
        let mut changed = baseline.clone();
        changed.context.verifier_digest = PrivacyVerifierDigestV1::new([0x84; 32]);
        mutations.push(("context.verifier_digest", changed));
        let mut changed = baseline.clone();
        changed.context.statement_schema_digest = PrivacyStatementSchemaDigestV1::new([0x85; 32]);
        mutations.push(("context.statement_schema_digest", changed));
        let mut changed = baseline.clone();
        changed.context.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0x86; 32]);
        mutations.push(("context.engine_manifest_digest", changed));

        let mut changed = baseline.clone();
        changed.trust_anchor_id = PrivacyIssuerIdV1::new([0x87; 32]);
        mutations.push(("trust_anchor_id", changed));
        let mut changed = baseline.clone();
        changed.certificate_policy_id = PrivacyPolicyIdV1::new([0x88; 32]);
        mutations.push(("certificate_policy_id", changed));
        let mut changed = baseline.clone();
        changed.trust_anchor_record_digest =
            PrivacyZkX509TrustAnchorRecordDigestV1::new([0x89; 32]);
        mutations.push(("trust_anchor_record_digest", changed));
        let mut changed = baseline.clone();
        changed.trust_anchor_record_epoch += 1;
        mutations.push(("trust_anchor_record_epoch", changed));
        let mut changed = baseline.clone();
        changed.certificate_policy_record_digest =
            PrivacyZkX509CertificatePolicyRecordDigestV1::new([0x8A; 32]);
        mutations.push(("certificate_policy_record_digest", changed));
        let mut changed = baseline.clone();
        changed.certificate_policy_record_epoch += 1;
        mutations.push(("certificate_policy_record_epoch", changed));
        let mut changed = baseline.clone();
        changed.crl_record_digest = PrivacyZkX509CrlRecordDigestV1::new([0x8B; 32]);
        mutations.push(("crl_record_digest", changed));
        let mut changed = baseline.clone();
        changed.crl_record_epoch += 1;
        mutations.push(("crl_record_epoch", changed));

        let mut changed = baseline.clone();
        changed.subject_public_key_digest = PrivacyCertificateKeyDigestV1::new([0x8C; 32]);
        mutations.push(("subject_public_key_digest", changed));
        let mut changed = baseline.clone();
        changed.ca_membership_root = PrivacyRootV1::new([0x8D; 32]);
        mutations.push(("ca_membership_root", changed));
        let mut changed = baseline.clone();
        changed.ca_membership_root_epoch += 1;
        mutations.push(("ca_membership_root_epoch", changed));
        let mut changed = baseline.clone();
        changed.key_usage.digital_signature =
            (!changed.key_usage.digital_signature.is_required()).into();
        mutations.push(("key_usage.digital_signature", changed));
        let mut changed = baseline.clone();
        changed.key_usage.content_commitment =
            (!changed.key_usage.content_commitment.is_required()).into();
        mutations.push(("key_usage.content_commitment", changed));
        let mut changed = baseline.clone();
        changed.key_usage.key_encipherment =
            (!changed.key_usage.key_encipherment.is_required()).into();
        mutations.push(("key_usage.key_encipherment", changed));
        let mut changed = baseline.clone();
        changed.key_usage.key_agreement = (!changed.key_usage.key_agreement.is_required()).into();
        mutations.push(("key_usage.key_agreement", changed));
        let mut changed = baseline.clone();
        changed
            .extended_key_usages
            .insert(1, PrivacyX509ExtendedKeyUsageV1::DocumentSigning);
        mutations.push(("extended_key_usages", changed));

        for disclosure in 0..baseline.disclosed_attributes.len() {
            let mut changed = baseline.clone();
            changed.disclosed_attributes[disclosure].index = match disclosure {
                0 => 1,
                _ => 2,
            };
            mutations.push(("disclosed_attributes.index", changed));
            let mut changed = baseline.clone();
            changed.disclosed_attributes[disclosure].attribute_digest =
                PrivacyAttributeDigestV1::new([0x90 + disclosure as u8; 32]);
            mutations.push(("disclosed_attributes.attribute_digest", changed));
        }
        let mut changed = baseline.clone();
        changed.disclosed_attributes.clear();
        mutations.push(("disclosed_attributes.length", changed));

        let mut changed = baseline.clone();
        changed.presentation_not_before_unix_seconds += 1;
        mutations.push(("presentation_not_before_unix_seconds", changed));
        let mut changed = baseline.clone();
        changed.presentation_not_after_unix_seconds -= 1;
        mutations.push(("presentation_not_after_unix_seconds", changed));
        let mut changed = baseline.clone();
        changed.wallet_account =
            crate::privacy_engines::zk_x509::projection_air::tests::account(0xA1);
        mutations.push(("wallet_account", changed));
        let mut changed = baseline.clone();
        changed.wallet_challenge = PrivacyChallengeV1::new([0x96; 32]);
        mutations.push(("wallet_challenge", changed));
        let mut changed = baseline.clone();
        changed.certificate_nullifier = PrivacyNullifierV1::new([0x97; 32]);
        mutations.push(("certificate_nullifier", changed));

        for (field, statement) in mutations {
            assert!(
                verify_zk_x509_projection_segmented_stark_v1(&statement, proof).is_err(),
                "replaying a proof after mutating {field} must reject"
            );
        }
    }

    #[test]
    fn compact_ca_registration_is_single_fixed_capacity_and_fail_closed() {
        assert_eq!(
            checked_compact_ca_degree_capacity_v1(
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            )
            .expect("canonical compact-CA capacity"),
            (1_171, 511)
        );
        assert_eq!(
            checked_compact_ca_degree_capacity_v1(
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                2,
            )
            .expect("lower-degree compact-CA capacity"),
            (738, 511)
        );
        for (trace_log2, lde_log2, constraint_degree) in [
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1 - 1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            ),
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1 + 1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            ),
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1 - 1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            ),
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1 + 1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            ),
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                1,
            ),
            (
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                4,
            ),
        ] {
            assert!(
                checked_compact_ca_degree_capacity_v1(trace_log2, lde_log2, constraint_degree,)
                    .is_err(),
                "adjacent compact-CA geometry ({trace_log2}, {lde_log2}, \
                 {constraint_degree}) must fail closed"
            );
        }
        assert!(
            checked_segment_degree_capacity_v1(
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            )
            .is_err(),
            "compact-CA must not be accepted under the MAIN mask and terminal profile"
        );

        let layout = accumulator_aggregate_layout();
        layout
            .validate_accumulator_registration_v1()
            .expect("canonical compact-CA registration");
        assert_eq!(layout.parameters_v1(), CA_AGGREGATE_PARAMETERS_V1);
        assert_eq!(layout.common_lde_log2, ZK_X509_CA_FRI_LDE_LOG2_V1);
        assert_eq!(layout.fri_rounds(), 5);
        assert_eq!(layout.trace_groups.len(), 1);
        assert_eq!(layout.registered_segments.len(), 1);
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::CaAccumulator, 0)
            .expect("compact-CA registration");
        assert_eq!(registration.segment.active_rows, 13);
        assert_eq!(registration.segment.trace_log2, 7);
        assert_eq!(registration.segment.lde_log2, ZK_X509_CA_FRI_LDE_LOG2_V1);
        assert_eq!(registration.segment.base_width, 695);
        assert_eq!(registration.segment.aux_width, 128);
        assert_eq!(registration.segment.fixed_width, 80);
        assert_eq!(registration.segment.constraint_count, 1_379);
        assert_eq!(registration.segment.constraint_degree, 3);
        assert_eq!(registration.column_chunks, ZK_X509_CA_ACCUMULATOR_CHUNKS_V1);
        assert_eq!(
            layout.trace_groups[0].column_chunks,
            ZK_X509_CA_ACCUMULATOR_CHUNKS_V1
        );

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.instance = 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.active_rows = 12;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.physical_chunks += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.common_lde_log2 -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.common_lde_log2 += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.lde_log2 += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.constraint_degree = 4;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.trace_log2 += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].column_chunks += 1;
        changed.trace_groups[0].column_chunks += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::Projection;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed
            .registered_segments
            .push(changed.registered_segments[0]);
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation.validate_accumulator_registration_v1().is_err(),
                "compact-CA mutation {index} must fail closed"
            );
        }
    }

    #[test]
    fn main_provider_registry_is_exactly_six_closed_ordered_groups() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");

        {
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainTraceProviderSetV1::new_v1(
                    &layout,
                    vec![
                        MainTraceGroupProviderV1::TestLog5(&mut a),
                        MainTraceGroupProviderV1::TestLog8(&mut b),
                        MainTraceGroupProviderV1::Log15(&mut c),
                        MainTraceGroupProviderV1::TestLog16(&mut d),
                        MainTraceGroupProviderV1::Log18(&mut e),
                    ],
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        {
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            let mut f = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainTraceProviderSetV1::new_v1(
                    &layout,
                    vec![
                        MainTraceGroupProviderV1::TestLog8(&mut a),
                        MainTraceGroupProviderV1::TestLog5(&mut b),
                        MainTraceGroupProviderV1::Log15(&mut c),
                        MainTraceGroupProviderV1::TestLog16(&mut d),
                        MainTraceGroupProviderV1::Log18(&mut e),
                        MainTraceGroupProviderV1::Log19(&mut f),
                    ],
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        {
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            let mut f = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainTraceProviderSetV1::new_v1(
                    &layout,
                    vec![
                        MainTraceGroupProviderV1::TestLog5(&mut a),
                        MainTraceGroupProviderV1::TestLog5(&mut b),
                        MainTraceGroupProviderV1::Log15(&mut c),
                        MainTraceGroupProviderV1::TestLog16(&mut d),
                        MainTraceGroupProviderV1::Log18(&mut e),
                        MainTraceGroupProviderV1::Log19(&mut f),
                    ],
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        {
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            let mut f = MockMainTraceGroupSourceV1::default();
            let mut g = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainTraceProviderSetV1::new_v1(
                    &layout,
                    vec![
                        MainTraceGroupProviderV1::TestLog5(&mut a),
                        MainTraceGroupProviderV1::TestLog8(&mut b),
                        MainTraceGroupProviderV1::Log15(&mut c),
                        MainTraceGroupProviderV1::TestLog16(&mut d),
                        MainTraceGroupProviderV1::Log18(&mut e),
                        MainTraceGroupProviderV1::Log19(&mut f),
                        MainTraceGroupProviderV1::Log19(&mut g),
                    ],
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        {
            let mut changed = layout.clone();
            changed.common_lde_log2 -= 1;
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            let mut f = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainTraceProviderSetV1::new_v1(
                    &changed,
                    mock_main_group_providers_v1([&mut a, &mut b, &mut c, &mut d, &mut e, &mut f,]),
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        {
            let mut a = MockMainTraceGroupSourceV1::default();
            let mut b = MockMainTraceGroupSourceV1::default();
            let mut c = MockMainTraceGroupSourceV1::default();
            let mut d = MockMainTraceGroupSourceV1::default();
            let mut e = MockMainTraceGroupSourceV1::default();
            let mut f = MockMainTraceGroupSourceV1::default();
            assert!(matches!(
                MainOpenedProviderSetV1::new_v1(
                    &layout,
                    vec![
                        MainOpenedGroupProviderV1::TestLog5(&mut a),
                        MainOpenedGroupProviderV1::TestLog8(&mut b),
                        MainOpenedGroupProviderV1::TestLog16(&mut c),
                        MainOpenedGroupProviderV1::TestLog15(&mut d),
                        MainOpenedGroupProviderV1::TestLog18(&mut e),
                        MainOpenedGroupProviderV1::TestLog19(&mut f),
                    ],
                ),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
    }

    #[test]
    fn main_provider_routes_every_group_column_through_verifier_owned_slices() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log8 = MockMainTraceGroupSourceV1::default();
        let mut log15 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log18 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let mut providers = MainTraceProviderSetV1::new_v1(
            &layout,
            mock_main_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("exact MAIN providers");

        for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            for kind in [MainTraceColumnKindV1::Base, MainTraceColumnKindV1::Aux] {
                let width = match kind {
                    MainTraceColumnKindV1::Base => layout.trace_groups[group_index].base_width,
                    MainTraceColumnKindV1::Aux => layout.trace_groups[group_index].aux_width,
                };
                if width == 0 {
                    assert!(
                        providers
                            .native_group_column_v1(group_index, kind, 0)
                            .is_err()
                    );
                    continue;
                }
                for column_index in [0, width - 1] {
                    let (registration, local_column) = providers
                        .registered_column_v1(group_index, kind, column_index)
                        .expect("verifier-owned slice");
                    let column = providers
                        .native_group_column_v1(group_index, kind, column_index)
                        .expect("routed native column");
                    assert_eq!(column.len(), registration.segment.trace_size());
                    assert_eq!(
                        column[0],
                        mock_main_column_value_v1(
                            registration,
                            local_column,
                            0,
                            matches!(kind, MainTraceColumnKindV1::Aux),
                        )
                    );
                    assert_eq!(
                        *column.last().expect("nonempty native column"),
                        mock_main_column_value_v1(
                            registration,
                            local_column,
                            registration.segment.trace_size() - 1,
                            matches!(kind, MainTraceColumnKindV1::Aux),
                        )
                    );
                }
            }
        }
        assert!(
            providers
                .native_group_column_v1(
                    FULL_PROFILE_TRACE_GROUPS_V1,
                    MainTraceColumnKindV1::Base,
                    0,
                )
                .is_err()
        );
        assert!(
            providers
                .native_group_column_v1(
                    0,
                    MainTraceColumnKindV1::Base,
                    layout.trace_groups[0].base_width,
                )
                .is_err()
        );

        drop(providers);
        log19.short_base_column = true;
        let mut providers = MainTraceProviderSetV1::new_v1(
            &layout,
            mock_main_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("short-column adversary registry");
        assert!(
            providers
                .native_group_column_v1(5, MainTraceColumnKindV1::Base, 0)
                .is_err()
        );

        drop(providers);
        log19.short_base_column = false;
        log19.noncanonical_aux_column = true;
        let mut providers = MainTraceProviderSetV1::new_v1(
            &layout,
            mock_main_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("noncanonical-column adversary registry");
        assert!(
            providers
                .native_group_column_v1(5, MainTraceColumnKindV1::Aux, 0)
                .is_err()
        );
    }

    #[test]
    fn main_projection_source_matches_every_direct_native_column() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        let (statement, witness) =
            crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let trace =
            build_zk_x509_projection_trace_v1(&statement, &witness).expect("projection trace");
        let post_base = projection_provider_post_base_v1(&statement);
        let direct_aux = build_zk_x509_projection_aux_trace_v1(
            &trace.base,
            &trace.fixed,
            post_base.projection(),
        )
        .expect("direct projection aux");
        let mut projection =
            MainProjectionTraceGroupSourceV1::for_main_v1(&layout, &statement, &trace)
                .expect("production projection source");

        assert!(
            projection.native_aux_column_v1(registration, 0).is_err(),
            "auxiliary columns must remain unavailable before challenge binding"
        );
        for local_column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
            assert_eq!(
                projection
                    .native_base_column_v1(registration, local_column)
                    .expect("production base column"),
                trace
                    .base
                    .rows
                    .iter()
                    .map(|row| row[local_column])
                    .collect::<Vec<_>>(),
                "base column {local_column}"
            );
        }
        projection
            .bind_challenges_v1(post_base)
            .expect("bind projection challenges");
        for local_column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
            assert_eq!(
                projection
                    .native_aux_column_v1(registration, local_column)
                    .expect("production aux column"),
                direct_aux
                    .rows
                    .iter()
                    .map(|row| row[local_column])
                    .collect::<Vec<_>>(),
                "aux column {local_column}"
            );
        }
        assert!(projection.bind_challenges_v1(post_base).is_err());
        assert!(
            projection
                .native_base_column_v1(registration, ZK_X509_PROJECTION_BASE_WIDTH_V1)
                .is_err()
        );
        assert!(
            projection
                .native_aux_column_v1(registration, ZK_X509_PROJECTION_AUX_WIDTH_V1)
                .is_err()
        );
        let io_registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("I/O registration");
        assert!(
            projection
                .native_base_column_v1(io_registration, 0)
                .is_err()
        );

        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log8 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log18 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let mut providers = MainTraceProviderSetV1::new_v1(
            &layout,
            vec![
                MainTraceGroupProviderV1::TestLog5(&mut log5),
                MainTraceGroupProviderV1::TestLog8(&mut log8),
                MainTraceGroupProviderV1::Log15(&mut projection),
                MainTraceGroupProviderV1::TestLog16(&mut log16),
                MainTraceGroupProviderV1::Log18(&mut log18),
                MainTraceGroupProviderV1::Log19(&mut log19),
            ],
        )
        .expect("production projection registry");
        for local_column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
            assert_eq!(
                providers
                    .native_group_column_v1(
                        registration.trace_group,
                        MainTraceColumnKindV1::Base,
                        registration.base_start + local_column,
                    )
                    .expect("routed production base column"),
                trace
                    .base
                    .rows
                    .iter()
                    .map(|row| row[local_column])
                    .collect::<Vec<_>>()
            );
        }
        for local_column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
            assert_eq!(
                providers
                    .native_group_column_v1(
                        registration.trace_group,
                        MainTraceColumnKindV1::Aux,
                        registration.aux_start + local_column,
                    )
                    .expect("routed production aux column"),
                direct_aux
                    .rows
                    .iter()
                    .map(|row| row[local_column])
                    .collect::<Vec<_>>()
            );
        }
        drop(providers);
        assert!(
            projection
                .aux
                .as_ref()
                .is_some_and(|aux| { aux.rows.iter().flatten().any(|value| *value != F::ZERO) }),
            "the source holds challenge-bound auxiliary witness material"
        );
        projection.zeroize_private_buffers_v1();
        assert!(
            projection.aux.is_none(),
            "challenge-bound auxiliary copies are zeroized before release"
        );

        let mut changed_trace = trace.clone();
        changed_trace.fixed.rows.swap(0, 1);
        assert!(
            MainProjectionTraceGroupSourceV1::for_main_v1(&layout, &statement, &changed_trace)
                .is_err(),
            "prover-native fixed material must equal the verifier compiler"
        );
    }

    #[test]
    fn main_projection_fixed_rows_and_residues_match_direct_common_domain_adapter() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let post_base = projection_provider_post_base_v1(&statement);
        let challenges = post_base.projection();
        let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(&statement)
            .expect("verifier projection fixed rows");
        let mut fixed_coefficients =
            transpose_array_rows_v1(&fixed_rows).expect("fixed native columns");
        let trace_root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
            .expect("projection native root");
        for coefficients in &mut fixed_coefficients {
            goldilocks_ifft_v1(coefficients, trace_root).expect("fixed interpolation");
        }
        let common_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common LDE root");
        let evaluate_fixed = |index: usize| {
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(index as u128));
            fixed_coefficients
                .iter()
                .map(|coefficients| {
                    coefficients
                        .iter()
                        .rev()
                        .fold(F::ZERO, |value, coefficient| value.mul(x).add(*coefficient))
                })
                .collect::<Vec<_>>()
        };
        let mut source =
            MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("verifier-safe projection source");
        let next_stride = layout.trace_groups[registration.trace_group]
            .next_stride(layout.common_lde_log2)
            .expect("projection next stride");
        for query_index in [
            0,
            1,
            (1_usize << registration.segment.lde_log2) - 1,
            layout.common_lde_size() - 1,
        ] {
            let next_query_index = (query_index + next_stride) % layout.common_lde_size();
            let fixed = source
                .verifier_fixed_opening_v1(registration, query_index, next_query_index)
                .expect("sampled projection fixed rows");
            let expected_current = evaluate_fixed(query_index);
            let expected_next = evaluate_fixed(next_query_index);
            assert_eq!(fixed.current.as_slice(), expected_current.as_slice());
            assert_eq!(fixed.next.as_slice(), expected_next.as_slice());

            let base_current = (0..registration.segment.base_width)
                .map(|column| F(u64::try_from(column + 3).expect("small base")))
                .collect::<Vec<_>>();
            let base_next = (0..registration.segment.base_width)
                .map(|column| F(u64::try_from(column + 37).expect("small base")))
                .collect::<Vec<_>>();
            let aux_current = (0..registration.segment.aux_width)
                .map(|column| F(u64::try_from(column + 71).expect("small aux")))
                .collect::<Vec<_>>();
            let aux_next = (0..registration.segment.aux_width)
                .map(|column| F(u64::try_from(column + 109).expect("small aux")))
                .collect::<Vec<_>>();
            let opening = RegisteredOpenedRowsV1 {
                base_current: &base_current,
                base_next: &base_next,
                aux_current: &aux_current,
                aux_next: &aux_next,
            };
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
            let production = source
                .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
                .expect("production projection residues");
            let direct = projection_constraint_residues_v1(
                &base_current,
                &base_next,
                &aux_current,
                &aux_next,
                &fixed.current,
                challenges,
            )
            .expect("direct projection residues");
            assert_eq!(production, direct);

            assert!(
                source
                    .constraint_residues_v1(
                        registration,
                        query_index,
                        next_query_index,
                        x.add(F::ONE),
                        opening,
                    )
                    .is_err(),
                "query coordinate and common-domain point cannot diverge"
            );
        }

        assert!(
            source
                .verifier_fixed_opening_v1(registration, layout.common_lde_size(), 0,)
                .is_err()
        );
        assert!(
            source
                .verifier_fixed_opening_v1(registration, 0, next_stride + 1)
                .is_err(),
            "the verifier-derived next stride cannot be caller-selected"
        );
        let io_registration = layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .expect("I/O registration");
        assert!(
            source
                .verifier_fixed_opening_v1(io_registration, 0, next_stride)
                .is_err()
        );

        let wrap_query = layout.common_lde_size() - next_stride;
        let wrap_fixed = source
            .verifier_fixed_opening_v1(registration, wrap_query, 0)
            .expect("wrapped verifier fixed rows");
        assert_eq!(wrap_fixed.next_query_index, 0);
        let expected_wrapped_next = evaluate_fixed(0);
        assert_eq!(wrap_fixed.next.as_slice(), expected_wrapped_next.as_slice());

        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let x = F(GOLDILOCKS_GENERATOR_V1);
        let cache_len = source.fixed_openings.len();
        let unseen_query = 73;
        let unseen_next = unseen_query + next_stride;
        assert!(
            source
                .constraint_residues_v1(registration, unseen_query, unseen_next, x, opening,)
                .is_err(),
            "a mismatched common-domain point is rejected before fixed sampling"
        );
        let malformed_query = unseen_query + 1;
        let malformed_next = malformed_query + next_stride;
        let malformed_x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(malformed_query as u128));
        let short_base = vec![F::ZERO; registration.segment.base_width - 1];
        let malformed_opening = RegisteredOpenedRowsV1 {
            base_current: &short_base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    malformed_query,
                    malformed_next,
                    malformed_x,
                    malformed_opening,
                )
                .is_err(),
            "malformed opened-row widths are rejected before fixed sampling"
        );
        assert!(
            source
                .constraint_residues_v1(io_registration, 0, next_stride, x, opening,)
                .is_err(),
            "the combined production route rejects a caller-spliced registration"
        );
        assert!(
            source
                .constraint_residues_v1(registration, 0, next_stride + 1, x, opening,)
                .is_err(),
            "the combined production route derives, rather than accepts, the next query"
        );
        assert!(
            source
                .constraint_residues_v1(registration, layout.common_lde_size(), 0, x, opening,)
                .is_err(),
            "the combined production route rejects an out-of-domain query"
        );
        assert_eq!(
            source.fixed_openings.len(),
            cache_len,
            "invalid points, openings, registrations, and queries cannot mutate the verifier cache"
        );
        for mutate_next in [false, true] {
            for column in 0..ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1 {
                let mut changed_fixed = source
                    .verifier_fixed_opening_v1(registration, 0, next_stride)
                    .expect("opaque fixed rows");
                let row = if mutate_next {
                    &mut changed_fixed.next
                } else {
                    &mut changed_fixed.current
                };
                row[column] = row[column].add(F::ONE);
                assert!(
                    source
                        .constraint_residues_from_fixed_opening_v1(
                            registration,
                            0,
                            next_stride,
                            x,
                            opening,
                            &changed_fixed,
                        )
                        .is_err(),
                    "{} fixed column {column} substitution must fail closed",
                    if mutate_next { "next" } else { "current" }
                );
            }
        }

        let mut changed_registration = source
            .verifier_fixed_opening_v1(registration, 0, next_stride)
            .expect("opaque fixed rows");
        changed_registration.registration = io_registration;
        assert!(
            source
                .constraint_residues_from_fixed_opening_v1(
                    registration,
                    0,
                    next_stride,
                    x,
                    opening,
                    &changed_registration,
                )
                .is_err(),
            "a fixed opening is registration-tagged"
        );
        let mut changed_query = source
            .verifier_fixed_opening_v1(registration, 0, next_stride)
            .expect("opaque fixed rows");
        changed_query.query_index = 1;
        assert!(
            source
                .constraint_residues_from_fixed_opening_v1(
                    registration,
                    0,
                    next_stride,
                    x,
                    opening,
                    &changed_query,
                )
                .is_err(),
            "a fixed opening is query-tagged"
        );
    }

    #[test]
    fn main_projection_verifier_cache_reuses_caps_and_rejects_the_117th_opening() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let post_base = projection_provider_post_base_v1(&statement);
        let mut source =
            MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("verifier projection source");
        let next_stride = layout.trace_groups[registration.trace_group]
            .next_stride(layout.common_lde_log2)
            .expect("projection next stride");

        let first = source
            .verifier_fixed_opening_v1(registration, 0, next_stride)
            .expect("first two sampled openings");
        assert_eq!(source.fixed_openings.len(), 2);
        let repeated = source
            .verifier_fixed_opening_v1(registration, 0, next_stride)
            .expect("cache reuse");
        assert_eq!(source.fixed_openings.len(), 2);
        assert_eq!(first.current, repeated.current);
        assert_eq!(first.next, repeated.next);

        let mut candidate = 1_usize;
        while source.fixed_openings.len() < VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
            if candidate != next_stride * 2 {
                source
                    .fixed_openings
                    .entry(candidate)
                    .or_insert([F::ZERO; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1]);
            }
            candidate += 1;
        }
        assert_eq!(
            source.fixed_openings.len(),
            VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
        );
        assert!(source.fixed_openings.contains_key(&next_stride));
        assert!(!source.fixed_openings.contains_key(&(next_stride * 2)));
        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let common_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(next_stride as u128));
        assert!(
            source
                .constraint_residues_v1(registration, next_stride, next_stride * 2, x, opening,)
                .is_err(),
            "one new index beyond the exact 116-opening bound must fail before sampling"
        );
        assert_eq!(
            source.fixed_openings.len(),
            VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1,
            "a rejected opening cannot mutate the verifier cache"
        );
    }

    #[test]
    fn main_projection_concrete_verifier_routes_through_the_closed_provider_set() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let post_base = projection_provider_post_base_v1(&statement);
        let mut projection =
            MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("concrete projection verifier");
        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log8 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log18 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let next_stride = layout.trace_groups[registration.trace_group]
            .next_stride(layout.common_lde_log2)
            .expect("projection next stride");
        let query_index = 17;
        let next_query_index = query_index + next_stride;
        let common_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            vec![
                MainOpenedGroupProviderV1::TestLog5(&mut log5),
                MainOpenedGroupProviderV1::TestLog8(&mut log8),
                MainOpenedGroupProviderV1::Projection(&mut projection),
                MainOpenedGroupProviderV1::TestLog16(&mut log16),
                MainOpenedGroupProviderV1::TestLog18(&mut log18),
                MainOpenedGroupProviderV1::TestLog19(&mut log19),
            ],
        )
        .expect("closed provider set with concrete projection verifier");
        let residues = providers
            .registered_constraint_residues_v1(
                registration,
                query_index,
                next_query_index,
                x,
                opening,
            )
            .expect("projection routed through concrete verifier variant");
        assert_eq!(residues.len(), registration.segment.constraint_count);
        drop(providers);
        assert_eq!(
            projection.fixed_openings.len(),
            2,
            "provider-set routing must mint verifier-owned fixed openings"
        );

        let mut forged = registration;
        forged.base_start += 1;
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            vec![
                MainOpenedGroupProviderV1::TestLog5(&mut log5),
                MainOpenedGroupProviderV1::TestLog8(&mut log8),
                MainOpenedGroupProviderV1::Projection(&mut projection),
                MainOpenedGroupProviderV1::TestLog16(&mut log16),
                MainOpenedGroupProviderV1::TestLog18(&mut log18),
                MainOpenedGroupProviderV1::TestLog19(&mut log19),
            ],
        )
        .expect("closed provider set");
        assert!(
            providers
                .registered_constraint_residues_v1(
                    forged,
                    query_index,
                    next_query_index,
                    x,
                    opening,
                )
                .is_err(),
            "caller-spliced registration slices fail before provider dispatch"
        );
    }

    #[test]
    fn main_projection_prover_streams_fixed_polynomials_without_verifier_cache() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let registration = layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .expect("projection registration");
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let post_base = projection_provider_post_base_v1(&statement);
        let source =
            MainProjectionProverConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
                .expect("streaming projection prover source");
        let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(&statement)
            .expect("projection fixed rows");
        let mut expected = transpose_array_rows_v1(&fixed_rows).expect("fixed columns");
        let trace_root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
            .expect("projection native root");
        for coefficients in &mut expected {
            goldilocks_ifft_v1(coefficients, trace_root).expect("fixed interpolation");
        }
        let mut seen = 0_usize;
        source
            .stream_fixed_polynomials_v1(|column, coefficients| {
                assert_eq!(coefficients, expected[column].as_slice());
                seen += 1;
                Ok(())
            })
            .expect("one-at-a-time fixed polynomial stream");
        assert_eq!(seen, ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1);

        let base = vec![F::ZERO; registration.segment.base_width];
        let aux = vec![F::ZERO; registration.segment.aux_width];
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base,
            base_next: &base,
            aux_current: &aux,
            aux_next: &aux,
        };
        let fixed = fixed_rows[0];
        let alphas = vec![E::ONE; registration.segment.constraint_count];
        for _ in 0..=VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
            source
                .composition_value_v1(
                    registration,
                    F(GOLDILOCKS_GENERATOR_V1),
                    opening,
                    &fixed,
                    &alphas,
                )
                .expect("prover composition is independent of verifier cache capacity");
        }

        let mut copied =
            copied_array_column_v1(&fixed_rows, 0).expect("zeroizing copied fixed column");
        assert!(copied.iter().any(|value| *value != F::ZERO));
        copied.zeroize_private_v1();
        assert!(copied.is_empty());
    }

    #[test]
    fn main_composition_and_opened_evaluator_share_one_exact_checked_path() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let trace_groups = mock_main_opened_groups_v1(&layout);
        let alphas = mock_main_alphas_v1(&layout);
        let mixes = mock_main_mixes_v1(&layout);
        let composition_chunks = (0..COMPOSITION_DEGREE_CHUNKS)
            .map(|index| E::from_base(F(u64::try_from(index + 71).expect("small chunk"))))
            .collect::<Vec<_>>();
        let query_index = 123_456;
        let lane = 0;

        let mut log5 = MockMainTraceGroupSourceV1::default();
        let mut log8 = MockMainTraceGroupSourceV1::default();
        let mut log15 = MockMainTraceGroupSourceV1::default();
        let mut log16 = MockMainTraceGroupSourceV1::default();
        let mut log18 = MockMainTraceGroupSourceV1::default();
        let mut log19 = MockMainTraceGroupSourceV1::default();
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            mock_main_opened_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("exact MAIN providers");
        let direct = main_opened_composition_value_v1(
            &mut providers,
            query_index,
            lane,
            &trace_groups,
            &alphas,
        )
        .expect("direct MAIN row composition");

        let lde_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("canonical MAIN LDE root");
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(query_index as u128));
        let independently_expected = layout
            .registered_segments
            .iter()
            .copied()
            .enumerate()
            .try_fold(E::ZERO, |sum, (index, registration)| {
                let opening = registered_opened_rows_v1(&layout, registration, &trace_groups)
                    .expect("registered opening slice");
                let next_stride = layout.trace_groups[registration.trace_group]
                    .next_stride(layout.common_lde_log2)
                    .expect("registered next stride");
                let next_query_index = (query_index + next_stride) % layout.common_lde_size();
                let fixed = MainFixedOpenedRowsV1 {
                    current: mock_main_fixed_row_v1(registration, query_index),
                    next: mock_main_fixed_row_v1(registration, next_query_index),
                };
                let residue =
                    mock_main_residue_value_v1(registration, query_index, x, opening, &fixed);
                accumulator_quotient_value_v1(
                    registration.segment,
                    x,
                    &vec![residue; registration.segment.constraint_count],
                    &alphas[index][lane],
                )
                .map(|value| sum.add(value))
            })
            .expect("independent registered quotient sum");
        assert_eq!(direct, independently_expected);

        let evaluated = {
            let mut evaluator = MainOpenedRowEvaluatorV1 {
                providers: &mut providers,
                alphas: &alphas,
                mixes: &mixes,
            };
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks,
            )
            .expect("MAIN opened evaluation")
        };
        assert_eq!(evaluated.composition, direct);
        let mut expected_fri = E::ZERO;
        for (group, lanes) in trace_groups.iter().zip(&mixes) {
            let mix = &lanes[lane];
            expected_fri = expected_fri.add(
                group
                    .base_current
                    .iter()
                    .zip(&mix.base)
                    .fold(E::ZERO, |sum, (value, coefficient)| {
                        sum.add(coefficient.mul_base(*value))
                    }),
            );
            expected_fri = expected_fri.add(
                group
                    .aux_current
                    .iter()
                    .zip(&mix.aux)
                    .fold(E::ZERO, |sum, (value, coefficient)| {
                        sum.add(coefficient.mul_base(*value))
                    }),
            );
        }
        expected_fri = expected_fri.add(
            mix_opened_composition_chunks_v1(&composition_chunks, &mixes[0][lane])
                .expect("composition mix"),
        );
        assert_eq!(evaluated.fri_base, expected_fri);

        let mut changed_rows = trace_groups.clone();
        changed_rows[0].base_current[0] = changed_rows[0].base_current[0].add(F::ONE);
        assert_ne!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &changed_rows,
                &alphas,
            )
            .expect("semantic row mutation"),
            direct
        );

        let mut short_rows = trace_groups.clone();
        short_rows[0].base_current.pop();
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &short_rows,
                &alphas,
            )
            .is_err()
        );
        let mut noncanonical_rows = trace_groups.clone();
        noncanonical_rows[0].base_current[0] =
            F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &noncanonical_rows,
                &alphas,
            )
            .is_err()
        );
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups[..FULL_PROFILE_TRACE_GROUPS_V1 - 1],
                &alphas,
            )
            .is_err()
        );
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                layout.common_lde_size(),
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                SECURITY_LANES,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
        let mut short_alphas = alphas.clone();
        short_alphas[0][lane].pop();
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &short_alphas,
            )
            .is_err()
        );
        let mut short_alpha_lanes = alphas.clone();
        short_alpha_lanes[0].pop();
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &short_alpha_lanes,
            )
            .is_err()
        );

        let mut short_mix = mixes.clone();
        short_mix[0][lane].base.pop();
        let mut evaluator = MainOpenedRowEvaluatorV1 {
            providers: &mut providers,
            alphas: &alphas,
            mixes: &short_mix,
        };
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks,
            )
            .is_err()
        );
        drop(evaluator);
        let mut short_mix_lanes = mixes.clone();
        short_mix_lanes[0].pop();
        let mut evaluator = MainOpenedRowEvaluatorV1 {
            providers: &mut providers,
            alphas: &alphas,
            mixes: &short_mix_lanes,
        };
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks,
            )
            .is_err()
        );
        drop(evaluator);
        let mut malformed_layout = layout.clone();
        malformed_layout.trace_groups.clear();
        assert!(validate_main_fri_mixes_v1(&malformed_layout, &[]).is_err());
        let mut inconsistent_mix = mixes.clone();
        inconsistent_mix[1][lane].composition[0] =
            inconsistent_mix[1][lane].composition[0].add(E::ONE);
        let mut evaluator = MainOpenedRowEvaluatorV1 {
            providers: &mut providers,
            alphas: &alphas,
            mixes: &inconsistent_mix,
        };
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks,
            )
            .is_err()
        );
        drop(evaluator);
        let mut evaluator = MainOpenedRowEvaluatorV1 {
            providers: &mut providers,
            alphas: &alphas,
            mixes: &mixes,
        };
        assert!(
            aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
                &mut evaluator,
                query_index,
                lane,
                &trace_groups,
                &composition_chunks[..COMPOSITION_DEGREE_CHUNKS - 1],
            )
            .is_err()
        );
        drop(evaluator);

        drop(providers);
        log5.short_residues = true;
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            mock_main_opened_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("short-residue adversary registry");
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );

        drop(providers);
        log5.short_residues = false;
        log5.noncanonical_residues = true;
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            mock_main_opened_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("noncanonical-residue adversary registry");
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );

        drop(providers);
        log5.noncanonical_residues = false;
        log5.short_fixed_row = true;
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            mock_main_opened_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("short-fixed-row adversary registry");
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );

        drop(providers);
        log5.short_fixed_row = false;
        log5.noncanonical_fixed_row = true;
        let mut providers = MainOpenedProviderSetV1::new_v1(
            &layout,
            mock_main_opened_group_providers_v1([
                &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
            ]),
        )
        .expect("noncanonical-fixed-row adversary registry");
        assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
    }

    #[test]
    fn main_base_commitment_session_mints_pre_aux_only_after_canonical_six_group_chronology() {
        fn root(group: usize) -> [u8; 32] {
            [u8::try_from(0x41 + group).expect("six groups"); 32]
        }

        let mut session = main_base_commitment_session_fixture_v1();
        let streamed = aggregate::StreamingRowCommitmentResultV1 {
            commitment: aggregate::StreamingMerkleCommitmentV1 {
                root: root(0),
                frontier: Vec::new(),
            },
            opened_rows: std::collections::BTreeMap::new(),
        };
        session
            .accept_streaming_base_commitment_v1(0, &streamed)
            .expect("derive log5 root from streamed commitment");
        for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
            .into_iter()
            .enumerate()
            .skip(1)
        {
            session
                .accept_base_root_v1(group, native_log, root(group))
                .expect("canonical ordered base root");
        }
        assert_eq!(session.next_group, FULL_PROFILE_TRACE_GROUPS_V1);
        let pre_aux = session
            .finish_pre_aux_v1()
            .expect("completed session mints pre-aux state");
        assert_eq!(pre_aux.consensus_context_digest_for_test_v1(), [0xB1; 32]);
        assert_eq!(
            pre_aux.main_profile_digest_for_test_v1(),
            TEST_COMPILED_PROFILE_DIGEST_V1
        );
        assert_eq!(
            pre_aux.main_base_roots_for_test_v1(),
            core::array::from_fn(|index| root(index))
        );
        derive_zk_x509_credential_pre_aux_binding_v1(pre_aux, [0xC1; 32], [0xD1; 32], [0xE1; 32])
            .expect("X5B1 begins only from session-minted MAIN pre-aux");
    }

    #[test]
    fn main_base_commitment_session_rejects_omission_reorder_duplicate_wrong_log_zero_and_excess() {
        fn root(group: usize) -> [u8; 32] {
            [u8::try_from(group + 1).expect("six groups"); 32]
        }

        for omitted_after in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            let mut session = main_base_commitment_session_fixture_v1();
            for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
                .into_iter()
                .enumerate()
                .take(omitted_after)
            {
                session
                    .accept_base_root_v1(group, native_log, root(group))
                    .expect("canonical prefix");
            }
            assert!(
                session.complete_v1().is_err(),
                "a {omitted_after}-root prefix must not complete"
            );
        }

        let mut zero = main_base_commitment_session_fixture_v1();
        assert!(matches!(
            zero.accept_base_root_v1(0, 5, [0_u8; 32]),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(zero.next_group, 0);
        assert_eq!(
            zero.roots,
            [[0_u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1]
        );
        assert_eq!(
            zero.recorded,
            [false; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1]
        );
        let zero_streamed = aggregate::StreamingRowCommitmentResultV1 {
            commitment: aggregate::StreamingMerkleCommitmentV1 {
                root: [0_u8; 32],
                frontier: Vec::new(),
            },
            opened_rows: std::collections::BTreeMap::new(),
        };
        assert!(matches!(
            zero.accept_streaming_base_commitment_v1(0, &zero_streamed),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(zero.next_group, 0);

        let mut session = main_base_commitment_session_fixture_v1();
        assert!(matches!(
            session.accept_base_root_v1(1, 8, root(1)),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(session.next_group, 0);
        assert!(matches!(
            session.accept_base_root_v1(0, 8, root(0)),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(session.next_group, 0);
        session
            .accept_base_root_v1(0, 5, root(0))
            .expect("canonical log5 root");
        assert!(matches!(
            session.accept_base_root_v1(0, 5, root(0)),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert!(matches!(
            session.accept_base_root_v1(2, 15, root(2)),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(session.next_group, 1);
        for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
            .into_iter()
            .enumerate()
            .skip(1)
        {
            session
                .accept_base_root_v1(group, native_log, root(group))
                .expect("remaining canonical roots");
        }
        assert!(matches!(
            session.accept_base_root_v1(FULL_PROFILE_TRACE_GROUPS_V1, 20, [0xFF; 32],),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        session
            .complete_v1()
            .expect("exactly six roots complete once");
    }

    #[test]
    fn main_base_commitment_session_rejects_wrong_layout_profile_count_and_internal_state_tampering()
     {
        fn group(root: [u8; 32]) -> TraceGroupProofV1 {
            TraceGroupProofV1 {
                base_root: root,
                aux_root: [0_u8; 32],
                base_frontier: Vec::new(),
                aux_frontier: Vec::new(),
            }
        }

        let unpinned_profile = unpinned_main_verifier_profile_fixture_v1();
        let canonical_layout =
            AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        assert!(matches!(
            ZkX509MainBaseCommitmentSessionV1::new_v1(
                &canonical_layout,
                [0xB1; 32],
                unpinned_profile,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));

        let isolated = AggregateProofLayoutV1::for_segments(&[
            SegmentLayoutV1::for_full_io().expect("full I/O")
        ])
        .expect("isolated I/O layout");
        assert!(matches!(
            ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
                &isolated,
                [0xB1; 32],
                TEST_COMPILED_PROFILE_DIGEST_V1,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));

        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let mut wrong_layout = layout.clone();
        wrong_layout.trace_groups.swap(0, 1);
        assert!(matches!(
            ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
                &wrong_layout,
                [0xB1; 32],
                TEST_COMPILED_PROFILE_DIGEST_V1,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
                &layout,
                [0_u8; 32],
                TEST_COMPILED_PROFILE_DIGEST_V1,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
                &layout, [0xB1; 32], [0_u8; 32],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));

        let canonical_groups = (0..FULL_PROFILE_TRACE_GROUPS_V1)
            .map(|index| group([u8::try_from(index + 1).expect("six groups"); 32]))
            .collect::<Vec<_>>();
        for wrong_count in [
            FULL_PROFILE_TRACE_GROUPS_V1 - 1,
            FULL_PROFILE_TRACE_GROUPS_V1 + 1,
        ] {
            let mut groups = canonical_groups.clone();
            groups.resize(wrong_count, group([0xFF; 32]));
            let mut session = main_base_commitment_session_fixture_v1();
            assert!(matches!(
                session.accept_decoded_base_groups_v1(&groups),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            ));
            assert_eq!(session.next_group, 0);
        }
        for zero_at in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            let mut groups = canonical_groups.clone();
            groups[zero_at].base_root = [0_u8; 32];
            let mut session = main_base_commitment_session_fixture_v1();
            assert!(matches!(
                session.accept_decoded_base_groups_v1(&groups),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            ));
            assert_eq!(session.next_group, 0);
            assert_eq!(
                session.roots, [[0_u8; 32]; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
                "decoded zero sentinel at group {zero_at} must fail transactionally"
            );
        }
        let mut session = main_base_commitment_session_fixture_v1();
        session
            .accept_decoded_base_groups_v1(&canonical_groups)
            .expect("exact decoded six-group roots");
        session
            .finish_pre_aux_v1()
            .expect("decoded canonical roots mint pre-aux");

        let mut partial = main_base_commitment_session_fixture_v1();
        partial
            .accept_base_root_v1(0, 5, [0x11; 32])
            .expect("first root");
        assert!(matches!(
            partial.accept_decoded_base_groups_v1(&canonical_groups),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));

        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.recorded[0] = true;
        assert!(matches!(
            corrupted.accept_base_root_v1(0, 5, [0x11; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.roots[0] = [0x11; 32];
        assert!(matches!(
            corrupted.accept_base_root_v1(0, 5, [0x22; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.next_group = 1;
        assert!(matches!(
            corrupted.complete_v1(),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.recorded[0] = true;
        corrupted.next_group = 1;
        assert!(matches!(
            corrupted.complete_v1(),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.consensus_context_digest = [0_u8; 32];
        assert!(matches!(
            corrupted.accept_base_root_v1(0, 5, [0x11; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.main_profile_digest = [0_u8; 32];
        assert!(matches!(
            corrupted.accept_base_root_v1(0, 5, [0x11; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut corrupted = main_base_commitment_session_fixture_v1();
        corrupted.layout.trace_groups.swap(0, 1);
        assert!(matches!(
            corrupted.accept_base_root_v1(0, 5, [0x11; 32]),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }

    #[test]
    fn main_trace_phase_root_recorder_rejects_every_reorder_duplicate_omission_zero_and_excess() {
        fn commitment(seed: u8) -> aggregate::StreamingRowCommitmentResultV1 {
            aggregate::StreamingRowCommitmentResultV1 {
                commitment: aggregate::StreamingMerkleCommitmentV1 {
                    root: [seed; 32],
                    frontier: Vec::new(),
                },
                opened_rows: BTreeMap::new(),
            }
        }

        let mut groups = Vec::new();
        assert!(matches!(
            record_main_group_commitment_v1(
                1,
                MainTraceColumnKindV1::Base,
                &commitment(2),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert!(
            groups.is_empty(),
            "a rejected base reorder is transactional"
        );
        assert!(matches!(
            record_main_group_commitment_v1(
                0,
                MainTraceColumnKindV1::Base,
                &commitment(0),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert!(
            groups.is_empty(),
            "the zero-root sentinel cannot mutate state"
        );

        record_main_group_commitment_v1(
            0,
            MainTraceColumnKindV1::Base,
            &commitment(1),
            &mut groups,
        )
        .expect("first canonical base root");
        assert!(matches!(
            record_main_group_commitment_v1(
                0,
                MainTraceColumnKindV1::Base,
                &commitment(7),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert!(matches!(
            record_main_group_commitment_v1(
                0,
                MainTraceColumnKindV1::Aux,
                &commitment(0x41),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(groups.len(), 1, "aux cannot start before all six bases");
        assert_eq!(groups[0].aux_root, [0_u8; 32]);

        for group in 1..FULL_PROFILE_TRACE_GROUPS_V1 {
            record_main_group_commitment_v1(
                group,
                MainTraceColumnKindV1::Base,
                &commitment(u8::try_from(group + 1).expect("six roots")),
                &mut groups,
            )
            .expect("remaining canonical base roots");
        }
        let base_snapshot = groups.clone();
        assert!(matches!(
            record_main_group_commitment_v1(
                1,
                MainTraceColumnKindV1::Aux,
                &commitment(0x42),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(
            groups, base_snapshot,
            "a rejected aux reorder is transactional"
        );
        assert!(matches!(
            record_main_group_commitment_v1(
                0,
                MainTraceColumnKindV1::Aux,
                &commitment(0),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(groups, base_snapshot, "a zero aux root is transactional");

        for group in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            record_main_group_commitment_v1(
                group,
                MainTraceColumnKindV1::Aux,
                &commitment(u8::try_from(0x41 + group).expect("six aux roots")),
                &mut groups,
            )
            .expect("canonical auxiliary root");
            assert!(
                groups[..=group]
                    .iter()
                    .all(|recorded| recorded.aux_root != [0_u8; 32])
            );
            assert!(
                groups[group + 1..]
                    .iter()
                    .all(|pending| pending.aux_root == [0_u8; 32])
            );
        }
        let complete = groups.clone();
        for hostile_group in [0, FULL_PROFILE_TRACE_GROUPS_V1] {
            assert!(matches!(
                record_main_group_commitment_v1(
                    hostile_group,
                    MainTraceColumnKindV1::Aux,
                    &commitment(0xF1),
                    &mut groups,
                ),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            ));
            assert_eq!(groups, complete);
        }
    }

    fn tiny_authenticated_main_polynomial_fixture_v1(
        seed: u8,
    ) -> aggregate::MaskedTracePolynomialSetV1 {
        let mut rng = StdRng::from_seed([seed; 32]);
        let (_, polynomials) = aggregate::commit_masked_trace_polynomial_columns_v1(
            b"iroha:test:zk-x509:main-scratch-leaf:v1",
            b"iroha:test:zk-x509:main-scratch-node:v1",
            usize::from(seed),
            2,
            4,
            1,
            1,
            &[],
            &mut rng,
            |_| Ok(vec![F(u64::from(seed)); 4]),
        )
        .expect("tiny authenticated masked polynomials");
        polynomials
    }

    #[test]
    fn main_polynomial_set_fails_closed_on_count_shape_and_phase_lifecycle() {
        assert!(core::mem::needs_drop::<MainTracePolynomialSetV1>());
        assert!(core::mem::needs_drop::<
            ZkX509MainAwaitingCredentialBindingV1<'static>,
        >());
        assert!(core::mem::needs_drop::<ZkX509MainCompositionPhaseV1<'static>>());

        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        for hostile_count in [
            FULL_PROFILE_TRACE_GROUPS_V1 - 1,
            FULL_PROFILE_TRACE_GROUPS_V1 + 1,
        ] {
            let polynomials = (0..hostile_count)
                .map(|index| {
                    tiny_authenticated_main_polynomial_fixture_v1(
                        u8::try_from(index + 1).expect("small hostile count"),
                    )
                })
                .collect();
            assert!(matches!(
                MainTracePolynomialSetV1::from_ordered_v1(
                    &layout,
                    MainTraceColumnKindV1::Base,
                    polynomials,
                ),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            ));
        }

        // Six authenticated polynomial sets are still rejected when any
        // native/commitment-domain shape is not the verifier-fixed MAIN shape.
        // The consuming constructor owns the vector, so every retained secret
        // coefficient is zeroized on this failure path.
        let wrong_shape = (0..FULL_PROFILE_TRACE_GROUPS_V1)
            .map(|index| {
                tiny_authenticated_main_polynomial_fixture_v1(
                    u8::try_from(index + 0x21).expect("six fixtures"),
                )
            })
            .collect();
        assert!(matches!(
            MainTracePolynomialSetV1::from_ordered_v1(
                &layout,
                MainTraceColumnKindV1::Aux,
                wrong_shape,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
    }

    #[test]
    fn main_phase_source_has_no_root_only_or_reconstruction_commit_path() {
        let source = include_str!("stark/main_aggregate.rs");
        let helper_start = source
            .find("fn commit_main_trace_group_v1")
            .expect("MAIN commitment helper");
        let helper_end = source[helper_start..]
            .find("fn main_trace_group_root_v1")
            .map(|offset| helper_start + offset)
            .expect("MAIN commitment helper end");
        let helper = &source[helper_start..helper_end];
        assert!(
            helper.contains("commit_masked_trace_polynomial_columns_v1"),
            "MAIN commitments must retain the authenticated masked polynomials"
        );
        assert!(
            !helper.contains("commit_masked_trace_columns_v1(")
                && !helper.contains("commit_masked_trace_columns_retaining_encrypted_scratch_v1")
                && !helper.contains("spill_replayed_masked_trace_columns_v1")
                && !helper.contains("replay_masked_trace_columns_via_encrypted_scratch_v1"),
            "MAIN must not discard its committed polynomials or reconstruct them from native witness"
        );

        let phase_start = source
            .find("pub(crate) fn commit_zk_x509_main_base_phase_v1_with_rng")
            .expect("typed MAIN phase one");
        let phase_end = source[phase_start..]
            .find("/// Exact six-provider registry")
            .map(|offset| phase_start + offset)
            .expect("typed MAIN phase end");
        let phases = &source[phase_start..phase_end];
        assert_eq!(
            phases.matches("base_polynomials.push(polynomials)").count(),
            FULL_PROFILE_TRACE_GROUPS_V1,
            "phase one must retain exactly six base polynomial sets"
        );
        assert_eq!(
            phases.matches("aux_polynomials.push(polynomials)").count(),
            FULL_PROFILE_TRACE_GROUPS_V1,
            "phase two must retain exactly six auxiliary polynomial sets"
        );
        let provenance = phases
            .find("matches_main_pre_aux_v1")
            .expect("provenance check");
        let absorption = phases
            .find("absorb_zk_x509_credential_pre_aux_binding_v1")
            .expect("local binding absorption");
        let child_bind = phases
            .find("projection.bind_challenges_v1")
            .expect("child bind");
        assert!(
            provenance < absorption && provenance < child_bind,
            "cross-phase provenance must fail before transcript or child mutation"
        );
    }

    #[test]
    fn retained_registration_plans_are_exact_for_all_49_and_reject_degree_domain_attacks() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        assert_eq!(
            layout.registered_segments.len(),
            FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
        );
        let mut group_counts = [0_usize; FULL_PROFILE_TRACE_GROUPS_V1];
        for registration in layout.registered_segments.iter().copied() {
            let segment = registration.segment;
            let plan = registered_retained_prover_plan_v1(segment, layout.common_lde_log2)
                .expect("canonical registration plan");
            let trace_rows = 1_usize << segment.trace_log2;
            let independently_derived_degree = usize::from(segment.constraint_degree)
                .checked_mul(trace_rows + MASK_DEGREE)
                .and_then(|degree| degree.checked_sub(trace_rows))
                .expect("release dimensions fit usize");
            let independently_derived_rows = independently_derived_degree
                .checked_add(1)
                .and_then(usize::checked_next_power_of_two)
                .expect("release quotient domain");
            assert_eq!(plan.maximum_quotient_degree, independently_derived_degree);
            assert_eq!(plan.quotient_coset_rows, independently_derived_rows);
            assert_eq!(
                plan.quotient_coset_log2,
                u8::try_from(independently_derived_rows.ilog2()).expect("small release log")
            );
            assert_eq!(
                plan.quotient_next_stride,
                independently_derived_rows / trace_rows
            );
            assert!(plan.maximum_quotient_degree < plan.quotient_coset_rows);
            assert!(plan.quotient_coset_log2 <= layout.common_lde_log2);
            group_counts[registration.trace_group] += 1;

            let mut low_degree = segment;
            low_degree.constraint_degree = 1;
            assert!(matches!(
                registered_retained_prover_plan_v1(low_degree, layout.common_lde_log2),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
            let mut excessive_degree = segment;
            excessive_degree.constraint_degree = ZK_X509_MAX_CONSTRAINT_DEGREE_V1 + 1;
            assert!(matches!(
                registered_retained_prover_plan_v1(excessive_degree, layout.common_lde_log2),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
            assert!(matches!(
                registered_retained_prover_plan_v1(segment, segment.lde_log2.saturating_sub(1),),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        assert_eq!(group_counts, [11, 5, 1, 10, 1, 21]);
    }

    fn empty_main_composition_chunks_v1() -> Vec<Vec<Vec<E>>> {
        (0..SECURITY_LANES)
            .map(|_| (0..COMPOSITION_DEGREE_CHUNKS).map(|_| Vec::new()).collect())
            .collect()
    }

    #[test]
    fn main_coefficient_accumulator_is_bounded_transactional_and_order_independent() {
        let coefficient_cap = 8;
        let value = |value| E::from_base(F(value));
        let mut first = empty_main_composition_chunks_v1();
        first[0][0] = vec![value(1), value(2), value(3)];
        first[0][2] = vec![value(9)];
        let mut second = empty_main_composition_chunks_v1();
        second[0][0] = vec![value(4), value(5)];
        second[0][1] = vec![value(7), value(8), value(9), value(10)];
        second[0][2] = vec![value(9).neg()];

        let mut first_then_second = empty_main_composition_chunks_v1();
        add_main_composition_coefficient_chunks_v1(&mut first_then_second, &first, coefficient_cap)
            .expect("first contribution");
        add_main_composition_coefficient_chunks_v1(
            &mut first_then_second,
            &second,
            coefficient_cap,
        )
        .expect("second contribution");
        let mut second_then_first = empty_main_composition_chunks_v1();
        add_main_composition_coefficient_chunks_v1(
            &mut second_then_first,
            &second,
            coefficient_cap,
        )
        .expect("second contribution first");
        add_main_composition_coefficient_chunks_v1(&mut second_then_first, &first, coefficient_cap)
            .expect("first contribution second");
        assert_eq!(first_then_second, second_then_first);
        assert!(
            first_then_second[0][2].is_empty(),
            "exact cancellation must remove the retained tail"
        );

        let canonical = first_then_second.clone();
        let mut wrong_lanes = second.clone();
        wrong_lanes.pop();
        assert!(matches!(
            add_main_composition_coefficient_chunks_v1(
                &mut first_then_second,
                &wrong_lanes,
                coefficient_cap,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert_eq!(first_then_second, canonical);

        let mut wrong_chunks = second.clone();
        wrong_chunks[0].pop();
        assert!(matches!(
            add_main_composition_coefficient_chunks_v1(
                &mut first_then_second,
                &wrong_chunks,
                coefficient_cap,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert_eq!(first_then_second, canonical);

        let mut oversized = second.clone();
        oversized[0][0].resize(coefficient_cap + 1, E::ONE);
        assert!(matches!(
            add_main_composition_coefficient_chunks_v1(
                &mut first_then_second,
                &oversized,
                coefficient_cap,
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert_eq!(first_then_second, canonical);
        assert!(matches!(
            add_main_composition_coefficient_chunks_v1(&mut first_then_second, &second, 0,),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert_eq!(first_then_second, canonical);
    }

    #[test]
    fn composition_chunk_split_rejects_hidden_high_degree_coefficients() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let shared = layout.as_shared().expect("shared MAIN layout");
        let value = |value| E::from_base(F(value));
        let mut coefficients = vec![value(1), value(2), value(3), E::ZERO, E::ZERO, E::ZERO];
        let chunks =
            composition_coefficient_chunks_v1(&coefficients, 2, &shared).expect("zero high tail");
        assert_eq!(chunks.len(), COMPOSITION_DEGREE_CHUNKS);
        assert_eq!(chunks[0], vec![value(1), value(2), value(3)]);
        coefficients[5] = value(1);
        assert!(matches!(
            composition_coefficient_chunks_v1(&coefficients, 2, &shared),
            Err(ZkX509StarkErrorV1::ConstraintOpening)
        ));
        assert!(matches!(
            composition_coefficient_chunks_v1(&coefficients, usize::MAX, &shared),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }

    #[test]
    fn main_finish_verifier_and_consensus_source_use_only_the_closed_release_path() {
        let source = include_str!("stark/main_aggregate.rs");
        let finish_start = source
            .find("pub(crate) fn finish_v1_with_rng")
            .expect("MAIN finish");
        let finish_end = source[finish_start..]
            .find("/// Exact six-provider registry")
            .map(|offset| finish_start + offset)
            .expect("MAIN finish end");
        let finish = &source[finish_start..finish_end];
        assert!(finish.contains("replay_masked_trace_polynomial_columns_v1"));
        assert!(finish.contains("self.composition_material_v1()"));
        let material_start = source
            .find("fn composition_material_v1(&self)")
            .expect("retained composition material");
        let material_end = source[material_start..]
            .find("pub(super) fn record_main_group_commitment_v1")
            .map(|offset| material_start + offset)
            .expect("retained composition material end");
        assert!(
            source[material_start..material_end]
                .contains("main_composition_material_from_polynomials_v1")
        );
        assert!(!finish.contains("verify_opened_query_relations_with_deep_v1"));
        for forbidden in [
            "spill_replayed_masked_trace_columns_v1",
            "replay_masked_trace_columns_via_encrypted_scratch_v1",
            "EncryptedFieldMatrixScratch",
            "native_base_column_v1",
            "native_aux_column_v1",
        ] {
            assert!(
                !finish.contains(forbidden),
                "MAIN finish must not use {forbidden}"
            );
        }

        let verifier_start = source
            .find("pub(crate) fn verify_zk_x509_main_aggregate_stark_v1")
            .expect("complete MAIN verifier");
        let verifier = &source[verifier_start..];
        let grinding = verifier
            .find("verify_grinding_nonce_v1")
            .expect("grinding verification");
        let queries = verifier
            .find("let expected_indices = query_indices_v1")
            .expect("post-grinding queries");
        let fixed = verifier
            .find("derive_zk_x509_main_fixed_openings_after_grinding_v1")
            .expect("verifier-derived fixed openings");
        assert!(grinding < queries && queries < fixed);
        assert_eq!(
            verifier.matches("MainOpenedGroupProviderV1::").count(),
            FULL_PROFILE_TRACE_GROUPS_V1
        );
        assert!(verifier.contains("verify_opened_query_relations_with_deep_v1"));

        let engine = include_str!("engine.rs");
        let engine_production = &engine[..engine
            .find("#[cfg(test)]")
            .expect("engine production/test boundary")];
        assert!(engine_production.contains("verify_zk_x509_main_aggregate_stark_v1"));
        assert!(engine_production.contains("ca_accumulator_subproof_binding_from_proof_v1"));
        assert!(engine_production.contains("validate_cross_subproof_binding_v1"));
        assert!(!engine_production.contains("ConsensusVerifierUnavailable"));
    }

    #[test]
    fn main_local_transcript_separates_binding_before_base_after_aux_and_wrong_outer_root() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
        let base_roots = (0..FULL_PROFILE_TRACE_GROUPS_V1)
            .map(|index| TraceGroupProofV1 {
                base_root: [u8::try_from(index + 1).expect("six roots"); 32],
                aux_root: [u8::try_from(index + 0x41).expect("six roots"); 32],
                base_frontier: Vec::new(),
                aux_frontier: Vec::new(),
            })
            .collect::<Vec<_>>();
        let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x81; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
            base_roots
                .iter()
                .map(|group| group.base_root)
                .collect::<Vec<_>>()
                .try_into()
                .expect("exact six MAIN roots"),
        );
        let binding = derive_zk_x509_credential_pre_aux_binding_v1(
            pre_aux, [0x91; 32], [0xA1; 32], [0xB1; 32],
        )
        .expect("canonical outer binding");
        let mut changed_pre_aux = pre_aux;
        changed_pre_aux.main_base_roots_mut_for_test_v1()[5][0] ^= 1;
        let changed_binding = derive_zk_x509_credential_pre_aux_binding_v1(
            changed_pre_aux,
            [0x91; 32],
            [0xA1; 32],
            [0xB1; 32],
        )
        .expect("binding under a hostile log19 root");
        let claims = main_log19_terminal_claims_fixture_v1();

        let alpha = |order: u8, binding: ZkX509CredentialPreAuxBindingV1| {
            let mut transcript = new_main_transcript_after_profile_validation_v1(
                &[0x81; 32],
                TEST_COMPILED_PROFILE_DIGEST_V1,
            )
            .expect("test MAIN transcript");
            absorb_aggregate_layout_v1(&mut transcript, MAIN_LAYOUT_DOMAIN_V1, &layout)
                .expect("MAIN layout");
            if order == 1 {
                absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                    .expect("premature binding still frames distinctly");
            }
            aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &base_roots)
                .expect("ordered base roots");
            if order == 0 {
                absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                    .expect("canonical binding");
            }
            aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &base_roots)
                .expect("ordered aux roots");
            if order == 2 {
                absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                    .expect("late binding still frames distinctly");
            }
            absorb_zk_x509_main_terminal_claims_v1(&mut transcript, claims)
                .expect("terminal claims");
            derive_constraint_alphas_v1(&mut transcript, &layout).expect("alphas")[0][0][0]
        };

        let canonical = alpha(0, binding);
        assert_ne!(
            canonical,
            alpha(1, binding),
            "binding-before-base must separate"
        );
        assert_ne!(
            canonical,
            alpha(2, binding),
            "binding-after-aux must separate"
        );
        assert_ne!(
            canonical,
            alpha(0, changed_binding),
            "changing only the outer-bound log19 root must separate"
        );
    }

    #[test]
    fn full_profile_layout_is_constant_exact_and_rejects_registration_splices() {
        let sha_layout = AggregateProofLayoutV1::for_equal_log_buckets_v1(
            &canonical_sha_segment_layouts_v1().expect("canonical SHA slices"),
        )
        .expect("canonical SHA-only aggregate layout");
        let sha_shared_layout = sha_layout.as_shared().expect("shared SHA layout");
        assert_eq!(
            maximum_encoded_aggregate_proof_bytes_v1(&sha_layout)
                .expect("maximum SHA-only X5S1 size")
                .checked_sub(
                    aggregate::exact_deep_opening_bytes_v1(
                        AGGREGATE_PARAMETERS_V1,
                        &sha_shared_layout,
                    )
                    .expect("exact SHA DEEP bytes"),
                )
                .expect("SHA pre-DEEP size"),
            ZK_X509_SHA_MAX_ENCODED_PROOF_BYTES_V1,
        );
        let layout = AggregateProofLayoutV1::for_full_profile_v1()
            .expect("canonical fixed-capacity X5S1 layout");
        layout
            .validate_exact_full_profile_registration_v1()
            .expect("exact full registration");
        assert_eq!(
            layout.registered_segments.len(),
            FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
        );
        assert_eq!(layout.trace_groups.len(), FULL_PROFILE_TRACE_GROUPS_V1);
        assert_eq!(
            layout
                .trace_groups
                .iter()
                .map(|group| group.column_chunks)
                .sum::<usize>(),
            FULL_PROFILE_PHYSICAL_CHUNKS_V1
        );
        assert_eq!(layout.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
        assert_eq!(
            layout
                .trace_groups
                .iter()
                .map(|group| group.native_trace_log2)
                .collect::<Vec<_>>(),
            vec![5, 8, 15, 16, 18, 19]
        );
        let maximum_encoded_bytes =
            maximum_encoded_aggregate_proof_bytes_v1(&layout).expect("maximum X5S1 size");
        let main_deep_bytes = aggregate::exact_deep_opening_bytes_v1(
            AGGREGATE_PARAMETERS_V1,
            &layout.as_shared().expect("shared full-profile layout"),
        )
        .expect("exact main DEEP bytes");
        assert_eq!(
            maximum_encoded_bytes
                .checked_sub(main_deep_bytes)
                .expect("main pre-DEEP size"),
            usize::try_from(ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1)
                .expect("profile proof bound fits usize")
        );
        assert!(
            maximum_encoded_bytes
                <= usize::try_from(ZK_X509_MAX_PROOF_BYTES_V1)
                    .expect("consensus proof cap fits usize")
        );
        for signature in 0..P256_SIGNATURE_COUNT_V1 {
            let arithmetic = layout
                .registered_segment(
                    SegmentAdapterIdV1::P256Arithmetic,
                    p256_instance_v1(signature, 0).expect("instance"),
                )
                .expect("one arithmetic registration per signature");
            assert_eq!(
                p256_instance_parts_v1(arithmetic.segment.instance),
                Some((signature, 0))
            );
        }

        let mut mutations = Vec::new();
        let mut changed = layout.clone();
        changed.registered_segments.remove(0);
        mutations.push(changed);
        let mut changed = layout.clone();
        changed
            .registered_segments
            .push(changed.registered_segments[0]);
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments.swap(0, 1);
        mutations.push(changed);
        let mut changed = layout.clone();
        let sha = changed
            .registered_segments
            .iter()
            .position(|registration| {
                registration.segment.adapter == SegmentAdapterIdV1::Sha256CallBus
            })
            .expect("SHA registration");
        changed.registered_segments[sha].segment.instance = 4;
        mutations.push(changed);
        let mut changed = layout.clone();
        let der = changed
            .registered_segments
            .iter()
            .position(|registration| registration.segment.adapter == SegmentAdapterIdV1::StrictDer)
            .expect("DER registration");
        changed.registered_segments[der].segment.active_rows -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.trace_groups[5].column_chunks -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.common_lde_log2 = 22;
        mutations.push(changed);
        for (index, mutation) in mutations.iter().enumerate() {
            assert!(
                mutation
                    .validate_exact_full_profile_registration_v1()
                    .is_err(),
                "full-layout splice {index} must fail closed"
            );
        }
    }

    #[test]
    fn projection_challenge_labels_are_explicit_and_pairwise_distinct() {
        let labels = ZK_X509_PROJECTION_CHALLENGE_LABELS_V1
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), ZK_X509_PROJECTION_COPY_LANES_V1 * 7);
        let unique = labels.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), labels.len());
    }
}
