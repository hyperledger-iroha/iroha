//! Deterministic manifest for native first-release privacy engines.
//!
//! Governance does not get to turn arbitrary non-zero digests into executable
//! consensus code. Every activatable protocol must have one compiled profile
//! whose parameter, verifier, structurally derived statement-schema,
//! engine-manifest, and limit
//! bindings exactly match the proposed activation record. A protocol whose
//! complete verifier is not compiled is rejected before it enters world state.

use std::{collections::BTreeMap, sync::OnceLock};

#[cfg(feature = "zk-stark")]
use iroha_data_model::privacy::ZkAcePqAuthorizationStatementV1;
use iroha_data_model::privacy::{
    ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1, ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
    AnonymousPgcActivationLimitsV1, AnonymousPgcKOutOfNStatementV1,
    BOOTLE_LANTERN_APPLICATION_MODULUS_V1 as BOOTLE_LANTERN_MODEL_APPLICATION_MODULUS_V1,
    BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1 as BOOTLE_LANTERN_MODEL_ATTRIBUTE_COUNT_V1,
    BOOTLE_LANTERN_RING_DEGREE_V1 as BOOTLE_LANTERN_MODEL_RING_DEGREE_V1,
    BootleLanternIssuerPolicyV1, FCMP_MAX_INPUTS_V1, FCMP_MAX_OUTPUTS_V1, FcmpActivationLimitsV1,
    IVM_PRIVATE_NOTE_MAX_INPUTS_V1, IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
    IrohaBootleLanternAnoncredStatementV1, IrohaIvmPrivateNoteStarkStatementV1,
    IrohaJindoPolynomialCommitmentStatementV1, IrohaZkAmsStatementV1,
    IrohaZkX509StarkP256StatementV1, IvmPrivateNoteActivationLimitsV1, JindoActivationLimitsV1,
    MoneroFcmpPlusPlusStatementV1, ORCHARD_MAX_ACTIONS_V1 as ORCHARD_MODEL_MAX_ACTIONS_V1,
    OrchardActivationLimitsV1, OrchardHalo2ActionsStatementV1, PQ_MASP_MAX_INPUTS_V1,
    PQ_MASP_MAX_OUTPUTS_V1, PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
    PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1, PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1,
    PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1, PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1,
    PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1, PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
    PqMaspActivationLimitsV1, PqMaspStarkStatementV1, PrivacyAssuranceV1, PrivacyCapabilityRowV1,
    PrivacyCapabilitySnapshotV1, PrivacyCapabilitySnapshotValidationErrorV1,
    PrivacyCompiledProfileCatalogArchiveValidationStatusV1, PrivacyCompiledProfileCatalogRowV1,
    PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileCatalogValidationErrorV1,
    PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
    PrivacyCompiledProfileUnavailableReasonV1, PrivacyCompiledStatementSchemaErrorV1,
    PrivacyConsensusPolicyV1, PrivacyEngineIdV1, PrivacyEngineManifestDigestV1,
    PrivacyFcmpPoolBootstrapV1, PrivacyIvmPrivateNotePoolBootstrapV1,
    PrivacyOrchardPoolBootstrapV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
    PrivacyPgcAccountBootstrapV1, PrivacyPqMaspPoolBootstrapV1, PrivacyProofSystemIdV1,
    PrivacyProtocolActivationLimitsV1, PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1,
    PrivacyProtocolLifecycleV1, PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
    TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1, TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
    TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1, VEGA_ISSUER_GOVERNANCE_RECORD_VERSION_V1,
    VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1, VEGA_ISSUER_RECORD_HASH_FRAME_DOMAIN_V1,
    VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1, VEGA_MAX_ISSUER_RECORDS_V1,
    VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1, VEGA_MDL_BIRTH_RANDOM_BYTES_V1,
    VEGA_MDL_FULL_DATE_TEXT_BYTES_V1, VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1,
    VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MAX_PRESENTATION_YEAR_V1,
    VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MIN_PRESENTATION_YEAR_V1,
    VEGA_MDL_MSO_PAYLOAD_BYTES_V1, VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1,
    VERANGE_HARD_MAX_AGGREGATION_COUNT_V1, VeRangeActivationLimitsV1,
    VeRangeTransparentRangeStatementV1, VegaExistingCredentialStatementV1,
    ZK_AMS_MAX_BATCH_SIZE_V1, ZK_AMS_MAX_RING_SIZE_V1,
    ZK_AMS_RING_SIZES_V1 as ZK_AMS_MODEL_RING_SIZES_V1, ZkAmsActivationLimitsV1,
    validate_privacy_compiled_profile_catalog_archive_v1,
};
use iroha_schema::{FloatMode, IntMode, IntoSchema, MetaMapEntry, Metadata};
#[cfg(test)]
use iroha_zkp_halo2::vega::vega_mdl_verifier_digest_v1;
use iroha_zkp_halo2::vega::{
    MAX_VEGA_PROOF_BYTES_V1, MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
    VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1, VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
    VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1,
    ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1, vega_mdl_canonical_relation_digest_v1,
    vega_mdl_compiled_profile_digest_v1, zk_ams_admission_relation_dimensions_v1,
    zk_ams_compiled_profile_digest_v1, zk_ams_mkhe_readiness_v1,
    zk_ams_release_candidate_profile_digest_v1, zk_ams_t256_generator_digest_v1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(feature = "zk-stark")]
use crate::privacy_engines::zk_ace::{
    ZK_ACE_AIR_RELATION_SCHEMA_V1, ZK_ACE_AUTHORIZATION_PROJECTION_V1,
    ZK_ACE_POSEIDON_MANIFEST_SHA256_V1, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,
    ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1, ZK_ACE_PROOF_WIRE_V1, ZK_ACE_SOURCE_PROFILE_V1,
    zk_ace_compiled_profile_digest_v1, zk_ace_stark_profile_descriptor_v1,
};
use crate::privacy_engines::{
    anonymous_pgc::{
        ANONYMOUS_PGC_FULL_ENGINE_AVAILABLE_V1, AnonymousPgcParametersV1, PGC_SOURCE_PROFILE_V1,
        bootstrap::{
            MAX_PGC_BOOTSTRAP_NAMESPACE_BYTES_V1, MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
            PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1, PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            PGC_BOOTSTRAP_MAX_AGGREGATE_BALANCE_V1, PGC_BOOTSTRAP_PROOF_VERSION_V1,
            PGC_BOOTSTRAP_SUITE_V1, PGC_BOOTSTRAP_TABLE_DIGEST_DOMAIN_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
        },
        payment::{
            MAX_PGC_PAYMENT_PROOF_BYTES_V1, PGC_PAYMENT_ANONYMITY_SET_SIZES_V1,
            PGC_PAYMENT_MAX_RECIPIENTS_V1, PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            PGC_PAYMENT_PROOF_VERSION_V1, PGC_PAYMENT_SUITE_V1,
        },
    },
    bootle_lantern::{
        BOOTLE_CREDENTIAL_RANDOMNESS_PROFILE_DESCRIPTOR_V1,
        BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
        BOOTLE_LANTERN_FALCON512_DEFAULT_KEYGEN_CANDIDATES_V1,
        BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
        BOOTLE_LANTERN_FALCON512_KEYGEN_PARITY_ATTEMPTS_V1,
        BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
        BOOTLE_LANTERN_FALCON512_PREIMAGE_PROPOSALS_PER_COEFFICIENT_V1,
        BOOTLE_LANTERN_FALCON512_PREIMAGE_TOTAL_PROPOSALS_V1,
        BOOTLE_LANTERN_FALCON512_PROFILE_DESCRIPTOR_V1, BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1,
        BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
        BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
        BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1, CREDENTIAL_RANDOMNESS_NORM_SQUARED_BOUND_V1,
        CREDENTIAL_RANDOMNESS_POLYNOMIALS_V1, MAX_CREDENTIAL_RANDOMNESS_COEFFICIENT_PROPOSALS_V1,
        MAX_CREDENTIAL_RANDOMNESS_VECTOR_ATTEMPTS_V1,
        codec::{
            BLIND_ISSUANCE_AUTHORIZATION_BYTES_V1, BLIND_ISSUANCE_REQUEST_BYTES_V1,
            BLIND_ISSUANCE_REQUEST_HEADER_BYTES_V1, BLIND_ISSUANCE_REQUEST_MAGIC_V1,
            BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1, BLIND_ISSUANCE_REQUEST_PROOF_PURPOSE_TAG_V1,
            BLIND_ISSUANCE_REQUEST_PURPOSE_TAG_V1, BLIND_ISSUANCE_REQUEST_RING_DEGREE_V1,
            BLIND_ISSUANCE_REQUEST_TARGET_POLYNOMIALS_V1, BLIND_ISSUANCE_REQUEST_VERSION_V1,
            BLIND_ISSUANCE_RESPONSE_BYTES_V1, PROOF_BYTES_V1 as BOOTLE_LANTERN_PROOF_BYTES_V1,
            PROOF_MAGIC_V1, PROOF_VERSION_V1,
        },
        issuance_store::{
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_RECORDS_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_TOTAL_BYTES_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_RETENTION_BLOCKS_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_RECORDS_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_TOTAL_BYTES_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_MAX_RECORD_BYTES_V1,
        },
        issuer::{
            BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
            MAX_BOOTLE_LANTERN_AUTHORIZATION_ID_ATTEMPTS_V1,
            MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1,
            MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1,
            MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1, bootle_lantern_issuer_profile_digest_v1,
        },
        params::{
            APPLICATION_MODULUS_V1 as BOOTLE_LANTERN_APPLICATION_MODULUS_V1,
            APPLICATION_RELATION_QUOTIENT_BOUND_V1, APPLICATION_RING_DEGREE_V1,
            APPLICATION_ROWS_V1, APPLICATION_WITNESS_POLYNOMIALS_V1, CHALLENGE_ETA_V1,
            CHALLENGE_NORM_POWER_V1, CHALLENGE_NORM_ROOT_DEGREE_V1, CHALLENGE_OMEGA_V1,
            CHALLENGE_SET_BITS_V1, COMPRESSION_GAMMA_V1, COMPRESSION_MODULUS_V1,
            DECOMPOSITION_BITS_V1, MAX_CHALLENGE_CANDIDATE_ATTEMPTS_V1,
            PROOF_MODULUS_V1 as BOOTLE_LANTERN_PROOF_MODULUS_V1, RANDOMNESS_NORM_SQUARED_BOUND_V1,
            RESPONSE_NORM_SQUARED_BOUND_V1, SIGNATURE_NORM_SQUARED_BOUND_V1, SOURCE_PROFILE_V1,
        },
        sampling::{BOOTLE_SAMPLING_PROFILE_DESCRIPTOR_V1, bootle_sampling_profile_digest_v1},
        scope::{
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1, BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
            BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1,
            BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1,
        },
        transcript::{PUBLIC_PARAMETER_SEED_DOMAIN_V1, public_parameter_seed_v1},
    },
    fcmp_plus_plus::{
        FCMP_BP_PLUS_GENERATOR_DIGEST_V1, FCMP_BP_PLUS_UPSTREAM_REVISION_V1,
        FCMP_COMPILED_PROFILE_DESCRIPTOR_V1, FCMP_LAYER_ONE_LEN_V1, FCMP_LAYER_TWO_LEN_V1,
        FCMP_MAX_INPUTS_NATIVE_V1, FCMP_MAX_OUTPUTS_NATIVE_V1, FCMP_MAX_PROOF_WIRE_BYTES_V1,
        FCMP_MAX_TREE_LAYERS_V1, FCMP_MIN_PROOF_WIRE_BYTES_V1, FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
        FCMP_NATIVE_KAT_WIRE_SHA256_V1, FCMP_OUTPUT_TUPLE_BYTES_V1, FCMP_POINT_BYTES_V1,
        FCMP_PROOF_INPUT_BYTES_V1, FCMP_PROOF_WIRE_HEADER_BYTES_V1, FCMP_PROOF_WIRE_MAGIC_V1,
        FCMP_SAL_PROOF_BYTES_V1, FCMP_SOURCE_PROFILE_V1, FCMP_UPSTREAM_REVISION_V1,
        fcmp_bp_plus_generator_digest_v1, fcmp_compiled_profile_digest_v1,
        fcmp_plus_plus_wire_size_v1,
    },
    ivm_private_note::{
        IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1, IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1,
        IVM_PRIVATE_NOTE_HASH_PROFILE_DESCRIPTOR_V1, IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1,
        IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1, IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
        IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1, PRIVATE_NOTE_MAX_INPUTS_V1,
        PRIVATE_NOTE_MAX_OUTPUTS_V1, PRIVATE_NOTE_TREE_DEPTH_V1, PRIVATE_PROGRAM_BYTES_V1,
        PRIVATE_PROGRAM_INSTRUCTION_BYTES_V1, PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1,
        PRIVATE_PROGRAM_REGISTER_COUNT_V1, validate_ivm_private_note_stark_profile_v1,
    },
    jindo::{
        JINDO_MAX_BATCH_SIZE_V1, JINDO_NATIVE_PROOF_BYTES_V1, JINDO_PARAMETER_MANIFEST_V1,
        JINDO_SOURCE_PROFILE_V1, JINDO_SOURCE_PROVENANCE_V1, JINDO_SUITE_V1, jindo_crs_digest_v1,
    },
    orchard::{
        ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
        ORCHARD_MAX_ACTIONS_V1 as ORCHARD_ENGINE_MAX_ACTIONS_V1,
        ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1, ORCHARD_PROVER_RANDOMNESS_POLICY_V1,
        ORCHARD_UPSTREAM_CRATE_VERSION_V1, ORCHARD_UPSTREAM_REVISION_V1,
        orchard_authorization_wire_size_v1, orchard_empty_root_v1,
    },
    pq_masp::{
        air::PQ_MASP_AGGREGATE_AIR_DESCRIPTOR_V1,
        relation::{
            PQ_MASP_ENGINE_DESCRIPTOR_V1, PQ_MASP_HASH_PROFILE_DESCRIPTOR_V1,
            PQ_MASP_INPUT_BOUND_V1, PQ_MASP_OUTPUT_BOUND_V1, PQ_MASP_TREE_DEPTH_V1,
        },
        stark::{
            PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1, PQ_MASP_STARK_KAT_PROOF_SHA256_V1,
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1, PQ_MASP_STARK_PROFILE_DIGEST_V1,
            validate_pq_masp_stark_profile_v1,
        },
        wire::{
            AUTHORIZATION_MAGIC_V1, ENCRYPTED_OUTPUT_MAGIC_V1, ML_DSA_65_PUBLIC_KEY_BYTES_V1,
            ML_DSA_65_SIGNATURE_BYTES_V1, ML_KEM_768_CIPHERTEXT_BYTES_V1,
            ML_KEM_768_PUBLIC_KEY_BYTES_V1, PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1,
            PQ_MASP_AUTHORIZATION_WIRE_KAT_SHA256_V1, PQ_MASP_ENCRYPTED_OUTPUT_BYTES_V1,
            PQ_MASP_ENCRYPTED_OUTPUT_KAT_SHA256_V1, PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1,
            PQ_MASP_MAX_STARK_PROOF_BYTES_V1, PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
            XCHACHA20_NONCE_BYTES_V1,
        },
    },
    proof_managed_note_stark::{
        PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
        PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1, proof_managed_note_stark_profile_digest_v1,
    },
    prover_randomness::{
        CURVE_PROVER_RANDOMNESS_POLICY_V1, TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
    },
    vega::{
        VEGA_MDL_DEVICE_AUTHENTICATION_DOMAIN_V1, VEGA_MDL_DEVICE_AUTHENTICATION_FRAME_VERSION_V1,
    },
    verange::{
        VERANGE_TYPE1_PROOF_VERSION_V1, VERANGE_TYPE1_SOURCE_PROFILE_V1, VERANGE_TYPE1_SUITE_V1,
        VeRangeBitLengthV1, VeRangeParametersV1,
    },
    zk_ams::{
        MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1, MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,
        MAX_ZK_AMS_LSAG_PROOF_BYTES_V1, ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1,
        ZK_AMS_ADMISSION_POSSESSION_SUITE_V1, ZK_AMS_BATCH_ADMISSION_PROOF_VERSION_V1,
        ZK_AMS_LSAG_DECODE_ALLOCATION_BYTES_V1, ZK_AMS_LSAG_PROOF_VERSION_V1, ZK_AMS_LSAG_SUITE_V1,
        ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, ZK_AMS_RING_SIZES_V1, ZK_AMS_SOURCE_PROFILE_V1,
        zk_ams_generator_digest_v1,
    },
    zk_x509::{
        engine::construct_zk_x509_compiled_profile_v1,
        profile::{
            ZK_X509_MAX_PROOF_BYTES_V1, ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1,
            ZK_X509_PROOF_VERSION_V1, ZK_X509_SOURCE_PROFILE_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1, ZK_X509_SUITE_V1, ZkX509ProfileErrorV1,
            require_activation_readiness_v1, validate_profile_v1 as validate_zk_x509_profile_v1,
            zk_x509_activation_readiness_v1,
        },
        stark::{ZK_X509_MAIN_PROOF_DESCRIPTOR_V1, ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1},
    },
};

const PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.digest.v1";
const PARAMETER_ID_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.parameter-id.v1";
const PARAMETER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.parameter-digest.v1";
const VERIFIER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.verifier-digest.v1";
const CANONICAL_SCHEMA_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.canonical-structural-schema.v1";
const ENGINE_MANIFEST_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.engine-manifest-digest.v1";
const VERANGE_PROTOCOL_LABEL_V1: &[u8] = b"verange-transparent-range-v1";
const VERANGE_PARAMETER_SET_LABEL_V1: &[u8] = b"p256-type1-bits32-bits64-v1";
const VERANGE_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:ordered-independent-type1-batch:strict-exact:v1";
const VERANGE_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2025-528:figure-1:type-1:v1";
const ANONYMOUS_PGC_PROTOCOL_LABEL_V1: &[u8] = b"anonymous-pgc-k-out-of-n-v1";
const ANONYMOUS_PGC_PARAMETER_SET_LABEL_V1: &[u8] = b"p256-twisted-elgamal-rfc9380-g-h-v1";
const ANONYMOUS_PGC_PAYMENT_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:anonymous-pgc-payment:fixed-range-arrays+statement-bounded-sequences:strict-exact:v1";
const ANONYMOUS_PGC_BOOTSTRAP_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:anonymous-pgc-bootstrap:fixed-range-arrays+statement-bounded-accounts:strict-exact:v1";
const ANONYMOUS_PGC_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2025-884:sections-3-4-6:linear-legality-and-bounded-bootstrap:v1";
const BOOTLE_LANTERN_PROTOCOL_LABEL_V1: &[u8] = b"iroha-bootle-lantern-anoncred-v1";
const BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1: &[u8] =
    b"falcon512-ntru-r512-as-r64-rank8-interleaved";
const BOOTLE_LANTERN_PROOF_WIRE_LABEL_V1: &[u8] =
    b"ILN1:fixed-70344-byte:51-bit-residues:strict-exact:v1";
const BOOTLE_LANTERN_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:BLNS-specialization-no-main-construction-reduction:eprint-2023-560:lazer-10eafeca4cd53ff4fc54193dce904dbd0026fefd:lantern-lnp22-figure18-full-centered-makeghint:vendored-Unlicense-rust-fn-dsa-workspace-0.3-daf14859b5aa3f8d75c42966ba7de83e6eb59997:portable-safe-rust-no-SIMD:v1";
const BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1: &[u8] = b"Falcon-512:NTRUGen(f,g,F,G):f*G-g*F=12289|public:h=g/f mod(X^512+1,12289)|R512-to-rank8-R64:H_i[j]=h[8*j+i]|policy-matrix:canonical-8x8-one-key-negacyclic-block-embedding|signature:[s1|s2]:s1+h*s2=target|target:t+A_tau*tau+credential-scope|norm2(s1,s2)<=34034726";
const BOOTLE_LANTERN_RELATION_SCHEMA_V1: &[u8] = b"active-self-digested-policy:H[8x8x64]mod12289-from-one-Falcon-h|required-disclosures:u8|allowed-values[8][<=32x8]|statement:issuer+policy+epoch+record-digest+issuer-parameter+ordered-disclosures|credential-scope:reusable-equation-term|relation:A_r*r+A_tau*tau+A_m*m+scope-s1-H*s2=0:8x48-ring64-linear|norms:randomness<=11881+signature<=34034726";
const BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1: &[u8] = b"ILQ1:fixed71576-byte-purpose1-strict-exact-allocation-bounded-complete-holder-request-with-count8-degree64-and-fixed70344-byte-ILB1|ILB1:fixed70344-byte-purpose1-strict-exact-inner-P1|ILN1:fixed70344-byte-purpose0-strict-exact|direct-cross-purpose-wire-decode-rejected|complete-header-splice=structurally-decodable-but-purpose-separated-transcript-verification-rejects|caller-cap-before-exact-length-before-allocation|no-trailing-bytes";
const BOOTLE_LANTERN_BLIND_ISSUANCE_SCHEMA_V1: &[u8] = b"canonical-first-release-only:keygen->issuer-authorize-ILA1-and-atomically-register-Fresh-in-bounded-ILS1->holder-one-master64-derived-r-and-P1->blind-request-t=A_r*r+A_m*m->encode-complete-ILQ1-with-strict-ILB1-P1-proof-of-r,m-bound-to-authorization-digest->issuer-decode-exact-ILQ1-before-nonmutating-store-preflight-before-P1-verification->atomic-height-aware-Fresh-to-Processing-before-one-master64-derived-tau-and-preimage->bounded-Falcon-preimage-for-t+A_tau*tau+scope->durably-commit-exact-ILR1-as-Completed-before-release->holder-finalize->ILN1-P2-presentation|identical-completed-replay=byte-identical-cached-ILR1-after-process-reopen-and-authorization-expiry-without-P1-or-rng|processing-same-request=Busy|different-request-or-Failed=AuthorizationConsumed|post-claim-failure=terminal-never-Fresh|explicit-authoritative-height-pruning-only|no-public-direct-or-trusted-issuance-shortcut";
const BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1: &[u8] = b"P1:typed-blind-issuance-binding=parameter-digest+genesis-hash+issuer-profile-digest+credential-scope-digest+issuer-policy-record-digest+masked-target-digest+issuer-generated-one-shot-issuance-authorization-digest+relation-digest+matrix-seed+public-parameter-seed:no-statement-or-transaction-intent|P2:challenge-binding=parameter-digest+genesis-hash+statement-digest+issuer-policy-record-digest+transaction-intent-digest+relation-digest+matrix-seed+public-parameter-seed|challenge-xof:SHAKE256;first32=sequential-rejection-bytes<255,byte%17-8;max-rejected-uniform-draws-per-coefficient=4096;c[32]=0;c[64-i]=-c[i]for-i=1..31;candidate-retry=next-sequential-single-XOF-bytes;max-candidates=4096|eta-check:integer-negacyclic-ring=Z[X]/(X^64+1);k=32;root-degree=64;L1(sigma_-1(c^32)*c^32)<=140^64|framing:u32-be";
const BOOTLE_LANTERN_NATIVE_PRODUCER_SCHEMA_V1: &[u8] = b"native-producers:keygen+issuer-authorization+blind-request/ILQ1-with-ILB1-P1+issuer-strict-byte-ingress/preflight/atomic-height-claim/issue/durable-complete+holder-finalize+presentation/P2|fallible-health-checked-source-boundaries:keygen,authorization-id,holder-master64,issuer-master64,P2|closed-purpose-separated-context-bound-substreams:holder-master->{holder-r,P1};issuer-master->{issuer-tau,issuer-preimage}|bounded-fail-closed-no-fallback|completed-replay-check-before-P1|cached-completed-replay-does-not-touch-rng-and-survives-authorization-expiry|self-check:NTRU+public-key+ILQ1+P1+Falcon-equation+ILR1+P2";
const BOOTLE_LANTERN_COMPRESSION_SCHEMA_V1: &[u8] = b"lnp22-figure18:power2round-q-D15:decompose-q-gamma:makeghint-full-canonical-centered-z22:useghint-centered-mod-m:hint-infinity-bound=floor(m/2)";
const JINDO_PROTOCOL_LABEL_V1: &[u8] = b"iroha-jindo-polynomial-commitment-v0";
const JINDO_PARAMETER_SET_LABEL_V1: &[u8] =
    b"jindo-current-univariate-coefficient-batch4-target256-ring1024-transparent-v1";
const JINDO_PROOF_WIRE_LABEL_V1: &[u8] =
    b"IJP2:fixed-phases:7-outer:12-inner:644-field:strict-exact:no-IJP1:v1";
const JINDO_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2026-044-current:figures-2-7:univariate-coefficient-specialization:ringo-snark-805eab27-oracle-only:v1";
const ORCHARD_PROTOCOL_LABEL_V1: &[u8] = b"orchard-halo2-actions-v1";
const ORCHARD_PARAMETER_SET_LABEL_V1: &[u8] = b"orchard-v3-post-nu6-3-halo2-ipa-pasta-v1";
const ORCHARD_PROOF_WIRE_LABEL_V1: &[u8] =
    b"ORC1:u8-actions:halo2-proof:ordered-redpallas-spend-signatures:binding-signature";
const ORCHARD_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:zcash-orchard-v3:post-nu6-3:first-release-no-legacy:v1";
const ORCHARD_FRONTIER_SCHEMA_V1: &[u8] =
    b"tree_size:u64|leaf:option<cmx32>|ommers:ordered<merkle_hash32>|root:32|depth:32";
const ORCHARD_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap_digest:32|asset_definition_id:norito|reserve_account:norito|anchor:32|anchor_epoch:u64|current_root:32|current_epoch:u64|successor_frontier|ordered_nullifiers[32]|value_balance:direction+u128|expiry_height:u64";
const FCMP_PROTOCOL_LABEL_V1: &[u8] = b"monero-fcmp-plus-plus-v1";
const FCMP_PARAMETER_SET_LABEL_V1: &[u8] =
    b"monero-fcmp++-ed25519-selene38-helios18-dual-generalized-bulletproofs+strict-positive-u64-bulletproofs-plus-v1";
const FCMP_PROOF_WIRE_LABEL_V1: &[u8] =
    b"IFC1:u8-inputs:u8-layers:u8-outputs:u8-zero:ordered-o~-i~-r-sal+dual-gbp+root-blind-pok+ordered-output-strict-positive-u64-bp-plus:strict-exact:v1";
const FCMP_RUNTIME_CONTEXT_SCHEMA_V1: &[u8] = b"sha256:domain+chain-id-u64be-len+genesis-hash+action-index-u32be+statement-digest+parameter-id+parameter-digest+verifier-digest+statement-schema-digest+engine-manifest-digest";
const FCMP_FRONTIER_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|epoch:u64|typed-root:layers-u8+point32|tree-size:u64|active-complete-output-tuples[O32,I32,C32]|mixed-radix-levels[ordered-point32]";
const FCMP_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|asset-definition-id:norito|current-typed-root-history-commitment:32|current-epoch:u64|validator-derived-next-typed-root-history-commitment:32|next-epoch:u64|ordered-key-images[32]|ordered-complete-output-tuples[O32,I32,C32]";
const FCMP_WALLET_CIPHERTEXT_SCHEMA_V1: &[u8] = b"IFCE|nonce24|xchacha20poly1305[IFN1+output-id32+O32+I32+C32+amount-u64le+commitment-mask32+spend-x32+output-y32]|x25519|sha256-domain-kdf|aad:pool-id+recipient-id+ephemeral-key+output-id+O+I+C";
const IVM_PRIVATE_NOTE_PROTOCOL_LABEL_V1: &[u8] = b"iroha-ivm-private-note-stark-v1";
const IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1: &[u8] =
    b"goldilocks-sha256-proof-managed-note-stark+private-note-vm16x8-tree32-v1";
const IVM_PRIVATE_NOTE_PROOF_WIRE_LABEL_V1: &[u8] =
    b"IPS1:u16-version:sha256-merkle+fri:strict-exact:v1";
const IVM_PRIVATE_NOTE_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:first-release:private-note-vm+sha256-aggregate-stark:v1";
const PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1: &[u8] = b"norito:iroha.privacy.native-consensus-binding.v1|fields:chain-id+genesis-hash32+action-index-u32+transaction-intent-digest32+parameter-id32+parameter-digest32+verifier-digest32+statement-schema-digest32+engine-manifest-digest32|digest:blake3(iroha:privacy:native-consensus-binding:v1+canonical-length-u64le+canonical-norito)";
const IVM_PRIVATE_NOTE_RUNTIME_CONTEXT_SCHEMA_V1: &[u8] = b"stark-public-input:sha256-frame(ivm-private-note-stark-public-input-with-consensus-binding-v1+canonical-statement+native-consensus-binding-digest32)";
const IVM_PRIVATE_NOTE_FRONTIER_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|root-role:program-state|program-id:32|epoch:u64|root:sha256-depth32|tree-size:u64|frontier[ordered-option<node32>]";
const IVM_PRIVATE_NOTE_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|asset-definition-id:norito|reserve-account:norito|program-id:32|anchor:32|anchor-epoch:u64|current-root:32|current-epoch:u64|validator-derived-successor-frontier|ordered-nullifiers[32]|ordered-output-commitments[32]|value-balance:direction+u128|expiry-height:u64";
const IVM_PRIVATE_NOTE_WALLET_CIPHERTEXT_SCHEMA_V1: &[u8] = b"IPNE|nonce24|xchacha20poly1305[IPW1+authority32+value-u128le+rho32+rseed32+program-state32+memo32]|x25519|sha256-domain-kdf|aad:pool-id+recipient-id+output-commitment";
const PQ_MASP_PROTOCOL_LABEL_V1: &[u8] = b"pq-masp-stark-v0";
const PQ_MASP_PARAMETER_SET_LABEL_V1: &[u8] =
    b"goldilocks-sha256-proof-managed-note-stark+pq-masp+mldsa65+mlkem768-v1";
const PQ_MASP_PROOF_WIRE_LABEL_V1: &[u8] =
    b"PQA1:u32be-inner-len:mldsa65-pk1952+signature3309+PQS1-inner-stark:strict-exact:v1";
const PQ_MASP_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:first-release:pq-masp+mldsa65+mlkem768+xchacha20poly1305+sha256-aggregate-stark:v1";
const PQ_MASP_RUNTIME_CONTEXT_SCHEMA_V1: &[u8] = b"stark-public-input:sha256-frame(pq-masp-stark-public-input-with-consensus-binding-v1+canonical-statement+native-consensus-binding-digest32)";
const PQ_MASP_FRONTIER_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|root-role:note-commitment-anchor|epoch:u64|root:sha256-depth32|tree-size:u64|frontier[ordered-option<node32>]";
const PQ_MASP_AUTHORIZATION_SCHEMA_V1: &[u8] = b"authorization-context:pq-masp-stark-v0|message:sha256-domain+statement-digest32+native-consensus-binding-digest32+inner-length-u64be+inner-sha256|authorization-key-digest:statement-bound+derived-from-canonical-pk1952|mldsa65:canonical-pk1952+canonical-signature3309|outer-wire:PQA1+u32be-inner-len+pk+signature+PQS1";
const PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap-digest:32|asset-definition-id:norito|current-root:32|current-epoch:u64|next-root:32|next-epoch:u64|transition:pq-masp{ordered-nullifiers[32]+ordered-output-commitments[32]+validator-derived-successor-frontier}|value-balance:none";
const VEGA_PARAMETER_SET_LABEL_V1: &[u8] = b"vega-figure9-mdl-age-microsoft-mc-2+6-sha256-t256-v1";
const VEGA_PROOF_WIRE_LABEL_V1: &[u8] =
    b"IROVEGMC:version-u8+context-keccak32+bincode-1.3.3-fixed-le-microsoft-vega-mc:strict-exact:v1";
const VEGA_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:microsoft-vega-prover:c0ee259053cd12eaf43ed71b5cde375452b3ee4d:vega-mc:figure9-2+6-sha256:external-rng-fail-closed-patch:v1";
const VEGA_AUTHORITATIVE_ISSUER_RUNTIME_SCHEMA_V1: &[u8] = b"issuer-governance:record-v1:issuer-id32+epoch-u64be+compressed-p256-33+document-policy+namespace-policy+digest-policy+issuer-auth-policy+device-auth-policy+predecessor-option32+lifecycle+self-digest32|lineage:immutable-append-only+epoch-one-origin+one-step-cas-rotation+terminal-preserving-revocation+bounded-global-and-per-lineage+permanent-global-p256-key-ownership+retired-p256-key-never-reactivated|statement:exact-issuer-id+record-epoch+record-digest+key+all-algorithm-policy|ledger-verifier:current-active-exact-record-before-native-proof";
const VEGA_DEVICE_AUTHENTICATION_GOVERNANCE_FRAME_SCHEMA_V1: &[u8] = b"length-framed:domain+frame-version+upstream-commit+chain-id+genesis-hash+action-index+transaction-intent-digest+parameter-id+parameter-digest+verifier-digest+statement-schema-digest+engine-manifest-digest+issuer-id+issuer-record-epoch+issuer-record-digest+document-type+namespace+digest-algorithm+issuer-authentication+device-authentication+issuer-public-key+presentation-date+minimum-age+reader-challenge+session-transcript-digest";
const VEGA_CANONICAL_MDL_WITNESS_SCHEMA_V1: &[u8] = b"figure9-v1:issuer-sig-structure-exact+embedded-mso-exact+birth-item-exact+birth-random-exact+full-date10+rfc3339-utc-seconds20+signed-not-after-valid-from-full-seconds+presentation-validity-date-granularity+presentation-year-closed+satisfiable-valid-until+age-threshold-closed";
const VEGA_CANONICAL_SIGNATURE_PREFLIGHT_POLICY_V1: &[u8] = b"native-witness-preflight:issuer-and-device-es256-signatures:p1363-r32s32+canonical-nonzero-scalars+low-s-required+reject-high-s-without-normalization+verify-prehash-before-r-s-inverse:v1";
const ZK_AMS_PROTOCOL_LABEL_V1: &[u8] = b"iroha-zk-ams-v1";
const ZK_AMS_PARAMETER_SET_LABEL_V1: &[u8] =
    b"zk-ams-v2-masked-relaxed-spartan-t256-lsag-ristretto255-v1";
const ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:zk-ams-batch-admission:version-u8+masked-relaxed-spartan-bytes+possession-count-u8+fixed-possession-slots8{version-u8+commitment32+response32}+all-zero-unused-tail:strict-exact:v1";
const ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:zk-ams-provision-account:lsag-ristretto255:strict-exact:v1";
const ZK_AMS_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:arxiv-2602.16130v2:algorithms-1-4:appendices-a-c:closed-phase-v:v1";
const ZK_AMS_BATCH_EFFECT_SCHEMA_V1: &[u8] = b"issuer_id:32|registry_id:32|prior_root:32|prior_epoch:u64|next_root:32|next_epoch:u64|ordered_anchors[seed_key:32,link_tag:32]";
const ZK_AMS_PROVISION_EFFECT_SCHEMA_V1: &[u8] = b"issuer_id:32|registry_id:32|current_root:32|current_epoch:u64|ring[seed_key:32]|account_id:norito|key_image:32";
#[cfg(feature = "zk-stark")]
const ZK_ACE_PROTOCOL_LABEL_V1: &[u8] = b"zk-ace-pq-authorization-v0";
#[cfg(feature = "zk-stark")]
const ZK_ACE_PARAMETER_SET_LABEL_V1: &[u8] = b"goldilocks-poseidon2-transparent-stark-v1";
const ZK_X509_PARAMETER_SET_LABEL_V1: &[u8] =
    b"goldilocks-fp4-sha256-p256-rfc5280-fixed-capacity-v1";
const ZK_X509_PROOF_WIRE_LABEL_V1: &[u8] =
    b"X5S1:exact-one-X5M1-main+exact-one-X5C1-compact-ca:strict-exact:no-legacy:v1";
const ZK_X509_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:original-transparent-x509-p256-sha256-stark:first-release:no-trusted-setup:no-legacy:v1";
const ZK_X509_RUNTIME_STATE_SCHEMA_V1: &[u8] =
    b"trusted-state:active-self-digested-trust-anchor-revision+active-self-digested-certificate-policy-revision+active-current-complete-signed-crl-revision+current-retained-ca-membership-root-head+certificate-nullifier-replay-set|trusted-block-time+taira-consensus-limits|verifier-owned-rfc-public-input";
const ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1: &[u8] = b"namespace_len:u64le|namespace:norito|epoch:u64le|total_supply:u32le|account_count:u32le|accounts[public_key:33,cipher_left:33,cipher_right:33]";
const ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|total_supply:u32|current_root:32|current_epoch:u64|next_root:32|next_epoch:u64|complete_accounts[public_key:33,cipher_left:33,cipher_right:33]";

/// Exact compiled bindings for one native privacy protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledPrivacyProfileV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Closed proof-system identity.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Closed native-engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Deterministic identifier of the compiled parameter set.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact compiled parameters.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier relation and proof wire.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of the exact public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the complete compiled engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Exact protocol-specific limits compiled into the verifier.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
}

impl CompiledPrivacyProfileV1 {
    /// Build the only valid activation record for this compiled profile.
    #[must_use]
    pub fn activation_record(
        self,
        lifecycle: PrivacyProtocolLifecycleV1,
    ) -> PrivacyProtocolActivationRecordV1 {
        PrivacyProtocolActivationRecordV1 {
            protocol_id: self.protocol_id,
            proof_system_id: self.proof_system_id,
            engine_id: self.engine_id,
            parameter_id: self.parameter_id,
            parameter_digest: self.parameter_digest,
            verifier_digest: self.verifier_digest,
            statement_schema_digest: self.statement_schema_digest,
            engine_manifest_digest: self.engine_manifest_digest,
            lifecycle,
            protocol_limits: self.protocol_limits,
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }
}

impl From<CompiledPrivacyProfileV1> for PrivacyCompiledProfileSnapshotV1 {
    fn from(profile: CompiledPrivacyProfileV1) -> Self {
        Self {
            protocol_id: profile.protocol_id,
            proof_system_id: profile.proof_system_id,
            engine_id: profile.engine_id,
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
            protocol_limits: profile.protocol_limits,
        }
    }
}

/// Build the exact local compiled-profile result for one public snapshot row.
#[must_use]
pub fn compiled_privacy_profile_snapshot_result_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> PrivacyCompiledProfileResultV1 {
    match compiled_privacy_profile_v1(protocol_id) {
        Ok(profile) => PrivacyCompiledProfileResultV1::Available(profile.into()),
        Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { .. }) => {
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
            )
        }
        Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { .. }) => {
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed,
            )
        }
        Err(CompiledPrivacyProfileErrorV1::StatementSchemaInvalid { source, .. }) => {
            let source = match source {
                CanonicalSchemaDigestErrorV1::ConflictingStableTypeId => {
                    PrivacyCompiledStatementSchemaErrorV1::ConflictingStableTypeId
                }
                CanonicalSchemaDigestErrorV1::MissingTypeReference => {
                    PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference
                }
            };
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(source),
            )
        }
    }
}

/// Build the exact local compiled-profile catalog for this binary.
///
/// The result contains one row for every closed first-release protocol in
/// canonical discriminant order. It intentionally contains no committed
/// height, policy, activation, lifecycle, or readiness projection; those are
/// authoritative only in a live Torii [`PrivacyCapabilitySnapshotV1`].
///
/// # Errors
///
/// Returns a deterministic validation error if an internal compiled profile
/// violates the closed catalog contract.
pub fn compiled_privacy_profile_catalog_v1()
-> Result<PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileCatalogValidationErrorV1> {
    static CATALOG: OnceLock<
        Result<PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileCatalogValidationErrorV1>,
    > = OnceLock::new();

    CATALOG
        .get_or_init(build_compiled_privacy_profile_catalog_v1)
        .clone()
}

fn build_compiled_privacy_profile_catalog_v1()
-> Result<PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileCatalogValidationErrorV1> {
    let catalog = PrivacyCompiledProfileCatalogV1 {
        version: PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1,
        protocols: PrivacyProtocolIdV1::ALL
            .into_iter()
            .map(|protocol_id| PrivacyCompiledProfileCatalogRowV1 {
                protocol_id,
                compiled_profile: compiled_privacy_profile_snapshot_result_v1(protocol_id),
            })
            .collect(),
    };
    catalog.validate()?;
    Ok(catalog)
}

/// Validate an archive as the exact compiled-profile catalog of this binary.
///
/// The data-model validator first enforces canonical bounded decoding and the
/// exact twelve-row shape. This second layer then requires byte-equivalent
/// typed content to the catalog derived from the current binary, preventing a
/// canonical but substituted profile digest from being accepted as local build
/// metadata. Success still does not establish network activation or readiness.
#[must_use]
pub fn validate_local_privacy_compiled_profile_catalog_archive_v1(
    archive: &[u8],
) -> PrivacyCompiledProfileCatalogArchiveValidationStatusV1 {
    use PrivacyCompiledProfileCatalogArchiveValidationStatusV1 as Status;

    let status = validate_privacy_compiled_profile_catalog_archive_v1(archive);
    if !status.is_valid() {
        return status;
    }
    let Ok(actual) = norito::decode_from_bytes::<PrivacyCompiledProfileCatalogV1>(archive) else {
        return Status::MalformedArchive;
    };
    let Ok(expected) = compiled_privacy_profile_catalog_v1() else {
        return Status::InvalidCatalog;
    };
    if actual != expected {
        return Status::InvalidCatalog;
    }
    Status::Valid
}

/// Build and validate an authoritative committed privacy capability snapshot.
///
/// `activation_for` must read from the same immutable committed world view as
/// `consensus_policy` and `committed_height`. The closure is invoked exactly
/// once for every protocol in canonical discriminant order.
///
/// # Errors
///
/// Returns a deterministic structural or height-consistency error if the
/// committed state cannot be represented by the closed snapshot contract.
pub fn committed_privacy_capability_snapshot_v1(
    committed_height: u64,
    consensus_policy: PrivacyConsensusPolicyV1,
    mut activation_for: impl FnMut(PrivacyProtocolIdV1) -> Option<PrivacyProtocolActivationRecordV1>,
) -> Result<PrivacyCapabilitySnapshotV1, PrivacyCapabilitySnapshotValidationErrorV1> {
    let protocols = PrivacyProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| PrivacyCapabilityRowV1 {
            protocol_id,
            compiled_profile: compiled_privacy_profile_snapshot_result_v1(protocol_id),
            activation: activation_for(protocol_id),
        })
        .collect();
    let snapshot = PrivacyCapabilitySnapshotV1 {
        version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
        committed_height,
        consensus_policy,
        protocols,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Return the exact compiled profile exposed to privacy governance.
///
/// # Errors
///
/// Returns [`CompiledPrivacyProfileErrorV1::EngineUnavailable`] for a protocol
/// whose complete end-to-end verifier is not compiled or whose independent
/// release-readiness gates remain closed, or
/// [`CompiledPrivacyProfileErrorV1::ProfileInitializationFailed`] if fixed
/// transparent parameters cannot be derived, or
/// [`CompiledPrivacyProfileErrorV1::StatementSchemaInvalid`] when the emitted
/// schema is ambiguous or internally unresolved.
pub fn compiled_privacy_profile_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    match protocol_id {
        #[cfg(feature = "zk-stark")]
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => compiled_zk_ace_profile_v1(),
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => compiled_anonymous_pgc_profile_v1(),
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => compiled_verange_profile_v1(),
        PrivacyProtocolIdV1::IrohaZkAmsV1 => compiled_zk_ams_profile_v1(),
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => compiled_jindo_profile_v1(),
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => compiled_bootle_lantern_profile_v1(),
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => compiled_vega_profile_v1(),
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => compiled_orchard_profile_v1(),
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => compiled_fcmp_profile_v1(),
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => compiled_ivm_private_note_profile_v1(),
        PrivacyProtocolIdV1::PqMaspStarkV0 => compiled_pq_masp_profile_v1(),
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => compiled_zk_x509_profile_v1(),
        // A build that deliberately excludes transparent STARK support must
        // expose ZK-ACE as unavailable; governance never substitutes an alias,
        // compatibility verifier, or incomplete implementation.
        #[cfg(not(feature = "zk-stark"))]
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
    }
}

fn compiled_zk_x509_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
    match require_activation_readiness_v1(zk_x509_activation_readiness_v1()) {
        Ok(()) => zk_x509_release_candidate_profile_material_v1(),
        Err(ZkX509ProfileErrorV1::EngineIncomplete) => {
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
        Err(_) => Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id }),
    }
}

/// Derive the deterministic ZK-X509 release-candidate profile material.
///
/// This accessor exists only for unsigned/offline intent preparation, release
/// KATs, and isolated resource measurements that must bind the exact profile
/// candidate before activation. Success does not imply compiled readiness,
/// activation, verifier availability, or permission to sign or submit a proof.
/// Production proof paths must use [`compiled_privacy_profile_v1`], which keeps
/// returning `EngineUnavailable` until every activation-readiness capture is
/// canonically admitted.
pub fn zk_x509_release_candidate_profile_material_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
    validate_zk_x509_profile_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let compiled_profile_digest = construct_zk_x509_compiled_profile_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .digest();
    let proof_version = ZK_X509_PROOF_VERSION_V1.to_be_bytes();
    let proof_bytes = ZK_X509_MAX_PROOF_BYTES_V1.to_be_bytes();
    let maximum_x5s1_bytes = ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    if compiled_profile_digest == [0; 32]
        || ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 > ZK_X509_MAX_PROOF_BYTES_V1
        || ZK_X509_MAX_PROOF_BYTES_V1 > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_PARAMETER_SET_LABEL_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1,
            &compiled_profile_digest,
            &proof_version,
            &proof_bytes,
            &maximum_x5s1_bytes,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_PARAMETER_SET_LABEL_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1,
            ZK_X509_MAIN_PROOF_DESCRIPTOR_V1,
            &compiled_profile_digest,
            &proof_version,
            &proof_bytes,
            &maximum_x5s1_bytes,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<IrohaZkX509StarkP256StatementV1>()
        .map_err(|source| {
        CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
            protocol_id,
            source,
        }
    })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_IMPLEMENTATION_PROVENANCE_V1,
            ZK_X509_PARAMETER_SET_LABEL_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_X509_PROOF_WIRE_LABEL_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1,
            ZK_X509_MAIN_PROOF_DESCRIPTOR_V1,
            ZK_X509_RUNTIME_STATE_SCHEMA_V1,
            &compiled_profile_digest,
            &proof_version,
            &proof_bytes,
            &maximum_x5s1_bytes,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:stark-fri-sha256-goldilocks",
            b"engine:native-goldilocks-stark-fri",
            ZK_X509_PARAMETER_SET_LABEL_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_X509_PROOF_WIRE_LABEL_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1,
            ZK_X509_MAIN_PROOF_DESCRIPTOR_V1,
            ZK_X509_RUNTIME_STATE_SCHEMA_V1,
            &compiled_profile_digest,
            &proof_version,
            &proof_bytes,
            &maximum_x5s1_bytes,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
        engine_id: PrivacyEngineIdV1::NativeGoldilocksStarkFri,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaZkX509StarkP256V0,
    })
}

fn compiled_ivm_private_note_profile_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    compiled_ivm_private_note_profile_v1_with_randomness_policies(
        TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
        CURVE_PROVER_RANDOMNESS_POLICY_V1,
    )
}

fn compiled_ivm_private_note_profile_v1_with_randomness_policies(
    proof_randomness_policy: &[u8],
    wallet_randomness_policy: &[u8],
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
    let profile_digest =
        proof_managed_note_stark_profile_digest_v1(IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1);
    if PRIVATE_NOTE_MAX_INPUTS_V1 != IVM_PRIVATE_NOTE_MAX_INPUTS_V1 as usize
        || PRIVATE_NOTE_MAX_OUTPUTS_V1 != IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1 as usize
        || IVM_PRIVATE_NOTE_MAX_INPUTS_V1 == 0
        || IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1 == 0
        || IVM_PRIVATE_NOTE_MAX_INPUTS_V1 > TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1
        || IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1 > TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        || PRIVATE_NOTE_TREE_DEPTH_V1 != 32
        || PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1 != 16
        || PRIVATE_PROGRAM_INSTRUCTION_BYTES_V1 != 8
        || PRIVATE_PROGRAM_REGISTER_COUNT_V1 != 8
        || PRIVATE_PROGRAM_BYTES_V1
            != 8 + PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1 * PRIVATE_PROGRAM_INSTRUCTION_BYTES_V1
        || PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1 != 224
        || PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1 != *b"IPNE"
        || IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1
            > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
        || profile_digest != IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1
        || IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1 == [0; 32]
        || validate_ivm_private_note_stark_profile_v1().is_err()
        || !IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1
            .windows(CURVE_PROVER_RANDOMNESS_POLICY_V1.len())
            .any(|window| window == CURVE_PROVER_RANDOMNESS_POLICY_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaIvmPrivateNoteStarkStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            }
        })?;
    let bootstrap_schema_digest =
        canonical_schema_digest_v1::<PrivacyIvmPrivateNotePoolBootstrapV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            }
        })?;
    let input_limit = IVM_PRIVATE_NOTE_MAX_INPUTS_V1.to_be_bytes();
    let output_limit = IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1.to_be_bytes();
    let tree_depth = usize_to_u64_v1(PRIVATE_NOTE_TREE_DEPTH_V1).to_be_bytes();
    let program_bytes = usize_to_u64_v1(PRIVATE_PROGRAM_BYTES_V1).to_be_bytes();
    let program_instruction_count =
        usize_to_u64_v1(PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1).to_be_bytes();
    let program_instruction_bytes =
        usize_to_u64_v1(PRIVATE_PROGRAM_INSTRUCTION_BYTES_V1).to_be_bytes();
    let program_register_count = usize_to_u64_v1(PRIVATE_PROGRAM_REGISTER_COUNT_V1).to_be_bytes();
    let encrypted_output_bytes =
        usize_to_u64_v1(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1).to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            IVM_PRIVATE_NOTE_PROTOCOL_LABEL_V1,
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
            &IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1,
            IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            IVM_PRIVATE_NOTE_PROTOCOL_LABEL_V1,
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1,
            proof_randomness_policy,
            wallet_randomness_policy,
            IVM_PRIVATE_NOTE_HASH_PROFILE_DESCRIPTOR_V1,
            IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
            &IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1,
            &IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1,
            &PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1,
            &input_limit,
            &output_limit,
            &tree_depth,
            &program_bytes,
            &program_instruction_count,
            &program_instruction_bytes,
            &program_register_count,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            IVM_PRIVATE_NOTE_PROTOCOL_LABEL_V1,
            IVM_PRIVATE_NOTE_IMPLEMENTATION_PROVENANCE_V1,
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            IVM_PRIVATE_NOTE_PROOF_WIRE_LABEL_V1,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            IVM_PRIVATE_NOTE_RUNTIME_CONTEXT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_FRONTIER_SCHEMA_V1,
            IVM_PRIVATE_NOTE_VERIFIED_EFFECT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_WALLET_CIPHERTEXT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1,
            proof_randomness_policy,
            wallet_randomness_policy,
            IVM_PRIVATE_NOTE_HASH_PROFILE_DESCRIPTOR_V1,
            IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
            &IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1,
            &IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &input_limit,
            &output_limit,
            &tree_depth,
            &program_bytes,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            IVM_PRIVATE_NOTE_PROTOCOL_LABEL_V1,
            IVM_PRIVATE_NOTE_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:stark-fri-sha256-goldilocks",
            b"engine:native-goldilocks-stark-fri",
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            IVM_PRIVATE_NOTE_PROOF_WIRE_LABEL_V1,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            IVM_PRIVATE_NOTE_RUNTIME_CONTEXT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_FRONTIER_SCHEMA_V1,
            IVM_PRIVATE_NOTE_VERIFIED_EFFECT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_WALLET_CIPHERTEXT_SCHEMA_V1,
            IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1,
            proof_randomness_policy,
            wallet_randomness_policy,
            IVM_PRIVATE_NOTE_HASH_PROFILE_DESCRIPTOR_V1,
            IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
            &IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1,
            &input_limit,
            &output_limit,
            &tree_depth,
            &program_bytes,
            &program_instruction_count,
            &program_instruction_bytes,
            &program_register_count,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
        engine_id: PrivacyEngineIdV1::NativeGoldilocksStarkFri,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
            IvmPrivateNoteActivationLimitsV1 {
                max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
            },
        ),
    })
}

fn compiled_pq_masp_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    compiled_pq_masp_profile_v1_with_schemas(
        PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
        PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1,
    )
}

fn compiled_pq_masp_profile_v1_with_schemas(
    wallet_ciphertext_schema: &[u8],
    verified_effect_schema: &[u8],
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV0;
    let profile_digest =
        proof_managed_note_stark_profile_digest_v1(PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1);
    if PQ_MASP_INPUT_BOUND_V1 != PQ_MASP_MAX_INPUTS_V1 as usize
        || PQ_MASP_OUTPUT_BOUND_V1 != PQ_MASP_MAX_OUTPUTS_V1 as usize
        || PQ_MASP_MAX_INPUTS_V1 == 0
        || PQ_MASP_MAX_OUTPUTS_V1 == 0
        || PQ_MASP_MAX_INPUTS_V1 > TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1
        || PQ_MASP_MAX_OUTPUTS_V1 > TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        || PQ_MASP_TREE_DEPTH_V1 != 32
        || AUTHORIZATION_MAGIC_V1 != b"PQA1"
        || ENCRYPTED_OUTPUT_MAGIC_V1 != b"PQE1"
        || ML_DSA_65_PUBLIC_KEY_BYTES_V1 != 1_952
        || ML_DSA_65_SIGNATURE_BYTES_V1 != 3_309
        || ML_KEM_768_PUBLIC_KEY_BYTES_V1 != 1_184
        || ML_KEM_768_CIPHERTEXT_BYTES_V1 != 1_088
        || XCHACHA20_NONCE_BYTES_V1 != 24
        || PQ_MASP_ENCRYPTED_OUTPUT_BYTES_V1 != 1_344
        || PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1
            != 8 + ML_DSA_65_PUBLIC_KEY_BYTES_V1 + ML_DSA_65_SIGNATURE_BYTES_V1
        || PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1
            != TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
        || PQ_MASP_MAX_STARK_PROOF_BYTES_V1 + PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1
            != PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1
        || profile_digest != PQ_MASP_STARK_PROFILE_DIGEST_V1
        || PQ_MASP_STARK_KAT_PROOF_SHA256_V1 == [0; 32]
        || PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1 == [0; 32]
        || PQ_MASP_ENCRYPTED_OUTPUT_KAT_SHA256_V1 == [0; 32]
        || PQ_MASP_AUTHORIZATION_WIRE_KAT_SHA256_V1 == [0; 32]
        || validate_pq_masp_stark_profile_v1().is_err()
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let statement_schema_digest =
        canonical_schema_digest_v1::<PqMaspStarkStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            }
        })?;
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyPqMaspPoolBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let input_limit = PQ_MASP_MAX_INPUTS_V1.to_be_bytes();
    let output_limit = PQ_MASP_MAX_OUTPUTS_V1.to_be_bytes();
    let tree_depth = usize_to_u64_v1(PQ_MASP_TREE_DEPTH_V1).to_be_bytes();
    let authorization_header_bytes =
        usize_to_u64_v1(PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1).to_be_bytes();
    let maximum_authorization_bytes =
        usize_to_u64_v1(PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1).to_be_bytes();
    let maximum_stark_bytes = usize_to_u64_v1(PQ_MASP_MAX_STARK_PROOF_BYTES_V1).to_be_bytes();
    let encrypted_output_bytes = usize_to_u64_v1(PQ_MASP_ENCRYPTED_OUTPUT_BYTES_V1).to_be_bytes();
    let mldsa_public_key_bytes = usize_to_u64_v1(ML_DSA_65_PUBLIC_KEY_BYTES_V1).to_be_bytes();
    let mldsa_signature_bytes = usize_to_u64_v1(ML_DSA_65_SIGNATURE_BYTES_V1).to_be_bytes();
    let mlkem_public_key_bytes = usize_to_u64_v1(ML_KEM_768_PUBLIC_KEY_BYTES_V1).to_be_bytes();
    let mlkem_ciphertext_bytes = usize_to_u64_v1(ML_KEM_768_CIPHERTEXT_BYTES_V1).to_be_bytes();
    let nonce_bytes = usize_to_u64_v1(XCHACHA20_NONCE_BYTES_V1).to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            PQ_MASP_PROTOCOL_LABEL_V1,
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
            &PQ_MASP_STARK_PROFILE_DIGEST_V1,
            PQ_MASP_AGGREGATE_AIR_DESCRIPTOR_V1,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            PQ_MASP_PROTOCOL_LABEL_V1,
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            PQ_MASP_ENGINE_DESCRIPTOR_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            PQ_MASP_HASH_PROFILE_DESCRIPTOR_V1,
            PQ_MASP_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
            &PQ_MASP_STARK_PROFILE_DIGEST_V1,
            &PQ_MASP_STARK_KAT_PROOF_SHA256_V1,
            &PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1,
            &PQ_MASP_ENCRYPTED_OUTPUT_KAT_SHA256_V1,
            &PQ_MASP_AUTHORIZATION_WIRE_KAT_SHA256_V1,
            AUTHORIZATION_MAGIC_V1,
            ENCRYPTED_OUTPUT_MAGIC_V1,
            &input_limit,
            &output_limit,
            &tree_depth,
            &authorization_header_bytes,
            &maximum_authorization_bytes,
            &maximum_stark_bytes,
            &encrypted_output_bytes,
            &mldsa_public_key_bytes,
            &mldsa_signature_bytes,
            &mlkem_public_key_bytes,
            &mlkem_ciphertext_bytes,
            &nonce_bytes,
        ],
    );
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            PQ_MASP_PROTOCOL_LABEL_V1,
            PQ_MASP_IMPLEMENTATION_PROVENANCE_V1,
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            PQ_MASP_PROOF_WIRE_LABEL_V1,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            PQ_MASP_RUNTIME_CONTEXT_SCHEMA_V1,
            PQ_MASP_FRONTIER_SCHEMA_V1,
            PQ_MASP_AUTHORIZATION_SCHEMA_V1,
            wallet_ciphertext_schema,
            verified_effect_schema,
            PQ_MASP_ENGINE_DESCRIPTOR_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            PQ_MASP_HASH_PROFILE_DESCRIPTOR_V1,
            PQ_MASP_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
            &PQ_MASP_STARK_PROFILE_DIGEST_V1,
            &PQ_MASP_STARK_KAT_PROOF_SHA256_V1,
            &PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1,
            &PQ_MASP_ENCRYPTED_OUTPUT_KAT_SHA256_V1,
            &PQ_MASP_AUTHORIZATION_WIRE_KAT_SHA256_V1,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &input_limit,
            &output_limit,
            &tree_depth,
            &maximum_authorization_bytes,
            &maximum_stark_bytes,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            PQ_MASP_PROTOCOL_LABEL_V1,
            PQ_MASP_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:stark-fri-sha256-goldilocks",
            b"engine:native-goldilocks-stark-fri",
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            PQ_MASP_PROOF_WIRE_LABEL_V1,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            PQ_MASP_RUNTIME_CONTEXT_SCHEMA_V1,
            PQ_MASP_FRONTIER_SCHEMA_V1,
            PQ_MASP_AUTHORIZATION_SCHEMA_V1,
            wallet_ciphertext_schema,
            verified_effect_schema,
            PQ_MASP_ENGINE_DESCRIPTOR_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            PQ_MASP_HASH_PROFILE_DESCRIPTOR_V1,
            PQ_MASP_AGGREGATE_AIR_DESCRIPTOR_V1,
            PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1,
            &PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1,
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
            &PQ_MASP_STARK_PROFILE_DIGEST_V1,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &PQ_MASP_STARK_KAT_PROOF_SHA256_V1,
            &PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1,
            &PQ_MASP_ENCRYPTED_OUTPUT_KAT_SHA256_V1,
            &PQ_MASP_AUTHORIZATION_WIRE_KAT_SHA256_V1,
            &input_limit,
            &output_limit,
            &tree_depth,
            &authorization_header_bytes,
            &maximum_authorization_bytes,
            &maximum_stark_bytes,
            &encrypted_output_bytes,
            &mldsa_public_key_bytes,
            &mldsa_signature_bytes,
            &mlkem_public_key_bytes,
            &mlkem_ciphertext_bytes,
            &nonce_bytes,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
        engine_id: PrivacyEngineIdV1::NativeGoldilocksStarkFri,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(
            PqMaspActivationLimitsV1 {
                max_input_count: PQ_MASP_MAX_INPUTS_V1,
                max_output_count: PQ_MASP_MAX_OUTPUTS_V1,
            },
        ),
    })
}

fn compiled_fcmp_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    compiled_fcmp_profile_v1_with_randomness_policy(CURVE_PROVER_RANDOMNESS_POLICY_V1)
}

fn compiled_fcmp_profile_v1_with_randomness_policy(
    randomness_policy: &[u8],
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
    let native_input_limit = u32::try_from(FCMP_MAX_INPUTS_NATIVE_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let native_output_limit = u32::try_from(FCMP_MAX_OUTPUTS_NATIVE_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let max_wire = fcmp_plus_plus_wire_size_v1(
        FCMP_MAX_INPUTS_NATIVE_V1,
        FCMP_MAX_TREE_LAYERS_V1,
        FCMP_MAX_OUTPUTS_NATIVE_V1,
    )
    .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let min_wire = fcmp_plus_plus_wire_size_v1(1, 1, 1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let bp_plus_generator_digest = fcmp_bp_plus_generator_digest_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if native_input_limit != FCMP_MAX_INPUTS_V1
        || native_output_limit != FCMP_MAX_OUTPUTS_V1
        || FCMP_MAX_INPUTS_V1 > TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1
        || FCMP_MAX_OUTPUTS_V1 > TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        || FCMP_POINT_BYTES_V1 != 32
        || FCMP_OUTPUT_TUPLE_BYTES_V1 != 96
        || PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1 != 280
        || FCMP_PROOF_WIRE_HEADER_BYTES_V1 != 8
        || FCMP_PROOF_INPUT_BYTES_V1 != 480
        || FCMP_SAL_PROOF_BYTES_V1 != 384
        || FCMP_LAYER_ONE_LEN_V1 != 38
        || FCMP_LAYER_TWO_LEN_V1 != 18
        || min_wire != FCMP_MIN_PROOF_WIRE_BYTES_V1
        || max_wire != FCMP_MAX_PROOF_WIRE_BYTES_V1
        || max_wire
            > usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1).map_err(|_| {
                CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id }
            })?
        || FCMP_NATIVE_KAT_WIRE_SHA256_V1 == [0; 32]
        || FCMP_NATIVE_KAT_PUBLIC_SHA256_V1 == [0; 32]
        || bp_plus_generator_digest != FCMP_BP_PLUS_GENERATOR_DIGEST_V1
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let statement_schema_digest = canonical_schema_digest_v1::<MoneroFcmpPlusPlusStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyFcmpPoolBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let compiled_profile_digest = fcmp_compiled_profile_digest_v1();
    let input_limit = FCMP_MAX_INPUTS_V1.to_be_bytes();
    let output_limit = FCMP_MAX_OUTPUTS_V1.to_be_bytes();
    let max_layers = [FCMP_MAX_TREE_LAYERS_V1];
    let layer_one_width = u64::try_from(FCMP_LAYER_ONE_LEN_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let layer_two_width = u64::try_from(FCMP_LAYER_TWO_LEN_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let point_bytes = u64::try_from(FCMP_POINT_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let tuple_bytes = u64::try_from(FCMP_OUTPUT_TUPLE_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let proof_input_bytes = u64::try_from(FCMP_PROOF_INPUT_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let encrypted_output_bytes = u64::try_from(PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let min_wire = u64::try_from(min_wire)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let max_wire = u64::try_from(max_wire)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            FCMP_PROTOCOL_LABEL_V1,
            FCMP_PARAMETER_SET_LABEL_V1,
            FCMP_SOURCE_PROFILE_V1,
            FCMP_UPSTREAM_REVISION_V1.as_bytes(),
            FCMP_BP_PLUS_UPSTREAM_REVISION_V1.as_bytes(),
            &FCMP_BP_PLUS_GENERATOR_DIGEST_V1,
            FCMP_COMPILED_PROFILE_DESCRIPTOR_V1,
            &compiled_profile_digest,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            FCMP_PROTOCOL_LABEL_V1,
            FCMP_PARAMETER_SET_LABEL_V1,
            FCMP_SOURCE_PROFILE_V1,
            FCMP_UPSTREAM_REVISION_V1.as_bytes(),
            FCMP_BP_PLUS_UPSTREAM_REVISION_V1.as_bytes(),
            &FCMP_BP_PLUS_GENERATOR_DIGEST_V1,
            FCMP_COMPILED_PROFILE_DESCRIPTOR_V1,
            randomness_policy,
            &FCMP_PROOF_WIRE_MAGIC_V1,
            &PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1,
            FCMP_WALLET_CIPHERTEXT_SCHEMA_V1,
            &compiled_profile_digest,
            &FCMP_NATIVE_KAT_WIRE_SHA256_V1,
            &FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
            &input_limit,
            &output_limit,
            &max_layers,
            &layer_one_width,
            &layer_two_width,
            &point_bytes,
            &tuple_bytes,
            &proof_input_bytes,
            &encrypted_output_bytes,
            &min_wire,
            &max_wire,
        ],
    );
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            FCMP_PROTOCOL_LABEL_V1,
            FCMP_SOURCE_PROFILE_V1,
            FCMP_UPSTREAM_REVISION_V1.as_bytes(),
            FCMP_BP_PLUS_UPSTREAM_REVISION_V1.as_bytes(),
            &FCMP_BP_PLUS_GENERATOR_DIGEST_V1,
            FCMP_PARAMETER_SET_LABEL_V1,
            FCMP_PROOF_WIRE_LABEL_V1,
            FCMP_RUNTIME_CONTEXT_SCHEMA_V1,
            FCMP_FRONTIER_SCHEMA_V1,
            FCMP_VERIFIED_EFFECT_SCHEMA_V1,
            FCMP_WALLET_CIPHERTEXT_SCHEMA_V1,
            FCMP_COMPILED_PROFILE_DESCRIPTOR_V1,
            randomness_policy,
            &compiled_profile_digest,
            &FCMP_NATIVE_KAT_WIRE_SHA256_V1,
            &FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &input_limit,
            &output_limit,
            &max_layers,
            &min_wire,
            &max_wire,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            FCMP_PROTOCOL_LABEL_V1,
            FCMP_SOURCE_PROFILE_V1,
            FCMP_UPSTREAM_REVISION_V1.as_bytes(),
            FCMP_BP_PLUS_UPSTREAM_REVISION_V1.as_bytes(),
            &FCMP_BP_PLUS_GENERATOR_DIGEST_V1,
            b"proof-system:fcmp-plus-plus-curve-tree-bulletproofs",
            b"engine:native-fcmp-plus-plus",
            FCMP_PARAMETER_SET_LABEL_V1,
            FCMP_PROOF_WIRE_LABEL_V1,
            FCMP_RUNTIME_CONTEXT_SCHEMA_V1,
            FCMP_FRONTIER_SCHEMA_V1,
            FCMP_VERIFIED_EFFECT_SCHEMA_V1,
            FCMP_WALLET_CIPHERTEXT_SCHEMA_V1,
            FCMP_COMPILED_PROFILE_DESCRIPTOR_V1,
            randomness_policy,
            &compiled_profile_digest,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &FCMP_NATIVE_KAT_WIRE_SHA256_V1,
            &FCMP_NATIVE_KAT_PUBLIC_SHA256_V1,
            &input_limit,
            &output_limit,
            &max_layers,
            &min_wire,
            &max_wire,
            &encrypted_output_bytes,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs,
        engine_id: PrivacyEngineIdV1::NativeFcmpPlusPlus,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(
            FcmpActivationLimitsV1 {
                max_input_count: FCMP_MAX_INPUTS_V1,
                max_output_count: FCMP_MAX_OUTPUTS_V1,
            },
        ),
    })
}

fn compiled_orchard_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    compiled_orchard_profile_v1_with_randomness_policies(
        TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
        ORCHARD_PROVER_RANDOMNESS_POLICY_V1,
    )
}

fn compiled_orchard_profile_v1_with_randomness_policies(
    source_randomness_policy: &[u8],
    bridge_randomness_policy: &[u8],
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::OrchardHalo2ActionsV1;
    if ORCHARD_ENGINE_MAX_ACTIONS_V1
        != usize::try_from(ORCHARD_MODEL_MAX_ACTIONS_V1)
            .expect("compiled Orchard action count fits usize")
        || ORCHARD_MODEL_MAX_ACTIONS_V1 > TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        || ORCHARD_MODEL_MAX_ACTIONS_V1 > TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let one_action_wire = orchard_authorization_wire_size_v1(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let two_action_wire = orchard_authorization_wire_size_v1(ORCHARD_ENGINE_MAX_ACTIONS_V1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if one_action_wire == 0
        || one_action_wire >= two_action_wire
        || two_action_wire > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let action_count = ORCHARD_MODEL_MAX_ACTIONS_V1.to_be_bytes();
    let one_action_wire = one_action_wire.to_be_bytes();
    let two_action_wire = two_action_wire.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let empty_root = orchard_empty_root_v1();
    if empty_root == [0; 32] {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let statement_schema_digest = canonical_schema_digest_v1::<OrchardHalo2ActionsStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyOrchardPoolBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_UPSTREAM_CRATE_VERSION_V1.as_bytes(),
            ORCHARD_UPSTREAM_REVISION_V1.as_bytes(),
            ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1.as_bytes(),
            &empty_root,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            source_randomness_policy,
            bridge_randomness_policy,
            ORCHARD_UPSTREAM_CRATE_VERSION_V1.as_bytes(),
            ORCHARD_UPSTREAM_REVISION_V1.as_bytes(),
            ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1.as_bytes(),
            &empty_root,
            &action_count,
            &one_action_wire,
            &two_action_wire,
        ],
    );
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_IMPLEMENTATION_PROVENANCE_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_PROOF_WIRE_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            source_randomness_policy,
            bridge_randomness_policy,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            ORCHARD_FRONTIER_SCHEMA_V1,
            ORCHARD_VERIFIED_EFFECT_SCHEMA_V1,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &action_count,
            &one_action_wire,
            &two_action_wire,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:halo2-ipa-pasta",
            b"engine:native-halo2-orchard",
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_PROOF_WIRE_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            source_randomness_policy,
            bridge_randomness_policy,
            PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_V1,
            ORCHARD_FRONTIER_SCHEMA_V1,
            ORCHARD_VERIFIED_EFFECT_SCHEMA_V1,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &empty_root,
            &action_count,
            &one_action_wire,
            &two_action_wire,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::Halo2IpaPasta,
        engine_id: PrivacyEngineIdV1::NativeHalo2Orchard,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
            OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1,
            },
        ),
    })
}

/// Require exact compiled cryptographic bindings and bounded governed policy.
///
/// # Errors
///
/// Returns a typed mismatch for the first differing consensus binding, or an
/// unavailable/initialization error when no executable profile exists.
pub fn validate_compiled_privacy_activation_v1(
    activation: &PrivacyProtocolActivationRecordV1,
) -> Result<(), CompiledPrivacyProfileValidationErrorV1> {
    let compiled = compiled_privacy_profile_v1(activation.protocol_id)
        .map_err(CompiledPrivacyProfileValidationErrorV1::Profile)?;
    validate_compiled_privacy_activation_against_profile_v1(activation, &compiled)
}

/// Validate a governance record against an already selected compiled profile.
///
/// This contains the consensus-critical binding comparison shared by normal
/// activation admission and pre-activation release-candidate evidence. The
/// caller selecting `compiled` is responsible for the separate availability
/// and readiness decision.
pub(crate) fn validate_compiled_privacy_activation_against_profile_v1(
    activation: &PrivacyProtocolActivationRecordV1,
    compiled: &CompiledPrivacyProfileV1,
) -> Result<(), CompiledPrivacyProfileValidationErrorV1> {
    if activation.protocol_id != compiled.protocol_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ProtocolMismatch);
    }
    if activation.proof_system_id != compiled.proof_system_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch);
    }
    if activation.engine_id != compiled.engine_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineMismatch);
    }
    if activation.parameter_id != compiled.parameter_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch);
    }
    if activation.parameter_digest != compiled.parameter_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch);
    }
    if activation.verifier_digest != compiled.verifier_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch);
    }
    if activation.statement_schema_digest != compiled.statement_schema_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch);
    }
    if activation.engine_manifest_digest != compiled.engine_manifest_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch);
    }
    activation
        .protocol_limits
        .validate_with_ceiling(&compiled.protocol_limits)
        .map_err(|_| CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch)?;
    if activation.assurance != PrivacyAssuranceV1::Experimental {
        return Err(CompiledPrivacyProfileValidationErrorV1::AssuranceMismatch);
    }
    Ok(())
}

#[cfg(feature = "zk-stark")]
fn compiled_zk_ace_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
    let compiled_profile_digest = zk_ace_compiled_profile_digest_v1();
    let proof_bytes = u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1);
    if compiled_profile_digest == [0; 32]
        || zk_ace_stark_profile_descriptor_v1().is_empty()
        || proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let poseidon_manifest = ZK_ACE_POSEIDON_MANIFEST_SHA256_V1.as_bytes();
    let stark_profile = zk_ace_stark_profile_descriptor_v1();
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<ZkAcePqAuthorizationStatementV1>()
        .map_err(|source| {
        CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
            protocol_id,
            source,
        }
    })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes(),
            ZK_ACE_PROOF_WIRE_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            b"proof-system:stark-fri-sha256-goldilocks",
            b"engine:native-goldilocks-stark-fri",
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes(),
            ZK_ACE_PROOF_WIRE_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
        engine_id: PrivacyEngineIdV1::NativeGoldilocksStarkFri,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0,
    })
}

fn compiled_zk_ams_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
    let readiness = zk_ams_mkhe_readiness_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if !readiness.is_ready() {
        return Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id });
    }
    let compiled_relation_digest = zk_ams_compiled_profile_digest_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    zk_ams_profile_material_v1(compiled_relation_digest)
}

/// Derive the deterministic ZK-AMS release-candidate profile material.
///
/// This accessor is restricted to unsigned/offline intent construction,
/// release KATs, adversarial tests, and resource measurements that must bind
/// the exact candidate before activation. Success does not imply that the MKHE
/// release gates are closed or that the profile may be activated, signed, or
/// submitted. Production paths must use [`compiled_privacy_profile_v1`].
pub fn zk_ams_release_candidate_profile_material_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
    let compiled_relation_digest = zk_ams_release_candidate_profile_digest_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    zk_ams_profile_material_v1(compiled_relation_digest)
}

fn zk_ams_profile_material_v1(
    compiled_relation_digest: [u8; 32],
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
    if ZK_AMS_RING_SIZES_V1 != [16, 32, 64]
        || ZK_AMS_MODEL_RING_SIZES_V1 != [16, 32, 64]
        || ZK_AMS_MAX_RING_SIZE_V1 != 64
        || ZK_AMS_MAX_BATCH_SIZE_V1 != 8
        || u32::try_from(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1) != Ok(ZK_AMS_MAX_BATCH_SIZE_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let dimensions = zk_ams_admission_relation_dimensions_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let encode_usize = |value: usize| {
        u64::try_from(value)
            .map(u64::to_be_bytes)
            .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })
    };
    let variable_count = encode_usize(dimensions.variable_count)?;
    let constraint_count = encode_usize(dimensions.constraint_count)?;
    let public_input_count = encode_usize(dimensions.public_input_count)?;
    let witness_commitment_points = encode_usize(dimensions.witness_commitment_points)?;
    let error_commitment_points = encode_usize(dimensions.error_commitment_points)?;
    let outer_sumcheck_rounds = encode_usize(dimensions.outer_sumcheck_rounds)?;
    let inner_sumcheck_rounds = encode_usize(dimensions.inner_sumcheck_rounds)?;
    let declared_public_input_count = encode_usize(ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1)?;
    if public_input_count != declared_public_input_count {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let phc_payload_bytes = encode_usize(ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1)?;

    let relation_proof_cap_value = u64::try_from(MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let batch_proof_cap_value = u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let possession_proof_cap_value = u64::try_from(MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let provision_proof_cap_value = u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let lsag_decode_allocation_value = u64::try_from(ZK_AMS_LSAG_DECODE_ALLOCATION_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let global_proof_cap_value = u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1);
    if batch_proof_cap_value > global_proof_cap_value
        || provision_proof_cap_value > global_proof_cap_value
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let relation_proof_cap = relation_proof_cap_value.to_be_bytes();
    let batch_proof_cap = batch_proof_cap_value.to_be_bytes();
    let possession_proof_cap = possession_proof_cap_value.to_be_bytes();
    let provision_proof_cap = provision_proof_cap_value.to_be_bytes();
    let lsag_decode_allocation = lsag_decode_allocation_value.to_be_bytes();
    let global_proof_cap = global_proof_cap_value.to_be_bytes();

    let max_batch_size = ZK_AMS_MAX_BATCH_SIZE_V1.to_be_bytes();
    let ring_size_16 = ZK_AMS_MODEL_RING_SIZES_V1[0].to_be_bytes();
    let ring_size_32 = ZK_AMS_MODEL_RING_SIZES_V1[1].to_be_bytes();
    let ring_size_64 = ZK_AMS_MODEL_RING_SIZES_V1[2].to_be_bytes();
    let admission_possession_version = [ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1];
    let batch_admission_version = [ZK_AMS_BATCH_ADMISSION_PROOF_VERSION_V1];
    let lsag_version = [ZK_AMS_LSAG_PROOF_VERSION_V1];
    let t256_generator_digest = zk_ams_t256_generator_digest_v1();
    let combined_generator_digest = zk_ams_generator_digest_v1();
    if compiled_relation_digest == [0; 32]
        || t256_generator_digest == [0; 32]
        || combined_generator_digest == [0; 32]
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            ZK_AMS_LSAG_SUITE_V1,
            ZK_AMS_ADMISSION_POSSESSION_SUITE_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &variable_count,
            &constraint_count,
            &public_input_count,
            &witness_commitment_points,
            &error_commitment_points,
            &outer_sumcheck_rounds,
            &inner_sumcheck_rounds,
            &phc_payload_bytes,
            &max_batch_size,
            &ring_size_16,
            &ring_size_32,
            &ring_size_64,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaZkAmsStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            }
        })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_IMPLEMENTATION_PROVENANCE_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1,
            ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1,
            ZK_AMS_LSAG_SUITE_V1,
            ZK_AMS_ADMISSION_POSSESSION_SUITE_V1,
            &admission_possession_version,
            &batch_admission_version,
            &lsag_version,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &variable_count,
            &constraint_count,
            &public_input_count,
            &witness_commitment_points,
            &error_commitment_points,
            &outer_sumcheck_rounds,
            &inner_sumcheck_rounds,
            &phc_payload_bytes,
            &relation_proof_cap,
            &possession_proof_cap,
            &batch_proof_cap,
            &provision_proof_cap,
            &lsag_decode_allocation,
            &max_batch_size,
            &ring_size_16,
            &ring_size_32,
            &ring_size_64,
            ZK_AMS_BATCH_EFFECT_SCHEMA_V1,
            ZK_AMS_PROVISION_EFFECT_SCHEMA_V1,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
            b"engine:native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1,
            ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1,
            ZK_AMS_BATCH_EFFECT_SCHEMA_V1,
            ZK_AMS_PROVISION_EFFECT_SCHEMA_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &relation_proof_cap,
            &possession_proof_cap,
            &batch_proof_cap,
            &provision_proof_cap,
            &lsag_decode_allocation,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512,
        engine_id: PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
            max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
            max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
        }),
    })
}

fn compiled_vega_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::VegaExistingCredentialZkV0;
    let canonical_relation_digest = vega_mdl_canonical_relation_digest_v1();
    let compiled_profile_digest = vega_mdl_compiled_profile_digest_v1();
    let proof_bytes = u64::try_from(MAX_VEGA_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if canonical_relation_digest == [0; 32]
        || compiled_profile_digest == [0; 32]
        || proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let issuer_record_version = VEGA_ISSUER_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
    let issuer_record_cap = u64::try_from(VEGA_MAX_ISSUER_RECORDS_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let issuer_lineage_cap = u64::try_from(VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let device_authentication_frame_version = [VEGA_MDL_DEVICE_AUTHENTICATION_FRAME_VERSION_V1];
    let issuer_authentication_bytes =
        u64::try_from(VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1)
            .map_err(
                |_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id },
            )?
            .to_be_bytes();
    let mso_payload_bytes = u64::try_from(VEGA_MDL_MSO_PAYLOAD_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let birth_item_bytes = u64::try_from(VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let birth_random_bytes = u64::try_from(VEGA_MDL_BIRTH_RANDOM_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let full_date_bytes = u64::try_from(VEGA_MDL_FULL_DATE_TEXT_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let rfc3339_bytes = u64::try_from(VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let min_presentation_year = VEGA_MDL_MIN_PRESENTATION_YEAR_V1.to_be_bytes();
    let max_presentation_year = VEGA_MDL_MAX_PRESENTATION_YEAR_V1.to_be_bytes();
    let age_threshold_bounds = [
        VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1,
        VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1,
    ];
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<VegaExistingCredentialStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1;
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:vega-neutron-nova-spartan-hyrax-t256",
            b"engine:native-vega",
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            VEGA_PROOF_WIRE_LABEL_V1,
            &canonical_relation_digest,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            VEGA_CANONICAL_MDL_WITNESS_SCHEMA_V1,
            VEGA_CANONICAL_SIGNATURE_PREFLIGHT_POLICY_V1,
            &issuer_authentication_bytes,
            &mso_payload_bytes,
            &birth_item_bytes,
            &birth_random_bytes,
            &full_date_bytes,
            &rfc3339_bytes,
            &min_presentation_year,
            &max_presentation_year,
            &age_threshold_bounds,
            VEGA_AUTHORITATIVE_ISSUER_RUNTIME_SCHEMA_V1,
            VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1,
            VEGA_ISSUER_RECORD_HASH_FRAME_DOMAIN_V1,
            &issuer_record_version,
            &issuer_record_cap,
            &issuer_lineage_cap,
            VEGA_DEVICE_AUTHENTICATION_GOVERNANCE_FRAME_SCHEMA_V1,
            VEGA_MDL_DEVICE_AUTHENTICATION_DOMAIN_V1,
            &device_authentication_frame_version,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256,
        engine_id: PrivacyEngineIdV1::NativeVega,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0,
    })
}

fn compiled_anonymous_pgc_profile_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
    if !ANONYMOUS_PGC_FULL_ENGINE_AVAILABLE_V1 {
        return Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id });
    }
    if PGC_PAYMENT_ANONYMITY_SET_SIZES_V1 != [16, 32, 64]
        || PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1 != [16, 32, 64]
        || u32::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1) != Ok(ANONYMOUS_PGC_MAX_RECIPIENTS_V1)
        || u32::try_from(
            *PGC_PAYMENT_ANONYMITY_SET_SIZES_V1
                .last()
                .expect("closed non-empty PGC anonymity-set profile"),
        ) != Ok(ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1)
        || MAX_PGC_PAYMENT_PROOF_BYTES_V1 > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
        || MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1 > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let parameters = AnonymousPgcParametersV1::get()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            ANONYMOUS_PGC_PARAMETER_SET_LABEL_V1,
            PGC_SOURCE_PROFILE_V1,
        ],
    );

    let payment_version = [PGC_PAYMENT_PROOF_VERSION_V1];
    let payment_count_16 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[0])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_count_32 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[1])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_count_64 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[2])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_max_recipients = u32::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_proof_cap = u32::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();

    let bootstrap_version = [PGC_BOOTSTRAP_PROOF_VERSION_V1];
    let bootstrap_initial_epoch = PGC_BOOTSTRAP_INITIAL_EPOCH_V1.to_be_bytes();
    let bootstrap_count_16 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[0])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_count_32 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[1])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_count_64 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[2])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_namespace_cap = u32::try_from(MAX_PGC_BOOTSTRAP_NAMESPACE_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_max_aggregate = PGC_BOOTSTRAP_MAX_AGGREGATE_BALANCE_V1.to_be_bytes();
    let bootstrap_proof_cap = u32::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let generator_digest = parameters.generator_digest();
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyPgcAccountBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            PGC_SOURCE_PROFILE_V1,
            PGC_PAYMENT_SUITE_V1,
            &payment_version,
            ANONYMOUS_PGC_PAYMENT_PROOF_WIRE_LABEL_V1,
            PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            &payment_count_16,
            &payment_count_32,
            &payment_count_64,
            &payment_max_recipients,
            &payment_proof_cap,
            PGC_BOOTSTRAP_SUITE_V1,
            &bootstrap_version,
            &bootstrap_initial_epoch,
            ANONYMOUS_PGC_BOOTSTRAP_PROOF_WIRE_LABEL_V1,
            &bootstrap_count_16,
            &bootstrap_count_32,
            &bootstrap_count_64,
            &bootstrap_namespace_cap,
            &bootstrap_max_aggregate,
            PGC_BOOTSTRAP_TABLE_DIGEST_DOMAIN_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
            &bootstrap_proof_cap,
            &bootstrap_schema_digest,
            PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
            ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1,
            ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1,
            &generator_digest,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<AnonymousPgcKOutOfNStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let parameter_digest = parameters.parameter_digest();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            ANONYMOUS_PGC_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:anonymous-pgc-p256",
            b"engine:native-anonymous-pgc-p256",
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
            &bootstrap_initial_epoch,
            PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
            ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1,
            ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1,
            &payment_proof_cap,
            &bootstrap_proof_cap,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::AnonymousPgcP256,
        engine_id: PrivacyEngineIdV1::NativeAnonymousPgcP256,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                max_recipient_count: ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
            },
        ),
    })
}

fn bootle_lantern_parameter_digest_v1(
    public_parameter_seed: &[u8; 32],
    sampling_profile_digest: &[u8; 32],
) -> [u8; 32] {
    let ring_degree = u64::try_from(APPLICATION_RING_DEGREE_V1)
        .expect("fixed Bootle/Lantern ring degree fits u64")
        .to_be_bytes();
    let application_modulus = BOOTLE_LANTERN_APPLICATION_MODULUS_V1.to_be_bytes();
    let proof_modulus = BOOTLE_LANTERN_PROOF_MODULUS_V1.to_be_bytes();
    let decomposition_bits = [DECOMPOSITION_BITS_V1];
    let compression_gamma = COMPRESSION_GAMMA_V1.to_be_bytes();
    let compression_modulus = COMPRESSION_MODULUS_V1.to_be_bytes();
    let relation_rows = u64::try_from(APPLICATION_ROWS_V1)
        .expect("fixed Bootle/Lantern row count fits u64")
        .to_be_bytes();
    let relation_columns = u64::try_from(APPLICATION_WITNESS_POLYNOMIALS_V1)
        .expect("fixed Bootle/Lantern column count fits u64")
        .to_be_bytes();
    let quotient_bound = APPLICATION_RELATION_QUOTIENT_BOUND_V1.to_be_bytes();
    let randomness_norm_bound = RANDOMNESS_NORM_SQUARED_BOUND_V1.to_be_bytes();
    let signature_norm_bound = SIGNATURE_NORM_SQUARED_BOUND_V1.to_be_bytes();
    let response_norm_bound = RESPONSE_NORM_SQUARED_BOUND_V1.to_be_bytes();
    let challenge_omega = CHALLENGE_OMEGA_V1.to_be_bytes();
    let challenge_set_bits = CHALLENGE_SET_BITS_V1.to_be_bytes();
    let challenge_norm_power = [CHALLENGE_NORM_POWER_V1];
    let challenge_norm_root_degree = [CHALLENGE_NORM_ROOT_DEGREE_V1];
    let challenge_eta = CHALLENGE_ETA_V1.to_be_bytes();
    let challenge_candidate_attempts = MAX_CHALLENGE_CANDIDATE_ATTEMPTS_V1.to_be_bytes();
    let scope_acceptance_limit = BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1.to_be_bytes();
    let scope_coefficient_attempts = BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1.to_be_bytes();
    let credential_randomness_polynomials = u64::try_from(CREDENTIAL_RANDOMNESS_POLYNOMIALS_V1)
        .expect("fixed credential-randomness width fits u64")
        .to_be_bytes();
    let credential_randomness_norm = CREDENTIAL_RANDOMNESS_NORM_SQUARED_BOUND_V1.to_be_bytes();
    let credential_vector_attempts = MAX_CREDENTIAL_RANDOMNESS_VECTOR_ATTEMPTS_V1.to_be_bytes();
    let credential_coefficient_proposals =
        MAX_CREDENTIAL_RANDOMNESS_COEFFICIENT_PROPOSALS_V1.to_be_bytes();
    let falcon_keygen_candidates =
        BOOTLE_LANTERN_FALCON512_DEFAULT_KEYGEN_CANDIDATES_V1.to_be_bytes();
    let falcon_keygen_parity_attempts =
        BOOTLE_LANTERN_FALCON512_KEYGEN_PARITY_ATTEMPTS_V1.to_be_bytes();
    let falcon_preimage_coefficient_proposals =
        BOOTLE_LANTERN_FALCON512_PREIMAGE_PROPOSALS_PER_COEFFICIENT_V1.to_be_bytes();
    let falcon_preimage_total_proposals =
        BOOTLE_LANTERN_FALCON512_PREIMAGE_TOTAL_PROPOSALS_V1.to_be_bytes();
    let issuer_keygen_candidates = MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1.to_be_bytes();
    let authorization_id_attempts = MAX_BOOTLE_LANTERN_AUTHORIZATION_ID_ATTEMPTS_V1.to_be_bytes();
    let authorization_lifetime = MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1.to_be_bytes();
    let issuer_preimage_attempts = MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1.to_be_bytes();
    let authorization_wire_bytes = u64::try_from(BLIND_ISSUANCE_AUTHORIZATION_BYTES_V1)
        .expect("fixed ILA1 length fits u64")
        .to_be_bytes();
    let request_wire_bytes = u64::try_from(BLIND_ISSUANCE_REQUEST_BYTES_V1)
        .expect("fixed ILQ1 length fits u64")
        .to_be_bytes();
    let request_header_bytes = u64::try_from(BLIND_ISSUANCE_REQUEST_HEADER_BYTES_V1)
        .expect("fixed ILQ1 header length fits u64")
        .to_be_bytes();
    let request_version = [BLIND_ISSUANCE_REQUEST_VERSION_V1];
    let request_purpose = [BLIND_ISSUANCE_REQUEST_PURPOSE_TAG_V1];
    let request_target_polynomials = BLIND_ISSUANCE_REQUEST_TARGET_POLYNOMIALS_V1.to_be_bytes();
    let request_ring_degree = BLIND_ISSUANCE_REQUEST_RING_DEGREE_V1.to_be_bytes();
    let request_proof_bytes = u32::try_from(BOOTLE_LANTERN_PROOF_BYTES_V1)
        .expect("fixed ILB1 length fits u32")
        .to_be_bytes();
    let response_wire_bytes = u64::try_from(BLIND_ISSUANCE_RESPONSE_BYTES_V1)
        .expect("fixed ILR1 length fits u64")
        .to_be_bytes();
    let p1_purpose = [BLIND_ISSUANCE_REQUEST_PROOF_PURPOSE_TAG_V1];
    let issuer_profile_digest = bootle_lantern_issuer_profile_digest_v1();
    let issuance_store_max_record_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_MAX_RECORD_BYTES_V1.to_be_bytes();
    let issuance_store_hard_max_records =
        u64::try_from(BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_RECORDS_V1)
            .expect("issuance-store hard record cap fits u64")
            .to_be_bytes();
    let issuance_store_hard_max_total_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_TOTAL_BYTES_V1.to_be_bytes();
    let issuance_store_default_max_records =
        u64::try_from(BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_RECORDS_V1)
            .expect("issuance-store default record cap fits u64")
            .to_be_bytes();
    let issuance_store_default_max_total_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_TOTAL_BYTES_V1.to_be_bytes();
    let issuance_store_default_retention_blocks =
        BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_RETENTION_BLOCKS_V1.to_be_bytes();

    digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            BOOTLE_LANTERN_PROTOCOL_LABEL_V1,
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            SOURCE_PROFILE_V1,
            BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
            BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
            BOOTLE_LANTERN_FALCON512_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
            &falcon_keygen_candidates,
            &falcon_keygen_parity_attempts,
            &falcon_preimage_coefficient_proposals,
            &falcon_preimage_total_proposals,
            BOOTLE_CREDENTIAL_RANDOMNESS_PROFILE_DESCRIPTOR_V1,
            &credential_randomness_polynomials,
            &credential_randomness_norm,
            &credential_vector_attempts,
            &credential_coefficient_proposals,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
            &scope_acceptance_limit,
            &scope_coefficient_attempts,
            BOOTLE_LANTERN_BLIND_ISSUANCE_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
            &issuance_store_max_record_bytes,
            &issuance_store_hard_max_records,
            &issuance_store_hard_max_total_bytes,
            &issuance_store_default_max_records,
            &issuance_store_default_max_total_bytes,
            &issuance_store_default_retention_blocks,
            &issuer_profile_digest,
            &issuer_keygen_candidates,
            &authorization_id_attempts,
            &authorization_lifetime,
            &issuer_preimage_attempts,
            &BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1,
            &p1_purpose,
            &authorization_wire_bytes,
            &BLIND_ISSUANCE_REQUEST_MAGIC_V1,
            &request_version,
            &request_purpose,
            &request_header_bytes,
            &request_wire_bytes,
            &request_target_polynomials,
            &request_ring_degree,
            &request_proof_bytes,
            &response_wire_bytes,
            PUBLIC_PARAMETER_SEED_DOMAIN_V1,
            public_parameter_seed,
            &ring_degree,
            &application_modulus,
            &proof_modulus,
            &decomposition_bits,
            &compression_gamma,
            &compression_modulus,
            &relation_rows,
            &relation_columns,
            &quotient_bound,
            &randomness_norm_bound,
            &signature_norm_bound,
            &response_norm_bound,
            &challenge_omega,
            &challenge_set_bits,
            &challenge_norm_power,
            &challenge_norm_root_degree,
            &challenge_eta,
            sampling_profile_digest,
            &challenge_candidate_attempts,
        ],
    )
}

fn compiled_bootle_lantern_profile_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1;
    if !BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1 {
        return Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id });
    }
    compiled_bootle_lantern_profile_material_v1()
}

fn compiled_bootle_lantern_profile_material_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1;
    if APPLICATION_RING_DEGREE_V1 != BOOTLE_LANTERN_MODEL_RING_DEGREE_V1
        || BOOTLE_LANTERN_APPLICATION_MODULUS_V1 != BOOTLE_LANTERN_MODEL_APPLICATION_MODULUS_V1
        || BOOTLE_LANTERN_MODEL_ATTRIBUTE_COUNT_V1 != 8
        || APPLICATION_ROWS_V1 != 8
        || APPLICATION_WITNESS_POLYNOMIALS_V1 != 48
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let proof_bytes = u64::try_from(BOOTLE_LANTERN_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let proof_bytes = proof_bytes.to_be_bytes();
    let proof_version = [PROOF_VERSION_V1];
    let decomposition_bits = [DECOMPOSITION_BITS_V1];
    let compression_gamma = COMPRESSION_GAMMA_V1.to_be_bytes();
    let compression_modulus = COMPRESSION_MODULUS_V1.to_be_bytes();
    let scope_acceptance_limit = BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1.to_be_bytes();
    let scope_coefficient_attempts = BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1.to_be_bytes();
    let credential_randomness_polynomials = u64::try_from(CREDENTIAL_RANDOMNESS_POLYNOMIALS_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let credential_randomness_norm = CREDENTIAL_RANDOMNESS_NORM_SQUARED_BOUND_V1.to_be_bytes();
    let credential_vector_attempts = MAX_CREDENTIAL_RANDOMNESS_VECTOR_ATTEMPTS_V1.to_be_bytes();
    let credential_coefficient_proposals =
        MAX_CREDENTIAL_RANDOMNESS_COEFFICIENT_PROPOSALS_V1.to_be_bytes();
    let falcon_keygen_candidates =
        BOOTLE_LANTERN_FALCON512_DEFAULT_KEYGEN_CANDIDATES_V1.to_be_bytes();
    let falcon_keygen_parity_attempts =
        BOOTLE_LANTERN_FALCON512_KEYGEN_PARITY_ATTEMPTS_V1.to_be_bytes();
    let falcon_preimage_coefficient_proposals =
        BOOTLE_LANTERN_FALCON512_PREIMAGE_PROPOSALS_PER_COEFFICIENT_V1.to_be_bytes();
    let falcon_preimage_total_proposals =
        BOOTLE_LANTERN_FALCON512_PREIMAGE_TOTAL_PROPOSALS_V1.to_be_bytes();
    let issuer_keygen_candidates = MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1.to_be_bytes();
    let authorization_id_attempts = MAX_BOOTLE_LANTERN_AUTHORIZATION_ID_ATTEMPTS_V1.to_be_bytes();
    let authorization_lifetime = MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1.to_be_bytes();
    let issuer_preimage_attempts = MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1.to_be_bytes();
    let authorization_wire_bytes = u64::try_from(BLIND_ISSUANCE_AUTHORIZATION_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let request_wire_bytes = u64::try_from(BLIND_ISSUANCE_REQUEST_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let request_header_bytes = u64::try_from(BLIND_ISSUANCE_REQUEST_HEADER_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let request_version = [BLIND_ISSUANCE_REQUEST_VERSION_V1];
    let request_purpose = [BLIND_ISSUANCE_REQUEST_PURPOSE_TAG_V1];
    let request_target_polynomials = BLIND_ISSUANCE_REQUEST_TARGET_POLYNOMIALS_V1.to_be_bytes();
    let request_ring_degree = BLIND_ISSUANCE_REQUEST_RING_DEGREE_V1.to_be_bytes();
    let request_proof_bytes = u32::try_from(BOOTLE_LANTERN_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let response_wire_bytes = u64::try_from(BLIND_ISSUANCE_RESPONSE_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let p1_purpose = [BLIND_ISSUANCE_REQUEST_PROOF_PURPOSE_TAG_V1];
    let issuer_profile_digest = bootle_lantern_issuer_profile_digest_v1();
    let issuance_store_max_record_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_MAX_RECORD_BYTES_V1.to_be_bytes();
    let issuance_store_hard_max_records =
        u64::try_from(BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_RECORDS_V1)
            .map_err(
                |_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id },
            )?
            .to_be_bytes();
    let issuance_store_hard_max_total_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_TOTAL_BYTES_V1.to_be_bytes();
    let issuance_store_default_max_records =
        u64::try_from(BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_RECORDS_V1)
            .map_err(
                |_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id },
            )?
            .to_be_bytes();
    let issuance_store_default_max_total_bytes =
        BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_TOTAL_BYTES_V1.to_be_bytes();
    let issuance_store_default_retention_blocks =
        BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_RETENTION_BLOCKS_V1.to_be_bytes();
    if issuer_profile_digest == [0; 32] {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let public_parameter_seed = public_parameter_seed_v1();
    let sampling_profile_digest = bootle_sampling_profile_digest_v1();
    if sampling_profile_digest == [0; 32] {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            BOOTLE_LANTERN_PROTOCOL_LABEL_V1,
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            SOURCE_PROFILE_V1,
            BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
            BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
            BOOTLE_LANTERN_FALCON512_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
            &issuance_store_max_record_bytes,
            &issuance_store_hard_max_records,
            &issuance_store_hard_max_total_bytes,
            &issuance_store_default_max_records,
            &issuance_store_default_max_total_bytes,
            &issuance_store_default_retention_blocks,
            &issuer_profile_digest,
            BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
            &BLIND_ISSUANCE_REQUEST_MAGIC_V1,
            &request_version,
            &request_purpose,
            &request_header_bytes,
            &request_wire_bytes,
            &request_target_polynomials,
            &request_ring_degree,
            &request_proof_bytes,
            PUBLIC_PARAMETER_SEED_DOMAIN_V1,
            &public_parameter_seed,
        ],
    );
    let parameter_digest =
        bootle_lantern_parameter_digest_v1(&public_parameter_seed, &sampling_profile_digest);
    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaBootleLanternAnoncredStatementV1>().map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let issuer_policy_schema_digest = canonical_schema_digest_v1::<BootleLanternIssuerPolicyV1>()
        .map_err(|source| {
        CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
            protocol_id,
            source,
        }
    })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            BOOTLE_LANTERN_PROTOCOL_LABEL_V1,
            BOOTLE_LANTERN_IMPLEMENTATION_PROVENANCE_V1,
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            SOURCE_PROFILE_V1,
            BOOTLE_LANTERN_PROOF_WIRE_LABEL_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
            BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
            BOOTLE_LANTERN_COMPRESSION_SCHEMA_V1,
            &decomposition_bits,
            &compression_gamma,
            &compression_modulus,
            &PROOF_MAGIC_V1,
            &proof_version,
            &proof_bytes,
            PUBLIC_PARAMETER_SEED_DOMAIN_V1,
            &public_parameter_seed,
            BOOTLE_LANTERN_RELATION_SCHEMA_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
            &scope_acceptance_limit,
            &scope_coefficient_attempts,
            BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
            &issuance_store_max_record_bytes,
            &issuance_store_hard_max_records,
            &issuance_store_hard_max_total_bytes,
            &issuance_store_default_max_records,
            &issuance_store_default_max_total_bytes,
            &issuance_store_default_retention_blocks,
            &issuer_profile_digest,
            BOOTLE_SAMPLING_PROFILE_DESCRIPTOR_V1,
            &sampling_profile_digest,
            &BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1,
            &p1_purpose,
            &authorization_wire_bytes,
            &BLIND_ISSUANCE_REQUEST_MAGIC_V1,
            &request_version,
            &request_purpose,
            &request_header_bytes,
            &request_wire_bytes,
            &request_target_polynomials,
            &request_ring_degree,
            &request_proof_bytes,
            &response_wire_bytes,
            &issuer_policy_schema_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            BOOTLE_LANTERN_PROTOCOL_LABEL_V1,
            BOOTLE_LANTERN_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:lantern-lnp22-module-linear-norm",
            b"engine:native-lantern-lnp22-falcon512-one-shot-blind-issuance",
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            SOURCE_PROFILE_V1,
            BOOTLE_LANTERN_PROOF_WIRE_LABEL_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
            BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
            BOOTLE_LANTERN_FALCON512_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
            &falcon_keygen_candidates,
            &falcon_keygen_parity_attempts,
            &falcon_preimage_coefficient_proposals,
            &falcon_preimage_total_proposals,
            BOOTLE_LANTERN_RELATION_SCHEMA_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1,
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
            &scope_acceptance_limit,
            &scope_coefficient_attempts,
            BOOTLE_LANTERN_BLIND_ISSUANCE_SCHEMA_V1,
            BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1,
            BOOTLE_LANTERN_COMPRESSION_SCHEMA_V1,
            BOOTLE_LANTERN_NATIVE_PRODUCER_SCHEMA_V1,
            BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
            BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
            &issuance_store_max_record_bytes,
            &issuance_store_hard_max_records,
            &issuance_store_hard_max_total_bytes,
            &issuance_store_default_max_records,
            &issuance_store_default_max_total_bytes,
            &issuance_store_default_retention_blocks,
            &issuer_profile_digest,
            &issuer_keygen_candidates,
            &authorization_id_attempts,
            &authorization_lifetime,
            &issuer_preimage_attempts,
            BOOTLE_SAMPLING_PROFILE_DESCRIPTOR_V1,
            &sampling_profile_digest,
            BOOTLE_CREDENTIAL_RANDOMNESS_PROFILE_DESCRIPTOR_V1,
            &credential_randomness_polynomials,
            &credential_randomness_norm,
            &credential_vector_attempts,
            &credential_coefficient_proposals,
            &BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1,
            &p1_purpose,
            &authorization_wire_bytes,
            &BLIND_ISSUANCE_REQUEST_MAGIC_V1,
            &request_version,
            &request_purpose,
            &request_header_bytes,
            &request_wire_bytes,
            &request_target_polynomials,
            &request_ring_degree,
            &request_proof_bytes,
            &response_wire_bytes,
            &decomposition_bits,
            &compression_gamma,
            &compression_modulus,
            PUBLIC_PARAMETER_SEED_DOMAIN_V1,
            &public_parameter_seed,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &issuer_policy_schema_digest,
            &statement_schema_digest,
            &proof_bytes,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm,
        engine_id: PrivacyEngineIdV1::NativeLanternLnp22,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaBootleLanternAnoncredV1,
    })
}

fn compiled_jindo_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0;
    let max_polynomial_count =
        u32::try_from(JINDO_MAX_BATCH_SIZE_V1).expect("fixed Jindo batch size fits u32");
    let max_polynomial_count_bytes = max_polynomial_count.to_be_bytes();
    let proof_bytes = u64::try_from(JINDO_NATIVE_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    if proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let crs_digest = jindo_crs_digest_v1();
    if JINDO_MAX_BATCH_SIZE_V1 == 0 || JINDO_NATIVE_PROOF_BYTES_V1 == 0 || crs_digest == [0; 32] {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_PARAMETER_SET_LABEL_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SOURCE_PROVENANCE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_PARAMETER_SET_LABEL_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SOURCE_PROVENANCE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaJindoPolynomialCommitmentStatementV1>().map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_IMPLEMENTATION_PROVENANCE_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SOURCE_PROVENANCE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            JINDO_PROOF_WIRE_LABEL_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:jindo-polynomial-commitment",
            b"engine:native-jindo",
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SOURCE_PROVENANCE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            JINDO_PROOF_WIRE_LABEL_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::JindoPolynomialCommitment,
        engine_id: PrivacyEngineIdV1::NativeJindo,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count,
            },
        ),
    })
}

fn compiled_verange_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    let bits_32 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    let bits_64 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits64).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    if bits_32.parameter_digest() != bits_64.parameter_digest() {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        });
    }

    let bits_32_descriptor = bits_32.descriptor();
    let bits_64_descriptor = bits_64.descriptor();
    if bits_32_descriptor.max_single_proof_bytes > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1
        || bits_32_descriptor.max_batch_proof_bytes > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1
        || bits_64_descriptor.max_single_proof_bytes > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1
        || bits_64_descriptor.max_batch_proof_bytes > TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        });
    }
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_PARAMETER_SET_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
        ],
    );
    let proof_version = [VERANGE_TYPE1_PROOF_VERSION_V1];
    let bit_length_32 = bits_32_descriptor.bit_length.to_be_bytes();
    let rows_32 = bits_32_descriptor.rows.to_be_bytes();
    let columns_32 = bits_32_descriptor.columns.to_be_bytes();
    let bit_length_64 = bits_64_descriptor.bit_length.to_be_bytes();
    let rows_64 = bits_64_descriptor.rows.to_be_bytes();
    let columns_64 = bits_64_descriptor.columns.to_be_bytes();
    let max_batch = bits_64_descriptor.max_batch_commitments.to_be_bytes();
    let max_batch_proof = bits_64_descriptor.max_batch_proof_bytes.to_be_bytes();
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
            VERANGE_TYPE1_SUITE_V1,
            &proof_version,
            VERANGE_PROOF_WIRE_LABEL_V1,
            &bit_length_32,
            &rows_32,
            &columns_32,
            &bits_32_descriptor.generator_digest,
            &bit_length_64,
            &rows_64,
            &columns_64,
            &bits_64_descriptor.generator_digest,
            &max_batch,
            &max_batch_proof,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<VeRangeTransparentRangeStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                source,
            }
        })?;
    let parameter_digest = bits_32.parameter_digest();
    let global_commitment_cap = TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1.to_be_bytes();
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:iroha-verange-p256",
            b"engine:native-verange-p256",
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            CURVE_PROVER_RANDOMNESS_POLICY_V1,
            &global_commitment_cap,
        ],
    );
    let max_aggregation_count =
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1.min(TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1);

    Ok(CompiledPrivacyProfileV1 {
        protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        proof_system_id: PrivacyProofSystemIdV1::IrohaVeRangeP256,
        engine_id: PrivacyEngineIdV1::NativeVeRangeP256,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count,
            },
        ),
    })
}

fn digest_fields_v1(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, domain);
    hash.update(usize_to_u64_v1(fields.len()).to_be_bytes());
    for field in fields {
        append_digest_field_v1(&mut hash, field);
    }
    hash.finalize().into()
}

fn append_digest_field_v1(hash: &mut Sha256, field: &[u8]) {
    hash.update(usize_to_u64_v1(field.len()).to_be_bytes());
    hash.update(field);
}

/// Hash the complete structural schema of `T` in a platform-independent order.
///
/// The `iroha_schema` map is keyed internally by Rust [`core::any::TypeId`],
/// whose ordering is not a wire contract. This function replaces every such
/// reference with its declared stable string identifier, collapses only
/// representation aliases with identical canonical metadata, rejects
/// conflicting reuse of an identifier, sorts top-level entries by that stable
/// identifier, and preserves field/variant order where order is part of the
/// representation. Consequently, adding, deleting, reordering, or retyping a
/// statement field changes the governed digest without relying on a
/// hand-maintained schema string.
///
/// # Errors
///
/// Returns a typed error if the generated schema has duplicate stable type
/// identifiers or contains a reference to a type omitted from its own map.
pub fn canonical_schema_digest_v1<T: IntoSchema>() -> Result<[u8; 32], CanonicalSchemaDigestErrorV1>
{
    let schema = T::schema();
    let mut type_names = BTreeMap::new();
    for (rust_type_id, entry) in schema.iter() {
        type_names.insert(*rust_type_id, entry.type_id.clone());
    }

    // `iroha_schema` deliberately gives representation aliases such as
    // `String` and `Box<str>` the same stable wire identifier. Rust `TypeId`
    // still makes them separate map entries. Collapse aliases only after their
    // complete canonical metadata agrees; a reused identifier with a different
    // name or representation is an ambiguous consensus schema and must fail.
    let mut entries = BTreeMap::<String, CanonicalSchemaEntryV1>::new();
    for (_, entry) in schema.iter() {
        let canonical = CanonicalSchemaEntryV1 {
            type_name: entry.type_name.clone(),
            metadata: canonical_schema_metadata_v1(entry, &type_names)?,
        };
        if let Some(existing) = entries.get(&entry.type_id) {
            if existing != &canonical {
                return Err(CanonicalSchemaDigestErrorV1::ConflictingStableTypeId);
            }
        } else {
            entries.insert(entry.type_id.clone(), canonical);
        }
    }

    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, CANONICAL_SCHEMA_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, T::id().as_bytes());
    append_digest_count_v1(&mut hash, entries.len());
    for (stable_id, entry) in entries {
        append_digest_field_v1(&mut hash, stable_id.as_bytes());
        append_digest_field_v1(&mut hash, entry.type_name.as_bytes());
        append_digest_field_v1(&mut hash, &entry.metadata);
    }
    Ok(hash.finalize().into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalSchemaEntryV1 {
    type_name: String,
    metadata: Vec<u8>,
}

fn canonical_schema_metadata_v1(
    entry: &MetaMapEntry,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<Vec<u8>, CanonicalSchemaDigestErrorV1> {
    let mut bytes = Vec::new();
    match &entry.metadata {
        Metadata::Struct(fields) => {
            append_schema_field_v1(&mut bytes, b"struct");
            append_schema_count_v1(&mut bytes, fields.declarations.len());
            for field in &fields.declarations {
                append_schema_field_v1(&mut bytes, field.name.as_bytes());
                append_schema_type_reference_v1(&mut bytes, field.ty, type_names)?;
            }
        }
        Metadata::Tuple(fields) => {
            append_schema_field_v1(&mut bytes, b"tuple");
            append_schema_count_v1(&mut bytes, fields.types.len());
            for field_type in &fields.types {
                append_schema_type_reference_v1(&mut bytes, *field_type, type_names)?;
            }
        }
        Metadata::Enum(enum_meta) => {
            append_schema_field_v1(&mut bytes, b"enum");
            append_schema_count_v1(&mut bytes, enum_meta.variants.len());
            for variant in &enum_meta.variants {
                append_schema_field_v1(&mut bytes, variant.tag.as_bytes());
                append_schema_field_v1(&mut bytes, &variant.discriminant.to_be_bytes());
                match variant.ty {
                    Some(variant_type) => {
                        append_schema_field_v1(&mut bytes, b"some");
                        append_schema_type_reference_v1(&mut bytes, variant_type, type_names)?;
                    }
                    None => append_schema_field_v1(&mut bytes, b"none"),
                }
            }
        }
        Metadata::Int(mode) => {
            append_schema_field_v1(&mut bytes, b"int");
            append_schema_field_v1(
                &mut bytes,
                match mode {
                    IntMode::FixedWidth => b"fixed-width",
                    IntMode::Compact => b"compact",
                },
            );
        }
        Metadata::Float(mode) => {
            append_schema_field_v1(&mut bytes, b"float");
            append_schema_field_v1(
                &mut bytes,
                match mode {
                    FloatMode::Binary32 => b"binary32",
                    FloatMode::Binary64 => b"binary64",
                },
            );
        }
        Metadata::String => append_schema_field_v1(&mut bytes, b"string"),
        Metadata::Bool => append_schema_field_v1(&mut bytes, b"bool"),
        Metadata::FixedPoint(fixed) => {
            append_schema_field_v1(&mut bytes, b"fixed-point");
            append_schema_type_reference_v1(&mut bytes, fixed.base, type_names)?;
            append_schema_field_v1(&mut bytes, &fixed.decimal_places.to_be_bytes());
        }
        Metadata::Array(array) => {
            append_schema_field_v1(&mut bytes, b"array");
            append_schema_type_reference_v1(&mut bytes, array.ty, type_names)?;
            append_schema_field_v1(&mut bytes, &array.len.to_be_bytes());
        }
        Metadata::Vec(vector) => {
            append_schema_field_v1(&mut bytes, b"vec");
            append_schema_type_reference_v1(&mut bytes, vector.ty, type_names)?;
        }
        Metadata::Map(map) => {
            append_schema_field_v1(&mut bytes, b"map");
            append_schema_type_reference_v1(&mut bytes, map.key, type_names)?;
            append_schema_type_reference_v1(&mut bytes, map.value, type_names)?;
        }
        Metadata::Option(option_type) => {
            append_schema_field_v1(&mut bytes, b"option");
            append_schema_type_reference_v1(&mut bytes, *option_type, type_names)?;
        }
        Metadata::Result(result) => {
            append_schema_field_v1(&mut bytes, b"result");
            append_schema_type_reference_v1(&mut bytes, result.ok, type_names)?;
            append_schema_type_reference_v1(&mut bytes, result.err, type_names)?;
        }
        Metadata::Bitmap(bitmap) => {
            append_schema_field_v1(&mut bytes, b"bitmap");
            append_schema_type_reference_v1(&mut bytes, bitmap.repr, type_names)?;
            append_schema_count_v1(&mut bytes, bitmap.masks.len());
            for mask in &bitmap.masks {
                append_schema_field_v1(&mut bytes, mask.name.as_bytes());
                append_schema_field_v1(&mut bytes, &mask.mask.to_be_bytes());
            }
        }
    }
    Ok(bytes)
}

fn append_schema_type_reference_v1(
    bytes: &mut Vec<u8>,
    rust_type_id: core::any::TypeId,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<(), CanonicalSchemaDigestErrorV1> {
    let stable_id = type_names
        .get(&rust_type_id)
        .ok_or(CanonicalSchemaDigestErrorV1::MissingTypeReference)?;
    append_schema_field_v1(bytes, stable_id.as_bytes());
    Ok(())
}

fn append_schema_field_v1(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&usize_to_u64_v1(field.len()).to_be_bytes());
    bytes.extend_from_slice(field);
}

fn append_schema_count_v1(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(&usize_to_u64_v1(count).to_be_bytes());
}

fn append_digest_count_v1(hash: &mut Sha256, count: usize) {
    hash.update(usize_to_u64_v1(count).to_be_bytes());
}

#[cfg(target_pointer_width = "64")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u32::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(target_pointer_width = "16")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u16::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
compile_error!("privacy profile digest framing supports 16-, 32-, and 64-bit pointer widths");

/// Invalid structural schema emitted by an [`IntoSchema`] implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CanonicalSchemaDigestErrorV1 {
    /// Two distinct Rust types claimed one identifier for different wire shapes.
    #[error("privacy statement schema reuses a stable type identifier for conflicting shapes")]
    ConflictingStableTypeId,
    /// Metadata referenced a type absent from the generated schema map.
    #[error("privacy statement schema contains an unresolved type reference")]
    MissingTypeReference,
}

/// Failure constructing a locally compiled privacy profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileErrorV1 {
    /// The protocol is not exposed to governance in this binary.
    #[error("native privacy engine for {protocol_id:?} is not governance-available")]
    EngineUnavailable {
        /// Protocol whose verifier is absent or whose release gates are closed.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Deterministic transparent parameter initialization failed.
    #[error("compiled privacy profile initialization failed for {protocol_id:?}")]
    ProfileInitializationFailed {
        /// Protocol whose fixed parameters failed to initialize.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// The compiled public-statement schema is ambiguous or internally broken.
    #[error("compiled privacy statement schema is invalid for {protocol_id:?}: {source}")]
    StatementSchemaInvalid {
        /// Protocol whose typed schema failed canonicalization.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact canonicalization failure.
        source: CanonicalSchemaDigestErrorV1,
    },
}

/// Failure matching governance material to compiled consensus code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileValidationErrorV1 {
    /// No executable compiled profile could be obtained.
    #[error(transparent)]
    Profile(CompiledPrivacyProfileErrorV1),
    /// Protocol identity differs from the selected compiled profile.
    #[error("privacy activation protocol differs from compiled profile")]
    ProtocolMismatch,
    /// Proof-system identity differs.
    #[error("privacy activation proof system differs from compiled profile")]
    ProofSystemMismatch,
    /// Engine identity differs.
    #[error("privacy activation engine differs from compiled profile")]
    EngineMismatch,
    /// Parameter-set identity differs.
    #[error("privacy activation parameter id differs from compiled profile")]
    ParameterIdMismatch,
    /// Parameter digest differs.
    #[error("privacy activation parameter digest differs from compiled profile")]
    ParameterDigestMismatch,
    /// Verifier digest differs.
    #[error("privacy activation verifier digest differs from compiled profile")]
    VerifierDigestMismatch,
    /// Public-statement schema digest differs.
    #[error("privacy activation statement schema differs from compiled profile")]
    StatementSchemaDigestMismatch,
    /// Engine manifest digest differs.
    #[error("privacy activation engine manifest differs from compiled profile")]
    EngineManifestDigestMismatch,
    /// Protocol-specific limits are invalid, target another protocol, or exceed
    /// the compiled hard ceilings.
    #[error("privacy activation protocol limits are outside the compiled profile ceilings")]
    ProtocolLimitsMismatch,
    /// The first-release assurance tag differs.
    #[error("privacy activation assurance differs from compiled testnet profile")]
    AssuranceMismatch,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        AnonymousPgcActivationLimitsV1, PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        PrivacyProposedLifecycleV1,
    };
    use iroha_schema::{Declaration, MetaMap, NamedFieldsMeta, TypeId};

    use super::*;

    struct SchemaOrderAb;

    impl TypeId for SchemaOrderAb {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderAb {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaOrderBa;

    impl TypeId for SchemaOrderBa {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderBa {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                ],
            }));
        }
    }

    struct SchemaRetyped;

    impl TypeId for SchemaRetyped {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaRetyped {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaEquivalentAliases;

    impl TypeId for SchemaEquivalentAliases {
        fn id() -> String {
            "privacy-test::EquivalentAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaEquivalentAliases {
        fn type_name() -> String {
            "EquivalentAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            String::update_schema_map(map);
            Box::<str>::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "owned".to_owned(),
                        ty: core::any::TypeId::of::<String>(),
                    },
                    Declaration {
                        name: "boxed".to_owned(),
                        ty: core::any::TypeId::of::<Box<str>>(),
                    },
                ],
            }));
        }
    }

    struct SchemaConflictLeft;

    impl TypeId for SchemaConflictLeft {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictLeft {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Int(IntMode::FixedWidth));
        }
    }

    struct SchemaConflictRight;

    impl TypeId for SchemaConflictRight {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictRight {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Bool);
        }
    }

    struct SchemaConflictingAliases;

    impl TypeId for SchemaConflictingAliases {
        fn id() -> String {
            "privacy-test::ConflictingAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictingAliases {
        fn type_name() -> String {
            "ConflictingAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            SchemaConflictLeft::update_schema_map(map);
            SchemaConflictRight::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "left".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictLeft>(),
                    },
                    Declaration {
                        name: "right".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictRight>(),
                    },
                ],
            }));
        }
    }

    fn verange_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("fixed VeRange parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[cfg(feature = "zk-stark")]
    fn zk_ace_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("fixed ZK-ACE profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn pgc_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("fixed Anonymous-PGC parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn jindo_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
            .expect("fixed Jindo parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn vega_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("fixed Vega profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn bootle_lantern_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_bootle_lantern_profile_material_v1()
            .expect("fixed Bootle/Lantern profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn orchard_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("fixed Orchard profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn fcmp_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("fixed FCMP++ profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn ivm_private_note_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
            .expect("fixed IVM private-note profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn pq_masp_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::PqMaspStarkV0)
            .expect("fixed PQ-MASP profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn zk_x509_activation() -> PrivacyProtocolActivationRecordV1 {
        zk_x509_release_candidate_profile_material_v1()
            .expect("release-pinned zk-X.509 candidate profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[test]
    fn semantic_parameter_labels_and_framed_note_profiles_cannot_drift() {
        assert_eq!(
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            b"goldilocks-sha256-proof-managed-note-stark+private-note-vm16x8-tree32-v1"
        );
        assert_eq!(
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            b"goldilocks-sha256-proof-managed-note-stark+pq-masp+mldsa65+mlkem768-v1"
        );
        #[cfg(feature = "zk-stark")]
        assert_eq!(
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            b"goldilocks-poseidon2-transparent-stark-v1"
        );
        for stale_geometry in [
            b"mask255".as_slice(),
            b"mask111".as_slice(),
            b"three-lane".as_slice(),
            b"blowup32".as_slice(),
        ] {
            assert!(
                !IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
            assert!(
                !PQ_MASP_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
            #[cfg(feature = "zk-stark")]
            assert!(
                !ZK_ACE_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
        }
        let shared_digest: [u8; 32] =
            Sha256::digest(PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1).into();
        assert_eq!(shared_digest, PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1);
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(
                IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
            ),
            IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1
        );
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1),
            PQ_MASP_STARK_PROFILE_DIGEST_V1
        );
        assert!(
            IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1
                < usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize"),
            "the independent private-note proof cap must remain below the governed global cap"
        );
        assert_eq!(
            PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1,
            usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                .expect("global proof cap fits usize"),
            "the complete PQ-MASP authorization wire consumes the governed global cap"
        );
    }

    #[test]
    fn pq_masp_profile_binds_the_exact_wallet_and_verified_effect_schemas() {
        assert_eq!(
            PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
            b"typed-output:recipient-id32+encapsulation-digest32+output-commitment32+ciphertext[PQE1+mlkem768-ciphertext1088+nonce24+xchacha20poly1305[PQN1+value-u128be+authorization-key-digest32+recipient-id32+nullifier-key-digest32+rho32+blinding32+memo-digest32]+tag16]|mlkem768-domain-kdf|aad:domain+asset-definition-id-u64be-length+norito+pool-id32+output-commitment32+recipient-id32+encapsulation-digest32"
        );
        assert_eq!(
            PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1,
            b"namespace:norito|bootstrap-digest:32|asset-definition-id:norito|current-root:32|current-epoch:u64|next-root:32|next-epoch:u64|transition:pq-masp{ordered-nullifiers[32]+ordered-output-commitments[32]+validator-derived-successor-frontier}|value-balance:none"
        );
        for stale_field in [
            b"value-u128le".as_slice(),
            b"rseed32".as_slice(),
            b"anchor-epoch".as_slice(),
            b"ordered-encrypted-outputs".as_slice(),
            b"expiry-height".as_slice(),
        ] {
            assert!(
                !PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1
                    .windows(stale_field.len())
                    .any(|window| window == stale_field)
                    && !PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1
                        .windows(stale_field.len())
                        .any(|window| window == stale_field),
                "stale PQ-MASP profile field survived: {}",
                String::from_utf8_lossy(stale_field)
            );
        }

        let exact = compiled_pq_masp_profile_v1().expect("compiled PQ-MASP profile");
        for changed in [
            compiled_pq_masp_profile_v1_with_schemas(
                b"substituted-wallet-schema",
                PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1,
            )
            .expect("structurally valid wallet-schema substitution"),
            compiled_pq_masp_profile_v1_with_schemas(
                PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
                b"substituted-verified-effect-schema",
            )
            .expect("structurally valid effect-schema substitution"),
        ] {
            assert_eq!(changed.parameter_id, exact.parameter_id);
            assert_eq!(changed.parameter_digest, exact.parameter_digest);
            assert_ne!(changed.verifier_digest, exact.verifier_digest);
            assert_eq!(
                changed.statement_schema_digest,
                exact.statement_schema_digest
            );
            assert_ne!(changed.engine_manifest_digest, exact.engine_manifest_digest);
        }
    }

    #[test]
    fn local_compiled_profile_catalog_is_exact12_and_contains_no_governance_state() {
        let catalog = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        assert_eq!(catalog.version, PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1);
        assert_eq!(catalog.protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert!(
            catalog
                .protocols
                .iter()
                .map(|row| row.protocol_id)
                .eq(PrivacyProtocolIdV1::ALL)
        );

        let json = norito::json::to_json(&catalog).expect("catalog JSON");
        for forbidden in [
            "committed_height",
            "consensus_policy",
            "activation",
            "lifecycle",
        ] {
            assert!(
                !json.contains(forbidden),
                "local catalog must not expose governance field {forbidden}"
            );
        }
    }

    #[test]
    fn compiled_profile_catalog_cache_returns_owned_isolated_clones() {
        let canonical = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        canonical.validate().expect("canonical compiled catalog");
        assert_eq!(canonical.protocols.len(), PrivacyProtocolIdV1::COUNT);
        let canonical_archive =
            norito::encode_canonical(&canonical).expect("canonical compiled catalog archive");

        let mut caller_owned = canonical;
        caller_owned.protocols.rotate_left(1);
        assert!(
            caller_owned.validate().is_err(),
            "mutating one returned clone must make only that caller's copy noncanonical"
        );

        let subsequent =
            compiled_privacy_profile_catalog_v1().expect("subsequent compiled profile catalog");
        subsequent
            .validate()
            .expect("the cached canonical catalog must remain valid");
        assert_eq!(subsequent.protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert!(
            subsequent
                .protocols
                .iter()
                .map(|row| row.protocol_id)
                .eq(PrivacyProtocolIdV1::ALL)
        );
        assert_eq!(
            norito::encode_canonical(&subsequent)
                .expect("subsequent canonical compiled catalog archive"),
            canonical_archive,
            "a caller mutation must not alias or modify the immutable cache"
        );
    }

    #[test]
    fn local_compiled_profile_catalog_archive_rejects_canonical_substitution() {
        use PrivacyCompiledProfileCatalogArchiveValidationStatusV1 as Status;

        let catalog = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        let archive = norito::encode_canonical(&catalog).expect("canonical catalog");
        assert_eq!(
            validate_local_privacy_compiled_profile_catalog_archive_v1(&archive),
            Status::Valid
        );

        let mut substituted = catalog;
        let profile = substituted
            .protocols
            .iter_mut()
            .find_map(|row| match &mut row.compiled_profile {
                PrivacyCompiledProfileResultV1::Available(profile) => Some(profile),
                PrivacyCompiledProfileResultV1::Unavailable(_) => None,
            })
            .expect("at least one compiled profile");
        let mut digest = *profile.parameter_digest.as_bytes();
        digest[0] ^= 0x80;
        profile.parameter_digest = PrivacyParameterDigestV1::new(digest);
        profile
            .validate()
            .expect("substituted profile remains structural");
        let substituted =
            norito::encode_canonical(&substituted).expect("canonical substituted catalog");
        assert_eq!(
            validate_privacy_compiled_profile_catalog_archive_v1(&substituted),
            Status::Valid,
            "the data-model validator establishes only canonical catalog structure"
        );
        assert_eq!(
            validate_local_privacy_compiled_profile_catalog_archive_v1(&substituted),
            Status::InvalidCatalog,
            "the local validator must reject a canonical profile substitution"
        );
    }

    #[test]
    fn only_governance_released_engines_have_compiled_profiles() {
        let available = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter(|protocol_id| compiled_privacy_profile_v1(*protocol_id).is_ok())
            .collect::<Vec<_>>();
        let mut expected = vec![
            #[cfg(feature = "zk-stark")]
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ];
        if require_activation_readiness_v1(zk_x509_activation_readiness_v1()).is_ok() {
            expected.push(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0);
        }
        assert!(
            zk_x509_release_candidate_profile_material_v1().is_ok(),
            "X.509 candidate material must derive independently of governance release"
        );
        assert_eq!(available, expected);
    }

    #[test]
    fn ivm_private_note_profile_binds_distinct_proof_and_wallet_randomness_policies() {
        let exact = compiled_ivm_private_note_profile_v1().expect("compiled IVM profile");
        assert_ne!(
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            CURVE_PROVER_RANDOMNESS_POLICY_V1
        );
        assert!(
            IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1
                .windows(CURVE_PROVER_RANDOMNESS_POLICY_V1.len())
                .any(|window| window == CURVE_PROVER_RANDOMNESS_POLICY_V1)
        );

        let mut changed_proof_policy = TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        changed_proof_policy[0] ^= 1;
        let mut changed_wallet_policy = CURVE_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        changed_wallet_policy[0] ^= 1;
        for changed in [
            compiled_ivm_private_note_profile_v1_with_randomness_policies(
                &changed_proof_policy,
                CURVE_PROVER_RANDOMNESS_POLICY_V1,
            )
            .expect("structurally valid proof-policy mutation"),
            compiled_ivm_private_note_profile_v1_with_randomness_policies(
                TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
                &changed_wallet_policy,
            )
            .expect("structurally valid wallet-policy mutation"),
        ] {
            assert_eq!(changed.parameter_id, exact.parameter_id);
            assert_ne!(changed.parameter_digest, exact.parameter_digest);
            assert_ne!(changed.verifier_digest, exact.verifier_digest);
            assert_eq!(
                changed.statement_schema_digest,
                exact.statement_schema_digest
            );
            assert_ne!(changed.engine_manifest_digest, exact.engine_manifest_digest);
        }
    }

    #[test]
    fn ivm_private_note_and_pq_masp_profiles_are_exact_bounded_and_mutation_closed() {
        let cases = [
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                ivm_private_note_activation(),
                PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
                    IvmPrivateNoteActivationLimitsV1 {
                        max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                        max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                    },
                ),
            ),
            (
                PrivacyProtocolIdV1::PqMaspStarkV0,
                pq_masp_activation(),
                PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(PqMaspActivationLimitsV1 {
                    max_input_count: PQ_MASP_MAX_INPUTS_V1,
                    max_output_count: PQ_MASP_MAX_OUTPUTS_V1,
                }),
            ),
        ];

        for (protocol_id, valid, expected_limits) in cases {
            let first = compiled_privacy_profile_v1(protocol_id).expect("compiled native profile");
            let second = compiled_privacy_profile_v1(protocol_id).expect("deterministic profile");
            assert_eq!(first, second);
            assert_eq!(
                first.proof_system_id,
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
            );
            assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
            assert_eq!(first.protocol_limits, expected_limits);
            for digest in [
                *first.parameter_id.as_bytes(),
                *first.parameter_digest.as_bytes(),
                *first.verifier_digest.as_bytes(),
                *first.statement_schema_digest.as_bytes(),
                *first.engine_manifest_digest.as_bytes(),
            ] {
                assert_ne!(digest, [0; 32]);
            }
            let expected_bindings = match protocol_id {
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => (
                    "b5db09ae42957802c502855459a102ba8e829bfb86a0356691455de0a08fbec0".to_owned(),
                    "a665cfcbea5576a1cf533997e575ebd49957ce320c483c019e784f8fc93457e1".to_owned(),
                    "5f2214526473a3b617e09c43dd9f48795f11d7f169bb645e76ce0693b0483abb".to_owned(),
                    "b30e388a3f3dbb6d2e93aa8c53a5df355238b763d6c3fcd766f7d0c3f0afca5f".to_owned(),
                    "99158955397f0aa94c2bae5285cb2e6f7602506366e6f583a6797ffaa77874d1".to_owned(),
                ),
                PrivacyProtocolIdV1::PqMaspStarkV0 => (
                    "10a8697291331061099a6c67eaeac3bc29f77aea951f2f2ad55ca29d0f816951".to_owned(),
                    "120ad9e6f616fdd05168a2dde5608654094a18b97bfc89ebedf86b7fbaf335b8".to_owned(),
                    "dc7c983c9b683ec2b4efc998408a59afd213272ac37bcee5720cf68a0f4516c9".to_owned(),
                    "4932c64b8f113632ba145e18ca5cc85496fbc96d103b19d712643348f3153727".to_owned(),
                    "e6cd364435e6ef1d85ef0a825b05cbf48a65ecf10e9f152d68935f84246c9601".to_owned(),
                ),
                _ => unreachable!("the test covers only IVM private note and PQ-MASP"),
            };
            assert_eq!(
                (
                    hex::encode(first.parameter_id.as_bytes()),
                    hex::encode(first.parameter_digest.as_bytes()),
                    hex::encode(first.verifier_digest.as_bytes()),
                    hex::encode(first.statement_schema_digest.as_bytes()),
                    hex::encode(first.engine_manifest_digest.as_bytes()),
                ),
                expected_bindings,
                "every consensus-critical {} binding is a pinned KAT",
                protocol_id.canonical_label(),
            );

            validate_compiled_privacy_activation_v1(&valid)
                .expect("exact compiled activation is accepted");
            let mutations: [(
                CompiledPrivacyProfileValidationErrorV1,
                fn(&mut PrivacyProtocolActivationRecordV1),
            ); 8] = [
                (
                    CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                    |record| record.proof_system_id = PrivacyProofSystemIdV1::Halo2IpaPasta,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                    |record| record.engine_id = PrivacyEngineIdV1::NativeHalo2Orchard,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                    |record| record.parameter_id.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                    |record| record.parameter_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                    |record| record.verifier_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                    |record| record.statement_schema_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                    |record| record.engine_manifest_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                    |record| match &mut record.protocol_limits {
                        PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(limits) => {
                            limits.max_input_count += 1;
                        }
                        PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(limits) => {
                            limits.max_output_count += 1;
                        }
                        _ => unreachable!("test covers only IVM private note and PQ-MASP"),
                    },
                ),
            ];
            for (expected, mutate) in mutations {
                let mut changed = valid;
                mutate(&mut changed);
                assert_eq!(
                    validate_compiled_privacy_activation_v1(&changed),
                    Err(expected)
                );
            }
        }
    }

    #[test]
    fn compiling_ivm_private_note_and_pq_masp_does_not_activate_their_lifecycles() {
        let snapshot = committed_privacy_capability_snapshot_v1(
            42,
            PrivacyConsensusPolicyV1::taira_default(),
            |_| None,
        )
        .expect("empty committed lifecycle state is valid");
        for protocol_id in [
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            let row = snapshot
                .protocols
                .iter()
                .find(|row| row.protocol_id == protocol_id)
                .expect("exact12 row");
            assert!(matches!(
                row.compiled_profile,
                PrivacyCompiledProfileResultV1::Available(_)
            ));
            assert_eq!(row.activation, None);
        }
    }

    #[test]
    fn fcmp_profile_is_deterministic_exact_bounded_and_mutation_closed() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("compiled FCMP++");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("compiled FCMP++");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeFcmpPlusPlus);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                max_input_count: FCMP_MAX_INPUTS_V1,
                max_output_count: FCMP_MAX_OUTPUTS_V1,
            })
        );
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(
                FCMP_MAX_INPUTS_NATIVE_V1,
                FCMP_MAX_TREE_LAYERS_V1,
                FCMP_MAX_OUTPUTS_NATIVE_V1,
            )
            .expect("maximum FCMP++ wire"),
            FCMP_MAX_PROOF_WIRE_BYTES_V1
        );
        assert!(
            FCMP_MAX_PROOF_WIRE_BYTES_V1
                <= usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize")
        );
        for digest in [
            fcmp_compiled_profile_digest_v1(),
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "8a24198f13ce0dbe0f4747874def956dc15ca98f9308c29ed678afddbe989a04".to_owned(),
                "92ee53970444330e37716b98a9eb1c04d8e52eb1ffe08103fb2745cc1abc9a89".to_owned(),
                "5e83f32ed7edf764e50fc8cebf5b4d8b75cb9e42a296965514b033d49dae4ac4".to_owned(),
                "c1577ce5a4a22e089a2fd7547f7fea32b7b35808967149d0e7f96a2ecb8c4ba7".to_owned(),
                "fb5e94756f9f234641b27899b7fd63bb48f3b5f92c24266d76e6d4de16231b27".to_owned(),
            ),
            "every consensus-critical FCMP++ binding is a pinned KAT",
        );
        let mut mutated_randomness_policy = CURVE_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_randomness_policy[0] ^= 1;
        let policy_mutation =
            compiled_fcmp_profile_v1_with_randomness_policy(&mutated_randomness_policy)
                .expect("structurally valid FCMP++ policy mutation");
        assert_eq!(policy_mutation.parameter_id, first.parameter_id);
        assert_ne!(policy_mutation.parameter_digest, first.parameter_digest);
        assert_ne!(policy_mutation.verifier_digest, first.verifier_digest);
        assert_eq!(
            policy_mutation.statement_schema_digest,
            first.statement_schema_digest
        );
        assert_ne!(
            policy_mutation.engine_manifest_digest,
            first.engine_manifest_digest
        );

        let valid = fcmp_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact FCMP++ activation");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::Halo2IpaPasta,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeHalo2Orchard,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(
                            FcmpActivationLimitsV1 {
                                max_input_count: FCMP_MAX_INPUTS_V1 + 1,
                                max_output_count: FCMP_MAX_OUTPUTS_V1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn bootle_lantern_profile_is_deterministic_complete_bounded_and_mutation_closed() {
        let first = compiled_bootle_lantern_profile_material_v1().expect("profile material");
        let second = compiled_bootle_lantern_profile_material_v1().expect("profile material");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeLanternLnp22);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaBootleLanternAnoncredV1
        );
        assert_eq!(APPLICATION_RING_DEGREE_V1, 64);
        assert_eq!(
            APPLICATION_RING_DEGREE_V1,
            BOOTLE_LANTERN_MODEL_RING_DEGREE_V1
        );
        assert_eq!(
            BOOTLE_LANTERN_APPLICATION_MODULUS_V1,
            BOOTLE_LANTERN_MODEL_APPLICATION_MODULUS_V1
        );
        assert_eq!(APPLICATION_ROWS_V1, 8);
        assert_eq!(APPLICATION_ROWS_V1, BOOTLE_LANTERN_MODEL_ATTRIBUTE_COUNT_V1);
        assert_eq!(APPLICATION_WITNESS_POLYNOMIALS_V1, 48);
        assert_eq!(
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            b"falcon512-ntru-r512-as-r64-rank8-interleaved"
        );
        for required in [
            &b"BLNS-specialization-no-main-construction-reduction"[..],
            &b"rust-fn-dsa-workspace-0.3-daf14859b5aa3f8d75c42966ba7de83e6eb59997"[..],
        ] {
            assert!(
                BOOTLE_LANTERN_IMPLEMENTATION_PROVENANCE_V1
                    .windows(required.len())
                    .any(|window| window == required),
                "implementation provenance omitted {}",
                String::from_utf8_lossy(required)
            );
        }
        for (descriptor, required) in [
            (
                BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
                &b"H_i[j]=h[8*j+i]"[..],
            ),
            (
                BOOTLE_LANTERN_RELATION_SCHEMA_V1,
                &b"A_r*r+A_tau*tau+A_m*m+scope-s1-H*s2=0"[..],
            ),
            (
                BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
                &b"excluded:action-index+transaction-intent-digest"[..],
            ),
            (
                BOOTLE_LANTERN_BLIND_ISSUANCE_SCHEMA_V1,
                &b"atomic-height-aware-Fresh-to-Processing-before-one-master64"[..],
            ),
            (
                BOOTLE_LANTERN_NATIVE_PRODUCER_SCHEMA_V1,
                &b"cached-completed-replay-does-not-touch-rng"[..],
            ),
            (
                BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1,
                &b"issuer-generated-one-shot-issuance-authorization-digest"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILA1:fixed320"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILR1:fixed3176"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILQ1:fixed71576"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1,
                &b"caller-cap-before-exact-length-before-allocation"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
                &b"authorization-state:Fresh-Processing-Completed-or-Failed"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
                &b"closed-purpose-enum:no-caller-selected-labels"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
                &b"canonical-process-lease+unix-nonblocking-exclusive-flock-held-for-lifetime"[..],
            ),
            (
                BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
                &b"H_i[j]=h[8*j+i]"[..],
            ),
            (
                BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
                &b"arbitrary-R512-target"[..],
            ),
            (
                BOOTLE_CREDENTIAL_RANDOMNESS_PROFILE_DESCRIPTOR_V1,
                &b"sign-cache:issuance-local-persistent"[..],
            ),
        ] {
            assert!(
                descriptor
                    .windows(required.len())
                    .any(|window| window == required),
                "compiled descriptor omitted {}",
                String::from_utf8_lossy(required)
            );
        }
        assert_eq!(
            BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1,
            BOOTLE_LANTERN_APPLICATION_MODULUS_V1 * 5
        );
        assert_eq!(BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1, 4_096);
        assert_eq!(CREDENTIAL_RANDOMNESS_POLYNOMIALS_V1, 16);
        assert_eq!(
            CREDENTIAL_RANDOMNESS_NORM_SQUARED_BOUND_V1,
            RANDOMNESS_NORM_SQUARED_BOUND_V1
        );
        assert_eq!(MAX_CREDENTIAL_RANDOMNESS_VECTOR_ATTEMPTS_V1, 64);
        assert_eq!(MAX_CREDENTIAL_RANDOMNESS_COEFFICIENT_PROPOSALS_V1, 256);
        assert_eq!(
            MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1,
            BOOTLE_LANTERN_FALCON512_DEFAULT_KEYGEN_CANDIDATES_V1
        );
        assert_eq!(MAX_BOOTLE_LANTERN_AUTHORIZATION_ID_ATTEMPTS_V1, 4);
        assert_eq!(MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1, 4_096);
        assert_eq!(MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1, 64);
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_MAX_RECORD_BYTES_V1, 3_310);
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_RECORDS_V1, 1_000_000);
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_TOTAL_BYTES_V1,
            3_310_000_000
        );
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_RECORDS_V1, 4_096);
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_TOTAL_BYTES_V1,
            13_557_760
        );
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_RETENTION_BLOCKS_V1,
            4_096
        );
        assert_eq!(BLIND_ISSUANCE_AUTHORIZATION_BYTES_V1, 320);
        assert_eq!(BLIND_ISSUANCE_REQUEST_BYTES_V1, 71_576);
        assert_eq!(BLIND_ISSUANCE_REQUEST_HEADER_BYTES_V1, 16);
        assert_eq!(BLIND_ISSUANCE_REQUEST_MAGIC_V1, *b"ILQ1");
        assert_eq!(BLIND_ISSUANCE_REQUEST_VERSION_V1, 1);
        assert_eq!(BLIND_ISSUANCE_REQUEST_PURPOSE_TAG_V1, 1);
        assert_eq!(BLIND_ISSUANCE_REQUEST_TARGET_POLYNOMIALS_V1, 8);
        assert_eq!(BLIND_ISSUANCE_REQUEST_RING_DEGREE_V1, 64);
        assert_eq!(BLIND_ISSUANCE_RESPONSE_BYTES_V1, 3_176);
        assert_eq!(BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1, *b"ILB1");
        assert_eq!(BLIND_ISSUANCE_REQUEST_PROOF_PURPOSE_TAG_V1, 1);
        assert_eq!(
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
            b"iroha.privacy.bootle-lantern.credential-scope-digest.v1"
        );
        assert_ne!(bootle_lantern_issuer_profile_digest_v1(), [0; 32]);
        assert_eq!(BOOTLE_LANTERN_PROOF_BYTES_V1, 70_344);
        assert!(
            u64::try_from(BOOTLE_LANTERN_PROOF_BYTES_V1).expect("proof size fits u64")
                <= u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
        );
        assert_ne!(public_parameter_seed_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "55bea016d0919cde8d24b54bb35eb01f7578a9a91189aececa34c7fc1b90e75c".to_owned(),
                "6a0b33463d71f6aec27ad330ae4424e3ed317a841dc1a0d79c5389905072ffc9".to_owned(),
                "7733ada1295556a13c3f626f270d1287324e28e987613d97e1e1605ff4d22ce8".to_owned(),
                "9c7c4f65128a4d924955b8b0fb6bfcc56ec34d14224ddfefebe32771c19a9e54".to_owned(),
                "e613fbbaf3e0470524a2924e72e5f8adc93c3950a26c5a4e9af8b7a74b88078b".to_owned(),
            ),
            "every consensus-critical Bootle/Lantern binding is a pinned KAT"
        );

        if !BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1 {
            return;
        }
        let valid = bootle_lantern_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id =
                        PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeFcmpPlusPlus,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                            JindoActivationLimitsV1 {
                                max_polynomial_count: 1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn bootle_lantern_complete_sampling_profile_is_parameter_bound_and_kat_pinned() {
        assert!(
            BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1
                .windows(b"max-rejected-uniform-draws-per-coefficient=4096".len())
                .any(|window| { window == b"max-rejected-uniform-draws-per-coefficient=4096" })
        );

        let public_parameter_seed = public_parameter_seed_v1();
        let sampling_profile_digest = bootle_sampling_profile_digest_v1();
        assert_eq!(
            hex::encode(sampling_profile_digest),
            "6e037c7342b327b75df5621f999506799174254ca7a7846d7549a6526f6ef897"
        );
        let governed =
            bootle_lantern_parameter_digest_v1(&public_parameter_seed, &sampling_profile_digest);
        assert_eq!(
            hex::encode(governed),
            "6a0b33463d71f6aec27ad330ae4424e3ed317a841dc1a0d79c5389905072ffc9"
        );
        for index in 0..sampling_profile_digest.len() {
            let mut mutated_sampling_profile_digest = sampling_profile_digest;
            mutated_sampling_profile_digest[index] ^= 1;
            assert_ne!(
                governed,
                bootle_lantern_parameter_digest_v1(
                    &public_parameter_seed,
                    &mutated_sampling_profile_digest
                ),
                "sampling-profile digest byte {index} was not parameter-bound"
            );
        }
    }

    #[test]
    fn orchard_profile_is_deterministic_complete_bounded_and_mutation_closed() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(first.proof_system_id, PrivacyProofSystemIdV1::Halo2IpaPasta);
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeHalo2Orchard);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1,
            })
        );
        assert_eq!(ORCHARD_ENGINE_MAX_ACTIONS_V1, 2);
        assert_eq!(ORCHARD_MODEL_MAX_ACTIONS_V1, 2);
        assert!(
            orchard_authorization_wire_size_v1(2).expect("wire size")
                <= usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize")
        );
        assert_ne!(orchard_empty_root_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        let mut mutated_source_policy = TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_source_policy[0] ^= 1;
        let mut mutated_bridge_policy = ORCHARD_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_bridge_policy[0] ^= 1;
        for (label, source_policy, bridge_policy) in [
            (
                "source",
                mutated_source_policy.as_slice(),
                ORCHARD_PROVER_RANDOMNESS_POLICY_V1,
            ),
            (
                "bridge",
                TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
                mutated_bridge_policy.as_slice(),
            ),
        ] {
            let policy_mutation =
                compiled_orchard_profile_v1_with_randomness_policies(source_policy, bridge_policy)
                    .expect("structurally valid Orchard policy mutation");
            assert_eq!(
                policy_mutation.parameter_id, first.parameter_id,
                "{label} policy changed the parameter family"
            );
            assert_ne!(
                policy_mutation.parameter_digest, first.parameter_digest,
                "{label} policy was not parameter-bound"
            );
            assert_ne!(
                policy_mutation.verifier_digest, first.verifier_digest,
                "{label} policy was not verifier-bound"
            );
            assert_eq!(
                policy_mutation.statement_schema_digest, first.statement_schema_digest,
                "{label} policy changed the statement schema"
            );
            assert_ne!(
                policy_mutation.engine_manifest_digest, first.engine_manifest_digest,
                "{label} policy was not engine-manifest-bound"
            );
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "8d5a2946c58314ac12d2968ffe9e8e0c672e3bbceefaaefad6a87420ea7dd212".to_owned(),
                "b27b73d59151415e21b158c75ed9371cccd795655b604e4a6b53db621660b66e".to_owned(),
                "c788016923d55e5455f3114735999f3c01f06aac8e7af2ce2bed4968b29800ea".to_owned(),
                "0412d379f8cbf01109d994bc74f148a13e38fc64350308597c047a0e6ec95fd9".to_owned(),
                "25f22d98c4f37d513361402fa5730caf214d097b624b2abd848dd932da39751e".to_owned(),
            ),
            "every consensus-critical Orchard profile binding is a pinned KAT"
        );

        let valid = orchard_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 7] = [
            |record| record.parameter_id.0[0] ^= 1,
            |record| record.parameter_digest.0[0] ^= 1,
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                record.proof_system_id = PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs
            },
            |record| record.engine_id = PrivacyEngineIdV1::NativeFcmpPlusPlus,
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }

    #[cfg(not(feature = "zk-stark"))]
    #[test]
    fn zk_ace_remains_fail_closed_without_a_sound_compiled_profile() {
        let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        assert_eq!(
            compiled_privacy_profile_v1(protocol_id),
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0
        );
        assert!(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1);
        assert_ne!(zk_ace_compiled_profile_digest_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "7f6efa99b249c5a95d2828338ffd533bd3e2e3cb8748f9bef984d34783cd727c".to_owned(),
                "eccf8e390650afa055dd617a18094f064eea06b1a9116fe9d6443d2f8ffb184f".to_owned(),
                "c6862c2f31dd4121b92af8fb272580101cc79344aea739a1b90f6cf8501b7509".to_owned(),
                "fc01374c09dc173e7c184f790fb959c495457ee8490eb3b18b48a802e5aa1d4e".to_owned(),
                "a94a0f8cfa1762a38921c47777c1c8ce22a82f0e9bb8ebf0857f51347ed73531".to_owned(),
            )
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_compiled_profile_rejects_every_binding_mismatch() {
        let valid = zk_ace_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::JindoPolynomialCommitment;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeJindo,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0;
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn zk_ams_profile_is_unavailable_until_every_mkhe_gate_closes() {
        let expected = CompiledPrivacyProfileErrorV1::EngineUnavailable {
            protocol_id: PrivacyProtocolIdV1::IrohaZkAmsV1,
        };
        assert_eq!(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1),
            Err(expected)
        );
        assert_eq!(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1),
            Err(expected),
            "the unavailable result must be deterministic"
        );

        let candidate = zk_ams_release_candidate_profile_material_v1()
            .expect("release-candidate profile material derives independently of activation");
        assert_eq!(candidate.protocol_id, PrivacyProtocolIdV1::IrohaZkAmsV1);
        for digest in [
            *candidate.parameter_id.as_bytes(),
            *candidate.parameter_digest.as_bytes(),
            *candidate.verifier_digest.as_bytes(),
            *candidate.statement_schema_digest.as_bytes(),
            *candidate.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }

        let candidate_activation = candidate.activation_record(
            PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
                proposed_at_height: 100,
                activate_at_height: 400,
            }),
        );
        assert_eq!(
            validate_compiled_privacy_activation_v1(&candidate_activation),
            Err(CompiledPrivacyProfileValidationErrorV1::Profile(expected)),
            "release-candidate material must never bypass the production readiness gate",
        );

        let readiness =
            iroha_zkp_halo2::vega::zk_ams_mkhe_readiness_v1().expect("candidate readiness derives");
        assert!(readiness.parameter_gate);
        assert!(readiness.noise_gate);
        assert!(!readiness.security_gate);
        assert!(!readiness.resource_gate);
        assert!(!readiness.wire_gate);
        assert!(!readiness.malicious_party_gate);
        assert!(!readiness.decryption_share_gate);
        assert!(!readiness.packing_gate);
        assert!(!readiness.phase23_gate);
        assert!(!readiness.release_kat_gate);
        assert!(!readiness.is_ready());
    }

    #[test]
    fn structural_schema_digest_detects_reordering_and_retyping() {
        let original = canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema");
        let reordered = canonical_schema_digest_v1::<SchemaOrderBa>().expect("schema");
        let retyped = canonical_schema_digest_v1::<SchemaRetyped>().expect("schema");
        assert_ne!(original, reordered);
        assert_ne!(original, retyped);
        assert_ne!(reordered, retyped);
        assert_eq!(
            original,
            canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema")
        );
    }

    #[test]
    fn structural_schema_digest_deduplicates_only_equivalent_aliases() {
        let equivalent =
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases");
        assert_ne!(equivalent, [0; 32]);
        assert_eq!(
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases"),
            equivalent
        );
        assert_eq!(
            canonical_schema_digest_v1::<SchemaConflictingAliases>(),
            Err(CanonicalSchemaDigestErrorV1::ConflictingStableTypeId)
        );
    }

    #[test]
    fn verange_profile_is_deterministic_and_uses_effective_global_cap() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                VeRangeActivationLimitsV1 {
                    max_aggregation_count: 8,
                }
            )
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "97e8be40e495bb6723db0ca73c04d2441ff166cf2163ddd2662c7e6a083f2c32".to_owned(),
                "3d79fe744741f956cb589f45774f922b849cf93833e6a9ebdedf1f815f1b7b44".to_owned(),
                "9b1a285d43ddc306b4d9ca6eac525b49b073f7d281ecf94299730613f683aa13".to_owned(),
                "32c038ab076bf2cab61bb15ffd07675e64b6849fce6e935252160b640d11b5c4".to_owned(),
                "5464e209f243f68189a84fad74e435aa78653d2fdd3458601787daf5479a45b0".to_owned(),
            )
        );
    }

    #[test]
    fn anonymous_pgc_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
        );
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 64,
                    max_recipient_count: 8,
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "58c1a93d39f23727ae8b5bbb661414f3dcadf2479575282cd7e3b9ebbb5589fc".to_owned(),
                "ca09d19ed5f3bb56ba7432a67b7ad14697c4874ab7870ea53441e4df0624bd7b".to_owned(),
                "aa352369f2a1fd0c9377414a2721728c35a95a4bc72497118e75c765edacd99e".to_owned(),
                "080aaf7d1f9d44c5dad6a5adc393034715fbf428d1dd1e5b59e33808c110aa96".to_owned(),
                "a74d8f690da89d50b9950e6d3496179f98bc6e60b71ec11e408c908aad73a81b".to_owned(),
            )
        );
    }

    #[test]
    fn jindo_profile_is_deterministic_complete_and_bounded() {
        let first =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        let second =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::JindoPolynomialCommitment
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeJindo);
        assert_eq!(JINDO_NATIVE_PROOF_BYTES_V1, 331_912);
        assert_ne!(jindo_crs_digest_v1(), [0; 32]);
        let provenance = core::str::from_utf8(JINDO_SOURCE_PROVENANCE_V1)
            .expect("Jindo source provenance is ASCII");
        assert!(provenance.contains("revision-2026-06-02"));
        assert!(provenance.contains("ringo-snark@805eab27"));
        let wire = core::str::from_utf8(JINDO_PROOF_WIRE_LABEL_V1)
            .expect("Jindo proof wire label is ASCII");
        for required in ["IJP2", "7-outer", "12-inner", "644-field", "no-IJP1"] {
            assert!(
                wire.contains(required),
                "Jindo wire descriptor lost {required}"
            );
        }
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: u32::try_from(JINDO_MAX_BATCH_SIZE_V1)
                        .expect("fixed Jindo batch size fits u32"),
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
                hex::encode(jindo_crs_digest_v1()),
            ),
            (
                "48bdc194dcd85c416db5b1c00e58dba42357098dfb807d060497d7495911692c".to_owned(),
                "56c9d07c283889a824768299b65dd69e2b6befbd123434be8571d21b32b0794b".to_owned(),
                "89fe6e1c19c8b4851bf33b66479fba2d747943442009679c8618158165fad76e".to_owned(),
                "7b87a8f64c9345e3ce13c2f4ce02a183e3806a8d2cea0faf7b6b0a00491aed28".to_owned(),
                "ae3bf287b0c3c0f8c3163db10a06f037f79e3a5967ed6a84eadb054cc809d95a".to_owned(),
                "424603d0ab5f57eed76aa365ec100cb4ac583e10dc801727363b6e188f5edd27".to_owned(),
            )
        );
    }

    #[test]
    fn vega_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeVega);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0
        );
        assert!(MAX_VEGA_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize);
        assert_ne!(vega_mdl_canonical_relation_digest_v1(), [0; 32]);
        assert_ne!(vega_mdl_compiled_profile_digest_v1(), [0; 32]);
        assert_eq!(
            vega_mdl_verifier_digest_v1().expect("canonical Vega-MC verifier digest"),
            VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "9fa2a07d17989e07bb7ff804bb408e95e127b80ab5e01258b77af9b00c82607d".to_owned(),
                "cf6bb53805e982444751db072c04d8b52dd9e14712cb90bbf23f68bbf2650c82".to_owned(),
                "6056ad21ff647212dcc81ff5508e5348400ca734a230073ac6367fa9c7b5ba3f".to_owned(),
                "f45032acceaf4b65e5afe114ca1f87fde477a73040e07c60a2c99e831f4cdc63".to_owned(),
                "c701b59a7083969770841a85a784608543c61e5849fed0670bfd97c2aa845009".to_owned(),
            )
        );
    }

    #[test]
    #[ignore = "operator-only KAT regeneration after an intentional compiled-profile change"]
    fn print_all_compiled_profile_tuples() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            let profile = if protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
                zk_x509_release_candidate_profile_material_v1()
            } else {
                compiled_privacy_profile_v1(protocol_id)
            }
            .unwrap_or_else(|error| {
                panic!(
                    "compiled profile for {}: {error}",
                    protocol_id.canonical_label()
                )
            });
            eprintln!(
                "{}={}|{}|{}|{}|{}",
                protocol_id.canonical_label(),
                hex::encode(profile.parameter_id.as_bytes()),
                hex::encode(profile.parameter_digest.as_bytes()),
                hex::encode(profile.verifier_digest.as_bytes()),
                hex::encode(profile.statement_schema_digest.as_bytes()),
                hex::encode(profile.engine_manifest_digest.as_bytes()),
            );
        }
    }

    #[test]
    fn vega_compiled_profile_rejects_every_binding_mismatch() {
        let valid = vega_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn jindo_compiled_profile_rejects_every_binding_and_policy_mismatch() {
        let valid = jindo_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                            JindoActivationLimitsV1 {
                                max_polynomial_count: 5,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn every_compiled_cryptographic_binding_is_immutable() {
        let valid = verange_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn compiled_validation_accepts_lower_protocol_policy_without_changing_digests() {
        let verange_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
                .expect("VeRange profile");
        let mut verange = verange_activation();
        verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&verange).expect("lower VeRange policy");
        assert_eq!(verange.parameter_id, verange_compiled.parameter_id);
        assert_eq!(verange.parameter_digest, verange_compiled.parameter_digest);
        assert_eq!(verange.verifier_digest, verange_compiled.verifier_digest);
        assert_eq!(
            verange.statement_schema_digest,
            verange_compiled.statement_schema_digest
        );
        assert_eq!(
            verange.engine_manifest_digest,
            verange_compiled.engine_manifest_digest
        );

        let pgc_compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("PGC profile");
        let mut pgc = pgc_activation();
        pgc.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&pgc).expect("lower PGC policy");
        assert_eq!(pgc.parameter_id, pgc_compiled.parameter_id);
        assert_eq!(pgc.parameter_digest, pgc_compiled.parameter_digest);
        assert_eq!(pgc.verifier_digest, pgc_compiled.verifier_digest);
        assert_eq!(
            pgc.statement_schema_digest,
            pgc_compiled.statement_schema_digest
        );
        assert_eq!(
            pgc.engine_manifest_digest,
            pgc_compiled.engine_manifest_digest
        );

        let jindo_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("Jindo profile");
        let mut jindo = jindo_activation();
        jindo.protocol_limits = PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&jindo).expect("lower Jindo policy");
        assert_eq!(jindo.parameter_id, jindo_compiled.parameter_id);
        assert_eq!(jindo.parameter_digest, jindo_compiled.parameter_digest);
        assert_eq!(jindo.verifier_digest, jindo_compiled.verifier_digest);
        assert_eq!(
            jindo.statement_schema_digest,
            jindo_compiled.statement_schema_digest
        );
        assert_eq!(
            jindo.engine_manifest_digest,
            jindo_compiled.engine_manifest_digest
        );

        let orchard_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
                .expect("Orchard profile");
        let mut orchard = orchard_activation();
        orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 1,
            });
        validate_compiled_privacy_activation_v1(&orchard).expect("lower Orchard policy");
        assert_eq!(orchard.parameter_id, orchard_compiled.parameter_id);
        assert_eq!(orchard.parameter_digest, orchard_compiled.parameter_digest);
        assert_eq!(orchard.verifier_digest, orchard_compiled.verifier_digest);
        assert_eq!(
            orchard.statement_schema_digest,
            orchard_compiled.statement_schema_digest
        );
        assert_eq!(
            orchard.engine_manifest_digest,
            orchard_compiled.engine_manifest_digest
        );
    }

    #[test]
    fn compiled_validation_rejects_protocol_limit_overflow_mismatch_and_invalid_lowering() {
        let mut invalid = Vec::new();

        let mut verange_over = verange_activation();
        verange_over.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 9,
            },
        );
        invalid.push(verange_over);

        let mut pgc_n_over = pgc_activation();
        pgc_n_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 65,
                max_recipient_count: 8,
            },
        );
        invalid.push(pgc_n_over);

        let mut pgc_k_over = pgc_activation();
        pgc_k_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 64,
                max_recipient_count: 9,
            },
        );
        invalid.push(pgc_k_over);

        let mut pgc_bad_closed_set = pgc_activation();
        pgc_bad_closed_set.protocol_limits =
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 17,
                    max_recipient_count: 1,
                },
            );
        invalid.push(pgc_bad_closed_set);

        let mut zero_verange = verange_activation();
        zero_verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 0,
            },
        );
        invalid.push(zero_verange);

        let mut jindo_over = jindo_activation();
        jindo_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 5,
                },
            );
        invalid.push(jindo_over);

        let mut zero_jindo = jindo_activation();
        zero_jindo.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 0,
                },
            );
        invalid.push(zero_jindo);

        let mut orchard_over = orchard_activation();
        orchard_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1 + 1,
            });
        invalid.push(orchard_over);

        let mut zero_orchard = orchard_activation();
        zero_orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 0,
            });
        invalid.push(zero_orchard);

        let mut wrong_variant = verange_activation();
        wrong_variant.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        invalid.push(wrong_variant);

        for activation in invalid {
            assert_eq!(
                validate_compiled_privacy_activation_v1(&activation),
                Err(CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch)
            );
        }
    }

    #[test]
    fn zk_x509_compiled_activation_is_complete_and_immutable() {
        let candidate = zk_x509_release_candidate_profile_material_v1()
            .expect("release candidate profile material");
        assert_eq!(
            (
                hex::encode(candidate.parameter_id.as_bytes()),
                hex::encode(candidate.parameter_digest.as_bytes()),
                hex::encode(candidate.verifier_digest.as_bytes()),
                hex::encode(candidate.statement_schema_digest.as_bytes()),
                hex::encode(candidate.engine_manifest_digest.as_bytes()),
            ),
            (
                "1ef8a47c6314a4a91e4446086b8c0c7110879e7770b441c663c1c398d5ea518b".to_owned(),
                "19c064109579bf83809043cec4e1ea9744af3486251e5253911f4d87634999ff".to_owned(),
                "4a7f1f34a569d9b5cedc137e12df012eee740dd32dbf2dff375b7f1b08766c0c".to_owned(),
                "f228f0d842277d2df246a1e6aa66880726a617d669e176efa37ad5a106bc7f60".to_owned(),
                "709883293be4fb2c89740490724394990c8f4d600c2b8e0a41a9539bd2211fdb".to_owned(),
            ),
            "every consensus-critical zk-X.509 binding is a pinned KAT",
        );
        let valid = zk_x509_activation();
        validate_compiled_privacy_activation_against_profile_v1(&valid, &candidate)
            .expect("exact release-pinned zk-X.509 activation");
        assert_eq!(
            valid.proof_system_id,
            PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        );
        assert_eq!(valid.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
        assert_eq!(
            valid.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaZkX509StarkP256V0
        );

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id =
                        PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_proof_system();
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| {
                    record.engine_id =
                        PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_engine();
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                            VeRangeActivationLimitsV1 {
                                max_aggregation_count: 1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_against_profile_v1(&changed, &candidate),
                Err(expected)
            );
        }

        let mut wrong_protocol = valid;
        wrong_protocol.protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        assert_eq!(
            validate_compiled_privacy_activation_against_profile_v1(&wrong_protocol, &candidate),
            Err(CompiledPrivacyProfileValidationErrorV1::ProtocolMismatch)
        );
    }

    #[test]
    fn anonymous_pgc_compiled_bindings_are_immutable() {
        let valid = pgc_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 5] = [
            |record| {
                record.parameter_digest.0[0] ^= 1;
            },
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                let PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(ref mut limits) =
                    record.protocol_limits
                else {
                    unreachable!("fixture is Anonymous PGC");
                };
                limits.max_recipient_count += 1;
            },
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }
}
