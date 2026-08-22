//! Private 40-limb direct cross-field RLWE kernel.
//!
//! This privately declared module settles the bounded generalized-Bulletproof
//! (GBP) child that replaces 200 independent 1,513-byte proofs.
//! Four cores each cover 50 `(limb, repetition)` evaluations.  Every evaluation
//! uses two 16,384-coordinate commitments, 206 Boolean multiplication gates,
//! and 413 linear constraints.  Thus one core has exactly 100 vector
//! commitments, 10,300 active gates, a 16,384-gate generator prefix, 20,650
//! constraints, and a 7,981-byte proof.  The four raw proofs occupy 31,924
//! bytes; the owned frame is 32,271 bytes under a hard 36,020-byte frame cap.
//! A non-empty global-lookup successor follows that frame and remains under the
//! inventory continuation cap; it is not incorrectly charged to the GBP frame.
//!
//! The checked integer relation for evaluation point `a` and limb modulus `q`
//! is
//!
//! ```text
//! K(a) = B(a) + beta A(a)                         (mod q)
//! C(a) = sum_j gamma^j (C0_j(a) + beta C1_j(a)) (mod q)
//! P~(a) = (a^N + 1) H~(a)                        (mod q)
//! U+(a) - U-(a) - (P~(a) + C(a)) = q (z+ - z-),
//! 0 <= z+, z- < 2^103.
//! ```
//!
//! `C+` and `C-` are derived from actual upstream commitment points, never
//! accepted from this wire.  Their coefficient-vector openings and the two
//! 103-bit quotient owners cross a one-shot source boundary.  Numeric values
//! `a,A,B,C0[0..43),C1[0..43)` are likewise obtained as values, validated, used
//! in the equation, and hashed canonically; a digest-only substitute is not an
//! implementation of the source trait.
//!
//! The candidate challenge order is explicit. A minimal q-mask point source
//! and demonstrably pre-qPCS safe axes hash all 6,400 `S` digit commitments
//! after the initial qPCS root. The transcript absorbs that root before it
//! derives the 200-point qPCS relation schedule. Only after qPCS does this
//! kernel combines a retained completed-qPCS lineage owner with a terminal
//! predecessor, an opaque future pre-direct inventory candidate projection,
//! and a candidate-only radix root, rehashes the same points, and derives the
//! authenticated RLWE `gamma/beta` schedule. The current inventory
//! `prior_context_digest_v1` and canonical
//! inventory root are prohibited: they inherit final terminal and continuation
//! state, including this direct proof. A dedicated pre-direct inventory
//! candidate projection must exclude cross/global/zero roots, final transcript
//! state and challenges, cross-section and zero-padding digests, continuation
//! state, and every direct-proof binding. GBP
//! core challenges bind that schedule, the canonical numeric root, and the
//! derived commitment root, but exclude the proof-set digest, successor
//! residual, codec digest, and final binding. Four completed cores enter a
//! move-only pending owner carrying an opaque successor-independent
//! cross-field-root capability. The prover's consuming typed bind takes the
//! sole matching qPCS transcript from that joint lineage owner; no caller may
//! supply a different transcript. The verifier first binds an encoded terminal
//! root claim through the same joint owner. After independently recomputing
//! the four-core root, it retains its own opaque verified-root evidence and the
//! transcript equality obligation together in a non-authorizing,
//! equality-pending owner. A concrete consuming transition moves that exact
//! direct-owned root into the transcript obligation and returns the
//! terminal-bound owner only on equality. The core transcript-set digest
//! remains private.
//!
//! A move-only provisional inventory preflight can now lend exactly the 6,400
//! proof-carried q-mask digits to the early root. It is self-consistent but
//! explicitly non-authorizing, exposes no raw proof or aggregate point owner,
//! and must later be consumed against the identical typed proof allocation and
//! linked final context. Its production proof-slice lease issuer is
//! uninhabited, so this does not make the chronology live.
//!
//! Production remains unavailable. The live numeric handoff and claimed
//! relation currently retain separate move-only qPCS schedules and final
//! transcript owners; public digest equality cannot prove sole lineage. The
//! declarations below therefore provide only a non-authorizing numeric cursor
//! split and a constructor-less membership-backed point projection. They do
//! not expose a schedule from numeric state or create a callable join.
//! Integration requires a top-level carrier established before the qPCS/direct
//! cycle: it must retain source-preflight and numeric/public owners, move the
//! sole lineaged schedule exactly once through a pre-auth claimed-qPCS state,
//! provisionally bind the claimed roots to obtain final challenge seeds,
//! authenticate qPCS while retaining that same ownership chain, discharge the
//! direct root obligations, and only then reach membership. It must also
//! introduce a dedicated pre-direct inventory candidate context/root; no
//! current inventory accessor satisfies that contract. Structural
//! wire/successor preflight always precedes traversal of an authoritative
//! source.
//! This kernel mints no composite, readiness, receipt, or release authority.

use core::{fmt, marker::PhantomData};
use std::sync::OnceLock;

#[cfg(test)]
use super::rns_native_transcript::ZkAmsMkheRnsNativeCrossFieldRootClaimV1;
use super::{
    rns_native_claimed_successor::RnsNativeClaimedSuccessorV1,
    rns_native_cross_field_inventory::{
        RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1,
        RnsNativeCrossFieldInventoryPrerequisiteV1, RnsNativePreQpcsQMaskInventoryPreflightV1,
    },
    rns_native_existing_radix_commitment_view::RnsNativeExistingRadixDirectAliasV1,
    rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
    rns_native_qpcs_fri_complete::{
        RnsNativeQpcsCompletedLineageV1, RnsNativeQpcsFriCompleteErrorV1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_transcript::{
        ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1,
        ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1,
        ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1, ZkAmsMkheRnsNativeTerminalRootsV1,
    },
};
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitStatement, ArithmeticCircuitWitness, GeneralizedBulletproofErrorV1,
        LinComb, ProofRandomSource, ProofSuite, ProverTranscript, Variable,
        VectorCommitmentOpening, VerifierTranscript, multiexp,
    },
    vega::{
        VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            SecretT256PointEncodingV1, ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
            ZkAmsT256BulletproofSuiteV1, with_borrowed_t256_scalar_encoding_v1,
        },
        sponge::{Keccak256, keccak256},
    },
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZRD4";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const LIMBS_V1: usize = 40;
const REPETITIONS_V1: usize = 5;
const EVALUATIONS_V1: usize = LIMBS_V1 * REPETITIONS_V1;
const RECORDS_V1: usize = 43;
const BLOCKS_PER_RECORD_V1: usize = 8;
const BLOCK_COORDINATES_V1: usize = 1 << 14;
const RING_DEGREE_V1: usize = BLOCKS_PER_RECORD_V1 * BLOCK_COORDINATES_V1;
const RADIX_BASE_V1: u64 = 1 << 15;
const RADIX_DIGITS_V1: usize = 18;
const SMALL_SOURCE_ROLES_V1: usize = 3;
const Q_MASK_DIGITS_V1: usize = 4;
const Q_MASK_OWNERS_V1: usize = EVALUATIONS_V1 * BLOCKS_PER_RECORD_V1;
const Q_MASK_S_POINTS_V1: usize = Q_MASK_OWNERS_V1 * Q_MASK_DIGITS_V1;
const Q_MASK_ROOT_BYTES_PER_POINT_V1: usize = 4 + 4 + POINT_BYTES_V1;
const Q_MASK_ROOT_FIXED_ABSORPTION_BYTES_V1: usize =
    Q_MASK_ROOT_DOMAIN_V1.len() + 1 + DIGEST_BYTES_V1 + 4;
const Q_MASK_ROOT_TOTAL_ABSORPTION_BYTES_V1: usize =
    Q_MASK_ROOT_FIXED_ABSORPTION_BYTES_V1 + Q_MASK_S_POINTS_V1 * Q_MASK_ROOT_BYTES_PER_POINT_V1;
const QUOTIENT_BITS_V1: usize = 103;
const GATES_PER_EVALUATION_V1: usize = 2 * QUOTIENT_BITS_V1;
const CONSTRAINTS_PER_EVALUATION_V1: usize = 2 * GATES_PER_EVALUATION_V1 + 1;
const CORES_V1: usize = 4;
const EVALUATIONS_PER_CORE_V1: usize = EVALUATIONS_V1 / CORES_V1;
const ACTIVE_GATES_PER_CORE_V1: usize = EVALUATIONS_PER_CORE_V1 * GATES_PER_EVALUATION_V1;
const PADDED_GATES_PER_CORE_V1: usize = 1 << 14;
const CONSTRAINTS_PER_CORE_V1: usize = EVALUATIONS_PER_CORE_V1 * CONSTRAINTS_PER_EVALUATION_V1;
const VECTOR_COMMITMENTS_PER_EVALUATION_V1: usize = 2;
const VECTOR_COMMITMENTS_PER_CORE_V1: usize =
    EVALUATIONS_PER_CORE_V1 * VECTOR_COMMITMENTS_PER_EVALUATION_V1;
const CORE_VECTOR_OPENING_SCALAR_BYTES_V1: usize =
    VECTOR_COMMITMENTS_PER_CORE_V1 * BLOCK_COORDINATES_V1 * SCALAR_BYTES_V1;
const LOG_N_V1: usize = 14;
const NI_V1: usize = 2 + 2 * (VECTOR_COMMITMENTS_PER_CORE_V1 / 2);
const L_POLYNOMIALS_V1: usize = NI_V1 + 2;
const T_POLYNOMIALS_V1: usize = 2 * L_POLYNOMIALS_V1 - 1;
const FIXED_PROOF_POINTS_V1: usize = 3 + T_POLYNOMIALS_V1 - 1;
const IPA_PROOF_POINTS_V1: usize = 2 * LOG_N_V1;
const PROOF_POINTS_PER_CORE_V1: usize = FIXED_PROOF_POINTS_V1 + IPA_PROOF_POINTS_V1;
const CIRCUIT_PROOF_SCALARS_V1: usize = 3;
const IPA_FINAL_SCALARS_V1: usize = 2;
const PROOF_SCALARS_PER_CORE_V1: usize = 5;
const CORE_PROOF_BYTES_V1: usize =
    PROOF_POINTS_PER_CORE_V1 * POINT_BYTES_V1 + PROOF_SCALARS_PER_CORE_V1 * SCALAR_BYTES_V1;
const ALL_CORE_PROOF_BYTES_V1: usize = CORES_V1 * CORE_PROOF_BYTES_V1;
const CORE_RECORD_HEADER_BYTES_V1: usize = 1 + 2 + 1 + 2;
const CORE_RECORD_BYTES_V1: usize = CORE_RECORD_HEADER_BYTES_V1 + CORE_PROOF_BYTES_V1;
// magic/version/flags/header/frame, nineteen geometry bytes, eight digests,
// and a u32 successor length.
const HEADER_BYTES_V1: usize = 4 + 1 + 1 + 2 + 4 + 19 + 8 * DIGEST_BYTES_V1 + 4;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const OWNED_WIRE_BYTES_V1: usize =
    HEADER_BYTES_V1 + CORES_V1 * CORE_RECORD_BYTES_V1 + CODEC_DIGEST_BYTES_V1;
const MIN_SUCCESSOR_BYTES_V1: usize = 1;
pub(super) const RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1: usize = OWNED_WIRE_BYTES_V1;
pub(super) const RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_MAX_BYTES_V1: usize = 36_020;
pub(super) const RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1: usize =
    RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 - OWNED_WIRE_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = OWNED_WIRE_BYTES_V1 + MIN_SUCCESSOR_BYTES_V1;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;
const MAX_Q_CHALLENGE_ATTEMPTS_V1: u16 = 256;
const GBP_CHALLENGES_PER_CORE_V1: usize = 4 + LOG_N_V1;
const POSITIVE_TERMS_PER_COORDINATE_V1: usize = 7_256;
const NEGATIVE_TERMS_PER_COORDINATE_V1: usize = 1_376;
const POSITIVE_TERMS_TOTAL_V1: usize = POSITIVE_TERMS_PER_COORDINATE_V1 * BLOCK_COORDINATES_V1;
const NEGATIVE_TERMS_TOTAL_V1: usize = NEGATIVE_TERMS_PER_COORDINATE_V1 * BLOCK_COORDINATES_V1;
const V_PLUS_BITS_V1: u16 = 88;
const V_MINUS_BITS_V1: u16 = 86;
const U_PLUS_BITS_V1: u16 = 162;
const U_MINUS_BITS_V1: u16 = 160;
const INTEGER_EXPRESSION_BITS_V1: u16 = 165;
const AGGREGATE_DISCREPANCY_DEGREE_V1: u64 = 262_185;
const CROSS_SOUNDNESS_BITS_X100_FLOOR_V1: u32 = 20_467;
const Q_MIN_V1: u64 = 1_152_921_504_396_869_633;
const Q_MAX_V1: u64 = 1_152_921_504_606_584_833;

const MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.manifest";
const PRE_QPCS_SAFE_AXES_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.pre-qpcs-safe-axes";
const FIXED_AXES_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.fixed-axes";
const Q_MASK_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.pre-qpcs-s-root";
const DIRECT_SCHEDULE_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.schedule-binding";
const AGGREGATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-rlwe-source.aggregation-challenge";
const NUMERIC_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.numeric-root";
const COMMITMENT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.commitment-root";
const CORE_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.core-transcript";
const CORE_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.core-challenge";
const PROOF_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.proof-set";
const CORE_TRANSCRIPT_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.core-transcript-set";
const CROSS_FIELD_CORE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.cross-field-core-root";
const DIRECT_CORE_SAFE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-direct-core-safe";
const SUCCESSOR_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.successor";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.codec";
const BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.binding";
const GEOMETRY_LANGUAGE_V1: &[u8] = b"limbs=40;repetitions=5;evaluations=200;cores=4;evaluations-per-core=50;quotient-bits=103;active-gates-per-evaluation=206;active-gates-per-core=10300;padded-gates-per-core=16384;constraints-per-evaluation=413;constraints-per-core=20650;vector-commitments-per-core=100;proof-points-per-core=237;proof-scalars-per-core=5;proof-bytes-per-core=7981;all-core-proof-bytes=31924;owned-frame-bytes=32271;frame-cap=36020;nonempty-successor-is-outside-owned-frame-and-inside-inventory-continuation-cap";
const RELATION_LANGUAGE_V1: &[u8] = b"K=B+beta*A mod q;C=sum-j-gamma^j*(C0_j+beta*C1_j) mod q;Ptilde=(a^N+1)*Htilde mod q;Uplus-Uminus-(Ptilde+C)=q*(zplus-zminus);zplus,zminus-in-[0,2^103);boolean-gates-use-(b,b,b);absolute-integer-expression<2^165<pT";
const DERIVATION_LANGUAGE_V1: &[u8] = b"Cplus=sum-j,b,h gamma^j*B^h*a^(bL)*CD[j,b,h]+sum-j,b gamma^j*K*a^(bL)*Cr-plus+gamma^j*pTmodq*a^(bL)*(Ce0-plus+beta*Ce1-plus)+(a^N+1)*sum-b,h B^h*a^(bL)*CS[b,h];Cminus=sum-j,b gamma^j*K*a^(bL)*Cr-minus+gamma^j*pTmodq*a^(bL)*(Ce0-minus+beta*Ce1-minus+Cone-Cborrow18)";
const NO_WRAP_LANGUAGE_V1: &[u8] = b"Vplus<7256*(B-1)*(qmax-1)<2^88;Vminus<1376*(B-1)*(qmax-1)<2^86;Uplus<118882304*(B-1)*(qmax-1)^2<2^162;Uminus<22544384*(B-1)*(qmax-1)^2<2^160;whole-signed-expression<2^165<pT;qmax=1152921504606584833";
const SOUNDNESS_LANGUAGE_V1: &[u8] = b"aggregate-discrepancy-degree=(2*131072-2)+42+1=262185;union-over-40-limbs-and-five-independent-repetitions<=40*(262185/qmin)^5<2^-204.67;qmin=1152921504396869633;plus-bounded-unbiased-q-rejection,GBP-knowledge-soundness,binding,and-Keccak-ROM-terms";
const SOURCE_LANGUAGE_V1: &[u8] = b"minimal-pre-qpcs-source-exposes-only-actual-q-mask-S-commitment-points;move-only-by-value-post-qpcs-authoritative-source;source-independent-successor-and-wire-structure/header/codec/cap-preflight-before-any-authoritative-source-call;take-a-A-B-C0[43]-C1[43]-qpcs-product-qpcs-opening-quotient-exactly-once-per-evaluation;read-actual-upstream-commitment-points;take-positive-and-negative-16384-coordinate-openings,masks,and-103-bit-owners-exactly-once;caller-zeroizing-destinations-precede-every-fallible-opening-call;drop-clears-retained-secret-copies;no-digest-only-evaluation-or-opening-source";
const TRANSCRIPT_LANGUAGE_V1: &[u8] = b"retain-exact-rns-native-rlwe-source-gamma/beta-schedule;hash-actual-6400-q-mask-S-digit-points-with-only-profile,source-binding,source-formula,source-mapping,rns-seed,qpcs-parameter,and-state-after-initial;exclude-source-terminal,packing,inventory,and-all-post-qpcs-results-from-S-root;derive-a-only-after-that-root-with-the-qpcs-prefix-rejection-map;after-qpcs-combine-only-terminal-predecessor,future-candidate-pre-direct-inventory-context,future-candidate-pre-direct-inventory-root,and-existing-radix-candidate axes;current-inventory-prior-context-and-canonical-root-are-prohibited-because-they-inherit-final-terminal-and-continuation-state;candidate-pre-direct-inventory-axes-must-exclude-cross/global/zero-roots,final-transcript-and-challenges,cross-section-and-zero-padding-digests,continuation-state,and-direct-proof-bindings;exclude-cross-proof,cross-link,inventory-binding,continuation-digest,packing-binding,radix-binding,and-successor-membership-from-direct-core-challenges;each-a-is-nonzero,distinct-across-five-same-limb-repetitions,a^131072+1!=0,and-a^524288!=1;core-challenges-bind-manifest,candidate-fixed-axes,S-root,direct-schedule-binding,relation-seed,numeric-root,derived-commitment-root,core-index,evaluation-range,actual-Cplus-Cminus,and-bp-basis;four-core-pending-owner-binds-proof-set-and-private-core-transcript-set-into-opaque-successor-independent-cross-field-root-capability;consuming-typed-bind-moves-root-into-staged-terminal-before-global-challenge;only-terminal-bound-pending-owner-may-seal-later-nonempty-successor;exclude-successor,codec,final-binding-from-core-root;admit-excluded-values-only-after-four-core-verification";
const INTEGRATION_LANGUAGE_V1: &[u8] = b"qpcs-source-settled:rns_native_transcript-enforces-initial,S,relation,quotient,batching,each-FRI-root/fold,query-order;rns_native_qpcs_prefix-prover-replay-verifier-consume-and-return-one-move-only-relation-schedule;staged-terminal-transcript-api-available:bind-cross-field-root,derive-global-challenge,bind-global-root,derive-zero-padding-challenge,bind-zero-padding-root;concrete-direct-verified-root/transcript-obligation-bridge-integrated;direct-activation-no-go-until:rns_native_cross_field_inventory-provides-a-dedicated-pre-direct-candidate-context-and-root-that-exclude-cross/global/zero-roots,final-transcript-and-challenges,cross-section-and-zero-padding-digests,continuation-state,and-direct-proof-bindings;current-inventory-prior-context-and-canonical-root-must-not-be-adapted;single-top-level-carrier-retains-source-preflight-and-numeric/public-owners,moves-the-sole-lineaged-schedule-once-into-a-pre-auth-claimed-qpcs-owner,provisionally-binds-claimed-roots-to-obtain-final-seeds,authenticates-qpcs-with-the-same-owner-chain,retains-authenticated-numeric-rows-for-later-direct-traversal,discharges-direct-root-obligations,and-only-then-reaches-membership;numeric-cursor-exposes-no-schedule-or-lineage;digest-equality-must-not-substitute-for-ownership;40-modulus-table-is-release-pinned;positive/negative-commitments-derived-only-by-this-formula;global-lookup-consumes-nonempty-successor;composite-recomputes-final-root-and-digest;production-source,pre-direct-inventory-axes,single-owner-chronology,direct-staged-adapter,padding,global-lookup,composite,readiness-remain-unavailable";

const DIRECT_RLWE_RELATION_KERNEL_AVAILABLE_V1: bool = true;
const PRE_DIRECT_CANDIDATE_AXIS_CONTRACT_SETTLED_V1: bool = true;
const MEMBERSHIP_BACKED_PUBLIC_POINT_ADAPTER_DECLARED_V1: bool = true;
const PRODUCTION_PRE_DIRECT_INVENTORY_AXES_INTEGRATED_V1: bool = false;
const PRE_QPCS_Q_MASK_TOKEN_INTEGRATED_V1: bool = false;
const AUTHORITATIVE_NUMERIC_SOURCE_INTEGRATED_V1: bool = false;
const POST_CORE_INVENTORY_LINK_INTEGRATED_V1: bool = false;
const SINGLE_OWNER_NUMERIC_MEMBERSHIP_CHRONOLOGY_AVAILABLE_V1: bool = false;
const VERIFIER_NUMERIC_MEMBERSHIP_JOIN_AVAILABLE_V1: bool = false;
const STAGED_TERMINAL_TRANSCRIPT_API_AVAILABLE_V1: bool = true;
const DIRECT_VERIFIED_ROOT_TYPE_BRIDGE_INTEGRATED_V1: bool = true;
const DIRECT_STAGED_TERMINAL_ADAPTER_INTEGRATED_V1: bool = false;
const GLOBAL_LOOKUP_SUCCESSOR_VERIFIED_V1: bool = false;
const COMPOSITE_ACCEPTANCE_AVAILABLE_V1: bool = false;
const MEASURED_RSS_QUALIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(EVALUATIONS_V1 == 200);
    assert!(RING_DEGREE_V1 == 131_072);
    assert!(Q_MASK_S_POINTS_V1 == 6_400);
    assert!(Q_MASK_ROOT_DOMAIN_V1.len() == 71);
    assert!(Q_MASK_ROOT_BYTES_PER_POINT_V1 == 41);
    assert!(Q_MASK_ROOT_FIXED_ABSORPTION_BYTES_V1 == 108);
    assert!(Q_MASK_ROOT_TOTAL_ABSORPTION_BYTES_V1 == 262_508);
    assert!(GATES_PER_EVALUATION_V1 == 206);
    assert!(CONSTRAINTS_PER_EVALUATION_V1 == 413);
    assert!(EVALUATIONS_PER_CORE_V1 == 50);
    assert!(ACTIVE_GATES_PER_CORE_V1 == 10_300);
    assert!(PADDED_GATES_PER_CORE_V1 == 16_384);
    assert!(CONSTRAINTS_PER_CORE_V1 == 20_650);
    assert!(VECTOR_COMMITMENTS_PER_CORE_V1 == 100);
    assert!(CORE_VECTOR_OPENING_SCALAR_BYTES_V1 == 52_428_800);
    assert!(NI_V1 == 102);
    assert!(L_POLYNOMIALS_V1 == 104);
    assert!(T_POLYNOMIALS_V1 == 207);
    assert!(FIXED_PROOF_POINTS_V1 == 209);
    assert!(IPA_PROOF_POINTS_V1 == 28);
    assert!(PROOF_POINTS_PER_CORE_V1 == 237);
    assert!(CIRCUIT_PROOF_SCALARS_V1 + IPA_FINAL_SCALARS_V1 == PROOF_SCALARS_PER_CORE_V1);
    assert!(CORE_PROOF_BYTES_V1 == 7_981);
    assert!(ALL_CORE_PROOF_BYTES_V1 == 31_924);
    assert!(HEADER_BYTES_V1 == 291);
    assert!(OWNED_WIRE_BYTES_V1 == 32_271);
    assert!(MIN_WIRE_BYTES_V1 == 32_272);
    assert!(RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1 == 32_271);
    assert!(RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1 < 36_020);
    assert!(RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 == 6_780_245);
    assert!(RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 == 6_747_974);
    assert!(GBP_CHALLENGES_PER_CORE_V1 == 18);
    assert!(POSITIVE_TERMS_PER_COORDINATE_V1 == RECORDS_V1 * BLOCKS_PER_RECORD_V1 * 21 + 32);
    assert!(NEGATIVE_TERMS_PER_COORDINATE_V1 == RECORDS_V1 * BLOCKS_PER_RECORD_V1 * 4);
    assert!(POSITIVE_TERMS_TOTAL_V1 == 118_882_304);
    assert!(NEGATIVE_TERMS_TOTAL_V1 == 22_544_384);
    assert!(V_PLUS_BITS_V1 == 88);
    assert!(V_MINUS_BITS_V1 == 86);
    assert!(U_PLUS_BITS_V1 == 162);
    assert!(U_MINUS_BITS_V1 == 160);
    assert!(INTEGER_EXPRESSION_BITS_V1 == 165);
    assert!(
        AGGREGATE_DISCREPANCY_DEGREE_V1 == (2 * RING_DEGREE_V1 - 2 + RECORDS_V1 - 1 + 1) as u64
    );
    assert!(CROSS_SOUNDNESS_BITS_X100_FLOOR_V1 == 20_467);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.len() == LIMBS_V1);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0] == Q_MAX_V1);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[LIMBS_V1 - 1] == Q_MIN_V1);
    assert!(Q_MIN_V1 <= Q_MAX_V1);
    assert!(DIRECT_RLWE_RELATION_KERNEL_AVAILABLE_V1);
    assert!(PRE_DIRECT_CANDIDATE_AXIS_CONTRACT_SETTLED_V1);
    assert!(MEMBERSHIP_BACKED_PUBLIC_POINT_ADAPTER_DECLARED_V1);
    assert!(!PRODUCTION_PRE_DIRECT_INVENTORY_AXES_INTEGRATED_V1);
    assert!(!PRE_QPCS_Q_MASK_TOKEN_INTEGRATED_V1);
    assert!(!AUTHORITATIVE_NUMERIC_SOURCE_INTEGRATED_V1);
    assert!(!POST_CORE_INVENTORY_LINK_INTEGRATED_V1);
    assert!(!SINGLE_OWNER_NUMERIC_MEMBERSHIP_CHRONOLOGY_AVAILABLE_V1);
    assert!(!VERIFIER_NUMERIC_MEMBERSHIP_JOIN_AVAILABLE_V1);
    assert!(STAGED_TERMINAL_TRANSCRIPT_API_AVAILABLE_V1);
    assert!(DIRECT_VERIFIED_ROOT_TYPE_BRIDGE_INTEGRATED_V1);
    assert!(!DIRECT_STAGED_TERMINAL_ADAPTER_INTEGRATED_V1);
    assert!(!GLOBAL_LOOKUP_SUCCESSOR_VERIFIED_V1);
    assert!(!COMPOSITE_ACCEPTANCE_AVAILABLE_V1);
    assert!(!MEASURED_RSS_QUALIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeCrossFieldRlweDirectErrorV1 {
    InvalidContext,
    InvalidGeometry,
    InvalidNumericEvaluation,
    InvalidPoint,
    InvalidScalar,
    InvalidCore,
    InvalidHeader,
    InvalidIntegrity,
    ProofCapExceeded,
    ChallengeExhausted,
    SourceUnavailable,
    ArithmeticOverflow,
    ResourceExhausted,
}

impl fmt::Display for RnsNativeCrossFieldRlweDirectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeCrossFieldRlweDirectErrorV1 {}

impl From<GeneralizedBulletproofErrorV1> for RnsNativeCrossFieldRlweDirectErrorV1 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        match error {
            GeneralizedBulletproofErrorV1::PointEncoding => Self::InvalidPoint,
            GeneralizedBulletproofErrorV1::ScalarEncoding => Self::InvalidScalar,
            GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted => Self::ChallengeExhausted,
            GeneralizedBulletproofErrorV1::ResourceOverflow => Self::ResourceExhausted,
            _ => Self::InvalidCore,
        }
    }
}

/// Minimal, demonstrably pre-qPCS axes used to hash the q-mask `S` points.
///
/// In particular, this type cannot carry source-terminal, packing, inventory,
/// qPCS-proof, or downstream bindings.
#[derive(Clone, Copy)]
pub(super) struct RnsNativeCrossFieldPreQpcsSafeAxesV1 {
    pub(super) profile_manifest_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_formula_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_mapping_digest: [u8; DIGEST_BYTES_V1],
    pub(super) rns_aggregation_challenge_seed: [u8; DIGEST_BYTES_V1],
    pub(super) qpcs_parameter_digest: [u8; DIGEST_BYTES_V1],
    /// Transcript state after the initial root and before the q-mask root.
    pub(super) qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeCrossFieldPreQpcsSafeAxesV1 {
    fn validate_v1(self) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let values = [
            self.profile_manifest_digest,
            self.source_binding_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.rns_aggregation_challenge_seed,
            self.qpcs_parameter_digest,
            self.qpcs_pre_relation_transcript_digest,
        ];
        if !nonzero_distinct_digests_v1(&values) {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        Ok(())
    }

    fn digest_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
        self.validate_v1()?;
        let mut hash = Keccak256::new();
        hash.update(PRE_QPCS_SAFE_AXES_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        for digest in [
            self.profile_manifest_digest,
            self.source_binding_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.rns_aggregation_challenge_seed,
            self.qpcs_parameter_digest,
            self.qpcs_pre_relation_transcript_digest,
        ] {
            hash.update(&digest);
        }
        hash.update(&manifest_digest_v1());
        Ok(hash.finalize())
    }
}

/// Candidate direct-core axes authenticated only after qPCS is available.
///
/// `terminal_predecessor_binding_digest` is the terminal kernel's predecessor
/// binding, not the source-terminal cross-proof or cross-link digest.
/// `candidate_inventory_axes` describes a future inventory projection that
/// does not yet exist. Its private context digest and root must exclude
/// cross-field, global-lookup, and
/// zero-padding roots; final transcript state and challenges; cross-section and
/// zero-padding digests; continuation state; and every direct-proof binding.
/// The current inventory `prior_context_digest_v1` and canonical inventory root
/// do not satisfy this contract and must never populate these fields.
/// `existing_radix_candidate_root` likewise excludes the inventory
/// continuation. This type cannot carry an inventory, packing, radix, or
/// continuation binding that would hash the direct proof itself.
pub(super) struct RnsNativeCrossFieldRlweFixedAxesV1 {
    pub(super) profile_manifest_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_formula_digest: [u8; DIGEST_BYTES_V1],
    pub(super) source_mapping_digest: [u8; DIGEST_BYTES_V1],
    pub(super) terminal_predecessor_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) candidate_inventory_axes: RnsNativePreDirectInventoryCandidateAxesV1,
    pub(super) existing_radix_candidate_root: [u8; DIGEST_BYTES_V1],
    pub(super) rns_aggregation_challenge_seed: [u8; DIGEST_BYTES_V1],
    pub(super) qpcs_parameter_digest: [u8; DIGEST_BYTES_V1],
    /// qPCS transcript state immediately before any relation challenge.
    pub(super) qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
}

/// Opaque successor-independent inventory projection required by the direct
/// challenge schedule.
///
/// There is deliberately no production constructor.  In particular, the
/// current inventory prior-context digest and inventory root cannot be adapted
/// into this type because both inherit the direct successor and final terminal
/// transcript.  A constructor may be added only with a dedicated authenticated
/// pre-direct projection.
#[allow(
    missing_copy_implementations,
    reason = "candidate inventory provenance must move into one direct schedule"
)]
pub(super) struct RnsNativePreDirectInventoryCandidateAxesV1 {
    context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePreDirectInventoryCandidateAxesV1 {
    fn validate_v1(&self) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        if self.context_digest == [0; DIGEST_BYTES_V1]
            || self.inventory_root == [0; DIGEST_BYTES_V1]
            || self.context_digest == self.inventory_root
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(
        context_digest: [u8; DIGEST_BYTES_V1],
        inventory_root: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        let value = Self {
            context_digest,
            inventory_root,
        };
        value.validate_v1()?;
        Ok(value)
    }
}

impl RnsNativeCrossFieldRlweFixedAxesV1 {
    const fn pre_qpcs_safe_axes_v1(&self) -> RnsNativeCrossFieldPreQpcsSafeAxesV1 {
        RnsNativeCrossFieldPreQpcsSafeAxesV1 {
            profile_manifest_digest: self.profile_manifest_digest,
            source_binding_digest: self.source_binding_digest,
            source_formula_digest: self.source_formula_digest,
            source_mapping_digest: self.source_mapping_digest,
            rns_aggregation_challenge_seed: self.rns_aggregation_challenge_seed,
            qpcs_parameter_digest: self.qpcs_parameter_digest,
            qpcs_pre_relation_transcript_digest: self.qpcs_pre_relation_transcript_digest,
        }
    }

    fn validate_v1(&self) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.candidate_inventory_axes.validate_v1()?;
        let values = [
            self.profile_manifest_digest,
            self.source_binding_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.terminal_predecessor_binding_digest,
            self.candidate_inventory_axes.context_digest,
            self.candidate_inventory_axes.inventory_root,
            self.existing_radix_candidate_root,
            self.rns_aggregation_challenge_seed,
            self.qpcs_parameter_digest,
            self.qpcs_pre_relation_transcript_digest,
        ];
        if values.contains(&[0; DIGEST_BYTES_V1])
            || values
                .iter()
                .enumerate()
                .any(|(index, value)| values[index + 1..].contains(value))
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        Ok(())
    }

    fn digest_v1(&self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
        self.validate_v1()?;
        let mut hash = Keccak256::new();
        hash.update(FIXED_AXES_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        for digest in [
            self.profile_manifest_digest,
            self.source_binding_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.terminal_predecessor_binding_digest,
            self.candidate_inventory_axes.context_digest,
            self.candidate_inventory_axes.inventory_root,
            self.existing_radix_candidate_root,
            self.rns_aggregation_challenge_seed,
            self.qpcs_parameter_digest,
            self.qpcs_pre_relation_transcript_digest,
        ] {
            hash.update(&digest);
        }
        hash.update(&manifest_digest_v1());
        Ok(hash.finalize())
    }
}

/// Post-qPCS capability combining the retained qPCS schedule, the safe `S`
/// root, and the future pre-direct candidate axes. It is move-only by
/// construction. Production cannot construct this capability until the
/// dedicated inventory candidate projection exists.
#[allow(missing_copy_implementations)]
pub(super) struct DirectQMaskScheduleBoundV1 {
    axes: RnsNativeCrossFieldRlweFixedAxesV1,
    pre_qpcs_safe_axes_digest: [u8; DIGEST_BYTES_V1],
    fixed_axes_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
    completed_qpcs: RnsNativeQpcsCompletedLineageV1,
}

impl DirectQMaskScheduleBoundV1 {
    pub(super) const fn q_mask_s_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.q_mask_s_root
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    const fn qpcs_schedule_v1(&self) -> &RnsNativeQpcsRelationScheduleV1 {
        self.completed_qpcs.relation_schedule_v1()
    }
}

/// Move-only direct relation schedule derived after qPCS with future
/// pre-direct inventory candidate axes.
#[allow(missing_copy_implementations)]
pub(super) struct RelationScheduleV1 {
    bound: DirectQMaskScheduleBoundV1,
    relation_seed: [u8; DIGEST_BYTES_V1],
    aggregation_challenges: [AggregationChallengeV1; EVALUATIONS_V1],
    cross_field_root_equality_obligation:
        Option<ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1>,
}

/// Move-only owner of one claimed direct relation and its complete terminal
/// chronology.
///
/// The original relation schedule retains the sole claimed-root equality
/// obligation. The opaque pre-global capability and final challenge record
/// remain paired with that exact schedule, and grant no verification,
/// composite, receipt, readiness, or release authority.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the claimed relation and its chronology must be consumed together"
)]
#[must_use = "a claimed relation remains non-authorizing until direct verification and root equality"]
pub(super) struct RnsNativeCrossFieldRlweClaimedRelationV1 {
    schedule: RelationScheduleV1,
    pre_global_capability: ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
    final_challenge_seeds: ZkAmsMkheRnsNativeChallengeSeedsV1,
}

/// Opaque core of a claimed direct frame. It retains the complete claimed
/// relation and exact-decoded frame for the later authoritative direct
/// verifier, but exposes no raw successor.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "claimed relation and frame preflight must remain paired"
)]
struct RnsNativeCrossFieldRlweClaimedFrameCoreV1<'proof> {
    claimed_relation: RnsNativeCrossFieldRlweClaimedRelationV1,
    preflight: FramePreflightV1<'proof>,
}

/// Temporary move-only owner that pairs one exact inventory with the claimed
/// relation/frame before the successor carrier is minted.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the exact inventory is destructured once into the claimed successor parent"
)]
pub(super) struct RnsNativeCrossFieldRlweClaimedFramePreflightV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    frame_core: RnsNativeCrossFieldRlweClaimedFrameCoreV1<'proof>,
    inventory: RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
}

/// Exact parent recursively retained by comparator and all later successors.
/// The sole inventory stays here while the frame core retains the claimed
/// transcript chronology and preflight for later direct verification.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "later proof stages recursively own this exact parent"
)]
pub(super) struct RnsNativeCrossFieldRlweClaimedInventoryParentV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    frame_core: RnsNativeCrossFieldRlweClaimedFrameCoreV1<'proof>,
    inventory: RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>
{
    /// Borrow only the opaque pre-global snapshot retained by this exact
    /// claimed relation. No raw transcript binding or seed is exposed.
    pub(super) const fn pre_global_lookup_capability_v1(
        &self,
    ) -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1 {
        &self.frame_core.claimed_relation.pre_global_capability
    }

    pub(super) const fn inventory(
        &self,
    ) -> &RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S> {
        &self.inventory
    }
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeCrossFieldRlweClaimedFramePreflightV1<'source, 'proof, S>
{
    fn into_claimed_successor_v1(
        self,
    ) -> RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    > {
        let Self {
            frame_core,
            inventory,
        } = self;
        let successor_claim = frame_core.preflight.claimed_successor_slice_v1();
        RnsNativeClaimedSuccessorV1::from_direct_claim_v1(
            RnsNativeCrossFieldRlweClaimedInventoryParentV1 {
                frame_core,
                inventory,
            },
            successor_claim,
        )
    }
}

impl RelationScheduleV1 {
    pub(super) const fn q_mask_s_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.bound.q_mask_s_root
    }

    pub(super) const fn direct_schedule_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.bound.binding_digest
    }

    pub(super) const fn relation_seed(&self) -> [u8; DIGEST_BYTES_V1] {
        self.relation_seed
    }

    /// Consume this schedule and all three tagged terminal roots atomically.
    /// The claimed-root equality obligation, exact pre-global chronology, and
    /// final non-authorizing challenges cannot be split by a production
    /// caller.
    #[allow(
        dead_code,
        reason = "the future successor-first adapter consumes this atomic owner"
    )]
    pub(super) fn bind_claimed_terminal_roots_v1(
        mut self,
        roots: ZkAmsMkheRnsNativeTerminalRootsV1,
    ) -> Result<RnsNativeCrossFieldRlweClaimedRelationV1, RnsNativeCrossFieldRlweDirectErrorV1>
    {
        if self.cross_field_root_equality_obligation.is_some() {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        let (claim, remaining_roots) = roots.into_cross_field_claim_v1();
        let (cross_field_transcript, equality_obligation) = self
            .bound
            .completed_qpcs
            .bind_claimed_cross_field_root_v1(claim)
            .map_err(map_qpcs_complete_error_v1)?;
        let (pre_global_capability, final_challenge_seeds) = cross_field_transcript
            .bind_remaining_terminal_roots_v1(remaining_roots)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)?;
        self.cross_field_root_equality_obligation = Some(equality_obligation);
        Ok(RnsNativeCrossFieldRlweClaimedRelationV1 {
            schedule: self,
            pre_global_capability,
            final_challenge_seeds,
        })
    }

    /// Bind the authenticated claimed root before the successor chain is
    /// traversed.  The returned transcript is provisional; this schedule keeps
    /// the sole equality obligation for the later direct-core verification.
    #[cfg(test)]
    pub(super) fn bind_claimed_cross_field_root_v1(
        &mut self,
        claim: ZkAmsMkheRnsNativeCrossFieldRootClaimV1,
    ) -> Result<ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1, RnsNativeCrossFieldRlweDirectErrorV1>
    {
        if self.cross_field_root_equality_obligation.is_some() {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        let (transcript, obligation) = self
            .bound
            .completed_qpcs
            .bind_claimed_cross_field_root_v1(claim)
            .map_err(map_qpcs_complete_error_v1)?;
        self.cross_field_root_equality_obligation = Some(obligation);
        Ok(transcript)
    }

    const fn has_claimed_cross_field_root_v1(&self) -> bool {
        self.cross_field_root_equality_obligation.is_some()
            && !self
                .bound
                .completed_qpcs
                .has_unconsumed_qpcs_transcript_v1()
    }

    fn take_cross_field_root_equality_obligation_v1(
        &mut self,
    ) -> Result<
        ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1,
        RnsNativeCrossFieldRlweDirectErrorV1,
    > {
        self.cross_field_root_equality_obligation
            .take()
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    }
}

fn map_qpcs_complete_error_v1(
    error: RnsNativeQpcsFriCompleteErrorV1,
) -> RnsNativeCrossFieldRlweDirectErrorV1 {
    match error {
        RnsNativeQpcsFriCompleteErrorV1::InvalidOrder
        | RnsNativeQpcsFriCompleteErrorV1::InvalidContext => {
            RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext
        }
        _ => RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AggregationChallengeV1 {
    gamma: u64,
    beta: u64,
}

#[derive(Clone, Copy)]
pub(super) struct RelationChallengesV1 {
    pub(super) gamma: u64,
    pub(super) beta: u64,
    pub(super) point: u64,
}

/// Actual public numeric values for one relation.  `Default` deliberately uses
/// non-canonical sentinels so a partial source write cannot silently validate.
#[derive(Clone, Copy)]
pub(super) struct RnsNativeCrossFieldNumericEvaluationV1 {
    pub(super) a: u64,
    pub(super) public_a: u64,
    pub(super) public_b: u64,
    pub(super) ciphertext_c0: [u64; RECORDS_V1],
    pub(super) ciphertext_c1: [u64; RECORDS_V1],
    pub(super) qpcs_product: u64,
    pub(super) qpcs_opening_quotient: u64,
}

impl Default for RnsNativeCrossFieldNumericEvaluationV1 {
    fn default() -> Self {
        Self {
            a: u64::MAX,
            public_a: u64::MAX,
            public_b: u64::MAX,
            ciphertext_c0: [u64::MAX; RECORDS_V1],
            ciphertext_c1: [u64::MAX; RECORDS_V1],
            qpcs_product: u64::MAX,
            qpcs_opening_quotient: u64::MAX,
        }
    }
}

/// Minimal early source for the actual 6,400 q-mask `S` commitment points.
///
/// It deliberately exposes no qPCS evaluation, quotient, packing, inventory,
/// or downstream proof data.
pub(super) trait RnsNativeQMaskSCommitmentSourceV1 {
    fn q_mask_s_digit_commitment_v1(
        &self,
        limb: usize,
        repetition: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1>;
}

/// One-shot cursor for authenticated public numeric values.
///
/// It deliberately has no point, secret-opening, schedule, lineage, or
/// completion surface. This split is preparatory and non-authorizing: the live
/// handoff may implement it, but no production numeric/membership join exists.
pub(super) trait RnsNativeCrossFieldNumericCursorV1: Sized {
    fn authoritative_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1];

    fn take_numeric_evaluation_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1>;
}

/// Authenticated public-point source used by deterministic `C+`/`C-`
/// derivation. Production construction is internal to the membership handoff:
/// callers cannot supply detached points or a raw inventory slice.
pub(super) trait RnsNativeCrossFieldAuthenticatedPublicPointSourceV1:
    RnsNativeQMaskSCommitmentSourceV1 + Sized
{
    fn message_radix_digit_commitment_v1(
        &self,
        record: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1>;

    fn small_signed_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1>;

    fn small_negative_magnitude_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1>;

    fn comparator_final_borrow_commitment_v1(
        &self,
        record: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1>;
}

/// Full verifier-side source. Numeric traversal and authenticated public
/// points remain distinct ownership surfaces and can be combined only inside
/// the exact membership-backed adapter once a real single-owner chronology
/// exists.
pub(super) trait RnsNativeCrossFieldAuthoritativeSourceV1:
    RnsNativeCrossFieldNumericCursorV1 + RnsNativeCrossFieldAuthenticatedPublicPointSourceV1 + Sized
{
}

impl<T> RnsNativeCrossFieldAuthoritativeSourceV1 for T where
    T: RnsNativeCrossFieldNumericCursorV1
        + RnsNativeCrossFieldAuthenticatedPublicPointSourceV1
        + Sized
{
}

fn direct_group_v1(
    record: usize,
    block: usize,
) -> Result<usize, RnsNativeCrossFieldRlweDirectErrorV1> {
    if record >= RECORDS_V1 || block >= BLOCKS_PER_RECORD_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    record
        .checked_mul(BLOCKS_PER_RECORD_V1)
        .and_then(|value| value.checked_add(block))
        .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)
}

fn direct_small_owner_v1(
    record: usize,
    role: usize,
    block: usize,
) -> Result<usize, RnsNativeCrossFieldRlweDirectErrorV1> {
    if record >= RECORDS_V1 || role >= SMALL_SOURCE_ROLES_V1 || block >= BLOCKS_PER_RECORD_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    record
        .checked_mul(SMALL_SOURCE_ROLES_V1)
        .and_then(|value| value.checked_add(role))
        .and_then(|value| value.checked_mul(BLOCKS_PER_RECORD_V1))
        .and_then(|value| value.checked_add(block))
        .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)
}

fn direct_q_mask_owner_v1(
    limb: usize,
    repetition: usize,
    block: usize,
) -> Result<usize, RnsNativeCrossFieldRlweDirectErrorV1> {
    if limb >= LIMBS_V1 || repetition >= REPETITIONS_V1 || block >= BLOCKS_PER_RECORD_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    limb.checked_mul(REPETITIONS_V1)
        .and_then(|value| value.checked_add(repetition))
        .and_then(|value| value.checked_mul(BLOCKS_PER_RECORD_V1))
        .and_then(|value| value.checked_add(block))
        .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)
}

/// Early verifier-only projection from the move-only provisional inventory
/// preflight.  This is intentionally only the minimal q-mask source trait: it
/// does not implement either authenticated public-point or authoritative
/// source ownership.
impl RnsNativeQMaskSCommitmentSourceV1 for RnsNativePreQpcsQMaskInventoryPreflightV1<'_> {
    fn q_mask_s_digit_commitment_v1(
        &self,
        limb: usize,
        repetition: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        if digit >= Q_MASK_DIGITS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        self.project_q_mask_s_digit_v1(direct_q_mask_owner_v1(limb, repetition, block)?, digit)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }
}

/// Ephemeral adapter assembled only after recovering the exact inventory from
/// the membership-owned claimed carrier. It owns the authenticated radix alias
/// and borrows both the inventory and live numeric cursor for one direct
/// traversal; no point bytes, numeric arrays, or raw owner parts escape it.
///
/// This declaration has no production constructor. In particular, the
/// current numeric handoff and claimed relation retain separate move-only qPCS
/// schedule/final-transcript owners, so they cannot truthfully construct this
/// adapter. A future top-level carrier must move the sole lineage through a
/// pre-auth claimed-qPCS state and retain only already-authenticated numeric
/// rows for this later borrow.
#[allow(
    dead_code,
    reason = "the single-owner numeric/membership chronology is deliberately not constructible"
)]
struct RnsNativeMembershipBackedDirectSourceV1<
    'owner,
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    N: RnsNativeCrossFieldNumericCursorV1,
> {
    numeric: &'owner mut N,
    existing_radix: RnsNativeExistingRadixDirectAliasV1<'proof>,
    inventory: &'owner RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
}

impl<S, N> RnsNativeCrossFieldNumericCursorV1
    for RnsNativeMembershipBackedDirectSourceV1<'_, '_, '_, S, N>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    N: RnsNativeCrossFieldNumericCursorV1,
{
    fn authoritative_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.numeric.authoritative_binding_digest_v1()
    }

    fn take_numeric_evaluation_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.numeric
            .take_numeric_evaluation_v1(limb, repetition, destination)
    }
}

impl<S, N> RnsNativeQMaskSCommitmentSourceV1
    for RnsNativeMembershipBackedDirectSourceV1<'_, '_, '_, S, N>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    N: RnsNativeCrossFieldNumericCursorV1,
{
    fn q_mask_s_digit_commitment_v1(
        &self,
        limb: usize,
        repetition: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        if digit >= Q_MASK_DIGITS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        self.inventory
            .q_mask_linear_commitments(direct_q_mask_owner_v1(limb, repetition, block)?)
            .map(|commitments| commitments.digits[digit])
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }
}

impl<S, N> RnsNativeCrossFieldAuthenticatedPublicPointSourceV1
    for RnsNativeMembershipBackedDirectSourceV1<'_, '_, '_, S, N>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    N: RnsNativeCrossFieldNumericCursorV1,
{
    fn message_radix_digit_commitment_v1(
        &self,
        record: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        if digit >= RADIX_DIGITS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        let group = direct_group_v1(record, block)?;
        if digit + 1 == RADIX_DIGITS_V1 {
            return self
                .inventory
                .comparator_top_commitments(group)
                .map(|(difference_top, _)| difference_top)
                .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
        }
        self.existing_radix
            .difference_low_commitment_v1(group, digit)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }

    fn small_signed_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.inventory
            .small_source_product_commitments(direct_small_owner_v1(record, role, block)?)
            .map(|commitments| commitments.signed)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }

    fn small_negative_magnitude_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.inventory
            .small_source_product_commitments(direct_small_owner_v1(record, role, block)?)
            .map(|commitments| commitments.negative_magnitude)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }

    fn comparator_final_borrow_commitment_v1(
        &self,
        record: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.inventory
            .comparator_range_carry_commitments(direct_group_v1(record, block)?)
            .map(|commitments| commitments.borrows[RADIX_DIGITS_V1 - 1])
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    }
}

/// Prover extension that moves each positive/negative vector opening and its
/// 103-bit quotient owner into caller-owned, already-zeroizing destinations.
pub(super) trait RnsNativeCrossFieldQuotientOpeningSourceV1:
    RnsNativeCrossFieldAuthoritativeSourceV1
{
    fn take_positive_quotient_owner_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1>;

    fn take_negative_quotient_owner_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1>;
}

#[derive(Clone, Copy)]
struct ValidatedEvaluationV1 {
    limb: u8,
    repetition: u8,
    modulus: u64,
    gamma: u64,
    beta: u64,
    point: u64,
    public_a: u64,
    public_b: u64,
    key_evaluation: u64,
    ciphertext_evaluation: u64,
    qpcs_product: u64,
    qpcs_opening_quotient: u64,
}

impl ValidatedEvaluationV1 {
    fn public_y_v1(self) -> u64 {
        mod_add_v1(self.qpcs_product, self.ciphertext_evaluation, self.modulus)
    }
}

#[derive(Clone, Copy)]
struct DerivedCommitmentsV1 {
    positive: Point,
    negative: Point,
}

struct PreparedInputsV1 {
    schedule: RelationScheduleV1,
    evaluations: [ValidatedEvaluationV1; EVALUATIONS_V1],
    commitments: [DerivedCommitmentsV1; EVALUATIONS_V1],
    numeric_root: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
}

fn mod_add_v1(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_mul_v1(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) * u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_pow_v1(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mod_mul_v1(result, base, modulus);
        }
        base = mod_mul_v1(base, base, modulus);
        exponent >>= 1;
    }
    result
}

fn t256_mod_q_v1(modulus: u64) -> u64 {
    VEGA_T256_SCALAR_MODULUS_BE_V1
        .iter()
        .fold(0, |value, byte| {
            ((u128::from(value) << 8) + u128::from(*byte)).rem_euclid(u128::from(modulus)) as u64
        })
}

fn manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for language in [
        GEOMETRY_LANGUAGE_V1,
        RELATION_LANGUAGE_V1,
        DERIVATION_LANGUAGE_V1,
        NO_WRAP_LANGUAGE_V1,
        SOUNDNESS_LANGUAGE_V1,
        SOURCE_LANGUAGE_V1,
        TRANSCRIPT_LANGUAGE_V1,
        INTEGRATION_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u32).to_be_bytes());
        hash.update(language);
    }
    hash.finalize()
}

fn nonzero_distinct_digests_v1(values: &[[u8; DIGEST_BYTES_V1]]) -> bool {
    !values.contains(&[0; DIGEST_BYTES_V1])
        && !values
            .iter()
            .enumerate()
            .any(|(index, value)| values[index + 1..].contains(value))
}

fn point_bytes_v1(
    point: Point,
) -> Result<[u8; POINT_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    point
        .to_non_identity_wire_bytes()
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
}

/// Hash the exact 6,400 q-mask points before qPCS relation sampling.
///
/// The type boundary excludes every post-qPCS/direct-core predecessor binding
/// from this root's preimage and requires only the minimal point source.
pub(super) fn q_mask_s_root_v1<P: RnsNativeQMaskSCommitmentSourceV1>(
    axes: RnsNativeCrossFieldPreQpcsSafeAxesV1,
    source: &P,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    axes.validate_v1()?;
    let mut hash = Keccak256::new();
    hash.update(Q_MASK_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&axes.digest_v1()?);
    hash.update(&(Q_MASK_S_POINTS_V1 as u32).to_be_bytes());
    let mut ordinal = 0_u32;
    for limb in 0..LIMBS_V1 {
        for repetition in 0..REPETITIONS_V1 {
            for block in 0..BLOCKS_PER_RECORD_V1 {
                for digit in 0..Q_MASK_DIGITS_V1 {
                    let point =
                        source.q_mask_s_digit_commitment_v1(limb, repetition, block, digit)?;
                    hash.update(&ordinal.to_be_bytes());
                    hash.update(&[limb as u8, repetition as u8, block as u8, digit as u8]);
                    hash.update(&point_bytes_v1(point)?);
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?;
                }
            }
        }
    }
    if ordinal as usize != Q_MASK_S_POINTS_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let root = hash.finalize();
    if root == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(root)
}

fn bind_direct_q_mask_schedule_v1(
    axes: RnsNativeCrossFieldRlweFixedAxesV1,
    completed_qpcs: RnsNativeQpcsCompletedLineageV1,
) -> Result<DirectQMaskScheduleBoundV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    let qpcs_schedule = completed_qpcs.relation_schedule_v1();
    let pre_qpcs_safe_axes_digest = axes.pre_qpcs_safe_axes_v1().digest_v1()?;
    let fixed_axes_digest = axes.digest_v1()?;
    let q_mask_s_root = qpcs_schedule.q_mask_s_root();
    if q_mask_s_root == [0; DIGEST_BYTES_V1]
        || q_mask_s_root == pre_qpcs_safe_axes_digest
        || q_mask_s_root == fixed_axes_digest
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    if qpcs_schedule.parameter_digest() != axes.qpcs_parameter_digest
        || qpcs_schedule.qpcs_pre_relation_transcript_digest()
            != axes.qpcs_pre_relation_transcript_digest
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_SCHEDULE_BINDING_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&pre_qpcs_safe_axes_digest);
    hash.update(&fixed_axes_digest);
    hash.update(&axes.qpcs_parameter_digest);
    hash.update(&axes.qpcs_pre_relation_transcript_digest);
    hash.update(&q_mask_s_root);
    let binding_digest = hash.finalize();
    if binding_digest == [0; DIGEST_BYTES_V1]
        || binding_digest == pre_qpcs_safe_axes_digest
        || binding_digest == fixed_axes_digest
        || binding_digest == q_mask_s_root
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(DirectQMaskScheduleBoundV1 {
        axes,
        pre_qpcs_safe_axes_digest,
        fixed_axes_digest,
        q_mask_s_root,
        binding_digest,
        completed_qpcs,
    })
}

fn derive_relation_schedule_v1(
    bound: DirectQMaskScheduleBoundV1,
) -> Result<RelationScheduleV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    let relation_seed = bound.qpcs_schedule_v1().relation_seed();
    if !nonzero_distinct_digests_v1(&[
        bound.pre_qpcs_safe_axes_digest,
        bound.fixed_axes_digest,
        bound.q_mask_s_root,
        bound.binding_digest,
        relation_seed,
    ]) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(RelationScheduleV1 {
        aggregation_challenges: derive_exact_aggregation_challenges_v1(&bound.axes)?,
        bound,
        relation_seed,
        cross_field_root_equality_obligation: None,
    })
}

/// Direct-kernel chronology entry after qPCS and future pre-direct inventory
/// candidate axes are available.
///
/// Rehash the actual `S` commitments against only the pre-qPCS-safe axes,
/// require the transcript-bound root, then combine the same move-only qPCS
/// owner with the candidate post-qPCS axes and derive the aggregation schedule.
/// The current inventory prior-context digest and canonical root are forbidden
/// inputs because both inherit successor-dependent state.
/// Rebuilding qPCS relation points inside this kernel is forbidden.
pub(super) fn prepare_direct_relation_schedule_after_qpcs_v1<
    P: RnsNativeCrossFieldAuthoritativeSourceV1,
>(
    axes: RnsNativeCrossFieldRlweFixedAxesV1,
    source: &P,
    completed_qpcs: RnsNativeQpcsCompletedLineageV1,
) -> Result<RelationScheduleV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    let qpcs_schedule = completed_qpcs.relation_schedule_v1();
    let q_mask_s_root = q_mask_s_root_v1(axes.pre_qpcs_safe_axes_v1(), source)?;
    if q_mask_s_root != qpcs_schedule.q_mask_s_root() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let bound = bind_direct_q_mask_schedule_v1(axes, completed_qpcs)?;
    derive_relation_schedule_v1(bound)
}

fn validate_relation_schedule_v1(
    schedule: &RelationScheduleV1,
) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    let pre_qpcs_safe_axes_digest = schedule.bound.axes.pre_qpcs_safe_axes_v1().digest_v1()?;
    let fixed_axes_digest = schedule.bound.axes.digest_v1()?;
    if pre_qpcs_safe_axes_digest != schedule.bound.pre_qpcs_safe_axes_digest
        || fixed_axes_digest != schedule.bound.fixed_axes_digest
        || schedule
            .bound
            .completed_qpcs
            .has_unconsumed_qpcs_transcript_v1()
            == schedule.cross_field_root_equality_obligation.is_some()
        || schedule.bound.q_mask_s_root == [0; DIGEST_BYTES_V1]
        || schedule.bound.q_mask_s_root != schedule.bound.qpcs_schedule_v1().q_mask_s_root()
        || schedule.bound.axes.qpcs_parameter_digest
            != schedule.bound.qpcs_schedule_v1().parameter_digest()
        || schedule.bound.axes.qpcs_pre_relation_transcript_digest
            != schedule
                .bound
                .qpcs_schedule_v1()
                .qpcs_pre_relation_transcript_digest()
        || schedule.relation_seed != schedule.bound.qpcs_schedule_v1().relation_seed()
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let mut binding_hash = Keccak256::new();
    binding_hash.update(DIRECT_SCHEDULE_BINDING_DOMAIN_V1);
    binding_hash.update(&[VERSION_V1]);
    binding_hash.update(&pre_qpcs_safe_axes_digest);
    binding_hash.update(&fixed_axes_digest);
    binding_hash.update(&schedule.bound.axes.qpcs_parameter_digest);
    binding_hash.update(&schedule.bound.axes.qpcs_pre_relation_transcript_digest);
    binding_hash.update(&schedule.bound.q_mask_s_root);
    if binding_hash.finalize() != schedule.bound.binding_digest {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    schedule
        .bound
        .qpcs_schedule_v1()
        .validate_context_v1(
            schedule.bound.axes.qpcs_parameter_digest,
            schedule.bound.q_mask_s_root,
            schedule.bound.axes.qpcs_pre_relation_transcript_digest,
            schedule.relation_seed,
        )
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)?;
    if derive_exact_aggregation_challenges_v1(&schedule.bound.axes)?
        != schedule.aggregation_challenges
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(())
}

fn map_unbiased_nonzero_q_challenge_v1(raw: u64, modulus: u64, used: &[u64]) -> Option<u64> {
    if modulus < 3 {
        return None;
    }
    let rejection_bound = u64::MAX - u64::MAX % modulus;
    if raw >= rejection_bound {
        return None;
    }
    let candidate = raw % modulus;
    (candidate != 0 && !used.contains(&candidate)).then_some(candidate)
}

fn derive_aggregation_challenge_coordinate_v1(
    axes: &RnsNativeCrossFieldRlweFixedAxesV1,
    limb: usize,
    repetition: usize,
    modulus: u64,
    role: u8,
    used: &[u64],
) -> Result<u64, RnsNativeCrossFieldRlweDirectErrorV1> {
    if limb >= LIMBS_V1
        || repetition >= REPETITIONS_V1
        || role > 1
        || !(Q_MIN_V1..=Q_MAX_V1).contains(&modulus)
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    for attempt in 0..MAX_Q_CHALLENGE_ATTEMPTS_V1 {
        let mut hash = Keccak256::new();
        hash.update(AGGREGATION_CHALLENGE_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&axes.qpcs_parameter_digest);
        hash.update(&axes.rns_aggregation_challenge_seed);
        hash.update(&axes.source_formula_digest);
        hash.update(&axes.source_mapping_digest);
        hash.update(&[limb as u8, repetition as u8, role]);
        hash.update(&modulus.to_be_bytes());
        hash.update(&attempt.to_be_bytes());
        let digest = hash.finalize();
        let raw = u64::from_be_bytes(
            digest[..8]
                .try_into()
                .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)?,
        );
        if let Some(candidate) = map_unbiased_nonzero_q_challenge_v1(raw, modulus, used) {
            return Ok(candidate);
        }
    }
    Err(RnsNativeCrossFieldRlweDirectErrorV1::ChallengeExhausted)
}

fn derive_exact_aggregation_challenges_v1(
    axes: &RnsNativeCrossFieldRlweFixedAxesV1,
) -> Result<[AggregationChallengeV1; EVALUATIONS_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    axes.validate_v1()?;
    let mut result = [AggregationChallengeV1 { gamma: 0, beta: 0 }; EVALUATIONS_V1];
    let mut prior_pairs = [(0_u64, 0_u64); EVALUATIONS_V1];
    let mut prior_pair_count = 0;
    for limb in 0..LIMBS_V1 {
        let modulus = release_modulus_v1(limb)?;
        let mut used = [0_u64; 2 * REPETITIONS_V1];
        let mut used_len = 0;
        for repetition in 0..REPETITIONS_V1 {
            let gamma = derive_aggregation_challenge_coordinate_v1(
                axes,
                limb,
                repetition,
                modulus,
                0,
                &used[..used_len],
            )?;
            used[used_len] = gamma;
            used_len += 1;
            let beta = derive_aggregation_challenge_coordinate_v1(
                axes,
                limb,
                repetition,
                modulus,
                1,
                &used[..used_len],
            )?;
            used[used_len] = beta;
            used_len += 1;
            if prior_pairs[..prior_pair_count].contains(&(gamma, beta)) {
                return Err(RnsNativeCrossFieldRlweDirectErrorV1::ChallengeExhausted);
            }
            prior_pairs[prior_pair_count] = (gamma, beta);
            prior_pair_count += 1;
            result[limb * REPETITIONS_V1 + repetition] = AggregationChallengeV1 { gamma, beta };
        }
    }
    Ok(result)
}

pub(super) fn relation_challenges_v1(
    schedule: &RelationScheduleV1,
    limb: usize,
    repetition: usize,
    modulus: u64,
) -> Result<RelationChallengesV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    if modulus != release_modulus_v1(limb)? || repetition >= REPETITIONS_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let aggregation = schedule.aggregation_challenges[limb * REPETITIONS_V1 + repetition];
    Ok(RelationChallengesV1 {
        gamma: aggregation.gamma,
        beta: aggregation.beta,
        point: schedule
            .bound
            .qpcs_schedule_v1()
            .point(limb, repetition)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry)?,
    })
}

fn release_modulus_v1(limb: usize) -> Result<u64, RnsNativeCrossFieldRlweDirectErrorV1> {
    ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .get(limb)
        .copied()
        .filter(|modulus| (Q_MIN_V1..=Q_MAX_V1).contains(modulus))
        .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry)
}

fn one_vector_commitment_v1() -> Point {
    static COMMITMENT: OnceLock<Point> = OnceLock::new();
    *COMMITMENT.get_or_init(|| {
        let generators = ZkAmsT256BulletproofSuiteV1::generators();
        let terms: Vec<_> = generators.g_bold[..BLOCK_COORDINATES_V1]
            .iter()
            .copied()
            .map(|point| (Scalar::one(), point))
            .collect();
        multiexp::<ZkAmsT256BulletproofSuiteV1>(&terms)
    })
}

fn push_public_term_v1(
    terms: &mut Vec<(Scalar, Point)>,
    coefficient: u64,
    point: Point,
    modulus: u64,
) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    if coefficient >= modulus || point.is_identity() || terms.len() >= terms.capacity() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
    }
    terms.push((Scalar::from_u64(coefficient), point));
    Ok(())
}

fn derive_commitments_v1<P: RnsNativeCrossFieldAuthoritativeSourceV1>(
    source: &P,
    evaluation: ValidatedEvaluationV1,
) -> Result<DerivedCommitmentsV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    let q = evaluation.modulus;
    let p_mod_q = t256_mod_q_v1(q);
    let block_step = mod_pow_v1(evaluation.point, BLOCK_COORDINATES_V1 as u64, q);
    let mask_factor = mod_add_v1(mod_pow_v1(evaluation.point, RING_DEGREE_V1 as u64, q), 1, q);
    if mask_factor == 0 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation);
    }
    let one_commitment = one_vector_commitment_v1();
    if one_commitment.is_identity() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
    }

    let mut positive_terms = Vec::new();
    positive_terms
        .try_reserve_exact(POSITIVE_TERMS_PER_COORDINATE_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    let mut negative_terms = Vec::new();
    negative_terms
        .try_reserve_exact(NEGATIVE_TERMS_PER_COORDINATE_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;

    let mut gamma_power = 1;
    for record in 0..RECORDS_V1 {
        let mut block_power = 1;
        for block in 0..BLOCKS_PER_RECORD_V1 {
            let mut radix_power = 1;
            for digit in 0..RADIX_DIGITS_V1 {
                let coefficient =
                    mod_mul_v1(mod_mul_v1(gamma_power, radix_power, q), block_power, q);
                push_public_term_v1(
                    &mut positive_terms,
                    coefficient,
                    source.message_radix_digit_commitment_v1(record, block, digit)?,
                    q,
                )?;
                radix_power = mod_mul_v1(radix_power, RADIX_BASE_V1, q);
            }

            let r_weight = mod_mul_v1(
                mod_mul_v1(gamma_power, evaluation.key_evaluation, q),
                block_power,
                q,
            );
            let e0_weight = mod_mul_v1(mod_mul_v1(gamma_power, p_mod_q, q), block_power, q);
            let e1_weight = mod_mul_v1(e0_weight, evaluation.beta, q);
            for (role, coefficient) in [(0, r_weight), (1, e0_weight), (2, e1_weight)] {
                let signed = source.small_signed_commitment_v1(record, role, block)?;
                let negative =
                    source.small_negative_magnitude_commitment_v1(record, role, block)?;
                let positive = signed + negative;
                if positive.is_identity() {
                    return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
                }
                push_public_term_v1(&mut positive_terms, coefficient, positive, q)?;
                push_public_term_v1(&mut negative_terms, coefficient, negative, q)?;
            }
            let final_borrow = source.comparator_final_borrow_commitment_v1(record, block)?;
            let sigma = one_commitment - final_borrow;
            push_public_term_v1(&mut negative_terms, e0_weight, sigma, q)?;
            block_power = mod_mul_v1(block_power, block_step, q);
        }
        gamma_power = mod_mul_v1(gamma_power, evaluation.gamma, q);
    }

    let mut block_power = 1;
    for block in 0..BLOCKS_PER_RECORD_V1 {
        let mut radix_power = 1;
        for digit in 0..Q_MASK_DIGITS_V1 {
            let coefficient = mod_mul_v1(mod_mul_v1(mask_factor, radix_power, q), block_power, q);
            push_public_term_v1(
                &mut positive_terms,
                coefficient,
                source.q_mask_s_digit_commitment_v1(
                    usize::from(evaluation.limb),
                    usize::from(evaluation.repetition),
                    block,
                    digit,
                )?,
                q,
            )?;
            radix_power = mod_mul_v1(radix_power, RADIX_BASE_V1, q);
        }
        block_power = mod_mul_v1(block_power, block_step, q);
    }

    if positive_terms.len() != POSITIVE_TERMS_PER_COORDINATE_V1
        || negative_terms.len() != NEGATIVE_TERMS_PER_COORDINATE_V1
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let positive = multiexp::<ZkAmsT256BulletproofSuiteV1>(&positive_terms);
    let negative = multiexp::<ZkAmsT256BulletproofSuiteV1>(&negative_terms);
    if positive.is_identity() || negative.is_identity() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
    }
    Ok(DerivedCommitmentsV1 { positive, negative })
}

fn validate_numeric_evaluation_v1(
    limb: usize,
    repetition: usize,
    modulus: u64,
    challenges: RelationChallengesV1,
    numeric: RnsNativeCrossFieldNumericEvaluationV1,
) -> Result<ValidatedEvaluationV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    if limb >= LIMBS_V1
        || repetition >= REPETITIONS_V1
        || numeric.a != challenges.point
        || challenges.gamma == 0
        || challenges.beta == 0
        || challenges.gamma == challenges.beta
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation);
    }
    if [
        numeric.a,
        numeric.public_a,
        numeric.public_b,
        numeric.qpcs_product,
        numeric.qpcs_opening_quotient,
    ]
    .iter()
    .chain(numeric.ciphertext_c0.iter())
    .chain(numeric.ciphertext_c1.iter())
    .any(|value| *value >= modulus)
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation);
    }
    let factor = mod_add_v1(
        mod_pow_v1(numeric.a, RING_DEGREE_V1 as u64, modulus),
        1,
        modulus,
    );
    if factor == 0
        || numeric.qpcs_product != mod_mul_v1(factor, numeric.qpcs_opening_quotient, modulus)
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation);
    }
    let key_evaluation = mod_add_v1(
        numeric.public_b,
        mod_mul_v1(challenges.beta, numeric.public_a, modulus),
        modulus,
    );
    let mut ciphertext_evaluation = 0;
    let mut gamma_power = 1;
    for record in 0..RECORDS_V1 {
        let c = mod_add_v1(
            numeric.ciphertext_c0[record],
            mod_mul_v1(challenges.beta, numeric.ciphertext_c1[record], modulus),
            modulus,
        );
        ciphertext_evaluation = mod_add_v1(
            ciphertext_evaluation,
            mod_mul_v1(gamma_power, c, modulus),
            modulus,
        );
        gamma_power = mod_mul_v1(gamma_power, challenges.gamma, modulus);
    }
    Ok(ValidatedEvaluationV1 {
        limb: limb as u8,
        repetition: repetition as u8,
        modulus,
        gamma: challenges.gamma,
        beta: challenges.beta,
        point: challenges.point,
        public_a: numeric.public_a,
        public_b: numeric.public_b,
        key_evaluation,
        ciphertext_evaluation,
        qpcs_product: numeric.qpcs_product,
        qpcs_opening_quotient: numeric.qpcs_opening_quotient,
    })
}

fn absorb_numeric_evaluation_v1(
    hash: &mut Keccak256,
    ordinal: usize,
    evaluation: ValidatedEvaluationV1,
    numeric: &RnsNativeCrossFieldNumericEvaluationV1,
) {
    hash.update(&(ordinal as u16).to_be_bytes());
    hash.update(&[evaluation.limb, evaluation.repetition]);
    for value in [
        evaluation.modulus,
        evaluation.gamma,
        evaluation.beta,
        numeric.a,
        numeric.public_a,
        numeric.public_b,
        evaluation.key_evaluation,
        evaluation.ciphertext_evaluation,
        numeric.qpcs_product,
        numeric.qpcs_opening_quotient,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for record in 0..RECORDS_V1 {
        hash.update(&(record as u16).to_be_bytes());
        hash.update(&numeric.ciphertext_c0[record].to_be_bytes());
        hash.update(&numeric.ciphertext_c1[record].to_be_bytes());
    }
}

fn prepare_inputs_v1<P: RnsNativeCrossFieldAuthoritativeSourceV1>(
    schedule: RelationScheduleV1,
    source: &mut P,
) -> Result<PreparedInputsV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    validate_relation_schedule_v1(&schedule)?;
    let axes = &schedule.bound.axes;
    axes.validate_v1()?;
    if source.authoritative_binding_digest_v1() != axes.source_binding_digest {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    if q_mask_s_root_v1(axes.pre_qpcs_safe_axes_v1(), source)? != schedule.bound.q_mask_s_root {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }

    let mut numeric_hash = Keccak256::new();
    numeric_hash.update(NUMERIC_ROOT_DOMAIN_V1);
    numeric_hash.update(&[VERSION_V1]);
    numeric_hash.update(&schedule.bound.fixed_axes_digest);
    numeric_hash.update(&schedule.bound.binding_digest);
    numeric_hash.update(&schedule.relation_seed);
    numeric_hash.update(&(EVALUATIONS_V1 as u16).to_be_bytes());
    let mut commitment_hash = Keccak256::new();
    commitment_hash.update(COMMITMENT_ROOT_DOMAIN_V1);
    commitment_hash.update(&[VERSION_V1]);
    commitment_hash.update(&schedule.bound.fixed_axes_digest);
    commitment_hash.update(&schedule.bound.binding_digest);
    commitment_hash.update(&(2 * EVALUATIONS_V1 as u16).to_be_bytes());

    let mut evaluations = Vec::new();
    evaluations
        .try_reserve_exact(EVALUATIONS_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    let mut commitments = Vec::new();
    commitments
        .try_reserve_exact(EVALUATIONS_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    for limb in 0..LIMBS_V1 {
        let modulus = release_modulus_v1(limb)?;
        for repetition in 0..REPETITIONS_V1 {
            let ordinal = limb * REPETITIONS_V1 + repetition;
            let challenges = relation_challenges_v1(&schedule, limb, repetition, modulus)?;
            let mut numeric = RnsNativeCrossFieldNumericEvaluationV1::default();
            source.take_numeric_evaluation_v1(limb, repetition, &mut numeric)?;
            let evaluation =
                validate_numeric_evaluation_v1(limb, repetition, modulus, challenges, numeric)?;
            absorb_numeric_evaluation_v1(&mut numeric_hash, ordinal, evaluation, &numeric);
            let derived = derive_commitments_v1(source, evaluation)?;
            commitment_hash.update(&(ordinal as u16).to_be_bytes());
            commitment_hash.update(&point_bytes_v1(derived.positive)?);
            commitment_hash.update(&point_bytes_v1(derived.negative)?);
            evaluations.push(evaluation);
            commitments.push(derived);
        }
    }
    if evaluations.len() != EVALUATIONS_V1 || commitments.len() != EVALUATIONS_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let numeric_root = numeric_hash.finalize();
    commitment_hash.update(&numeric_root);
    let commitment_root = commitment_hash.finalize();
    if !nonzero_distinct_digests_v1(&[
        schedule.bound.fixed_axes_digest,
        schedule.bound.q_mask_s_root,
        schedule.bound.binding_digest,
        schedule.relation_seed,
        numeric_root,
        commitment_root,
    ]) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(PreparedInputsV1 {
        schedule,
        evaluations: evaluations
            .try_into()
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry)?,
        commitments: commitments
            .try_into()
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry)?,
        numeric_root,
        commitment_root,
    })
}

fn boolean_constraints_v1(gate: usize) -> [LinComb<Scalar>; 2] {
    [
        LinComb::empty()
            .term(Scalar::one(), Variable::aL(gate))
            .term(-Scalar::one(), Variable::aR(gate)),
        LinComb::empty()
            .term(Scalar::one(), Variable::aO(gate))
            .term(-Scalar::one(), Variable::aL(gate)),
    ]
}

fn build_core_statement_v1<S: ProofSuite<Scalar = Scalar, Point = Point>>(
    inputs: &PreparedInputsV1,
    core: usize,
) -> Result<ArithmeticCircuitStatement<'static, S>, RnsNativeCrossFieldRlweDirectErrorV1> {
    if core >= CORES_V1
        || inputs.evaluations.len() != EVALUATIONS_V1
        || inputs.commitments.len() != EVALUATIONS_V1
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let first = core * EVALUATIONS_PER_CORE_V1;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(CONSTRAINTS_PER_CORE_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    let mut vector_commitments = Vec::new();
    vector_commitments
        .try_reserve_exact(VECTOR_COMMITMENTS_PER_CORE_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;

    for local in 0..EVALUATIONS_PER_CORE_V1 {
        let ordinal = first + local;
        let evaluation = inputs.evaluations[ordinal];
        let derived = inputs.commitments[ordinal];
        vector_commitments.push(derived.positive);
        vector_commitments.push(derived.negative);
        let gate_base = local * GATES_PER_EVALUATION_V1;
        for gate in gate_base..gate_base + GATES_PER_EVALUATION_V1 {
            constraints.extend(boolean_constraints_v1(gate));
        }

        let mut relation = LinComb::empty().constant(-Scalar::from_u64(evaluation.public_y_v1()));
        let mut point_power = 1;
        for index in 0..BLOCK_COORDINATES_V1 {
            let weight = Scalar::from_u64(point_power);
            relation = relation
                .term(
                    weight,
                    Variable::CG {
                        commitment: 2 * local,
                        index,
                    },
                )
                .term(
                    -weight,
                    Variable::CG {
                        commitment: 2 * local + 1,
                        index,
                    },
                );
            point_power = mod_mul_v1(point_power, evaluation.point, evaluation.modulus);
        }
        let mut quotient_weight = Scalar::from_u64(evaluation.modulus);
        for bit in 0..QUOTIENT_BITS_V1 {
            relation = relation
                .term(-quotient_weight, Variable::aL(gate_base + bit))
                .term(
                    quotient_weight,
                    Variable::aL(gate_base + QUOTIENT_BITS_V1 + bit),
                );
            quotient_weight += quotient_weight;
        }
        constraints.push(relation);
    }
    if constraints.len() != CONSTRAINTS_PER_CORE_V1
        || vector_commitments.len() != VECTOR_COMMITMENTS_PER_CORE_V1
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    Ok(ArithmeticCircuitStatement::new(
        S::generators().reduce(PADDED_GATES_PER_CORE_V1)?,
        constraints,
        vector_commitments,
        Vec::new(),
    )?)
}

fn append_frame_v1(
    state: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    state.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    state.extend_from_slice(value);
    Ok(())
}

fn initial_core_transcript_state_v1(
    inputs: &PreparedInputsV1,
    core: usize,
) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1> {
    if core >= CORES_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    let first = core * EVALUATIONS_PER_CORE_V1;
    let mut state = Vec::new();
    state
        .try_reserve_exact(8_192)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    for value in [
        CORE_TRANSCRIPT_DOMAIN_V1,
        &[VERSION_V1],
        GEOMETRY_LANGUAGE_V1,
        RELATION_LANGUAGE_V1,
        DERIVATION_LANGUAGE_V1,
        TRANSCRIPT_LANGUAGE_V1,
        manifest_digest_v1().as_slice(),
        inputs.schedule.bound.fixed_axes_digest.as_slice(),
        inputs.schedule.bound.q_mask_s_root.as_slice(),
        inputs.schedule.bound.binding_digest.as_slice(),
        inputs.schedule.relation_seed.as_slice(),
        inputs.numeric_root.as_slice(),
        inputs.commitment_root.as_slice(),
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1.as_slice(),
    ] {
        append_frame_v1(&mut state, value)?;
    }
    append_frame_v1(&mut state, &[core as u8])?;
    append_frame_v1(&mut state, &(first as u16).to_be_bytes())?;
    append_frame_v1(&mut state, &[EVALUATIONS_PER_CORE_V1 as u8])?;
    for local in 0..EVALUATIONS_PER_CORE_V1 {
        let ordinal = first + local;
        let evaluation = inputs.evaluations[ordinal];
        let derived = inputs.commitments[ordinal];
        append_frame_v1(&mut state, &(ordinal as u16).to_be_bytes())?;
        append_frame_v1(&mut state, &[evaluation.limb, evaluation.repetition])?;
        for value in [
            evaluation.modulus,
            evaluation.gamma,
            evaluation.beta,
            evaluation.point,
            evaluation.public_a,
            evaluation.public_b,
            evaluation.key_evaluation,
            evaluation.ciphertext_evaluation,
            evaluation.qpcs_product,
            evaluation.qpcs_opening_quotient,
            evaluation.public_y_v1(),
        ] {
            append_frame_v1(&mut state, &value.to_be_bytes())?;
        }
        append_frame_v1(&mut state, &point_bytes_v1(derived.positive)?)?;
        append_frame_v1(&mut state, &point_bytes_v1(derived.negative)?)?;
    }
    Ok(state)
}

fn derive_nonzero_t256_challenge_v1(
    state: &mut Vec<u8>,
    ordinal: u32,
) -> Result<Scalar, RnsNativeCrossFieldRlweDirectErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut prefix = Vec::with_capacity(CORE_CHALLENGE_DOMAIN_V1.len() + state.len() + 8);
        prefix.extend_from_slice(CORE_CHALLENGE_DOMAIN_V1);
        prefix.extend_from_slice(state);
        prefix.extend_from_slice(&ordinal.to_be_bytes());
        prefix.push(attempt);
        let mut left = prefix.clone();
        left.push(0);
        prefix.push(1);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&keccak256(&left));
        wide[32..].copy_from_slice(&keccak256(&prefix));
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            state.push(2);
            state.extend_from_slice(&ordinal.to_be_bytes());
            state.push(attempt);
            state.extend_from_slice(&challenge.to_le_bytes());
            return Ok(challenge);
        }
    }
    Err(RnsNativeCrossFieldRlweDirectErrorV1::ChallengeExhausted)
}

struct CoreProverTranscriptV1<S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: [u8; CORE_PROOF_BYTES_V1],
    cursor: usize,
    challenge_ordinal: u32,
    _suite: PhantomData<S>,
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> CoreProverTranscriptV1<S> {
    fn new_v1(state: Vec<u8>) -> Self {
        Self {
            state,
            proof: [0; CORE_PROOF_BYTES_V1],
            cursor: 0,
            challenge_ordinal: 0,
            _suite: PhantomData,
        }
    }

    fn push_bytes_v1(&mut self, value: &[u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(value.len())
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let destination = self.proof.get_mut(self.cursor..end).ok_or(
            GeneralizedBulletproofErrorV1::ProofLength {
                actual: end,
                expected: CORE_PROOF_BYTES_V1,
            },
        )?;
        destination.copy_from_slice(value);
        self.cursor = end;
        Ok(())
    }

    fn finish_v1(
        self,
    ) -> Result<
        ([u8; CORE_PROOF_BYTES_V1], [u8; DIGEST_BYTES_V1]),
        RnsNativeCrossFieldRlweDirectErrorV1,
    > {
        if self.cursor != CORE_PROOF_BYTES_V1
            || self.challenge_ordinal as usize != GBP_CHALLENGES_PER_CORE_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidCore);
        }
        Ok((self.proof, keccak256(&self.state)))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> ProverTranscript<S>
    for CoreProverTranscriptV1<S>
{
    fn push_scalar(&mut self, scalar: &Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        with_borrowed_t256_scalar_encoding_v1(scalar, |encoded| {
            self.state.push(0);
            self.state.extend_from_slice(encoded);
            self.push_bytes_v1(encoded)
        })
    }

    fn push_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256PointEncodingV1::new(point)?;
        self.state.push(1);
        self.state.extend_from_slice(encoded.as_ref());
        let result = self.push_bytes_v1(encoded.as_ref());
        drop(encoded);
        result
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let challenge = derive_nonzero_t256_challenge_v1(&mut self.state, self.challenge_ordinal)
            .map_err(|_| GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)?;
        self.challenge_ordinal = self
            .challenge_ordinal
            .checked_add(1)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        Ok(challenge)
    }
}

struct CoreVerifierTranscriptV1<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> {
    state: Vec<u8>,
    proof: &'a [u8],
    cursor: usize,
    challenge_ordinal: u32,
    _suite: PhantomData<S>,
}

impl<'a, S: ProofSuite<Scalar = Scalar, Point = Point>> CoreVerifierTranscriptV1<'a, S> {
    fn new_v1(
        state: Vec<u8>,
        proof: &'a [u8],
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        if proof.len() != CORE_PROOF_BYTES_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidCore);
        }
        Ok(Self {
            state,
            proof,
            cursor: 0,
            challenge_ordinal: 0,
            _suite: PhantomData,
        })
    }

    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let value =
            self.proof
                .get(self.cursor..end)
                .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                    actual: self.proof.len(),
                    expected: end,
                })?;
        self.cursor = end;
        Ok(value)
    }

    fn finish_v1(self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
        if self.cursor != CORE_PROOF_BYTES_V1
            || self.challenge_ordinal as usize != GBP_CHALLENGES_PER_CORE_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidCore);
        }
        Ok(keccak256(&self.state))
    }
}

impl<S: ProofSuite<Scalar = Scalar, Point = Point>> VerifierTranscript<S>
    for CoreVerifierTranscriptV1<'_, S>
{
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; SCALAR_BYTES_V1] = self
            .take_v1(SCALAR_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        Ok(scalar)
    }

    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let encoded: [u8; POINT_BYTES_V1] = self
            .take_v1(POINT_BYTES_V1)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        Ok(point)
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let challenge = derive_nonzero_t256_challenge_v1(&mut self.state, self.challenge_ordinal)
            .map_err(|_| GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)?;
        self.challenge_ordinal = self
            .challenge_ordinal
            .checked_add(1)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        Ok(challenge)
    }
}

struct SecretScalarsV1(Vec<Scalar>);

impl SecretScalarsV1 {
    fn try_zeroed_v1(count: usize) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
        values.resize(count, Scalar::zero());
        Ok(Self(values))
    }

    fn as_slice_v1(&self) -> &[Scalar] {
        &self.0
    }

    fn as_mut_slice_v1(&mut self) -> &mut [Scalar] {
        &mut self.0
    }

    fn into_vec_v1(mut self) -> Vec<Scalar> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for SecretScalarsV1 {
    fn drop(&mut self) {
        for value in &mut self.0 {
            value.clear_secret();
        }
    }
}

struct SecretScalarV1(Scalar);

impl SecretScalarV1 {
    fn zero_v1() -> Self {
        Self(Scalar::zero())
    }

    fn as_mut_v1(&mut self) -> &mut Scalar {
        &mut self.0
    }
}

impl Drop for SecretScalarV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

fn validate_quotient_bits_v1(bits: &[Scalar]) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    if bits.len() != QUOTIENT_BITS_V1
        || bits
            .iter()
            .any(|bit| !bit.is_zero() && *bit != Scalar::one())
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidScalar);
    }
    Ok(())
}

fn build_core_witness_v1<S, P>(
    source: &mut P,
    core: usize,
) -> Result<ArithmeticCircuitWitness<S>, RnsNativeCrossFieldRlweDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeCrossFieldQuotientOpeningSourceV1,
{
    if core >= CORES_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    // Establish the gate owners before the first call and every per-opening
    // destination before its own fallible call.  Previously accepted openings
    // already live in zeroizing `VectorCommitmentOpening` owners, so a partial
    // write, later allocation/source error, or unwind clears all material.
    let mut a_l = SecretScalarsV1::try_zeroed_v1(ACTIVE_GATES_PER_CORE_V1)?;
    let mut a_r = SecretScalarsV1::try_zeroed_v1(ACTIVE_GATES_PER_CORE_V1)?;
    let mut openings = Vec::new();
    openings
        .try_reserve_exact(VECTOR_COMMITMENTS_PER_CORE_V1)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    let first = core * EVALUATIONS_PER_CORE_V1;
    for local in 0..EVALUATIONS_PER_CORE_V1 {
        let ordinal = first + local;
        let limb = ordinal / REPETITIONS_V1;
        let repetition = ordinal % REPETITIONS_V1;
        let gate_base = local * GATES_PER_EVALUATION_V1;

        let mut positive_values = SecretScalarsV1::try_zeroed_v1(BLOCK_COORDINATES_V1)?;
        let mut positive_mask = SecretScalarV1::zero_v1();
        let mut positive_bits = SecretScalarsV1::try_zeroed_v1(QUOTIENT_BITS_V1)?;
        source.take_positive_quotient_owner_v1(
            limb,
            repetition,
            positive_values.as_mut_slice_v1(),
            positive_mask.as_mut_v1(),
            positive_bits.as_mut_slice_v1(),
        )?;
        validate_quotient_bits_v1(positive_bits.as_slice_v1())?;
        for (bit, value) in positive_bits.as_slice_v1().iter().copied().enumerate() {
            a_l.as_mut_slice_v1()[gate_base + bit] = value;
            a_r.as_mut_slice_v1()[gate_base + bit] = value;
        }
        openings.push(VectorCommitmentOpening::take_mask_from_slot(
            positive_values.into_vec_v1(),
            positive_mask.as_mut_v1(),
        ));

        let mut negative_values = SecretScalarsV1::try_zeroed_v1(BLOCK_COORDINATES_V1)?;
        let mut negative_mask = SecretScalarV1::zero_v1();
        let mut negative_bits = SecretScalarsV1::try_zeroed_v1(QUOTIENT_BITS_V1)?;
        source.take_negative_quotient_owner_v1(
            limb,
            repetition,
            negative_values.as_mut_slice_v1(),
            negative_mask.as_mut_v1(),
            negative_bits.as_mut_slice_v1(),
        )?;
        validate_quotient_bits_v1(negative_bits.as_slice_v1())?;
        for (bit, value) in negative_bits.as_slice_v1().iter().copied().enumerate() {
            let gate = gate_base + QUOTIENT_BITS_V1 + bit;
            a_l.as_mut_slice_v1()[gate] = value;
            a_r.as_mut_slice_v1()[gate] = value;
        }
        openings.push(VectorCommitmentOpening::take_mask_from_slot(
            negative_values.into_vec_v1(),
            negative_mask.as_mut_v1(),
        ));
    }
    if openings.len() != VECTOR_COMMITMENTS_PER_CORE_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    Ok(ArithmeticCircuitWitness::<S>::new(
        a_l.into_vec_v1(),
        a_r.into_vec_v1(),
        openings,
    )?)
}

#[derive(Clone, Copy)]
struct SuccessorPreflightV1 {
    successor_len: usize,
    total_len: usize,
}

impl SuccessorPreflightV1 {
    fn new_v1(successor: &[u8]) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        if successor.is_empty() {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader);
        }
        if successor.len() > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1
            || OWNED_WIRE_BYTES_V1 > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_MAX_BYTES_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded);
        }
        let total_len = OWNED_WIRE_BYTES_V1
            .checked_add(successor.len())
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?;
        if total_len > RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded);
        }
        Ok(Self {
            successor_len: successor.len(),
            total_len,
        })
    }

    fn validate_v1(self, successor: &[u8]) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        if self.successor_len != successor.len()
            || self.total_len
                != OWNED_WIRE_BYTES_V1
                    .checked_add(successor.len())
                    .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        Ok(())
    }
}

fn successor_digest_v1(
    fixed_axes_digest: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
    successor: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    let preflight = SuccessorPreflightV1::new_v1(successor)?;
    let mut hash = Keccak256::new();
    hash.update(SUCCESSOR_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&fixed_axes_digest);
    hash.update(&commitment_root);
    hash.update(&(preflight.successor_len as u32).to_be_bytes());
    hash.update(successor);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn proof_set_digest_v1(
    inputs: &PreparedInputsV1,
    proofs: &[&[u8]; CORES_V1],
    transcript_digests: &[[u8; DIGEST_BYTES_V1]; CORES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    if transcript_digests.contains(&[0; DIGEST_BYTES_V1]) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_SET_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&inputs.schedule.bound.fixed_axes_digest);
    hash.update(&inputs.schedule.bound.binding_digest);
    hash.update(&inputs.numeric_root);
    hash.update(&inputs.commitment_root);
    hash.update(&(CORES_V1 as u16).to_be_bytes());
    for core in 0..CORES_V1 {
        hash.update(&[core as u8]);
        hash.update(&((core * EVALUATIONS_PER_CORE_V1) as u16).to_be_bytes());
        hash.update(&[EVALUATIONS_PER_CORE_V1 as u8]);
        hash.update(&(CORE_PROOF_BYTES_V1 as u16).to_be_bytes());
        hash.update(&transcript_digests[core]);
        hash.update(&proofs[core]);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn core_transcript_set_digest_v1(
    transcript_digests: &[[u8; DIGEST_BYTES_V1]; CORES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    if !nonzero_distinct_digests_v1(transcript_digests) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let mut hash = Keccak256::new();
    hash.update(CORE_TRANSCRIPT_SET_DOMAIN_V1);
    hash.update(&[VERSION_V1, CORES_V1 as u8]);
    for (core, digest) in transcript_digests.iter().enumerate() {
        hash.update(&[core as u8]);
        hash.update(&((core * EVALUATIONS_PER_CORE_V1) as u16).to_be_bytes());
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] || transcript_digests.contains(&digest) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Opaque successor-independent root capability for the four verified cores.
///
/// The inner digest is deliberately private and has no raw accessor. It can
/// enter the terminal transcript only through the consuming typed bind below.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the undeclared staged direct adapter moves this capability exactly once"
)]
#[derive(PartialEq, Eq)]
#[must_use = "the opaque core root must be consumed by the typed terminal bind"]
pub(super) struct RnsNativeCrossFieldRlweCoreRootV1([u8; DIGEST_BYTES_V1]);

/// Direct-owned opaque evidence that the verifier recomputed the four-core
/// root only after all four proofs and the proof-set binding succeeded.
///
/// It deliberately has no raw accessor or public constructor. The transcript
/// accepts this exact concrete type when consuming its claimed-root equality
/// obligation; no sibling can synthesize a discharging value from raw bytes.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "verified root evidence is consumed exactly once by the transcript obligation"
)]
#[must_use = "verified root evidence must remain paired with its claimed-root obligation"]
pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1(RnsNativeCrossFieldRlweCoreRootV1);

impl RnsNativeCrossFieldRlweVerifiedCoreRootV1 {
    /// Compare this direct-verifier-owned root with the transcript's private
    /// claim inputs without exposing the recomputed digest.
    pub(super) fn matches_claimed_cross_field_root_v1(
        self,
        claimed_root: [u8; DIGEST_BYTES_V1],
        qpcs_bound_transcript_state: [u8; DIGEST_BYTES_V1],
    ) -> bool {
        let recomputed_root = self.0.0;
        recomputed_root != [0; DIGEST_BYTES_V1]
            && recomputed_root != qpcs_bound_transcript_state
            && recomputed_root == claimed_root
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(
        root: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        if root == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        Ok(Self(RnsNativeCrossFieldRlweCoreRootV1(root)))
    }
}

fn direct_core_safe_digest_v1(
    private_cross_field_core_root: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    numeric_root: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    if !nonzero_distinct_digests_v1(&[
        private_cross_field_core_root,
        q_mask_s_root,
        numeric_root,
        commitment_root,
    ]) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let mut hash = Keccak256::new();
    hash.update(DIRECT_CORE_SAFE_DOMAIN_V1);
    hash.update(&private_cross_field_core_root);
    hash.update(&q_mask_s_root);
    hash.update(&numeric_root);
    hash.update(&commitment_root);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1]
        || [
            private_cross_field_core_root,
            q_mask_s_root,
            numeric_root,
            commitment_root,
        ]
        .contains(&digest)
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Non-authorizing projection of the direct verifier's successor-independent
/// core. The private verified root is one input to the final digest but is not
/// exposed by this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeCrossFieldRlweSafeCoreProjectionV1 {
    pub(super) terminal_predecessor_context_binding_digest: [u8; DIGEST_BYTES_V1],
    pub(super) candidate_pre_direct_inventory_context_digest: [u8; DIGEST_BYTES_V1],
    pub(super) candidate_pre_direct_inventory_root: [u8; DIGEST_BYTES_V1],
    pub(super) existing_radix_candidate_root: [u8; DIGEST_BYTES_V1],
    pub(super) direct_core_safe_digest: [u8; DIGEST_BYTES_V1],
}

fn cross_field_core_root_v1(
    inputs: &PreparedInputsV1,
    proof_set_digest: [u8; DIGEST_BYTES_V1],
    core_transcript_digest: [u8; DIGEST_BYTES_V1],
) -> Result<RnsNativeCrossFieldRlweCoreRootV1, RnsNativeCrossFieldRlweDirectErrorV1> {
    let identities = [
        inputs.schedule.bound.fixed_axes_digest,
        inputs.schedule.bound.q_mask_s_root,
        inputs.schedule.bound.binding_digest,
        inputs.schedule.relation_seed,
        inputs.numeric_root,
        inputs.commitment_root,
        proof_set_digest,
        core_transcript_digest,
    ];
    if !nonzero_distinct_digests_v1(&identities) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let mut hash = Keccak256::new();
    hash.update(CROSS_FIELD_CORE_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1, CORES_V1 as u8]);
    hash.update(&manifest_digest_v1());
    for digest in identities {
        hash.update(&digest);
    }
    let root = hash.finalize();
    if root == [0; DIGEST_BYTES_V1] || identities.contains(&root) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(RnsNativeCrossFieldRlweCoreRootV1(root))
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

#[cfg(test)]
fn encode_wire_v1(
    inputs: &PreparedInputsV1,
    proofs: &[[u8; CORE_PROOF_BYTES_V1]; CORES_V1],
    transcript_digests: &[[u8; DIGEST_BYTES_V1]; CORES_V1],
    successor: &[u8],
) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1> {
    let successor_preflight = SuccessorPreflightV1::new_v1(successor)?;
    let proof_refs = core::array::from_fn(|core| proofs[core].as_slice());
    let proof_set_digest = proof_set_digest_v1(inputs, &proof_refs, transcript_digests)?;
    encode_wire_preflighted_v1(
        inputs,
        proofs,
        transcript_digests,
        proof_set_digest,
        successor,
        successor_preflight,
    )
}

fn encode_wire_preflighted_v1(
    inputs: &PreparedInputsV1,
    proofs: &[[u8; CORE_PROOF_BYTES_V1]; CORES_V1],
    transcript_digests: &[[u8; DIGEST_BYTES_V1]; CORES_V1],
    proof_set_digest: [u8; DIGEST_BYTES_V1],
    successor: &[u8],
    successor_preflight: SuccessorPreflightV1,
) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1> {
    successor_preflight.validate_v1(successor)?;
    let proof_refs = core::array::from_fn(|core| proofs[core].as_slice());
    if proof_set_digest_v1(inputs, &proof_refs, transcript_digests)? != proof_set_digest {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let successor_digest = successor_digest_v1(
        inputs.schedule.bound.fixed_axes_digest,
        inputs.commitment_root,
        successor,
    )?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(successor_preflight.total_len)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ResourceExhausted)?;
    wire.extend_from_slice(&MAGIC_V1);
    wire.push(VERSION_V1);
    wire.push(FLAGS_V1);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(OWNED_WIRE_BYTES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&[
        LIMBS_V1 as u8,
        REPETITIONS_V1 as u8,
        CORES_V1 as u8,
        EVALUATIONS_PER_CORE_V1 as u8,
        QUOTIENT_BITS_V1 as u8,
    ]);
    wire.extend_from_slice(&(ACTIVE_GATES_PER_CORE_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(PADDED_GATES_PER_CORE_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(CONSTRAINTS_PER_CORE_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(VECTOR_COMMITMENTS_PER_CORE_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(CORE_PROOF_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(ALL_CORE_PROOF_BYTES_V1 as u32).to_be_bytes());
    for digest in [
        inputs.schedule.bound.fixed_axes_digest,
        inputs.schedule.bound.q_mask_s_root,
        inputs.schedule.bound.binding_digest,
        inputs.schedule.relation_seed,
        inputs.numeric_root,
        inputs.commitment_root,
        proof_set_digest,
        successor_digest,
    ] {
        wire.extend_from_slice(&digest);
    }
    wire.extend_from_slice(&(successor_preflight.successor_len as u32).to_be_bytes());
    if wire.len() != HEADER_BYTES_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    for core in 0..CORES_V1 {
        wire.push(core as u8);
        wire.extend_from_slice(&((core * EVALUATIONS_PER_CORE_V1) as u16).to_be_bytes());
        wire.push(EVALUATIONS_PER_CORE_V1 as u8);
        wire.extend_from_slice(&(CORE_PROOF_BYTES_V1 as u16).to_be_bytes());
        wire.extend_from_slice(&proofs[core]);
    }
    let codec_digest = codec_digest_v1(&wire);
    wire.extend_from_slice(&codec_digest);
    if wire.len() != OWNED_WIRE_BYTES_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    wire.extend_from_slice(successor);
    if wire.len() != successor_preflight.total_len {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    Ok(wire)
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take_v1(&mut self, count: usize) -> Result<&'a [u8], RnsNativeCrossFieldRlweDirectErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array_v1<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeCrossFieldRlweDirectErrorV1> {
        self.take_v1(N)?
            .try_into()
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader)
    }

    fn u8_v1(&mut self) -> Result<u8, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.take_v1(1)?
            .first()
            .copied()
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader)
    }

    fn u16_v1(&mut self) -> Result<u16, RnsNativeCrossFieldRlweDirectErrorV1> {
        Ok(u16::from_be_bytes(self.array_v1()?))
    }

    fn u32_v1(&mut self) -> Result<u32, RnsNativeCrossFieldRlweDirectErrorV1> {
        Ok(u32::from_be_bytes(self.array_v1()?))
    }
}

fn validate_core_proof_codec_v1(proof: &[u8]) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    if proof.len() != CORE_PROOF_BYTES_V1 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader);
    }
    let mut decoder = DecoderV1::new(proof);
    for _ in 0..FIXED_PROOF_POINTS_V1 {
        let encoded = decoder.array_v1::<POINT_BYTES_V1>()?;
        Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)?;
    }
    for _ in 0..CIRCUIT_PROOF_SCALARS_V1 {
        Scalar::from_le_bytes_exact(decoder.array_v1::<SCALAR_BYTES_V1>()?)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidScalar)?;
    }
    for _ in 0..IPA_PROOF_POINTS_V1 {
        let encoded = decoder.array_v1::<POINT_BYTES_V1>()?;
        Point::from_non_identity_wire_bytes_exact(&encoded)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)?;
    }
    for _ in 0..IPA_FINAL_SCALARS_V1 {
        Scalar::from_le_bytes_exact(decoder.array_v1::<SCALAR_BYTES_V1>()?)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidScalar)?;
    }
    if decoder.cursor != proof.len() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
    }
    Ok(())
}

/// Opaque one-shot claim to the exact successor structurally authenticated by
/// a direct frame preflight. It does not assert the direct algebra; only the
/// generic carrier may reveal its borrow.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "a validated successor claim must mint exactly one carrier"
)]
pub(super) struct RnsNativeCrossFieldRlweClaimedSuccessorSliceV1<'proof> {
    successor: &'proof [u8],
}

impl<'proof> RnsNativeCrossFieldRlweClaimedSuccessorSliceV1<'proof> {
    pub(super) const fn into_borrowed_successor_v1(self) -> &'proof [u8] {
        self.successor
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(
        successor: &'proof [u8],
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        if successor.is_empty()
            || successor.len() > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded);
        }
        Ok(Self { successor })
    }
}

struct FramePreflightV1<'a> {
    fixed_axes_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    direct_schedule_binding_digest: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    numeric_root: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
    proof_set_digest: [u8; DIGEST_BYTES_V1],
    successor_digest: [u8; DIGEST_BYTES_V1],
    core_proofs: [&'a [u8]; CORES_V1],
    successor: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> FramePreflightV1<'a> {
    fn decode_exact_v1(bytes: &'a [u8]) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        if OWNED_WIRE_BYTES_V1 > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_MAX_BYTES_V1
            || bytes.len() > RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array_v1::<4>()? != MAGIC_V1
            || decoder.u8_v1()? != VERSION_V1
            || decoder.u8_v1()? != FLAGS_V1
            || usize::from(decoder.u16_v1()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32_v1()?)
                .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?
                != OWNED_WIRE_BYTES_V1
            || usize::from(decoder.u8_v1()?) != LIMBS_V1
            || usize::from(decoder.u8_v1()?) != REPETITIONS_V1
            || usize::from(decoder.u8_v1()?) != CORES_V1
            || usize::from(decoder.u8_v1()?) != EVALUATIONS_PER_CORE_V1
            || usize::from(decoder.u8_v1()?) != QUOTIENT_BITS_V1
            || usize::from(decoder.u16_v1()?) != ACTIVE_GATES_PER_CORE_V1
            || usize::from(decoder.u16_v1()?) != PADDED_GATES_PER_CORE_V1
            || usize::from(decoder.u16_v1()?) != CONSTRAINTS_PER_CORE_V1
            || usize::from(decoder.u16_v1()?) != VECTOR_COMMITMENTS_PER_CORE_V1
            || usize::from(decoder.u16_v1()?) != CORE_PROOF_BYTES_V1
            || usize::try_from(decoder.u32_v1()?)
                .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?
                != ALL_CORE_PROOF_BYTES_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        let fixed_axes_digest = decoder.array_v1()?;
        let q_mask_s_root = decoder.array_v1()?;
        let direct_schedule_binding_digest = decoder.array_v1()?;
        let relation_seed = decoder.array_v1()?;
        let numeric_root = decoder.array_v1()?;
        let commitment_root = decoder.array_v1()?;
        let proof_set_digest = decoder.array_v1()?;
        let successor_digest = decoder.array_v1()?;
        let successor_len = usize::try_from(decoder.u32_v1()?)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?;
        if decoder.cursor != HEADER_BYTES_V1
            || !nonzero_distinct_digests_v1(&[
                fixed_axes_digest,
                q_mask_s_root,
                direct_schedule_binding_digest,
                relation_seed,
                numeric_root,
                commitment_root,
                proof_set_digest,
                successor_digest,
            ])
            || successor_len == 0
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        if successor_len > RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded);
        }
        let expected_total = OWNED_WIRE_BYTES_V1
            .checked_add(successor_len)
            .ok_or(RnsNativeCrossFieldRlweDirectErrorV1::ArithmeticOverflow)?;
        if expected_total != bytes.len()
            || expected_total > RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader);
        }
        let mut core_proofs = [&[][..]; CORES_V1];
        for core in 0..CORES_V1 {
            if usize::from(decoder.u8_v1()?) != core
                || usize::from(decoder.u16_v1()?) != core * EVALUATIONS_PER_CORE_V1
                || usize::from(decoder.u8_v1()?) != EVALUATIONS_PER_CORE_V1
                || usize::from(decoder.u16_v1()?) != CORE_PROOF_BYTES_V1
            {
                return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
            }
            core_proofs[core] = decoder.take_v1(CORE_PROOF_BYTES_V1)?;
        }
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array_v1()?;
        if decoder.cursor != OWNED_WIRE_BYTES_V1
            || codec_digest == [0; DIGEST_BYTES_V1]
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        for proof in core_proofs {
            validate_core_proof_codec_v1(proof)?;
        }
        let successor = decoder.take_v1(successor_len)?;
        if decoder.cursor != bytes.len()
            || successor_digest_v1(fixed_axes_digest, commitment_root, successor)?
                != successor_digest
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        Ok(Self {
            fixed_axes_digest,
            q_mask_s_root,
            direct_schedule_binding_digest,
            relation_seed,
            numeric_root,
            commitment_root,
            proof_set_digest,
            successor_digest,
            core_proofs,
            successor,
            codec_digest,
        })
    }

    fn validate_schedule_v1(
        &self,
        schedule: &RelationScheduleV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        validate_relation_schedule_v1(schedule)?;
        if self.fixed_axes_digest != schedule.bound.fixed_axes_digest
            || self.q_mask_s_root != schedule.bound.q_mask_s_root
            || self.direct_schedule_binding_digest != schedule.bound.binding_digest
            || self.relation_seed != schedule.relation_seed
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        Ok(())
    }

    const fn claimed_successor_slice_v1(
        &self,
    ) -> RnsNativeCrossFieldRlweClaimedSuccessorSliceV1<'a> {
        RnsNativeCrossFieldRlweClaimedSuccessorSliceV1 {
            successor: self.successor,
        }
    }

    fn bind_inputs_v1(
        self,
        inputs: &PreparedInputsV1,
    ) -> Result<FrameViewV1<'a>, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.validate_schedule_v1(&inputs.schedule)?;
        if self.numeric_root != inputs.numeric_root
            || self.commitment_root != inputs.commitment_root
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
        }
        Ok(FrameViewV1 {
            proof_set_digest: self.proof_set_digest,
            successor_digest: self.successor_digest,
            core_proofs: self.core_proofs,
            successor: self.successor,
            codec_digest: self.codec_digest,
        })
    }
}

fn validate_claimed_inventory_transcript_v1(
    claimed_relation: &RnsNativeCrossFieldRlweClaimedRelationV1,
    inventory_terminal_transcript_digest: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    if inventory_terminal_transcript_digest == [0; DIGEST_BYTES_V1]
        || claimed_relation.final_challenge_seeds.transcript_digest()
            != inventory_terminal_transcript_digest
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    Ok(())
}

/// Consume the exact claimed relation and authenticated inventory, require
/// their final terminal transcript identity, and exact-preflight the existing
/// direct frame before minting its sole successor carrier.
#[allow(
    dead_code,
    reason = "the authoritative direct verifier will later consume the retained frame core"
)]
pub(super) fn preflight_rns_native_cross_field_rlwe_claimed_frame_v1<'source, 'proof, S>(
    claimed_relation: RnsNativeCrossFieldRlweClaimedRelationV1,
    inventory: RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    >,
    RnsNativeCrossFieldRlweDirectErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    validate_claimed_inventory_transcript_v1(
        &claimed_relation,
        inventory.terminal_transcript_digest_v1(),
    )?;
    let preflight = FramePreflightV1::decode_exact_v1(inventory.continuation())?;
    preflight.validate_schedule_v1(&claimed_relation.schedule)?;
    Ok(RnsNativeCrossFieldRlweClaimedFramePreflightV1 {
        frame_core: RnsNativeCrossFieldRlweClaimedFrameCoreV1 {
            claimed_relation,
            preflight,
        },
        inventory,
    }
    .into_claimed_successor_v1())
}

struct FrameViewV1<'a> {
    proof_set_digest: [u8; DIGEST_BYTES_V1],
    successor_digest: [u8; DIGEST_BYTES_V1],
    core_proofs: [&'a [u8]; CORES_V1],
    successor: &'a [u8],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> FrameViewV1<'a> {
    #[cfg(test)]
    fn decode_exact_v1(
        bytes: &'a [u8],
        inputs: &PreparedInputsV1,
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        FramePreflightV1::decode_exact_v1(bytes)?.bind_inputs_v1(inputs)
    }
}

fn final_binding_digest_v1(
    inputs: &PreparedInputsV1,
    view: &FrameViewV1<'_>,
    transcript_digests: &[[u8; DIGEST_BYTES_V1]; CORES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeCrossFieldRlweDirectErrorV1> {
    let core_transcript_digest = core_transcript_set_digest_v1(transcript_digests)?;
    let cross_field_core_root =
        cross_field_core_root_v1(inputs, view.proof_set_digest, core_transcript_digest)?;
    let mut hash = Keccak256::new();
    hash.update(BINDING_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for digest in [
        manifest_digest_v1(),
        inputs.schedule.bound.fixed_axes_digest,
        inputs.schedule.bound.q_mask_s_root,
        inputs.schedule.bound.binding_digest,
        inputs.schedule.relation_seed,
        inputs.numeric_root,
        inputs.commitment_root,
        view.proof_set_digest,
        core_transcript_digest,
        cross_field_core_root.0,
        view.successor_digest,
        view.codec_digest,
    ] {
        hash.update(&digest);
    }
    for digest in transcript_digests {
        hash.update(digest);
    }
    hash.update(&(view.successor.len() as u32).to_be_bytes());
    let binding = hash.finalize();
    if binding == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    Ok(binding)
}

/// Move-only four-core proof owner awaiting its typed terminal-transcript bind.
///
/// Neither the successor-independent root nor the private transcript-set
/// digest is exposed as an interchangeable byte array. This owner must be
/// consumed by `bind_to_terminal_transcript_v1` before it can be sealed.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the undeclared staged direct adapter will consume this owner exactly once"
)]
#[must_use = "the four-core owner must be consumed by the typed terminal bind"]
pub(super) struct RnsNativeCrossFieldRlweFourCorePendingSealV1 {
    inputs: PreparedInputsV1,
    proofs: [[u8; CORE_PROOF_BYTES_V1]; CORES_V1],
    transcript_digests: [[u8; DIGEST_BYTES_V1]; CORES_V1],
    proof_set_digest: [u8; DIGEST_BYTES_V1],
    core_transcript_digest: [u8; DIGEST_BYTES_V1],
    cross_field_core_root: RnsNativeCrossFieldRlweCoreRootV1,
}

impl RnsNativeCrossFieldRlweFourCorePendingSealV1 {
    fn from_parts_v1(
        inputs: PreparedInputsV1,
        proofs: [[u8; CORE_PROOF_BYTES_V1]; CORES_V1],
        transcript_digests: [[u8; DIGEST_BYTES_V1]; CORES_V1],
    ) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        for proof in &proofs {
            validate_core_proof_codec_v1(proof)?;
        }
        let proof_refs = core::array::from_fn(|core| proofs[core].as_slice());
        let proof_set_digest = proof_set_digest_v1(&inputs, &proof_refs, &transcript_digests)?;
        let core_transcript_digest = core_transcript_set_digest_v1(&transcript_digests)?;
        let cross_field_core_root =
            cross_field_core_root_v1(&inputs, proof_set_digest, core_transcript_digest)?;
        Ok(Self {
            inputs,
            proofs,
            transcript_digests,
            proof_set_digest,
            core_transcript_digest,
            cross_field_core_root,
        })
    }

    fn validate_v1(&self) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let proof_refs = core::array::from_fn(|core| self.proofs[core].as_slice());
        let proof_set_digest =
            proof_set_digest_v1(&self.inputs, &proof_refs, &self.transcript_digests)?;
        let core_transcript_digest = core_transcript_set_digest_v1(&self.transcript_digests)?;
        let cross_field_core_root =
            cross_field_core_root_v1(&self.inputs, proof_set_digest, core_transcript_digest)?;
        if proof_set_digest != self.proof_set_digest
            || core_transcript_digest != self.core_transcript_digest
            || cross_field_core_root != self.cross_field_core_root
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
        }
        Ok(())
    }

    #[cfg(test)]
    fn seal_preflighted_v1(
        self,
        successor: &[u8],
        successor_preflight: SuccessorPreflightV1,
    ) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1> {
        successor_preflight.validate_v1(successor)?;
        self.validate_v1()?;
        encode_wire_preflighted_v1(
            &self.inputs,
            &self.proofs,
            &self.transcript_digests,
            self.proof_set_digest,
            successor,
            successor_preflight,
        )
    }
}

/// Four-core owner after its opaque root has been consumed by the exact qPCS
/// terminal transcript stage. Only this owner can seal a successor.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the undeclared staged direct adapter moves this owner into the successor seal"
)]
#[must_use = "the terminal-bound owner must be consumed by the successor seal"]
pub(super) struct RnsNativeCrossFieldRlweTerminalBoundPendingSealV1 {
    inputs: PreparedInputsV1,
    proofs: [[u8; CORE_PROOF_BYTES_V1]; CORES_V1],
    transcript_digests: [[u8; DIGEST_BYTES_V1]; CORES_V1],
    proof_set_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativeCrossFieldRlweTerminalBoundPendingSealV1 {
    /// Consume the terminal-bound owner and seal a non-empty successor built
    /// from the challenge carried by the returned cross-field transcript stage.
    pub(super) fn seal_v1(
        self,
        successor: &[u8],
    ) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1> {
        let successor_preflight = SuccessorPreflightV1::new_v1(successor)?;
        encode_wire_preflighted_v1(
            &self.inputs,
            &self.proofs,
            &self.transcript_digests,
            self.proof_set_digest,
            successor,
            successor_preflight,
        )
    }
}

/// Consume both the four-core owner and the sole qPCS-bound transcript stage,
/// bind the opaque cross-field root, and return the only seal-capable owner.
#[allow(
    dead_code,
    reason = "the undeclared staged direct adapter is the sole production caller"
)]
pub(super) fn bind_to_terminal_transcript_v1(
    mut pending: RnsNativeCrossFieldRlweFourCorePendingSealV1,
) -> Result<
    (
        RnsNativeCrossFieldRlweTerminalBoundPendingSealV1,
        ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1,
    ),
    RnsNativeCrossFieldRlweDirectErrorV1,
> {
    pending.validate_v1()?;
    if pending.inputs.schedule.has_claimed_cross_field_root_v1() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let transcript = pending
        .inputs
        .schedule
        .bound
        .completed_qpcs
        .take_qpcs_transcript_v1()
        .map_err(map_qpcs_complete_error_v1)?;
    let transcript = transcript
        .bind_cross_field_root(pending.cross_field_core_root.0)
        .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)?;
    let RnsNativeCrossFieldRlweFourCorePendingSealV1 {
        inputs,
        proofs,
        transcript_digests,
        proof_set_digest,
        core_transcript_digest: _,
        cross_field_core_root: _,
    } = pending;
    Ok((
        RnsNativeCrossFieldRlweTerminalBoundPendingSealV1 {
            inputs,
            proofs,
            transcript_digests,
            proof_set_digest,
        },
        transcript,
    ))
}

/// Equality-pending owner after all four direct proofs and their successor
/// binding verify.
///
/// The claimed-root obligation and direct-owned verified-root evidence remain
/// privately paired here. Only its consuming concrete equality transition can
/// produce the terminal-bound owner.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the equality evidence remains private until the concrete transition consumes it"
)]
#[must_use = "direct verification remains non-authorizing until claimed-root equality is discharged"]
pub(super) struct RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a> {
    successor: &'a [u8],
    binding_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    numeric_root: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
    safe_core_projection: RnsNativeCrossFieldRlweSafeCoreProjectionV1,
    cross_field_root_equality_obligation: ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1,
    verified_cross_field_core_root: RnsNativeCrossFieldRlweVerifiedCoreRootV1,
}

impl<'a> RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a> {
    /// Consume the sole transcript obligation and direct-owned verified root.
    /// A mismatch consumes both values and returns no terminal-bound owner.
    pub(super) fn discharge_claimed_root_equality_v1(
        self,
    ) -> Result<
        RnsNativeCrossFieldRlweTerminalBoundVerifiedV1<'a>,
        RnsNativeCrossFieldRlweDirectErrorV1,
    > {
        let Self {
            successor,
            binding_digest,
            q_mask_s_root,
            numeric_root,
            commitment_root,
            safe_core_projection,
            cross_field_root_equality_obligation,
            verified_cross_field_core_root,
        } = self;
        cross_field_root_equality_obligation
            .discharge_v1(verified_cross_field_core_root)
            .map_err(|_| RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)?;
        Ok(RnsNativeCrossFieldRlweTerminalBoundVerifiedV1 {
            successor,
            binding_digest,
            q_mask_s_root,
            numeric_root,
            commitment_root,
            safe_core_projection,
        })
    }

    pub(super) const fn successor(&self) -> &'a [u8] {
        self.successor
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    pub(super) const fn q_mask_s_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.q_mask_s_root
    }

    pub(super) const fn numeric_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.numeric_root
    }

    pub(super) const fn commitment_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.commitment_root
    }
}

/// Terminal-bound direct owner after the concrete claimed-root equality has
/// been discharged exactly once.
///
/// This remains a private proof-stage owner and grants no composite, receipt,
/// readiness, or release authority.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the private downstream adapter will consume this stage once"
)]
#[must_use = "the terminal-bound owner must be consumed by the private downstream adapter"]
pub(super) struct RnsNativeCrossFieldRlweTerminalBoundVerifiedV1<'a> {
    successor: &'a [u8],
    binding_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    numeric_root: [u8; DIGEST_BYTES_V1],
    commitment_root: [u8; DIGEST_BYTES_V1],
    safe_core_projection: RnsNativeCrossFieldRlweSafeCoreProjectionV1,
}

impl<'a> RnsNativeCrossFieldRlweTerminalBoundVerifiedV1<'a> {
    pub(super) const fn successor(&self) -> &'a [u8] {
        self.successor
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    pub(super) const fn q_mask_s_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.q_mask_s_root
    }

    pub(super) const fn numeric_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.numeric_root
    }

    pub(super) const fn commitment_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.commitment_root
    }

    pub(super) const fn safe_core_projection_v1(
        &self,
    ) -> RnsNativeCrossFieldRlweSafeCoreProjectionV1 {
        self.safe_core_projection
    }
}

fn prove_pending_kernel_for_suite_v1<S, P, R>(
    schedule: RelationScheduleV1,
    mut source: P,
    rng: &mut R,
) -> Result<RnsNativeCrossFieldRlweFourCorePendingSealV1, RnsNativeCrossFieldRlweDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeCrossFieldQuotientOpeningSourceV1,
    R: ProofRandomSource,
{
    if schedule.has_claimed_cross_field_root_v1()
        || !schedule
            .bound
            .completed_qpcs
            .has_unconsumed_qpcs_transcript_v1()
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let inputs = prepare_inputs_v1(schedule, &mut source)?;
    let mut proofs = [[0_u8; CORE_PROOF_BYTES_V1]; CORES_V1];
    let mut transcript_digests = [[0_u8; DIGEST_BYTES_V1]; CORES_V1];
    for core in 0..CORES_V1 {
        let state = initial_core_transcript_state_v1(&inputs, core)?;
        let mut transcript = CoreProverTranscriptV1::<S>::new_v1(state);
        let witness = build_core_witness_v1::<S, P>(&mut source, core)?;
        build_core_statement_v1::<S>(&inputs, core)?.prove(rng, &mut transcript, witness)?;
        let (proof, transcript_digest) = transcript.finish_v1()?;
        proofs[core] = proof;
        transcript_digests[core] = transcript_digest;
    }
    drop(source);
    RnsNativeCrossFieldRlweFourCorePendingSealV1::from_parts_v1(inputs, proofs, transcript_digests)
}

#[cfg(test)]
fn prove_kernel_for_suite_v1<S, P, R>(
    schedule: RelationScheduleV1,
    source: P,
    successor: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeCrossFieldQuotientOpeningSourceV1,
    R: ProofRandomSource,
{
    let successor_preflight = SuccessorPreflightV1::new_v1(successor)?;
    prove_pending_kernel_for_suite_v1::<S, P, R>(schedule, source, rng)?
        .seal_preflighted_v1(successor, successor_preflight)
}

fn verify_kernel_for_suite_v1<'a, S, P>(
    schedule: RelationScheduleV1,
    mut source: P,
    wire: &'a [u8],
) -> Result<
    RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a>,
    RnsNativeCrossFieldRlweDirectErrorV1,
>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    P: RnsNativeCrossFieldAuthoritativeSourceV1,
{
    let preflight = FramePreflightV1::decode_exact_v1(wire)?;
    preflight.validate_schedule_v1(&schedule)?;
    if !schedule.has_claimed_cross_field_root_v1() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let mut inputs = prepare_inputs_v1(schedule, &mut source)?;
    drop(source);
    let view = preflight.bind_inputs_v1(&inputs)?;
    let mut transcript_digests = [[0_u8; DIGEST_BYTES_V1]; CORES_V1];
    for core in 0..CORES_V1 {
        let state = initial_core_transcript_state_v1(&inputs, core)?;
        let mut transcript = CoreVerifierTranscriptV1::<S>::new_v1(state, view.core_proofs[core])?;
        build_core_statement_v1::<S>(&inputs, core)?.verify(&mut transcript)?;
        transcript_digests[core] = transcript.finish_v1()?;
    }
    if proof_set_digest_v1(&inputs, &view.core_proofs, &transcript_digests)?
        != view.proof_set_digest
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity);
    }
    let core_transcript_digest = core_transcript_set_digest_v1(&transcript_digests)?;
    let cross_field_core_root =
        cross_field_core_root_v1(&inputs, view.proof_set_digest, core_transcript_digest)?;
    let binding_digest = final_binding_digest_v1(&inputs, &view, &transcript_digests)?;
    let safe_core_projection = RnsNativeCrossFieldRlweSafeCoreProjectionV1 {
        terminal_predecessor_context_binding_digest: inputs
            .schedule
            .bound
            .axes
            .terminal_predecessor_binding_digest,
        candidate_pre_direct_inventory_context_digest: inputs
            .schedule
            .bound
            .axes
            .candidate_inventory_axes
            .context_digest,
        candidate_pre_direct_inventory_root: inputs
            .schedule
            .bound
            .axes
            .candidate_inventory_axes
            .inventory_root,
        existing_radix_candidate_root: inputs.schedule.bound.axes.existing_radix_candidate_root,
        direct_core_safe_digest: direct_core_safe_digest_v1(
            cross_field_core_root.0,
            inputs.schedule.bound.q_mask_s_root,
            inputs.numeric_root,
            inputs.commitment_root,
        )?,
    };
    let verified_cross_field_core_root =
        RnsNativeCrossFieldRlweVerifiedCoreRootV1(cross_field_core_root);
    let cross_field_root_equality_obligation = inputs
        .schedule
        .take_cross_field_root_equality_obligation_v1()?;
    Ok(RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1 {
        successor: view.successor,
        binding_digest,
        q_mask_s_root: inputs.schedule.bound.q_mask_s_root,
        numeric_root: inputs.numeric_root,
        commitment_root: inputs.commitment_root,
        safe_core_projection,
        cross_field_root_equality_obligation,
        verified_cross_field_core_root,
    })
}

/// Produce the four successor-independent direct cores. The caller must move
/// this owner through `bind_to_terminal_transcript_v1` before constructing and
/// sealing the global-lookup successor.
#[allow(
    dead_code,
    reason = "private and inactive until the authoritative source/qPCS chronology adapter is implemented"
)]
pub(super) fn prove_rns_native_cross_field_rlwe_direct_pending_v1<P, R>(
    schedule: RelationScheduleV1,
    source: P,
    rng: &mut R,
) -> Result<RnsNativeCrossFieldRlweFourCorePendingSealV1, RnsNativeCrossFieldRlweDirectErrorV1>
where
    P: RnsNativeCrossFieldQuotientOpeningSourceV1,
    R: ProofRandomSource,
{
    prove_pending_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, P, R>(schedule, source, rng)
}

/// Test-only compatibility wrapper for source-preflight regression coverage.
/// It cannot exist in a production build; production must use the typed staged
/// pending API above.
#[cfg(test)]
pub(super) fn prove_rns_native_cross_field_rlwe_direct_v1<P, R>(
    schedule: RelationScheduleV1,
    source: P,
    successor: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, RnsNativeCrossFieldRlweDirectErrorV1>
where
    P: RnsNativeCrossFieldQuotientOpeningSourceV1,
    R: ProofRandomSource,
{
    prove_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, P, R>(schedule, source, successor, rng)
}

/// Private verification kernel returning only a non-authorizing,
/// claimed-root-equality-pending owner.
#[allow(
    dead_code,
    reason = "private and inactive until the authoritative source/qPCS chronology adapter is implemented"
)]
pub(super) fn verify_rns_native_cross_field_rlwe_direct_v1<'a, P>(
    schedule: RelationScheduleV1,
    source: P,
    wire: &'a [u8],
) -> Result<
    RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a>,
    RnsNativeCrossFieldRlweDirectErrorV1,
>
where
    P: RnsNativeCrossFieldAuthoritativeSourceV1,
{
    verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, P>(schedule, source, wire)
}

fn same_borrowed_slice_identity_v1(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && core::ptr::eq(left.as_ptr(), right.as_ptr())
}

fn validate_claimed_handoff_fixed_axes_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    claimed_relation: &RnsNativeCrossFieldRlweClaimedRelationV1,
    inventory: &RnsNativeCrossFieldInventoryPrerequisiteV1<'_, '_, S>,
    expected_existing_radix_candidate_root: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
    validate_claimed_inventory_transcript_v1(
        claimed_relation,
        inventory.terminal_transcript_digest_v1(),
    )?;
    validate_relation_schedule_v1(&claimed_relation.schedule)?;
    let axes = &claimed_relation.schedule.bound.axes;
    let final_transcript = &claimed_relation.final_challenge_seeds;
    let linked = inventory.linked();
    if axes.profile_manifest_digest != final_transcript.profile_manifest_digest()
        || axes.source_binding_digest != final_transcript.source_binding_digest()
        || axes.source_formula_digest != linked.source().formula_digest()
        || axes.source_mapping_digest != linked.source().mapping_digest()
        || axes.terminal_predecessor_binding_digest != linked.terminal().binding_digest()
        || axes.existing_radix_candidate_root != expected_existing_radix_candidate_root
        || axes.rns_aggregation_challenge_seed != final_transcript.rns_aggregation_challenge_seed()
        || axes.qpcs_parameter_digest != linked.source().qpcs().parameter_digest()
        || axes.qpcs_pre_relation_transcript_digest
            != final_transcript.qpcs_pre_relation_transcript_digest()
        || claimed_relation.schedule.bound.q_mask_s_root != final_transcript.q_mask_s_root()
    {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    Ok(())
}

/// Consume the exact direct parent recovered from the membership chain,
/// verify the already-preflighted four-core frame with an internal
/// authoritative source, and discharge its claimed-root equality.
///
/// This is deliberately crate-private and generic only over the still-private
/// source trait. No production source implementation, staged adapter,
/// composite capability, readiness, or release authority is created here.
#[allow(
    dead_code,
    reason = "the generic verifier handoff remains internal until an authenticated production source adapter exists"
)]
pub(super) fn verify_rns_native_cross_field_rlwe_claimed_carrier_v1<'source, 'proof, S, P>(
    parent: RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    exact_claimed_successor: &'proof [u8],
    expected_existing_radix_candidate_root: [u8; DIGEST_BYTES_V1],
    source: P,
) -> Result<
    (
        RnsNativeCrossFieldRlweTerminalBoundVerifiedV1<'proof>,
        RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
    ),
    RnsNativeCrossFieldRlweDirectErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    P: RnsNativeCrossFieldAuthoritativeSourceV1,
{
    let RnsNativeCrossFieldRlweClaimedInventoryParentV1 {
        frame_core,
        inventory,
    } = parent;
    let RnsNativeCrossFieldRlweClaimedFrameCoreV1 {
        claimed_relation,
        preflight,
    } = frame_core;
    validate_claimed_handoff_fixed_axes_v1(
        &claimed_relation,
        &inventory,
        expected_existing_radix_candidate_root,
    )?;
    preflight.validate_schedule_v1(&claimed_relation.schedule)?;
    if !same_borrowed_slice_identity_v1(preflight.successor, exact_claimed_successor) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let RnsNativeCrossFieldRlweClaimedRelationV1 {
        schedule,
        pre_global_capability: _,
        final_challenge_seeds: _,
    } = claimed_relation;
    drop(preflight);

    let equality_pending = verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, P>(
        schedule,
        source,
        inventory.continuation(),
    )?;
    if !same_borrowed_slice_identity_v1(equality_pending.successor(), exact_claimed_successor) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    let terminal_bound = equality_pending.discharge_claimed_root_equality_v1()?;
    if !same_borrowed_slice_identity_v1(terminal_bound.successor(), exact_claimed_successor) {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext);
    }
    Ok((terminal_bound, inventory))
}

#[cfg(test)]
#[path = "rns_native_cross_field_rlwe_direct_tests.rs"]
mod tests;
