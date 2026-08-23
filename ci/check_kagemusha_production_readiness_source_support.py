"""Authenticated source inventory for Kagemusha production readiness."""

import ast

if globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1") is not True:
    raise RuntimeError("readiness source-support provider must run inside the authenticated gate")
_readiness_source_support_source = globals().get(
    "_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1"
)
if not isinstance(_readiness_source_support_source, str) or not _readiness_source_support_source:
    raise RuntimeError("readiness source-support provider requires its exact loaded bytes")

MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_INCLUDE = 'include!("kagemusha_model.rs");'
MODEL_VERIFIER_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_release_verifier.rs"
MODEL_VERIFIER_MODULE = "mod kagemusha_release_verifier;"
MODEL_PROMOTION_RECEIPT_COMPONENT = (
    "crates/iroha_data_model/src/offline/kagemusha_promotion_receipt.rs"
)
MODEL_PROMOTION_RECEIPT_MODULE = "mod kagemusha_promotion_receipt;"
MODEL_CANARY_EVIDENCE_COMPONENT = (
    "crates/iroha_data_model/src/offline/kagemusha_canary_evidence.rs"
)
MODEL_CANARY_EVIDENCE_MODULE = "mod kagemusha_canary_evidence;"
MODEL_CANARY_LIVENESS_COMPONENT = (
    "crates/iroha_data_model/src/offline/kagemusha_post_canary_validator_liveness.rs"
)
MODEL_CANARY_LIVENESS_MODULE = "mod kagemusha_post_canary_validator_liveness;"
MODEL_ISI_OFFLINE = "crates/iroha_data_model/src/isi/offline.rs"
MODEL_ISI_MOD = "crates/iroha_data_model/src/isi/mod.rs"
PRIVACY = "crates/iroha_data_model/src/privacy.rs"
PRIVACY_PROTOCOL = "crates/iroha_data_model/src/privacy/protocol.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
CATALOG_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4_release_catalog_impl.rs"
CATALOG_INCLUDE = 'include!("kagemusha_terminal_registry_v4_release_catalog_impl.rs");\n'
CATALOG_VALIDATOR_QUALIFICATION_COMPONENT = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "kagemusha_terminal_registry_v4_validator_qualification.rs"
)
CATALOG_VALIDATOR_QUALIFICATION_INCLUDE = (
    'include!("kagemusha_terminal_registry_v4_validator_qualification.rs");\n'
)
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "kagemusha_runtime_effective_config.rs"
)
CORE_RUNTIME_EFFECTIVE_CONFIG_MODULE = "mod kagemusha_runtime_effective_config;"
CORE_KAGEMUSHA_ACTIVATION_COMPONENT = (
    "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_activation.rs"
)
CORE_KAGEMUSHA_ACTIVATION_INCLUDE = 'include!("offline/kagemusha_activation.rs");'
CORE_KAGEMUSHA_CANARY_COMPONENT = (
    "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_taira_canary.rs"
)
CORE_KAGEMUSHA_CANARY_INCLUDE = 'include!("offline/kagemusha_taira_canary.rs");'
CORE_ISI_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/isi_tests.rs"
CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "isi_kagemusha_taira_canary_context_tests.rs"
)
CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE = (
    'include!("isi_kagemusha_taira_canary_context_tests.rs");'
)
CORE_ISI_MOD = "crates/iroha_core/src/smartcontracts/isi/mod.rs"
CORE_STATE = "crates/iroha_core/src/state.rs"
CORE_COMMITTED_TX_CONTEXT = (
    "crates/iroha_core/src/state/committed_transaction_context.rs"
)
CORE_BLOCK = "crates/iroha_core/src/block.rs"
CORE_EXECUTOR = "crates/iroha_core/src/executor.rs"
STEP_TRANSITION = "crates/iroha_core/src/zk/kagemusha_step_transition.rs"
RECURSIVE_BACKEND = "crates/iroha_core/src/zk/kagemusha_v2.rs"
RECURSION_ADAPTER = "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs"
VALUE_CONTRACT = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
SCHEMA_GOLDEN = "crates/iroha_data_model/tests/offline_public_schema_golden.rs"
CONFIG = "crates/iroha_config/src/parameters/user.rs"
NODE = "crates/irohad/src/main.rs"
NODE_VALIDATOR_QUALIFICATION_COMPONENT = (
    "crates/irohad/src/main/kagemusha_validator_qualification.rs"
)
NODE_VALIDATOR_QUALIFICATION_MODULE = (
    '#[path = "main/kagemusha_validator_qualification.rs"]\n'
    "mod kagemusha_validator_qualification;"
)
NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT = (
    "crates/irohad/src/main/kagemusha_runtime_effective_config_projection.rs"
)
NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_MODULE = (
    '#[path = "main/kagemusha_runtime_effective_config_projection.rs"]\n'
    "mod kagemusha_runtime_effective_config_projection;"
)
NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT = (
    "crates/irohad/src/main/kagemusha_validator_qualification_command.rs"
)
NODE_VALIDATOR_QUALIFICATION_COMMAND_MODULE = (
    '#[path = "main/kagemusha_validator_qualification_command.rs"]\n'
    "mod kagemusha_validator_qualification_command;"
)
NODE_ROOT_OWNED_PUBLICATION_COMPONENT = (
    "crates/irohad/src/main/root_owned_artifact_publication.rs"
)
NODE_ROOT_OWNED_PUBLICATION_MODULE = (
    '#[path = "main/root_owned_artifact_publication.rs"]\n'
    "mod root_owned_artifact_publication;"
)
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"
AUTHENTICATED_TOOL_CONTROLLER = (
    "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs"
)
KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT = (
    "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller/"
    "kagemusha_promotion_publisher.rs"
)
KAGEMUSHA_PROMOTION_PUBLISHER_MODULE = (
    '#[path = "iroha_authenticated_tool_controller/'
    'kagemusha_promotion_publisher.rs"]\n'
    "mod kagemusha_promotion_publisher;"
)
KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT = (
    "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller/"
    "kagemusha_python_launcher.rs"
)
KAGEMUSHA_PYTHON_LAUNCHER_MODULE = (
    '#[path = "iroha_authenticated_tool_controller/kagemusha_python_launcher.rs"]\n'
    "mod kagemusha_python_launcher;"
)
OFFLINE_CLI = "crates/iroha_cli/src/offline.rs"
KAGEMUSHA_ROLLOUT_COMPONENT = "crates/iroha_cli/src/offline/kagemusha_rollout.rs"
KAGEMUSHA_ROLLOUT_MODULE = "mod kagemusha_rollout;"
KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT = (
    "crates/iroha_cli/src/offline/kagemusha_rollout/liveness.rs"
)
KAGEMUSHA_ROLLOUT_LIVENESS_MODULE = "mod liveness;"
KAGEMUSHA_RELEASE_RUST_TEST_FILTERS = (
    "cargo test -p iroha_data_model --test iroha_data_model_group_02 offline_public_schema_golden -- --nocapture",
    "cargo test -p iroha_data_model --lib --features transparent_api canary_ -- --nocapture",
    "cargo test -p iroha_data_model --lib --features transparent_api post_canary_liveness_rejects_receipt_and_transaction_wire_anchor_splices -- --nocapture",
    "cargo test -p iroha_data_model --lib --features transparent_api kagemusha_post_canary_validator_liveness -- --nocapture",
    "cargo test -p iroha_core --lib taira_canary -- --nocapture",
    "cargo test -p iroha_torii --lib bridge_finality_attestation_route_tests -- --nocapture",
    "cargo test -p iroha_cli --bin iroha kagemusha_rollout -- --nocapture",
)


def runtime_projection_source_errors(
    core_projection: str, wrapper: str, node: str, catalog: str, model: str
) -> list[str]:
    """Check Core-only projection derivation and verified-value signing."""
    errors: list[str] = []
    validator_check = node.split("fn validate_config_for_check_mode(", 1)[-1].split(
        "fn continue_after_full_kagemusha_check", 1
    )[0]
    require(model, MODEL, errors, "pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;")
    require_pattern(
        wrapper, NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT, errors,
        (r"pub\(super\) fn build_kagemusha_runtime_effective_config_projection_v1\(\s*"
         r"config: &Config,\s*genesis: &GenesisBlock,\s*bootstrap: &GenesisV2Bootstrap,\s*"
         r"\) -> Result<VerifiedKagemushaV4RuntimeEffectiveConfigV1, String> \{\s*"
         r"VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive\(config, genesis, bootstrap\)\s*\}"),
        "thin verified runtime-projection wrapper",
    )
    forbid(
        wrapper, NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT, errors,
        "KagemushaV4RuntimeEffectiveConfigProjectionV1", "NodeRole", "signed_genesis_validator_pops",
    )
    require_pattern(
        core_projection, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, errors,
        (r"pub struct VerifiedKagemushaV4RuntimeEffectiveConfigV1 \{\s*"
         r"projection: KagemushaV4RuntimeEffectiveConfigProjectionV1,\s*\}.*?"
         r"pub fn derive\(\s*config: &Config,\s*genesis: &GenesisBlock,\s*"
         r"bootstrap: &GenesisV2Bootstrap,\s*\).*?"
         r"let metadata = exact_signed_consensus_metadata\(genesis\)\?;.*?"
         r"let context = bootstrap\.context\(\);.*?"
         r"let staged_pops = bootstrap\.proofs_of_possession\(\);"),
        "opaque Core runtime-effective projection derivation",
    )
    require_pattern(
        core_projection, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, errors,
        (r"config\.sumeragi\.role != NodeRole::Validator.*?"
         r"metadata\.mode != SumeragiConsensusMode::Permissioned.*?"
         r"context\.mode != ConsensusMode::Permissioned.*?"
         r"metadata\.sumeragi_v2 != staged_parameters.*?"
         r"context\.roster\.len\(\) != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT.*?"
         r"staged_pops\.len\(\) != context\.roster\.len\(\).*?"
         r"context\.roster\.iter\(\)\.any\(\|member\| member\.power != 1\)"),
        "permissioned four-unit runtime roster",
    )
    require_pattern(
        core_projection, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, errors,
        (r"let signed = signed_genesis_validator_pops\(genesis\).*?"
         r"signed\.len\(\) == staged_pops\.len\(\).*?"
         r"signed\.len\(\) == trusted\.pops\.len\(\).*?"
         r"trusted\.pops\.get\(signed_id\.public_key\(\)\) == Some\(signed_pop\).*?"
         r"configured_validators != context_validators.*?!exact_pops"),
        "exact signed/staged/configured PoP map",
    )
    require_pattern(
        core_projection, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, errors,
        (r"if validator_id == local_id \{\s*"
         r"config\.network\.public_address\.value\(\)\.clone\(\).*?"
         r"Duration::from_millis\(metadata\.block_cadence_ms\.get\(\)\),\s*context\.mode,.*?"
         r"projection\.validate\(\)\.map_err\(\|error\| error\.to_string\(\)\)\?;\s*"
         r"Ok\(Self \{ projection \}\)"),
        "advertised endpoint, signed cadence, and validated projection",
    )
    require_pattern(
        core_projection, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, errors,
        (r"fn exact_signed_consensus_metadata\(.*?genesis\.0\.external_transactions\(\).*?"
         r"custom\.id\(\) != &consensus_metadata::handshake_meta_id\(\).*?"
         r"metadata\s*\.validate\(\).*?found\.replace\(metadata\)\.is_some\(\).*?"
         r"found\.ok_or_else"),
        "unique validated signed consensus metadata",
    )
    require_pattern(
        validator_check, NODE, errors,
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
    if node.count(builder) != 2 or validator_check.count(builder) != 1:
        errors.append(f"{NODE}: runtime-effective projection must be built once inside full validation")
    production_signer = catalog.split(
        "fn build_and_sign_validator_qualification_seal_v1(", 1
    )[-1].split("\nfn validate_exact_kagemusha_promotion_sources_v1(", 1)[0]
    require(
        production_signer, CATALOG_VALIDATOR_QUALIFICATION_COMPONENT, errors,
        "runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1",
        "runtime_effective_config.projection()",
    )
    if production_signer.count(
        "runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1"
    ) != 2:
        errors.append(f"{CATALOG_VALIDATOR_QUALIFICATION_COMPONENT}: production signers require verified runtime config")
    forbid(
        production_signer, CATALOG_VALIDATOR_QUALIFICATION_COMPONENT, errors,
        "runtime_effective_config: &KagemushaV4RuntimeEffectiveConfigProjectionV1",
    )
    require_pattern(
        node, NODE, errors,
        (r"fn continue_after_full_kagemusha_check<T,\s*V>\(\s*"
         r"full_validation:\s*ReportResult<V,\s*MainError>,\s*"
         r"action:\s*impl FnOnce\(V\)\s*->\s*ReportResult<T,\s*MainError>,\s*"
         r"\)\s*->\s*ReportResult<T,\s*MainError>\s*\{\s*"
         r"action\(full_validation\?\)\s*\}"),
        "validation result gate",
    )
    return errors


def canary_source_errors(
    canary: str,
    liveness: str,
    rollout: str,
    rollout_liveness: str,
    promotion_receipt: str,
    model_isi_offline: str,
    model_isi_mod: str,
    core_canary: str,
    core_isi_mod: str,
    core_state: str,
    core_committed_transaction_context: str,
    core_block: str,
    core_executor: str,
) -> list[str]:
    """Check the post-receipt canary, reservation, and four-validator liveness chain."""
    errors: list[str] = []
    corridor = promotion_receipt.split(
        "pub(super) fn validate_finality_corridor_context(", 1
    )[-1].split("fn enforce_activation_receipt_frame_size", 1)[0]
    require_pattern(
        corridor, MODEL_PROMOTION_RECEIPT_COMPONENT, errors,
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
    require_pattern(
        origin, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
        (r"origin != origin\.to_ascii_lowercase\(\).*?strip_prefix\(\"https://\"\).*?"
         r"matches!\(character, '/' \| '\?' \| '#' \| '@' \| '\[' \| '\]'\).*?"
         r"host\.parse::<std::net::IpAddr>\(\)\.is_ok\(\).*?"
         r"byte\.is_ascii_lowercase\(\).*?port == 0 \|\| port == 443"),
        "canonical lower-case HTTPS DNS canary origin",
    )
    permit = canary.split("impl KagemushaV4TairaCanaryPermitV1", 1)[-1].split(
        "pub struct KagemushaV4TairaCanaryReservationBodyV1", 1
    )[0]
    require_pattern(
        permit, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        reservation, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    forbid(
        reservation, "on-chain canary reservation", errors,
        "pub canary_transaction: SignedTransaction",
        "pub exact_transaction_wire: Vec<u8>",
        "KagemushaV4TairaCanaryAuthorizationV1",
    )
    authorization = canary.split(
        "pub struct KagemushaV4TairaCanaryAuthorizationPackageV1", 1
    )[-1].split("pub struct KagemushaV4VerifiedTairaCanaryAuthorizationV1", 1)[0]
    require_pattern(
        authorization, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        transaction, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        binding, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        query, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        evidence, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
        (r"norito::encode_canonical\(self\).*?!= exact_evidence_bytes.*?"
         r"verify_evidence_signature\(&self\.signature, &self\.body\.issuer, "
         r"self\.body\.signing_hash\(\)\).*?"
         r"verify_evidence_body\(\s*&self\.body,\s*authorization,\s*"
         r"exact_authorization_bytes,\s*expectations,\s*receipt,\s*exact_receipt_bytes,"),
        "exact issuer-signed canary evidence entrypoint",
    )
    require_pattern(
        canary, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
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
    require_pattern(
        evidence_body, MODEL_CANARY_EVIDENCE_COMPONENT, errors,
        (r"decode_exact_finalized_block\(body\.finalized_block_wire\.as_bytes\(\).*?"
         r"authorization\.verify_exact\(.*?block_time_unix_ms.*?"
         r"body\.canary_authorization != authorization_identity.*?"
         r"body\.canary_transaction_intent != verified_authorization\.canary_transaction_intent\(\).*?"
         r"body\.canary_transaction_wire != verified_authorization\.canary_transaction_wire\(\).*?"
         r"body\.finalized_height >= verified_authorization\.expires_at_height\(\)\.get\(\).*?"
         r"committed\.verify_inclusion_in_block\(&block\).*?committed_wire != authorized_wire.*?"
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
    require_pattern(
        isi, MODEL_ISI_OFFLINE, errors,
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
    forbid(
        authorize_field, "AuthorizeKagemushaTairaCanaryV4 payload", errors,
        "SignedTransaction", "KagemushaV4TairaCanaryAuthorizationV1", "Vec<u8>",
    )
    require(
        model_isi_mod, MODEL_ISI_MOD, errors,
        "impl_direct_instruction_box!(crate::isi::offline::RecordKagemushaTairaCanaryV4);",
        "impl_direct_instruction_box!(crate::isi::offline::AuthorizeKagemushaTairaCanaryV4);",
    )
    require(
        core_isi_mod, CORE_ISI_MOD, errors,
        "dispatch_instruction::<iroha_data_model::isi::offline::RecordKagemushaTairaCanaryV4>",
        "dispatch_instruction::<iroha_data_model::isi::offline::AuthorizeKagemushaTairaCanaryV4>",
    )
    wire_boundary = "complete signed canary wire bound at all transaction boundaries"
    require_pattern(
        core_canary, CORE_KAGEMUSHA_CANARY_COMPONENT, errors,
        (r"pub\(crate\) fn signed_kagemusha_taira_canary_wire_identity_v1\(.*?"
         r"transaction: &SignedTransaction,.*?"
         r"Result<Option<KagemushaExactBytesDigestV1>.*?"
         r"Executable::Instructions\(instructions\) = transaction\.instructions\(\).*?"
         r"let \[instruction\] = instructions\.as_ref\(\).*?"
         r"downcast_ref::<RecordKagemushaTairaCanaryV4>\(\).*?"
         r"transaction\.encode_wire_v1\(\).*?KagemushaExactBytesDigestV1::from_bytes\(&wire\)"
         r".*?\.map\(Some\)"),
        wire_boundary,
    )
    require_pattern(
        core_state, CORE_STATE, errors,
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
        wire_boundary,
    )
    require_pattern(
        core_committed_transaction_context,
        CORE_COMMITTED_TX_CONTEXT,
        errors,
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
        wire_boundary,
    )
    require_pattern(
        core_block, CORE_BLOCK, errors,
        (r"fn validate_block_transaction_admission\(.*?"
         r"canary_wire_identity\s*=\s*crate::smartcontracts::isi::offline::"
         r"signed_kagemusha_taira_canary_wire_identity_v1\(tx\).*?"
         r"map_err\(TransactionRejectionReason::Validation\).*?"
         r"state_tx\.kagemusha_taira_canary_external_entrypoint\s*=\s*true;.*?"
         r"state_tx\.kagemusha_taira_canary_wire_identity\s*=\s*canary_wire_identity;.*?"
         r"StateBlock::validate_stateful_admission\(tx, state_tx, Some\(routing\)\)"),
        wire_boundary,
    )
    require_pattern(
        core_executor, CORE_EXECUTOR, errors,
        (r"pub fn execute_transaction\(.*?"
         r"state_transaction\.kagemusha_taira_canary_wire_identity\s*=\s*None;.*?"
         r"transaction\.authority\(\) != authority.*?"
         r"if state_transaction\.kagemusha_taira_canary_external_entrypoint.*?"
         r"state_transaction\.kagemusha_taira_canary_wire_identity\s*=\s*"
         r"signed_kagemusha_taira_canary_wire_identity_v1\(&transaction\)\?;.*?"
         r"state_transaction\.tx_call_hash = Some"),
        wire_boundary,
    )
    require_pattern(
        core_canary, CORE_KAGEMUSHA_CANARY_COMPONENT, errors,
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
    require_pattern(
        core_canary, CORE_KAGEMUSHA_CANARY_COMPONENT, errors,
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
    forbid(
        core_canary.split("impl Execute for AuthorizeKagemushaTairaCanaryV4", 1)[-1],
        "consensus canary authorization", errors,
        "SignedTransaction", "CanActivateKagemushaRecursiveReleaseV4", "self.authorization",
    )

    phase = rollout.split("/// Phase-separated rollout command.", 1)[-1].split(
        "struct TrustedInputs", 1
    )[0]
    require_pattern(
        phase, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require(
        rollout, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        create, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        create.split("publish_root_owned(&self.output", 1)[-1],
        KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"fresh_head\s*\.refresh\(&client.*?require_canary_expiry_margin\(head.*?"
         r"fresh_time = current_unix_ms\(\).*?decode_canonical\(published\).*?"
         r"verify_exact\(.*?fresh_time.*?"
         r"require_canary_authorization_wall_margin\(&verified, fresh_time\)"),
        "fresh post-head authorization publication verification",
    )
    require_pattern(
        create, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"publish_root_owned\(&self\.output, &bytes.*?context\.print_data\(&report\).*?"
         r"PublicationError::CommitUncertain.*?published canary-authorization report failed"),
        "canary authorization no-replace commit-uncertain reporting",
    )
    require_pattern(
        rollout, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"struct SubmitCanaryAuthorization.*?"
         r"#\[arg\(long, required = true, action = clap::ArgAction::SetTrue\)\].*?"
         r"impl SubmitCanaryAuthorization.*?if !self\.write_authorized.*?require_root\(\).*?"
         r"struct SubmitCanary \{.*?"
         r"#\[arg\(long, required = true, action = clap::ArgAction::SetTrue\)\].*?"
         r"impl SubmitCanary \{.*?if !self\.write_authorized.*?require_root\(\)"),
        "explicit write authorization before both canary network phases",
    )
    submit_auth = rollout.split("impl SubmitCanaryAuthorization", 1)[-1].split(
        "struct SubmitCanary", 1
    )[0]
    require_pattern(
        submit_auth, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        reservation_journal, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"transaction\.authority\(\) != &authorization\.artifact\.permit\(\)\.body\.canary_authority.*?"
         r"downcast_ref::<AuthorizeKagemushaTairaCanaryV4>\(\).*?"
         r"reservation\.reservation\(\) != authorization\.artifact\.reservation\(\).*?"
         r"SignedTransaction::decode_all_versioned\(bytes\).*?encode_wire_v1\(\).*?!= bytes"),
        "exact minimal reservation transaction journal",
    )
    submit = rollout.split("impl SubmitCanary {", 1)[-1].split(
        "fn require_canary_wait_outcome", 1
    )[0]
    require_pattern(
        submit, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    forbid(
        submit, "post-authorization canary submit", errors,
        "publish_canary_submission_journal(", "SubmissionJournalAction::Publish",
    )
    helpers = rollout.split("fn load_verified_canary_authorization(", 1)[-1].split(
        "fn verify_submission_journal_bytes(", 1
    )[0]
    require_pattern(
        helpers, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        helpers, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        finalize, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
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
    require_pattern(
        finalize, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"publish_root_owned\(&self\.output, &bytes.*?context\.print_data\(&report\).*?"
         r"PublicationError::CommitUncertain.*?published canary-evidence report failed"),
        "canary evidence no-replace commit-uncertain reporting",
    )
    for section, minimum in ((submit_auth, 3), (submit, 4)):
        if section.count("CanarySubmissionUncertain") < minimum:
            errors.append(
                f"{KAGEMUSHA_ROLLOUT_COMPONENT}: canary network outcomes must remain commit-uncertain"
            )

    challenge = liveness.split(
        "impl KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1", 1
    )[-1].split("pub struct KagemushaV4PostCanaryValidatorLivenessObservationV1", 1)[0]
    require_pattern(
        challenge, MODEL_CANARY_LIVENESS_COMPONENT, errors,
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
    liveness_verify = liveness.split("fn verify_evidence_body_with_trust(", 1)[-1].split(
        "fn validate_liveness_torii_origin(", 1
    )[0]
    require_pattern(
        liveness, MODEL_CANARY_LIVENESS_COMPONENT, errors,
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
    require_pattern(
        liveness_verify, MODEL_CANARY_LIVENESS_COMPONENT, errors,
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
    require_pattern(
        liveness_verify, MODEL_CANARY_LIVENESS_COMPONENT, errors,
        (r"fn verify_challenge_with_trust\(.*?"
         r"challenge\.body\.binding != \*trust\.binding.*?"
         r"challenge\.body\.canary_anchor != \*trust\.canary_anchor.*?"
         r"challenge\.body\.issuer != \*trust\.issuer.*?"
         r"zip\(&trust\.validator_ids\).*?target\.validator_id != expected"),
        "exact activation canary issuer and four-validator challenge binding",
    )
    require_pattern(
        liveness, MODEL_CANARY_LIVENESS_COMPONENT, errors,
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
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
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
    loaded_canary = rollout_liveness.split(
        "fn load_verified_canary_evidence(", 1
    )[-1].split("fn parse_validator_targets(", 1)[0]
    require_pattern(
        loaded_canary, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
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
    liveness_collection = rollout_liveness.split("fn collect_validator_observations(", 1)[-1].split(
        "enum AttestationFetch", 1
    )[0]
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"build_liveness_http_client\(client\.torii_request_timeout\).*?"
         r"collect_validator_observations\(\s*&http,\s*&challenge,"),
        "zero inherited credentials across direct validator origins",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
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
    require_pattern(
        liveness_collection, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
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
    if re.search(r"\bClient\b", liveness_collection) is not None:
        errors.append(
            f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: ambient Client enters direct validator collection"
        )
    forbid(
        liveness_collection, "direct validator collection transport isolation", errors,
        ".get_status(", "base_client", "headers.clear()",
    )
    status_request = rollout_liveness.split("fn build_validator_status_request(", 1)[-1].split(
        "fn fetch_validator_status_height", 1
    )[0]
    require_pattern(
        status_request, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"http: &HttpClient,\s*canonical_torii_origin: &str,\s*status_timeout: Duration.*?"
         r'format!\(\s*"\{\}\{\}/blocks"\s*,\s*canonical_torii_origin\s*,\s*'
         r"iroha_torii_shared::uri::STATUS\s*\).*?"
         r"http\.get\(url\)\s*\.timeout\(status_timeout\)\s*"
         r"\.header\(ACCEPT, APPLICATION_JSON\)\s*"
         r'\.header\(ACCEPT_ENCODING, "identity"\)\s*\.build\(\)'),
        "direct validator status exact URL and two protocol headers",
    )
    if status_request.count(".header(") != 2:
        errors.append(
            f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: direct validator status requires exact two protocol headers"
        )
    forbid(
        status_request, "direct validator status credential isolation", errors,
        ".headers(",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"fn fetch_validator_status_height\(.*?current_unix_ms\(\).*?"
         r"build_validator_status_request\(http, canonical_torii_origin, status_timeout\).*?"
         r"let requested_url = request\.url\(\)\.clone\(\).*?"
         r"http\s*\.execute\(request\).*?response\.url\(\) != &requested_url.*?"
         r"read_status_hint_response\(response\).*?current_unix_ms\(\).*?"
         r"norito::json::from_slice\(&exact_bytes\).*?"
         r"norito::json::to_json\(&height\).*?canonical\.as_bytes\(\) != exact_bytes"),
        "direct validator status bounded exact canonical scalar",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
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
    liveness_fetch = rollout_liveness.split("fn fetch_validator_attestation(", 1)[-1].split(
        "fn build_validator_attestation_request", 1
    )[0]
    require_pattern(
        liveness_fetch, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"build_validator_attestation_request\(http, target, challenge, height\).*?"
         r"let url = request\.url\(\)\.clone\(\).*?http\s*\.execute\(request\).*?"
         r"response\.url\(\) != &url.*?decode_canonical_with_limits\(.*?"
         r"encode_canonical\(&attestation\).*?!= exact_bytes"),
        "direct common-challenge collection with exact canonical attestations",
    )
    attestation_request = rollout_liveness.split(
        "fn build_validator_attestation_request(", 1
    )[-1].split("fn read_attestation_response", 1)[0]
    require_pattern(
        attestation_request, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"BRIDGE_FINALITY_ATTESTATION\s*\.path\(\).*?"
         r"http\.get\(url\)\s*\.header\(ACCEPT, APPLICATION_NORITO\)\s*"
         r'\.header\(ACCEPT_ENCODING, "identity"\)\s*'
         r"\.header\(FINALITY_CHALLENGE_HEADER, hex::encode\(challenge\)\)\s*\.build\(\)"),
        "direct validator attestation exact three protocol headers",
    )
    if attestation_request.count(".header(") != 3:
        errors.append(
            f"{KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT}: direct validator attestation requires exact three protocol headers"
        )
    forbid(
        attestation_request, "direct validator attestation credential isolation", errors,
        ".headers(",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"OsRng\s*\.try_fill_bytes\(&mut nonce\).*?nonce != \[0; 32\].*?"
         r"redirect\(reqwest::redirect::Policy::none\(\)\).*?"
         r"retry\(reqwest::retry::never\(\)\).*?\.no_proxy\(\).*?"
         r"for target in challenge\.body\.targets\.clone\(\).*?"
         r"attestation\.body\.challenge != endpoint_challenge.*?"
         r"attestation\s*\.verify\(\)"),
        "direct common-challenge collection with exact canonical attestations",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"fn read_attestation_response\(.*?content_type != APPLICATION_NORITO.*?"
         r"contains_key\(CONTENT_ENCODING\).*?Cache-Control: no-store.*?"
         r"take\(u64::try_from\(maximum\)\?\.saturating_add\(1\)\)"),
        "bounded identity-encoded no-store attestation response",
    )
    require_pattern(
        rollout_liveness, KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, errors,
        (r"fn collect_shared_finality_chain\(.*?require_qualified_finality_context\(canary_proof.*?"
         r"BridgeFinalityVerifier::with_context\(.*?verifier\s*\.verify\(canary_proof\).*?"
         r"get_next_bridge_finality_proof\(height, &mut verifier\)"),
        "canary-anchored contiguous shared finality collection",
    )
    return errors


def release_closure_source_errors(
    core: str, schema: str, workflow: str, overrides: dict[str, str]
) -> list[str]:
    """Reject release-source gaps that focused filters or compilation would expose."""
    errors: list[str] = []
    isi_tests = (
        overrides[CORE_ISI_TESTS]
        if CORE_ISI_TESTS in overrides
        else read(CORE_ISI_TESTS, errors)
    )
    context_tests = (
        overrides[CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS]
        if CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS in overrides
        else read(CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS, errors)
    )
    forbid_merge_conflict_markers(isi_tests, CORE_ISI_TESTS, errors)
    forbid_merge_conflict_markers(
        context_tests, CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS, errors
    )
    require(
        core,
        CORE,
        errors,
        "pub(crate) use isi::signed_kagemusha_taira_canary_wire_identity_v1;",
    )
    require(
        isi_tests,
        CORE_ISI_TESTS,
        errors,
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE,
    )
    require_pattern(
        context_tests,
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        errors,
        (r"fn taira_canary_committed_replay_seeds_only_one_direct_wire\(\).*?"
         r"TransactionEntrypoint::External\(first\.canary_transaction\.clone\(\)\).*?"
         r"Some\(first_wire\).*?"
         r"TransactionEntrypoint::External\(second\.canary_transaction\.clone\(\)\).*?"
         r"Some\(second_wire\).*?"
         r"TransactionEntrypoint::External\(multi\).*?"
         r"kagemusha_taira_canary_wire_identity, None.*?"
         r"TransactionEntrypoint::External\(batch\).*?"
         r"kagemusha_taira_canary_wire_identity, None.*?"
         r"TransactionEntrypoint::SealedReveal\(.*?"
         r"kagemusha_taira_canary_external_entrypoint\).*?"
         r"kagemusha_taira_canary_wire_identity, None"),
        "External-only committed-replay exact-wire seeding boundaries",
    )
    require_pattern(
        context_tests,
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        errors,
        (r"fn taira_canary_sealed_reveal_validation_cannot_gain_external_provenance\(\).*?"
         r"TransactionEntrypoint::SealedCommitment\(.*?"
         r"TransactionEntrypoint::SealedReveal\(.*?"
         r"validate_transaction\(.*?expect_err\(.*?"
         r"canary_external_entrypoint_required.*?"
         r"replay_keys_before"),
        "sealed-reveal validation rejects External canary provenance",
    )
    require_pattern(
        context_tests,
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        errors,
        (r"fn taira_canary_executor_enforces_exact_wire_shape_and_proof\(\).*?"
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
    require_pattern(
        context_tests,
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        errors,
        (r"fn taira_canary_nested_trigger_cannot_inherit_outer_wire\(\).*?"
         r"ExecuteTriggerEventFilter::new\(\)\.for_trigger\(trigger_id\.clone\(\)\).*?"
         r"RecordKagemushaTairaCanaryV4::new\(outer\.permit\).*?"
         r"kagemusha_taira_canary_wire_identity, None.*?"
         r"ExecuteTrigger::new\(trigger_id\).*?expect_err\(.*?"
         r"canary_authorization_missing"),
        "nested-trigger affine signed-wire rejection",
    )
    if '"pending-' in schema:
        errors.append(f"{SCHEMA_GOLDEN}: public schema golden contains pending placeholder")
    require(workflow, WORKFLOW, errors, *KAGEMUSHA_RELEASE_RUST_TEST_FILTERS)
    return errors


def source_provider_pipeline_errors(readiness: str) -> list[str]:
    """Validate authentication and byte-only dispatch for every source provider."""
    errors: list[str] = []
    require_pattern(
        readiness,
        READINESS,
        errors,
        (
            r"READINESS_SOURCE_PROVIDERS = \(\s*READINESS_SOURCE_SUPPORT,\s*"
            r"READINESS_RECURSION_SOURCE_CONTRACT,\s*READINESS_SOURCE_CONTRACT,\s*\)"
        ),
        "exact authenticated source-provider set",
    )
    try:
        embedded_source = readiness.split("<<'PY'\n", 1)[1].rsplit("\nPY\n", 1)[0]
        readiness_tree = ast.parse(embedded_source)
    except (IndexError, SyntaxError) as error:
        errors.append(f"{READINESS}: embedded Python is not statically parseable: {error}")
        readiness_tree = None
    if readiness_tree is not None:
        provider_stores = [
            node
            for node in ast.walk(readiness_tree)
            if isinstance(node, ast.Name)
            and isinstance(node.ctx, ast.Store)
            and node.id == "READINESS_SOURCE_PROVIDERS"
        ]
        provider_assignments = [
            node
            for node in ast.walk(readiness_tree)
            if isinstance(node, ast.Assign)
            and len(node.targets) == 1
            and isinstance(node.targets[0], ast.Name)
            and node.targets[0].id == "READINESS_SOURCE_PROVIDERS"
        ]
        expected_names = (
            "READINESS_SOURCE_SUPPORT",
            "READINESS_RECURSION_SOURCE_CONTRACT",
            "READINESS_SOURCE_CONTRACT",
        )
        exact_provider_tuple = (
            len(provider_assignments) == 1
            and isinstance(provider_assignments[0].value, ast.Tuple)
            and tuple(
                element.id if isinstance(element, ast.Name) else None
                for element in provider_assignments[0].value.elts
            )
            == expected_names
        )
        if len(provider_stores) != 1 or not exact_provider_tuple:
            errors.append(
                f"{READINESS}: expected exactly one immutable authenticated "
                "source-provider tuple"
            )
    promotion = readiness.split("def promotion_errors() -> list[str]:", 1)[-1].split(
        "\nsource_contract_errors: list[str] = []\n", 1
    )[0]
    require_pattern(
        readiness,
        READINESS,
        errors,
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
    require_pattern(
        promotion,
        READINESS,
        errors,
        (
            r"pin_authenticated_reviewed_source_file\(\s*SOURCE_TREE_SEAL,\s*"
            r"reviewed_source_commit,\s*MAX_REVIEWED_HELPER_BYTES,.*?"
            r"snapshot_private_bytes\(\s*source_helper_bytes,.*?"
            r"str\(trusted_source_helper_snapshot\)"
        ),
        "source-closure-authenticated source-tree helper snapshot",
    )
    require_pattern(
        promotion,
        READINESS,
        errors,
        (
            r"for relative in READINESS_SOURCE_PROVIDERS:\s*path = root / relative\s*"
            r"authenticated_readiness_source_contract_bytes\[relative\] = \(\s*"
            r"pin_authenticated_reviewed_source_file\(\s*relative,\s*"
            r"reviewed_source_commit,\s*MAX_READINESS_SOURCE_CONTRACT_BYTES,"
        ),
        "root-custodied source-closure-authenticated source-provider set",
    )
    require_pattern(
        promotion,
        READINESS,
        errors,
        (
            r"if self_test:\s*authenticated_readiness_self_test_bytes = "
            r"pin_authenticated_reviewed_source_file\(\s*READINESS_SELF_TEST,\s*"
            r"reviewed_source_commit,\s*MAX_READINESS_SELF_TEST_BYTES,"
        ),
        "root-custodied source-closure-authenticated readiness self-test bytes",
    )
    dispatch = readiness.rsplit("\nsource_contract_errors: list[str] = []\n", 1)[-1]
    source_dispatch, dispatch_separator, self_test_dispatch = dispatch.partition(
        "\nerrors = source_contract_errors\n"
    )
    if not dispatch_separator:
        errors.append(f"{READINESS}: source-provider dispatch boundary is missing")
    self_test_dispatch = self_test_dispatch.split("\nPY\n", 1)[0]
    require_pattern(
        source_dispatch,
        READINESS,
        errors,
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
    require_pattern(
        source_dispatch,
        READINESS,
        errors,
        (
            r"support_bytes = source_contract_bytes\.get\(READINESS_SOURCE_SUPPORT\).*?"
            r"_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1.*?"
            r"compile\(support_bytes, READINESS_SOURCE_SUPPORT, \"exec\"\).*?"
            r"recursion_bytes = source_contract_bytes\.get\("
            r"READINESS_RECURSION_SOURCE_CONTRACT\).*?"
            r"_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_SOURCE_V1.*?"
            r"compile\(recursion_bytes, READINESS_RECURSION_SOURCE_CONTRACT, \"exec\"\).*?"
            r"recursion_context\.get\(\"recursion_source_contract_errors\"\).*?"
            r"primary_bytes = source_contract_bytes\.get\(READINESS_SOURCE_CONTRACT\).*?"
            r"_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_EVALUATOR_V1.*?"
            r"compile\(\s*primary_bytes,\s*READINESS_SOURCE_CONTRACT,\s*\"exec\",?\s*\).*?"
            r"source_contract_context\.get\(\"static_errors\"\).*?"
            r"callable\(source_contract_evaluator\).*?"
            r"source_contract_errors\.extend\(source_contract_evaluator\(\)\)"
        ),
        "authenticated byte-only support, recursion, and readiness source-contract dispatch",
    )
    require(
        source_dispatch,
        READINESS,
        errors,
        "if support_bytes is None:",
        '"readiness source-support provider bytes are unavailable"',
        'source_provider_base_names = frozenset(globals()) | {',
        'support_context = dict(globals())',
        'support_context.get("source_provider_pipeline_errors")',
        '"readiness source-support provider evaluator is unavailable"',
        "if recursion_bytes is None:",
        '"recursion source-contract provider bytes are unavailable"',
        '"recursion source-contract provider evaluator is unavailable"',
        'source_contract_context = dict(support_context)',
        "if primary_bytes is None:",
        '"readiness source-contract provider bytes are unavailable"',
        '"readiness source-contract provider evaluator is unavailable"',
    )
    require_pattern(
        self_test_dispatch,
        READINESS,
        errors,
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
        source_dispatch_tree = ast.parse(source_dispatch)
        self_test_dispatch_tree = ast.parse(self_test_dispatch)
    except SyntaxError as error:
        errors.append(f"{READINESS}: provider dispatch is not statically parseable: {error}")
        source_dispatch_tree = None
        self_test_dispatch_tree = None
    if source_dispatch_tree is not None:
        for target, provider in (
            ("support_bytes", "READINESS_SOURCE_SUPPORT"),
            ("recursion_bytes", "READINESS_RECURSION_SOURCE_CONTRACT"),
            ("primary_bytes", "READINESS_SOURCE_CONTRACT"),
        ):
            stores = [
                node
                for node in ast.walk(source_dispatch_tree)
                if isinstance(node, ast.Name)
                and isinstance(node.ctx, ast.Store)
                and node.id == target
            ]
            assignments = [
                node
                for node in ast.walk(source_dispatch_tree)
                if isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id == target
            ]
            exact_lookup = False
            if len(assignments) == 1:
                value = assignments[0].value
                exact_lookup = (
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
            if len(stores) != 1 or not exact_lookup:
                errors.append(
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
                for node in ast.walk(source_dispatch_tree)
                if isinstance(node, ast.Name)
                and isinstance(node.ctx, ast.Store)
                and node.id == target
            ]
            assignments = [
                node
                for node in ast.walk(source_dispatch_tree)
                if exact_context_copy(node, target, source)
            ]
            if len(stores) != 1 or len(assignments) != 1:
                errors.append(
                    f"{READINESS}: {target} must have exactly one isolated namespace copy"
                )

        base_name_stores = [
            node
            for node in ast.walk(source_dispatch_tree)
            if isinstance(node, ast.Name)
            and isinstance(node.ctx, ast.Store)
            and node.id == "source_provider_base_names"
        ]
        base_name_assignments = [
            node
            for node in ast.walk(source_dispatch_tree)
            if isinstance(node, ast.Assign)
            and len(node.targets) == 1
            and isinstance(node.targets[0], ast.Name)
            and node.targets[0].id == "source_provider_base_names"
        ]
        exact_base_names = False
        if len(base_name_assignments) == 1:
            value = base_name_assignments[0].value
            exact_base_names = (
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
        if len(base_name_stores) != 1 or not exact_base_names:
            errors.append(
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

        generic_reads = 0
        forbidden_reads: set[str] = set()
        for node in ast.walk(source_dispatch_tree):
            if not isinstance(node, ast.Call):
                continue
            if generic_candidate_read(node):
                generic_reads += 1
                continue
            if isinstance(node.func, ast.Name) and node.func.id in {"open", "read"}:
                forbidden_reads.add(node.func.id)
            elif isinstance(node.func, ast.Attribute) and node.func.attr in {
                "open",
                "read",
                "read_bytes",
                "read_text",
            }:
                forbidden_reads.add(node.func.attr)
        if generic_reads != 1:
            errors.append(
                f"{READINESS}: source providers must have exactly one generic "
                "candidate-only filesystem read"
            )
        for call in sorted(forbidden_reads):
            errors.append(
                f"{READINESS}: source-provider dispatch has forbidden filesystem "
                f"call {call}"
            )
    if self_test_dispatch_tree is not None:
        self_test_context_stores = [
            node
            for node in ast.walk(self_test_dispatch_tree)
            if isinstance(node, ast.Name)
            and isinstance(node.ctx, ast.Store)
            and node.id == "self_test_context"
        ]
        self_test_context_bindings = [
            node
            for node in ast.walk(self_test_dispatch_tree)
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
            errors.append(
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
            condition = node.body[0]
            if not (
                isinstance(condition.test, ast.Compare)
                and isinstance(condition.test.left, ast.Name)
                and condition.test.left.id == "provider_name"
                and len(condition.test.ops) == 1
                and isinstance(condition.test.ops[0], ast.NotIn)
                and len(condition.test.comparators) == 1
                and isinstance(condition.test.comparators[0], ast.Name)
                and condition.test.comparators[0].id == "source_provider_base_names"
                and len(condition.body) == 1
                and isinstance(condition.body[0], ast.Assign)
                and not condition.orelse
            ):
                return False
            assignment = condition.body[0]
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
            for node in ast.walk(self_test_dispatch_tree)
            if exact_provider_export(node)
        ]
        global_subscript_stores = [
            node
            for node in ast.walk(self_test_dispatch_tree)
            if isinstance(node, ast.Subscript)
            and isinstance(node.ctx, ast.Store)
            and isinstance(node.value, ast.Call)
            and isinstance(node.value.func, ast.Name)
            and node.value.func.id == "globals"
        ]
        if len(provider_exports) != 1 or len(global_subscript_stores) != 1:
            errors.append(
                f"{READINESS}: self-test may export only new authenticated provider symbols"
            )

        error_bindings = [
            node
            for node in ast.walk(self_test_dispatch_tree)
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
            errors.append(
                f"{READINESS}: self-test errors must be rebound exactly once"
            )
    promotion_dispatch = source_dispatch.split("\nelse:\n", 1)[0]
    forbid(
        promotion_dispatch,
        "promotion source-contract provider dispatch",
        errors,
        "read_bytes",
        "compile(",
        "exec(",
        "runpy.run_path",
        "import_module",
    )
    forbid(
        source_dispatch,
        "readiness source-contract provider dispatch",
        errors,
        "(root / READINESS_SOURCE_SUPPORT).read_bytes()",
        "(root / READINESS_RECURSION_SOURCE_CONTRACT).read_bytes()",
        "(root / READINESS_SOURCE_CONTRACT).read_bytes()",
        "runpy.run_path",
        "import_module",
    )
    return errors
