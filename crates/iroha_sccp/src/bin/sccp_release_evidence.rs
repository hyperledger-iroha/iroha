//! Offline canonical validator for public SCCP release-lane evidence.
//!
//! This binary is intentionally read-only and deterministic. It accepts one
//! bounded canonical Norito JSON file, invokes the same typed native verifier
//! used by admission, and emits one bounded machine-readable validation
//! receipt. It never accesses a network, signs data, or writes files.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use iroha_data_model::block::consensus_v2::PROTOCOL_VERSION as SUMERAGI_V2_PROTOCOL_VERSION;
use iroha_data_model::bridge::{
    BridgeSccpDestinationProofBackendV1, SccpDestinationDeploymentV1,
    SccpEvmDestinationDeploymentV1, SccpGovernedRouteV1, SccpGroth16Bn254SemanticCircuitV1,
    SccpGroth16Bn254VerifyingKeyV1, SccpNativeTrustAnchorV1, SccpNetworkV1,
    SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1, SccpSourceIdentityV1,
    SccpTronDestinationDeploymentV1, sccp_groth16_bn254_public_signal_schema_hash_v1,
    sccp_network_identity_hash_v1, sccp_semantic_proof_profile_hash_v1,
    sccp_sora_finality_anchor_hash_v1,
};
use iroha_sccp::{
    SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1, SCCP_TAIRA_FINALITY_CHAIN_ID_V1,
    SccpNativeInboundMessageProofV1, SccpPayloadV1, ValidatedSccpNativeInboundMessageV1,
    canonical_sccp_groth16_bn254_verifying_key_bytes_v1,
    decode_canonical_sccp_groth16_bn254_proof_artifact_v1,
    decode_canonical_taira_sccp_message_bundle_v1, sccp_groth16_bn254_public_signal_words,
    sccp_groth16_bn254_verifying_key_hash_v1, sccp_native_inbound_source_available_v1,
    verify_sccp_native_inbound_message_proof_v1,
};
use sha2::{Digest, Sha256};
use tiny_keccak::{Hasher as _, Keccak};

const INPUT_SCHEMA: &str = "sccp-release-lane-evidence-v1";
const OUTPUT_SCHEMA: &str = "sccp-release-lane-validation-v1";
const RELEASE_SIGNATURE_OUTPUT_SCHEMA: &str = "sccp-release-signature-validation-v1";
const SEMANTIC_PROOF_OUTPUT_SCHEMA: &str = "sccp-semantic-proof-validation-v1";
const PRODUCTION_POLICY_SCHEMA: &str = "sccp-release-trust-policy-v1";
const TEST_POLICY_SCHEMA: &str = "sccp-release-test-trust-policy-v1";
const RELEASE_EVIDENCE_SCHEMA: &str = "sccp-release-evidence-v1";
const VALIDATOR_PROTOCOL_VERSION: u8 = 1;
#[cfg(feature = "test-fixtures")]
const MAX_INPUT_BYTES: u64 = 40 * 1024 * 1024;
const MAX_LANE_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RELEASE_POLICY_BYTES: u64 = 64 * 1024;
const MAX_RELEASE_EVIDENCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_VALIDATOR_BINARY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TRANSCRIPT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_AUDIT_REPORT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SEMANTIC_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RELEASE_ARTIFACTS: usize = 64;
const MAX_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_RUNTIME_CODE_BYTES: usize = 24_576;
const MAX_DESTINATION_ATTESTATION_AGE_MS: u64 = 24 * 60 * 60 * 1_000;
const BUILD_ID_DOMAIN: &[u8] = b"sccp:release-evidence-validator-build:v1\0";
const DESTINATION_ATTESTATION_DOMAIN: &[u8] = b"iroha:sccp:destination-state-attestation:v1\0";
const RELEASE_SIGNING_DOMAIN: &[u8] = b"iroha:sccp:release-evidence:v1\0";
const CIRCUIT_AUDIT_DOMAIN: &[u8] = b"iroha:sccp:circuit-policy-audit:v1\0";
const RELEASE_PROFILES: [&str; 3] = ["ethereum-mainnet", "bsc-mainnet", "tron-mainnet"];
const RELEASE_DOMAINS: [u32; 3] = [1, 2, 5];
const RELEASE_ROLES: [&str; 2] = ["release-engineering", "release-security"];
const CIRCUIT_AUDIT_ROLES: [&str; 2] = ["semantic-security-audit", "prover-reproducibility-audit"];
const SEMANTIC_ARTIFACT_ROLES: [(&str, &str, &str); 7] = [
    ("circuit-artifact", "semantic-circuit", "circuit.bin"),
    (
        "witness-generator",
        "witness-generator",
        "witness-generator.bin",
    ),
    ("verifying-key", "verifying-key", "verifying-key.bin"),
    ("prover-build", "prover-build", "prover-build.bin"),
    ("toolchain-lock", "toolchain-lock", "toolchain.lock"),
    ("honest-witness", "honest-witness", "honest-witness.bin"),
    ("honest-proof", "honest-proof", "honest-proof.norito"),
];
const FORBIDDEN_FIXTURE_PUBLIC_KEYS: [&str; 53] = [
    "3908a9df4eb45c2c3eb744f5a5fde5af87f346a59a4995378e95c3895b9e2d5d",
    "4baed4d3a15b3269ab5e710393de6f01944c3af9691dc7a8661474ced9a033f2",
    "0ffb0e0e942b1f2250eb5674aa5674334cb0e84a7374369cc9d9ec636392198e",
    "64bd5cff290fca9a6102466a0be471375712f102cb6548acf9cdec4d0505e6a9",
    "6c78a68b726ddad7bbedcad5d8e118d6c8bde280fa09c2ce543b83a68d339a5c",
    "2c3bc99608eb07dcd184bf8d459b616256bbcc08ae6b54339d3aa41ce18226a8",
    "56b99cacf316965f254d214d011b18fecc16db9bd4d849d484ee127f7ec9404e",
    "d90ee0c2aa6e1f57f8aefe1d29dc8959664320e05885478920a9a9d50443d7fc",
    "3358d5cc6df49720a5e4930f2d265384ca54b9357ae4b0cabb365fa679e8cca1",
    "52c9bf4edf5edbfdee818f492da93d3bd9e5b7ccd729c5742f0b73b9654968e0",
    "f41d0ecf2085d23684181cb9f91e87ce8569504c5910f383578ebebb9c4501a2",
    "bc9b93208bca878fdc78dfad81c66aeee61648c2f2ee244e8e2248053854e0cc",
    "dd34325c20f1be9a0f4ff5486d841692a5aa0ed32db8b3fc4f7c1a2c2d82915d",
    "71855fa376f5bb419aa57d85b0a014b41811a6c4e18c776acdcf18c5f94d4309",
    "a2e5089b86562bc2994e55d4aa44d6923b208e7a29901b5d533798f29885775f",
    "bd0c9cca744a3bb392778a1f3925fe384ea16ea84dd80ac92f3fb453321593ee",
    "0568eb8928f1a3c9623ced2dcd749a000ee25a6922ba32686892357765ef3b91",
    "030af83691318aa2a4c6091d8f64afdc8af513c387b7cde2228e7c5589ba7c74",
    "3a6344e5b76fabf07f91ff396c82b36642ff30eb26d7d66d4acef8d389f354b1",
    "3b6b6fa357dcec265b24a70ce8808a4a75e2393994be06ad3958be3c9c68749a",
    "a5b2610c54fcf817d94fb832578cc477eaeade34bd0a58de9b503213ef908e64",
    "f40674938b1a40e4670d318b42b47ba9fef3582099bcfefc92790244b0f4cb68",
    "7b93db743c32a07ccc2c48569645a3cf2a980a1733da7f07d60161a09cef679b",
    "1c0f6ccb3f6003808376dd4090ed76d9e1f4c830fcd4bf8df2aa8a0616ea754f",
    "4eb6252d1332fe20b1baa620e80635f3a4cd0a131d6d3abcb93cfa925732ce12",
    "05f80c4badfbc7015606fcb192dda45f7536f7c1191ef063260bf982ae4e52c0",
    "07ecef22532a6859823046b92b183b90e38b6c367fc1af6ead429be7cbbdc0f5",
    "1b60f8f63d68bb772e5cb5ff7dd98996895a5a7430d9e82f48f48d4776cd1a3b",
    "366e703d99bdbe0a2a4db1a664acd52c43b03f9d053025eb19bda13a5e0a6066",
    "df62654404d5e37e3ba68dd14b97117eb199803f4a10a2473e3b7b848e67a1b5",
    "073fb6ce0ac504252d2fe848ad7cbf6afe92bc727a340667f9d2ca56e3331ad7",
    "f34444167e0c2810cf4072d1c34b7175d380de2e0efdf48b762247b8bfd5d04b",
    "0666855cf4012140b0cea429d456f14cfbdc53982eab592af675f918d435947c",
    "b38b424605d0a3d4a4718f497dde90932444e7f96f48539a7f5a9b6ad8ef0fdd",
    "4fceb3bc8a659bce4beba05fe63c79671a4430a112c4c5448e69deeec1d52770",
    "38861629012e021d8fcfc202ae485b431adff8aa87d5b0b3b8c92048461c1779",
    "330dde2b028c8853134e29aa3ae92832df2ecbe1a5d36f4d800a233fd7e8f4ae",
    "dbbcfd7c3b1c494e9bf8e52d76c4d388d45a4f62da5b36dead40a852e7693bb3",
    "971e807f423e356f0b14adc7a933448b409b97e2f59e75f74e9999875daf384c",
    "fe2b875714f38b99fdfc116fa3f86baba2377602c08f91818f115042afa9360b",
    "28606717bbb2ad7b0540afc392dda40c1df589161243f06b3ab84455d3ceae52",
    "c3515b02fa51a33640b346dcf9d2cb60b16c362b7e95b4dbd38711923635dfb3",
    "f32dc052551832ded5f27d9ca3234ec984b1c07bb540368beb8163c3f2c1c480",
    "15eaf0882db809a33a3fb533353b4afe43af0ffde1e86c5fd13f91e943b6ee00",
    "453ed15553be21331012655ee17d1dfaca6b86a87df7d0e6c040e87a23396c9e",
    "7ddb0a311b568eb3875864f641b0993ab5303c952278166a40d8e7e658fb9908",
    "a7e7cbae831e6b2cce0a80f072608a8d441ffcd78e519163cbf604f02abc6eb7",
    "5896c7ec6a3c44685efec5c23bea9e0c79026e8c844de5df3e9f723abc53dadd",
    "04f866e68e71310baba066fd1d0005d08885c04e5557c356a0a8a7e1270a3937",
    "111fa14f8f6a46dc184a584610d78372ffabc532a40c5bcea6a6812546b8cf38",
    "a38817b53f5d49f0c95057ac0f0ac0896c9b31a60dada241a3e68c9f0e6a7f01",
    "428cbad36d48107627a178faf4678967ed56a453698a8c41a102ed8176dbc316",
    "a916597e070ce70ae69a4a3bbb564714a9b95559ed941eb3b2edfb6568fb6bf3",
];
const REQUIRED_PHASES: [&str; 12] = [
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "tvm-contract-smoke",
    "core-admission",
    "runtime-api",
];
const FORBIDDEN_ALGEBRAIC_SMOKE_VK: [u8; 32] = [
    0x9e, 0xf8, 0x06, 0x7d, 0x26, 0x05, 0x32, 0xf8, 0x8e, 0x60, 0xcf, 0xa4, 0xb4, 0x58, 0xfe, 0x67,
    0x8f, 0xc4, 0x6b, 0x9c, 0x24, 0x2d, 0xe1, 0x8f, 0xc9, 0x1b, 0xa6, 0x46, 0xe0, 0x85, 0x7f, 0xc4,
];
const OUTBOUND_UNAVAILABLE_REASON: &str = "authenticated-destination-state-is-unavailable";
const REQUIRED_SEMANTICS: [&str; 7] = [
    "sccp-canonical-transfer-v1",
    "sccp-message-leaf-v1",
    "sccp-merkle-inclusion-v1",
    "sora-taira-block-commitment-v1",
    "sora-taira-v2-finality-artifact-v1",
    "sora-taira-v2-dual-quorum-v1",
    "sora-taira-anchor-continuity-v1",
];
const RELEASE_CIRCUIT_IDS: [&str; 3] = [
    "sccp-sora-taira-to-ethereum-mainnet-groth16-bn254-v1",
    "sccp-sora-taira-to-bsc-mainnet-groth16-bn254-v1",
    "sccp-sora-taira-to-tron-mainnet-groth16-bn254-v1",
];
/// Exact SHA-256 of the diagnostic-only labeled-signal-binding circuit.
///
/// Keep this literal independent of the current repository artifact: a future
/// edit to that diagnostic source must not make the already-published unsafe
/// digest acceptable to a production release validator.
const FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256: [u8; 32] = [
    0xd7, 0x04, 0x9d, 0xe0, 0xf0, 0xb0, 0xec, 0xb7, 0xec, 0x4f, 0x64, 0xb8, 0x85, 0x64, 0x6a, 0xb9,
    0x9f, 0x85, 0xfc, 0xba, 0xb0, 0x5d, 0xfa, 0xf7, 0x10, 0xd3, 0x00, 0x2f, 0x17, 0x63, 0x2b, 0xb9,
];
const VALIDATOR_SOURCE: &[u8] = include_bytes!("sccp_release_evidence.rs");
const SCCP_CRATE_MANIFEST: &[u8] = include_bytes!("../../Cargo.toml");
const SCCP_BUILD_SCRIPT: &[u8] = include_bytes!("../../build.rs");
const WORKSPACE_MANIFEST: &[u8] = include_bytes!("../../../../Cargo.toml");
const CARGO_LOCK: &[u8] = include_bytes!("../../../../Cargo.lock");
const RUST_TOOLCHAIN_LOCK: &[u8] = include_bytes!("../../../../rust-toolchain.toml");

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseLaneEvidenceV1 {
    schema: String,
    version: u8,
    profile: SccpNetworkV1,
    inbound: ReleaseInboundEvidenceV1,
    outbound: ReleaseOutboundEvidenceV1,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "status", content = "evidence", rename_all = "snake_case")]
enum ReleaseInboundEvidenceV1 {
    Available(AvailableInboundEvidenceV1),
    Unavailable(UnavailableDirectionV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct AvailableInboundEvidenceV1 {
    proof: SccpNativeInboundMessageProofV1,
    governed_source_identity: SccpSourceIdentityV1,
    governed_trust_anchor: SccpNativeTrustAnchorV1,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "status", content = "evidence", rename_all = "snake_case")]
enum ReleaseOutboundEvidenceV1 {
    Available(AvailableOutboundEvidenceV1),
    Unavailable(UnavailableDirectionV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct AvailableOutboundEvidenceV1 {
    statement: DestinationStateStatementV1,
    attestor_id: String,
    signature_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(tag = "family", content = "state", rename_all = "snake_case")]
enum DestinationStateStatementV1 {
    Evm(EvmDestinationStateV1),
    Tron(TronDestinationStateV1),
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct EvmDestinationStateV1 {
    schema: String,
    profile: SccpNetworkV1,
    observed_at_unix_ms: u64,
    finalized_block_height: u64,
    finalized_block_hash: [u8; 32],
    rpc_chain_id: u64,
    network_identity_hash: [u8; 32],
    governed_route: SccpGovernedRouteV1,
    route_revision: u32,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: String,
    verifier_runtime_code_hex: String,
    route_runtime_code_hex: String,
    verifier_key_hash: [u8; 32],
    semantic_proof_profile_hash: [u8; 32],
    sora_finality_anchor_hash: [u8; 32],
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TronDestinationStateV1 {
    schema: String,
    profile: SccpNetworkV1,
    observed_at_unix_ms: u64,
    solid_block_height: u64,
    solid_block_hash: [u8; 32],
    network_magic: u32,
    network_identity_hash: [u8; 32],
    governed_route: SccpGovernedRouteV1,
    route_revision: u32,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: String,
    verifier_runtime_code_hex: String,
    route_runtime_code_hex: String,
    verifier_key_hash: [u8; 32],
    semantic_proof_profile_hash: [u8; 32],
    sora_finality_anchor_hash: [u8; 32],
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct UnavailableDirectionV1 {
    reason: String,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct ReleaseLaneValidationV1 {
    schema: String,
    validator: ValidatorIdentityV1,
    trust_policy_id: String,
    trust_policy_sha256_hex: String,
    release_id: String,
    release_evidence_sha256_hex: String,
    artifact_sha256_hex: String,
    profile: String,
    inbound_status: String,
    outbound_status: String,
    unavailable_reasons: Vec<String>,
    source_profile: Option<String>,
    target_profile: Option<String>,
    lane_hash_hex: Option<String>,
    source_identity_hash_hex: Option<String>,
    native_anchor_hash_hex: Option<String>,
    message_id_hex: Option<String>,
    payload_hash_hex: Option<String>,
    source_event_digest_hex: Option<String>,
    finality_height: Option<String>,
    finality_block_hash_hex: Option<String>,
    destination_attestor_id: Option<String>,
    destination_statement_sha256_hex: Option<String>,
    destination_observed_at_unix_ms: Option<String>,
    destination_finality_height: Option<String>,
    destination_finality_block_hash_hex: Option<String>,
    destination_binding_hash_hex: Option<String>,
    route_configuration_hash_hex: Option<String>,
    governed_route_configuration_hash_hex: Option<String>,
    verifier_key_hash_hex: Option<String>,
    route_revision: Option<String>,
    verifying_key_sha256_hex: Option<String>,
    semantic_circuit_id: Option<String>,
    circuit_artifact_sha256_hex: Option<String>,
    witness_generator_sha256_hex: Option<String>,
    public_signal_schema_hash_hex: Option<String>,
    semantic_proof_profile_hash_hex: Option<String>,
    sora_finality_anchor_hash_hex: Option<String>,
    prover_build_sha256_hex: Option<String>,
    toolchain_lock_sha256_hex: Option<String>,
    destination_build_policy_sha256_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, norito::JsonSerialize, norito::JsonDeserialize)]
struct ValidatorIdentityV1 {
    protocol_version: u8,
    crate_name: String,
    crate_version: String,
    enabled_features: Vec<String>,
    build_profile: String,
    target_triple: String,
    rustc_version: String,
    source_sha256_hex: String,
    crate_manifest_sha256_hex: String,
    build_script_sha256_hex: String,
    workspace_manifest_sha256_hex: String,
    cargo_lock_sha256_hex: String,
    toolchain_lock_sha256_hex: String,
    executable_sha256_hex: String,
    build_identity_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseTrustPolicyV1 {
    schema: String,
    environment: String,
    policy_id: String,
    roles: Vec<TrustedReleaseRoleV1>,
    destination_attestors: Vec<TrustedDestinationAttestorV1>,
    circuit_auditors: Vec<TrustedCircuitAuditorV1>,
    proof_systems: Vec<ProofSystemPolicyV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedReleaseRoleV1 {
    role: String,
    signer_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedDestinationAttestorV1 {
    counterparty_profile: String,
    attestor_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct TrustedCircuitAuditorV1 {
    role: String,
    auditor_id: String,
    public_key_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct SoraFinalityAnchorPolicyV1 {
    version: u8,
    source_profile: String,
    protocol_version: u16,
    chain_id_hash_hex: String,
    checkpoint_height: u64,
    checkpoint_block_hash_hex: String,
    checkpoint_context_id_hex: String,
    checkpoint_finality_artifact_hash_hex: String,
}

#[derive(Debug, Clone, Copy)]
struct ValidatedSoraFinalityAnchorPolicyV1 {
    anchor: SccpSoraFinalityAnchorV1,
    anchor_hash: [u8; 32],
    chain_id_hash: [u8; 32],
    checkpoint_block_hash: [u8; 32],
    checkpoint_context_id: [u8; 32],
    checkpoint_finality_artifact_hash: [u8; 32],
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ProofSystemPolicyV1 {
    counterparty_profile: String,
    circuit_id: String,
    semantics: Vec<String>,
    circuit_artifact_sha256_hex: String,
    witness_generator_sha256_hex: String,
    public_signal_schema_hash_hex: String,
    semantic_proof_profile_hash_hex: String,
    sora_finality_anchor: SoraFinalityAnchorPolicyV1,
    sora_finality_anchor_hash_hex: String,
    verifier_key_hash_hex: String,
    route_revision: u32,
    verifying_key_sha256_hex: String,
    prover_build_sha256_hex: String,
    toolchain_lock_sha256_hex: String,
    destination_build: DestinationBuildPolicyV1,
    audit_attestations: Vec<CircuitAuditAttestationV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct DestinationBuildPolicyV1 {
    source_bundle_sha256_hex: String,
    compiler_build_sha256_hex: String,
    token_artifact_sha256_hex: String,
    token_interface_sha256_hex: String,
    token_runtime_hash_hex: String,
    verifier_artifact_sha256_hex: String,
    verifier_interface_sha256_hex: String,
    verifier_runtime_hash_hex: String,
    route_artifact_sha256_hex: String,
    route_interface_sha256_hex: String,
    route_runtime_hash_hex: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct CircuitAuditAttestationV1 {
    role: String,
    auditor_id: String,
    algorithm: String,
    public_key_hex: String,
    report_sha256_hex: String,
    signature_b64: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseEvidenceSignaturesV1 {
    schema: String,
    release_id: String,
    protocol_version: u8,
    hub_profile: String,
    hub_chain_id: String,
    created_at_unix_ms: u64,
    trust_policy_id: String,
    trust_policy_sha256_hex: String,
    validator: ValidatorIdentityV1,
    lanes: Vec<SignedLaneSummaryV1>,
    artifacts: Vec<ReleaseArtifactV1>,
    validation: ReleaseValidationV1,
    provenance: Vec<ReleaseProvenanceV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct SignedLaneSummaryV1 {
    counterparty_profile: String,
    counterparty_domain: u32,
    inbound_status: String,
    outbound_status: String,
    evidence_artifact_path: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseArtifactV1 {
    path: String,
    kind: String,
    sha256_hex: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseValidationV1 {
    corridor: String,
    phases: Vec<ReleaseValidationPhaseV1>,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseValidationPhaseV1 {
    name: String,
    status: String,
    artifact_path: String,
}

#[derive(Debug, Clone, norito::JsonSerialize, norito::JsonDeserialize)]
struct ReleaseProvenanceV1 {
    role: String,
    signer_id: String,
    algorithm: String,
    public_key_hex: String,
    signature_b64: String,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct ReleaseSignatureValidationV1 {
    schema: String,
    environment: String,
    policy_id: String,
    release_id: String,
    policy_sha256_hex: String,
    evidence_sha256_hex: String,
    release_signatures_verified: u8,
    circuit_audit_signatures_verified: u8,
    destination_attestors_validated: u8,
    distinct_trust_identities: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, norito::JsonSerialize)]
struct SemanticProofClaimV1 {
    source_profile: String,
    target_profile: String,
    target_domain: u32,
    route_revision: u32,
    message_id_hex: String,
    payload_hash_hex: String,
    commitment_root_hex: String,
    finality_height: String,
    finality_block_hash_hex: String,
    destination_binding_hash_hex: String,
    route_configuration_hash_hex: String,
    statement_hash_hex: String,
    request_hash_hex: String,
    result_hash_hex: String,
    verifier_key_hash_hex: String,
    semantic_proof_profile_hash_hex: String,
    sora_finality_anchor_hash_hex: String,
    public_signal_words_hex: Vec<String>,
}

#[derive(Debug, Clone, norito::JsonSerialize)]
struct SemanticProofValidationV1 {
    schema: String,
    environment: String,
    policy_id: String,
    release_id: String,
    policy_sha256_hex: String,
    evidence_sha256_hex: String,
    proof_artifact_path: String,
    proof_artifact_sha256_hex: String,
    canonical_norito_verified: bool,
    pairing_verified: bool,
    claim: SemanticProofClaimV1,
}

#[derive(Debug, Clone)]
struct ApprovedProofSystemV1 {
    circuit_id: String,
    circuit_artifact_sha256: [u8; 32],
    witness_generator_sha256: [u8; 32],
    public_signal_schema_hash: [u8; 32],
    semantic_proof_profile_hash: [u8; 32],
    sora_finality_anchor_hash: [u8; 32],
    verifier_key_hash: [u8; 32],
    route_revision: u32,
    verifying_key_sha256: [u8; 32],
    prover_build_sha256: [u8; 32],
    toolchain_lock_sha256: [u8; 32],
    destination_build_policy_sha256: [u8; 32],
    token_runtime_hash: [u8; 32],
    verifier_runtime_hash: [u8; 32],
    route_runtime_hash: [u8; 32],
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn push_length_prefixed(output: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let length = u32::try_from(value.len())
        .map_err(|_| "validator build metadata exceeds u32 framing".to_owned())?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value);
    Ok(())
}

fn enabled_build_features() -> Vec<String> {
    env!("IROHA_SCCP_BUILD_FEATURES")
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(str::to_owned)
        .collect()
}

struct ValidatorBuildIdentityInputs<'a> {
    protocol_version: u8,
    crate_name: &'a str,
    crate_version: &'a str,
    enabled_features: &'a [String],
    build_profile: &'a str,
    target_triple: &'a str,
    rustc_version: &'a str,
    source_hash: [u8; 32],
    crate_manifest_hash: [u8; 32],
    build_script_hash: [u8; 32],
    workspace_manifest_hash: [u8; 32],
    cargo_lock_hash: [u8; 32],
    toolchain_lock_hash: [u8; 32],
}

fn validator_build_identity_hash(
    inputs: &ValidatorBuildIdentityInputs<'_>,
) -> Result<[u8; 32], String> {
    let mut build = Vec::with_capacity(512);
    build.extend_from_slice(BUILD_ID_DOMAIN);
    build.push(inputs.protocol_version);
    for value in [
        inputs.crate_name,
        inputs.crate_version,
        inputs.build_profile,
        inputs.target_triple,
        inputs.rustc_version,
    ] {
        push_length_prefixed(&mut build, value.as_bytes())?;
    }
    let feature_count = u32::try_from(inputs.enabled_features.len())
        .map_err(|_| "validator feature count exceeds u32 framing".to_owned())?;
    build.extend_from_slice(&feature_count.to_le_bytes());
    for feature in inputs.enabled_features {
        push_length_prefixed(&mut build, feature.as_bytes())?;
    }
    for hash in [
        inputs.source_hash,
        inputs.crate_manifest_hash,
        inputs.build_script_hash,
        inputs.workspace_manifest_hash,
        inputs.cargo_lock_hash,
        inputs.toolchain_lock_hash,
    ] {
        build.extend_from_slice(&hash);
    }
    Ok(sha256(&build))
}

fn validator_identity() -> Result<ValidatorIdentityV1, String> {
    let source_hash = sha256(VALIDATOR_SOURCE);
    let crate_manifest_hash = sha256(SCCP_CRATE_MANIFEST);
    let build_script_hash = sha256(SCCP_BUILD_SCRIPT);
    let workspace_manifest_hash = sha256(WORKSPACE_MANIFEST);
    let cargo_lock_hash = sha256(CARGO_LOCK);
    let toolchain_lock_hash = sha256(RUST_TOOLCHAIN_LOCK);
    let executable_path = env::current_exe()
        .map_err(|_| "validator executable path cannot be authenticated".to_owned())?;
    let executable_hash = sha256(&read_direct_input(
        &executable_path,
        MAX_VALIDATOR_BINARY_BYTES,
    )?);
    let enabled_features = enabled_build_features();
    let build_identity = validator_build_identity_hash(&ValidatorBuildIdentityInputs {
        protocol_version: VALIDATOR_PROTOCOL_VERSION,
        crate_name: env!("CARGO_PKG_NAME"),
        crate_version: env!("CARGO_PKG_VERSION"),
        enabled_features: &enabled_features,
        build_profile: env!("IROHA_SCCP_BUILD_PROFILE"),
        target_triple: env!("IROHA_SCCP_BUILD_TARGET"),
        rustc_version: env!("IROHA_SCCP_RUSTC_VERSION"),
        source_hash,
        crate_manifest_hash,
        build_script_hash,
        workspace_manifest_hash,
        cargo_lock_hash,
        toolchain_lock_hash,
    })?;
    Ok(ValidatorIdentityV1 {
        protocol_version: VALIDATOR_PROTOCOL_VERSION,
        crate_name: env!("CARGO_PKG_NAME").to_owned(),
        crate_version: env!("CARGO_PKG_VERSION").to_owned(),
        enabled_features,
        build_profile: env!("IROHA_SCCP_BUILD_PROFILE").to_owned(),
        target_triple: env!("IROHA_SCCP_BUILD_TARGET").to_owned(),
        rustc_version: env!("IROHA_SCCP_RUSTC_VERSION").to_owned(),
        source_sha256_hex: lowercase_hex(&source_hash),
        crate_manifest_sha256_hex: lowercase_hex(&crate_manifest_hash),
        build_script_sha256_hex: lowercase_hex(&build_script_hash),
        workspace_manifest_sha256_hex: lowercase_hex(&workspace_manifest_hash),
        cargo_lock_sha256_hex: lowercase_hex(&cargo_lock_hash),
        toolchain_lock_sha256_hex: lowercase_hex(&toolchain_lock_hash),
        executable_sha256_hex: lowercase_hex(&executable_hash),
        build_identity_hex: lowercase_hex(&build_identity),
    })
}

fn locked_rust_version() -> Result<&'static str, String> {
    let text = std::str::from_utf8(RUST_TOOLCHAIN_LOCK)
        .map_err(|_| "Rust toolchain lock is not UTF-8".to_owned())?;
    text.strip_prefix("[toolchain]\nchannel = \"")
        .and_then(|value| value.strip_suffix("\"\n"))
        .filter(|value| {
            let mut components = value.split('.');
            let first = components.next();
            let second = components.next();
            let third = components.next();
            components.next().is_none()
                && [first, second, third].into_iter().all(|component| {
                    component.is_some_and(|component| {
                        !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
                    })
                })
        })
        .ok_or_else(|| "Rust toolchain lock does not pin one exact stable version".to_owned())
}

fn canonical_rustc_version(value: &str, locked_version: &str) -> bool {
    let mut parts = value.split(' ');
    if parts.next() != Some("rustc") || parts.next() != Some(locked_version) {
        return false;
    }
    let Some(commit) = parts.next().and_then(|part| part.strip_prefix('(')) else {
        return false;
    };
    let Some(date) = parts.next().and_then(|part| part.strip_suffix(')')) else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    (9..=40).contains(&commit.len())
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn validate_validator_identity_shape(identity: &ValidatorIdentityV1) -> Result<(), String> {
    if identity.protocol_version != VALIDATOR_PROTOCOL_VERSION
        || identity.crate_name != env!("CARGO_PKG_NAME")
        || identity.crate_version != env!("CARGO_PKG_VERSION")
        || !identity.enabled_features.is_empty()
        || !matches!(identity.build_profile.as_str(), "debug" | "release")
        || !canonical_identifier(&identity.target_triple)
        || identity.target_triple.matches('-').count() < 2
        || !canonical_rustc_version(&identity.rustc_version, locked_rust_version()?)
    {
        return Err("validator build metadata is not exact or production-capable".to_owned());
    }
    let source_hash = require_hash(&identity.source_sha256_hex, "validator source hash")?;
    let crate_manifest_hash = require_hash(
        &identity.crate_manifest_sha256_hex,
        "validator crate manifest hash",
    )?;
    let build_script_hash = require_hash(
        &identity.build_script_sha256_hex,
        "validator build script hash",
    )?;
    let workspace_manifest_hash = require_hash(
        &identity.workspace_manifest_sha256_hex,
        "validator workspace manifest hash",
    )?;
    let cargo_lock_hash =
        require_hash(&identity.cargo_lock_sha256_hex, "validator Cargo lock hash")?;
    let toolchain_lock_hash = require_hash(
        &identity.toolchain_lock_sha256_hex,
        "validator toolchain lock hash",
    )?;
    let executable_hash =
        require_hash(&identity.executable_sha256_hex, "validator executable hash")?;
    let build_identity = require_hash(&identity.build_identity_hex, "validator build identity")?;
    let hashes = [
        source_hash,
        crate_manifest_hash,
        build_script_hash,
        workspace_manifest_hash,
        cargo_lock_hash,
        toolchain_lock_hash,
        executable_hash,
        build_identity,
    ];
    if hashes.iter().copied().collect::<BTreeSet<_>>().len() != hashes.len() {
        return Err("validator build hash roles must be pairwise distinct".to_owned());
    }
    let expected_build_identity = validator_build_identity_hash(&ValidatorBuildIdentityInputs {
        protocol_version: identity.protocol_version,
        crate_name: &identity.crate_name,
        crate_version: &identity.crate_version,
        enabled_features: &identity.enabled_features,
        build_profile: &identity.build_profile,
        target_triple: &identity.target_triple,
        rustc_version: &identity.rustc_version,
        source_hash,
        crate_manifest_hash,
        build_script_hash,
        workspace_manifest_hash,
        cargo_lock_hash,
        toolchain_lock_hash,
    })?;
    if build_identity != expected_build_identity {
        return Err("validator build identity does not bind its exact inputs".to_owned());
    }
    Ok(())
}

fn decode_lower_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be exactly {N} bytes of lowercase hex"
        ));
    }
    let mut decoded = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("alphabet checked above"),
        };
        decoded[index] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    if decoded.iter().all(|byte| *byte == 0) {
        return Err(format!("{label} must not be zero"));
    }
    Ok(decoded)
}

fn canonical_identifier(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
}

fn canonical_relative_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(1..=240).contains(&bytes.len())
        || !value.is_ascii()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
    {
        return false;
    }
    value.split('/').all(|segment| {
        let bytes = segment.as_bytes();
        (1..=96).contains(&bytes.len())
            && bytes
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(*byte, b'.' | b'_' | b'-')
            })
    })
}

fn decode_signature_base64(value: &str) -> Result<[u8; 64], String> {
    if value.len() != 88 || !value.ends_with("==") || !value.is_ascii() {
        return Err("signature must be canonical padded base64 for 64 bytes".to_owned());
    }
    let digit = |byte: u8| -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let bytes = value.as_bytes();
    let mut decoded = [0_u8; 64];
    for group in 0..21 {
        let offset = group * 4;
        let a = digit(bytes[offset]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let b = digit(bytes[offset + 1]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let c = digit(bytes[offset + 2]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        let d = digit(bytes[offset + 3]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
        decoded[group * 3] = (a << 2) | (b >> 4);
        decoded[group * 3 + 1] = (b << 4) | (c >> 2);
        decoded[group * 3 + 2] = (c << 6) | d;
    }
    let a = digit(bytes[84]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
    let b = digit(bytes[85]).ok_or_else(|| "signature base64 is invalid".to_owned())?;
    if b & 0x0f != 0 {
        return Err("signature base64 has non-canonical pad bits".to_owned());
    }
    decoded[63] = (a << 2) | (b >> 4);
    Ok(decoded)
}

fn validate_ed25519_public_key(value: &str, label: &str) -> Result<[u8; 32], String> {
    let key = decode_lower_hex::<32>(value, label)?;
    iroha_crypto::PublicKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &key)
        .map_err(|_| format!("{label} is not a strict Ed25519 public key"))?;
    Ok(key)
}

fn verify_ed25519_signature(
    key: &[u8; 32],
    signature: &[u8; 64],
    message: &[u8],
) -> Result<(), String> {
    iroha_crypto::ed25519_verify_batch_deterministic(
        &[message],
        &[signature.as_slice()],
        &[key.as_slice()],
        sha256(message),
    )
    .map_err(|_| "detached Ed25519 signature is invalid".to_owned())
}

fn parse_canonical_sorted_json(bytes: &[u8], label: &str) -> Result<norito::json::Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{label} must be UTF-8"))?;
    if text.as_bytes().contains(&0) {
        return Err(format!("{label} must not contain NUL"));
    }
    let value = norito::json::parse_value(text)
        .map_err(|_| format!("{label} is not strict Norito JSON"))?;
    validate_json_shape(&value, label)?;
    let canonical = norito::json::to_json(&value)
        .map_err(|_| format!("{label} cannot be canonically encoded"))?;
    if format!("{canonical}\n") != text {
        return Err(format!("{label} must be sorted canonical JSON plus one LF"));
    }
    Ok(value)
}

fn validate_json_shape(value: &norito::json::Value, label: &str) -> Result<(), String> {
    const MAX_JSON_DEPTH: usize = 32;
    const MAX_JSON_NODES: usize = 32_768;

    let mut pending = vec![(value, 0_usize)];
    let mut nodes = 0_usize;
    while let Some((current, depth)) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| format!("{label} JSON node count overflowed"))?;
        if nodes > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return Err(format!("{label} exceeds the canonical JSON shape limits"));
        }
        match current {
            norito::json::Value::Array(values) => {
                pending.extend(values.iter().map(|child| (child, depth + 1)));
            }
            norito::json::Value::Object(values) => {
                pending.extend(values.values().map(|child| (child, depth + 1)));
            }
            norito::json::Value::Number(norito::json::native::Number::F64(_)) => {
                return Err(format!("{label} must not contain floating-point numbers"));
            }
            norito::json::Value::Null
            | norito::json::Value::Bool(_)
            | norito::json::Value::Number(_)
            | norito::json::Value::String(_) => {}
        }
    }
    Ok(())
}

fn require_hash(value: &str, label: &str) -> Result<[u8; 32], String> {
    decode_lower_hex::<32>(value, label)
}

fn destination_build_hash_roles(
    value: &DestinationBuildPolicyV1,
) -> Result<Vec<(&'static str, [u8; 32])>, String> {
    let fields = [
        ("source_bundle_sha256_hex", &value.source_bundle_sha256_hex),
        (
            "compiler_build_sha256_hex",
            &value.compiler_build_sha256_hex,
        ),
        (
            "token_artifact_sha256_hex",
            &value.token_artifact_sha256_hex,
        ),
        (
            "token_interface_sha256_hex",
            &value.token_interface_sha256_hex,
        ),
        ("token_runtime_hash_hex", &value.token_runtime_hash_hex),
        (
            "verifier_artifact_sha256_hex",
            &value.verifier_artifact_sha256_hex,
        ),
        (
            "verifier_interface_sha256_hex",
            &value.verifier_interface_sha256_hex,
        ),
        (
            "verifier_runtime_hash_hex",
            &value.verifier_runtime_hash_hex,
        ),
        (
            "route_artifact_sha256_hex",
            &value.route_artifact_sha256_hex,
        ),
        (
            "route_interface_sha256_hex",
            &value.route_interface_sha256_hex,
        ),
        ("route_runtime_hash_hex", &value.route_runtime_hash_hex),
    ];
    let mut parsed = Vec::with_capacity(fields.len());
    for (field, digest) in fields {
        parsed.push((field, require_hash(digest, field)?));
    }
    Ok(parsed)
}

fn register_hash_role(
    local: &mut BTreeSet<[u8; 32]>,
    global: &mut BTreeMap<[u8; 32], &'static str>,
    role: &'static str,
    digest: [u8; 32],
) -> Result<(), String> {
    if local.contains(&digest) {
        return Err("proof-system hash roles must be pairwise distinct".to_owned());
    }
    match global.entry(digest) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(role);
        }
        std::collections::btree_map::Entry::Occupied(entry) if *entry.get() != role => {
            return Err("proof-system digest is aliased across profiles and roles".to_owned());
        }
        std::collections::btree_map::Entry::Occupied(_) => {}
    }
    local.insert(digest);
    Ok(())
}

fn register_audit_report(reports: &mut BTreeSet<[u8; 32]>, digest: [u8; 32]) -> Result<(), String> {
    if !reports.insert(digest) {
        return Err("circuit audit reports must be globally distinct".to_owned());
    }
    Ok(())
}

fn validate_sora_finality_anchor_policy(
    policy: &SoraFinalityAnchorPolicyV1,
) -> Result<ValidatedSoraFinalityAnchorPolicyV1, String> {
    if policy.version != 1
        || policy.source_profile != "sora-taira"
        || policy.protocol_version != SUMERAGI_V2_PROTOCOL_VERSION
    {
        return Err("SORA finality anchor must select exact Taira Sumeragi-v2".to_owned());
    }
    let chain_id_hash = require_hash(&policy.chain_id_hash_hex, "SORA finality anchor chain id")?;
    let checkpoint_block_hash = require_hash(
        &policy.checkpoint_block_hash_hex,
        "SORA finality checkpoint block",
    )?;
    let checkpoint_context_id = require_hash(
        &policy.checkpoint_context_id_hex,
        "SORA finality checkpoint context id",
    )?;
    let checkpoint_finality_artifact_hash = require_hash(
        &policy.checkpoint_finality_artifact_hash_hex,
        "SORA finality checkpoint artifact",
    )?;
    let anchor = SccpSoraFinalityAnchorV1 {
        version: policy.version,
        source_network: SccpNetworkV1::SoraTaira,
        protocol_version: policy.protocol_version,
        chain_id_hash,
        checkpoint_height: policy.checkpoint_height,
        checkpoint_block_hash,
        checkpoint_context_id,
        checkpoint_finality_artifact_hash,
    };
    anchor
        .validate()
        .map_err(|_| "SORA finality anchor is invalid or aliases consensus roles".to_owned())?;
    let anchor_hash = sccp_sora_finality_anchor_hash_v1(anchor)
        .map_err(|_| "SORA finality anchor cannot be canonically hashed".to_owned())?;
    Ok(ValidatedSoraFinalityAnchorPolicyV1 {
        anchor,
        anchor_hash,
        chain_id_hash,
        checkpoint_block_hash,
        checkpoint_context_id,
        checkpoint_finality_artifact_hash,
    })
}

struct ValidatedReleaseTrustV1 {
    release_keys: [[u8; 32]; 2],
}

fn validate_release_trust_policy(
    policy: &ReleaseTrustPolicyV1,
    expected_environment: &str,
    signature_set: &mut BTreeSet<[u8; 64]>,
) -> Result<ValidatedReleaseTrustV1, String> {
    let expected_schema = match expected_environment {
        "production" => PRODUCTION_POLICY_SCHEMA,
        "test-fixture" => TEST_POLICY_SCHEMA,
        _ => return Err("release signature mode is invalid".to_owned()),
    };
    if policy.schema != expected_schema
        || policy.environment != expected_environment
        || !canonical_identifier(&policy.policy_id)
        || policy.roles.len() != RELEASE_ROLES.len()
        || policy.destination_attestors.len() != RELEASE_PROFILES.len()
        || policy.circuit_auditors.len() != CIRCUIT_AUDIT_ROLES.len()
        || policy.proof_systems.len() != RELEASE_PROFILES.len()
    {
        return Err("release trust policy has the wrong schema, mode, or cardinality".to_owned());
    }

    let mut identities = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut key_encodings = BTreeSet::new();
    let mut release_keys = [[0_u8; 32]; 2];
    for (index, role) in policy.roles.iter().enumerate() {
        if role.role != RELEASE_ROLES[index]
            || !canonical_identifier(&role.signer_id)
            || key_encodings.contains(&role.signer_id)
            || !identities.insert(role.signer_id.clone())
        {
            return Err("release role identity is invalid or reused".to_owned());
        }
        let key = validate_ed25519_public_key(&role.public_key_hex, "release role key")?;
        if identities.contains(&role.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(role.public_key_hex.clone())
        {
            return Err("release trust-policy key is reused".to_owned());
        }
        release_keys[index] = key;
    }

    for (index, attestor) in policy.destination_attestors.iter().enumerate() {
        if attestor.counterparty_profile != RELEASE_PROFILES[index]
            || !canonical_identifier(&attestor.attestor_id)
            || key_encodings.contains(&attestor.attestor_id)
            || !identities.insert(attestor.attestor_id.clone())
        {
            return Err("destination attestor profile or identity is invalid".to_owned());
        }
        let key =
            validate_ed25519_public_key(&attestor.public_key_hex, "destination attestor key")?;
        if identities.contains(&attestor.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(attestor.public_key_hex.clone())
        {
            return Err("destination attestor key is reused".to_owned());
        }
    }

    let mut audit_keys = [[0_u8; 32]; 2];
    for (index, auditor) in policy.circuit_auditors.iter().enumerate() {
        if auditor.role != CIRCUIT_AUDIT_ROLES[index]
            || !canonical_identifier(&auditor.auditor_id)
            || key_encodings.contains(&auditor.auditor_id)
            || !identities.insert(auditor.auditor_id.clone())
        {
            return Err("circuit auditor role or identity is invalid".to_owned());
        }
        let key = validate_ed25519_public_key(&auditor.public_key_hex, "circuit auditor key")?;
        if identities.contains(&auditor.public_key_hex)
            || !keys.insert(key)
            || !key_encodings.insert(auditor.public_key_hex.clone())
        {
            return Err("circuit auditor key is reused".to_owned());
        }
        audit_keys[index] = key;
    }

    if expected_environment == "production"
        && (identities
            .iter()
            .any(|identity| identity.starts_with("fixture-"))
            || FORBIDDEN_FIXTURE_PUBLIC_KEYS
                .iter()
                .any(|key| key_encodings.contains(*key)))
    {
        return Err("production trust policy contains a fixture-only identity or key".to_owned());
    }

    let mut global_hash_roles = BTreeMap::new();
    let mut audit_report_hashes = BTreeSet::new();
    for (profile_index, proof) in policy.proof_systems.iter().enumerate() {
        if proof.counterparty_profile != RELEASE_PROFILES[profile_index]
            || proof.circuit_id != RELEASE_CIRCUIT_IDS[profile_index]
            || !canonical_identifier(&proof.circuit_id)
            || proof.circuit_id.contains("smoke")
            || proof.circuit_id.contains("test")
            || proof.circuit_id.contains("signal-binding")
            || proof.circuit_id.contains("labeled-signal")
            || !proof
                .semantics
                .iter()
                .map(String::as_str)
                .eq(REQUIRED_SEMANTICS)
            || proof.audit_attestations.len() != CIRCUIT_AUDIT_ROLES.len()
        {
            return Err("semantic proof-system policy is invalid".to_owned());
        }
        let circuit_artifact = require_hash(
            &proof.circuit_artifact_sha256_hex,
            "circuit artifact digest",
        )?;
        if circuit_artifact == FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256 {
            return Err("labeled-signal-only circuit is forbidden in release policy".to_owned());
        }
        let witness_generator = require_hash(
            &proof.witness_generator_sha256_hex,
            "witness generator digest",
        )?;
        let public_signal_schema = require_hash(
            &proof.public_signal_schema_hash_hex,
            "public signal schema hash",
        )?;
        if public_signal_schema != sccp_groth16_bn254_public_signal_schema_hash_v1() {
            return Err("proof policy uses a different public-signal schema".to_owned());
        }
        let semantic_profile = SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
            SccpGroth16Bn254SemanticCircuitV1 {
                version: 1,
                circuit_commitment: circuit_artifact,
                witness_generator_commitment: witness_generator,
                public_signal_schema_hash: public_signal_schema,
            },
        );
        let semantic_profile_hash = sccp_semantic_proof_profile_hash_v1(semantic_profile)
            .map_err(|_| "semantic proof profile is invalid".to_owned())?;
        if require_hash(
            &proof.semantic_proof_profile_hash_hex,
            "semantic proof profile hash",
        )? != semantic_profile_hash
        {
            return Err("semantic proof profile hash does not match its commitments".to_owned());
        }
        let finality_anchor = validate_sora_finality_anchor_policy(&proof.sora_finality_anchor)?;
        if require_hash(
            &proof.sora_finality_anchor_hash_hex,
            "SORA finality anchor hash",
        )? != finality_anchor.anchor_hash
        {
            return Err("SORA finality anchor hash does not match its checkpoint".to_owned());
        }
        let verifier_key = require_hash(&proof.verifier_key_hash_hex, "verifier key hash")?;
        if proof.route_revision == 0 {
            return Err("proof-system route revision must be nonzero".to_owned());
        }
        let verifying_key =
            require_hash(&proof.verifying_key_sha256_hex, "full verifying-key digest")?;
        if verifier_key == FORBIDDEN_ALGEBRAIC_SMOKE_VK {
            return Err("algebraic smoke-test verifier key is forbidden".to_owned());
        }
        let prover_build = require_hash(&proof.prover_build_sha256_hex, "prover build digest")?;
        let toolchain_lock =
            require_hash(&proof.toolchain_lock_sha256_hex, "toolchain lock digest")?;
        let destination_roles = destination_build_hash_roles(&proof.destination_build)?;
        let mut local_hash_roles = BTreeSet::new();
        for (role, digest) in [
            ("circuit_artifact_sha256_hex", circuit_artifact),
            ("witness_generator_sha256_hex", witness_generator),
            ("public_signal_schema_hash_hex", public_signal_schema),
            ("semantic_proof_profile_hash_hex", semantic_profile_hash),
            ("sora_finality_anchor_hash_hex", finality_anchor.anchor_hash),
            ("anchor_chain_id_hash_hex", finality_anchor.chain_id_hash),
            (
                "anchor_checkpoint_block_hash_hex",
                finality_anchor.checkpoint_block_hash,
            ),
            (
                "anchor_checkpoint_context_id_hex",
                finality_anchor.checkpoint_context_id,
            ),
            (
                "anchor_checkpoint_finality_artifact_hash_hex",
                finality_anchor.checkpoint_finality_artifact_hash,
            ),
            ("verifier_key_hash_hex", verifier_key),
            ("verifying_key_sha256_hex", verifying_key),
            ("prover_build_sha256_hex", prover_build),
            ("toolchain_lock_sha256_hex", toolchain_lock),
        ] {
            register_hash_role(&mut local_hash_roles, &mut global_hash_roles, role, digest)?;
        }
        for (role, digest) in destination_roles {
            register_hash_role(&mut local_hash_roles, &mut global_hash_roles, role, digest)?;
        }

        let unsigned = value_without_field(
            norito::json::to_value(proof)
                .map_err(|_| "proof policy cannot be encoded".to_owned())?,
            "audit_attestations",
            "proof-system policy",
        )?;
        let unsigned_json = norito::json::to_json(&unsigned)
            .map_err(|_| "proof policy cannot be canonically encoded".to_owned())?;
        for (audit_index, audit) in proof.audit_attestations.iter().enumerate() {
            let trusted = &policy.circuit_auditors[audit_index];
            if audit.role != CIRCUIT_AUDIT_ROLES[audit_index]
                || audit.auditor_id != trusted.auditor_id
                || audit.public_key_hex != trusted.public_key_hex
                || audit.algorithm != "ed25519"
            {
                return Err("circuit audit does not match its trusted role".to_owned());
            }
            let report_hash = require_hash(&audit.report_sha256_hex, "circuit audit report")?;
            register_audit_report(&mut audit_report_hashes, report_hash)?;
            register_hash_role(
                &mut local_hash_roles,
                &mut global_hash_roles,
                "audit_report_sha256_hex",
                report_hash,
            )?;
            let signature = decode_signature_base64(&audit.signature_b64)?;
            if !signature_set.insert(signature) {
                return Err("detached signature is replayed across trust roles".to_owned());
            }
            let mut payload = Vec::with_capacity(
                CIRCUIT_AUDIT_DOMAIN.len() + unsigned_json.len() + report_hash.len(),
            );
            payload.extend_from_slice(CIRCUIT_AUDIT_DOMAIN);
            payload.extend_from_slice(unsigned_json.as_bytes());
            payload.extend_from_slice(&report_hash);
            verify_ed25519_signature(&audit_keys[audit_index], &signature, &payload)?;
        }
    }
    Ok(ValidatedReleaseTrustV1 { release_keys })
}

fn release_artifact_limit(kind: &str) -> Option<u64> {
    match kind {
        "phase-transcript" => Some(MAX_TRANSCRIPT_BYTES),
        "lane-evidence" => Some(MAX_LANE_INPUT_BYTES),
        "circuit-audit-report" => Some(MAX_AUDIT_REPORT_BYTES),
        "honest-proof" => u64::try_from(SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).ok(),
        "semantic-circuit" | "witness-generator" | "verifying-key" | "prover-build"
        | "toolchain-lock" | "honest-witness" => Some(MAX_SEMANTIC_ARTIFACT_BYTES),
        _ => None,
    }
}

fn semantic_artifact_role(kind: &str) -> Option<(&'static str, &'static str)> {
    SEMANTIC_ARTIFACT_ROLES
        .iter()
        .find_map(|(role, expected_kind, filename)| {
            (*expected_kind == kind).then_some((*role, *filename))
        })
}

fn semantic_policy_digest<'a>(proof: &'a ProofSystemPolicyV1, role: &str) -> Option<&'a str> {
    match role {
        "circuit-artifact" => Some(&proof.circuit_artifact_sha256_hex),
        "witness-generator" => Some(&proof.witness_generator_sha256_hex),
        "verifying-key" => Some(&proof.verifying_key_sha256_hex),
        "prover-build" => Some(&proof.prover_build_sha256_hex),
        "toolchain-lock" => Some(&proof.toolchain_lock_sha256_hex),
        "honest-witness" | "honest-proof" => None,
        _ => None,
    }
}

fn semantic_artifact_path(role: &str, digest: &str, filename: &str) -> String {
    format!("artifacts/semantic/{role}/{digest}-{filename}")
}

fn circuit_audit_report_path(profile: &str, role: &str) -> String {
    format!("artifacts/semantic/audits/{profile}-{role}.json")
}

fn validate_release_semantic_inventory<'a>(
    policy: &ReleaseTrustPolicyV1,
    artifact_by_path: &BTreeMap<&'a str, &'a ReleaseArtifactV1>,
    referenced: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for artifact in artifact_by_path.values() {
        if artifact.kind == "circuit-audit-report"
            || semantic_artifact_role(&artifact.kind).is_some()
        {
            *counts.entry(artifact.kind.as_str()).or_default() += 1;
        }
    }
    if counts.get("circuit-audit-report").copied() != Some(6) {
        return Err(
            "production evidence requires exactly two circuit audit reports per profile".to_owned(),
        );
    }
    for (_, kind, _) in SEMANTIC_ARTIFACT_ROLES {
        if !(1..=RELEASE_PROFILES.len()).contains(&counts.get(kind).copied().unwrap_or(0)) {
            return Err(format!(
                "production evidence has an invalid {kind} artifact cardinality"
            ));
        }
    }

    for proof in &policy.proof_systems {
        for (role, kind, filename) in SEMANTIC_ARTIFACT_ROLES {
            let Some(digest) = semantic_policy_digest(proof, role) else {
                continue;
            };
            let path = semantic_artifact_path(role, digest, filename);
            let artifact = artifact_by_path.get(path.as_str()).ok_or_else(|| {
                format!(
                    "production {} {role} artifact is absent",
                    proof.counterparty_profile
                )
            })?;
            if artifact.kind != kind || artifact.sha256_hex != digest {
                return Err("production policy-bound semantic artifact is substituted".to_owned());
            }
            referenced.insert(artifact.path.as_str());
        }
        for (index, role) in CIRCUIT_AUDIT_ROLES.iter().enumerate() {
            let path = circuit_audit_report_path(&proof.counterparty_profile, role);
            let artifact = artifact_by_path
                .get(path.as_str())
                .ok_or_else(|| "production circuit audit report is absent".to_owned())?;
            if artifact.kind != "circuit-audit-report"
                || artifact.sha256_hex != proof.audit_attestations[index].report_sha256_hex
            {
                return Err("production circuit audit report is substituted".to_owned());
            }
            referenced.insert(artifact.path.as_str());
        }
    }

    for artifact in artifact_by_path.values() {
        if let Some((role, filename)) = semantic_artifact_role(&artifact.kind) {
            let expected = semantic_artifact_path(role, &artifact.sha256_hex, filename);
            if artifact.path != expected {
                return Err("semantic artifact path must be exact and content-addressed".to_owned());
            }
            referenced.insert(artifact.path.as_str());
        } else if artifact.kind == "circuit-audit-report"
            && !referenced.contains(artifact.path.as_str())
        {
            return Err(
                "production evidence contains an untrusted circuit audit report".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_release_evidence_envelope(
    evidence: &ReleaseEvidenceSignaturesV1,
    policy: &ReleaseTrustPolicyV1,
    expected_environment: &str,
) -> Result<(), String> {
    if evidence.created_at_unix_ms == 0
        || evidence.validation.corridor != "sccp-production-corridor-v1"
        || evidence.validation.phases.len() != REQUIRED_PHASES.len()
        || !(RELEASE_PROFILES.len() + REQUIRED_PHASES.len()..=MAX_RELEASE_ARTIFACTS)
            .contains(&evidence.artifacts.len())
    {
        return Err("release evidence inventory or corridor is not exact".to_owned());
    }

    validate_validator_identity_shape(&evidence.validator)?;
    let expected_validator = validator_identity()?;
    if evidence.validator != expected_validator {
        return Err("release evidence selects a different Rust validator build".to_owned());
    }

    let mut artifact_by_path = BTreeMap::new();
    let mut artifact_hashes = BTreeSet::new();
    let mut previous_path: Option<&str> = None;
    let mut total_size = 0_u64;
    for artifact in &evidence.artifacts {
        if !canonical_relative_path(&artifact.path)
            || previous_path.is_some_and(|previous| artifact.path.as_str() <= previous)
        {
            return Err("release artifact paths are unsafe, duplicated, or unsorted".to_owned());
        }
        previous_path = Some(&artifact.path);
        let maximum = release_artifact_limit(&artifact.kind)
            .ok_or_else(|| "release artifact kind is not part of SCCP V1".to_owned())?;
        if artifact.size_bytes == 0 || artifact.size_bytes > maximum {
            return Err("release artifact size is outside its kind-specific bound".to_owned());
        }
        total_size = total_size
            .checked_add(artifact.size_bytes)
            .ok_or_else(|| "release artifact total size overflowed".to_owned())?;
        if total_size > MAX_TOTAL_ARTIFACT_BYTES {
            return Err("release artifact total exceeds the SCCP V1 bound".to_owned());
        }
        let digest = require_hash(&artifact.sha256_hex, "release artifact digest")?;
        if !artifact_hashes.insert(digest)
            || artifact_by_path
                .insert(artifact.path.as_str(), artifact)
                .is_some()
        {
            return Err("release artifact paths and digests must be distinct".to_owned());
        }
    }

    let mut referenced = BTreeSet::new();
    for (index, phase) in evidence.validation.phases.iter().enumerate() {
        if phase.name != REQUIRED_PHASES[index]
            || phase.status != "passed"
            || !canonical_relative_path(&phase.artifact_path)
        {
            return Err("release validation phases are not exact, ordered passes".to_owned());
        }
        let artifact = artifact_by_path
            .get(phase.artifact_path.as_str())
            .ok_or_else(|| "release validation phase references no artifact".to_owned())?;
        if artifact.kind != "phase-transcript" || !referenced.insert(phase.artifact_path.as_str()) {
            return Err("release validation phases must reference distinct transcripts".to_owned());
        }
    }

    for (index, lane) in evidence.lanes.iter().enumerate() {
        if lane.counterparty_profile != RELEASE_PROFILES[index]
            || lane.counterparty_domain != RELEASE_DOMAINS[index]
            || !matches!(lane.inbound_status.as_str(), "verified" | "unavailable")
            || !matches!(lane.outbound_status.as_str(), "verified" | "unavailable")
            || !canonical_relative_path(&lane.evidence_artifact_path)
        {
            return Err("release evidence lane matrix is not exact".to_owned());
        }
        let artifact = artifact_by_path
            .get(lane.evidence_artifact_path.as_str())
            .ok_or_else(|| "release lane references no artifact".to_owned())?;
        if artifact.kind != "lane-evidence"
            || !referenced.insert(lane.evidence_artifact_path.as_str())
        {
            return Err("release lanes must reference distinct typed evidence".to_owned());
        }
    }
    if expected_environment == "production" {
        validate_release_semantic_inventory(policy, &artifact_by_path, &mut referenced)?;
    }
    if referenced.len() != artifact_by_path.len() {
        return Err("release evidence contains an unreferenced artifact".to_owned());
    }
    Ok(())
}

fn validate_release_hub(evidence: &ReleaseEvidenceSignaturesV1) -> Result<(), String> {
    if evidence.hub_profile != "sora-taira"
        || evidence.hub_chain_id != SCCP_TAIRA_FINALITY_CHAIN_ID_V1
    {
        return Err("release evidence must identify exact SORA Taira V1".to_owned());
    }
    Ok(())
}

fn validate_release_context(
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<
    (
        ReleaseTrustPolicyV1,
        ReleaseEvidenceSignaturesV1,
        ReleaseSignatureValidationV1,
    ),
    String,
> {
    let policy_value = parse_canonical_sorted_json(policy_bytes, "release trust policy")?;
    let evidence_value = parse_canonical_sorted_json(evidence_bytes, "release evidence")?;
    let policy_text =
        std::str::from_utf8(policy_bytes).map_err(|_| "release trust policy is not UTF-8")?;
    let evidence_text =
        std::str::from_utf8(evidence_bytes).map_err(|_| "release evidence is not UTF-8")?;
    let policy = norito::json::from_str::<ReleaseTrustPolicyV1>(policy_text)
        .map_err(|_| "release trust policy does not match its typed schema".to_owned())?;
    let evidence = norito::json::from_str::<ReleaseEvidenceSignaturesV1>(evidence_text)
        .map_err(|_| "release evidence does not match its typed signature schema".to_owned())?;
    if norito::json::to_value(&policy).ok().as_ref() != Some(&policy_value)
        || norito::json::to_value(&evidence).ok().as_ref() != Some(&evidence_value)
    {
        return Err("release policy or evidence has unknown or mistyped fields".to_owned());
    }

    let mut signatures = BTreeSet::new();
    let trust = validate_release_trust_policy(&policy, expected_environment, &mut signatures)?;
    if evidence.schema != RELEASE_EVIDENCE_SCHEMA
        || evidence.protocol_version != 1
        || !canonical_identifier(&evidence.release_id)
        || evidence.trust_policy_id != policy.policy_id
        || require_hash(
            &evidence.trust_policy_sha256_hex,
            "release evidence trust policy hash",
        )? != sha256(policy_bytes)
        || evidence.lanes.len() != RELEASE_PROFILES.len()
        || evidence.provenance.len() != RELEASE_ROLES.len()
    {
        return Err("release evidence signature envelope is invalid".to_owned());
    }
    validate_release_hub(&evidence)?;
    validate_release_evidence_envelope(&evidence, &policy, expected_environment)?;
    if expected_environment == "production" && evidence.validator.build_profile != "release" {
        return Err("production evidence requires a release-profile validator build".to_owned());
    }

    let unsigned = value_without_field(evidence_value, "provenance", "release evidence")?;
    let unsigned_json = norito::json::to_json(&unsigned)
        .map_err(|_| "release evidence signing payload cannot be encoded".to_owned())?;
    let mut payload = Vec::with_capacity(RELEASE_SIGNING_DOMAIN.len() + unsigned_json.len());
    payload.extend_from_slice(RELEASE_SIGNING_DOMAIN);
    payload.extend_from_slice(unsigned_json.as_bytes());
    for (index, provenance) in evidence.provenance.iter().enumerate() {
        let trusted = &policy.roles[index];
        if provenance.role != RELEASE_ROLES[index]
            || provenance.signer_id != trusted.signer_id
            || provenance.public_key_hex != trusted.public_key_hex
            || provenance.algorithm != "ed25519"
        {
            return Err("release provenance does not match its trusted role".to_owned());
        }
        let signature = decode_signature_base64(&provenance.signature_b64)?;
        if !signatures.insert(signature) {
            return Err("release signature is replayed across trust roles".to_owned());
        }
        verify_ed25519_signature(&trust.release_keys[index], &signature, &payload)?;
    }

    let receipt = ReleaseSignatureValidationV1 {
        schema: RELEASE_SIGNATURE_OUTPUT_SCHEMA.to_owned(),
        environment: expected_environment.to_owned(),
        policy_id: policy.policy_id.clone(),
        release_id: evidence.release_id.clone(),
        policy_sha256_hex: lowercase_hex(&sha256(policy_bytes)),
        evidence_sha256_hex: lowercase_hex(&sha256(evidence_bytes)),
        release_signatures_verified: 2,
        circuit_audit_signatures_verified: 6,
        destination_attestors_validated: 3,
        distinct_trust_identities: 7,
    };
    Ok((policy, evidence, receipt))
}

fn validate_release_signatures(
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<ReleaseSignatureValidationV1, String> {
    validate_release_context(policy_bytes, evidence_bytes, expected_environment)
        .map(|(_, _, receipt)| receipt)
}

fn semantic_proof_claim(
    proof_bytes: &[u8],
    expected_profile: &str,
    policy: &ProofSystemPolicyV1,
) -> Result<SemanticProofClaimV1, String> {
    if policy.counterparty_profile != expected_profile {
        return Err("semantic proof policy selects a different profile".to_owned());
    }
    let target_network = SccpNetworkV1::from_profile_key(expected_profile)
        .filter(|network| release_profile_supported(*network))
        .ok_or_else(|| "semantic proof profile is not in the production launch set".to_owned())?;
    let expected_backend = if target_network == SccpNetworkV1::TronMainnet {
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
    } else {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254
    };
    let artifact =
        decode_canonical_sccp_groth16_bn254_proof_artifact_v1(proof_bytes).ok_or_else(|| {
            "honest proof is not one canonical, pairing-valid SCCP Groth16 artifact".to_owned()
        })?;
    if artifact.request.source_network != SccpNetworkV1::SoraTaira
        || artifact.request.target_network != target_network
        || artifact.request.backend != expected_backend
        || artifact.request.public_inputs.target_domain != target_network.domain_id()
    {
        return Err("honest proof selects the wrong source, target, or backend".to_owned());
    }

    let circuit_artifact = require_hash(
        &policy.circuit_artifact_sha256_hex,
        "semantic circuit artifact digest",
    )?;
    let witness_generator = require_hash(
        &policy.witness_generator_sha256_hex,
        "semantic witness generator digest",
    )?;
    let public_signal_schema = require_hash(
        &policy.public_signal_schema_hash_hex,
        "semantic public signal schema hash",
    )?;
    let expected_semantic_profile =
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
            SccpGroth16Bn254SemanticCircuitV1 {
                version: 1,
                circuit_commitment: circuit_artifact,
                witness_generator_commitment: witness_generator,
                public_signal_schema_hash: public_signal_schema,
            },
        );
    let expected_semantic_hash = sccp_semantic_proof_profile_hash_v1(expected_semantic_profile)
        .map_err(|_| "semantic proof policy commitments are invalid".to_owned())?;
    if expected_semantic_hash
        != require_hash(
            &policy.semantic_proof_profile_hash_hex,
            "semantic proof profile hash",
        )?
        || artifact.request.semantic_proof_profile != expected_semantic_profile
        || artifact.request.semantic_proof_profile_hash != expected_semantic_hash
    {
        return Err("honest proof does not bind the audited semantic circuit".to_owned());
    }

    let expected_anchor = validate_sora_finality_anchor_policy(&policy.sora_finality_anchor)?;
    if expected_anchor.anchor_hash
        != require_hash(
            &policy.sora_finality_anchor_hash_hex,
            "SORA finality anchor hash",
        )?
        || artifact.request.sora_finality_anchor != expected_anchor.anchor
        || artifact.request.sora_finality_anchor_hash != expected_anchor.anchor_hash
    {
        return Err("honest proof does not bind the governed SORA finality anchor".to_owned());
    }

    let expected_verifier_key_hash =
        require_hash(&policy.verifier_key_hash_hex, "verifier key hash")?;
    let verifying_key_bytes =
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&artifact.request.verifying_key)
            .ok_or_else(|| "honest proof verification key is not canonical".to_owned())?;
    if artifact.request.verifier_key_hash != expected_verifier_key_hash
        || sccp_groth16_bn254_verifying_key_hash_v1(&artifact.request.verifying_key)
            != Some(expected_verifier_key_hash)
        || sha256(&verifying_key_bytes)
            != require_hash(
                &policy.verifying_key_sha256_hex,
                "full verifying-key digest",
            )?
    {
        return Err("honest proof substitutes the audited verification key".to_owned());
    }

    let bundle = decode_canonical_taira_sccp_message_bundle_v1(&artifact.request.bundle_bytes)
        .ok_or_else(|| "honest proof embeds a non-canonical SCCP bundle".to_owned())?;
    let SccpPayloadV1::Transfer(transfer) = bundle.payload;
    if transfer.route_revision != policy.route_revision {
        return Err("honest proof selects the wrong governed route revision".to_owned());
    }
    let public_signal_words = sccp_groth16_bn254_public_signal_words(
        &artifact.request.public_inputs,
        artifact.request.source_network.domain_id(),
        artifact.request.statement_hash,
        artifact.request.destination_binding_hash,
        artifact.request.route_configuration_hash,
        artifact.request.sora_finality_anchor_hash,
    );
    Ok(SemanticProofClaimV1 {
        source_profile: "sora-taira".to_owned(),
        target_profile: expected_profile.to_owned(),
        target_domain: target_network.domain_id(),
        route_revision: transfer.route_revision,
        message_id_hex: lowercase_hex(&artifact.request.public_inputs.message_id),
        payload_hash_hex: lowercase_hex(&artifact.request.public_inputs.payload_hash),
        commitment_root_hex: lowercase_hex(&artifact.request.public_inputs.commitment_root),
        finality_height: artifact.request.public_inputs.finality_height.to_string(),
        finality_block_hash_hex: lowercase_hex(&artifact.request.public_inputs.finality_block_hash),
        destination_binding_hash_hex: lowercase_hex(&artifact.request.destination_binding_hash),
        route_configuration_hash_hex: lowercase_hex(&artifact.request.route_configuration_hash),
        statement_hash_hex: lowercase_hex(&artifact.request.statement_hash),
        request_hash_hex: lowercase_hex(&artifact.request.request_hash),
        result_hash_hex: lowercase_hex(&artifact.result.result_hash),
        verifier_key_hash_hex: lowercase_hex(&artifact.request.verifier_key_hash),
        semantic_proof_profile_hash_hex: lowercase_hex(
            &artifact.request.semantic_proof_profile_hash,
        ),
        sora_finality_anchor_hash_hex: lowercase_hex(&artifact.request.sora_finality_anchor_hash),
        public_signal_words_hex: public_signal_words
            .iter()
            .map(|word| lowercase_hex(word))
            .collect(),
    })
}

fn validate_semantic_proof_in_release_context(
    proof_bytes: &[u8],
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_profile: &str,
    expected_environment: &str,
) -> Result<SemanticProofValidationV1, String> {
    if expected_environment != "production" {
        return Err(
            "semantic proof validation is available only for production evidence".to_owned(),
        );
    }
    let (policy, evidence, _) =
        validate_release_context(policy_bytes, evidence_bytes, expected_environment)?;
    let profile_index = RELEASE_PROFILES
        .iter()
        .position(|profile| *profile == expected_profile)
        .ok_or_else(|| "semantic proof profile is not supported".to_owned())?;
    let claim = semantic_proof_claim(
        proof_bytes,
        expected_profile,
        &policy.proof_systems[profile_index],
    )?;
    let proof_digest = lowercase_hex(&sha256(proof_bytes));
    let proof_path = semantic_artifact_path("honest-proof", &proof_digest, "honest-proof.norito");
    let metadata = evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.path == proof_path)
        .ok_or_else(|| {
            "honest proof bytes are not present in signed release evidence".to_owned()
        })?;
    if metadata.kind != "honest-proof"
        || metadata.sha256_hex != proof_digest
        || metadata.size_bytes != proof_bytes.len() as u64
    {
        return Err("honest proof bytes do not match signed artifact metadata".to_owned());
    }
    Ok(SemanticProofValidationV1 {
        schema: SEMANTIC_PROOF_OUTPUT_SCHEMA.to_owned(),
        environment: expected_environment.to_owned(),
        policy_id: policy.policy_id,
        release_id: evidence.release_id,
        policy_sha256_hex: lowercase_hex(&sha256(policy_bytes)),
        evidence_sha256_hex: lowercase_hex(&sha256(evidence_bytes)),
        proof_artifact_path: proof_path,
        proof_artifact_sha256_hex: proof_digest,
        canonical_norito_verified: true,
        pairing_verified: true,
        claim,
    })
}

fn value_without_field(
    value: norito::json::Value,
    field: &str,
    label: &str,
) -> Result<norito::json::Value, String> {
    let norito::json::Value::Object(mut object) = value else {
        return Err(format!("{label} must be an object"));
    };
    if object.remove(field).is_none() {
        return Err(format!("{label} is missing {field}"));
    }
    Ok(norito::json::Value::Object(object))
}

fn decode_runtime_code(value: &str, label: &str) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || value.len() > MAX_RUNTIME_CODE_BYTES * 2
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be bounded lowercase runtime-code hex"
        ));
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("alphabet checked above"),
        };
        decoded.push((digit(pair[0]) << 4) | digit(pair[1]));
    }
    Ok(decoded)
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut hash);
    hash
}

#[derive(Debug)]
struct ValidatedDestinationStateV1 {
    observed_at_unix_ms: u64,
    finality_height: u64,
    finality_block_hash: [u8; 32],
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
    verifier_key_hash: [u8; 32],
    semantic_proof_profile_hash: [u8; 32],
    sora_finality_anchor_hash: [u8; 32],
    route_revision: u32,
    verifying_key_sha256: [u8; 32],
    token_runtime_hash: [u8; 32],
    verifier_runtime_hash: [u8; 32],
    route_runtime_hash: [u8; 32],
}

#[derive(Debug)]
struct EvmDestinationReadback<'a> {
    deployment: SccpEvmDestinationDeploymentV1,
    token_bridge_address: [u8; 20],
    route_token_address: [u8; 20],
    route_verifier_address: [u8; 20],
    token_runtime_code_hex: &'a str,
    verifier_runtime_code_hex: &'a str,
    route_runtime_code_hex: &'a str,
    verifier_key_hash: [u8; 32],
    semantic_proof_profile_hash: [u8; 32],
    sora_finality_anchor_hash: [u8; 32],
    route_revision: u32,
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    destination_binding_hash: [u8; 32],
    route_configuration_hash: [u8; 32],
    governed_route_configuration_hash: [u8; 32],
    observed_at_unix_ms: u64,
    finality_height: u64,
    finality_block_hash: [u8; 32],
}

fn validate_evm_destination_state(
    expected_profile: SccpNetworkV1,
    state: &EvmDestinationStateV1,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_chain_id = match expected_profile {
        SccpNetworkV1::EthereumMainnet => 1,
        SccpNetworkV1::BscMainnet => 56,
        _ => return Err("EVM destination attestation uses the wrong profile".to_owned()),
    };
    if state.schema != "sccp-evm-destination-state-v1"
        || state.profile != expected_profile
        || state.rpc_chain_id != expected_chain_id
        || state.network_identity_hash != sccp_network_identity_hash_v1(expected_profile)
        || state.observed_at_unix_ms == 0
        || state.finalized_block_height == 0
        || state.finalized_block_hash.iter().all(|byte| *byte == 0)
    {
        return Err("EVM destination chain identity/finality is not canonical".to_owned());
    }
    let governed_route = &state.governed_route;
    governed_route
        .validate()
        .map_err(|error| format!("governed EVM route is invalid: {error}"))?;
    if governed_route.lane_id.source != expected_profile
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || !governed_route.activation.allows_outbound()
    {
        return Err("governed EVM route is not outbound-active for this profile".to_owned());
    }
    let SccpDestinationDeploymentV1::Evm(deployment) = governed_route.destination else {
        return Err("governed EVM route has a different destination family".to_owned());
    };
    validate_destination_readback(
        governed_route,
        EvmDestinationReadback {
            deployment,
            token_bridge_address: state.token_bridge_address,
            route_token_address: state.route_token_address,
            route_verifier_address: state.route_verifier_address,
            token_runtime_code_hex: &state.token_runtime_code_hex,
            verifier_runtime_code_hex: &state.verifier_runtime_code_hex,
            route_runtime_code_hex: &state.route_runtime_code_hex,
            verifier_key_hash: state.verifier_key_hash,
            semantic_proof_profile_hash: state.semantic_proof_profile_hash,
            sora_finality_anchor_hash: state.sora_finality_anchor_hash,
            route_revision: state.route_revision,
            verifying_key: state.verifying_key,
            destination_binding_hash: state.destination_binding_hash,
            route_configuration_hash: state.route_configuration_hash,
            governed_route_configuration_hash: state.governed_route_configuration_hash,
            observed_at_unix_ms: state.observed_at_unix_ms,
            finality_height: state.finalized_block_height,
            finality_block_hash: state.finalized_block_hash,
        },
    )
}

fn validate_destination_readback(
    route: &SccpGovernedRouteV1,
    readback: EvmDestinationReadback<'_>,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_binding = route
        .destination_binding_hash()
        .map_err(|error| format!("destination binding derivation failed: {error}"))?;
    let expected_route_configuration = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .map_err(|error| format!("route configuration derivation failed: {error}"))?;
    let expected_governed_configuration = route
        .route_configuration_hash()
        .map_err(|error| format!("governed route derivation failed: {error}"))?;
    let token_code_hash = keccak256(&decode_runtime_code(
        readback.token_runtime_code_hex,
        "token runtime code",
    )?);
    let verifier_code_hash = keccak256(&decode_runtime_code(
        readback.verifier_runtime_code_hex,
        "verifier runtime code",
    )?);
    let route_code_hash = keccak256(&decode_runtime_code(
        readback.route_runtime_code_hex,
        "route runtime code",
    )?);
    let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
        &readback.verifying_key,
    )
    .ok_or_else(|| "authenticated EVM verifying key is not a canonical subgroup key".to_owned())?;
    let verifying_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(&readback.verifying_key)
        .ok_or_else(|| "authenticated EVM verifying key cannot be hashed".to_owned())?;
    let expected_semantic_profile_hash = readback
        .deployment
        .outbound_proof_policy
        .semantic_profile_hash()
        .map_err(|_| "governed EVM semantic proof profile is invalid".to_owned())?;
    let expected_finality_anchor_hash = readback
        .deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()
        .map_err(|_| "governed EVM finality anchor is invalid".to_owned())?;
    if readback.token_bridge_address != readback.deployment.route_address
        || readback.route_token_address != readback.deployment.token_address
        || readback.route_verifier_address != readback.deployment.verifier_address
        || token_code_hash != readback.deployment.token_code_hash
        || verifier_code_hash != readback.deployment.verifier_code_hash
        || route_code_hash != readback.deployment.route_code_hash
        || readback.verifier_key_hash != readback.deployment.verifier_key_hash
        || readback.semantic_proof_profile_hash != expected_semantic_profile_hash
        || readback.sora_finality_anchor_hash != expected_finality_anchor_hash
        || readback.verifying_key != readback.deployment.verifying_key
        || verifying_key_hash != readback.verifier_key_hash
        || readback.route_revision == 0
        || readback.route_revision != route.revision
        || readback.destination_binding_hash != expected_binding
        || readback.route_configuration_hash != expected_route_configuration
        || readback.governed_route_configuration_hash != expected_governed_configuration
    {
        return Err("authenticated EVM destination state differs from governed route".to_owned());
    }
    Ok(ValidatedDestinationStateV1 {
        observed_at_unix_ms: readback.observed_at_unix_ms,
        finality_height: readback.finality_height,
        finality_block_hash: readback.finality_block_hash,
        destination_binding_hash: readback.destination_binding_hash,
        route_configuration_hash: readback.route_configuration_hash,
        governed_route_configuration_hash: readback.governed_route_configuration_hash,
        verifier_key_hash: readback.verifier_key_hash,
        semantic_proof_profile_hash: readback.semantic_proof_profile_hash,
        sora_finality_anchor_hash: readback.sora_finality_anchor_hash,
        route_revision: readback.route_revision,
        verifying_key_sha256: sha256(&verifying_key_bytes),
        token_runtime_hash: token_code_hash,
        verifier_runtime_hash: verifier_code_hash,
        route_runtime_hash: route_code_hash,
    })
}

fn validate_tron_destination_state(
    expected_profile: SccpNetworkV1,
    state: &TronDestinationStateV1,
) -> Result<ValidatedDestinationStateV1, String> {
    if expected_profile != SccpNetworkV1::TronMainnet
        || state.schema != "sccp-tron-destination-state-v1"
        || state.profile != expected_profile
        || state.network_magic != 0x2b66_53dc
        || state.network_identity_hash != sccp_network_identity_hash_v1(expected_profile)
        || state.observed_at_unix_ms == 0
        || state.solid_block_height == 0
        || state.solid_block_hash.iter().all(|byte| *byte == 0)
    {
        return Err("TRON destination chain identity/finality is not canonical".to_owned());
    }
    let governed_route = &state.governed_route;
    governed_route
        .validate()
        .map_err(|error| format!("governed TRON route is invalid: {error}"))?;
    if governed_route.lane_id.source != expected_profile
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || !governed_route.activation.allows_outbound()
    {
        return Err("governed TRON route is not outbound-active for this profile".to_owned());
    }
    let SccpDestinationDeploymentV1::Tron(deployment) = governed_route.destination else {
        return Err("governed TRON route has a different destination family".to_owned());
    };
    validate_tron_destination_readback(state, governed_route, deployment)
}

fn validate_tron_destination_readback(
    state: &TronDestinationStateV1,
    route: &SccpGovernedRouteV1,
    deployment: SccpTronDestinationDeploymentV1,
) -> Result<ValidatedDestinationStateV1, String> {
    let expected_binding = route
        .destination_binding_hash()
        .map_err(|error| format!("destination binding derivation failed: {error}"))?;
    let expected_route_configuration = route
        .destination
        .route_configuration_hash(
            route.lane_id,
            &route.route_id,
            &route.asset_key,
            route.revision,
            route.settlement.payload_amount_scale,
        )
        .map_err(|error| format!("route configuration derivation failed: {error}"))?;
    let expected_governed_configuration = route
        .route_configuration_hash()
        .map_err(|error| format!("governed route derivation failed: {error}"))?;
    let token_code_hash = keccak256(&decode_runtime_code(
        &state.token_runtime_code_hex,
        "TRON token runtime code",
    )?);
    let verifier_code_hash = keccak256(&decode_runtime_code(
        &state.verifier_runtime_code_hex,
        "TRON verifier runtime code",
    )?);
    let route_code_hash = keccak256(&decode_runtime_code(
        &state.route_runtime_code_hex,
        "TRON route runtime code",
    )?);
    let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
        &state.verifying_key,
    )
    .ok_or_else(|| "authenticated TRON verifying key is not a canonical subgroup key".to_owned())?;
    let verifying_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(&state.verifying_key)
        .ok_or_else(|| "authenticated TRON verifying key cannot be hashed".to_owned())?;
    let expected_semantic_profile_hash =
        deployment
            .outbound_proof_policy
            .semantic_profile_hash()
            .map_err(|_| "governed TRON semantic proof profile is invalid".to_owned())?;
    let expected_finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()
        .map_err(|_| "governed TRON finality anchor is invalid".to_owned())?;
    if state.token_bridge_address != deployment.route_address
        || state.route_token_address != deployment.token_address
        || state.route_verifier_address != deployment.verifier_address
        || token_code_hash != deployment.token_code_hash
        || verifier_code_hash != deployment.verifier_code_hash
        || route_code_hash != deployment.route_code_hash
        || state.verifier_key_hash != deployment.verifier_key_hash
        || state.semantic_proof_profile_hash != expected_semantic_profile_hash
        || state.sora_finality_anchor_hash != expected_finality_anchor_hash
        || state.verifying_key != deployment.verifying_key
        || verifying_key_hash != state.verifier_key_hash
        || state.route_revision == 0
        || state.route_revision != route.revision
        || state.destination_binding_hash != expected_binding
        || state.route_configuration_hash != expected_route_configuration
        || state.governed_route_configuration_hash != expected_governed_configuration
    {
        return Err("authenticated TRON destination state differs from governed route".to_owned());
    }
    Ok(ValidatedDestinationStateV1 {
        observed_at_unix_ms: state.observed_at_unix_ms,
        finality_height: state.solid_block_height,
        finality_block_hash: state.solid_block_hash,
        destination_binding_hash: state.destination_binding_hash,
        route_configuration_hash: state.route_configuration_hash,
        governed_route_configuration_hash: state.governed_route_configuration_hash,
        verifier_key_hash: state.verifier_key_hash,
        semantic_proof_profile_hash: state.semantic_proof_profile_hash,
        sora_finality_anchor_hash: state.sora_finality_anchor_hash,
        route_revision: state.route_revision,
        verifying_key_sha256: sha256(&verifying_key_bytes),
        token_runtime_hash: token_code_hash,
        verifier_runtime_hash: verifier_code_hash,
        route_runtime_hash: route_code_hash,
    })
}

fn authenticate_destination_state(
    expected_profile: SccpNetworkV1,
    available: &AvailableOutboundEvidenceV1,
    expected_attestor_id: &str,
    expected_attestor_public_key: [u8; 32],
    approved_proof_system: &ApprovedProofSystemV1,
) -> Result<(ValidatedDestinationStateV1, [u8; 32]), String> {
    if available.attestor_id != expected_attestor_id
        || expected_attestor_id.is_empty()
        || expected_attestor_id.len() > 128
        || !expected_attestor_id.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
    {
        return Err("destination attestor identity does not match pinned policy".to_owned());
    }
    let signature = decode_lower_hex::<64>(&available.signature_hex, "destination signature")?;
    let statement_json = norito::json::to_json(&available.statement)
        .map_err(|_| "destination statement cannot be canonically encoded".to_owned())?;
    let mut signed =
        Vec::with_capacity(DESTINATION_ATTESTATION_DOMAIN.len() + statement_json.len());
    signed.extend_from_slice(DESTINATION_ATTESTATION_DOMAIN);
    signed.extend_from_slice(statement_json.as_bytes());
    iroha_crypto::ed25519_verify_batch_deterministic(
        &[signed.as_slice()],
        &[signature.as_slice()],
        &[expected_attestor_public_key.as_slice()],
        sha256(&signed),
    )
    .map_err(|_| "destination state has an invalid pinned-attestor signature".to_owned())?;
    let validated = match &available.statement {
        DestinationStateStatementV1::Evm(state) => {
            validate_evm_destination_state(expected_profile, state)?
        }
        DestinationStateStatementV1::Tron(state) => {
            validate_tron_destination_state(expected_profile, state)?
        }
    };
    validate_approved_proof_system(&validated, approved_proof_system)?;
    Ok((validated, sha256(statement_json.as_bytes())))
}

fn validate_approved_proof_system(
    validated: &ValidatedDestinationStateV1,
    approved: &ApprovedProofSystemV1,
) -> Result<(), String> {
    if approved.circuit_id.is_empty()
        || approved.circuit_id.len() > 128
        || !approved.circuit_id.as_bytes().iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
        || approved.circuit_id.contains("smoke")
        || approved.circuit_id.contains("test")
        || approved.circuit_id.contains("signal-binding")
        || approved.circuit_id.contains("labeled-signal")
        || approved.circuit_artifact_sha256 == FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256
        || approved.verifier_key_hash == FORBIDDEN_ALGEBRAIC_SMOKE_VK
        || validated.verifier_key_hash != approved.verifier_key_hash
        || validated.route_revision != approved.route_revision
        || validated.verifying_key_sha256 != approved.verifying_key_sha256
        || validated.semantic_proof_profile_hash != approved.semantic_proof_profile_hash
        || validated.sora_finality_anchor_hash != approved.sora_finality_anchor_hash
        || validated.token_runtime_hash != approved.token_runtime_hash
        || validated.verifier_runtime_hash != approved.verifier_runtime_hash
        || validated.route_runtime_hash != approved.route_runtime_hash
    {
        return Err("destination verifier is not the policy-approved semantic circuit".to_owned());
    }
    Ok(())
}

fn approved_proof_system_from_policy(
    proof: &ProofSystemPolicyV1,
) -> Result<ApprovedProofSystemV1, String> {
    let destination_build_value = norito::json::to_value(&proof.destination_build)
        .map_err(|_| "destination build policy cannot be represented as JSON".to_owned())?;
    let destination_build_json = norito::json::to_json(&destination_build_value)
        .map_err(|_| "destination build policy cannot be canonically encoded".to_owned())?;
    Ok(ApprovedProofSystemV1 {
        circuit_id: proof.circuit_id.clone(),
        circuit_artifact_sha256: require_hash(
            &proof.circuit_artifact_sha256_hex,
            "circuit artifact digest",
        )?,
        witness_generator_sha256: require_hash(
            &proof.witness_generator_sha256_hex,
            "witness generator digest",
        )?,
        public_signal_schema_hash: require_hash(
            &proof.public_signal_schema_hash_hex,
            "public signal schema hash",
        )?,
        semantic_proof_profile_hash: require_hash(
            &proof.semantic_proof_profile_hash_hex,
            "semantic proof profile hash",
        )?,
        sora_finality_anchor_hash: require_hash(
            &proof.sora_finality_anchor_hash_hex,
            "SORA finality anchor hash",
        )?,
        verifier_key_hash: require_hash(&proof.verifier_key_hash_hex, "verifier key hash")?,
        route_revision: proof.route_revision,
        verifying_key_sha256: require_hash(
            &proof.verifying_key_sha256_hex,
            "full verifying-key digest",
        )?,
        prover_build_sha256: require_hash(&proof.prover_build_sha256_hex, "prover build digest")?,
        toolchain_lock_sha256: require_hash(
            &proof.toolchain_lock_sha256_hex,
            "toolchain lock digest",
        )?,
        destination_build_policy_sha256: sha256(destination_build_json.as_bytes()),
        token_runtime_hash: require_hash(
            &proof.destination_build.token_runtime_hash_hex,
            "token runtime hash",
        )?,
        verifier_runtime_hash: require_hash(
            &proof.destination_build.verifier_runtime_hash_hex,
            "verifier runtime hash",
        )?,
        route_runtime_hash: require_hash(
            &proof.destination_build.route_runtime_hash_hex,
            "route runtime hash",
        )?,
    })
}

fn unavailable_reason_is_canonical(reason: &str) -> bool {
    (1..=160).contains(&reason.len())
        && reason
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && reason
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && reason
            .as_bytes()
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !reason.contains("--")
}

fn exact_unavailable_reason(profile: SccpNetworkV1) -> Option<&'static str> {
    release_profile_supported(profile)
        .then_some("authenticated-native-inbound-proof-is-unavailable")
}

fn release_profile_supported(profile: SccpNetworkV1) -> bool {
    matches!(
        profile,
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet | SccpNetworkV1::TronMainnet
    )
}

#[cfg(unix)]
fn metadata_is_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}

#[cfg(not(unix))]
fn metadata_is_single_link(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn metadata_identity_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn metadata_identity_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn read_direct_input(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path).map_err(|_| "input is not accessible")?;
    if before.file_type().is_symlink()
        || !before.is_file()
        || !metadata_is_single_link(&before)
        || before.len() == 0
        || before.len() > maximum
    {
        return Err("input must be one bounded direct regular file".to_owned());
    }
    let mut file = File::open(path).map_err(|_| "input cannot be opened")?;
    let opened = file
        .metadata()
        .map_err(|_| "input metadata cannot be read")?;
    if !opened.is_file()
        || !metadata_is_single_link(&opened)
        || !metadata_identity_matches(&before, &opened)
    {
        return Err("input changed while opening".to_owned());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    file.by_ref()
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input cannot be read")?;
    let after = fs::symlink_metadata(path).map_err(|_| "input disappeared while reading")?;
    if bytes.len() as u64 != before.len()
        || !metadata_identity_matches(&before, &after)
        || !metadata_identity_matches(&opened, &after)
    {
        return Err("input changed while reading".to_owned());
    }
    Ok(bytes)
}

fn validated_fields(
    validated: ValidatedSccpNativeInboundMessageV1,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    (
        lowercase_hex(&validated.lane_hash),
        lowercase_hex(&validated.source_identity_hash),
        lowercase_hex(&validated.trust_anchor.anchor_hash),
        lowercase_hex(&validated.message_key.message_id),
        lowercase_hex(&validated.payload_hash),
        lowercase_hex(&validated.source_event_digest),
        validated.source_finality.height.to_string(),
        lowercase_hex(&validated.source_finality.block_hash),
    )
}

fn validate_input(
    bytes: &[u8],
    expected_attestor_id: &str,
    expected_attestor_public_key: [u8; 32],
    approved_proof_system: &ApprovedProofSystemV1,
) -> Result<ReleaseLaneValidationV1, String> {
    let json = std::str::from_utf8(bytes).map_err(|_| "input must be canonical UTF-8")?;
    if json.as_bytes().contains(&0) {
        return Err("input must not contain NUL".to_owned());
    }
    let evidence = norito::json::from_str::<ReleaseLaneEvidenceV1>(json)
        .map_err(|_| "input is not strict SCCP release-lane Norito JSON")?;
    let canonical =
        norito::json::to_json(&evidence).map_err(|_| "input cannot be canonically encoded")?;
    if format!("{canonical}\n") != json {
        return Err("input must equal its canonical Norito JSON encoding plus one LF".to_owned());
    }
    if evidence.schema != INPUT_SCHEMA
        || evidence.version != 1
        || !release_profile_supported(evidence.profile)
    {
        return Err("input does not select an exact production SCCP V1 profile".to_owned());
    }

    let mut receipt = ReleaseLaneValidationV1 {
        schema: OUTPUT_SCHEMA.to_owned(),
        validator: validator_identity()?,
        trust_policy_id: String::new(),
        trust_policy_sha256_hex: String::new(),
        release_id: String::new(),
        release_evidence_sha256_hex: String::new(),
        artifact_sha256_hex: lowercase_hex(&sha256(bytes)),
        profile: evidence.profile.profile_key().to_owned(),
        inbound_status: String::new(),
        outbound_status: String::new(),
        unavailable_reasons: Vec::new(),
        source_profile: None,
        target_profile: None,
        lane_hash_hex: None,
        source_identity_hash_hex: None,
        native_anchor_hash_hex: None,
        message_id_hex: None,
        payload_hash_hex: None,
        source_event_digest_hex: None,
        finality_height: None,
        finality_block_hash_hex: None,
        destination_attestor_id: None,
        destination_statement_sha256_hex: None,
        destination_observed_at_unix_ms: None,
        destination_finality_height: None,
        destination_finality_block_hash_hex: None,
        destination_binding_hash_hex: None,
        route_configuration_hash_hex: None,
        governed_route_configuration_hash_hex: None,
        verifier_key_hash_hex: None,
        route_revision: None,
        verifying_key_sha256_hex: None,
        semantic_circuit_id: None,
        circuit_artifact_sha256_hex: None,
        witness_generator_sha256_hex: None,
        public_signal_schema_hash_hex: None,
        semantic_proof_profile_hash_hex: None,
        sora_finality_anchor_hash_hex: None,
        prover_build_sha256_hex: None,
        toolchain_lock_sha256_hex: None,
        destination_build_policy_sha256_hex: None,
    };

    match &evidence.outbound {
        ReleaseOutboundEvidenceV1::Available(available) => {
            let (validated, statement_hash) = authenticate_destination_state(
                evidence.profile,
                available,
                expected_attestor_id,
                expected_attestor_public_key,
                approved_proof_system,
            )?;
            receipt.outbound_status = "verified".to_owned();
            receipt.destination_attestor_id = Some(available.attestor_id.clone());
            receipt.destination_statement_sha256_hex = Some(lowercase_hex(&statement_hash));
            receipt.destination_observed_at_unix_ms =
                Some(validated.observed_at_unix_ms.to_string());
            receipt.destination_finality_height = Some(validated.finality_height.to_string());
            receipt.destination_finality_block_hash_hex =
                Some(lowercase_hex(&validated.finality_block_hash));
            receipt.destination_binding_hash_hex =
                Some(lowercase_hex(&validated.destination_binding_hash));
            receipt.route_configuration_hash_hex =
                Some(lowercase_hex(&validated.route_configuration_hash));
            receipt.governed_route_configuration_hash_hex =
                Some(lowercase_hex(&validated.governed_route_configuration_hash));
            receipt.verifier_key_hash_hex = Some(lowercase_hex(&validated.verifier_key_hash));
            receipt.route_revision = Some(validated.route_revision.to_string());
            receipt.verifying_key_sha256_hex = Some(lowercase_hex(&validated.verifying_key_sha256));
            receipt.semantic_circuit_id = Some(approved_proof_system.circuit_id.clone());
            receipt.circuit_artifact_sha256_hex = Some(lowercase_hex(
                &approved_proof_system.circuit_artifact_sha256,
            ));
            receipt.witness_generator_sha256_hex = Some(lowercase_hex(
                &approved_proof_system.witness_generator_sha256,
            ));
            receipt.public_signal_schema_hash_hex = Some(lowercase_hex(
                &approved_proof_system.public_signal_schema_hash,
            ));
            receipt.semantic_proof_profile_hash_hex = Some(lowercase_hex(
                &approved_proof_system.semantic_proof_profile_hash,
            ));
            receipt.sora_finality_anchor_hash_hex = Some(lowercase_hex(
                &approved_proof_system.sora_finality_anchor_hash,
            ));
            receipt.prover_build_sha256_hex =
                Some(lowercase_hex(&approved_proof_system.prover_build_sha256));
            receipt.toolchain_lock_sha256_hex =
                Some(lowercase_hex(&approved_proof_system.toolchain_lock_sha256));
            receipt.destination_build_policy_sha256_hex = Some(lowercase_hex(
                &approved_proof_system.destination_build_policy_sha256,
            ));
        }
        ReleaseOutboundEvidenceV1::Unavailable(outbound) => {
            if outbound.reason != OUTBOUND_UNAVAILABLE_REASON {
                return Err("outbound must use the exact fail-closed V1 reason".to_owned());
            }
            receipt.outbound_status = "unavailable".to_owned();
            receipt.unavailable_reasons.push(outbound.reason.clone());
        }
    }

    match evidence.inbound {
        ReleaseInboundEvidenceV1::Available(available) => {
            if !sccp_native_inbound_source_available_v1(evidence.profile)
                || available.proof.source.lane.source != evidence.profile
            {
                return Err("profile cannot use an available native inbound proof".to_owned());
            }
            let validated = verify_sccp_native_inbound_message_proof_v1(
                &available.proof,
                &available.governed_source_identity,
                available.governed_trust_anchor,
            )
            .map_err(|error| format!("native inbound proof failed: {error}"))?;
            let source = available.proof.source.lane.source.profile_key().to_owned();
            let target = available.proof.source.lane.target.profile_key().to_owned();
            let (lane, identity, anchor, message, payload, event, height, block) =
                validated_fields(validated);
            receipt.inbound_status = "verified".to_owned();
            receipt.source_profile = Some(source);
            receipt.target_profile = Some(target);
            receipt.lane_hash_hex = Some(lane);
            receipt.source_identity_hash_hex = Some(identity);
            receipt.native_anchor_hash_hex = Some(anchor);
            receipt.message_id_hex = Some(message);
            receipt.payload_hash_hex = Some(payload);
            receipt.source_event_digest_hex = Some(event);
            receipt.finality_height = Some(height);
            receipt.finality_block_hash_hex = Some(block);
        }
        ReleaseInboundEvidenceV1::Unavailable(unavailable) => {
            if !unavailable_reason_is_canonical(&unavailable.reason) {
                return Err("inbound unavailable reason is not canonical".to_owned());
            }
            if let Some(expected) = exact_unavailable_reason(evidence.profile)
                && unavailable.reason != expected
            {
                return Err("inbound must use the exact fail-closed V1 reason".to_owned());
            }
            receipt.inbound_status = "unavailable".to_owned();
            receipt.unavailable_reasons.push(unavailable.reason);
        }
    }
    Ok(receipt)
}

fn validate_lane_in_release_context(
    lane_bytes: &[u8],
    policy_bytes: &[u8],
    evidence_bytes: &[u8],
    expected_environment: &str,
) -> Result<ReleaseLaneValidationV1, String> {
    let (policy, evidence, release_receipt) =
        validate_release_context(policy_bytes, evidence_bytes, expected_environment)?;
    let lane_json = std::str::from_utf8(lane_bytes)
        .map_err(|_| "lane evidence must be canonical UTF-8".to_owned())?;
    let lane_value = norito::json::from_str::<ReleaseLaneEvidenceV1>(lane_json)
        .map_err(|_| "lane evidence does not match the typed SCCP V1 schema".to_owned())?;
    let profile = lane_value.profile.profile_key();
    let profile_index = RELEASE_PROFILES
        .iter()
        .position(|expected| *expected == profile)
        .ok_or_else(|| "lane evidence profile is outside SCCP V1".to_owned())?;
    let trusted_attestor = &policy.destination_attestors[profile_index];
    let trusted_key =
        validate_ed25519_public_key(&trusted_attestor.public_key_hex, "destination attestor key")?;
    let approved = approved_proof_system_from_policy(&policy.proof_systems[profile_index])?;
    let mut receipt = validate_input(
        lane_bytes,
        &trusted_attestor.attestor_id,
        trusted_key,
        &approved,
    )?;

    let signed_lane = &evidence.lanes[profile_index];
    let signed_artifact = evidence
        .artifacts
        .iter()
        .find(|artifact| artifact.path == signed_lane.evidence_artifact_path)
        .ok_or_else(|| "signed lane artifact is absent from the inventory".to_owned())?;
    if signed_artifact.kind != "lane-evidence"
        || signed_artifact.size_bytes != lane_bytes.len() as u64
        || signed_artifact.sha256_hex != receipt.artifact_sha256_hex
        || signed_lane.inbound_status != receipt.inbound_status
        || signed_lane.outbound_status != receipt.outbound_status
    {
        return Err("typed lane result does not match its signed evidence inventory".to_owned());
    }
    if let Some(observed_at_unix_ms) = receipt.destination_observed_at_unix_ms.as_deref() {
        let observed_at_unix_ms = observed_at_unix_ms
            .parse::<u64>()
            .map_err(|_| "destination observation time is not a canonical u64".to_owned())?;
        if observed_at_unix_ms > evidence.created_at_unix_ms
            || evidence.created_at_unix_ms - observed_at_unix_ms
                > MAX_DESTINATION_ATTESTATION_AGE_MS
        {
            return Err("destination state attestation is future-dated or stale".to_owned());
        }
    }
    receipt.trust_policy_id = release_receipt.policy_id;
    receipt.trust_policy_sha256_hex = release_receipt.policy_sha256_hex;
    receipt.release_id = release_receipt.release_id;
    receipt.release_evidence_sha256_hex = release_receipt.evidence_sha256_hex;
    Ok(receipt)
}

fn print_receipt(receipt: &ReleaseLaneValidationV1) -> Result<(), String> {
    let json = norito::json::to_json(receipt)
        .map_err(|_| "validation receipt cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("validation receipt exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

fn print_release_signature_receipt(receipt: &ReleaseSignatureValidationV1) -> Result<(), String> {
    let json = norito::json::to_json(receipt)
        .map_err(|_| "release signature receipt cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("release signature receipt exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

fn print_semantic_proof_receipt(receipt: &SemanticProofValidationV1) -> Result<(), String> {
    let json = norito::json::to_json(receipt)
        .map_err(|_| "semantic proof receipt cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("semantic proof receipt exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

fn print_validator_identity() -> Result<(), String> {
    let identity = validator_identity()?;
    let json = norito::json::to_json(&identity)
        .map_err(|_| "validator identity cannot be encoded".to_owned())?;
    if json.len() > MAX_OUTPUT_BYTES {
        return Err("validator identity exceeds the output bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

#[cfg(feature = "test-fixtures")]
fn emit_ethereum_fixture() -> Result<(), String> {
    let (proof, governed_source_identity, governed_trust_anchor) =
        iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
    let evidence = ReleaseLaneEvidenceV1 {
        schema: INPUT_SCHEMA.to_owned(),
        version: 1,
        profile: SccpNetworkV1::EthereumMainnet,
        inbound: ReleaseInboundEvidenceV1::Available(AvailableInboundEvidenceV1 {
            proof,
            governed_source_identity,
            governed_trust_anchor,
        }),
        outbound: ReleaseOutboundEvidenceV1::Unavailable(UnavailableDirectionV1 {
            reason: OUTBOUND_UNAVAILABLE_REASON.to_owned(),
        }),
    };
    let json =
        norito::json::to_json(&evidence).map_err(|_| "fixture cannot be encoded".to_owned())?;
    if json.len() as u64 > MAX_INPUT_BYTES {
        return Err("fixture exceeds input bound".to_owned());
    }
    println!("{json}");
    Ok(())
}

#[cfg(feature = "test-fixtures")]
fn emit_unavailable_fixture(profile: &str) -> Result<(), String> {
    let network = SccpNetworkV1::from_profile_key(profile)
        .filter(|network| release_profile_supported(*network))
        .ok_or_else(|| "fixture profile must be one supported production profile".to_owned())?;
    let reason = exact_unavailable_reason(network)
        .unwrap_or("no-canonical-native-proof-in-release-fixture")
        .to_owned();
    let evidence = ReleaseLaneEvidenceV1 {
        schema: INPUT_SCHEMA.to_owned(),
        version: 1,
        profile: network,
        inbound: ReleaseInboundEvidenceV1::Unavailable(UnavailableDirectionV1 { reason }),
        outbound: ReleaseOutboundEvidenceV1::Unavailable(UnavailableDirectionV1 {
            reason: OUTBOUND_UNAVAILABLE_REASON.to_owned(),
        }),
    };
    let json =
        norito::json::to_json(&evidence).map_err(|_| "fixture cannot be encoded".to_owned())?;
    println!("{json}");
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let command = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| {
            "usage: sccp_release_evidence identity | validate-release <trust-policy-json> <evidence-json> <production|test-fixture> | validate-semantic-proof <proof-norito> <trust-policy-json> <evidence-json> <profile> production | validate <lane-json> <trust-policy-json> <evidence-json> <production|test-fixture>".to_owned()
        })?;
    match command.as_str() {
        "identity" => {
            if args.next().is_some() {
                return Err("identity accepts no arguments".to_owned());
            }
            print_validator_identity()
        }
        "validate-release" => {
            let policy_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate-release requires one trust-policy path".to_owned())?;
            let evidence_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate-release requires one evidence path".to_owned())?;
            let environment = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate-release requires one environment".to_owned())?;
            if args.next().is_some() {
                return Err("validate-release accepts exactly three arguments".to_owned());
            }
            let policy_bytes = read_direct_input(&policy_path, MAX_RELEASE_POLICY_BYTES)?;
            let evidence_bytes = read_direct_input(&evidence_path, MAX_RELEASE_EVIDENCE_BYTES)?;
            let receipt =
                validate_release_signatures(&policy_bytes, &evidence_bytes, &environment)?;
            print_release_signature_receipt(&receipt)
        }
        "validate-semantic-proof" => {
            let proof_path = args.next().map(PathBuf::from).ok_or_else(|| {
                "validate-semantic-proof requires one proof artifact path".to_owned()
            })?;
            let policy_path = args.next().map(PathBuf::from).ok_or_else(|| {
                "validate-semantic-proof requires one trust-policy path".to_owned()
            })?;
            let evidence_path = args.next().map(PathBuf::from).ok_or_else(|| {
                "validate-semantic-proof requires one release-evidence path".to_owned()
            })?;
            let profile = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate-semantic-proof requires one profile".to_owned())?;
            let environment = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate-semantic-proof requires one environment".to_owned())?;
            if args.next().is_some() {
                return Err("validate-semantic-proof accepts exactly five arguments".to_owned());
            }
            let proof_bytes = read_direct_input(
                &proof_path,
                u64::try_from(SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1)
                    .map_err(|_| "semantic proof size bound exceeds u64".to_owned())?,
            )?;
            let policy_bytes = read_direct_input(&policy_path, MAX_RELEASE_POLICY_BYTES)?;
            let evidence_bytes = read_direct_input(&evidence_path, MAX_RELEASE_EVIDENCE_BYTES)?;
            let receipt = validate_semantic_proof_in_release_context(
                &proof_bytes,
                &policy_bytes,
                &evidence_bytes,
                &profile,
                &environment,
            )?;
            print_semantic_proof_receipt(&receipt)
        }
        "validate" => {
            let lane_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one lane-evidence path".to_owned())?;
            let policy_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one trust-policy path".to_owned())?;
            let evidence_path = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "validate requires one release-evidence path".to_owned())?;
            let environment = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "validate requires one environment".to_owned())?;
            if args.next().is_some() {
                return Err("validate accepts exactly four arguments".to_owned());
            }
            let lane_bytes = read_direct_input(&lane_path, MAX_LANE_INPUT_BYTES)?;
            let policy_bytes = read_direct_input(&policy_path, MAX_RELEASE_POLICY_BYTES)?;
            let evidence_bytes = read_direct_input(&evidence_path, MAX_RELEASE_EVIDENCE_BYTES)?;
            let receipt = validate_lane_in_release_context(
                &lane_bytes,
                &policy_bytes,
                &evidence_bytes,
                &environment,
            )?;
            print_receipt(&receipt)
        }
        #[cfg(feature = "test-fixtures")]
        "emit-ethereum-fixture" => {
            if args.next().is_some() {
                return Err("emit-ethereum-fixture accepts no arguments".to_owned());
            }
            emit_ethereum_fixture()
        }
        #[cfg(feature = "test-fixtures")]
        "emit-unavailable-fixture" => {
            let profile = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "emit-unavailable-fixture requires one profile".to_owned())?;
            if args.next().is_some() {
                return Err("emit-unavailable-fixture accepts exactly one profile".to_owned());
            }
            emit_unavailable_fixture(&profile)
        }
        _ => Err(
            "unknown command; expected identity, validate-release, validate-semantic-proof, or validate"
                .to_owned(),
        ),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln_bounded_stderr(&error);
            ExitCode::FAILURE
        }
    }
}

fn writeln_bounded_stderr(error: &str) -> io::Result<()> {
    use std::io::Write as _;
    let mut bytes = error.as_bytes();
    if bytes.len() > 1024 {
        bytes = &bytes[..1024];
    }
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(b"SCCP release evidence validation failed: ")?;
    lock.write_all(bytes)?;
    lock.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_identity_is_stable_and_nonzero() {
        let first = validator_identity().unwrap();
        let second = validator_identity().unwrap();
        assert_eq!(first.source_sha256_hex, second.source_sha256_hex);
        assert_eq!(first.build_identity_hex, second.build_identity_hex);
        assert_ne!(first.source_sha256_hex, "00".repeat(32));
        assert_ne!(first.build_identity_hex, "00".repeat(32));
        #[cfg(not(feature = "test-fixtures"))]
        validate_validator_identity_shape(&first).unwrap();
    }

    #[test]
    #[cfg(not(feature = "test-fixtures"))]
    fn validator_identity_rejects_test_features_and_mismatched_rustc() {
        let identity = validator_identity().unwrap();
        let mut test_features = identity.clone();
        test_features
            .enabled_features
            .push("test-fixtures".to_owned());
        assert!(validate_validator_identity_shape(&test_features).is_err());

        let mut wrong_rustc = identity.clone();
        wrong_rustc.rustc_version = "rustc 0.0.0 (000000000 1970-01-01)".to_owned();
        assert!(validate_validator_identity_shape(&wrong_rustc).is_err());

        let mut aliased_hash = identity;
        aliased_hash.toolchain_lock_sha256_hex = aliased_hash.source_sha256_hex.clone();
        assert!(validate_validator_identity_shape(&aliased_hash).is_err());
    }

    #[test]
    fn validator_build_identity_matches_python_golden() {
        let features = Vec::<String>::new();
        let identity = validator_build_identity_hash(&ValidatorBuildIdentityInputs {
            protocol_version: 1,
            crate_name: "iroha_sccp",
            crate_version: "2.0.0-rc.2.0",
            enabled_features: &features,
            build_profile: "release",
            target_triple: "aarch64-apple-darwin",
            rustc_version: "rustc 1.93.1 (01f6ddf75 2026-02-11)",
            source_hash: [1; 32],
            crate_manifest_hash: [2; 32],
            build_script_hash: [3; 32],
            workspace_manifest_hash: [4; 32],
            cargo_lock_hash: [5; 32],
            toolchain_lock_hash: [6; 32],
        })
        .unwrap();
        assert_eq!(
            lowercase_hex(&identity),
            "5f2fb61fb1622ae4e5a233f72f431cae8cb96c6ea64fde57bb130f3940344f9b"
        );
    }

    #[test]
    #[cfg(feature = "test-fixtures")]
    fn release_validator_rejects_a_build_with_test_fixture_features() {
        assert!(validate_validator_identity_shape(&validator_identity().unwrap()).is_err());
    }

    #[test]
    fn proof_hash_registry_rejects_local_and_cross_profile_role_aliases() {
        let mut global = BTreeMap::new();
        let mut ethereum = BTreeSet::new();
        register_hash_role(
            &mut ethereum,
            &mut global,
            "circuit_artifact_sha256_hex",
            [1; 32],
        )
        .unwrap();
        assert!(
            register_hash_role(&mut ethereum, &mut global, "verifier_key_hash_hex", [1; 32],)
                .is_err()
        );

        let mut bsc = BTreeSet::new();
        assert!(
            register_hash_role(
                &mut bsc,
                &mut global,
                "witness_generator_sha256_hex",
                [1; 32],
            )
            .is_err()
        );
        register_hash_role(
            &mut bsc,
            &mut global,
            "circuit_artifact_sha256_hex",
            [1; 32],
        )
        .unwrap();
    }

    #[test]
    fn audit_report_registry_rejects_cross_profile_reuse() {
        let mut reports = BTreeSet::new();
        register_audit_report(&mut reports, [7; 32]).unwrap();
        assert!(register_audit_report(&mut reports, [7; 32]).is_err());
        register_audit_report(&mut reports, [8; 32]).unwrap();
    }

    #[test]
    fn unavailable_reason_rejects_aliases_and_controls() {
        assert!(unavailable_reason_is_canonical(
            "sound-proof-is-unavailable"
        ));
        for invalid in [
            "",
            "UPPERCASE",
            "-leading",
            "trailing-",
            "double--separator",
            " leading",
            "trailing ",
            "contains_underscore",
            "contains\ncontrol",
        ] {
            assert!(!unavailable_reason_is_canonical(invalid));
        }
    }

    #[test]
    fn first_release_profiles_exclude_domains_three_and_four() {
        for supported in [
            SccpNetworkV1::EthereumMainnet,
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::TronMainnet,
        ] {
            assert!(release_profile_supported(supported));
        }
        for unsupported in [
            "solana-mainnet-beta",
            "ton-mainnet",
            "ethereum-sepolia",
            "tron-nile",
        ] {
            if let Some(profile) = SccpNetworkV1::from_profile_key(unsupported) {
                assert!(!release_profile_supported(profile));
            }
        }
    }

    #[test]
    fn runtime_decoder_and_keccak_are_strict() {
        assert_eq!(decode_runtime_code("6000", "runtime").unwrap(), [0x60, 0]);
        assert_eq!(
            lowercase_hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        for invalid in ["", "0", "0X00", "AA", "zz"] {
            assert!(decode_runtime_code(invalid, "runtime").is_err());
        }
    }

    #[test]
    fn fixed_hex_decoder_rejects_zero_uppercase_and_length_drift() {
        assert_eq!(decode_lower_hex::<2>("0102", "value").unwrap(), [1, 2]);
        for invalid in ["0000", "010", "01020", "ABCD", "0x01"] {
            assert!(decode_lower_hex::<2>(invalid, "value").is_err());
        }
    }

    #[test]
    fn release_signature_decoder_and_crypto_reject_noncanonical_material() {
        let key = decode_lower_hex::<32>(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "key",
        )
        .unwrap();
        let signature = decode_signature_base64(
            "5VZDAMNgrHKQhuLMgG6CioSHfx645dl02HPgZSJJAVVfuIIVkKM7rMYeOXAc+bRr0lv18FlbviRlUUFDjnoQCw==",
        )
        .unwrap();
        verify_ed25519_signature(&key, &signature, b"").unwrap();

        let mut high_s = signature;
        high_s[32..].fill(0xff);
        assert!(verify_ed25519_signature(&key, &high_s, b"").is_err());
        assert!(decode_signature_base64(
            "5VZDAMNgrHKQhuLMgG6CioSHfx645dl02HPgZSJJAVVfuIIVkKM7rMYeOXAc+bRr0lv18FlbviRlUUFDjnoQCx=="
        )
        .is_err());
        assert!(
            validate_ed25519_public_key(
                "0100000000000000000000000000000000000000000000000000000000000000",
                "small-order key"
            )
            .is_err()
        );
    }

    #[test]
    fn sorted_json_parser_rejects_duplicates_and_layout_drift() {
        assert!(parse_canonical_sorted_json(b"{\"a\":1}\n", "json").is_ok());
        assert!(parse_canonical_sorted_json(b"{\"a\":1,\"a\":2}\n", "json").is_err());
        assert!(parse_canonical_sorted_json(b"{ \"a\": 1 }\n", "json").is_err());
        assert!(parse_canonical_sorted_json(b"{\"b\":1,\"a\":2}\n", "json").is_err());
    }

    #[test]
    fn sumeragi_v2_finality_anchor_policy_is_exact_and_role_separated() {
        let anchor = SoraFinalityAnchorPolicyV1 {
            version: 1,
            source_profile: "sora-taira".to_owned(),
            protocol_version: SUMERAGI_V2_PROTOCOL_VERSION,
            chain_id_hash_hex: "cf1cfc0f57b0bfa4c21882a9870317a1f4812f86533897095e3944be34c5bba7"
                .to_owned(),
            checkpoint_height: 5,
            checkpoint_block_hash_hex: lowercase_hex(&[0x73; 32]),
            checkpoint_context_id_hex: lowercase_hex(&[0x74; 32]),
            checkpoint_finality_artifact_hash_hex: lowercase_hex(&[0x75; 32]),
        };
        let validated = validate_sora_finality_anchor_policy(&anchor).unwrap();
        assert_eq!(
            lowercase_hex(&validated.anchor_hash),
            "690888c1b9a1409ea47fc682be915184e86a817a2f0b3439eef82e64e08e990b"
        );

        for mutation in 0..=10 {
            let mut candidate = anchor.clone();
            match mutation {
                0 => candidate.protocol_version = 1,
                1 => candidate.protocol_version = 3,
                2 => candidate.checkpoint_context_id_hex = lowercase_hex(&[0; 32]),
                3 => {
                    candidate.checkpoint_finality_artifact_hash_hex = lowercase_hex(&[0; 32]);
                }
                4 => {
                    candidate.checkpoint_context_id_hex = candidate.chain_id_hash_hex.clone();
                }
                5 => {
                    candidate.checkpoint_context_id_hex =
                        candidate.checkpoint_block_hash_hex.clone();
                }
                6 => {
                    candidate.checkpoint_context_id_hex =
                        candidate.checkpoint_finality_artifact_hash_hex.clone();
                }
                7 => {
                    candidate.checkpoint_finality_artifact_hash_hex =
                        candidate.checkpoint_context_id_hex.clone();
                }
                8 => {
                    candidate.checkpoint_finality_artifact_hash_hex =
                        candidate.checkpoint_block_hash_hex.clone();
                }
                9 => {
                    candidate.checkpoint_finality_artifact_hash_hex =
                        candidate.chain_id_hash_hex.clone();
                }
                10 => candidate.checkpoint_height = 0,
                _ => unreachable!(),
            }
            assert!(
                validate_sora_finality_anchor_policy(&candidate).is_err(),
                "accepted malformed v2 finality anchor mutation {mutation}"
            );
        }

        fn matches_exact_anchor_schema(json: &str) -> bool {
            let Ok(value) = norito::json::parse_value(json) else {
                return false;
            };
            let Ok(decoded) = norito::json::from_str::<SoraFinalityAnchorPolicyV1>(json) else {
                return false;
            };
            norito::json::to_value(&decoded).ok().as_ref() == Some(&value)
        }

        let json = norito::json::to_json(&anchor).unwrap();
        assert!(matches_exact_anchor_schema(&json));
        for confused in [
            json.replace("\"protocol_version\":2", "\"protocol_version\":true"),
            json.replace("\"protocol_version\":2", "\"protocol_version\":\"2\""),
            json.replace("\"checkpoint_height\":5", "\"checkpoint_height\":true"),
        ] {
            assert!(
                !matches_exact_anchor_schema(&confused),
                "accepted type-confused v2 finality anchor: {confused}"
            );
        }
        for legacy_field in [
            "\"validator_set_epoch\":17,",
            "\"validator_set_hash_hex\":\"7676767676767676767676767676767676767676767676767676767676767676\",",
            "\"validator_set_hash_version\":1,",
        ] {
            let injected = json.replacen('{', &format!("{{{legacy_field}"), 1);
            assert!(
                !matches_exact_anchor_schema(&injected),
                "accepted retired validator-set field: {legacy_field}"
            );
        }
    }

    #[test]
    fn release_lane_fixtures_match_the_production_typed_schema() {
        for fixture in [
            include_str!(
                "../../../../fixtures/sccp/release_evidence_v1/artifacts/lanes/ethereum-mainnet.json"
            ),
            include_str!(
                "../../../../fixtures/sccp/release_evidence_v1/artifacts/lanes/bsc-mainnet.json"
            ),
            include_str!(
                "../../../../fixtures/sccp/release_evidence_v1/artifacts/lanes/tron-mainnet.json"
            ),
        ] {
            let lane = norito::json::from_str::<ReleaseLaneEvidenceV1>(fixture)
                .expect("release fixture must use the production typed lane schema");
            let canonical = norito::json::to_json(&lane)
                .expect("typed release fixture must have canonical Norito JSON");
            assert_eq!(format!("{canonical}\n"), fixture);
        }
    }

    fn validated_destination() -> ValidatedDestinationStateV1 {
        ValidatedDestinationStateV1 {
            observed_at_unix_ms: 1,
            finality_height: 1,
            finality_block_hash: [1; 32],
            destination_binding_hash: [2; 32],
            route_configuration_hash: [3; 32],
            governed_route_configuration_hash: [4; 32],
            verifier_key_hash: [5; 32],
            semantic_proof_profile_hash: [14; 32],
            sora_finality_anchor_hash: [15; 32],
            route_revision: 1,
            verifying_key_sha256: [13; 32],
            token_runtime_hash: [6; 32],
            verifier_runtime_hash: [7; 32],
            route_runtime_hash: [8; 32],
        }
    }

    fn approved_proof_system() -> ApprovedProofSystemV1 {
        ApprovedProofSystemV1 {
            circuit_id: RELEASE_CIRCUIT_IDS[0].to_owned(),
            circuit_artifact_sha256: [9; 32],
            witness_generator_sha256: [16; 32],
            public_signal_schema_hash: [17; 32],
            semantic_proof_profile_hash: [14; 32],
            sora_finality_anchor_hash: [15; 32],
            verifier_key_hash: [5; 32],
            route_revision: 1,
            verifying_key_sha256: [13; 32],
            prover_build_sha256: [10; 32],
            toolchain_lock_sha256: [11; 32],
            destination_build_policy_sha256: [12; 32],
            token_runtime_hash: [6; 32],
            verifier_runtime_hash: [7; 32],
            route_runtime_hash: [8; 32],
        }
    }

    #[cfg(not(feature = "test-fixtures"))]
    fn release_evidence_envelope() -> ReleaseEvidenceSignaturesV1 {
        let lanes = RELEASE_PROFILES
            .iter()
            .zip(RELEASE_DOMAINS)
            .map(|(profile, domain)| SignedLaneSummaryV1 {
                counterparty_profile: (*profile).to_owned(),
                counterparty_domain: domain,
                inbound_status: "unavailable".to_owned(),
                outbound_status: "unavailable".to_owned(),
                evidence_artifact_path: format!("artifacts/lanes/{profile}.json"),
            })
            .collect::<Vec<_>>();
        let phases = REQUIRED_PHASES
            .iter()
            .map(|name| ReleaseValidationPhaseV1 {
                name: (*name).to_owned(),
                status: "passed".to_owned(),
                artifact_path: format!("artifacts/phases/{name}.log"),
            })
            .collect::<Vec<_>>();
        let mut artifact_specs = lanes
            .iter()
            .map(|lane| (lane.evidence_artifact_path.clone(), "lane-evidence"))
            .chain(
                phases
                    .iter()
                    .map(|phase| (phase.artifact_path.clone(), "phase-transcript")),
            )
            .collect::<Vec<_>>();
        artifact_specs.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let artifacts = artifact_specs
            .into_iter()
            .enumerate()
            .map(|(index, (path, kind))| ReleaseArtifactV1 {
                path,
                kind: kind.to_owned(),
                sha256_hex: lowercase_hex(&[u8::try_from(index + 1).unwrap(); 32]),
                size_bytes: 1,
            })
            .collect();
        ReleaseEvidenceSignaturesV1 {
            schema: RELEASE_EVIDENCE_SCHEMA.to_owned(),
            release_id: "release-envelope-test-v1".to_owned(),
            protocol_version: 1,
            hub_profile: "sora-taira".to_owned(),
            hub_chain_id: SCCP_TAIRA_FINALITY_CHAIN_ID_V1.to_owned(),
            created_at_unix_ms: 1,
            trust_policy_id: "external-policy-v1".to_owned(),
            trust_policy_sha256_hex: lowercase_hex(&[99; 32]),
            validator: validator_identity().unwrap(),
            lanes,
            artifacts,
            validation: ReleaseValidationV1 {
                corridor: "sccp-production-corridor-v1".to_owned(),
                phases,
            },
            provenance: Vec::new(),
        }
    }

    #[cfg(not(feature = "test-fixtures"))]
    fn release_envelope_test_policy() -> ReleaseTrustPolicyV1 {
        ReleaseTrustPolicyV1 {
            schema: TEST_POLICY_SCHEMA.to_owned(),
            environment: "test-fixture".to_owned(),
            policy_id: "external-policy-v1".to_owned(),
            roles: Vec::new(),
            destination_attestors: Vec::new(),
            circuit_auditors: Vec::new(),
            proof_systems: Vec::new(),
        }
    }

    #[test]
    fn approved_proof_system_binds_semantics_vk_and_every_runtime() {
        let validated = validated_destination();
        let approved = approved_proof_system();
        validate_approved_proof_system(&validated, &approved).unwrap();

        assert_eq!(
            lowercase_hex(&FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256),
            "d7049de0f0b0ecb7ec4f64b885646ab99f85fcbab05dfaf710d3002f17632bb9"
        );

        for mutation in 0..=12 {
            let mut candidate = approved.clone();
            match mutation {
                0 => candidate.circuit_id = "algebraic-smoke-v1".to_owned(),
                1 => {
                    candidate.circuit_id = "sccp-bsc-labeled-signal-binding-v1".to_owned();
                }
                2 => {
                    candidate.circuit_id = "public-signal-binding-material-only".to_owned();
                }
                3 => candidate.verifier_key_hash = FORBIDDEN_ALGEBRAIC_SMOKE_VK,
                4 => candidate.verifier_key_hash[0] ^= 1,
                5 => candidate.token_runtime_hash[0] ^= 1,
                6 => candidate.verifier_runtime_hash[0] ^= 1,
                7 => candidate.route_runtime_hash[0] ^= 1,
                8 => candidate.route_revision = 2,
                9 => candidate.verifying_key_sha256[0] ^= 1,
                10 => candidate.semantic_proof_profile_hash[0] ^= 1,
                11 => candidate.sora_finality_anchor_hash[0] ^= 1,
                12 => {
                    candidate.circuit_artifact_sha256 = FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256;
                }
                _ => unreachable!(),
            }
            assert!(validate_approved_proof_system(&validated, &candidate).is_err());
        }
    }

    #[cfg(feature = "test-fixtures")]
    fn semantic_proof_policy_from_fixture() -> (Vec<u8>, ProofSystemPolicyV1) {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let proof_bytes =
            iroha_sccp::encode_canonical_sccp_groth16_bn254_proof_artifact_v1(&fixture.artifact)
                .expect("fixture proof must encode canonically");
        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit) =
            fixture.artifact.request.semantic_proof_profile;
        let anchor = fixture.artifact.request.sora_finality_anchor;
        let SccpPayloadV1::Transfer(transfer) = fixture.bundle.payload;
        let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
            &fixture.artifact.request.verifying_key,
        )
        .expect("fixture verifying key must be canonical");
        let filler = |byte: u8| lowercase_hex(&[byte; 32]);
        (
            proof_bytes,
            ProofSystemPolicyV1 {
                counterparty_profile: "ethereum-mainnet".to_owned(),
                circuit_id: RELEASE_CIRCUIT_IDS[0].to_owned(),
                semantics: REQUIRED_SEMANTICS.iter().map(ToString::to_string).collect(),
                circuit_artifact_sha256_hex: lowercase_hex(&circuit.circuit_commitment),
                witness_generator_sha256_hex: lowercase_hex(&circuit.witness_generator_commitment),
                public_signal_schema_hash_hex: lowercase_hex(&circuit.public_signal_schema_hash),
                semantic_proof_profile_hash_hex: lowercase_hex(
                    &fixture.artifact.request.semantic_proof_profile_hash,
                ),
                sora_finality_anchor: SoraFinalityAnchorPolicyV1 {
                    version: anchor.version,
                    source_profile: "sora-taira".to_owned(),
                    protocol_version: anchor.protocol_version,
                    chain_id_hash_hex: lowercase_hex(&anchor.chain_id_hash),
                    checkpoint_height: anchor.checkpoint_height,
                    checkpoint_block_hash_hex: lowercase_hex(&anchor.checkpoint_block_hash),
                    checkpoint_context_id_hex: lowercase_hex(&anchor.checkpoint_context_id),
                    checkpoint_finality_artifact_hash_hex: lowercase_hex(
                        &anchor.checkpoint_finality_artifact_hash,
                    ),
                },
                sora_finality_anchor_hash_hex: lowercase_hex(
                    &fixture.artifact.request.sora_finality_anchor_hash,
                ),
                verifier_key_hash_hex: lowercase_hex(&fixture.artifact.request.verifier_key_hash),
                route_revision: transfer.route_revision,
                verifying_key_sha256_hex: lowercase_hex(&sha256(&verifying_key_bytes)),
                prover_build_sha256_hex: filler(0xa1),
                toolchain_lock_sha256_hex: filler(0xa2),
                destination_build: DestinationBuildPolicyV1 {
                    source_bundle_sha256_hex: filler(0xb1),
                    compiler_build_sha256_hex: filler(0xb2),
                    token_artifact_sha256_hex: filler(0xb3),
                    token_interface_sha256_hex: filler(0xb4),
                    token_runtime_hash_hex: filler(0xb5),
                    verifier_artifact_sha256_hex: filler(0xb6),
                    verifier_interface_sha256_hex: filler(0xb7),
                    verifier_runtime_hash_hex: filler(0xb8),
                    route_artifact_sha256_hex: filler(0xb9),
                    route_interface_sha256_hex: filler(0xba),
                    route_runtime_hash_hex: filler(0xbb),
                },
                audit_attestations: Vec::new(),
            },
        )
    }

    #[test]
    #[cfg(feature = "test-fixtures")]
    fn semantic_proof_claim_decodes_pairs_and_rejects_every_governed_substitution() {
        let (proof_bytes, policy) = semantic_proof_policy_from_fixture();
        let claim = semantic_proof_claim(&proof_bytes, "ethereum-mainnet", &policy)
            .expect("pairing-valid fixture must produce an exact semantic claim");
        assert_eq!(claim.target_profile, "ethereum-mainnet");
        assert_eq!(claim.public_signal_words_hex.len(), 11);
        assert_eq!(claim.route_revision, policy.route_revision);

        let mut corrupted = proof_bytes.clone();
        *corrupted.last_mut().expect("proof is nonempty") ^= 1;
        assert!(semantic_proof_claim(&corrupted, "ethereum-mainnet", &policy).is_err());
        assert!(semantic_proof_claim(&proof_bytes, "bsc-mainnet", &policy).is_err());

        for mutation in 0..7 {
            let mut candidate = policy.clone();
            match mutation {
                0 => candidate.route_revision += 1,
                1 => candidate.circuit_artifact_sha256_hex = lowercase_hex(&[0xc1; 32]),
                2 => candidate.witness_generator_sha256_hex = lowercase_hex(&[0xc2; 32]),
                3 => candidate.semantic_proof_profile_hash_hex = lowercase_hex(&[0xc3; 32]),
                4 => candidate.sora_finality_anchor_hash_hex = lowercase_hex(&[0xc4; 32]),
                5 => candidate.verifier_key_hash_hex = lowercase_hex(&[0xc5; 32]),
                6 => candidate.verifying_key_sha256_hex = lowercase_hex(&[0xc6; 32]),
                _ => unreachable!(),
            }
            assert!(
                semantic_proof_claim(&proof_bytes, "ethereum-mainnet", &candidate).is_err(),
                "accepted semantic policy mutation {mutation}"
            );
        }
    }

    #[test]
    #[cfg(not(feature = "test-fixtures"))]
    fn release_envelope_binds_exact_profiles_inventory_and_corridor() {
        let evidence = release_evidence_envelope();
        let policy = release_envelope_test_policy();
        validate_release_evidence_envelope(&evidence, &policy, "test-fixture").unwrap();
        validate_release_hub(&evidence).unwrap();

        let mut nexus = release_evidence_envelope();
        nexus.hub_profile = "sora-nexus".to_owned();
        nexus.hub_chain_id = "00000000-0000-0000-0000-000000000753".to_owned();
        assert!(validate_release_hub(&nexus).is_err());

        for mutation in 0..=8 {
            let mut candidate = release_evidence_envelope();
            match mutation {
                0 => candidate.lanes[0].counterparty_profile = "solana-mainnet-beta".to_owned(),
                1 => candidate.lanes[0].counterparty_domain = 3,
                2 => candidate.lanes[0].evidence_artifact_path = "../escape.json".to_owned(),
                3 => candidate.validation.phases[0].status = "skipped".to_owned(),
                4 => candidate.artifacts[1].sha256_hex = candidate.artifacts[0].sha256_hex.clone(),
                5 => candidate.artifacts[0].size_bytes = 0,
                6 => candidate.artifacts.swap(0, 1),
                7 => {
                    let digest = lowercase_hex(&[0xee; 32]);
                    candidate.artifacts.push(ReleaseArtifactV1 {
                        path: semantic_artifact_path(
                            "honest-proof",
                            &digest,
                            "honest-proof.norito",
                        ),
                        kind: "honest-proof".to_owned(),
                        sha256_hex: digest,
                        size_bytes: 1,
                    });
                    candidate
                        .artifacts
                        .sort_unstable_by(|left, right| left.path.cmp(&right.path));
                }
                8 => candidate.artifacts[0].size_bytes = MAX_TOTAL_ARTIFACT_BYTES,
                _ => unreachable!(),
            }
            assert!(
                validate_release_evidence_envelope(&candidate, &policy, "test-fixture").is_err()
            );
        }
    }

    #[test]
    fn semantic_artifact_schema_has_exact_content_addressing_and_proof_bound() {
        assert_eq!(SEMANTIC_ARTIFACT_ROLES.len(), 7);
        assert_eq!(
            semantic_artifact_role("honest-proof"),
            Some(("honest-proof", "honest-proof.norito"))
        );
        assert_eq!(semantic_artifact_role("unknown"), None);
        assert_eq!(
            release_artifact_limit("honest-proof"),
            u64::try_from(SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).ok()
        );
        assert!(
            release_artifact_limit("honest-proof").expect("honest proof kind")
                < release_artifact_limit("honest-witness").expect("honest witness kind")
        );
        let digest = "ab".repeat(32);
        assert_eq!(
            semantic_artifact_path("honest-proof", &digest, "honest-proof.norito"),
            format!("artifacts/semantic/honest-proof/{digest}-honest-proof.norito")
        );
    }

    #[test]
    fn relative_release_paths_are_a_narrow_ascii_subset() {
        assert!(canonical_relative_path(
            "artifacts/lanes/ethereum-mainnet.json"
        ));
        for invalid in [
            "",
            "/absolute",
            "../escape",
            "artifacts//lane",
            "artifacts/./lane",
            "artifacts/../lane",
            "artifacts\\lane",
            "artifacts/UPPERCASE",
            "artifacts/ lane",
        ] {
            assert!(!canonical_relative_path(invalid), "accepted {invalid:?}");
        }
    }
}
