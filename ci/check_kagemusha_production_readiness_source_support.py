import ast
import re

if globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1") is not True:
 raise RuntimeError("readiness source-support provider must run inside the authenticated gate")
_readiness_source_support_source = globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1")
if not isinstance(_readiness_source_support_source, str) or not _readiness_source_support_source:
 raise RuntimeError("readiness source-support provider requires its exact loaded bytes")
rp, rq, fb = require_pattern, require, forbid

MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_INCLUDE = 'include!("kagemusha_model.rs");'
MODEL_VERIFIER_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_release_verifier.rs"
MODEL_VERIFIER_MODULE = "mod kagemusha_release_verifier;"
MODEL_PROMOTION_RECEIPT_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_promotion_receipt.rs"
MODEL_PROMOTION_RECEIPT_MODULE = "mod kagemusha_promotion_receipt;"
MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT = (
    "crates/iroha_data_model/src/offline/kagemusha_internal_validation_receipt.rs"
)
MODEL_INTERNAL_VALIDATION_RECEIPT_MODULE = (
    "mod kagemusha_internal_validation_receipt;"
)
MODEL_CANARY_EVIDENCE_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_canary_evidence.rs"
MODEL_CANARY_EVIDENCE_MODULE = "mod kagemusha_canary_evidence;"
MODEL_CANARY_LIVENESS_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_post_canary_validator_liveness.rs"
MODEL_CANARY_LIVENESS_MODULE = "mod kagemusha_post_canary_validator_liveness;"
MODEL_DEVICE_ATTESTATION_CONSTANTS_COMPONENT = "crates/iroha_data_model/src/offline/device_attestation_constants.rs"
MODEL_DEVICE_ATTESTATION_CONSTANTS_INCLUDE = 'include!("device_attestation_constants.rs");'
MODEL_DEVICE_ATTESTATION_POLICY_COMPONENT = "crates/iroha_data_model/src/offline/device_attestation_policy.rs"
MODEL_DEVICE_ATTESTATION_POLICY_INCLUDE = 'include!("device_attestation_policy.rs");'
MODEL_ISI_OFFLINE = "crates/iroha_data_model/src/isi/offline.rs"
MODEL_ISI_MOD = "crates/iroha_data_model/src/isi/mod.rs"
PRIVACY = "crates/iroha_data_model/src/privacy.rs"
PRIVACY_PROTOCOL = "crates/iroha_data_model/src/privacy/protocol.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
CATALOG_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4_release_catalog_impl.rs"
CATALOG_INCLUDE = 'include!("kagemusha_terminal_registry_v4_release_catalog_impl.rs");\n'
CATALOG_VALIDATOR_QUALIFICATION_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4_validator_qualification.rs"
CATALOG_VALIDATOR_QUALIFICATION_INCLUDE = 'include!("kagemusha_terminal_registry_v4_validator_qualification.rs");\n'
QUAL_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4/validator_qualification_tests.rs"
QUAL_TESTS_INCLUDE = 'include!("kagemusha_terminal_registry_v4/validator_qualification_tests.rs");'
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_runtime_effective_config.rs"
CORE_RUNTIME_EFFECTIVE_CONFIG_MODULE = "mod kagemusha_runtime_effective_config;"
CORE_KAGEMUSHA_ACTIVATION_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_activation.rs"
CORE_KAGEMUSHA_ACTIVATION_INCLUDE = 'include!("offline/kagemusha_activation.rs");'
CORE_KAGEMUSHA_CANARY_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_taira_canary.rs"
CORE_KAGEMUSHA_CANARY_INCLUDE = 'include!("offline/kagemusha_taira_canary.rs");'
CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/attestation_certificate_validation.rs"
CORE_ATTESTATION_CERTIFICATE_VALIDATION_INCLUDE = 'include!("offline/attestation_certificate_validation.rs");'
CORE_DEVICE_ATTESTATION_ROOTS_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/device_attestation_roots.rs"
CORE_DEVICE_ATTESTATION_ROOTS_INCLUDE = 'include!("offline/device_attestation_roots.rs");'
CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/attestation_policy_validation.rs"
CORE_ATTESTATION_POLICY_VALIDATION_INCLUDE = 'include!("offline/attestation_policy_validation.rs");'
CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/attestation_certificate_der_profile.rs"
CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_INCLUDE = 'include!("offline/attestation_certificate_der_profile.rs");'
CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/device_attestation_registration_validation.rs"
CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_INCLUDE = 'include!("offline/device_attestation_registration_validation.rs");'
ANDROID_AUTH = "crates/iroha_core/src/smartcontracts/isi/offline/android_attestation_authorization_validation.rs"
ANDROID_AUTH_INCLUDE = 'include!("offline/android_attestation_authorization_validation.rs");'
CORE_ISI_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/isi_tests.rs"
CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/isi_kagemusha_taira_canary_context_tests.rs"
CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE = 'include!("isi_kagemusha_taira_canary_context_tests.rs");'
POLICY_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/isi_attestation_policy_release_tests.rs"
POLICY_TESTS_INCLUDE = 'include!("isi_attestation_policy_release_tests.rs");'
CORE_ISI_TESTS_PARENT_INCLUDE = 'include!("offline/isi_tests.rs");'
CORE_ISI_MOD = "crates/iroha_core/src/smartcontracts/isi/mod.rs"
CORE_TX = "crates/iroha_core/src/tx.rs"
CORE_STATE = "crates/iroha_core/src/state.rs"
CORE_STATE_TESTS = "crates/iroha_core/src/state/tests.rs"
CORE_COMMITTED_TX_CONTEXT = "crates/iroha_core/src/state/committed_transaction_context.rs"
CORE_AUTONOMOUS_MERGE_TESTS = "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs"
CORE_AUTONOMOUS_MERGE_TESTS_PARENT_INCLUDE = 'include!("autonomous_merge_and_queue_plan_tests.rs");'
CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS = "crates/iroha_core/src/state/autonomous_merge_admission_intent_tests.rs"
CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS_INCLUDE = 'include!("autonomous_merge_admission_intent_tests.rs");'
CORE_BLOCK = "crates/iroha_core/src/block.rs"
CORE_EXECUTOR = "crates/iroha_core/src/executor.rs"
STEP_TRANSITION = "crates/iroha_core/src/zk/kagemusha_step_transition.rs"
RECURSIVE_BACKEND = "crates/iroha_core/src/zk/kagemusha_v2.rs"
RECURSION_ADAPTER = "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs"
VALUE_CONTRACT = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
SCHEMA_GOLDEN = "crates/iroha_data_model/tests/offline_public_schema_golden.rs"
CONFIG = "crates/iroha_config/src/parameters/user.rs"
NODE = "crates/irohad/src/main.rs"
NODE_VALIDATOR_QUALIFICATION_COMPONENT = "crates/irohad/src/main/kagemusha_validator_qualification.rs"
NODE_VALIDATOR_QUALIFICATION_MODULE = '#[path = "main/kagemusha_validator_qualification.rs"]\nmod kagemusha_validator_qualification;'
NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT = "crates/irohad/src/main/kagemusha_runtime_effective_config_projection.rs"
NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_MODULE = '#[path = "main/kagemusha_runtime_effective_config_projection.rs"]\nmod kagemusha_runtime_effective_config_projection;'
NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT = "crates/irohad/src/main/kagemusha_validator_qualification_command.rs"
NODE_VALIDATOR_QUALIFICATION_COMMAND_MODULE = '#[path = "main/kagemusha_validator_qualification_command.rs"]\nmod kagemusha_validator_qualification_command;'
NODE_ROOT_OWNED_PUBLICATION_COMPONENT = "crates/irohad/src/main/root_owned_artifact_publication.rs"
NODE_ROOT_OWNED_PUBLICATION_MODULE = '#[path = "main/root_owned_artifact_publication.rs"]\nmod root_owned_artifact_publication;'
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"
AUTHENTICATED_TOOL_CONTROLLER = "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs"
KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT = "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller/kagemusha_promotion_publisher.rs"
KAGEMUSHA_PROMOTION_PUBLISHER_MODULE = '#[path = "iroha_authenticated_tool_controller/kagemusha_promotion_publisher.rs"]\nmod kagemusha_promotion_publisher;'
KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT = "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller/kagemusha_python_launcher.rs"
KAGEMUSHA_PYTHON_LAUNCHER_MODULE = '#[path = "iroha_authenticated_tool_controller/kagemusha_python_launcher.rs"]\nmod kagemusha_python_launcher;'
CLIENT = "crates/iroha/src/client.rs"
HTTP_DEFAULT = "crates/iroha/src/http_default.rs"
CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT = "crates/iroha/src/client/canonical_request_auth.rs"
CLIENT_CANONICAL_REQUEST_AUTH_INCLUDE = 'include!("client/canonical_request_auth.rs");'
OFFLINE_CLI = "crates/iroha_cli/src/offline.rs"
CLI_MAIN_SHARED = "crates/iroha_cli/src/main_shared.rs"
KAGEMUSHA_LIFECYCLE_COMPONENT = "crates/iroha_cli/src/offline/kagemusha_lifecycle.rs"
KAGEMUSHA_LIFECYCLE_MODULE = "mod kagemusha_lifecycle;"
KAGEMUSHA_ROLLOUT_COMPONENT = "crates/iroha_cli/src/offline/kagemusha_rollout.rs"
KAGEMUSHA_ROLLOUT_MODULE = "mod kagemusha_rollout;"
KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT = "crates/iroha_cli/src/offline/kagemusha_rollout/liveness.rs"
KAGEMUSHA_ROLLOUT_LIVENESS_MODULE = "mod liveness;"
STATUS_CAPTURE = "scripts/capture_android_attestation_status.py"
STATUS_CAPTURE_TEST = "scripts/tests/capture_android_attestation_status_test.py"
ANDROID_CERT = "scripts/android_attestation_certificate_profile.py"
ANDROID_CERT_FIX = "scripts/tests/android_attestation_certificate_profile_fixtures.py"
ANDROID_CERT_TEST = "scripts/tests/android_attestation_certificate_profile_test.py"
ANDROID_DEVICE_LAB_SLOT = "scripts/check_android_device_lab_slot.py"
ANDROID_DEVICE_LAB_RUNNER = "scripts/run_kagemusha_candidate_android_lab.sh"
DEVICE_ATTESTATION_SOURCE_PATHS = (
 MODEL_DEVICE_ATTESTATION_CONSTANTS_COMPONENT, MODEL_DEVICE_ATTESTATION_POLICY_COMPONENT,
 CORE_DEVICE_ATTESTATION_ROOTS_COMPONENT, CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
 CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_COMPONENT, CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_COMPONENT, ANDROID_AUTH,
 POLICY_TESTS, QUAL_TESTS, STATUS_CAPTURE, STATUS_CAPTURE_TEST,
 ANDROID_CERT, ANDROID_CERT_FIX, ANDROID_CERT_TEST,
 ANDROID_DEVICE_LAB_SLOT, ANDROID_DEVICE_LAB_RUNNER,
)
KAGEMUSHA_RELEASE_RUST_TEST_FILTERS = (
 "cargo test -p iroha_data_model --lib --features transparent_api kagemusha_v4 -- --nocapture",
 "cargo test -p iroha_data_model --test iroha_data_model_group_02 offline_public_schema_golden -- --nocapture",
 "cargo test -p iroha_data_model --lib --features transparent_api canary_ -- --nocapture",
 "cargo test -p iroha_data_model --lib --features transparent_api post_canary_liveness_rejects_receipt_and_transaction_wire_anchor_splices -- --nocapture",
 "cargo test -p iroha_data_model --lib --features transparent_api kagemusha_post_canary_validator_liveness -- --nocapture",
 "cargo test -p iroha_core --lib kagemusha_ -- --nocapture",
 "cargo test -p iroha_core production_proposal_validation_enforces_kagemusha_runtime_projection --lib -- --nocapture",
 "cargo test -p iroha_core production_commit_apply_enforces_kagemusha_runtime_projection --lib -- --nocapture",
 "cargo test -p iroha_core production_vote_worker_rejects_missing_and_mismatched_kagemusha_projection --lib -- --nocapture",
 "cargo test -p iroha_core production_vote_worker_signs_prepare_and_commit_for_exact_kagemusha_projection --lib -- --nocapture",
 "cargo test -p irohad --bin iroha3d authenticated_snapshot_ -- --nocapture",
 "cargo test -p iroha_kagami --bin kagami kagemusha::tests:: -- --nocapture",
 "cargo test -p iroha_cli --bin iroha kagemusha_lifecycle -- --nocapture",
 "cargo test -p iroha --lib http_default::tests:: -- --nocapture",
 "cargo test -p iroha_core --lib taira_canary -- --nocapture",
 "cargo test -p iroha_core --lib autonomous_merge_admission_intent_ -- --nocapture",
 "cargo test -p iroha_core --lib attestation_certificate_validation_tests -- --nocapture",
 "cargo test -p iroha_torii --lib bridge_finality_attestation_route_tests -- --nocapture",
 "cargo test -p iroha_torii kagemusha_lifecycle -- --nocapture",
 "cargo test -p iroha_torii_shared --lib kagemusha_lifecycle -- --nocapture",
 "cargo test -p iroha_cli --bin iroha kagemusha_rollout -- --nocapture",
)
KAGEMUSHA_RELEASE_PYTHON_TEST_PATHS = (
 "scripts/tests/build_kagemusha_v4_candidate_bundle_test.py",
 "scripts/tests/build_kagemusha_production_ios_policy_test.py",
 "scripts/tests/check_kagemusha_candidate_ios_evidence_test.py",
 "scripts/tests/kagemusha_candidate_ios_lab_source_test.py",
 "scripts/tests/kagemusha_app_attest_freshness_authority_test.py",
 "scripts/tests/kagemusha_production_app_attest_lab_source_test.py",
 "scripts/tests/measure_kagemusha_production_app_attest_bundle_test.py",
 "scripts/tests/sign_kagemusha_production_ios_evidence_test.py",
 "scripts/tests/kagemusha_source_tree_seal_test.py",
 "scripts/tests/produce_kagemusha_v4_source_seal_projection_test.py",
 "scripts/tests/kagemusha_staged_resource_guard_test.py",
 "scripts/tests/stage_kagemusha_candidate_android_artifacts_test.py",
 "scripts/tests/stage_kagemusha_candidate_android_lab_test.py",
 STATUS_CAPTURE_TEST, ANDROID_CERT_TEST,
 "pytests/scripts/run_kagemusha_v4_generation_test.py",
 "pytests/scripts/run_kagemusha_v4_generation_benchmark_test.py",
)

D_RECORD_CANARY = ('Record canary',)
D_WIRE_BOUNDARY = ('signed canary wire',)
D_CANARY_MARKER = ('activation-bound exact reservation',)
D_CANARY_EXEC = ('one-shot canary execution',)
D_BLOCK_PROOF = ('full canary wire block proof',)
D_EXPECTATIONS = ('expectations provenance',)
D_ROLLOUT_PHASES = ('eight phase rollout',)
D_FINAL_RECEIPT = ('no-replace final receipt',)
D_EXACT_PERMIT = ('fresh exact permit Record',)
D_POST_RECEIPT = ('post-receipt promotion',)
D_STRUCTURAL = ('expired-journal reconciliation',)
D_CANARY_JOURNAL = ('private canary journal',)
D_VALIDATOR_TIPS = ('shared canary-rooted finality',)
D_ROOT_TCB = ('root-TCB pinned',)
D_RESERVATION = ('exact-call reservation',)
D_ROSTER = ('four-unit runtime roster',)
D_KAGAMI_EXEC = ('Kagami verifier execution',)
D_PRECOMMIT = ('controller-signed permit',)
D_PROOF_CHAIN = ('contiguous post-receipt finality',)
D_ROOT_PUBLISH = ('root-owned no-replace',)
D_SAFE_RESUME = ('matching-journal safe-resume',)
D_CATALOG_HISTORY = ('catalog validator boundary',)
D_VALIDATION_SIGN = ('validation before signing',)
D_ROLLOUT_CLI = ('rollout-v4 offline CLI',)
D_PROMOTION_PUBLISH = ('promotion-record publication',)
D_STATUS_IDENTITY = ('status identity uncertainty',)
D_RECEIPT_PATH = ('fixed receipt path',)
D_FINAL_RECHECK = ('final signing recheck',)
D_WRITE_AUTH = ('explicit write authorization',)
D_SUBMIT_UNCERTAIN = ('submission uncertainty',)
D_EXPECTATIONS_JOURNAL = ('signed expectations journal',)
D_FINALITY_CORRIDOR = ('four-validator DA Nexus',)
D_DEFERRED_EXPECTATIONS = ('deferred signing',)
D_CATALOG_SIGNATURE = ('catalog authority signature',)
D_BOUNDED_DESCRIPTOR = ('descriptor validation',)
D_ROOT_READ = ('no-follow root-owned read',)
D_BLOCK_ENTRYPOINT = ('block entrypoint proof',)
D_ANDROID_STATUS = ('Android status freshness',)
D_STRICT_X509 = ('strict raw X.509 DER',)
D_ANDROID_CHAIN = ('Factory/RKP time profiles',)
D_ANDROID_CAPTURE = ('status capture',)
CAC, KRC, KRL, MCE = (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
 KAGEMUSHA_ROLLOUT_COMPONENT, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
 MODEL_CANARY_EVIDENCE_COMPONENT)
KCT, CAP, AMT = (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
 CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
 CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS)
CDP, CDR, CRE, MCL, MDC, AMG = (
 CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_COMPONENT,
 CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_COMPONENT,
 CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, MODEL_CANARY_LIVENESS_COMPONENT,
 MODEL_DEVICE_ATTESTATION_CONSTANTS_COMPONENT, CORE_AUTONOMOUS_MERGE_TESTS)
MDP, CDRT, DLS, CVQ, CKC, NRP = (
 MODEL_DEVICE_ATTESTATION_POLICY_COMPONENT, CORE_DEVICE_ATTESTATION_ROOTS_COMPONENT,
 ANDROID_DEVICE_LAB_SLOT, CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
 CORE_KAGEMUSHA_CANARY_COMPONENT, NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT)


def runtime_projection_source_errors(
 cpr: str, wrapper: str, node: str, catalog: str, model: str
) -> list[str]:
 e: list[str] = []
 vc = node.split("fn validate_config_for_check_mode(", 1)[-1].split(
  "fn continue_after_full_kagemusha_check", 1
 )[0]
 rq(model, MODEL, e, "pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;")
 rp(
  wrapper, NRP, e,
  (r"pub fn build_kagemusha_runtime_effective_config_projection_v1\(\s*"
   r"config: &Config,\s*genesis: &GenesisBlock,\s*bootstrap: &GenesisV2Bootstrap,\s*"
   r"\) -> Result<VerifiedKagemushaV4RuntimeEffectiveConfigV1, String> \{\s*"
   r"VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive\(config, genesis, bootstrap\)\s*\}"),
  "thin verified runtime-projection wrapper",
 )
 fb(
  wrapper, NRP, e,
  "KagemushaV4RuntimeEffectiveConfigProjectionV1", "NodeRole", "signed_genesis_validator_pops",
 )
 rp(
  cpr, CRE, e,
  (r"pub struct VerifiedKagemushaV4RuntimeEffectiveConfigV1 \{\s*"
   r"projection: KagemushaV4RuntimeEffectiveConfigProjectionV1,\s*\}.*?"
   r"pub fn derive\(\s*config: &Config,\s*genesis: &GenesisBlock,\s*"
   r"bootstrap: &GenesisV2Bootstrap,\s*\).*?"
   r"let metadata = exact_signed_consensus_metadata\(genesis\)\?;.*?"
   r"let context = bootstrap\.context\(\);.*?"
   r"let staged_pops = bootstrap\.proofs_of_possession\(\);"),
  "opaque Core runtime-effective projection derivation",
 )
 rq(
  cpr, CRE, e,
  "metadata.mode != SumeragiConsensusMode::Permissioned",
  "context.mode != ConsensusMode::Permissioned",
  "metadata.sumeragi_v2 != staged_parameters",
  "context.roster.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT",
  "staged_pops.len() != context.roster.len()",
  "context.roster.iter().any(|member| member.power != 1)",
  "config.sumeragi.role != NodeRole::Validator",
  "validator_pops.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT",
 )
 rq(
  cpr, CRE, e,
  "signed.len() == staged_pops.len()",
  "signed_id == staged_id && signed_pop == staged_pop",
  "trusted.pops.len() == validator_pops.len()",
  "trusted.pops.get(validator_id.public_key()) == Some(pop)",
  "bls_normal_pop_verify(validator_id.public_key(), pop).is_ok()",
  "configured_validators != validator_ids",
  "|| !exact_pops",
 )
 rq(
  cpr, CRE, e,
  "Duration::from_millis(metadata.block_cadence_ms.get())",
  "if validator_id == local_id {",
  "config.network.public_address.value().clone()",
  ".v2_config(block_cadence, mode)",
  "projection.validate().map_err(|error| error.to_string())?;",
  "Ok(Self { projection })",
 )
 rp(
  cpr, CRE, e,
  (r"fn exact_signed_consensus_metadata\(.*?genesis\.0\.external_transactions\(\).*?"
   r"custom\.id\(\) != &consensus_metadata::handshake_meta_id\(\).*?"
   r"metadata\s*\.validate\(\).*?found\.replace\(metadata\)\.is_some\(\).*?"
   r"found\.ok_or_else"),
  "unique validated signed consensus metadata",
 )
 rp(
  vc, NODE, e,
  (r"let full_validation\s*=\s*validate_available_genesis_for_check\("
   r"config,\s*genesis,\s*catalog\);\s*artifacts\.validator_seal\s*=\s*"
   r"continue_after_full_kagemusha_check\(\s*full_validation,\s*"
   r"\|\(validated_genesis,\s*_block_cadence_ms\)\|\s*\{.*?"
   r"let runtime_effective_config\s*=\s*kagemusha_runtime_effective_config_projection::"
   r"build_kagemusha_runtime_effective_config_projection_v1\(\s*"
   r"config,\s*genesis,\s*&validated_genesis,\s*\).*?"
   r"try_build_kagemusha_validator_qualification_v1\(.*?"
   r"Some\(&runtime_effective_config\),"),
  "validation before signing",
 )
 builder = "build_kagemusha_runtime_effective_config_projection_v1("
 if node.count(builder) != 2 or vc.count(builder) != 1:
  e.append(f"{NODE}: runtime-effective projection must be built once inside full validation")
 ps = catalog.split(
  "fn build_and_sign_validator_qualification_seal_v1(", 1
 )[-1].split("\nfn validate_exact_kagemusha_promotion_sources_v1(", 1)[0]
 rq(
  ps, CVQ, e,
  "runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1",
  "runtime_effective_config.projection()",
 )
 if ps.count(
  "runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1"
 ) != 2:
  e.append(f"{CVQ}: production signers require verified runtime config")
 fb(
  ps, CVQ, e,
  "runtime_effective_config: &KagemushaV4RuntimeEffectiveConfigProjectionV1",
 )
 rp(
  node, NODE, e,
  (r"fn continue_after_full_kagemusha_check<T,\s*V>\(\s*"
   r"full_validation:\s*ReportResult<V,\s*MainError>,\s*"
   r"action:\s*impl FnOnce\(V\)\s*->\s*ReportResult<T,\s*MainError>,\s*"
   r"\)\s*->\s*ReportResult<T,\s*MainError>\s*\{\s*"
   r"action\(full_validation\?\)\s*\}"),
  "validation result gate",
 )
 return e


def attestation_certificate_source_errors(attestation: str, core: str) -> list[str]:
 e: list[str] = []
 rq(core, CORE, e, CORE_ATTESTATION_CERTIFICATE_VALIDATION_INCLUDE)
 fb(
  attestation, CAC, e, "#[ignore]"
 )
 rp(
  attestation, CAC, e,
  (r"fn parse_x509_certificate_der\(.*?"
   r"OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES.*?"
   r"X509Certificate::from_der.*?!remaining\.is_empty\(\).*?"
   r"validate_x509_certificate_signature_algorithm\(&certificate\)\?"),
  "bounded DER parsing with strict signature algorithm validation",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_x509_certificate_signature_algorithm\(.*?"
   r"tbs_certificate\.signature != certificate\.signature_algorithm.*?"
   r"x509_certificate_signature_oid_is_weak.*?prohibited weak signature algorithm.*?"
   r"1\.2\.840\.113549\.1\.1\.10.*?"
   r"validate_x509_rsa_pss_signature_algorithm\(signature_algorithm\)\?;"),
  "strict certificate signature algorithm profile",
 )
 rq(
  attestation, CAC, e,
  "1.2.840.113549.1.1.2", "1.2.840.113549.1.1.4", "1.2.840.113549.1.1.5",
  "1.2.840.10040.4.3", "1.2.840.10045.4.1",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_x509_rsa_pss_signature_algorithm\(.*?"
   r"for expected_tag in \[0, 1, 2\].*?"
   r"RsaSsaPssParams::try_from.*?hash_algorithm_oid.*?"
   r"mask_gen_algorithm_raw.*?1\.2\.840\.113549\.1\.1\.8.*?"
   r"mask_hash_oid\.to_string\(\) != hash_oid.*?"
   r"salt_length\(\) != expected_salt_length.*?trailer_field\(\) != 1"),
  "strict RSA-PSS verifier parameter profile",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_x509_certificate_signature_algorithm\(.*?"
   r"1\.2\.840\.113549\.1\.1\.10.*?"
   r"validate_x509_rsa_pss_signature_algorithm\(signature_algorithm\)\?;"),
  "strict RSA-PSS verifier parameter profile",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_x509_certificate_critical_extensions\(.*?"
   r"seen_extension_oids\.insert\(extension_oid\.clone\(\)\).*?"
   r"2\.5\.29\.30.*?2\.5\.29\.32.*?2\.5\.29\.33.*?2\.5\.29\.36.*?2\.5\.29\.54.*?"
   r"extension\.critical.*?ParsedExtension::BasicConstraints.*?"
   r"ParsedExtension::KeyUsage"),
  "strict certificate extension processing",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_x509_leaf_certificate_profile\(.*?"
   r"basic_constraints\.value\.ca.*?path_len_constraint\.is_some.*?"
   r"!key_usage\.critical.*?!key_usage\.value\.digital_signature\(\).*?"
   r"key_usage\.value\.key_cert_sign\(\)"),
  "explicit end-entity certificate profile",
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_attestation_certificate_chain\(.*?"
   r"certificate_chain\.len\(\) > OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES.*?"
   r"for root_der in trusted_roots_der \{.*?tail_der != root_der.*?return Ok\(\(\)\);.*?"
   r"for root_der in trusted_roots_der \{.*?tail\.issuer\(\) != root\.subject\(\).*?"
   r"if let Err\(error\) = verify_x509_certificate_signature\(tail, &root\).*?continue;.*?"
   r"if let Err\(error\) = validate_x509_ca_path_len_constraint.*?continue;.*?return Ok"),
  "order-independent exact-pinned trust-anchor validation",
 )
 rp(
  attestation, CAC, e,
  (r"fn x509_root_nearest_unique_extension_value\(.*?"
   r"certificate_chain\.iter\(\)\.enumerate\(\)\.rev\(\).*?"
   r"fn android_keymint_leaf_attestation_extension\(.*?"
   r"OFFLINE_ATTESTATION_ANDROID_KEY_OID.*?certificate_index != 0"),
  "root-nearest Android KeyMint extension selection",
 )
 rp(
  core, CORE, e,
  (r"OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES: usize = 16 \* 1024.*?"
   r"OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES: usize = 8.*?"
   r"OFFLINE_ATTESTATION_MAX_IOS_X509_CHAIN_CERTIFICATES: usize = 4.*?"
   r"android_keymint_leaf_attestation_extension\(&report\.certificates\)"),
  "bounded Apple and Android certificate inputs",
 )
 for test_name in (
  "same_subject_trust_anchor_rollover_is_order_independent",
  "parsed_critical_and_path_processing_extensions_fail_closed",
  "android_keymint_uses_only_a_directly_attested_leaf_extension",
  "certificate_signature_algorithm_identifiers_must_match_exactly",
  "weak_certificate_signature_algorithm_oids_are_rejected",
  "rsa_pss_signature_parameters_must_match_verifier_profile",
  "certificate_validity_preserves_exact_millisecond_boundaries",
  "android_factory_validity_rejects_first_millisecond_after_not_after",
 ):
  rp(
   attestation, CAC, e,
   rf"#\[test\]\s*fn {test_name}\(\)", f"active {test_name} regression",
  )
 return e


def device_attestation_governance_source_errors(texts: dict[str, str]) -> list[str]:
 e: list[str] = []
 model = texts[MODEL]
 core = texts[CORE]
 constants = texts[MDC]
 policy_model = texts[MDP]
 roots = texts[CDRT]
 policy = texts[CAP]
 der = texts[CDP]
 attestation = texts[CAC]
 android_auth = texts[ANDROID_AUTH]
 registration = texts[CDR]
 activation = texts[CORE_KAGEMUSHA_ACTIVATION_COMPONENT]
 qualification = texts[CATALOG]
 qual_tests = texts[QUAL_TESTS]
 pt = texts[POLICY_TESTS]
 capture = texts[STATUS_CAPTURE]
 cp = texts[ANDROID_CERT]
 cert_fixtures = texts[ANDROID_CERT_FIX]
 cts = texts[ANDROID_CERT_TEST]
 al = texts[DLS]
 runner = texts[ANDROID_DEVICE_LAB_RUNNER]

 for parent, parent_path, marker, component_path in (
  (model, MODEL, MODEL_DEVICE_ATTESTATION_CONSTANTS_INCLUDE,
   MDC),
  (model, MODEL, MODEL_DEVICE_ATTESTATION_POLICY_INCLUDE,
   MDP),
  (core, CORE, CORE_DEVICE_ATTESTATION_ROOTS_INCLUDE,
   CDRT),
  (core, CORE, CORE_ATTESTATION_POLICY_VALIDATION_INCLUDE,
   CAP),
  (core, CORE, CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_INCLUDE,
   CDP),
  (core, CORE, CORE_ATTESTATION_CERTIFICATE_VALIDATION_INCLUDE,
   CAC),
  (core, CORE, CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_INCLUDE,
   CDR),
  (core, CORE, ANDROID_AUTH_INCLUDE, ANDROID_AUTH),
 ):
  if parent.count(marker) != 1:
   e.append(
    f"{parent_path}: expected exactly one authenticated {component_path} include"
   )
 if qualification.count(QUAL_TESTS_INCLUDE) != 1:
  e.append(
   f"{CATALOG}: expected exactly one authenticated "
   f"{QUAL_TESTS} include"
  )

 rp(
  pt, POLICY_TESTS, e,
  (r"#\[test\]\s*fn release_activation_device_policy_is_production_and_fail_closed\(\).*?"
   r"expect_err\(\"activation must pin the exact Apple App Attest root\"\).*?"
   r"for platform in \[.*?IOS_APP_ATTEST.*?ANDROID_KEYMINT.*?\].*?"
   r"activation must require every exact production root to be governance-active.*?"
   r"#\[test\]\s*fn production_device_policy_constructor_binds_explicit_apps_and_builtin_roots\(\).*?"
   r"assert_eq!\(policy\.trusted_roots\.len\(\), 3\)"),
  "active exact built-in release-root policy regressions",
 )
 rp(
  qual_tests, QUAL_TESTS, e,
  (r"#\[test\]\s*fn validator_qualification_freshness_is_bounded_at_the_signing_clock\(\).*?"
   r"cache_max_age_seconds.*?expires - 1.*?inclusive millisecond before expiry.*?"
   r"boundary_policy,.*?expires,.*?\.is_err\(\).*?"
   r"first stale status millisecond must fail closed"),
  "active qualification Android-status +1ms freshness boundary regression",
 )
 fb(
  pt + cts,
  "device attestation release regressions", e, "#[ignore]", "#[cfg",
 )

 rq(
  constants, MDC, e,
  '"https://android.googleapis.com/attestation/status"',
  "OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1: u16 = 1",
  "OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1: usize = 4_096",
  "OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1: usize = 40",
  "OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1: u32 = 86_400",
  "OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024",
 )
 rp(
  policy_model, MDP, e,
  (r"pub struct OfflineAndroidAttestationStatusSnapshotV1\s*\{.*?"
   r"version: u16.*?payload_sha256: \[u8; 32\].*?response_date_ms: u64.*?"
   r"last_modified_ms: Option<u64>.*?cache_max_age_seconds: u32.*?"
   r"non_valid_serials: Vec<String>.*?pub struct OfflineDeviceAttestationPolicy\s*\{.*?"
   r"revoked_certificate_tbs_sha256: Vec<Vec<u8>>.*?"
   r"android_status_snapshot: Option<OfflineAndroidAttestationStatusSnapshotV1>"),
  "governed Android status snapshot and TBSCertificate revocation model",
 )
 rq(
  roots, CDRT, e,
  "APPLE_APP_ATTESTATION_ROOT_CA_DER_B64",
  "ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64",
  "ANDROID_KEY_ATTESTATION_CA_DER_B64",
 )
 rp(
  policy, CAP, e,
  (r"if let Some\(snapshot\) = &policy\.android_status_snapshot.*?"
   r"snapshot\.non_valid_serials\.len\(\).*?"
   r"OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1.*?"
   r"serial\.len\(\).*?OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1"),
  "bounded governed Android status snapshot",
 )
 rp(
  policy, CAP, e,
  (r"fn android_attestation_status_snapshot_fresh_until_ms\(.*?"
   r"snapshot\.version.*?OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1.*?"
   r"snapshot\.payload_sha256 == \[0; 32\].*?snapshot\.response_date_ms == 0.*?"
   r"response_date_ms\.is_multiple_of\(1_000\).*?cache_max_age_seconds == 0.*?"
   r"OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1.*?"
   r"last_modified_ms == 0.*?last_modified_ms\.is_multiple_of\(1_000\).*?"
   r"last_modified_ms > snapshot\.response_date_ms.*?"
   r"previous_serial: Option<&str>.*?serial\.is_empty\(\).*?"
   r"byte\.is_ascii_digit\(\).*?matches!\(byte, b'a'\.\.=b'f'\).*?"
   r"serial\.starts_with\('0'\).*?previous >= serial\.as_str\(\).*?"
   r"response_date_ms\s*\.checked_add\(u64::from\(snapshot\.cache_max_age_seconds\) \* 1_000\)"),
  "canonical governed Android status metadata and serial set",
 )
 rp(
  policy, CAP, e,
  (r"fn validate_android_attestation_status_snapshot_at\(.*?"
   r"evaluation_time_ms < snapshot\.response_date_ms \|\| evaluation_time_ms >= fresh_until_ms.*?"
   r"fn validate_android_attestation_status_transition\(.*?"
   r"\(Some\(_\), None\) => \{\s*return Err\(.*?anti-rollback state cannot be removed.*?"
   r"previous == candidate.*?candidate\.response_date_ms <= previous\.response_date_ms.*?"
   r"candidate < previous.*?previous\.last_modified_ms\.is_some\(\)"
   r" && candidate\.last_modified_ms\.is_none\(\).*?"
   r"candidate\.last_modified_ms == previous\.last_modified_ms.*?"
   r"candidate\.payload_sha256 != previous\.payload_sha256.*?"
   r"candidate\.non_valid_serials != previous\.non_valid_serials"),
  D_ANDROID_STATUS[0],
 )
 rp(
  policy, CAP, e,
  (r"fn validate_offline_attestation_policy_transition_from_state\(.*?"
   r"OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY.*?"
   r"decode_canonical::<OfflineDeviceAttestationPolicy>.*?"
   r"validate_offline_attestation_policy_transition\(&previous, candidate\).*?"
   r"fn validate_offline_attestation_policy_status_coverage\(.*?"
   r"policy\.require_android_app_policy.*?android_status_snapshot\.as_ref\(\).*?"
   r"exclusive_end_ms > fresh_until_ms"),
  "state-authenticated Android status transition and validity coverage",
 )
 rp(
  policy, CAP, e,
  (r"let ios_roots = policy.*?APPLE_APP_ATTESTATION_ROOT_CA_DER_B64.*?"
   r"if ios_roots != expected_ios_roots.*?let mut android_roots = policy.*?"
   r"OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT.*?root\.der\.clone\(\).*?"
   r"android_roots\.sort_unstable\(\).*?"
   r"ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64.*?"
   r"ANDROID_KEY_ATTESTATION_CA_DER_B64.*?"
   r"expected_android_roots\.sort_unstable\(\).*?"
   r"if android_roots != expected_android_roots.*?"
   r"any\(\|root\| !trusted_root_is_active\(root, block_unix_timestamp_ms\)\)"),
  D_ANDROID_STATUS[0],
 )
 rp(
  core, CORE, e,
  (r"fn validate_android_keymint_report\(.*?android_status_snapshot\.as_ref\(\).*?"
   r"non_valid_serials\s*\.iter\(\)\s*\.cloned\(\).*?"
   r"x509_evaluation_time\(\s*registration.*?expires_at_ms.*?checked_sub\(1\).*?"
   r"validate_android_key_attestation_certificate_chain\(.*?"
   r"&non_valid_certificate_serials.*?registration_last_valid_time"),
  "state-governed Android status consumption",
 )
 rp(
  core, CORE, e,
  (r"impl Execute for SetOfflineDeviceAttestationPolicy.*?"
   r"validate_offline_attestation_policy_transition_from_state\(&policy, state_transaction\)"),
  "state-governed Android status transition",
 )
 rq(
  activation, CORE_KAGEMUSHA_ACTIVATION_COMPONENT, e,
  "validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?;",
 )
 rp(
  registration, CDR, e,
  (r"validate_offline_attestation_policy_status_coverage\(\s*"
   r"&lifetime_policy,\s*registration\.expires_at_ms,\s*\)\?;.*?"
   r"let last_valid_ms = registration\.expires_at_ms\.saturating_sub\(1\)"),
  "Android status coverage through registration expiry",
 )
 rp(
  qualification, CVQ, e,
  (r"let status_coverage_exclusive_end_ms = validator_qualification_expires_at_unix_ms\s*"
   r"\.checked_add\(1\).*?validate_offline_attestation_policy_status_coverage\(\s*"
   r"&device_attestation_policy,\s*status_coverage_exclusive_end_ms,\s*\)"),
  "Android status coverage through validator qualification expiry",
 )

 rp(
  android_auth, ANDROID_AUTH, e,
  (r"packages\.len\(\) != 1.*?signature_digests\.len\(\) != 1.*?"
   r"verified_boot_key\.is_empty\(\).*?!device_locked.*?VERIFIED.*?"
   r"verified_boot_hash\.len\(\) != 32.*?seen_tags.*?"
   r"!seen_tags\.insert\(tag\.number\).*?!hardware_enforced.*?"
   r"validate_android_root_of_trust\(value\)"),
  "strict Android application, boot, and authorization identity",
 )
 rp(
  der, CDP, e,
  (r"fn parse_strict_x509_algorithm_identifier\(.*?"
   r"DerReader::sequence\(encoded\).*?read_expected\(0x06\).*?"
   r"read_tlv_full_with_raw\(\).*?sequence\.has_remaining\(\).*?"
   r"fn strict_x509_algorithm_parameters_are_absent_or_null\(.*?"
   r"tag\.first_byte == 0x05.*?value\.is_empty\(\).*?raw == \[0x05, 0x00\]"),
  "strict raw AlgorithmIdentifier parsing",
 )
 rp(
  der, CDP, e,
  (r"fn validate_strict_x509_rsa_pss_parameters\(.*?"
   r"read_expected\(0xa0\).*?read_expected\(0xa1\).*?read_expected\(0xa2\).*?"
   r"parameters\.has_remaining\(\).*?strict_single_der_tlv\(hash_field, 0x30\).*?"
   r"MGF1_OID.*?strict_single_der_tlv\(mask_parameters_raw, 0x30\).*?"
   r"mask_hash_oid != hash_oid \|\| mask_digest_bytes != digest_bytes.*?"
   r"salt_length != digest_bytes"),
  "strict explicit SHA-2 RSA-PSS byte profile",
 )
 rq(
  core, CORE, e, ".checked_mul(128)",
  ".and_then(|number| number.checked_add(u32::from(byte & 0x7F)))",
 )
 rq(
  der, CDP, e,
  "fn der_high_tag_number_overflow_cannot_alias_a_known_tag()",
 )
 rp(
  der, CDP, e,
  (r"fn strict_x509_tbs_certificate_der\(.*?DerReader::sequence\(certificate_der\).*?"
   r"read_tlv_full_with_raw\(\).*?signature_value\[0\] != 0.*?"
   r"certificate\.has_remaining\(\).*?first_tag\.first_byte != 0xa0.*?"
   r"version != 2.*?validate_strict_x509_positive_serial.*?"
   r"inner_algorithm_raw != outer_algorithm_raw.*?"
   r"validate_strict_x509_signature_algorithm\(inner_algorithm_raw\).*?Ok\(tbs_raw\)"),
  D_STRICT_X509[0],
 )
 rp(
  attestation, CAC, e,
  (r"fn parse_x509_certificate_der\(.*?"
   r"let strict_tbs_der = strict_x509_tbs_certificate_der\(certificate_der\)\?;.*?"
   r"X509Certificate::from_der\(certificate_der\).*?"
   r"strict_tbs_der != certificate\.tbs_certificate\.as_ref\(\)"),
  D_STRICT_X509[0],
 )
 rp(
  attestation, CAC, e,
  (r"fn policy_revoked_certificate_tbs_hashes\(.*?"
   r"policy\.revoked_certificate_tbs_sha256.*?"
   r"sha256_bytes\(certificate\.tbs_certificate\.as_ref\(\)\)"),
  D_STRICT_X509[0],
 )
 if attestation.count(
  "sha256_bytes(certificate.tbs_certificate.as_ref())"
 ) < 2 or "sha256_bytes(certificate_der)" in attestation:
  e.append(
   f"{CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT}: {D_STRICT_X509[0]}"
  )

 rp(
  attestation, CAC, e,
  (r"enum AndroidKeyAttestationCertificateChainKind\s*\{\s*Factory,\s*RemoteKeyProvisioning.*?"
   r"fn classify_android_key_attestation_certificate_chain\(.*?2\.5\.4\.5.*?"
   r'"Droid CA2".*?"Google LLC".*?subject\.iter_attributes\(\)\.count\(\) == 2.*?'
   r"classification is ambiguous.*?classification is unknown"),
  "exact fail-closed Android Factory/RKP classifier",
 )
 rp(
  attestation, CAC, e,
  (r"struct X509EvaluationTime.*?subsecond_millis.*?"
   r"unix_timestamp_seconds == boundary\.timestamp\(\) && self\.subsecond_millis != 0.*?"
   r"subsecond_millis: block_unix_timestamp_ms % 1_000.*?"
   r"fn validate_android_key_attestation_certificate_chain_time_profile\(.*?"
   r"registration_expiry_time < evaluation_time.*?"
   r"ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64.*?anchor_der == legacy_google_root\.as_slice\(\).*?"
   r"evaluation_time\.is_before\(certificate\.validity\(\)\.not_before\).*?"
   r"Factory.*?!factory_may_ignore_expiration.*?evaluation_time\.is_after\(certificate\.validity\(\)\.not_after\).*?"
   r"RemoteKeyProvisioning.*?validate_x509_certificate_time\(certificate, evaluation_time\)\.is_err\(\).*?"
   r"validate_x509_certificate_time\(certificate, registration_expiry_time\)\.is_err\(\).*?"
   r"parsed_chain\.iter\(\)\.skip\(1\)"),
  D_ANDROID_CHAIN[0],
 )
 rp(
  attestation, CAC, e,
  (r"fn validate_android_key_attestation_certificate_chain\(.*?"
   r"for certificate_der in certificate_chain \{.*?"
   r"x509_certificate_canonical_serial_hex\(&certificate\).*?"
   r"non_valid_certificate_serials.*?"
   r"for root_der in trusted_roots_der.*?"
   r"non_valid_certificate_serials\s*\.contains\(&x509_certificate_canonical_serial_hex\(&root\)\)"),
  D_ANDROID_CHAIN[0],
 )
 fb(
  der + attestation + policy,
  "device attestation admission regressions", e, "#[ignore]",
 )

 rq(
  capture, STATUS_CAPTURE, e,
  'STATUS_HOST = "android.googleapis.com"',
  'STATUS_PATH = "/attestation/status"',
  'NON_VALID_STATUSES = frozenset(("REVOKED", "SUSPENDED"))',
  "MAX_PAYLOAD_BYTES = 256 * 1024",
  "MAX_NON_VALID_SERIALS = 4_096",
  "MAX_SERIAL_HEX_BYTES = 40",
  "MAX_CACHE_AGE_SECONDS = 86_400",
 )
 rp(
  capture, STATUS_CAPTURE, e,
  (r"def _strict_json_object\(.*?object_pairs_hook=reject_duplicates.*?"
   r"parse_constant=.*?CaptureError.*?def _canonical_non_valid_serials\(.*?"
   r"set\(status\) != \{\"entries\"\}.*?len\(entries\) > MAX_NON_VALID_SERIALS.*?"
   r"SERIAL_RE\.fullmatch\(serial\).*?record\.get\(\"status\"\) not in NON_VALID_STATUSES.*?"
   r"return sorted\(serials\)"),
  "strict canonical Android non-valid status payload",
 )
 rp(
  capture, STATUS_CAPTURE, e,
  (r"def build_capture\(.*?1 <= len\(payload\) <= MAX_PAYLOAD_BYTES.*?"
   r'_one_header\(header_list, "Date"\).*?_one_header\(header_list, "Age"\).*?'
   r'_one_header\(header_list, "Cache-Control"\).*?_one_header\(header_list, "Expires"\).*?'
   r'Content-Encoding.*?casefold\(\) != "identity".*?age_seconds >= cache_max_age_seconds.*?'
   r"abs\(captured_at_ms - expected_capture_ms\) > HTTP_CLOCK_TOLERANCE_MS.*?"
   r"captured_at_ms < response_date_ms or captured_at_ms >= fresh_until_ms.*?"
   r"payload_sha256 = hashlib\.sha256\(payload\)\.digest\(\).*?"
   r'"source_url": STATUS_URL.*?"response_headers".*?"snapshot": snapshot'),
  D_ANDROID_CAPTURE[0],
 )
 rp(
  capture, STATUS_CAPTURE, e,
  (r"def fetch_status\(.*?http\.client\.HTTPSConnection\(\s*STATUS_HOST,\s*port=443.*?"
   r"ssl\.create_default_context\(\).*?connection\.request\(\s*\"GET\",\s*STATUS_PATH.*?"
   r'"Accept-Encoding": "identity".*?response\.status != 200.*?'
   r"response\.read\(MAX_PAYLOAD_BYTES \+ 1\).*?"
   r"def _write_new_private\(.*?os\.O_EXCL.*?0o600.*?os\.fsync.*?"
   r"def _fsync_directory\(.*?O_DIRECTORY.*?os\.fsync.*?"
   r"def publish_capture\(.*?target\.mkdir\(mode=0o700\).*?"
   r"_fsync_directory\(target\).*?_fsync_directory\(parent\)"),
  "fixed HTTPS and owner-only no-replace Android status capture",
 )
 rp(
  al, DLS, e,
  (r"from scripts import android_attestation_certificate_profile as _android_x509.*?"
   r"import android_attestation_certificate_profile as _android_x509.*?"
   r"_decode_attestation_certificate_chain = _android_x509\._decode_attestation_certificate_chain.*?"
   r"_classify_android_attestation_certificate_chain = \(\s*"
   r"_android_x509\._classify_android_attestation_certificate_chain\s*\).*?"
   r"_validate_android_attestation_certificate_time_profile = \(\s*"
   r"_android_x509\._validate_android_attestation_certificate_time_profile\s*\)"),
  "exact Android certificate-profile module delegation",
 )
 rp(
  cp, ANDROID_CERT, e,
  (r"def _x509_certificate_validity_and_subject\(.*?"
   r"return \(not_before \* 1_000, not_after \* 1_000\), subject.*?"
   r"def _classify_android_attestation_certificate_chain\(.*?"
   r"X509_SERIAL_NUMBER_OID_DER_VALUE.*?len\(attributes\) == 2.*?"
   r'"Droid CA2".*?"Google LLC".*?classification is ambiguous.*?classification is unknown.*?'
   r"def _validate_android_attestation_certificate_time_profile\(.*?"
   r"evaluation_time_ms: int.*?"
   r"certificates\[-2\].*?ANDROID_LEGACY_GOOGLE_ATTESTATION_ROOT_SHA256.*?"
   r"for certificate in certificates\[1:\].*?evaluation_time_ms < not_before.*?"
   r'chain_kind == "factory".*?not legacy_factory_root.*?evaluation_time_ms > not_after.*?'
   r"elif evaluation_time_ms > not_after"),
  D_ANDROID_CHAIN[0],
 )
 rp(
  cp, ANDROID_CERT, e,
  (r"class _StrictDerReader:.*?DER high tag number is non-minimal.*?"
   r"DER length is non-minimal.*?explicitly use X\.509 version 3.*?"
   r"len\(serial_value\) > 20.*?inner and outer signature algorithms must match exactly.*?"
   r"duplicate or empty extension OIDs.*?def _decode_attestation_certificate_chain\(.*?"
   r"len\(certificates\) > 8.*?repeats a certificate.*?"
   r"def _x509_certificate_serial_and_attestation_extension\(.*?"
   r"_x509_extensions\(extension_payload\)\.get\(oid_value\)"),
  "strict bounded Android certificate DER and serial parsing",
 )
 rq(
  al, DLS, e,
  "evaluation_time_ms=time.time_ns() // 1_000_000,",
 )
 rp(
  cert_fixtures,
  ANDROID_CERT_FIX,
  e,
  (r"def bind_device_lab\(module: Any\).*?def test_android_attestation_chain\(.*?"
   r'chain_kind: str = "factory".*?leaf_days: int = 3650.*?'
   r'chain_kind == "rkp".*?chain_kind == "unknown".*?'
   r"test_android_attestation_chain\.__test__ = False"),
  "bound Factory/RKP Android certificate fixtures",
 )
 rq(
  cts, ANDROID_CERT_TEST, e,
  *(f"def {name}(self)" for name in (
   "test_android_factory_profile_ignores_only_target_leaf_expiration",
   "test_android_factory_profile_rejects_not_yet_valid_non_target",
   "test_android_factory_expiration_exception_is_exactly_legacy_root",
   "test_android_rkp_profile_is_valid_at_the_evidence_validation_horizon",
   "test_android_rkp_profile_rejects_expired_non_target",
   "test_android_unknown_chain_profile_is_rejected",
   "test_android_path_verification_uses_manual_time_profile",
  )),
  'self.assertIn("-no_check_time", call.args[0])',
 )
 rp(
  al, DLS, e,
  (r"capture_android_attestation_status as android_status_capture.*?"
   r"MAX_ANDROID_ATTESTATION_STATUS_CAPTURE_RECEIPT_BYTES = 256 \* 1024.*?"
   r"def configure_android_evidence_authority\(.*?attestation_status_capture_receipt.*?"
   r"_read_pinned_authority_file\(\s*attestation_status_capture_receipt.*?"
   r"android_status_capture\.build_capture\(\s*status_bytes,\s*headers,.*?"
   r"if capture_receipt != rebuilt_receipt.*?evaluated_at_ms >= fresh_until_ms.*?"
   r'"attestation_status_capture_receipt_sha256".*?"android_status_snapshot".*?'
   r'"payload_sha256".*?"non_valid_serial_count"'),
  D_ANDROID_CAPTURE[0],
 )
 rp(
  runner, ANDROID_DEVICE_LAB_RUNNER, e,
  (r"the full run requires a pinned Android attestation status capture receipt.*?"
   r"verify_pinned_file\s*\\?\s*\"\$AUTHORITY_STATUS_CAPTURE_RECEIPT\".*?"
   r"\"\$AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256\".*?"
   r"AUTHORITY_VALIDATOR_ARGS=\(.*?"
   r"--android-attestation-status-capture-receipt \"\$AUTHORITY_STATUS_CAPTURE_RECEIPT\".*?"
   r"--android-attestation-status-capture-receipt-sha256 \"\$AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256\".*?"
   r'"attestation_status_capture_receipt_sha256".*?"android_status_snapshot".*?'
   r"write_run_receipt.*?\"\$AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256\".*?"
   r"\"\$ANDROID_STATUS_NON_VALID_SERIAL_COUNT\""),
  D_ANDROID_CAPTURE[0],
 )
 return e


def canary_source_errors(
 canary: str,
 liveness: str,
 rollout: str,
 rl: str,
 promotion_receipt: str,
 model_isi_offline: str,
 model_isi_mod: str,
 core: str,
 cc: str,
 core_attestation_certificate_validation: str,
 core_isi_mod: str,
 core_tx: str,
 cs: str,
 core_committed_transaction_context: str,
 core_block: str,
 core_executor: str,
) -> list[str]:
 e = attestation_certificate_source_errors(
  core_attestation_certificate_validation, core
 )
 corridor = promotion_receipt.split(
  "pub(super) fn validate_finality_corridor_context(", 1
 )[-1].split("fn enforce_activation_receipt_frame_size", 1)[0]
 rp(
  corridor, MODEL_PROMOTION_RECEIPT_COMPONENT, e,
  (r"context\.network_id != binding\.network_id.*?"
   r"context\.mode != crate::block::consensus_v2::ConsensusMode::Permissioned.*?"
   r"context\.nexus_amx_context_hash.*?runtime\.genesis_context\.nexus_amx_context_hash.*?"
   r"context\.execution_policy_hash != binding\.execution_policy_hash.*?"
   r"context\.da_layout != runtime\.genesis_context\.da_layout.*?"
   r"context\.snapshot_bootstrap\.is_some\(\).*?"
   r"context\.roster\.len\(\) != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT.*?"
   r"validator_set_pops\.len\(\) != runtime\.validators\.len\(\).*?"
   r"member\.power != 1.*?member\.validator != body\.validator_id.*?"
   r"actual != &expected\.bls_pop"),
  "exact four-validator DA Nexus and PoP finality corridor",
 )
 origin = canary.split(
  "pub fn validate_kagemusha_v4_taira_canary_torii_origin(", 1
 )[-1].split("pub struct KagemushaV4TairaCanaryAuthorizationBodyV1", 1)[0]
 rp(
  origin, MCE, e,
  (r"origin != origin\.to_ascii_lowercase\(\).*?strip_prefix\(\"https://\"\).*?"
   r"matches!\(character, '/' \| '\?' \| '#' \| '@' \| '\[' \| '\]'\).*?"
   r"host\.parse::<std::net::IpAddr>\(\)\.is_ok\(\).*?"
   r"byte\.is_ascii_lowercase\(\).*?port == 0 \|\| port == 443"),
  "canonical lower-case HTTPS DNS canary origin",
 )
 permit = canary.split("impl KagemushaV4TairaCanaryPermitV1", 1)[-1].split(
  "pub struct KagemushaV4TairaCanaryReservationBodyV1", 1
 )[0]
 rp(
  permit, MCE, e,
  (r"validate_permit_binding\(&body, expectations, receipt, exact_receipt_bytes\).*?"
   r"controller\.public_key\(\) != &body\.binding\.promotion_controller.*?"
   r"SignatureOf::try_from_hash\(controller\.private_key\(\), body\.signing_hash\(\)\).*?"
   r"verify_authorization_signature\(.*?self\.body\.signing_hash\(\).*?"
   r"pub fn verify_for_execution\(.*?network_id: &NetworkId,.*?"
   r"canary_authority: &AccountId,.*?block_time_unix_ms: u64,.*?block_height: u64,.*?"
   r"binding\.network_id != network_id.*?canary_authority != canary_authority.*?"
   r"block_time_unix_ms < self\.body\.authorized_at_unix_ms.*?"
   r"block_time_unix_ms >= self\.body\.expires_at_unix_ms.*?"
   r"block_height >= self\.body\.expires_at_height\.get\(\)"),
  "pre-commit controller-signed permit with consensus time authority and height bounds",
 )
 reservation = canary.split(
  "pub struct KagemushaV4TairaCanaryReservationBodyV1", 1
 )[-1].split("pub struct KagemushaV4TairaCanaryAuthorizationPackageV1", 1)[0]
 rp(
  reservation, MCE, e,
  (r"pub permit: KagemushaV4TairaCanaryPermitV1,.*?"
   r"pub canary_transaction_intent: HashOf<SignedTransaction>,.*?"
   r"pub canary_transaction_wire: KagemushaExactBytesDigestV1,.*?"
   r"pub canary_entrypoint_hash: Hash,.*?"
   r"KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SIGNATURE_DOMAIN.*?"
   r"canary_transaction_intent: canary_transaction\.hash\(\).*?"
   r"canary_transaction_wire: KagemushaExactBytesDigestV1::from_bytes\(&transaction_wire\).*?"
   r"canary_entrypoint_hash: Hash::from\(canary_transaction\.hash_as_entrypoint\(\)\).*?"
   r"SignatureOf::try_from_hash\(controller\.private_key\(\), body\.signing_hash\(\)\).*?"
   r"verify_for_execution\(.*?authorizer: &AccountId,.*?"
   r"permit\.verify_for_execution\(\s*network_id,\s*authorizer,.*?"
   r"verify_reservation_signature\(.*?self\.body\.signing_hash\(\)"),
  "minimal controller-signed non-disclosing exact-call reservation",
 )
 fb(
  reservation, "on-chain canary reservation", e,
  "pub canary_transaction: SignedTransaction",
  "pub exact_transaction_wire: Vec<u8>",
  "KagemushaV4TairaCanaryAuthorizationV1",
 )
 authorization = canary.split(
  "pub struct KagemushaV4TairaCanaryAuthorizationPackageV1", 1
 )[-1].split("pub struct KagemushaV4VerifiedTairaCanaryAuthorizationV1", 1)[0]
 rp(
  authorization, MCE, e,
  (r"pub reservation: KagemushaV4TairaCanaryReservationV1,.*?"
   r"pub canary_transaction: SignedTransaction,.*?"
   r"SignatureOf<KagemushaV4TairaCanaryAuthorizationPackageV1>.*?"
   r"KagemushaV4TairaCanaryReservationV1::try_sign\(.*?"
   r"validate_canary_transaction\(&package\).*?"
   r"SignatureOf::try_from_hash\(controller\.private_key\(\), package\.signing_hash\(\)\).*?"
   r"norito::encode_canonical\(self\).*?!= exact_authorization_bytes.*?"
   r"self\.reservation\.verify_structure_and_signature\(\).*?"
   r"verify_authorization_package_signature\(.*?package\.signing_hash\(\)"),
  "full private authorization with exact reservation transaction and outer signature",
 )
 transaction = canary.split("fn validate_canary_transaction(", 1)[-1].split(
  "fn validate_permit_binding(", 1
 )[0]
 rp(
  transaction, MCE, e,
  (r"reservation\s*\.verify_structure_and_signature\(\).*?"
   r"transaction\.network_id\(\) != Some\(&body\.binding\.network_id\).*?"
   r"transaction\.authority\(\) != &body\.canary_authority.*?"
   r"transaction\.nonce\(\)\.is_none\(\).*?transaction\.attachments\(\)\.is_some\(\).*?"
   r"TransactionAdmissionIntent::Ordinary.*?transaction\s*\.verify_signature\(\).*?"
   r"transaction\.hash\(\) != reservation\.canary_transaction_intent.*?"
   r"hash_as_entrypoint\(\).*?reservation\.canary_entrypoint_hash.*?"
   r"transaction\s*\.time_to_live\(\).*?time_to_live_ms == 0.*?"
   r"wall_expiry > body\.expires_at_unix_ms.*?"
   r"transaction\s*\.expires_at_height\(\).*?expires_at_height != body\.expires_at_height.*?"
   r"transaction\.metadata\(\) != &expected_metadata.*?"
   r"Executable::Instructions\(instructions\).*?let \[instruction\].*?"
   r"downcast_ref::<RecordKagemushaTairaCanaryV4>\(\).*?"
   r"record\.permit\(\) != permit.*?embedded_permit != packaged_permit.*?"
   r"canary_transaction_wire\s*\.matches_bytes\(&transaction_wire\)"),
  "one exact signed ordinary nonce TTL height-expiry Record canary",
 )
 binding = canary.split("fn validate_permit_binding(", 1)[-1].split(
  "fn verify_evidence_body(", 1
 )[0]
 rp(
  binding, MCE, e,
  (r"norito::encode_canonical\(receipt\).*?!= exact_receipt_bytes.*?"
   r"activation_finality_receipt\s*\.matches_bytes\(exact_receipt_bytes\).*?"
   r"receipt\s*\.verify\(expectations\).*?"
   r"body\.binding != \*expectations\.binding\(\).*?"
   r"body\.activation_expectations_artifact != expectations\.activation_expectations_artifact\(\).*?"
   r"body\.authorized_at_unix_ms <= activation_block_time.*?"
   r"KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1.*?height\.checked_add\(1\).*?"
   r"expires_at_height <= verified_receipt\.finalized_height\(\)\.saturating_add\(1\).*?"
   r"expires_at_height > maximum_expiry"),
  "exact post-receipt promotion network and exclusive proof-corridor binding",
 )
 query = canary.split("impl KagemushaV4TairaCanaryQueryObservationV1", 1)[-1].split(
  "pub struct KagemushaV4TairaCanaryEvidenceBodyV1", 1
 )[0]
 rp(
  query, MCE, e,
  (r"pipeline_status_scope != \"global\".*?pipeline_status_resolved_from != \"state\".*?"
   r"pipeline_transaction_intent != canary_transaction_intent.*?"
   r"pipeline_status_kind != \"Applied\".*?pipeline_status_block_height != finalized_height.*?"
   r"transaction_details_trigger_completion_count != 0.*?"
   r"node_status_before_height < finalized_height.*?finality_proof_count == 0"),
  "canary-specific global Applied query observation",
 )
 evidence = canary.split("impl KagemushaV4TairaCanaryEvidenceV1", 1)[-1].split(
  "pub struct KagemushaV4VerifiedTairaCanaryEvidenceV1", 1
 )[0]
 rp(
  evidence, MCE, e,
  (r"norito::encode_canonical\(self\).*?!= exact_evidence_bytes.*?"
   r"verify_evidence_signature\(&self\.signature, &self\.body\.issuer, "
   r"self\.body\.signing_hash\(\)\).*?"
   r"let verified = verify_evidence_body\(\s*&self\.body,\s*"
   r"EvidenceVerificationInputs \{\s*authorization,\s*"
   r"exact_authorization_bytes,\s*expectations,\s*receipt,\s*"
   r"exact_receipt_bytes,\s*\},\s*\)\?;\s*Ok\(verified\)"),
  "exact issuer-signed canary evidence entrypoint",
 )
 rp(
  canary, MCE, e,
  (r"pub struct KagemushaV4VerifiedTairaCanaryEvidenceV1 \{.*?"
   r"activation_expectations_artifact: KagemushaExactBytesDigestV1,.*?"
   r"pub const fn activation_expectations_artifact\(&self\).*?"
   r"self\.activation_expectations_artifact.*?"
   r"Ok\(KagemushaV4VerifiedTairaCanaryEvidenceV1 \{.*?"
   r"activation_expectations_artifact: body\.activation_expectations_artifact"),
  "exact expectations provenance and canary anchor binding",
 )
 evidence_body = canary.split("fn verify_evidence_body(", 1)[-1].split(
  "fn canonical_digest", 1
 )[0]
 rp(
  evidence_body, MCE, e,
  (r"let verified = verify_evidence_prerequisites\(body, &inputs\)\?;.*?"
   r"verify_evidence_block_binding\(body, &verified\)\?;.*?"
   r"let authorized_wire = verify_committed_canary\(body, &verified\)\?;.*?"
   r"verify_evidence_finality\(\s*body,\s*inputs\.receipt,\s*"
   r"inputs\.expectations,\s*&verified,\s*&authorized_wire,\s*\)\?;.*?"
   r"fn verify_evidence_prerequisites\(.*?"
   r"decode_exact_finalized_block\(body\.finalized_block_wire\.as_bytes\(\).*?"
   r"authorization\.verify_exact\(.*?block_time_unix_ms.*?"
   r"body\.canary_authorization != authorization_identity.*?"
   r"body\.canary_transaction_intent != authorization\.canary_transaction_intent\(\).*?"
   r"body\.canary_transaction_wire != authorization\.canary_transaction_wire\(\).*?"
   r"fn verify_evidence_block_binding\(.*?"
   r"body\.finalized_height >= verified\.authorization\.expires_at_height\(\)\.get\(\).*?"
   r"fn verify_committed_canary\(.*?"
   r"committed\.verify_inclusion_in_block\(&verified\.block\).*?"
   r"committed_wire != authorized_wire.*?"
   r"body\.canary_transaction_wire\.matches_bytes\(&committed_wire\).*?"
   r"fn verify_evidence_finality\(.*?"
   r"finality_proof_chain\s*\.first\(\).*?checked_add\(1\).*?"
   r"checked_add\(proof_count\).*?Some\(body\.finalized_height\).*?"
   r"BridgeFinalityVerifier::with_context\(.*?"
   r"verify\(expectations\.trusted_finality_anchor\(\)\).*?"
   r"for proof in &receipt\.body\.finality_proof_chain.*?"
   r"for proof in &body\.finality_proof_chain.*?"
   r"TrustedBlockProofAnchor::from_untrusted_finality_artifact\(.*?"
   r"block_transaction\s*\.encode_wire_v1\(\).*?!= authorized_wire"),
  "proof-anchored exact committed canary and contiguous post-receipt finality",
 )

 isi = model_isi_offline.split("pub struct RecordKagemushaTairaCanaryV4", 1)[-1]
 rp(
  isi, MODEL_ISI_OFFLINE, e,
  (r"pub permit: KagemushaV4TairaCanaryPermitV1,.*?"
   r"pub struct AuthorizeKagemushaTairaCanaryV4.*?"
   r"pub reservation: KagemushaV4TairaCanaryReservationV1,.*?"
   r"impl crate::seal::Instruction for RecordKagemushaTairaCanaryV4.*?"
   r"impl crate::seal::Instruction for AuthorizeKagemushaTairaCanaryV4.*?"
   r"taira_canary\.record\.v1.*?pub fn new\(permit: KagemushaV4TairaCanaryPermitV1\).*?"
   r"taira_canary\.authorize\.v1.*?pub fn new\(reservation: KagemushaV4TairaCanaryReservationV1\).*?"
   r"impl_decode_one_canonical_offline_field!\(RecordKagemushaTairaCanaryV4.*?"
   r"impl_decode_one_canonical_offline_field!\(AuthorizeKagemushaTairaCanaryV4"),
  "canonical minimal canary Record and Authorize instruction wires",
 )
 authorize_field = isi.split("pub struct AuthorizeKagemushaTairaCanaryV4", 1)[-1].split(
  "impl PartialOrd", 1
 )[0]
 fb(
  authorize_field, "AuthorizeKagemushaTairaCanaryV4 payload", e,
  "SignedTransaction", "KagemushaV4TairaCanaryAuthorizationV1", "Vec<u8>",
 )
 rq(
  model_isi_mod, MODEL_ISI_MOD, e,
  "impl_direct_instruction_box!(crate::isi::offline::RecordKagemushaTairaCanaryV4);",
  "impl_direct_instruction_box!(crate::isi::offline::AuthorizeKagemushaTairaCanaryV4);",
 )
 rq(
  core_isi_mod, CORE_ISI_MOD, e,
  "dispatch_instruction::<iroha_data_model::isi::offline::RecordKagemushaTairaCanaryV4>",
  "dispatch_instruction::<iroha_data_model::isi::offline::AuthorizeKagemushaTairaCanaryV4>",
 )
 wb = "complete signed canary wire bound attestation all transaction boundaries"
 rp(
  cc, CKC, e,
  (r"pub\(crate\) fn signed_kagemusha_taira_canary_wire_identity_v1\(.*?"
   r"transaction: &SignedTransaction,.*?"
   r"Result<Option<KagemushaExactBytesDigestV1>.*?"
   r"transaction\.admission_intent\(\) != TransactionAdmissionIntent::Ordinary.*?"
   r"return Ok\(None\);.*?"
   r"Executable::Instructions\(instructions\) = transaction\.instructions\(\).*?"
   r"let \[instruction\] = instructions\.as_ref\(\).*?"
   r"downcast_ref::<RecordKagemushaTairaCanaryV4>\(\).*?"
   r"transaction\.encode_wire_v1\(\).*?KagemushaExactBytesDigestV1::from_bytes\(&wire\)"
   r".*?\.map\(Some\)"),
  wb,
 )
 rp(
  core_tx, CORE_TX, e,
  (r"fn validate_transaction\(.*?"
   r"kagemusha_taira_canary_external_entrypoint\s*=\s*"
   r"matches!\(tx\.entrypoint\(\), TransactionEntrypoint::External\(_\)\)"),
  "External-only live transaction canary provenance",
 )
 rp(
  cs, CORE_STATE, e,
  (r"mod committed_transaction_context;.*?"
   r"use committed_transaction_context::seed_committed_transaction_context;.*?"
   r"pub\(crate\) kagemusha_taira_canary_wire_identity:\s*"
   r"Option<KagemushaExactBytesDigestV1>.*?"
   r"pub\(crate\) kagemusha_taira_canary_external_entrypoint: bool.*?"
   r"kagemusha_taira_canary_wire_identity: None.*?"
   r"kagemusha_taira_canary_external_entrypoint: false.*?"
   r"if block\.error\(entrypoint_index\)\.is_none\(\).*?"
   r"let mut transaction = self\.transaction\(\);.*?"
   r"crate::state::seed_committed_transaction_context\(\s*"
   r"&mut transaction,\s*&entrypoint,\s*entrypoint_index,\s*\)"),
  wb,
 )
 rp(
  cs, CORE_STATE, e,
  (r"fn preexecute_merge_execution_sources_into\(.*?"
   r"if source\.input\.entrypoints\.iter\(\)\.any\(\|entrypoint\| \{.*?"
   r"entrypoint\.admission_intent\(\).*?TransactionAdmissionIntent::QueuePlanSynced.*?"
   r"autonomous merge entrypoint does not carry QueuePlanSynced admission intent.*?"
   r"for \(\(\(entrypoint"),
  "QueuePlanSynced-only authenticated autonomous merge producer boundary",
 )
 rp(
  cs, CORE_STATE, e,
  (r"fn validate_merge_execution_batch\(.*?"
   r"merge_execution_batch_commitments_match\(batch\).*?"
   r"if batch\.lanes\.iter\(\)\.any\(\|execution\| \{.*?"
   r"entrypoint\.admission_intent\(\).*?TransactionAdmissionIntent::QueuePlanSynced.*?"
   r"autonomous merge entrypoint does not carry QueuePlanSynced admission intent.*?"
   r"if batch\.application_block_header"),
  "commitment-checked QueuePlanSynced autonomous merge follower boundary",
 )
 rp(
  core_committed_transaction_context,
  CORE_COMMITTED_TX_CONTEXT,
  e,
  (r"pub\(crate\) fn seed_committed_transaction_context\(\s*"
   r"state_transaction: &mut StateTransaction.*?"
   r"entrypoint: &TransactionEntrypoint,.*?entrypoint_index: usize,.*?"
   r"kagemusha_taira_canary_external_entrypoint = false;.*?"
   r"TransactionEntrypoint::External\(transaction\).*?"
   r"kagemusha_taira_canary_external_entrypoint = true;.*?"
   r"TransactionEntrypoint::SealedReveal\(reveal\).*?reveal\.signed_transaction\(\).*?"
   r"state_transaction\.tx_call_hash\s*=\s*Some\(Hash::from\("
   r"entrypoint\.execution_call_hash\(\)\)\);.*?"
   r"state_transaction\.current_tx_hash\s*=.*?"
   r"AcceptedTransaction::prepare_signed_metadata\(transaction\)\.signed_hash.*?"
   r"if state_transaction\.kagemusha_taira_canary_external_entrypoint.*?"
   r"state_transaction\.kagemusha_taira_canary_wire_identity\s*=\s*"
   r"signed_kagemusha_taira_canary_wire_identity_v1\(transaction\).*?"
   r"\.expect\(\"committed external canary wire must encode\"\).*?"
   r"state_transaction\.current_entrypoint_index\s*=\s*"
   r"Some\(u64::try_from\(entrypoint_index\)\.unwrap_or\(u64::MAX\)\)"),
  wb,
 )
 rp(
  core_block, CORE_BLOCK, e,
  (r"fn validate_block_transaction_admission\(.*?"
   r"canary_wire_identity\s*=\s*crate::smartcontracts::isi::offline::"
   r"signed_kagemusha_taira_canary_wire_identity_v1\(tx\).*?"
   r"map_err\(TransactionRejectionReason::Validation\).*?"
   r"state_tx\.kagemusha_taira_canary_external_entrypoint\s*=\s*true;.*?"
   r"state_tx\.kagemusha_taira_canary_wire_identity\s*=\s*canary_wire_identity;.*?"
   r"StateBlock::validate_stateful_admission\(tx, state_tx, Some\(routing\)\)"),
  wb,
 )
 rp(
  core_block, CORE_BLOCK, e,
  (r"fn sequential_entrypoints_for_live_execution\(.*?"
   r"entrypoints\s*\.iter\(\)\s*\.any\(\|entrypoint\|\s*"
   r"!matches!\(entrypoint, TransactionEntrypoint::External\(_\)\)\).*?"
   r"needs_sequential\.then\(\|\| entrypoints\.to_vec\(\)\)"),
  "non-External live block sequential-execution selector",
 )
 rp(
  core_executor, CORE_EXECUTOR, e,
  (r"pub fn execute_transaction\(.*?"
   r"state_transaction\.kagemusha_taira_canary_wire_identity\s*=\s*None;.*?"
   r"transaction\.authority\(\) != authority.*?"
   r"if state_transaction\.kagemusha_taira_canary_external_entrypoint.*?"
   r"state_transaction\.kagemusha_taira_canary_wire_identity\s*=\s*"
   r"signed_kagemusha_taira_canary_wire_identity_v1\(&transaction\)\?;.*?"
   r"state_transaction\.tx_call_hash = Some"),
  wb,
 )
 rp(
  cc, CKC, e,
  (r"fn plan_v4_promotion_binding\(.*?"
   r"plan_v4_promotion_id\(binding\.promotion_id, state_transaction\).*?"
   r"fn require_v4_promotion_binding\(.*?promotion_marker.*?binding_marker.*?"
   r"fn plan_v4_taira_canary\(.*?canary_replay.*?fn commit_v4_taira_canary\(.*?"
   r"fn v4_taira_canary_authorization_markers\(.*?wire_identity:.*?"
   r"wire_identity\.validate\(\).*?exact_wire = kagemusha_v2_marker\(.*?"
   r"KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZED_WIRE_DOMAIN.*?&wire_identity\.sha256.*?"
   r"fn plan_v4_taira_canary_authorization\(.*?"
   r"reservation: &iroha_data_model::offline::KagemushaV4TairaCanaryReservationV1.*?"
   r"norito::encode_canonical\(reservation\).*?exact_reservation = kagemusha_v2_marker\(.*?"
   r"KAGEMUSHA_V4_TAIRA_CANARY_EXACT_RESERVATION_DOMAIN.*?"
   r"promotion_id\.as_slice\(\).*?exact_call_hash\.as_ref\(\).*?&reservation_bytes.*?"
   r"\(false, false, false, false\) => \{.*?"
   r"plan_v4_taira_canary\(promotion_id, state_transaction\).*?"
   r"Ok\(Some\(\(slot, exact_call, exact_wire, exact_reservation\)\)\).*?"
   r"\(true, true, true, true\) => Ok\(None\).*?"
   r"a different exact Taira canary reservation already occupies this promotion slot.*?"
   r"fn commit_v4_taira_canary_authorization\(.*?"
   r"exact_wire: Hash.*?exact_reservation: Hash.*?insert\(exact_wire, \(\)\).*?"
   r"insert\(exact_reservation, \(\)\).*?"
   r"fn require_v4_taira_canary_authorization\(.*?wire_identity:.*?"
   r"get\(&exact_wire\)"),
  "activation-bound exact reservation and signed-wire marker",
 )
 rp(
  cc, CKC, e,
  (r"impl Execute for RecordKagemushaTairaCanaryV4.*?"
   r"verify_for_execution\(\s*state_transaction\.network_id\(\),\s*authority,.*?"
   r"require_v4_promotion_binding\(binding, state_transaction\).*?"
   r"if !state_transaction\.kagemusha_taira_canary_external_entrypoint.*?"
   r"canary_external_entrypoint_required.*?"
   r"state_transaction\.tx_call_hash.*?"
   r"state_transaction\s*\.kagemusha_taira_canary_wire_identity\s*"
   r"\.take\(\)\s*\.ok_or_else\(.*?"
   r"require_v4_taira_canary_authorization\(.*?wire_identity.*?"
   r"plan_v4_taira_canary\(binding\.promotion_id, state_transaction\).*?"
   r"commit_v4_taira_canary\(marker, state_transaction\).*?"
   r"impl Execute for AuthorizeKagemushaTairaCanaryV4.*?"
   r"self\.reservation\s*\.verify_for_execution\(\s*"
   r"state_transaction\.network_id\(\),\s*authority,.*?"
   r"require_v4_promotion_binding\(binding, state_transaction\).*?"
   r"exact_call_hash = self\.reservation\.body\.canary_entrypoint_hash.*?"
   r"plan_v4_taira_canary_authorization\(.*?&self\.reservation.*?"
   r"if let Some\(\(slot, exact_call, exact_wire, exact_reservation\)\).*?"
   r"commit_v4_taira_canary_authorization\(\s*slot,\s*exact_call,\s*"
   r"exact_wire,\s*exact_reservation,\s*state_transaction,"),
  "consensus-authorized exact-wire one-shot canary execution",
 )
 fb(
  cc.split("impl Execute for AuthorizeKagemushaTairaCanaryV4", 1)[-1],
  "consensus canary authorization", e,
  "SignedTransaction", "CanActivateKagemushaRecursiveReleaseV4", "self.authorization",
 )

 phase = rollout.split("/// Phase-separated rollout command.", 1)[-1].split(
  "struct TrustedInputs", 1
 )[0]
 rp(
  phase, KRC, e,
  (r"matches!\(&self\.command, Command::CreateExpectations\(_\)\).*?"
   r"CreateExpectations\(CreateExpectations\).*?Submit\(Submit\).*?"
   r"FinalizeReceipt\(FinalizeReceipt\).*?"
   r"CreateCanaryAuthorization\(CreateCanaryAuthorization\).*?"
   r"SubmitCanaryAuthorization\(SubmitCanaryAuthorization\).*?"
   r"SubmitCanary\(SubmitCanary\).*?FinalizeCanaryEvidence\(FinalizeCanaryEvidence\).*?"
   r"FinalizeValidatorLiveness\(liveness::FinalizeValidatorLiveness\).*?"
   r"Command::CreateExpectations\(args\) => args\.run\(context\).*?"
   r"Command::Submit\(args\) => args\.run\(context\).*?"
   r"Command::FinalizeReceipt\(args\) => args\.run\(context\).*?"
   r"Command::CreateCanaryAuthorization\(args\) => args\.run\(context\).*?"
   r"Command::SubmitCanaryAuthorization\(args\) => args\.run\(context\).*?"
   r"Command::SubmitCanary\(args\) => args\.run\(context\).*?"
   r"Command::FinalizeCanaryEvidence\(args\) => args\.run\(context\).*?"
   r"Command::FinalizeValidatorLiveness\(args\) => args\.run\(context\)"),
  "eight phase rollout with isolated create-only fallback credentials",
 )
 rq(
  rollout, KRC, e,
  'const CANARY_AUTHORIZATION_FILE_NAME: &str = "canary-authorization-v1.norito";',
  '"canary-authorization-submission-journal-v1.norito";',
  'const CANARY_SUBMISSION_JOURNAL_FILE_NAME: &str = "canary-submission-journal-v1.norito";',
  'const CANARY_EVIDENCE_FILE_NAME: &str = "canary-evidence-v1.norito";',
  '"post-canary-validator-liveness-challenge-v1.norito";',
  '"post-canary-validator-liveness-evidence-v1.norito";',
  KAGEMUSHA_ROLLOUT_LIVENESS_MODULE,
 )
 create = rollout.split("impl CreateCanaryAuthorization", 1)[-1].split(
  "struct SubmitCanaryAuthorization", 1
 )[0]
 rp(
  create, KRC, e,
  (r"CANARY_AUTHORIZATION_FILE_NAME.*?preflight_root_owned_output\(&self\.output\).*?"
   r"canonical_torii_origin\(&client\.torii_url\).*?"
   r"AuthenticatedCanaryHead::new.*?refresh\(&client.*?require_canary_expiry_margin.*?"
   r"authorized_at_unix_ms = current_unix_ms\(\).*?load_root_custodied_key\(.*?"
   r"KagemushaV4TairaCanaryAuthorizationBodyV1 \{.*?"
   r"binding: loaded\.verified\.binding\(\)\.clone\(\).*?"
   r"activation_expectations_artifact:.*?activation_finality_receipt: receipt_digest.*?"
   r"canary_authority: client\.account\.clone\(\).*?"
   r"canonical_torii_origin: canonical_torii_origin\.clone\(\).*?"
   r"KagemushaV4TairaCanaryPermitV1::try_sign\(.*?"
   r"client\.add_transaction_nonce = true.*?"
   r"transaction_ttl = Some\(Duration::from_millis\(transaction_ttl_ms\)\).*?"
   r"RecordKagemushaTairaCanaryV4::new\(\s*permit\.clone\(\).*?"
   r"TransactionAdmissionIntent::Ordinary.*?"
   r"KagemushaV4TairaCanaryAuthorizationV1::try_sign\(.*?"
   r"authenticated_head\.refresh\(&client.*?require_canary_expiry_margin.*?"
   r"verification_time_unix_ms = current_unix_ms\(\).*?"
   r"authorization\s*\.verify_exact\(.*?verification_time_unix_ms.*?"
   r"publish_root_owned\(&self\.output, &bytes"),
  "fresh exact permit Record authorization creation and private no-replace publication",
 )
 rp(
  create.split("publish_root_owned(&self.output", 1)[-1],
  KRC, e,
  (r"fresh_head\s*\.refresh\(&client.*?require_canary_expiry_margin\(head.*?"
   r"fresh_time = current_unix_ms\(\).*?decode_canonical\(published\).*?"
   r"verify_exact\(.*?fresh_time.*?"
   r"require_canary_authorization_wall_margin\(&verified, fresh_time\)"),
  "fresh post-head authorization publication verification",
 )
 rp(
  create, KRC, e,
  (r"publish_root_owned\(&self\.output, &bytes.*?context\.print_data\(&report\).*?"
   r"PublicationError::CommitUncertain.*?published canary-authorization report failed"),
  "canary authorization no-replace commit-uncertain reporting",
 )
 rp(
  rollout, KRC, e,
  (r"struct SubmitCanaryAuthorization.*?"
   r"#\[arg\(long, required = true, action = clap::ArgAction::SetTrue\)\].*?"
   r"impl SubmitCanaryAuthorization.*?if !self\.write_authorized.*?require_root\(\).*?"
   r"struct SubmitCanary \{.*?"
   r"#\[arg\(long, required = true, action = clap::ArgAction::SetTrue\)\].*?"
   r"impl SubmitCanary \{.*?if !self\.write_authorized.*?require_root\(\)"),
  "explicit write authorization before both canary network phases",
 )
 sa = rollout.split("impl SubmitCanaryAuthorization", 1)[-1].split(
  "struct SubmitCanary", 1
 )[0]
 rp(
  sa, KRC, e,
  (r"CANARY_AUTHORIZATION_SUBMISSION_JOURNAL_FILE_NAME.*?"
   r"AuthorizeKagemushaTairaCanaryV4::new\(\s*"
   r"authorization\.artifact\.reservation\(\)\.clone\(\).*?"
   r"publish_root_owned\(&journal_path, &exact.*?"
   r"refresh\(&client.*?require_canary_expiry_margin.*?"
   r"now = current_unix_ms\(\).*?"
   r"verify_for_authorization_execution\(\s*&client\.network_id,\s*&client\.account,.*?"
   r"get_transaction_status_response_auto\(transaction\.hash\(\).*?is_some\(\).*?"
   r"CANARY_SUBMISSION_JOURNAL_FILE_NAME.*?"
   r"SubmissionJournalObservation::Absent.*?"
   r"get_transaction_status_response_auto\(transaction\.hash\(\).*?is_some\(\).*?"
   r"authorization\.verified\.canary_transaction\(\)\.hash\(\).*?is_some\(\).*?"
   r"publish_canary_submission_journal\(.*?"
   r"!= SubmissionJournalObservation::Matching.*?"
   r"prepare_transaction_payload\(&transaction\).*?"
   r"status\.is_none\(\).*?refresh\(&client.*?now = current_unix_ms\(\).*?"
   r"verify_for_authorization_execution\(.*?"
   r"submit_prepared_transaction_payload\(&prepared\)"),
  "private canary journal committed before minimal reservation disclosure and POST",
 )
 reservation_journal = rollout.split(
  "fn verify_canary_authorization_submission_transaction", 1
 )[-1].split("fn verify_canary_submission_journal_bytes", 1)[0]
 rp(
  reservation_journal, KRC, e,
  (r"transaction\.authority\(\) != &authorization\.artifact\.permit\(\)\.body\.canary_authority.*?"
   r"downcast_ref::<AuthorizeKagemushaTairaCanaryV4>\(\).*?"
   r"reservation\.reservation\(\) != authorization\.artifact\.reservation\(\).*?"
   r"SignedTransaction::decode_all_versioned\(bytes\).*?encode_wire_v1\(\).*?!= bytes"),
  "exact minimal reservation transaction journal",
 )
 submit = rollout.split("impl SubmitCanary {", 1)[-1].split(
  "fn require_canary_wait_outcome", 1
 )[0]
 rp(
  submit, KRC, e,
  (r"require_canary_client_binding\(&client, &authorization\).*?"
   r"require_finalized_canary_authorization\(.*?"
   r"canary_transaction_wire\(\)\s*\.matches_bytes\(&exact_wire\).*?"
   r"prepared\.as_bytes\(\) != exact_wire\.as_slice\(\).*?"
   r"CANARY_SUBMISSION_JOURNAL_FILE_NAME.*?"
   r"inspect_canary_submission_journal\(.*?"
   r"!= SubmissionJournalObservation::Matching.*?"
   r"exact journal must be committed before the on-chain authorization reveals.*?"
   r"if initial_status\.is_none\(\).*?refresh\(&client.*?require_canary_expiry_margin.*?"
   r"fresh_time = current_unix_ms\(\).*?verify_exact\(.*?fresh_time.*?"
   r"submit_prepared_transaction_payload\(&prepared\)"),
  "precommitted exact journal and fresh verification before canary POST",
 )
 fb(
  submit, "post-authorization canary submit", e,
  "publish_canary_submission_journal(", "SubmissionJournalAction::Publish",
 )
 helpers = rollout.split("fn load_verified_canary_authorization(", 1)[-1].split(
  "fn verify_submission_journal_bytes(", 1
 )[0]
 rp(
  helpers, KRC, e,
  (r"read_root_private_artifact\(.*?\"canary authorization\".*?"
   r"verification_time_unix_ms = artifact\.permit\(\)\.body\.authorized_at_unix_ms.*?"
   r"verify_exact\(.*?verification_time_unix_ms.*?"
   r"fn verify_canary_submission_journal_bytes\(.*?"
   r"if bytes != authorization\.exact_bytes.*?verify_exact\(.*?verification_time_unix_ms.*?"
   r"fn publish_canary_submission_journal\(.*?"
   r"fresh_head\s*\.refresh\(client.*?require_canary_expiry_margin.*?"
   r"verification_time_unix_ms = current_unix_ms\(\).*?"
   r"verify_canary_submission_journal_bytes\(.*?"
   r"require_canary_authorization_wall_margin.*?"
   r"get_transaction_status_response_auto\(\s*"
   r"authorization\.verified\.canary_transaction\(\)\.hash\(\).*?is_some\(\)"),
  "structural expired-journal reconciliation and fresh precommit publication",
 )
 rp(
  helpers, KRC, e,
  (r"fn require_canary_client_binding\(.*?"
   r"client\.network_id != authorization\.verified\.network_id\(\).*?"
   r"client\.account != &authorization\.artifact\.permit\(\)\.body\.canary_authority.*?"
   r"canonical_torii_origin\(&client\.torii_url\).*?"
   r"authorization\.verified\.canonical_torii_origin\(\)"),
  "exact canary network authority and HTTPS origin client binding",
 )
 finalize = rollout.split("impl FinalizeCanaryEvidence", 1)[-1].split(
  "struct LoadedVerifiedExpectations", 1
 )[0]
 rp(
  finalize, KRC, e,
  (r"require_rollout_state_path\(\s*&self\.output,\s*"
   r"expectations\.binding\(\)\.promotion_id,\s*CANARY_EVIDENCE_FILE_NAME,\s*\).*?"
   r"preflight_root_owned_output\(&self\.output\).*?"
   r"CANARY_SUBMISSION_JOURNAL_FILE_NAME.*?"
   r"inspect_canary_submission_journal\(.*?!= SubmissionJournalObservation::Matching.*?"
   r"get_transaction_status_response_auto\(transaction\.hash\(\).*?"
   r"status\.kind != \"Applied\".*?scope != \"global\".*?resolved_from != \"state\".*?"
   r"collect_finalized_canary_evidence\(.*?require_canary_block_within_authorization.*?"
   r"canary_authorization: authorization\.verified\.authorization_identity\(\).*?"
   r"committed_transaction: fresh\.committed\.clone\(\).*?"
   r"finality_proof_chain: finality_proof_chain\.clone\(\).*?"
   r"KagemushaV4TairaCanaryEvidenceV1::try_sign\(.*?"
   r"evidence\s*\.verify_exact\(.*?publish_root_owned\(&self\.output, &bytes"),
  "promotion-keyed full canary wire block proof evidence and issuer signature",
 )
 rp(
  finalize, KRC, e,
  (r"publish_root_owned\(&self\.output, &bytes.*?context\.print_data\(&report\).*?"
   r"PublicationError::CommitUncertain.*?published canary-evidence report failed"),
  "canary evidence no-replace commit-uncertain reporting",
 )
 for section, minimum in ((sa, 3), (submit, 4)):
  if section.count("CanarySubmissionUncertain") < minimum:
   e.append(
    f"{KAGEMUSHA_ROLLOUT_COMPONENT}: canary network outcomes must remain commit-uncertain"
   )

 challenge = liveness.split(
  "impl KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1", 1
 )[-1].split("pub struct KagemushaV4PostCanaryValidatorLivenessObservationV1", 1)[0]
 rp(
  challenge, MCL, e,
  (r"self\.issuer == self\.binding\.promotion_controller.*?self\.nonce == \[0; 32\].*?"
   r"issued_at_unix_ms <= self\.canary_anchor\.canary_finalized_block_time_unix_ms.*?"
   r"KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS.*?"
   r"for \(index, target\) in self\.targets\.iter\(\)\.enumerate\(\).*?"
   r"id >= &target\.validator_id.*?target\.validator_id\.public_key\(\) == &self\.issuer.*?"
   r"prior\.canonical_torii_origin == target\.canonical_torii_origin.*?"
   r"SignatureOf::try_from_hash\(issuer\.private_key\(\), body\.signing_hash\(\)\).*?"
   r"endpoint_challenge\(.*?"
   r"KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ENDPOINT_CHALLENGE_DOMAIN.*?&bytes"),
  "issuer-signed fresh canary challenge with four distinct qualified targets",
 )
 lv = liveness.split("fn verify_evidence_body_with_trust(", 1)[-1].split(
  "fn validate_liveness_torii_origin(", 1
 )[0]
 rp(
  liveness, MCL, e,
  (r"impl<'a> LivenessTrust<'a>.*?fn from_expectations\(.*?"
   r"verified_canary\.promotion_id\(\) != expectations\.binding\(\)\.promotion_id.*?"
   r"verified_canary\.activation_expectations_artifact\(\).*?"
   r"!= expectations\.activation_expectations_artifact\(\).*?"
   r"verified_canary\.activation_transaction_intent\(\).*?"
   r"!= expectations\.activation_transaction_intent\(\).*?"
   r"canary_anchor\.activation_finality_receipt\s*"
   r"!= verified_canary\.activation_finality_receipt\(\).*?"
   r"canary_anchor\.canary_authorization != verified_canary\.authorization_identity\(\).*?"
   r"canary_anchor\.canary_transaction_intent\s*"
   r"!= verified_canary\.canary_transaction_intent\(\).*?"
   r"canary_anchor\.canary_transaction_wire != verified_canary\.canary_transaction_wire\(\).*?"
   r"canary_anchor\.canary_finalized_height != verified_canary\.finalized_height\(\).*?"
   r"canary_anchor\.canary_finalized_block_hash != verified_canary\.finalized_block_hash\(\)"),
  "exact expectations provenance and canary anchor binding",
 )
 rp(
  lv, MCL, e,
  (r"request_started_at_unix_ms < challenge\.issued_at_unix_ms.*?"
   r"response_completed_at_unix_ms >= challenge\.expires_at_unix_ms.*?"
   r"attestation\s*\.verify\(\).*?"
   r"attestation_body\.challenge != body\.endpoint_challenge.*?"
   r"attestation_body\.network_id != trust\.binding\.network_id.*?"
   r"attestation_body\.node_id != trust\.validator_ids\[index\].*?"
   r"genesis_block_hash != expected_genesis.*?config_fingerprint != expected_config_fingerprint.*?"
   r"build_fingerprint == zero.*?common_genesis.*?common_build_fingerprint.*?"
   r"tip_height < canary_height.*?"
   r"proof_count != body\.post_canary_finality_proof_chain\.len\(\).*?"
   r"BridgeFinalityVerifier::with_context\(.*?verify\(trust\.canary_finality_proof\).*?"
   r"for proof in &body\.post_canary_finality_proof_chain.*?"
   r"validate_finality_corridor\(proof, trust\).*?verifier\s*\.verify\(proof\).*?"
   r"for observation in &body\.observations.*?if tip != expected"),
  "four signed validator identities with shared canary-rooted finality and exact tips",
 )
 rp(
  lv, MCL, e,
  (r"fn verify_challenge_with_trust\(.*?"
   r"challenge\.body\.binding != \*trust\.binding.*?"
   r"challenge\.body\.canary_anchor != \*trust\.canary_anchor.*?"
   r"challenge\.body\.issuer != \*trust\.issuer.*?"
   r"zip\(&trust\.validator_ids\).*?target\.validator_id != expected"),
  "exact activation canary issuer and four-validator challenge binding",
 )
 rp(
  liveness, MCL, e,
  (r"fn validate_canary_finality_anchor\(.*?"
   r"height != trust\.canary_anchor\.canary_finalized_height.*?"
   r"block_hash != trust\.canary_anchor\.canary_finalized_block_hash.*?"
   r"block_header\.hash\(\) != trust\.canary_anchor\.canary_finalized_block_hash.*?"
   r"fn validate_finality_corridor\(.*?context\.network_id != trust\.binding\.network_id.*?"
   r"context\.mode != ConsensusMode::Permissioned.*?context\.nexus_amx_context_hash.*?"
   r"context\.execution_policy_hash != trust\.binding\.execution_policy_hash.*?"
   r"context\.da_layout != runtime\.genesis_context\.da_layout.*?"
   r"context\.roster\.len\(\) != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT.*?"
   r"member\.power != 1.*?member\.validator != expected.*?actual != &expected\.bls_pop"),
  "liveness canary anchor and exact four-validator DA Nexus PoP corridor",
 )
 rp(
  rl, KRL, e,
  (r"pub\(super\) struct FinalizeValidatorLiveness.*?"
   r"num_args = 4.*?"
   r"load_verified_canary_evidence\(.*?"
   r"CANARY_VALIDATOR_LIVENESS_EVIDENCE_FILE_NAME.*?"
   r"preflight_root_owned_output\(&self\.output\).*?"
   r"parse_validator_targets\(.*?CANARY_VALIDATOR_LIVENESS_CHALLENGE_FILE_NAME.*?"
   r"load_or_publish_challenge\(.*?"
   r"challenge\s*\.verify_bound\(.*?&canary\.verified.*?&canary\.finality_proof.*?"
   r"collect_validator_observations\(.*?collect_shared_finality_chain\(.*?"
   r"KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign\(.*?"
   r"evidence\s*\.verify_exact\(.*?publish_root_owned\(&self\.output, &bytes.*?"
   r"PublicationError::CommitUncertain"),
  "post-canary four-validator liveness phase and no-replace evidence",
 )
 loaded_canary = rl.split(
  "fn load_verified_canary_evidence(", 1
 )[-1].split("fn parse_validator_targets(", 1)[0]
 rp(
  loaded_canary, KRL, e,
  (r"read_root_private_artifact\(.*?\"canary evidence\".*?"
   r"KagemushaV4TairaCanaryEvidenceV1::decode_canonical\(&exact_bytes\).*?"
   r"artifact\s*\.verify_exact\(.*?&authorization\.exact_bytes.*?&receipt\.exact_bytes.*?"
   r"finality_proof_chain\s*\.last\(\).*?"
   r"activation_finality_receipt: KagemushaExactBytesDigestV1::from_bytes\(&receipt\.exact_bytes\).*?"
   r"canary_authorization: verified\.authorization_identity\(\).*?"
   r"canary_transaction_intent: verified\.canary_transaction_intent\(\).*?"
   r"canary_transaction_wire: authorization\.verified\.canary_transaction_wire\(\).*?"
   r"canary_finalized_height: verified\.finalized_height\(\).*?"
   r"canary_finalized_block_hash: verified\.finalized_block_hash\(\)"),
  "liveness anchor derived only from exact verified canary evidence",
 )
 lc = rl.split("fn collect_validator_observations(", 1)[-1].split(
  "enum AttestationFetch", 1
 )[0]
 rp(
  rl, KRL, e,
  (r"build_liveness_http_client\(client\.torii_request_timeout\).*?"
   r"collect_validator_observations\(\s*&http,\s*&challenge,"),
  "zero inherited credentials across direct validator origins",
 )
 rp(
  rl, KRL, e,
  (r"struct DirectLivenessHttp \{\s*client: HttpClient,\s*status_timeout: Duration,\s*\}.*?"
   r"fn build_liveness_http_client\(configured_timeout: Duration\).*?"
   r"configured_timeout == Duration::ZERO\s*\{\s*ATTESTATION_TIMEOUT\s*\} else \{\s*"
   r"configured_timeout\.min\(ATTESTATION_TIMEOUT\).*?"
   r"let status_timeout = timeout\.min\(STATUS_HINT_TIMEOUT\);.*?"
   r"redirect\(reqwest::redirect::Policy::none\(\)\).*?"
   r"retry\(reqwest::retry::never\(\)\).*?\.no_proxy\(\).*?"
   r"connect_timeout\(status_timeout\).*?\.timeout\(timeout\).*?"
   r"Ok\(DirectLivenessHttp \{\s*client,\s*status_timeout,\s*\}\).*?"
   r"let configured_timeout = Duration::from_secs\(1\);.*?"
   r"assert_eq!\(http\.status_timeout, configured_timeout\).*?"
   r"assert_eq!\(status\.timeout\(\), Some\(&configured_timeout\)\)"),
  "configured-or-60s direct client with non-expanding status timeout",
 )
 rp(
  lc, KRL, e,
  (r"http: &DirectLivenessHttp,\s*"
   r"challenge: &KagemushaV4PostCanaryValidatorLivenessChallengeV1.*?"
   r"for target in challenge\.body\.targets\.clone\(\).*?"
   r"let http = http\.clone\(\).*?"
   r"collect_validator_observation\(\s*http,\s*target,.*?"
   r"fn collect_validator_observation\(\s*http: DirectLivenessHttp,\s*"
   r"target: KagemushaV4PostCanaryValidatorLivenessTargetV1.*?"
   r"fetch_validator_status_height\(\s*&http\.client,\s*&target\.canonical_torii_origin,.*?"
   r"http\.status_timeout,\s*\).*?"
   r"fetch_validator_attestation\(\s*&http\.client,\s*&target,"),
  "zero inherited credentials across direct validator origins",
 )
 if re.search(r"\bClient\b", lc) is not None:
  e.append(
   f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: ambient Client enters direct validator collection"
  )
 fb(
  lc, "direct validator collection transport isolation", e,
  ".get_status(", "base_client", "headers.clear()",
 )
 sr = rl.split("fn build_validator_status_request(", 1)[-1].split(
  "fn fetch_validator_status_height", 1
 )[0]
 rp(
  sr, KRL, e,
  (r"http: &HttpClient,\s*canonical_torii_origin: &str,\s*status_timeout: Duration.*?"
   r'format!\(\s*"\{\}\{\}/blocks"\s*,\s*canonical_torii_origin\s*,\s*'
   r"iroha_torii_shared::uri::STATUS\s*\).*?"
   r"http\.get\(url\)\s*\.timeout\(status_timeout\)\s*"
   r"\.header\(ACCEPT, APPLICATION_JSON\)\s*"
   r'\.header\(ACCEPT_ENCODING, "identity"\)\s*\.build\(\)'),
  "direct validator status exact URL and two protocol headers",
 )
 if sr.count(".header(") != 2:
  e.append(
   f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: direct validator status requires exact two protocol headers"
  )
 fb(
  sr, "direct validator status credential isolation", e,
  ".headers(",
 )
 rp(
  rl, KRL, e,
  (r"fn fetch_validator_status_height\(.*?current_unix_ms\(\).*?"
   r"build_validator_status_request\(http, canonical_torii_origin, status_timeout\).*?"
   r"let requested_url = request\.url\(\)\.clone\(\).*?"
   r"http\s*\.execute\(request\).*?response\.url\(\) != &requested_url.*?"
   r"read_status_hint_response\(response\).*?current_unix_ms\(\).*?"
   r"norito::json::from_slice\(&exact_bytes\).*?"
   r"norito::json::to_json\(&height\).*?canonical\.as_bytes\(\) != exact_bytes"),
  "direct validator status bounded exact canonical scalar",
 )
 rp(
  rl, KRL, e,
  (r"const STATUS_HINT_MAX_BYTES: usize = 32;.*?"
   r"fn read_status_hint_response\(.*?response\.status\(\) != StatusCode::OK.*?"
   r"get_all\(CONTENT_TYPE\).*?content_type != APPLICATION_JSON.*?"
   r"content_types\.next\(\)\.is_some\(\).*?contains_key\(CONTENT_ENCODING\).*?"
   r"length > u64::try_from\(STATUS_HINT_MAX_BYTES\).*?"
   r"\.min\(STATUS_HINT_MAX_BYTES\).*?"
   r"take\(u64::try_from\(STATUS_HINT_MAX_BYTES\)\?\.saturating_add\(1\)\).*?"
   r"bytes\.is_empty\(\) \|\| bytes\.len\(\) > STATUS_HINT_MAX_BYTES"),
  "bounded identity-encoded direct validator status response",
 )
 liveness_fetch = rl.split("fn fetch_validator_attestation(", 1)[-1].split(
  "fn build_validator_attestation_request", 1
 )[0]
 rp(
  liveness_fetch, KRL, e,
  (r"build_validator_attestation_request\(http, target, challenge, height\).*?"
   r"let url = request\.url\(\)\.clone\(\).*?http\s*\.execute\(request\).*?"
   r"response\.url\(\) != &url.*?decode_canonical_with_limits\(.*?"
   r"encode_canonical\(&attestation\).*?!= exact_bytes"),
  "direct common-challenge collection with exact canonical attestations",
 )
 ar = rl.split(
  "fn build_validator_attestation_request(", 1
 )[-1].split("fn read_attestation_response", 1)[0]
 rp(
  ar, KRL, e,
  (r"BRIDGE_FINALITY_ATTESTATION\s*\.path\(\).*?"
   r"http\.get\(url\)\s*\.header\(ACCEPT, APPLICATION_NORITO\)\s*"
   r'\.header\(ACCEPT_ENCODING, "identity"\)\s*'
   r"\.header\(FINALITY_CHALLENGE_HEADER, hex::encode\(challenge\)\)\s*\.build\(\)"),
  "direct validator attestation exact three protocol headers",
 )
 if ar.count(".header(") != 3:
  e.append(
   f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: direct validator attestation requires exact three protocol headers"
  )
 fb(
  ar, "direct validator attestation credential isolation", e,
  ".headers(",
 )
 rp(
  rl, KRL, e,
  (r"OsRng\s*\.try_fill_bytes\(&mut nonce\).*?nonce != \[0; 32\].*?"
   r"redirect\(reqwest::redirect::Policy::none\(\)\).*?"
   r"retry\(reqwest::retry::never\(\)\).*?\.no_proxy\(\).*?"
   r"for target in challenge\.body\.targets\.clone\(\).*?"
   r"attestation\.body\.challenge != endpoint_challenge.*?"
   r"attestation\s*\.verify\(\)"),
  "direct common-challenge collection with exact canonical attestations",
 )
 rp(
  rl, KRL, e,
  (r"fn read_attestation_response\(.*?content_type != APPLICATION_NORITO.*?"
   r"contains_key\(CONTENT_ENCODING\).*?Cache-Control: no-store.*?"
   r"take\(u64::try_from\(maximum\)\?\.saturating_add\(1\)\)"),
  "bounded identity-encoded no-store attestation response",
 )
 rp(
  rl, KRL, e,
  (r"fn collect_shared_finality_chain\(.*?require_qualified_finality_context\(canary_proof.*?"
   r"BridgeFinalityVerifier::with_context\(.*?verifier\s*\.verify\(canary_proof\).*?"
   r"get_next_bridge_finality_proof\(height, &mut verifier\)"),
  "canary-anchored contiguous shared finality collection",
 )
 return e


def release_closure_source_errors(
 core: str, schema: str, workflow: str, overrides: dict[str, str]
) -> list[str]:
 e: list[str] = []
 it = (
  overrides[CORE_ISI_TESTS]
  if CORE_ISI_TESTS in overrides
  else read(CORE_ISI_TESTS, e)
 )
 ct = (
  overrides[KCT]
  if KCT in overrides
  else read(KCT, e)
 )
 mt = (
  overrides[AMG]
  if AMG in overrides
  else read(AMG, e)
 )
 mit = (
  overrides[AMT]
  if AMT in overrides
  else read(AMT, e)
 )
 stt = (
  overrides[CORE_STATE_TESTS]
  if CORE_STATE_TESTS in overrides
  else read(CORE_STATE_TESTS, e)
 )
 forbid_merge_conflict_markers(it, CORE_ISI_TESTS, e)
 forbid_merge_conflict_markers(
  ct, KCT, e
 )
 forbid_merge_conflict_markers(mt, AMG, e)
 forbid_merge_conflict_markers(
  mit, AMT, e
 )
 forbid_merge_conflict_markers(stt, CORE_STATE_TESTS, e)
 fb(
  ct, KCT, e,
  "#[ignore]", "#[cfg",
 )
 fb(
  mit, AMT, e,
  "#[ignore]", "#[cfg",
 )
 rq(
  core,
  CORE,
  e,
  "pub(crate) use isi::signed_kagemusha_taira_canary_wire_identity_v1;",
  CORE_ISI_TESTS_PARENT_INCLUDE,
 )
 rq(
  stt,
  CORE_STATE_TESTS,
  e,
  CORE_AUTONOMOUS_MERGE_TESTS_PARENT_INCLUDE,
 )
 rq(
  it,
  CORE_ISI_TESTS,
  e,
  CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE,
 )
 if it.count(POLICY_TESTS_INCLUDE) != 1:
  e.append(
   f"{CORE_ISI_TESTS}: expected exactly one authenticated "
   f"{POLICY_TESTS} include"
  )
 rp(
  it, CORE_ISI_TESTS, e,
  (r"#\[test\]\s*fn kagemusha_v4_activation_validates_identity_and_policy_before_state_mutation\(\).*?"
   r"#\[test\]\s*fn android_root_of_trust_must_be_hardware_verified_and_complete\(\).*?"
   r"#\[test\]\s*fn android_authorization_list_rejects_duplicate_unknown_tags\(\).*?"
   r"#\[test\]\s*fn android_application_id_must_bind_one_exact_package_and_signer\(\)"),
  "active Android authorization and activation regressions",
 )
 rq(
  mt,
  AMG,
  e,
  CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS_INCLUDE,
 )
 rp(
  mit,
  AMT,
  e,
  (r"#\[test\]\s*"
   r"fn autonomous_merge_admission_intent_producer_rejects_ordinary_external_before_effects\(\).*?"
   r"TransactionAdmissionIntent::Ordinary.*?preexecute_merge_execution_sources_into.*?"
   r"expect_err\(.*?assert_merge_queue_plan_synced_intent_error.*?"
   r"direct_committed_entrypoints\.is_empty\(\).*?external_event_buf\.is_empty\(\)"),
  "Ordinary autonomous merge producer no-effects regression",
 )
 rp(
  mit,
  AMT,
  e,
  (r"#\[test\]\s*"
   r"fn autonomous_merge_admission_intent_follower_and_historical_reject_ordinary_external\(\).*?"
   r"TransactionAdmissionIntent::QueuePlanSynced.*?"
   r"for validate_live_authority in \[true, false\].*?"
   r"QueuePlanSynced merge content remains valid.*?"
   r"merge_execution_batch_commitments_match.*?"
   r"for validate_live_authority in \[true, false\].*?expect_err\(.*?"
   r"assert_merge_queue_plan_synced_intent_error"),
  "Ordinary autonomous merge live and historical follower regression",
 )
 rp(
  ct,
  KCT,
  e,
  (r"#\[test\]\s*fn taira_canary_committed_replay_seeds_only_one_direct_wire\(\).*?"
   r"TransactionEntrypoint::External\(first\.canary_transaction\.clone\(\)\).*?"
   r"Some\(first_wire\).*?"
   r"TransactionEntrypoint::External\(second\.canary_transaction\.clone\(\)\).*?"
   r"Some\(second_wire\).*?"
   r"TransactionEntrypoint::External\(multi\).*?"
   r"kagemusha_taira_canary_wire_identity, None.*?"
   r"TransactionAdmissionIntent::QueuePlanSynced.*?"
   r"identity\(&queue_plan\).*?None.*?"
   r"TransactionEntrypoint::External\(queue_plan\).*?"
   r"kagemusha_taira_canary_wire_identity, None.*?"
   r"TransactionEntrypoint::External\(batch\).*?"
   r"kagemusha_taira_canary_wire_identity, None.*?"
   r"TransactionEntrypoint::SealedReveal\(.*?"
   r"kagemusha_taira_canary_external_entrypoint\).*?"
   r"kagemusha_taira_canary_wire_identity, None"),
  "External-only committed-replay exact-wire seeding boundaries",
 )
 rp(
  ct,
  KCT,
  e,
  (r"#\[test\]\s*fn taira_canary_sealed_reveal_validation_cannot_gain_external_provenance\(\).*?"
   r"TransactionEntrypoint::SealedCommitment\(.*?"
   r"TransactionEntrypoint::SealedReveal\(.*?"
   r"validate_transaction\(.*?expect_err\(.*?"
   r"canary_external_entrypoint_required.*?"
   r"replay_keys_before"),
  "sealed-reveal validation rejects External canary provenance",
 )
 rp(
  ct,
  KCT,
  e,
  (r"#\[test\]\s*fn taira_canary_executor_enforces_exact_wire_shape_and_proof\(\).*?"
   r"signed canary without External entrypoint provenance must fail.*?"
   r"canary_external_entrypoint_required.*?"
   r"kagemusha_taira_canary_external_entrypoint = true.*?"
   r"execute_transaction\(.*?second\.canary_transaction.*?expect_err\(.*?"
   r"execute_transaction\(&mut transaction, &authority, multi.*?expect_err\(.*?"
   r"execute_transaction\(&mut transaction, &authority, batch.*?expect_err\(.*?"
   r"execute_transaction\(.*?first\.canary_transaction.*?expect\(.*?"
   r"kagemusha_taira_canary_wire_identity, None"),
  "automatic executor exact-wire proof and shape boundaries",
 )
 rp(
  ct,
  KCT,
  e,
  (r"#\[test\]\s*fn taira_canary_nested_trigger_cannot_inherit_outer_wire\(\).*?"
   r"ExecuteTriggerEventFilter::new\(\)\.for_trigger\(trigger_id\.clone\(\)\).*?"
   r"RecordKagemushaTairaCanaryV4::new\(outer\.permit\).*?"
   r"kagemusha_taira_canary_wire_identity, None.*?"
   r"ExecuteTrigger::new\(trigger_id\).*?expect_err\(.*?"
   r"canary_authorization_missing"),
  "nested-trigger affine signed-wire rejection",
 )
 if '"pending-' in schema:
  e.append(f"{SCHEMA_GOLDEN}: public schema golden contains pending placeholder")
 sm = re.search(
  r"(?m)^\s*-\s+name:\s+Run Kagemusha release-tool regression suites\s*$\n"
  r"^\s+run:\s+>-\s*$\n(?P<body>(?:^\s{10,}\S.*(?:\n|$))+)",
  workflow,
 )
 if sm is None:
  e.append(f"{WORKFLOW}: missing active Kagemusha release-tool regression suite")
 else:
  suite = sm.group("body")
  rp(suite, WORKFLOW, e, r"(?m)^\s+python -m pytest -q\s*$",
   "active Kagemusha release pytest invocation")
  for path in KAGEMUSHA_RELEASE_PYTHON_TEST_PATHS:
   rp(suite, WORKFLOW, e, rf"(?m)^\s+{re.escape(path)}\s*$",
    f"active Kagemusha release Python test {path}")
 for command in KAGEMUSHA_RELEASE_RUST_TEST_FILTERS:
  workflow_command = f'"{command}"' if ":: " in command else command
  rp(workflow, WORKFLOW, e,
   rf"(?m)^\s*-\s+run:\s+{re.escape(workflow_command)}\s*$",
   f"active Kagemusha release Rust filter {command}")
 return e


def lifecycle_cli_source_errors(texts: dict[str, str]) -> list[str]:
 e: list[str] = []
 o = texts[OFFLINE_CLI]
 m = texts[CLI_MAIN_SHARED]
 l = texts[KAGEMUSHA_LIFECYCLE_COMPONENT]
 c = texts[CLIENT]
 hp = HTTP_DEFAULT
 h = texts[hp]
 k = texts[KAGAMI]
 a = texts[CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT]
 lp, ap = KAGEMUSHA_LIFECYCLE_COMPONENT, CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT
 rq(c, CLIENT, e, CLIENT_CANONICAL_REQUEST_AUTH_INCLUDE)
 rp(o, OFFLINE_CLI, e,
  (r"mod kagemusha_lifecycle\s*\{.*?pub\(crate\) enum KagemushaCommand.*?"
   r'"lifecycle-v4".*?LifecycleV4\(kagemusha_lifecycle::Args\).*?'
   r"LifecycleV4\(args\).*?allows_fallback_config.*?LifecycleV4\(args\) => args\.run"),
  "lifecycle route")
 rp(m, CLI_MAIN_SHARED, e,
  (r"let mut config = match Config::load\(load_path\).*?apply_transaction_overrides\(&mut config, &value\);.*?"
   r"ChainDiscriminantGuard::enter\(config\.account_chain_discriminant\);.*?"
   r"if let Command::Offline\(command\) = &args\.command \{\s*command\s*"
   r"\.preflight_before_operator_key_load\(\).*?\}\s*let operator_key_pair = args\s*"
   r"\.operator_private_key_file.*?\.map\(operator_key::load_operator_key_pair\).*?"
   r"let mut context = PrintJsonContext.*?args\.command\s*\.run\(&mut context\)"),
  "guarded key-free signing preflight")
 if m.count("ChainDiscriminantGuard::enter(config.account_chain_discriminant)") != 1:
  e.append(f"{CLI_MAIN_SHARED}: configured chain guard must enter exactly once before key access")
 rp(o, OFFLINE_CLI, e,
  (r"pub\(crate\) fn preflight_before_operator_key_load\(&self\).*?match self \{\s*"
   r"Self::Kagemusha\(command\) => command\.preflight_before_operator_key_load\(\),\s*"
   r"Self::Petal\(_\) => Ok\(\(\)\).*?fn preflight_before_operator_key_load\(&self\).*?"
   r"Self::LifecycleV4\(args\) => args\.preflight_before_operator_key_load\(\),\s*"
   r"Self::RolloutV4\(_\) => Ok\(\(\)\)"), "narrow key-free signing preflight")
 rp(l, lp, e,
  (r"enum Command\s*\{.*?Prepare\(Prepare\).*?SignFeeQuote\(SignFeeQuote\).*?FinalizeFeeQuote\(FinalizeFeeQuote\).*?"
   r"SignTransaction\(SignTransaction\).*?AssembleTransaction\(AssembleTransaction\).*?SubmitTransaction\(SubmitTransaction\).*?"
   r"impl Run for Args.*?Command::Prepare.*?Command::SignFeeQuote.*?Command::FinalizeFeeQuote.*?"
   r"Command::SignTransaction.*?Command::AssembleTransaction.*?Command::SubmitTransaction"),
  "six phases")
 rp(l.split("impl Args", 1)[-1].split("#[derive(Subcommand", 1)[0], lp, e,
  (r"allows_fallback_config.*?matches!\(\s*&self\.command,\s*Command::SignFeeQuote\(_\)\s*"
   r"\| Command::SignTransaction\(_\)\s*\| Command::AssembleTransaction\(_\)\s*\)"),
  "pinned signing")
 rp(l.split("impl Args", 1)[-1].split("#[derive(Subcommand", 1)[0], lp, e,
  (r"preflight_before_operator_key_load.*?match &self\.command \{\s*"
   r"Command::SignFeeQuote\(args\) => args\.validated_signing_input\(\)\.map\(drop\),\s*"
   r"Command::SignTransaction\(args\) => args\.validated_signing_input\(\)\.map\(drop\),\s*"
   r"Command::Prepare\(_\)\s*\| Command::FinalizeFeeQuote\(_\)\s*"
   r"\| Command::AssembleTransaction\(_\)\s*\| Command::SubmitTransaction\(_\) => Ok\(\(\)\)"),
  "narrow key-free signing preflight")
 rp(
  l.split("fn lifecycle_kind_for_type_id", 1)[-1].split("fn require_lifecycle_payload", 1)[0], lp, e,
  (r"ActivateKagemushaRecursiveReleaseV4.*?Stage.*?"
   r"EnableKagemushaRecursiveIssuanceV4.*?Enable.*?CancelKagemushaRecursiveReleaseV4.*?Cancel.*?"
   r"DeactivateKagemushaRecursiveIssuanceV4.*?Deactivate"),
  "kinds",
 )
 p = l.split("impl Prepare {", 1)[-1].split("struct SignFeeQuote", 1)[0]
 rq(p, lp, e, "let instructions: Vec<InstructionBox> = crate::parse_json",
  "let [instruction] = instructions.as_slice() else", "require_lifecycle_instruction(instruction, self.kind)?;",
  ".with_instructions(instructions)", ".with_admission_intent(TransactionAdmissionIntent::Ordinary)",
  "require_lifecycle_payload(&payload, self.kind)?;")
 rp(
  l, lp, e,
  (r"fn require_lifecycle_payload\(.*?TransactionAdmissionIntent::Ordinary.*?attachments\.is_some.*?"
   r"require_multisig_policy.*?Executable::Instructions\(instructions\).*?"
   r"let \[instruction\] = instructions\.as_ref\(\) else.*?require_lifecycle_instruction"),
  "one Ordinary instruction",
 )
 rq(l, lp, e,
  "const LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS: u64 = 60_000;",
  "fn fee_quote_draft_binds_request_network_authority_payload_url_and_response_intent()",
  "fn fee_quote_timestamp_rejects_stale_and_future_material()",
  "canonical_network_request_hash(network_id, &HttpMethod::POST, &url, &body)?;",
  "timestamp_ms.abs_diff(now_ms) > LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS")
 sf = l.split("struct SignFeeQuote {", 1)[-1].split("struct FinalizeFeeQuote", 1)[0]
 st = l.split("struct SignTransaction {", 1)[-1].split("struct AssembleTransaction", 1)[0]
 for s, n, decode, extra in (
  (sf, "draft", r"decode_canonical\(&draft_bytes", r"validate_fee_quote_draft\(&draft, self\.kind\)"),
  (st, "payload", r"decode_transaction_payload_archive\(&payload_bytes",
   r"require_lifecycle_payload\(&payload, self\.kind\)")):
  rp(s, lp, e,
   (rf"expected_network_id: String.*?expected_{n}_sha256: String.*?read_bounded_stable\(.*?"
    rf"require_expected_artifact_sha256\(\s*&{n}_bytes,\s*&self\.expected_{n}_sha256,\s*"
    rf'"--expected-{n}-sha256".*?{decode}.*?require_expected_signing_network\(\s*'
    rf"&self\.expected_network_id,.*?{extra}.*?require_expected_authority\(.*?"
    r"context\.operator_key_pair\(\)"), "pinned signing")
 rp(l, lp, e,
  (r"fn require_expected_signing_network\(.*?let expected: iroha::data_model::NetworkId =\s*"
   r"literal\.parse\(\).*?payload\.network_id\(\) != Some\(&expected\).*?"
   r"fn require_expected_artifact_sha256\(.*?hex::decode\(literal\).*?"
   r"let expected: \[u8; 32\] = decoded\s*\.try_into\(\).*?"
   r"let actual: \[u8; 32\] = Sha256::digest\(bytes\)\.into\(\);.*?actual != expected"),
  "pinned signing")
 rp(l, lp, e,
  (r"fn require_expected_authority\(\s*literal: &str,\s*payload: &TransactionPayload\s*\).*?"
   r"crate::resolve_account_id_with\(literal\).*?require_multisig_policy\(&expected\).*?"
   r"require_authority_match\(&expected, payload\)"), "guarded key-free signing preflight")
 rp(l, lp, e,
  (r"fn validate_fee_quote_draft\(.*?Url::parse\(&draft\.fee_quote_url\).*?"
   r"require_secure_fee_quote_origin\(&url\).*?fn require_secure_fee_quote_origin\(.*?"
   r"Host::Domain\(domain\).*?domain == \"localhost\".*?Host::Ipv4\(address\).*?"
   r"address\.is_loopback\(\).*?Host::Ipv6\(address\).*?address\.is_loopback\(\).*?"
   r"None => false.*?"
   r"url\.scheme\(\) != \"https\" && \(url\.scheme\(\) != \"http\" \|\| !loopback\).*?"
   r"fn fee_quote_url\(.*?\.join\(.*?require_secure_fee_quote_origin\(&url\)"),
  "secure fee-quote transport")
 q = c.split("pub fn quote_fees_with_multisig_witness(", 1)[-1].split(
  "/// Convenience: POST `/v1/assets/aliases/resolve`", 1)[0]
 rp(q, CLIENT, e,
  (r"canonical_network_request_hash\(.*?witness\.canonical_request_hash != expected_hash.*?"
   r"canonical_request_witness_header_value\(witness\).*?let cleartext = url\.scheme\(\) == \"http\";.*?"
   r"if !cleartext && url\.scheme\(\) != \"https\".*?request_without_canonical_account_auth.*?"
   r"\.body\(body\).*?\.max_response_bytes\(FEE_QUOTE_RESPONSE_MAX_BYTES\);.*?"
   r"let request = if cleartext \{\s*request\.direct_loopback\(\)\s*\} else \{\s*request\s*\};.*?"
   r"self\.send_builder\(request\)"), "proxy-free loopback fee-quote dispatch")
 rp(h, hp, e,
  (r"fn direct_loopback\(self\).*?Host::Domain\(domain\).*?domain == \"localhost\".*?"
   r"Host::Ipv4\(address\).*?address\.is_loopback\(\).*?Host::Ipv6\(address\).*?"
   r"address\.is_loopback\(\).*?None => false.*?scheme\(\) != \"http\" \|\| !loopback.*?"
   r"pending\.direct_loopback = true.*?fn into_response\(self\).*?let direct_client = direct_loopback\s*"
   r"\.then\(\|\| build_direct_loopback_http_client\(&url\)\)\s*\.transpose\(\)\?;.*?"
   r"Some\(client\) => client,\s*None => http_client\(\).*?client\.request\(method\.clone\(\), url\.clone\(\)\)"),
  "proxy-free loopback HTTP selection before dispatch")
 rp(h, hp, e,
  (r"fn http_client\(\).*?get_or_init\(build_http_client\).*?fn blocking_http_client_builder\(\).*?"
   r"fn build_http_client\(\).*?blocking_http_client_builder\(\)\s*\.build\(\).*?"
   r"fn build_direct_loopback_http_client\(url: &Url\).*?blocking_http_client_builder\(\)\.no_proxy\(\).*?"
   r"Host::Domain\(\"localhost\"\).*?127, 0, 0, 1.*?0, 0, 0, 0, 0, 0, 0, 1.*?"
   r"resolve_to_addrs\(\"localhost\", &addresses\).*?Host::Ipv4\(address\).*?address\.is_loopback\(\).*?"
   r"Host::Ipv6\(address\).*?address\.is_loopback\(\).*?_ => return Err.*?builder\s*\.build\(\)"),
  "proxy-safe loopback client")
 if h.count(".no_proxy()") != 1:
  e.append(f"{hp}: sole proxy-disabled client")
 rq(h, hp, e, "direct_loopback: pending.direct_loopback", "direct_loopback: false")
 rq(q, CLIENT, e, "payload.network_id() != Some(&self.network_id)",
  "witness.subject_account != payload.authority", "AccountController::Multisig(policy)",
  "witness.signatures.len() < 2", "policy.members().len()",
  "pair[0].signer >= pair[1].signer", "canonical_request_witness_message(witness)?;",
  "member.public_key() == &entry.signer", "verify_signature_for_admission(",
  "checked_add(u32::from(member.weight()))", "total_weight < u32::from(policy.threshold())",
  "witness.canonical_request_hash != expected_hash",
  "request_without_canonical_account_auth(HttpMethod::POST, url)", "HEADER_WITNESS")
 submit = "fn secure_transaction_submission_uses_direct_loopback(" + c.split("fn secure_transaction_submission_uses_direct_loopback(", 1)[-1].split(
  "/// Submit a prebuilt transaction with the asynchronous HTTP transport.", 1)[0]
 rp(submit, CLIENT, e,
  (r"fn secure_transaction_submission_uses_direct_loopback\(.*?url\.host\(\)\.is_none\(\).*?"
   r"!url\.username\(\)\.is_empty\(\).*?url\.password\(\)\.is_some\(\).*?"
   r"url\.query\(\)\.is_some\(\).*?url\.fragment\(\)\.is_some\(\).*?"
   r"Host::Domain\(domain\).*?domain == \"localhost\".*?Host::Ipv4\(address\).*?"
   r"address\.is_loopback\(\).*?Host::Ipv6\(address\).*?address\.is_loopback\(\).*?"
   r'"https" => Ok\(false\).*?"http" if loopback => Ok\(true\).*?_ => Err'),
  "exact verified-submit origin")
 rp(submit, CLIENT, e,
  (r"fn exact_single_response_header.*?get_all\(name\)\.iter\(\).*?values\.next\(\)\.is_some\(\).*?"
   r"impl VerifiedTransactionResponseHandler.*?response\.body\(\)\.len\(\) > TRANSACTION_SUBMISSION_RESPONSE_MAX_BYTES.*?"
   r"response\.status\(\) != StatusCode::ACCEPTED.*?contains_key\(\"x-iroha-reject-code\"\).*?"
   r"exact_single_response_header\(response, \"content-type\"\).*?eq_ignore_ascii_case\(APPLICATION_NORITO\).*?"
   r"exact_single_response_header\(response, TRANSACTION_ENTRYPOINT_HASH_HEADER\).*?"
   r"entrypoint_hash != expected_identity\.entrypoint_hash\.to_string\(\).*?"
   r"exact_single_response_header\(response, SIGNED_TRANSACTION_HASH_HEADER\).*?"
   r"signed_transaction_hash != expected_identity\.signed_transaction_hash\.to_string\(\).*?"
   r"decode_and_verify_sorafs_orderbook_submission_receipt_v1\(\s*response\.body\(\),\s*"
   r"expected_identity,\s*expected_receipt_signer"), "authenticated lifecycle receipt")
 rp(submit, CLIENT, e,
  (r"pub fn submit_prepared_kagemusha_lifecycle_payload\(.*?"
   r"let canonical = Self::prepare_transaction_payload\(transaction\);.*?"
   r"canonical\.hash\(\) != payload\.hash\(\) \|\| canonical\.as_bytes\(\) != payload\.as_bytes\(\).*?"
   r"secure_transaction_submission_uses_direct_loopback\(&self\.torii_url\)\?;.*?"
   r"join_torii_url\(&self\.torii_url, torii_uri::KAGEMUSHA_LIFECYCLE_TRANSACTION\).*?"
   r"ensure_transaction_submit_compatibility_with_transport\(direct_loopback, true\)\?;.*?"
   r"DefaultRequestBuilder::new\(HttpMethod::POST, url\).*?header\(\"Content-Type\", APPLICATION_NORITO\).*?"
   r"header\(\"Accept\", APPLICATION_NORITO\).*?body\(payload\.as_bytes\(\)\.to_vec\(\)\).*?"
   r"max_response_bytes\(TRANSACTION_SUBMISSION_RESPONSE_MAX_BYTES\).*?"
   r"if direct_loopback \{\s*request = request\.direct_loopback\(\);.*?"
   r"uncertain_context.*?do not retry automatically.*?request\s*\.build\(\)\?\s*\.send\(\).*?"
   r"\.wrap_err_with\(\|\| uncertain_context\.clone\(\)\).*?"
   r"entrypoint_hash: transaction\.hash_as_entrypoint\(\).*?signed_transaction_hash: payload\.hash\(\).*?"
   r"VerifiedTransactionResponseHandler::handle\(&response, &identity, expected_receipt_signer\).*?"
   r"\.wrap_err_with\(\|\| uncertain_context\)"),
  "fresh direct exact-byte lifecycle submit with authenticated receipt")
 rq(submit, CLIENT, e, "DataModelCompatibility::SubmitCompatible if !require_fresh_probe",
  "DataModelCompatibility::Incompatible(err) if !require_fresh_probe",
  "DataModelCompatibility::SchemaIncompatible(err) if !require_fresh_probe",
  "get_node_capabilities_json_with_transport(direct_loopback)?")
 rq(c, CLIENT, e, 'let torii_origin = self.torii_url.origin().ascii_serialization();',
  '.field("headers", &"<redacted>")', "fn client_debug_redacts_runtime_headers_and_torii_url_secrets()")
 rq(c, CLIENT, e,
  "fn verified_lifecycle_submit_pins_direct_transport_body_and_receipt()",
  "fn verified_lifecycle_submit_marks_acknowledgement_validation_outcome_uncertain()",
  "fn fresh_submit_compatibility_probe_replaces_cached_failures()",
  "fn ordinary_submit_compatibility_preserves_cached_failures_without_io()",
  "fn verified_lifecycle_submit_rejects_unsafe_origins_before_io()",
  "fn verified_transaction_response_rejects_missing_or_untrusted_evidence()")
 f = a.split("struct CanonicalRequestWitnessPayloadV1", 1)[-1].split(
  "validate_canonical_request_witness_for_encoding(witness)?;", 1)[0]
 rq(f, ap, e,
  "schema_version: u16", "subject_account: AccountId", "timestamp_ms: u64",
  "nonce: String", "canonical_request_hash: Hash")
 fb(f, ap, e, "signatures:")
 rq(a, ap, e, "pub fn canonical_request_witness_message(")
 a = a.split("pub fn canonical_request_witness_message(", 1)[-1].split("pub fn canonical_request_witness_header_value(", 1)[0]
 rq(a, ap, e, "validate_canonical_request_witness_for_encoding(witness)?;",
  "schema_version: witness.schema_version", "subject_account: witness.subject_account.clone()",
  "timestamp_ms: witness.timestamp_ms", "nonce: witness.nonce.clone()",
  "canonical_request_hash: witness.canonical_request_hash",
  "to_bytes_bounded(", "CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1")
 rp(
  l, lp, e,
  (r"fn assemble_transaction\(.*?signatures\.len\(\) < 2.*?pair\[0\]\.signer >= pair\[1\]\.signer.*?"
   r"MultisigSignatures::new.*?from_payload.*?with_multisig_signatures.*?verify_signature\(\).*?"
   r"fn prepare_exact_lifecycle_submission\(.*?decode_all_versioned.*?canonical != bytes.*?"
   r"require_lifecycle_transaction.*?prepare_transaction_payload.*?prepared\.as_bytes\(\) != bytes"),
  "exact raw assembly",
 )
 rp(l, lp, e,
  (r"struct SubmitTransaction.*?expected_receipt_signer: String.*?receipt_output: PathBuf.*?"
   r"if !self\.write_authorized.*?prepare_exact_lifecycle_submission\(&bytes, self\.kind\)\?;.*?"
   r"require_expected_authority\(&self\.governance_authority, transaction\.payload\(\)\)\?;.*?"
   r"self\.expected_receipt_signer\.parse\(\).*?expected_receipt_signer\.to_string\(\) != self\.expected_receipt_signer.*?"
   r"require_publication_destination_absent\(&self\.receipt_output.*?"
   r"transaction\.network_id\(\) != Some\(&client\.network_id\).*?"
   r"submit_prepared_kagemusha_lifecycle_payload\(&transaction, &prepared, &expected_receipt_signer\).*?"
   r"encode_and_publish_verified_lifecycle_receipt\(&receipt, transaction_hash\.clone\(\), &self\.receipt_output\).*?"
 r"println_data\(transaction_hash\).*?do not retry automatically"), "authenticated dedicated lifecycle submit")
 rq(l, lp, e, "enum LifecyclePostAcknowledgementError", "was durably acknowledged",
  "do not retry automatically", "do not treat", "fn verified_receipt_publication_race_is_post_acknowledgement_and_no_retry()")
 publication = l.split("enum LifecycleArtifactPublicationError", 1)[-1].split(
  "fn read_bounded_stable", 1)[0]
 rp(publication, lp, e,
  (r"PreCommit \{.*?CommitUncertain \{.*?fn publish_no_replace\(.*?"
   r"publish_no_replace_with_hooks\(\s*path,\s*bytes,\s*label,\s*\|_, _\| Ok\(\(\)\),\s*"
   r"rename_lifecycle_staging_no_replace,\s*\|_\| Ok\(\(\)\),\s*File::sync_all,\s*\)\s*"
   r"\.map_err\(eyre::Report::new\).*?"
   r"#\[cfg\(not\(unix\)\)\].*?production lifecycle artifact publication requires Unix "
   r"descriptor-relative no-replace APIs"), "bounded archive")
 if publication.count("production lifecycle artifact publication requires Unix descriptor-relative no-replace APIs") != 2:
  e.append(f"{lp}: exact non-Unix fail-closed publication errors")
 rp(publication, lp, e,
  (r"impl PinnedLifecyclePublicationParent.*?fn open\(.*?fs::canonicalize\(&requested_path\).*?"
   r"OFlags::RDONLY \| OFlags::DIRECTORY \| OFlags::NOFOLLOW \| OFlags::CLOEXEC.*?"
   r"for component in canonical_path\.components\(\)\.skip\(1\).*?"
   r"statat\(&file, name, AtFlags::SYMLINK_NOFOLLOW\).*?openat\(.*?"
   r"OFlags::RDONLY \| OFlags::DIRECTORY \| OFlags::NOFOLLOW \| OFlags::CLOEXEC.*?"
   r"lifecycle_directory_snapshot_matches_stat\(next_snapshot, &before\).*?"
   r"lifecycle_directory_snapshot_matches_stat\(next_snapshot, &after\).*?"
   r"next_snapshot\.validate_trusted.*?parent\.verify_path_identity_against\(snapshot\).*?"
   r"fn snapshot_after_staging\(.*?\.checked_sub\(self\.snapshot\.links\).*?"
   r"\.is_some_and\(\|delta\| delta <= 1\).*?self\.verify_path_identity_against\(current\).*?"
   r"fn verify_path_identity_against\(.*?opened != Some\(expected_parent\).*?"
   r"if index == final_index.*?current == expected_parent.*?"
   r"expected\.matches_identity\(current\) && current\.links > 0.*?"
   r"named\.file_type\(\)\.is_symlink\(\) \|\| !matches.*?"
   r"fs::canonicalize\(&self\.requested_path\).*?resolved != self\.canonical_path"),
  "bounded archive")
 rq(publication, lp, e, "fn lifecycle_file_snapshot_matches_stat(",
  "snapshot.same_inode(stat)", "stat.st_mode, snapshot.mode", "stat.st_uid, snapshot.uid",
  "stat.st_gid, snapshot.gid", "stat.st_nlink, snapshot.links", "stat.st_size, snapshot.length",
  "fn lifecycle_precommit_after_cleanup(", "Ok(_) => lifecycle_publication_precommit(",
  "Err(cleanup) => lifecycle_publication_precommit(")
 rp(publication, lp, e,
  (r"enum LifecycleStagingCleanupOutcome \{\s*Removed,\s*AlreadyAbsent,\s*\}.*?"
   r"fn cleanup_lifecycle_staging\(.*?statat\(&parent\.file, staging_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"Err\(error\) if error == rustix::io::Errno::NOENT.*?AlreadyAbsent.*?"
   r"if !expected\.same_inode\(&named\).*?unlinkat\(&parent\.file, staging_name, AtFlags::empty\(\)\).*?"
   r"parent\s*\.file\s*\.sync_all\(\).*?LifecycleStagingCleanupOutcome::Removed"),
  "rename-error reconciliation")
 ren = publication.split("fn rename_lifecycle_staging_no_replace(", 1)[-1].split(
  "fn reconcile_lifecycle_rename_error(", 1)[0]
 rp(ren, lp, e, r"renameat_with\(.*?rustix::fs::RenameFlags::NOREPLACE.*?wrap_err",
  "bounded archive no-replace rename wrapper")
 rp(publication, lp, e,
  (r"fn reconcile_lifecycle_rename_error\(.*?match statat\(&parent\.file, target_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"Ok\(binding\) if staged\.same_inode\(&binding\).*?commit_uncertain.*?"
   r"Ok\(_\) \| Err\(rustix::io::Errno::NOENT\) => \{\}.*?Err\(error\).*?commit_uncertain.*?"
   r"statat\(&parent\.file, staging_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"Ok\(binding\) if lifecycle_file_snapshot_matches_stat\(staged, &binding\).*?"
   r"Ok\(_\).*?commit_uncertain.*?Err\(error\).*?commit_uncertain.*?"
   r"verify_path_identity_against\(publication_parent\).*?cleanup_lifecycle_staging\(parent, staging_name, staged\).*?"
   r"Ok\(LifecycleStagingCleanupOutcome::Removed\) => \{\}.*?AlreadyAbsent.*?commit_uncertain.*?"
   r"Err\(error\).*?commit_uncertain.*?let destination_is_not_owned\s*=.*?"
   r"Ok\(binding\) => !staged\.same_inode\(&binding\).*?NOENT => true.*?Err\(error\).*?commit_uncertain.*?"
   r"if !destination_is_not_owned.*?commit_uncertain.*?verify_path_identity_against\(publication_parent\).*?"
   r"commit_uncertain.*?lifecycle_publication_precommit\("), "rename-error reconciliation")
 rp(publication, lp, e,
  (r"fn verify_lifecycle_artifact_file\(.*?statat\(&parent\.file, name, AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"if !lifecycle_file_snapshot_matches_stat\(expected, &before\).*?openat\(.*?"
   r"OFlags::RDONLY \| OFlags::NOFOLLOW \| OFlags::CLOEXEC.*?"
   r"LifecycleFileSnapshot::from_metadata\(.*?!= Some\(expected\).*?"
   r"\.take\(limit\).*?read_to_end\(&mut observed\).*?observed != bytes.*?"
   r"after != Some\(expected\) \|\| !lifecycle_file_snapshot_matches_stat\(expected, &linked_after\)"),
  "bounded archive")
 rp(publication, lp, e,
  (r"fn publish_no_replace_with_hooks.*?PinnedLifecyclePublicationParent::open\(parent_path\).*?"
   r"statat\(&parent\.file, target_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"OFlags::RDWR \| OFlags::CREATE \| OFlags::EXCL \| OFlags::NOFOLLOW \| OFlags::CLOEXEC.*?"
   r"Mode::from_raw_mode\(0o600\).*?initial\.validate\(0o600, 0,.*?"
   r"snapshot_after_staging\(\).*?staging\s*\.write_all\(bytes\).*?staging\s*\.sync_all\(\).*?"
   r"fchmod\(&staging, Mode::from_raw_mode\(0o400\)\).*?staging\s*\.sync_all\(\).*?"
   r"snapshot\.validate\(0o400, expected_length,.*?before_rename\(&mut staging, &staging_path\).*?"
   r"parent\.verify_path_identity_against\(publication_parent\).*?"
   r"verify_lifecycle_artifact_file\(\s*&parent,\s*&staging_name,\s*staged,\s*bytes,.*?"
   r"if let Err\(error\) = rename\(&parent\.file, &staging_name, target_name\).*?"
   r"drop\(staging\);\s*return Err\(reconcile_lifecycle_rename_error\(\s*&parent,\s*"
   r"publication_parent,\s*&staging_name,\s*target_name,\s*staged,\s*path,\s*label,\s*&error,\s*\)\);\s*"
   r"\}\s*drop\(staging\);.*?after_rename\(&final_path\).*?"
   r"verify_lifecycle_artifact_file\(\s*&parent,\s*target_name,\s*staged,\s*bytes,.*?"
   r"sync_parent\(&parent\.file\).*?parent\.verify_path_identity_against\(publication_parent\).*?"
   r"verify_lifecycle_artifact_file\(\s*&parent,\s*target_name,\s*staged,\s*bytes,.*?"
   r"committed\.map_err\(\|error\| lifecycle_publication_commit_uncertain\(&final_path, label, error\)\)"),
  "bounded archive")
 rp(l, lp, e,
  (r"fn read_bounded_stable\(.*?is_symlink\(\).*?length == 0 \|\| length > maximum.*?"
   r"OFlags::NOFOLLOW.*?same_file_snapshot"), "bounded archive")
 rq(l, lp, e,
  "fn classifier_covers_stage_enable", "fn fee_quote_witness_enforces_distinct_floor",
  "fn nonordinary_and_multi_instruction", "fn raw_submission_preserves_authorized_wire",
  "fn archive_publication_is_no_replace",
  "fn lifecycle_publication_rejects_partial_staged_write_before_commit",
  "fn lifecycle_publication_rejects_parent_substitution_before_commit",
  "fn lifecycle_publication_noreplace_race_is_precommit_and_preserves_destination",
  "fn lifecycle_publication_rename_error_with_intact_staging_is_precommit",
  "fn lifecycle_publication_lost_rename_ack_is_commit_uncertain",
  "fn lifecycle_publication_missing_names_after_rename_error_is_commit_uncertain",
  "fn lifecycle_publication_parent_drift_preserves_staging_during_rename_reconciliation",
  "fn lifecycle_publication_reports_post_rename_replacement_as_commit_uncertain",
  "fn lifecycle_publication_reports_parent_sync_failure_as_commit_uncertain",
  "fn lifecycle_signing_pins_reject_wrong_network_and_artifact_digest",
   "fn lifecycle_fee_quote_transport_requires_https_except_exact_loopback")
 rq(l, lp, e, "fn lifecycle_operator_key_preflight_rejects_bad_digest_and_network",
  "fn lifecycle_operator_key_preflight_precedes_key_file_load_in_root")
 rq(h, hp, e, "fn direct_loopback_builder_is_fail_closed",
  "fn kagemusha_lifecycle_loopback_transport_ignores_proxy")
 rr = k.split("enum NoReplaceRenameNameStateV1", 1)[-1].split("struct PinnedPromotionParentV1", 1)[0]
 rp(rr, KAGAMI, e,
  (r"fn classify_failed_no_replace_rename_v1\(.*?matches!\(\s*target,\s*"
   r"Some\(NoReplaceRenameNameStateV1::Missing \| NoReplaceRenameNameStateV1::Foreign\)\s*\)\s*"
   r"&& staging == Some\(NoReplaceRenameNameStateV1::Owned\).*?PreCommit.*?else \{.*?CommitUncertain.*?"
   r"fn reconcile_failed_no_replace_rename_v1.*?classify_failed_no_replace_rename_v1\(\s*"
   r"target\.as_ref\(\)\.ok\(\)\.copied\(\),\s*staging\.as_ref\(\)\.ok\(\)\.copied\(\).*?"
   r"if disposition == FailedNoReplaceRenameDispositionV1::PreCommit.*?cleanup_exact_owned_staging\(\).*?"
   r"Ok\(\(\)\) => FailedNoReplaceRenameReconciliationV1::PreCommit.*?Err\(error\) =>.*?CommitUncertain.*?"
   r"\{rename_error_chain\}.*?\{error:#\}.*?CommitUncertain.*?left all names untouched.*?"
   r"fn inspect_no_replace_rename_name_v1.*?statat\(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW\).*?"
   r"Ok\(stat\) if matches_owned\(&stat\).*?Owned.*?Ok\(_\).*?Foreign.*?Err\(error\) if error == rustix::io::Errno::NOENT.*?Missing"),
  "failed no-replace publication reconciliation")
 pf = k.split("fn write_new_durable_file_with_hooks_v1", 1)[-1].split(
  "fn promotion_file_snapshot_has_stat_identity_v1", 1)[0]
 rp(pf, KAGAMI, e,
  (r"RenameFlags::NOREPLACE.*?format!\(\"\{rename_error:#\}\"\).*?"
   r"target_observation.*?target_name.*?promotion_file_snapshot_has_stat_identity_v1\(snapshot, stat\).*?"
   r"staging_observation.*?temporary_name.*?release_circuit_params_file_snapshot_matches_stat_v1\(snapshot, stat\).*?"
   r"reconcile_failed_no_replace_rename_v1.*?verify_path_identity_against\(publication_parent_snapshot\).*?"
   r"verify_pinned_file_contents_v1\(.*?temporary_name.*?snapshot.*?bytes.*?post-error staged promotion record.*?"
   r"cleanup_promotion_temporary_v1.*?parent\.verify_path_identity\(\).*?PreCommit => Err\(rename_error\).*?"
   r"CommitUncertain \{ reason \}.*?DurableFilePublicationOutcomeV1::CommitUncertain"),
  "promotion-record rename-error reconciliation")
 cd = k.split("fn write_release_circuit_params_directory_with_hooks_v1", 1)[-1].split(
  "fn write_release_circuit_params_directory_v1", 1)[0]
 rp(cd, KAGAMI, e,
  (r"RenameFlags::NOREPLACE.*?format!\(\"\{rename_error:#\}\"\).*?"
   r"target_observation.*?promotion_directory_snapshot_has_stat_identity_v1\(complete_staging_snapshot, stat\).*?"
   r"staging_observation.*?release_circuit_params_directory_snapshot_matches_stat_v1\(\s*"
   r"complete_staging_snapshot,\s*stat.*?reconcile_failed_no_replace_rename_v1.*?"
   r"verify_path_identity_against\(publication_parent_snapshot\).*?"
   r"verify_release_circuit_params_directory_contents_v1\(.*?temporary_name.*?complete_staging_snapshot.*?"
   r"eq_snapshot.*?ep_snapshot.*?bytes.*?post-error staged circuit-parameter.*?"
   r"cleanup_release_circuit_params_staging_v1.*?parent\.verify_path_identity\(\).*?"
   r"PreCommit => Err\(rename_error\).*?CommitUncertain \{ reason \}.*?"
   r"ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain"),
  "circuit-directory rename-error reconciliation")
 rq(k, KAGAMI, e, "fn failed_no_replace_rename_is_precommit_only",
  "fn failed_no_replace_rename_cleanup_uncertainty", "fn failed_no_replace_rename_commit_uncertainty",
  "fn failed_no_replace_rename_for_release_circuit_params", "fn failed_no_replace_rename_for_promotion",
  "fn promotion_publication_rejects_same_length_staged_content_mutation",
  "fn release_circuit_params_publication_rejects_staged_leaf_substitution",
  "fn release_circuit_params_post_rename_mutation_is_commit_uncertain")
 fb(l, lp, e, "#[ignore]")
 return e


def source_provider_pipeline_errors(readiness: str) -> list[str]:
 e: list[str] = []
 rp(
  readiness,
  READINESS,
  e,
  (
   r"READINESS_SOURCE_PROVIDERS = \(\s*READINESS_SOURCE_SUPPORT,\s*"
   r"READINESS_RECURSION_SOURCE_CONTRACT,\s*"
   r"READINESS_LIFECYCLE_SOURCE_CONTRACT,\s*READINESS_SOURCE_CONTRACT,\s*\)"
  ),
  "exact authenticated source-provider set",
 )
 try:
  embedded_source = readiness.split("<<'PY'\n", 1)[1].rsplit("\nPY\n", 1)[0]
  rt = ast.parse(embedded_source)
 except (IndexError, SyntaxError) as error:
  e.append(f"{READINESS}: embedded Python is not statically parseable: {error}")
  rt = None
 if rt is not None:
  provider_stores = [
   node
   for node in ast.walk(rt)
   if isinstance(node, ast.Name)
   and isinstance(node.ctx, ast.Store)
   and node.id == "READINESS_SOURCE_PROVIDERS"
  ]
  pa = [
   node
   for node in ast.walk(rt)
   if isinstance(node, ast.Assign)
   and len(node.targets) == 1
   and isinstance(node.targets[0], ast.Name)
   and node.targets[0].id == "READINESS_SOURCE_PROVIDERS"
  ]
  expected_names = (
   "READINESS_SOURCE_SUPPORT",
   "READINESS_RECURSION_SOURCE_CONTRACT",
   "READINESS_LIFECYCLE_SOURCE_CONTRACT",
   "READINESS_SOURCE_CONTRACT",
  )
  exact_provider_tuple = (
   len(pa) == 1
   and isinstance(pa[0].value, ast.Tuple)
   and tuple(
    element.id if isinstance(element, ast.Name) else None
    for element in pa[0].value.elts
   )
   == expected_names
  )
  if len(provider_stores) != 1 or not exact_provider_tuple:
   e.append(
    f"{READINESS}: expected exactly one immutable authenticated "
    "source-provider tuple"
   )
 promotion = readiness.split("def promotion_errors() -> list[str]:", 1)[-1].split(
  "\nsource_contract_errors: list[str] = []\n", 1
 )[0]
 rp(
  readiness,
  READINESS,
  e,
  (
   r"def pin_authenticated_reviewed_source_file\(.*?"
   r"pin_regular_metadata\(path, label\).*?"
   r"require_production_root_custody\(descriptor, label\).*?"
   r"read_pinned_descriptor\(descriptor, fingerprint, maximum_bytes, label\).*?"
   r"authenticate_reviewed_source_file\(relative, payload, source_commit, maximum_bytes\).*?"
   r"retained_pins\.append\(\(path, descriptor, fingerprint, label\)\)"
  ),
  "generic root-custodied source-closure-authenticated reviewed-file loader",
 )
 rp(
  promotion,
  READINESS,
  e,
  (
   r"pin_authenticated_reviewed_source_file\(\s*SOURCE_TREE_SEAL,\s*"
   r"reviewed_source_commit,\s*MAX_REVIEWED_HELPER_BYTES,.*?"
   r"snapshot_private_bytes\(\s*source_helper_bytes,.*?"
   r"str\(trusted_source_helper_snapshot\)"
  ),
  "source-closure-authenticated source-tree helper snapshot",
 )
 rp(
  promotion,
  READINESS,
  e,
  (
   r"for relative in READINESS_SOURCE_PROVIDERS:\s*path = root / relative\s*"
   r"authenticated_readiness_source_contract_bytes\[relative\] = \(\s*"
   r"pin_authenticated_reviewed_source_file\(\s*relative,\s*"
   r"reviewed_source_commit,\s*MAX_READINESS_SOURCE_CONTRACT_BYTES,"
  ),
  "root-custodied source-closure-authenticated source-provider set",
 )
 rp(
  promotion,
  READINESS,
  e,
  (
   r"if self_test:\s*authenticated_readiness_self_test_bytes = "
   r"pin_authenticated_reviewed_source_file\(\s*READINESS_SELF_TEST,\s*"
   r"reviewed_source_commit,\s*MAX_READINESS_SELF_TEST_BYTES,"
  ),
  "root-custodied source-closure-authenticated readiness self-test bytes",
 )
 dispatch = readiness.rsplit("\nsource_contract_errors: list[str] = []\n", 1)[-1]
 sdx, dispatch_separator, std = dispatch.partition(
  "\nerrors = source_contract_errors\n"
 )
 if not dispatch_separator:
  e.append(f"{READINESS}: source-provider dispatch boundary is missing")
 std = std.split("\nPY\n", 1)[0]
 rp(
  sdx,
  READINESS,
  e,
  (
   r"if mode == \"promotion\":\s*"
   r"source_contract_errors\.extend\(promotion_errors\(\)\)\s*"
   r"source_contract_bytes = authenticated_readiness_source_contract_bytes.*?"
   r"else:\s*for relative in READINESS_SOURCE_PROVIDERS:\s*try:\s*"
   r"payload = \(root / relative\)\.read_bytes\(\).*?"
   r"source_contract_bytes\[relative\] = payload"
  ),
  "authenticated promotion and candidate-only source-provider loading",
 )
 rp(
  sdx,
  READINESS,
  e,
  (
   r"support_bytes = source_contract_bytes\.get\(READINESS_SOURCE_SUPPORT\).*?"
   r"_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1.*?"
   r"compile\(support_bytes, READINESS_SOURCE_SUPPORT, \"exec\"\).*?"
   r"recursion_bytes = source_contract_bytes\.get\("
   r"READINESS_RECURSION_SOURCE_CONTRACT\).*?"
   r"_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_SOURCE_V1.*?"
   r"compile\(recursion_bytes, READINESS_RECURSION_SOURCE_CONTRACT, \"exec\"\).*?"
   r"recursion_context\.get\(\"recursion_source_contract_errors\"\).*?"
   r"lifecycle_bytes = source_contract_bytes\.get\("
   r"READINESS_LIFECYCLE_SOURCE_CONTRACT\).*?"
   r"_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_SOURCE_V1.*?"
   r"compile\(\s*lifecycle_bytes, READINESS_LIFECYCLE_SOURCE_CONTRACT, \"exec\"\s*\).*?"
   r"lifecycle_context\.get\(\"lifecycle_source_contract_errors\"\).*?"
   r"primary_bytes = source_contract_bytes\.get\(READINESS_SOURCE_CONTRACT\).*?"
   r"_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_EVALUATOR_V1.*?"
   r"_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_EVALUATOR_V1.*?"
   r"compile\(\s*primary_bytes,\s*READINESS_SOURCE_CONTRACT,\s*\"exec\",?\s*\).*?"
   r"source_contract_context\.get\(\"static_errors\"\).*?"
   r"callable\(source_contract_evaluator\).*?"
   r"source_contract_errors\.extend\(source_contract_evaluator\(\)\)"
  ),
  "authenticated byte-only support, recursion, lifecycle, and readiness source-contract dispatch",
 )
 rq(
  sdx,
  READINESS,
  e,
  "if support_bytes is None:",
  '"readiness source-support provider bytes are unavailable"',
  'source_provider_base_names = frozenset(globals()) | {',
  'support_context = dict(globals())',
  'support_context.get("source_provider_pipeline_errors")',
  '"readiness source-support provider evaluator is unavailable"',
  "if recursion_bytes is None:",
  '"recursion source-contract provider bytes are unavailable"',
  '"recursion source-contract provider evaluator is unavailable"',
  "if lifecycle_bytes is None:",
  '"lifecycle source-contract provider bytes are unavailable"',
  '"lifecycle source-contract provider evaluator is unavailable"',
  'source_contract_context = dict(support_context)',
  "source_contract_context.update(lifecycle_context)",
  "if primary_bytes is None:",
  '"readiness source-contract provider bytes are unavailable"',
  '"readiness source-contract provider evaluator is unavailable"',
 )
 rp(
  std,
  READINESS,
  e,
  (
   r"for provider_name, provider_value in source_contract_context\.items\(\):\s*"
   r"if provider_name not in source_provider_base_names:\s*"
   r"globals\(\)\[provider_name\] = provider_value.*?"
   r"self_test_context = globals\(\).*?"
   r"self_test_context\[\"errors\"\] = errors.*?"
   r"exec\(code, self_test_context, self_test_context\)"
  ),
  "authenticated provider-export readiness self-test dispatch",
 )
 try:
  sdt = ast.parse(sdx)
  stdt = ast.parse(std)
 except SyntaxError as error:
  e.append(f"{READINESS}: provider dispatch is not statically parseable: {error}")
  sdt = None
  stdt = None
 if sdt is not None:
  for target, provider in (
   ("support_bytes", "READINESS_SOURCE_SUPPORT"),
   ("recursion_bytes", "READINESS_RECURSION_SOURCE_CONTRACT"),
   ("lifecycle_bytes", "READINESS_LIFECYCLE_SOURCE_CONTRACT"),
   ("primary_bytes", "READINESS_SOURCE_CONTRACT"),
  ):
   stores = [
    node
    for node in ast.walk(sdt)
    if isinstance(node, ast.Name)
    and isinstance(node.ctx, ast.Store)
    and node.id == target
   ]
   asns = [
    node
    for node in ast.walk(sdt)
    if isinstance(node, ast.Assign)
    and len(node.targets) == 1
    and isinstance(node.targets[0], ast.Name)
    and node.targets[0].id == target
   ]
   el = False
   if len(asns) == 1:
    value = asns[0].value
    el = (
     isinstance(value, ast.Call)
     and isinstance(value.func, ast.Attribute)
     and value.func.attr == "get"
     and isinstance(value.func.value, ast.Name)
     and value.func.value.id == "source_contract_bytes"
     and len(value.args) == 1
     and isinstance(value.args[0], ast.Name)
     and value.args[0].id == provider
     and not value.keywords
    )
   if len(stores) != 1 or not el:
    e.append(
     f"{READINESS}: {target} must have exactly one authenticated "
     "provider-map assignment"
    )

  def exact_context_copy(node: ast.AST, target: str, source: str) -> bool:
   if not (
    isinstance(node, ast.Assign)
    and len(node.targets) == 1
    and isinstance(node.targets[0], ast.Name)
    and node.targets[0].id == target
    and isinstance(node.value, ast.Call)
    and isinstance(node.value.func, ast.Name)
    and node.value.func.id == "dict"
    and len(node.value.args) == 1
    and not node.value.keywords
   ):
    return False
   argument = node.value.args[0]
   if source == "globals":
    return (
     isinstance(argument, ast.Call)
     and isinstance(argument.func, ast.Name)
     and argument.func.id == "globals"
     and not argument.args
     and not argument.keywords
    )
   return isinstance(argument, ast.Name) and argument.id == source

  for target, source in (
   ("support_context", "globals"),
   ("source_contract_context", "support_context"),
  ):
   stores = [
    node
    for node in ast.walk(sdt)
    if isinstance(node, ast.Name)
    and isinstance(node.ctx, ast.Store)
    and node.id == target
   ]
   asns = [
    node
    for node in ast.walk(sdt)
    if exact_context_copy(node, target, source)
   ]
   if len(stores) != 1 or len(asns) != 1:
    e.append(
     f"{READINESS}: {target} must have exactly one isolated namespace copy"
    )

  base_name_stores = [
   node
   for node in ast.walk(sdt)
   if isinstance(node, ast.Name)
   and isinstance(node.ctx, ast.Store)
   and node.id == "source_provider_base_names"
  ]
  bna = [
   node
   for node in ast.walk(sdt)
   if isinstance(node, ast.Assign)
   and len(node.targets) == 1
   and isinstance(node.targets[0], ast.Name)
   and node.targets[0].id == "source_provider_base_names"
  ]
  ebn = False
  if len(bna) == 1:
   value = bna[0].value
   ebn = (
    isinstance(value, ast.BinOp)
    and isinstance(value.op, ast.BitOr)
    and isinstance(value.left, ast.Call)
    and isinstance(value.left.func, ast.Name)
    and value.left.func.id == "frozenset"
    and len(value.left.args) == 1
    and isinstance(value.left.args[0], ast.Call)
    and isinstance(value.left.args[0].func, ast.Name)
    and value.left.args[0].func.id == "globals"
    and not value.left.args[0].args
    and not value.left.args[0].keywords
    and not value.left.keywords
    and isinstance(value.right, ast.Set)
    and {
     element.value
     for element in value.right.elts
     if isinstance(element, ast.Constant)
     and isinstance(element.value, str)
    }
    == {"errors", "readiness_self_test_bytes", "self_test_context"}
    and len(value.right.elts) == 3
   )
  if len(base_name_stores) != 1 or not ebn:
   e.append(
    f"{READINESS}: provider namespace baseline must be captured exactly once"
   )

  def generic_candidate_read(node: ast.Call) -> bool:
   function = node.func
   receiver = function.value if isinstance(function, ast.Attribute) else None
   return (
    isinstance(function, ast.Attribute)
    and function.attr == "read_bytes"
    and isinstance(receiver, ast.BinOp)
    and isinstance(receiver.op, ast.Div)
    and isinstance(receiver.left, ast.Name)
    and receiver.left.id == "root"
    and isinstance(receiver.right, ast.Name)
    and receiver.right.id == "relative"
    and not node.args
    and not node.keywords
   )

  gr = 0
  fr: set[str] = set()
  for node in ast.walk(sdt):
   if not isinstance(node, ast.Call):
    continue
   if generic_candidate_read(node):
    gr += 1
    continue
   if isinstance(node.func, ast.Name) and node.func.id in {"open", "read"}:
    fr.add(node.func.id)
   elif isinstance(node.func, ast.Attribute) and node.func.attr in {
    "open",
    "read",
    "read_bytes",
    "read_text",
   }:
    fr.add(node.func.attr)
  if gr != 1:
   e.append(
    f"{READINESS}: source providers must have exactly one generic "
    "candidate-only filesystem read"
   )
  for call in sorted(fr):
   e.append(
    f"{READINESS}: source-provider dispatch has forbidden filesystem "
    f"call {call}"
   )
 if stdt is not None:
  self_test_context_stores = [
   node
   for node in ast.walk(stdt)
   if isinstance(node, ast.Name)
   and isinstance(node.ctx, ast.Store)
   and node.id == "self_test_context"
  ]
  self_test_context_bindings = [
   node
   for node in ast.walk(stdt)
   if isinstance(node, ast.Assign)
   and len(node.targets) == 1
   and isinstance(node.targets[0], ast.Name)
   and node.targets[0].id == "self_test_context"
   and isinstance(node.value, ast.Call)
   and isinstance(node.value.func, ast.Name)
   and node.value.func.id == "globals"
   and not node.value.args
   and not node.value.keywords
  ]
  if len(self_test_context_stores) != 1 or len(self_test_context_bindings) != 1:
   e.append(
    f"{READINESS}: self_test_context must have exactly one authenticated "
    "main-namespace binding"
   )

  def exact_provider_export(node: ast.AST) -> bool:
   if not (
    isinstance(node, ast.For)
    and isinstance(node.target, ast.Tuple)
    and tuple(
     element.id if isinstance(element, ast.Name) else None
     for element in node.target.elts
    )
    == ("provider_name", "provider_value")
    and isinstance(node.iter, ast.Call)
    and isinstance(node.iter.func, ast.Attribute)
    and isinstance(node.iter.func.value, ast.Name)
    and node.iter.func.value.id == "source_contract_context"
    and node.iter.func.attr == "items"
    and not node.iter.args
    and not node.iter.keywords
    and len(node.body) == 1
    and isinstance(node.body[0], ast.If)
    and not node.orelse
   ):
    return False
   co = node.body[0]
   if not (
    isinstance(co.test, ast.Compare)
    and isinstance(co.test.left, ast.Name)
    and co.test.left.id == "provider_name"
    and len(co.test.ops) == 1
    and isinstance(co.test.ops[0], ast.NotIn)
    and len(co.test.comparators) == 1
    and isinstance(co.test.comparators[0], ast.Name)
    and co.test.comparators[0].id == "source_provider_base_names"
    and len(co.body) == 1
    and isinstance(co.body[0], ast.Assign)
    and not co.orelse
   ):
    return False
   assignment = co.body[0]
   if len(assignment.targets) != 1 or not isinstance(
    assignment.targets[0], ast.Subscript
   ):
    return False
   target = assignment.targets[0]
   return (
    isinstance(target.value, ast.Call)
    and isinstance(target.value.func, ast.Name)
    and target.value.func.id == "globals"
    and not target.value.args
    and not target.value.keywords
    and isinstance(target.slice, ast.Name)
    and target.slice.id == "provider_name"
    and isinstance(assignment.value, ast.Name)
    and assignment.value.id == "provider_value"
   )

  provider_exports = [
   node
   for node in ast.walk(stdt)
   if exact_provider_export(node)
  ]
  global_subscript_stores = [
   node
   for node in ast.walk(stdt)
   if isinstance(node, ast.Subscript)
   and isinstance(node.ctx, ast.Store)
   and isinstance(node.value, ast.Call)
   and isinstance(node.value.func, ast.Name)
   and node.value.func.id == "globals"
  ]
  if len(provider_exports) != 1 or len(global_subscript_stores) != 1:
   e.append(
    f"{READINESS}: self-test may export only new authenticated provider symbols"
   )

  error_bindings = [
   node
   for node in ast.walk(stdt)
   if isinstance(node, ast.Assign)
   and len(node.targets) == 1
   and isinstance(node.targets[0], ast.Subscript)
   and isinstance(node.targets[0].value, ast.Name)
   and node.targets[0].value.id == "self_test_context"
   and isinstance(node.targets[0].slice, ast.Constant)
   and node.targets[0].slice.value == "errors"
   and isinstance(node.value, ast.Name)
   and node.value.id == "errors"
  ]
  if len(error_bindings) != 1:
   e.append(
    f"{READINESS}: self-test errors must be rebound exactly once"
   )
 promotion_dispatch = sdx.split("\nelse:\n", 1)[0]
 fb(
  promotion_dispatch,
  "promotion source-contract provider dispatch",
  e,
  "read_bytes",
  "compile(",
  "exec(",
  "runpy.run_path",
  "import_module",
 )
 fb(
  sdx,
  "readiness source-contract provider dispatch",
  e,
  "(root / READINESS_SOURCE_SUPPORT).read_bytes()",
  "(root / READINESS_RECURSION_SOURCE_CONTRACT).read_bytes()",
  "(root / READINESS_SOURCE_CONTRACT).read_bytes()",
  "runpy.run_path",
  "import_module",
 )
 return e
