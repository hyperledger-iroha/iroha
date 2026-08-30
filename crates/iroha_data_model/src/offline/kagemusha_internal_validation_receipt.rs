//! Candidate-bound internal release-validation evidence for Kagemusha V4.

use iroha_crypto::{Algorithm, KeyPair, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V2, KagemushaExactBytesDigestV1,
    KagemushaReviewedTrackedCargoLockV2, kagemusha_recursive_spend_qualified_candidate_sha256_v4,
};

/// Exact schema identifier for a candidate-bound internal-validation receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.internal_validation_receipt.v1";
/// Canonical release-bundle file name for the internal-validation receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1: &str =
    "internal-validation-receipt-v1.norito";
/// Current internal-validation receipt version.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_VERSION_V1: u16 = 1;
/// Domain separator embedded in every signed V1 validation body.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_SIGNATURE_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-internal-validation:v1";
/// Hash domain binding one validation-runner public key to its executable bytes.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RUNNER_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:recursive-spend-internal-validation-runner:v1\0";
/// Maximum canonical Norito bytes accepted for one internal-validation receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1: usize = 1024 * 1024;
/// Exact minimum completed executions required from each mandatory fuzz target.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1: u64 = 10_000_000;
/// Maximum bytes accepted in a host or target triple.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_TRIPLE_BYTES_V1: usize = 128;
/// Maximum bytes accepted in one command identifier.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_COMMAND_ID_BYTES_V1: usize = 128;
/// Maximum arguments accepted for one command.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_ARGUMENTS_V1: usize = 32;
/// Maximum UTF-8 bytes accepted in one command argument.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_ARGUMENT_BYTES_V1: usize = 4096;

/// One executable role in the closed internal-validation toolchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "role", content = "value", rename_all = "snake_case")]
pub enum KagemushaInternalValidationToolRoleV1 {
    /// Cargo executable used for every Rust workspace command.
    Cargo,
    /// Rust compiler selected by Cargo.
    Rustc,
    /// Rust documentation compiler from the same toolchain.
    Rustdoc,
    /// Cargo's Clippy subcommand executable.
    CargoClippy,
    /// Clippy compiler driver selected by `cargo clippy`.
    ClippyDriver,
    /// Cargo-fuzz executable selected by the two fuzz commands.
    CargoFuzz,
    /// Authenticated runner that captured outcomes and emitted this receipt.
    ValidationRunner,
}

/// Exact executable and version-report identity for one validation tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaInternalValidationToolV1 {
    /// Closed semantic role served by the executable.
    pub role: KagemushaInternalValidationToolRoleV1,
    /// Exact executable byte identity.
    pub executable: KagemushaExactBytesDigestV1,
    /// Exact nonempty verbose-version output retained by the runner.
    pub version_output: KagemushaExactBytesDigestV1,
}

/// Portable working-directory identity for one validation command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "directory", content = "value", rename_all = "snake_case")]
pub enum KagemushaInternalValidationWorkingDirectoryV1 {
    /// Root of the immutable candidate source snapshot.
    SourceRoot,
}

/// Mandatory fuzz surface exercised by internal validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "target", content = "value", rename_all = "snake_case")]
pub enum KagemushaInternalValidationFuzzTargetV1 {
    /// Bounded canonical release-bundle and receipt parsing.
    Parser,
    /// Recursive-spend parent/topology validation.
    Topology,
}

/// Exact outcome retained for one mandatory fuzz campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaInternalValidationFuzzOutcomeV1 {
    /// Closed fuzz surface exercised by this campaign.
    pub target: KagemushaInternalValidationFuzzTargetV1,
    /// Policy threshold in force when the campaign ran.
    pub minimum_executions: u64,
    /// Executions completed by the fuzz engine.
    pub completed_executions: u64,
    /// Crashing inputs observed by the campaign; production requires zero.
    pub crashes: u64,
    /// Inputs terminated by the per-input timeout; production requires zero.
    pub timeouts: u64,
    /// Inputs terminated for memory exhaustion; production requires zero.
    pub out_of_memory: u64,
    /// Exact canonical initial-corpus archive.
    pub initial_corpus: KagemushaExactBytesDigestV1,
    /// Exact canonical final-corpus archive.
    pub final_corpus: KagemushaExactBytesDigestV1,
    /// Exact fuzz-engine summary retained by the runner.
    pub engine_report: KagemushaExactBytesDigestV1,
}

/// Exact command invocation and terminal outcome retained by validation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaInternalValidationCommandOutcomeV1 {
    /// Zero-based position in the mandatory validation plan.
    pub ordinal: u16,
    /// Stable portable identifier of the mandatory command.
    pub command_id: String,
    /// Authenticated executable role launched for this command.
    pub program: KagemushaInternalValidationToolRoleV1,
    /// Exact argument vector, excluding the executable path.
    pub argv: Vec<String>,
    /// Logical immutable working directory.
    pub working_directory: KagemushaInternalValidationWorkingDirectoryV1,
    /// Exact canonical closed-environment manifest.
    pub environment_manifest: KagemushaExactBytesDigestV1,
    /// Process exit code; production requires zero.
    pub exit_code: i32,
    /// Terminating signal, if any; production requires none.
    pub termination_signal: Option<u16>,
    /// Whether the command exceeded its outer deadline; production requires false.
    pub timed_out: bool,
    /// Exact nonempty canonical stdout/stderr frame archive.
    pub log_archive: KagemushaExactBytesDigestV1,
    /// Campaign counters for a fuzz command and `None` for every other command.
    pub fuzz: Option<KagemushaInternalValidationFuzzOutcomeV1>,
}

/// Immutable code-level specification for one command in the V1 validation plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaInternalValidationCommandSpecV1 {
    /// Stable receipt identifier.
    pub command_id: &'static str,
    /// Executable role that must be launched.
    pub program: KagemushaInternalValidationToolRoleV1,
    /// Exact argument vector excluding the executable path.
    pub argv: &'static [&'static str],
    /// Required fuzz target, if the command is a fuzz campaign.
    pub fuzz_target: Option<KagemushaInternalValidationFuzzTargetV1>,
}

const CARGO: KagemushaInternalValidationToolRoleV1 = KagemushaInternalValidationToolRoleV1::Cargo;
const SOURCE_ROOT: KagemushaInternalValidationWorkingDirectoryV1 =
    KagemushaInternalValidationWorkingDirectoryV1::SourceRoot;

/// Exact, ordered V1 command plan required for an accepted receipt.
pub const KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1:
    &[KagemushaInternalValidationCommandSpecV1] = &[
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-kagemusha-v4",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "kagemusha_v4",
            "--lib",
            "--tests",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-offline-schema-golden",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "--test",
            "iroha_data_model_group_02",
            "offline_public_schema_golden",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-canary",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "--lib",
            "--features",
            "transparent_api",
            "canary_",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-post-canary-wire-splices",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "--lib",
            "--features",
            "transparent_api",
            "post_canary_liveness_rejects_receipt_and_transaction_wire_anchor_splices",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-post-canary-validator-liveness",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "--lib",
            "--features",
            "transparent_api",
            "kagemusha_post_canary_validator_liveness",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "data-model-receiver-snapshot",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_data_model",
            "receiver_snapshot",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-kagemusha-v4",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "kagemusha_v4",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-taira-canary",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "--lib",
            "taira_canary",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-autonomous-merge-admission-intent",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "--lib",
            "autonomous_merge_admission_intent_",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-attestation-certificate-validation",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "--lib",
            "attestation_certificate_validation_tests",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-offline-device-attestation-policy",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "offline_device_attestation_policy",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-device-registration",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "device_registration_",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-kagemusha-online-registration",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "kagemusha_online_registration_",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-active-receiver-snapshot",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "active_receiver_snapshot_",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-final-release-inventory",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "--features",
            "dev-tools,zk-halo2-ipa,kagemusha-candidate-evidence-lab",
            "--bin",
            "kagemusha_recursive_spend_v4_bundle",
            "final_release_inventory_is_exact_and_includes_both_receipts",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-sparse-confidential-subtree-roots",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "sparse_confidential_subtree_roots_match_dense_reference",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-next-zero-confidential-path",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "next_zero_confidential_path_matches_padded_tree_path",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-sequential-append-paths",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "sequential_append_paths",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-recursive-state-vector",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "recursive_state_vector_is_exact_and_zero_padded",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-output-membership",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "output_membership",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-v4-eq-frontier-copy-constraints",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "v4_eq_frontier_copy_constraints",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-v4-manifest-state-limbs",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "v4_manifest_preserves_exact_little_endian_state_limbs",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-v4-shared-result-frontier",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "v4_eq_and_ep_public_columns_share_the_v2_result_frontier_limb",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "core-kagemusha-terminal-registry-v4",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "kagemusha_terminal_registry_v4",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "kagami-harden-private-tree",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_kagami",
            "--bin",
            "kagami",
            "harden_private_tree",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "kagami-private-custody-readme",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_kagami",
            "--bin",
            "kagami",
            "private_custody_readme_invokes_non_executable_scripts_through_bash",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "kagami-raw-npos-genesis-seed",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_kagami",
            "--bin",
            "kagami",
            "raw_npos_genesis_receives_the_chain_bound_localnet_epoch_seed",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "kagami-atomic-activation",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_kagami",
            "--bin",
            "kagami",
            "atomic_activation_",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "kagami-backing",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_kagami",
            "--bin",
            "kagami",
            "backing_",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-shared-offline-api",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii_shared",
            "offline_api",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-generated-spec-strict-offline-schemas",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "generated_spec_documents_strict_typed_offline_request_schemas_and_states",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-generated-spec-offline",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "generated_spec_offline",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-generated-spec-lifecycle",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "generated_spec_matches_offline_negotiation_and_operation_lifecycle",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-bridge-finality-attestation-routes",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "--lib",
            "bridge_finality_attestation_route_tests",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-readiness-authenticates-release",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "readiness_authenticates_exact_release_without_global_backend_flag",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-v4-snapshot-authenticates-release",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "torii-offline-commands",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_torii",
            "offline_commands",
            "--lib",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "config-settlement-offline",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_config",
            "settlement_offline_tests",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "config-torii-kagemusha-commands",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_config",
            "torii_kagemusha_commands_tests",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "connect-bridge-recursive-spend-v4",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "connect_norito_bridge",
            "recursive_spend_v4",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "connect-bridge-output-membership-carrier",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "connect_norito_bridge",
            "output_membership_local_carrier",
            "--lib",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "cli-kagemusha-rollout",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "iroha_cli",
            "--bin",
            "iroha",
            "kagemusha_rollout",
            "--",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "workspace-tests",
        program: CARGO,
        argv: &["test", "--locked", "--workspace"],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "workspace-strict-clippy",
        program: CARGO,
        argv: &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "connect-bridge-production-release-kat",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "connect_norito_bridge",
            "--features",
            "privacy-production-enabled",
            "--lib",
            "kagemusha_bridge_tests::recursive_spend_v4_production_feature_installs_and_executes_real_release",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
        ],
        fuzz_target: None,
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "fuzz-release-bundle-parser",
        program: CARGO,
        argv: &[
            "fuzz",
            "run",
            "kagemusha_v4_release_bundle_parser",
            "--",
            "-runs=10000000",
            "-max_len=1048576",
            "-timeout=10",
            "-rss_limit_mb=2048",
        ],
        fuzz_target: Some(KagemushaInternalValidationFuzzTargetV1::Parser),
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "fuzz-recursive-topology",
        program: CARGO,
        argv: &[
            "fuzz",
            "run",
            "kagemusha_v4_recursive_topology",
            "--",
            "-runs=10000000",
            "-max_len=262144",
            "-timeout=10",
            "-rss_limit_mb=2048",
        ],
        fuzz_target: Some(KagemushaInternalValidationFuzzTargetV1::Topology),
    },
    KagemushaInternalValidationCommandSpecV1 {
        command_id: "four-validator-activation-restart-replay",
        program: CARGO,
        argv: &[
            "test",
            "--locked",
            "-p",
            "integration_tests",
            "--test",
            "kagemusha_v4_release_activation",
            "kagemusha_v4_four_validator_activation_restart_replay",
            "--",
            "--exact",
            "--nocapture",
        ],
        fuzz_target: None,
    },
];

const REQUIRED_TOOL_ROLES: [KagemushaInternalValidationToolRoleV1; 7] = [
    KagemushaInternalValidationToolRoleV1::Cargo,
    KagemushaInternalValidationToolRoleV1::Rustc,
    KagemushaInternalValidationToolRoleV1::Rustdoc,
    KagemushaInternalValidationToolRoleV1::CargoClippy,
    KagemushaInternalValidationToolRoleV1::ClippyDriver,
    KagemushaInternalValidationToolRoleV1::CargoFuzz,
    KagemushaInternalValidationToolRoleV1::ValidationRunner,
];

/// Signed statement that the complete internal V1 release plan passed.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRecursiveSpendInternalValidationReceiptBodyV1 {
    /// Cross-protocol replay separator covered by the runner signature.
    pub signature_domain: String,
    /// Ed25519 validation-runner key that signs this exact body.
    pub validation_runner_public_key: PublicKey,
    /// Domain-separated identity of the runner key and exact executable bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub validation_runner_identity_sha256: [u8; 32],
    /// SHA-256 of the exact canonical unsigned candidate record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_sha256: [u8; 32],
    /// SHA-256 of the canonical actual-recursion qualification receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub qualification_receipt_sha256: [u8; 32],
    /// Domain-separated identity of the candidate and qualification receipt.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub qualified_candidate_sha256: [u8; 32],
    /// Lowercase 40-hex signed source commit.
    pub source_commit: String,
    /// Lowercase 40-hex Git tree named by `source_commit`.
    pub source_git_tree: String,
    /// SHA-256 of the exact signed tracked source tree.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub source_tree_sha256: [u8; 32],
    /// Whether the validation source differed from the signed commit; production requires false.
    pub source_repo_dirty: bool,
    /// SHA-256 of the canonical independently reviewed source-closure descriptor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reviewed_source_closure_descriptor_sha256: [u8; 32],
    /// SHA-256 of the authenticated source-seal projection.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub authenticated_source_seal_projection_sha256: [u8; 32],
    /// Exact tracked root `Cargo.lock` descriptor proven against `source_git_tree` by the runner.
    pub tracked_cargo_lock: KagemushaReviewedTrackedCargoLockV2,
    /// Reviewed Cargo executable digest copied from the candidate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reviewed_cargo_binary_sha256: [u8; 32],
    /// Reviewed rustc executable digest copied from the candidate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reviewed_rustc_binary_sha256: [u8; 32],
    /// Sealed candidate-generator executable digest copied from the candidate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub generator_binary_sha256: [u8; 32],
    /// Canonical sealed double-build report digest copied from the candidate.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sealed_candidate_build_report_sha256: [u8; 32],
    /// Exact existing candidate-structure validation report.
    pub candidate_validation_report: KagemushaExactBytesDigestV1,
    /// Host triple reported by the authenticated Rust toolchain.
    pub host_triple: String,
    /// Target triple used by every compiled validation command.
    pub target_triple: String,
    /// Exact authenticated validation-runner executable bytes.
    pub validator_binary: KagemushaExactBytesDigestV1,
    /// Exact canonical manifest of the complete compiler, native-tool, sysroot, and dependency closure.
    pub toolchain_manifest: KagemushaExactBytesDigestV1,
    /// Exactly one identity for every required tool role, in enum order.
    pub tools: Vec<KagemushaInternalValidationToolV1>,
    /// Exact ordered outcomes for the complete closed validation plan.
    pub commands: Vec<KagemushaInternalValidationCommandOutcomeV1>,
}

/// Canonical runner-signed candidate-bound internal-validation receipt.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct KagemushaRecursiveSpendInternalValidationReceiptV1 {
    /// Exact receipt schema identifier.
    pub schema: String,
    /// Receipt layout version.
    pub version: u16,
    /// Complete candidate, source, toolchain, command, and evidence statement.
    pub body: KagemushaRecursiveSpendInternalValidationReceiptBodyV1,
    /// Validation-runner signature over `body`, including its domain and runner identity.
    pub signature: SignatureOf<KagemushaRecursiveSpendInternalValidationReceiptBodyV1>,
}

impl KagemushaRecursiveSpendInternalValidationReceiptV1 {
    /// Sign one structurally valid candidate-bound body with its declared runner.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaInternalValidationReceiptErrorV1`] when the body is
    /// invalid, the supplied Ed25519 key differs from the declared runner, the
    /// signature cannot be created, or the canonical artifact exceeds 1 MiB.
    pub fn try_sign(
        body: KagemushaRecursiveSpendInternalValidationReceiptBodyV1,
        validation_runner: &KeyPair,
    ) -> Result<Self, KagemushaInternalValidationReceiptErrorV1> {
        body.validate()?;
        if validation_runner.public_key() != &body.validation_runner_public_key
            || !matches!(
                validation_runner.public_key().try_algorithm(),
                Ok(Algorithm::Ed25519)
            )
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::SignerMismatch);
        }
        let signature = SignatureOf::try_new(validation_runner.private_key(), &body)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::InvalidSignature)?;
        let receipt = Self {
            schema: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_VERSION_V1,
            body,
            signature,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Decode and verify one exact canonical receipt under cumulative resource limits.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaInternalValidationReceiptErrorV1`] before decoding for
    /// an empty or oversized frame, and after decoding for non-canonical bytes,
    /// an invalid runner signature, or any signed-body policy mismatch.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, KagemushaInternalValidationReceiptErrorV1> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::ReceiptSize {
                actual: bytes.len(),
                maximum: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            });
        }
        let limits = norito::core::DecodeLimits::new(
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_ARGUMENT_BYTES_V1,
            4 * KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            32,
        );
        let receipt: Self = norito::decode_canonical_with_limits(bytes, limits)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptDecode)?;
        receipt.validate()?;
        let canonical = norito::encode_canonical(&receipt)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptEncode)?;
        if canonical != bytes {
            return Err(KagemushaInternalValidationReceiptErrorV1::ReceiptDecode);
        }
        Ok(receipt)
    }

    /// Validate the schema, complete signed body, runner signature, and size ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaInternalValidationReceiptErrorV1`] on any structural,
    /// policy, runner-identity, signature, canonical-encoding, or size failure.
    pub fn validate(&self) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_SCHEMA_V1
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_VERSION_V1
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "receipt",
            ));
        }
        self.body.validate()?;
        self.signature
            .verify(&self.body.validation_runner_public_key, &self.body)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::InvalidSignature)?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptEncode)?;
        if bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1 {
            return Err(KagemushaInternalValidationReceiptErrorV1::ReceiptSize {
                actual: bytes.len(),
                maximum: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
            });
        }
        Ok(())
    }

    /// Return SHA-256 of the exact validated canonical signed receipt bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaInternalValidationReceiptErrorV1`] when validation or
    /// canonical encoding fails.
    pub fn canonical_sha256(&self) -> Result<[u8; 32], KagemushaInternalValidationReceiptErrorV1> {
        self.validate()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptEncode)?;
        Ok(Sha256::digest(bytes).into())
    }
}

impl KagemushaRecursiveSpendInternalValidationReceiptBodyV1 {
    /// Validate every signed-body field and the complete exact V1 command plan.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaInternalValidationReceiptErrorV1`] when a source,
    /// candidate, lock, tool, command, outcome, fuzz threshold, or evidence
    /// identity is missing, malformed, substituted, duplicated, or out of order.
    pub fn validate(&self) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        self.validate_identity()?;
        self.validate_tools()?;
        self.validate_commands()
    }

    fn validate_identity(&self) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        let expected_qualified = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            self.candidate_sha256,
            self.qualification_receipt_sha256,
        );
        let identity_digests = [
            self.candidate_sha256,
            self.qualification_receipt_sha256,
            self.qualified_candidate_sha256,
            self.source_tree_sha256,
            self.reviewed_source_closure_descriptor_sha256,
            self.authenticated_source_seal_projection_sha256,
            self.reviewed_cargo_binary_sha256,
            self.reviewed_rustc_binary_sha256,
            self.generator_binary_sha256,
            self.sealed_candidate_build_report_sha256,
        ];
        let expected_runner_identity = validation_runner_identity_sha256(
            &self.validation_runner_public_key,
            &self.validator_binary,
        )?;
        if self.signature_domain
            != KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_SIGNATURE_DOMAIN_V1
            || !matches!(
                self.validation_runner_public_key.try_algorithm(),
                Ok(Algorithm::Ed25519)
            )
            || self.validation_runner_identity_sha256 != expected_runner_identity
            || identity_digests.contains(&[0; 32])
            || self.candidate_sha256 == self.qualification_receipt_sha256
            || self.qualified_candidate_sha256 != expected_qualified
            || !is_nonzero_lower_hex_40(&self.source_commit)
            || !is_nonzero_lower_hex_40(&self.source_git_tree)
            || self.source_repo_dirty
            || !is_target_triple(&self.host_triple)
            || !is_target_triple(&self.target_triple)
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "identity",
            ));
        }
        validate_exact_bytes(
            &self.candidate_validation_report,
            "candidate_validation_report",
        )?;
        validate_exact_bytes(&self.validator_binary, "validator_binary")?;
        validate_exact_bytes(&self.toolchain_manifest, "toolchain_manifest")?;
        let lock = &self.tracked_cargo_lock;
        if lock.path != "Cargo.lock"
            || lock.git_mode != "100644"
            || !is_nonzero_lower_hex_40(&lock.git_blob_oid)
            || lock.sha256 == [0; 32]
            || lock.size_bytes == 0
            || lock.size_bytes > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V2
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "tracked_cargo_lock",
            ));
        }
        Ok(())
    }

    fn validate_tools(&self) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        if self.tools.len() != REQUIRED_TOOL_ROLES.len() {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "tools.length",
            ));
        }
        for (tool, expected_role) in self.tools.iter().zip(REQUIRED_TOOL_ROLES) {
            if tool.role != expected_role {
                return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                    "tools.order",
                ));
            }
            validate_exact_bytes(&tool.executable, "tools.executable")?;
            validate_exact_bytes(&tool.version_output, "tools.version_output")?;
        }
        if self.tools[0].executable.sha256 != self.reviewed_cargo_binary_sha256
            || self.tools[1].executable.sha256 != self.reviewed_rustc_binary_sha256
            || self.tools[6].executable != self.validator_binary
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "tools.candidate_binding",
            ));
        }
        Ok(())
    }

    fn validate_commands(&self) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        if self.commands.len()
            != KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1.len()
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "commands.length",
            ));
        }
        let mut fuzz_targets = [false; 2];
        for (index, (command, spec)) in self
            .commands
            .iter()
            .zip(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1)
            .enumerate()
        {
            let expected_ordinal = u16::try_from(index).map_err(|_| {
                KagemushaInternalValidationReceiptErrorV1::InvalidField("commands.ordinal")
            })?;
            let argv_matches = command.argv.len() == spec.argv.len()
                && command
                    .argv
                    .iter()
                    .zip(spec.argv)
                    .all(|(actual, expected)| actual == expected);
            if command.ordinal != expected_ordinal
                || command.command_id != spec.command_id
                || command.command_id.len()
                    > KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_COMMAND_ID_BYTES_V1
                || command.program != spec.program
                || command.working_directory != SOURCE_ROOT
                || command.argv.is_empty()
                || command.argv.len()
                    > KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_ARGUMENTS_V1
                || command.argv.iter().any(|argument| {
                    argument.is_empty()
                        || argument.len()
                            > KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_ARGUMENT_BYTES_V1
                        || argument.as_bytes().contains(&0)
                })
                || !argv_matches
                || command.exit_code != 0
                || command.termination_signal.is_some()
                || command.timed_out
            {
                return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                    "commands.plan_or_outcome",
                ));
            }
            validate_exact_bytes(
                &command.environment_manifest,
                "commands.environment_manifest",
            )?;
            validate_exact_bytes(&command.log_archive, "commands.log_archive")?;
            match (spec.fuzz_target, command.fuzz.as_ref()) {
                (None, None) => {}
                (Some(expected), Some(fuzz)) => {
                    fuzz.validate(expected)?;
                    let target_index = match expected {
                        KagemushaInternalValidationFuzzTargetV1::Parser => 0,
                        KagemushaInternalValidationFuzzTargetV1::Topology => 1,
                    };
                    if fuzz_targets[target_index] {
                        return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                            "commands.fuzz.duplicate",
                        ));
                    }
                    fuzz_targets[target_index] = true;
                }
                _ => {
                    return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                        "commands.fuzz",
                    ));
                }
            }
        }
        if fuzz_targets != [true, true] {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "commands.fuzz.missing",
            ));
        }
        Ok(())
    }
}

impl KagemushaInternalValidationFuzzOutcomeV1 {
    fn validate(
        &self,
        expected: KagemushaInternalValidationFuzzTargetV1,
    ) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
        if self.target != expected
            || self.minimum_executions
                != KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1
            || self.completed_executions < self.minimum_executions
            || self.crashes != 0
            || self.timeouts != 0
            || self.out_of_memory != 0
        {
            return Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(
                "commands.fuzz.outcome",
            ));
        }
        validate_exact_bytes(&self.initial_corpus, "commands.fuzz.initial_corpus")?;
        validate_exact_bytes(&self.final_corpus, "commands.fuzz.final_corpus")?;
        validate_exact_bytes(&self.engine_report, "commands.fuzz.engine_report")
    }
}

/// Derive the domain-separated identity of a validation-runner key and executable.
///
/// # Errors
///
/// Returns [`KagemushaInternalValidationReceiptErrorV1`] when the executable
/// identity is empty or the public key cannot be encoded canonically.
pub fn kagemusha_internal_validation_runner_identity_sha256_v1(
    public_key: &PublicKey,
    executable: &KagemushaExactBytesDigestV1,
) -> Result<[u8; 32], KagemushaInternalValidationReceiptErrorV1> {
    validate_exact_bytes(executable, "validator_binary")?;
    validation_runner_identity_sha256(public_key, executable)
}

fn validation_runner_identity_sha256(
    public_key: &PublicKey,
    executable: &KagemushaExactBytesDigestV1,
) -> Result<[u8; 32], KagemushaInternalValidationReceiptErrorV1> {
    let public_key_bytes = norito::encode_canonical(public_key)
        .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptEncode)?;
    let public_key_len = u64::try_from(public_key_bytes.len())
        .map_err(|_| KagemushaInternalValidationReceiptErrorV1::ReceiptEncode)?;
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RUNNER_IDENTITY_DOMAIN_V1);
    hasher.update(public_key_len.to_be_bytes());
    hasher.update(public_key_bytes);
    hasher.update(executable.byte_len.to_be_bytes());
    hasher.update(executable.sha256);
    Ok(hasher.finalize().into())
}

fn validate_exact_bytes(
    value: &KagemushaExactBytesDigestV1,
    field: &'static str,
) -> Result<(), KagemushaInternalValidationReceiptErrorV1> {
    value
        .validate()
        .map_err(|_| KagemushaInternalValidationReceiptErrorV1::InvalidField(field))
}

fn is_nonzero_lower_hex_40(value: &str) -> bool {
    value.len() == 40
        && value.bytes().any(|byte| byte != b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_target_triple(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MAX_TRIPLE_BYTES_V1
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

/// Failure while decoding or validating a Kagemusha internal-validation receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KagemushaInternalValidationReceiptErrorV1 {
    /// One named structural or policy field is invalid.
    #[error("invalid Kagemusha internal-validation receipt field: {0}")]
    InvalidField(&'static str),
    /// Supplied signing key differs from the validation runner declared in the body.
    #[error("Kagemusha internal-validation receipt signer differs from its declared runner")]
    SignerMismatch,
    /// Validation-runner signature is malformed or does not authenticate the body.
    #[error("invalid Kagemusha internal-validation receipt runner signature")]
    InvalidSignature,
    /// Canonical receipt encoding failed.
    #[error("failed to encode the canonical Kagemusha internal-validation receipt")]
    ReceiptEncode,
    /// Bounded canonical receipt decoding failed.
    #[error("failed to decode the canonical bounded Kagemusha internal-validation receipt")]
    ReceiptDecode,
    /// The supplied receipt frame violates the outer byte ceiling.
    #[error("Kagemusha internal-validation receipt is {actual} bytes; maximum is {maximum}")]
    ReceiptSize {
        /// Actual supplied byte length.
        actual: usize,
        /// Maximum accepted byte length.
        maximum: usize,
    },
}

#[cfg(test)]
/// Shared constructors for release-validation tests that need signed internal receipts.
pub mod internal_validation_receipt_test_support {
    use super::super::{
        KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    };
    use super::*;

    fn exact(seed: u8) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1 {
            byte_len: u64::from(seed) + 1,
            sha256: [seed.max(1); 32],
        }
    }

    fn fuzz_outcome(
        target: KagemushaInternalValidationFuzzTargetV1,
        seed: u8,
    ) -> KagemushaInternalValidationFuzzOutcomeV1 {
        KagemushaInternalValidationFuzzOutcomeV1 {
            target,
            minimum_executions:
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1,
            completed_executions:
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1,
            crashes: 0,
            timeouts: 0,
            out_of_memory: 0,
            initial_corpus: exact(seed),
            final_corpus: exact(seed.wrapping_add(1)),
            engine_report: exact(seed.wrapping_add(2)),
        }
    }

    fn validation_runner() -> KeyPair {
        KeyPair::from_seed(vec![0xA7; 32], Algorithm::Ed25519)
    }

    fn valid_body(
        validation_runner: &KeyPair,
    ) -> KagemushaRecursiveSpendInternalValidationReceiptBodyV1 {
        let candidate_sha256 = [1; 32];
        let qualification_receipt_sha256 = [2; 32];
        let reviewed_cargo_binary_sha256 = [8; 32];
        let reviewed_rustc_binary_sha256 = [9; 32];
        let validator_binary = exact(14);
        let tools = REQUIRED_TOOL_ROLES
            .into_iter()
            .enumerate()
            .map(|(index, role)| {
                let mut executable = exact(u8::try_from(index + 8).expect("tool seed fits"));
                if role == KagemushaInternalValidationToolRoleV1::Cargo {
                    executable.sha256 = reviewed_cargo_binary_sha256;
                } else if role == KagemushaInternalValidationToolRoleV1::Rustc {
                    executable.sha256 = reviewed_rustc_binary_sha256;
                } else if role == KagemushaInternalValidationToolRoleV1::ValidationRunner {
                    executable = validator_binary;
                }
                KagemushaInternalValidationToolV1 {
                    role,
                    executable,
                    version_output: exact(u8::try_from(index + 40).expect("version seed fits")),
                }
            })
            .collect();
        let commands = KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
            .iter()
            .enumerate()
            .map(
                |(index, spec)| KagemushaInternalValidationCommandOutcomeV1 {
                    ordinal: u16::try_from(index).expect("command ordinal fits"),
                    command_id: spec.command_id.to_owned(),
                    program: spec.program,
                    argv: spec
                        .argv
                        .iter()
                        .map(|argument| (*argument).to_owned())
                        .collect(),
                    working_directory: SOURCE_ROOT,
                    environment_manifest: exact(
                        u8::try_from(index + 70).expect("environment seed fits"),
                    ),
                    exit_code: 0,
                    termination_signal: None,
                    timed_out: false,
                    log_archive: exact(u8::try_from(index + 130).expect("log seed fits")),
                    fuzz: spec.fuzz_target.map(|target| {
                        fuzz_outcome(target, u8::try_from(index + 190).expect("fuzz seed fits"))
                    }),
                },
            )
            .collect();
        let validation_runner_public_key = validation_runner.public_key().clone();
        KagemushaRecursiveSpendInternalValidationReceiptBodyV1 {
            signature_domain: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_SIGNATURE_DOMAIN_V1
                .to_owned(),
            validation_runner_identity_sha256:
                kagemusha_internal_validation_runner_identity_sha256_v1(
                    &validation_runner_public_key,
                    &validator_binary,
                )
                .expect("derive validation-runner identity"),
            validation_runner_public_key,
            candidate_sha256,
            qualification_receipt_sha256,
            qualified_candidate_sha256: kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                candidate_sha256,
                qualification_receipt_sha256,
            ),
            source_commit: "1111111111111111111111111111111111111111".to_owned(),
            source_git_tree: "2222222222222222222222222222222222222222".to_owned(),
            source_tree_sha256: [3; 32],
            source_repo_dirty: false,
            reviewed_source_closure_descriptor_sha256: [4; 32],
            authenticated_source_seal_projection_sha256: [5; 32],
            tracked_cargo_lock: KagemushaReviewedTrackedCargoLockV2 {
                path: "Cargo.lock".to_owned(),
                git_blob_oid: "3333333333333333333333333333333333333333".to_owned(),
                git_mode: "100644".to_owned(),
                sha256: [6; 32],
                size_bytes: 1024,
            },
            reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256,
            generator_binary_sha256: [10; 32],
            sealed_candidate_build_report_sha256: [11; 32],
            candidate_validation_report: exact(12),
            host_triple: "aarch64-apple-darwin".to_owned(),
            target_triple: "aarch64-apple-darwin".to_owned(),
            validator_binary,
            toolchain_manifest: exact(15),
            tools,
            commands,
        }
    }

    fn valid_receipt() -> KagemushaRecursiveSpendInternalValidationReceiptV1 {
        let validation_runner = validation_runner();
        KagemushaRecursiveSpendInternalValidationReceiptV1::try_sign(
            valid_body(&validation_runner),
            &validation_runner,
        )
        .expect("sign valid receipt")
    }

    /// Build a signed receipt for `candidate` using the workspace Cargo.lock evidence.
    pub fn signed_receipt_for_v4_candidate(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        finalized_manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    ) -> KagemushaRecursiveSpendInternalValidationReceiptV1 {
        signed_receipt_for_v4_candidate_with_tracked_cargo_lock(
            candidate,
            finalized_manifest,
            finalized_manifest
                .reviewed_source_closure
                .ignored_cargo_lock_sha256,
            finalized_manifest
                .reviewed_source_closure
                .ignored_cargo_lock_size_bytes,
        )
    }

    /// Build a signed receipt with an explicitly supplied tracked Cargo.lock binding.
    pub fn signed_receipt_for_v4_candidate_with_tracked_cargo_lock(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        finalized_manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        tracked_cargo_lock_sha256: [u8; 32],
        tracked_cargo_lock_size_bytes: u64,
    ) -> KagemushaRecursiveSpendInternalValidationReceiptV1 {
        let validation_runner = validation_runner();
        let mut body = valid_body(&validation_runner);
        body.candidate_sha256 = candidate.sha256().expect("valid V4 candidate identity");
        body.qualification_receipt_sha256 = finalized_manifest.qualification_receipt_sha256;
        body.qualified_candidate_sha256 = finalized_manifest.qualified_candidate_sha256;
        body.source_commit = finalized_manifest.source_commit.clone();
        body.source_git_tree = finalized_manifest.source_commit.clone();
        body.source_tree_sha256 = finalized_manifest.source_tree_sha256;
        body.source_repo_dirty = finalized_manifest.source_repo_dirty;
        body.reviewed_source_closure_descriptor_sha256 =
            finalized_manifest.reviewed_source_closure_descriptor_sha256;
        body.authenticated_source_seal_projection_sha256 =
            finalized_manifest.authenticated_source_seal_projection_sha256;
        body.tracked_cargo_lock.sha256 = tracked_cargo_lock_sha256;
        body.tracked_cargo_lock.size_bytes = tracked_cargo_lock_size_bytes;
        body.reviewed_cargo_binary_sha256 = finalized_manifest.reviewed_cargo_binary_sha256;
        body.reviewed_rustc_binary_sha256 = finalized_manifest.reviewed_rustc_binary_sha256;
        body.generator_binary_sha256 = finalized_manifest.generator_binary_sha256;
        body.sealed_candidate_build_report_sha256 =
            finalized_manifest.sealed_candidate_build_report_sha256;
        body.tools[0].executable.sha256 = finalized_manifest.reviewed_cargo_binary_sha256;
        body.tools[1].executable.sha256 = finalized_manifest.reviewed_rustc_binary_sha256;
        KagemushaRecursiveSpendInternalValidationReceiptV1::try_sign(body, &validation_runner)
            .expect("sign candidate-bound V4 internal-validation receipt")
    }

    #[test]
    fn exact_plan_contains_current_matrix_and_release_qualification() {
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1.len(),
            48
        );
        for required in [
            "data-model-post-canary-wire-splices",
            "workspace-tests",
            "workspace-strict-clippy",
            "core-final-release-inventory",
            "connect-bridge-production-release-kat",
            "fuzz-release-bundle-parser",
            "fuzz-recursive-topology",
            "four-validator-activation-restart-replay",
        ] {
            assert!(
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
                    .iter()
                    .any(|command| command.command_id == required),
                "missing mandatory command {required}"
            );
        }
        let command = |id| {
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
                .iter()
                .find(|command| command.command_id == id)
                .expect("mandatory command exists")
        };
        assert_eq!(
            command("data-model-post-canary-validator-liveness")
                .argv
                .iter()
                .filter(|argument| **argument == "--nocapture")
                .count(),
            1
        );
        assert_eq!(
            command("torii-generated-spec-offline")
                .argv
                .iter()
                .filter(|argument| **argument == "--locked")
                .count(),
            1
        );
        assert_eq!(
            command("core-final-release-inventory").argv.last(),
            Some(&"final_release_inventory_is_exact_and_includes_both_receipts")
        );
    }

    #[test]
    fn valid_receipt_roundtrips_canonical_bytes() {
        let receipt = valid_receipt();
        receipt.validate().expect("valid receipt");
        let bytes = norito::encode_canonical(&receipt).expect("encode receipt");
        let decoded = KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(&bytes)
            .expect("decode exact receipt");
        assert_eq!(decoded, receipt);
        assert_eq!(
            receipt.canonical_sha256().expect("digest receipt"),
            <[u8; 32]>::from(Sha256::digest(bytes))
        );
    }

    #[test]
    fn bounded_decoder_rejects_empty_oversized_and_noncanonical_bytes() {
        assert!(matches!(
            KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(&[]),
            Err(KagemushaInternalValidationReceiptErrorV1::ReceiptSize { actual: 0, .. })
        ));
        let oversized =
            vec![0_u8; KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1 + 1];
        assert!(matches!(
            KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(&oversized),
            Err(KagemushaInternalValidationReceiptErrorV1::ReceiptSize { .. })
        ));
        let mut bytes = norito::encode_canonical(&valid_receipt()).expect("encode receipt");
        bytes.push(0);
        assert_eq!(
            KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(&bytes),
            Err(KagemushaInternalValidationReceiptErrorV1::ReceiptDecode)
        );
    }

    #[test]
    fn candidate_source_lock_and_tool_substitution_fail_closed() {
        let mut receipt = valid_receipt();
        receipt.body.qualified_candidate_sha256 = [99; 32];
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.source_repo_dirty = true;
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.tracked_cargo_lock.git_mode = "100755".to_owned();
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.tools.swap(0, 1);
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.tools[0].executable.sha256 = [77; 32];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn runner_signature_key_and_binary_substitution_fail_closed() {
        let declared_runner = validation_runner();
        let different_runner = KeyPair::from_seed(vec![0xA8; 32], Algorithm::Ed25519);
        assert_eq!(
            KagemushaRecursiveSpendInternalValidationReceiptV1::try_sign(
                valid_body(&declared_runner),
                &different_runner,
            ),
            Err(KagemushaInternalValidationReceiptErrorV1::SignerMismatch)
        );

        let mut receipt = valid_receipt();
        receipt.body.validation_runner_public_key = different_runner.public_key().clone();
        assert!(matches!(
            receipt.validate(),
            Err(KagemushaInternalValidationReceiptErrorV1::InvalidField(_)
                | KagemushaInternalValidationReceiptErrorV1::InvalidSignature)
        ));

        let mut receipt = valid_receipt();
        receipt.body.validator_binary = exact(0xB1);
        receipt.body.tools[6].executable = receipt.body.validator_binary;
        receipt.body.validation_runner_identity_sha256 =
            kagemusha_internal_validation_runner_identity_sha256_v1(
                &receipt.body.validation_runner_public_key,
                &receipt.body.validator_binary,
            )
            .expect("derive substituted runner identity");
        assert_eq!(
            receipt.validate(),
            Err(KagemushaInternalValidationReceiptErrorV1::InvalidSignature)
        );
    }

    #[test]
    fn exact_command_plan_and_success_outcomes_fail_closed() {
        let mut receipt = valid_receipt();
        receipt.body.commands.pop();
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.commands[0]
            .argv
            .push("--unexpected".to_owned());
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.commands[0].exit_code = 1;
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.commands[0].termination_signal = Some(9);
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.commands[0].log_archive = KagemushaExactBytesDigestV1 {
            byte_len: 0,
            sha256: [0; 32],
        };
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn both_fuzz_targets_require_ten_million_clean_executions() {
        let parser_index = KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
            .iter()
            .position(|command| {
                command.fuzz_target == Some(KagemushaInternalValidationFuzzTargetV1::Parser)
            })
            .expect("parser fuzz command");
        let mut receipt = valid_receipt();
        receipt.body.commands[parser_index]
            .fuzz
            .as_mut()
            .expect("parser outcome")
            .completed_executions -= 1;
        assert!(receipt.validate().is_err());

        let topology_index = KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
            .iter()
            .position(|command| {
                command.fuzz_target == Some(KagemushaInternalValidationFuzzTargetV1::Topology)
            })
            .expect("topology fuzz command");
        let mut receipt = valid_receipt();
        receipt.body.commands[topology_index]
            .fuzz
            .as_mut()
            .expect("topology outcome")
            .crashes = 1;
        assert!(receipt.validate().is_err());

        let mut receipt = valid_receipt();
        receipt.body.commands[topology_index].fuzz = None;
        assert!(receipt.validate().is_err());
    }
}
