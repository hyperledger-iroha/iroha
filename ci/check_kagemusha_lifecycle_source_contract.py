import re
from pathlib import Path
PROVIDER="ci/check_kagemusha_lifecycle_source_contract.py"
MODEL="crates/iroha_data_model/src/offline/mod.rs"
MODEL_KAGEMUSHA_MODEL="crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_KAGEMUSHA_MODEL_INCLUDE='include!("kagemusha_model.rs");'
MODEL_RELEASE_V4="crates/iroha_data_model/src/offline/kagemusha_release_v4.rs"
MODEL_RELEASE_V4_INCLUDE='include!("kagemusha_release_v4.rs");'
MODEL_LIFECYCLE="crates/iroha_data_model/src/offline/kagemusha_release_lifecycle.rs"
MODEL_LIFECYCLE_MODULE="mod kagemusha_release_lifecycle;"
MODEL_TAIL_TESTS="crates/iroha_data_model/src/offline/kagemusha_v4_release_tail_inline_tests.rs"
MODEL_TAIL_TESTS_INCLUDE='include!("kagemusha_v4_release_tail_inline_tests.rs");'
MODEL_PROMOTION_RECEIPT_TESTS=(
 "crates/iroha_data_model/src/offline/kagemusha_promotion_receipt_inline_tests.rs"
)
MODEL_PROMOTION_RECEIPT_TESTS_INCLUDE='include!("kagemusha_promotion_receipt_inline_tests.rs");'
MODEL_PROMOTION_RECEIPT="crates/iroha_data_model/src/offline/kagemusha_promotion_receipt.rs"
MODEL_ISI="crates/iroha_data_model/src/isi/offline.rs"
MODEL_ISI_REGISTRY="crates/iroha_data_model/src/isi/mod.rs"
CORE="crates/iroha_core/src/smartcontracts/isi/offline.rs"
CORE_ACTIVATION="crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_activation.rs"
CORE_LIFECYCLE="crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_release_lifecycle.rs"
CORE_LIFECYCLE_MODULE="mod kagemusha_release_lifecycle;"
CORE_RUNTIME_CONFIG=(
 "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_runtime_effective_config.rs"
)
CORE_REDEMPTION_POLICY=(
 "crates/iroha_core/src/smartcontracts/isi/offline/"
 "kagemusha_redemption_policy_validation.rs"
)
CORE_REDEMPTION_POLICY_INCLUDE=(
 'include!("offline/kagemusha_redemption_policy_validation.rs");'
)
CORE_ISI_TESTS="crates/iroha_core/src/smartcontracts/isi/offline/isi_tests.rs"
CORE_REDEMPTION_POLICY_TESTS=(
 "crates/iroha_core/src/smartcontracts/isi/offline/"
 "isi_kagemusha_redemption_policy_tests.rs"
)
CORE_REDEMPTION_POLICY_TESTS_INCLUDE=(
 'include!("isi_kagemusha_redemption_policy_tests.rs");'
)
CORE_ISI_REGISTRY="crates/iroha_core/src/smartcontracts/isi/mod.rs"
CORE_TX="crates/iroha_core/src/tx.rs"
CORE_TX_AUTHORITY_ADMISSION="crates/iroha_core/src/tx/authority_admission.rs"
CORE_TX_AUTHORITY_ADMISSION_MODULE="mod authority_admission;"
CORE_TX_LIFECYCLE_TESTS="crates/iroha_core/src/tx/kagemusha_lifecycle_admission_tests.rs"
CORE_TX_LIFECYCLE_TESTS_INCLUDE='include!("tx/kagemusha_lifecycle_admission_tests.rs");'
CORE_STATE="crates/iroha_core/src/state.rs"
CORE_STATE_RUNTIME_CONFIG="crates/iroha_core/src/state/runtime_configuration.rs"
CORE_STATE_RUNTIME_CONFIG_INCLUDE='include!("state/runtime_configuration.rs");'
CORE_STATE_TESTS="crates/iroha_core/src/state/tests.rs"
CORE_STATE_RUNTIME_CONFIG_TESTS=(
 "crates/iroha_core/src/state/tests/kagemusha_runtime_effective_config_tests.rs"
)
CORE_STATE_RUNTIME_CONFIG_TESTS_INCLUDE=(
 'include!("tests/kagemusha_runtime_effective_config_tests.rs");'
)
CORE_COMMITTED_CONTEXT="crates/iroha_core/src/state/committed_transaction_context.rs"
CORE_BLOCK="crates/iroha_core/src/block.rs"
CORE_EXECUTOR="crates/iroha_core/src/executor.rs"
CORE_WORLD="crates/iroha_core/src/smartcontracts/isi/world.rs"
CORE_IVM_HOST="crates/iroha_core/src/smartcontracts/ivm/host.rs"
CORE_PROXY="crates/iroha_core/src/torii_proxy.rs"
TORII="crates/iroha_torii/src/lib.rs"
TORII_ROUTING="crates/iroha_torii/src/routing.rs"
TORII_TESTS="crates/iroha_torii/src/tests/lib_runtime_handlers/part_3.rs"
TORII_PENDING_TESTS="crates/iroha_torii/src/tests/lib_runtime_handlers/part_8.rs"
TORII_CATALOG="crates/iroha_torii_shared/src/route_catalog.rs"
TORII_CATALOG_TESTS="crates/iroha_torii_shared/src/route_catalog/tests.rs"
CORE_SUMERAGI_APPLY="crates/iroha_core/src/sumeragi/v2_apply.rs"
CORE_SUMERAGI_APPLY_TESTS="crates/iroha_core/src/sumeragi/v2_apply_tests.rs"
CORE_SUMERAGI_APPLY_TESTS_MODULE='#[path = "v2_apply_tests.rs"]\nmod tests;'
CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS=(
 "crates/iroha_core/src/sumeragi/tests/v2_apply_kagemusha_runtime_gate.rs"
)
CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS_INCLUDE=(
 'include!("tests/v2_apply_kagemusha_runtime_gate.rs");'
)
CORE_SUMERAGI_WORKER="crates/iroha_core/src/sumeragi/v2_worker_completion.rs"
CORE_SUMERAGI_WORKER_ROOT="crates/iroha_core/src/sumeragi/v2_worker.rs"
CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS=(
 "crates/iroha_core/src/sumeragi/tests/v2_worker_kagemusha_runtime_gate.rs"
)
CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS_INCLUDE=(
 'include!("tests/v2_worker_kagemusha_runtime_gate.rs");'
)
CORE_SUMERAGI_RUNNER="crates/iroha_core/src/sumeragi/v2_runner.rs"
CORE_SUMERAGI_PENDING_KURA=(
 "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
)
IROHAD="crates/irohad/src/main.rs"
IROHAD_STARTUP="crates/irohad/src/main/kagemusha_startup.rs"
IROHAD_STARTUP_MODULE='#[path = "main/kagemusha_startup.rs"]\nmod kagemusha_startup;'
IROHAD_STARTUP_TESTS=(
 "crates/irohad/src/main/kagemusha_runtime_effective_config_projection_tests.rs"
)
IROHAD_STARTUP_TESTS_INCLUDE=(
 'include!("main/kagemusha_runtime_effective_config_projection_tests.rs");'
)
IROHAD_VALIDATOR_SEAL_READER=(
 "crates/irohad/src/main/kagemusha_validator_qualification_command.rs"
)
LIFECYCLE_SOURCE_PATHS=(
 MODEL_KAGEMUSHA_MODEL,
 MODEL_RELEASE_V4,
 MODEL_LIFECYCLE,
 MODEL_TAIL_TESTS,
 MODEL_PROMOTION_RECEIPT_TESTS,
 MODEL_PROMOTION_RECEIPT,
 MODEL_ISI,
 MODEL_ISI_REGISTRY,
 CORE_ACTIVATION,
 CORE_LIFECYCLE,
 CORE_RUNTIME_CONFIG,
 CORE_REDEMPTION_POLICY,
 CORE_REDEMPTION_POLICY_TESTS,
 CORE_ISI_REGISTRY,
 CORE_TX,
 CORE_TX_AUTHORITY_ADMISSION,
 CORE_TX_LIFECYCLE_TESTS,
 CORE_STATE,
 CORE_STATE_RUNTIME_CONFIG,
 CORE_STATE_TESTS,
 CORE_STATE_RUNTIME_CONFIG_TESTS,
 CORE_COMMITTED_CONTEXT,
 CORE_BLOCK,
 CORE_EXECUTOR,
 CORE_WORLD,
 CORE_IVM_HOST,
 CORE_PROXY,TORII,TORII_ROUTING,TORII_TESTS,TORII_PENDING_TESTS,TORII_CATALOG,TORII_CATALOG_TESTS,
 CORE_SUMERAGI_APPLY,
 CORE_SUMERAGI_APPLY_TESTS,
 CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS,
 CORE_SUMERAGI_WORKER,
 CORE_SUMERAGI_WORKER_ROOT,
 CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS,
 CORE_SUMERAGI_RUNNER,
 CORE_SUMERAGI_PENDING_KURA,
 IROHAD,
 IROHAD_STARTUP,
 IROHAD_STARTUP_TESTS,
 IROHAD_VALIDATOR_SEAL_READER,
)
EQ="&lifecycle.step_eq_verifier_key_id";EP="&lifecycle.step_ep_verifier_key_id";DX="fn direct_ordinary_multisig_deactivate_executes_exact_enabled_transition(";CX="impl Execute for CancelKagemushaRecursiveReleaseV4";RK=".kagemusha_replay_keys";VK=".verifying_keys_by_circuit";LB="Some(&lifecycle_before)";CT="fn direct_ordinary_multisig_cancel_executes_exact_staged_transition(";RC=".require_committed_kagemusha_runtime_effective_config(";WS="transaction.world.smart_contract_state.get(&lifecycle_key)"
if globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_CONTEXT_V1") is not True:
 raise RuntimeError("lifecycle source-contract provider must run inside the authenticated gate")
_SOURCE = globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_SOURCE_V1")
if not isinstance(_SOURCE, str) or not _SOURCE:
 raise RuntimeError("lifecycle source-contract provider requires its exact loaded bytes")
def _read(
 root,relative,overrides,errors
):
 if relative in overrides:
  return overrides[relative]
 try:
  return (root / relative).read_text(encoding="utf-8")
 except (OSError, UnicodeError) as error:
  errors.append(f"{relative}: could not read lifecycle source input: {error}")
  return ""
def _require(
 text,relative,errors,label,*needles
):
 for needle in needles:
  if needle not in text:
   errors.append(f"{relative}: missing {label}: {needle!r}")
def _forbid(
 text,relative,errors,label,*needles
):
 for needle in needles:
  if needle in text:
   errors.append(f"{relative}: forbidden {label}: {needle!r}")
def _count(
 text,relative,errors,needle,expected,label,
):
 observed = text.count(needle)
 if observed != expected:
  errors.append(
   f"{relative}: {label} count for {needle!r} is {observed}, expected {expected}"
  )
def _section(
 text,relative,errors,start,end=None,
):
 offset = text.find(start)
 if offset < 0:
  errors.append(f"{relative}: missing lifecycle boundary {start!r}")
  return ""
 result = text[offset:]
 if end is not None:
  finish = result.find(end, len(start))
  if finish < 0:
   errors.append(f"{relative}: missing lifecycle boundary {end!r}")
   return ""
  result = result[:finish]
 return result
def _ordered(
 text,relative,errors,label,*needles
):
 offsets=[]
 cursor = 0
 for needle in needles:
  offset = text.find(needle, cursor)
  if offset < 0:
   errors.append(f"{relative}: missing ordered {label}: {needle!r}")
   return
  offsets.append(offset)
  cursor = offset + len(needle)
 if offsets != sorted(offsets):
  errors.append(f"{relative}: {label} is not ordered")
q,f,n,s,o=_require,_forbid,_count,_section,_ordered
def _topology(texts,errors):
 for parent, relative, marker, component in (
  (texts[MODEL], MODEL, MODEL_KAGEMUSHA_MODEL_INCLUDE, MODEL_KAGEMUSHA_MODEL),
  (texts[MODEL], MODEL, MODEL_RELEASE_V4_INCLUDE, MODEL_RELEASE_V4),
  (texts[MODEL], MODEL, MODEL_LIFECYCLE_MODULE, MODEL_LIFECYCLE),
  (texts[MODEL], MODEL, MODEL_TAIL_TESTS_INCLUDE, MODEL_TAIL_TESTS),
  (
   texts[MODEL_TAIL_TESTS],
   MODEL_TAIL_TESTS,
   MODEL_PROMOTION_RECEIPT_TESTS_INCLUDE,
   MODEL_PROMOTION_RECEIPT_TESTS,
  ),
  (texts[CORE], CORE, CORE_LIFECYCLE_MODULE, CORE_LIFECYCLE),
  (texts[CORE], CORE, CORE_REDEMPTION_POLICY_INCLUDE, CORE_REDEMPTION_POLICY),
  (
   texts[CORE_ISI_TESTS],
   CORE_ISI_TESTS,
   CORE_REDEMPTION_POLICY_TESTS_INCLUDE,
   CORE_REDEMPTION_POLICY_TESTS,
  ),
  (
   texts[CORE_TX],
   CORE_TX,
   CORE_TX_AUTHORITY_ADMISSION_MODULE,
   CORE_TX_AUTHORITY_ADMISSION,
  ),
  (
   texts[CORE_TX],
   CORE_TX,
   CORE_TX_LIFECYCLE_TESTS_INCLUDE,
   CORE_TX_LIFECYCLE_TESTS,
  ),
  (
   texts[CORE_STATE],
   CORE_STATE,
   CORE_STATE_RUNTIME_CONFIG_INCLUDE,
   CORE_STATE_RUNTIME_CONFIG,
  ),
  (
   texts[CORE_STATE_TESTS],
   CORE_STATE_TESTS,
   CORE_STATE_RUNTIME_CONFIG_TESTS_INCLUDE,
   CORE_STATE_RUNTIME_CONFIG_TESTS,
  ),
  (texts[IROHAD], IROHAD, IROHAD_STARTUP_MODULE, IROHAD_STARTUP),
  (
   texts[IROHAD],
   IROHAD,
   IROHAD_STARTUP_TESTS_INCLUDE,
   IROHAD_STARTUP_TESTS,
  ),
  (
   texts[CORE_SUMERAGI_APPLY],
   CORE_SUMERAGI_APPLY,
   CORE_SUMERAGI_APPLY_TESTS_MODULE,
   CORE_SUMERAGI_APPLY_TESTS,
  ),
  (
   texts[CORE_SUMERAGI_APPLY_TESTS],
   CORE_SUMERAGI_APPLY_TESTS,
   CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS_INCLUDE,
   CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS,
  ),
  (
   texts[CORE_SUMERAGI_WORKER_ROOT],
   CORE_SUMERAGI_WORKER_ROOT,
   CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS_INCLUDE,
   CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS,
  ),
 ):
  n(
   parent,
   relative,
   errors,
   marker,
   1,
   f"exactly one authenticated {Path(component).name} attachment",
  )
 q(
  texts[MODEL],
  MODEL,
  errors,
  "public lifecycle model export",
  "kagemusha_release_lifecycle::*",
 )
def _model_contracts(texts,errors):
 lifecycle = texts[MODEL_LIFECYCLE]
 q(
  lifecycle,
  MODEL_LIFECYCLE,
  errors,
  "manifest-scoped bounded lifecycle model",
  '"kagemusha_release_lifecycle_v4_"',
  "pub fn kagemusha_v4_release_lifecycle_state_key(",
  'require_nonzero(manifest_sha256, "lifecycle.manifest_sha256")?',
  "OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1 + 256 * 1024",
  "pub expected_predecessor_lifecycle: KagemushaExactBytesDigestV1",
  "pub transition_id: [u8; 32]",
  "pub device_attestation_policy: OfflineDeviceAttestationPolicy",
  "pub enum KagemushaV4ReleaseLifecyclePhaseV1",
  "Staged",
  "Enabled(Box<KagemushaV4ReleaseEnabledV1>)",
  "Cancelled(Box<KagemushaV4ReleaseCancelledV1>)",
  "Deactivated(Box<KagemushaV4ReleaseDeactivatedV1>)",
  "matches!(&self.phase, KagemushaV4ReleaseLifecyclePhaseV1::Enabled(_))",
  "let staged_predecessor_id = staged_predecessor.canonical_digest_unchecked()?",
  "deactivated.deactivation.expected_predecessor_lifecycle",
  "enabled_predecessor.canonical_digest_unchecked()?",
  ".device_attestation_policy_norito",
  ".matches_bytes(&device_attestation_policy_norito)",
  "governance_authority.controller().multisig_policy()",
  "kagemusha_v4_governance_policy_requires_distinct_signers(governance_policy)",
 )
 q(
  texts[MODEL_TAIL_TESTS],
  MODEL_TAIL_TESTS,
  errors,
  "active lifecycle transition and retained-policy regressions",
  "fn release_lifecycle_state_enforces_exact_predecessors_and_terminal_phases()",
  "the retained redemption policy must match its signed promotion identity",
  "deactivation must retain the exact policy required for full redemption",
  "deactivation cannot name the staged state instead of the exact enabled predecessor",
 )
 q(
  texts[MODEL_PROMOTION_RECEIPT_TESTS],
  MODEL_PROMOTION_RECEIPT_TESTS,
  errors,
  "promotion identity and exact reservation-generation regressions",
  "fn github_promotion_id_derivation_matches_known_vector()",
  "fn validator_seals_reject_mixed_exact_reservation_generations()",
 )
 q(
  lifecycle,
  MODEL_LIFECYCLE,
  errors,
  "active bounded lifecycle decoder regressions",
  "fn lifecycle_state_key_is_exact_and_rejects_zero_manifest_digest()",
  "fn terminal_transitions_are_bounded_canonical_and_reason_closed()",
  "fn bounded_decoders_reject_empty_and_trailing_input()",
 )
 f(
  lifecycle + texts[MODEL_TAIL_TESTS] + texts[MODEL_PROMOTION_RECEIPT_TESTS],
  "lifecycle model regressions",
  errors,
  "disabled lifecycle regression",
  "#[ignore]",
  "#[cfg(any())]",
 )
def _internal_validation_trust_contracts(texts,errors):
 model = s(
  texts[MODEL_KAGEMUSHA_MODEL],
  MODEL_KAGEMUSHA_MODEL,
  errors,
  "pub struct KagemushaRecursiveSpendReleasePolicyV1 {",
  "/// Verified signer identity retained",
 )
 q(
  model,
  MODEL_KAGEMUSHA_MODEL,
  errors,
  "policy-owned internal-validation runner trust root",
  "pub internal_validation_runner_identity_sha256: [u8; 32]",
  "Domain-separated identity of the only internal-validation runner authorized by policy",
 )
 release = texts[MODEL_RELEASE_V4]
 policy_validation = s(
  release,
  MODEL_RELEASE_V4,
  errors,
  "impl KagemushaRecursiveSpendReleasePolicyV1 {",
  "impl KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {",
 )
 q(
  policy_validation,
  MODEL_RELEASE_V4,
  errors,
  "nonzero internal-validation runner trust root",
  "self.internal_validation_runner_identity_sha256 == [0; 32]",
  "KagemushaReleaseVerificationError::InvalidPolicy",
 )
 receipt_validation = s(
  release,
  MODEL_RELEASE_V4,
  errors,
  "fn validate_internal_validation_receipt_v4(",
  "impl KagemushaAuthenticatedReleaseV4 {",
 )
 o(
  receipt_validation,
  MODEL_RELEASE_V4,
  errors,
  "receipt runner identity against the policy trust root",
  "expected_runner_identity_sha256: Option<[u8; 32]>",
  "let body = &receipt.body",
  "expected_runner_identity_sha256",
  ".is_some_and(|expected| body.validation_runner_identity_sha256 != expected)",
  "KagemushaReleaseVerificationError::InvalidInternalValidationReceipt",
 )
 authenticated_v4 = s(
  release,
  MODEL_RELEASE_V4,
  errors,
  "impl KagemushaAuthenticatedReleaseV4 {",
  "impl KagemushaRecursiveSpendPromotedReleaseV4 {",
 )
 o(
  authenticated_v4,
  MODEL_RELEASE_V4,
  errors,
  "authenticated V4 internal-validation trust-root forwarding",
  "validate_internal_validation_receipt_v4(",
  "internal_validation_receipt",
  "Some(policy.internal_validation_runner_identity_sha256)",
 )
 q(
  texts[MODEL],
  MODEL,
  errors,
  "hostile runner-root release regressions",
  "wrong_runner_policy.internal_validation_runner_identity_sha256[0] ^= 1",
  "a valid self-declared runner signature is not an authorization root",
  "unpinned_runner_policy.internal_validation_runner_identity_sha256 = [0; 32]",
  "an authenticated V4 release policy must name a runner trust root",
 )
def _native_namespace_contracts(texts,errors):
 host = texts[CORE_IVM_HOST]
 read_only = s(
  host,
  CORE_IVM_HOST,
  errors,
  "const READ_ONLY_SYSTEM_CONTRACT_STATE_PREFIXES: &[&str] = &[",
  "];",
 )
 n(
  read_only,
  CORE_IVM_HOST,
  errors,
  '"kagemusha",',
  1,
  "exact delimiter-aware native Kagemusha namespace root",
 )
 f(
  read_only,
  CORE_IVM_HOST,
  errors,
  "narrow or delimiter-bearing Kagemusha namespace root",
  '"kagemusha_",',
  '"kagemusha_online_registration_",',
 )
 classifier = s(
  host,
  CORE_IVM_HOST,
  errors,
  "fn contract_state_key_matches_namespace(",
  "fn scoped_durable_state_path(",
 )
 q(
  classifier,
  CORE_IVM_HOST,
  errors,
  "delimiter-aware read-only native namespace ownership",
  "key == prefix",
  ".strip_prefix(prefix)",
  "suffix.starts_with('_') || suffix.starts_with('/')",
  "READ_ONLY_SYSTEM_CONTRACT_STATE_PREFIXES",
  "ContractStateNamespaceAccess::ReadOnlySystem",
  "fn ensure_contract_state_write_allowed(",
  "!= ContractStateNamespaceAccess::User",
  "ivm::VMError::PermissionDenied",
 )
 classifier_test = s(
  host,
  CORE_IVM_HOST,
  errors,
  "fn contract_state_namespace_access_covers_consensus_owned_prefixes()",
  "fn state_syscalls_cannot_forge_delete_or_disclose_queue_plan_admission_marker()",
 )
 q(
  classifier_test,
  CORE_IVM_HOST,
  errors,
  "Kagemusha namespace ownership and delimiter regression",
  '"kagemusha_release_lifecycle_v4_deadbeef"',
  '"kagemushax"',
  "ContractStateNamespaceAccess::ReadOnlySystem",
  "ContractStateNamespaceAccess::User",
  "delimiter-aware matching must not reserve similarly named user state",
 )
def _native_verifier_hydration_contracts(texts,errors):
 core = texts[CORE]
 identity = s(core, CORE, errors, "fn kagemusha_v4_release_verifier_id_has_exact_digest(",
  "fn ensure_release_qualified_kagemusha_v4_verifier_id(")
 q(identity, CORE, errors, "VK digest",
  "if id.backend.as_str()\n        != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4",
  "id.name", ".strip_prefix(circuit)",
  "suffix.strip_prefix('-')", "digest.len() == 64", "byte.is_ascii_digit()",
  "matches!(byte, b'a'..=b'f')")
 o(identity, CORE, errors, "narrow V4 candidate",
  "fn kagemusha_v4_release_verifier_candidate(",
  'kagemusha_v4_release_verifier_id_has_exact_digest(id)\n        || kagemusha_v4_parity_for_circuit(circuit_id).is_some()',
  "fn exact_kagemusha_v4_release_verifier_identity(",
  "if !kagemusha_v4_release_verifier_candidate(id, &record.circuit_id)",
  "return Ok(None)")
 o(identity, CORE, errors, "native VK identity",
  "kagemusha_v4_parity_for_circuit(&record.circuit_id).ok_or_else",
  "verifier_owner_manifest_sha256(record, role)?", "let expected =",
  "kagemusha_recursive_spend_verifier_key_id_v4(", "if !id.is_portable_registry_id()",
  "id != &expected", "record.namespace !=", "record.backend != BackendTag::Halo2IpaPasta",
  "record.curve != kagemusha_v4_verifier_curve(parity)",
  "Ok(Some(KagemushaV4ReleaseVerifierIdentity")
 hydration = s(core, CORE, errors,
  "fn ensure_exact_kagemusha_v4_native_verifier_storage_shape(",
  "fn decode_kagemusha_v4_consensus_release_state(")
 q(hydration, CORE, errors, "native VK storage shape",
  "record.version == 0", "record.commitment == [0; 32]",
  "record.public_inputs_schema_hash == [0; 32]", "record.max_proof_bytes == 0",
  "ConfidentialStatus::Active",
  "record.activation_height.is_some_and(|height| height > 0)",
  "record.withdraw_height.is_some()", "key.bytes.is_empty()",
  "u32::try_from(key.bytes.len()).ok() != Some(record.vk_len)",
  "crate::zk::hash_vk(key) != record.commitment", "ConfidentialStatus::Withdrawn",
  "record.activation_height.is_some()", "record.key.is_some()", "record.vk_len != 0",
  "ConfidentialStatus::Proposed")
 o(hydration, CORE, errors, "atomic native Eq/Ep hydration",
  "pub(crate) fn exact_kagemusha_v4_native_verifier_ids_for_hydration(",
  "for (id, record) in world.verifying_keys().iter()",
  "exact_kagemusha_v4_release_verifier_identity(id, record)?",
  "ensure_exact_kagemusha_v4_native_verifier_storage_shape(record, identity.parity)?",
  "world.verifying_keys_by_circuit().get(&expected_index) != Some(id)",
  ".entry(identity.manifest_sha256)", ".or_insert([None, None])",
  "if slot.replace((id, record)).is_some()", "for pair in pairs.values()",
  "Some((step_eq_id, step_eq)), Some((step_ep_id, step_ep))",
  "step_eq.version != step_ep.version", "step_eq.status != step_ep.status",
  "step_eq.activation_height != step_ep.activation_height",
  "step_eq.withdraw_height != step_ep.withdraw_height",
  "ids.insert((*step_eq_id).clone())", "ids.insert((*step_ep_id).clone())",
  "for ((circuit_id, version), id) in world.verifying_keys_by_circuit().iter()",
  "if !kagemusha_v4_release_verifier_candidate(id, circuit_id)", "continue;",
  "world.verifying_keys().get(id).ok_or_else",
  "if !ids.contains(id) || record.circuit_id != *circuit_id || record.version != *version",
  "Ok(ids)")
 host = s(texts[CORE_IVM_HOST], CORE_IVM_HOST, errors,
  "pub(crate) fn set_zk_snapshots_from_world(", "/// Snapshot durable smart-contract state")
 o(host, CORE_IVM_HOST, errors, "native VK exclusion before generic hydration",
  "Self::validate_zk_elections_snapshot(&elections)?",
  "let native_kagemusha_v4_ids =", "exact_kagemusha_v4_native_verifier_ids_for_hydration(",
  ".map_err(|_| ivm::VMError::NoritoInvalid)?", "let mut vks = BTreeMap::new()",
  "for (id, rec) in world.verifying_keys().iter()",
  "if native_kagemusha_v4_ids.contains(id)", "continue;",
  "vks.insert(id.clone(), rec.clone())", "self.set_verifying_keys(vks)?")
 tests = s(texts[CORE_IVM_HOST], CORE_IVM_HOST, errors,
  "fn zk_snapshot_hydration_separates_exact_native_kagemusha_v4_pairs_from_open_verify(",
  "fn load_vk_record_any_namespace_uses_keyless_registry_backend(")
 q(tests, CORE_IVM_HOST, errors, "native/generic hydration tests",
  "for cancelled in [false, true]", "host.verifying_keys.contains_key(&generic_id)",
  "assert!(!host.verifying_keys.contains_key(id))", "direct_host.set_verifying_keys(direct_map)",
  "fn zk_snapshot_hydration_rejects_malformed_kagemusha_v4_near_matches()",
  'for corruption in ["metadata", "missing_ep", "misindexed"]',
  "assert!(!host.verifying_keys.contains_key(&generic_id))",
  "fn zk_snapshot_hydration_does_not_exempt_allowed_same_backend_lookalike()",
  "assert!(excluded.is_empty())", "allowed WSV lookalike must reach")
 q(texts[CORE_IVM_HOST],CORE_IVM_HOST,errors,"a","fn zk_snapshot_hydration_rejects_invalid_election_selector_without_partial_replacement(")
def _ordinary_lifecycle_transport_contracts(t,e):
 P,R,X,T,V,C,U=CORE_PROXY,TORII_ROUTING,TORII,TORII_TESTS,TORII_PENDING_TESTS,TORII_CATALOG,TORII_CATALOG_TESTS;L="proxy"
 p=t[P];q(p,P,e,L,
  "pub fn validate_ordinary_kagemusha_lifecycle_entrypoint(",
  "signed_lifecycle_entrypoint_context(transaction)",
  "if durable.global_admission_identity.is_some()",
  "durable.enqueue_timestamp_ms,\n            None,",
  "attestation.enqueue_timestamp_ms,\n            None,",
  "OrdinaryKagemushaLifecycleDurable(OrdinaryKagemushaLifecycleAdmissionBindingV1)",
  "fn ordinary_kagemusha_lifecycle_scope_accepts_exact_and_rejects_near_matches(",
  "fn ordinary_kagemusha_lifecycle_certificate_requires_two_distinct_of_four(",
  "fn ordinary_kagemusha_lifecycle_certificate_rejects_binding_roster_route_and_journal_drift(",
  "fn ordinary_kagemusha_lifecycle_durable_claim_must_remain_globally_unbound(")
 for z in ("attestation_count != durability_threshold","previous >= attestation.validator_index",".verify(validator.public_key(), &signing_bytes)"):n(p,P,e,z,2,"p")
 q(t[R],R,e,L,
  "push_accepted_ordinary_kagemusha_lifecycle_for_ingress_strict_durable_claim(",
  ".push_with_lane_with_state_and_routing_plan_strict_durable_claim(",
  "&expected_binding.admission_context","validate_ordinary_kagemusha_lifecycle_entrypoint(")
 x=t[X]
 q(x,X,e,L,
  "expected_binding.validate_durable_admission(",
  "OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Partial",
  "if ordinary_lifecycle_attestations.len()\n                                >= expected.durability_threshold",
  "async fn validate_ordinary_kagemusha_lifecycle_admission_quorum_response(",
  "OrdinaryKagemushaLifecycleAdmissionBindingV1::new(",
  "push_accepted_ordinary_kagemusha_lifecycle_for_ingress_strict_durable_claim(",
  "validate_ordinary_kagemusha_lifecycle_entrypoint(accepted_tx.entrypoint())",
  "validate_ordinary_kagemusha_lifecycle_entrypoint(&TransactionEntrypoint::External(",
 "execute_torii_ordinary_kagemusha_lifecycle_via_proxy(",
 "KAGEMUSHA_LIFECYCLE_TRANSACTION => limited_canonical_signed_post(handler_post_kagemusha_lifecycle_transaction, transaction_max_content_len)")
 q(x,X,e,L,"Vec<PendingToriiProxyRequest>",".entry(pending_key)",".push(PendingToriiProxyRequest","waiter.waiter_token, waiter_token","for pending in pending_waiters {")
 q(t[V],V,e,L,"fn identical_torii_proxy_submissions_keep_all_pending_waiters(","fn identical_torii_proxy_attempt_cleanup_is_waiter_scoped(")
 for z,c in ((".take(expected.durability_threshold)",2),("admission_binding: None",4),("!= Some(submitted_signed_transaction_hash)",2)):n(x,X,e,z,c,"p")
 n(x,X,e,"if queue_plan_binding.is_some()",2,L)
 n(x,X,e,"OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Quorum",2,"quorum")
 q(t[T],T,e,L,
  "fn ordinary_kagemusha_lifecycle_proxy_requires_exact_f_plus_one_unbound_certificate(",
  "fn generic_transaction_proxy_rejects_ordinary_lifecycle_and_dedicated_route_rejects_bare_202(","fn generic_transaction_batch_rejects_ordinary_lifecycle_without_enqueuing(",
  "fn ordinary_kagemusha_lifecycle_receiver_rolls_durable_retry_without_queue_plan_publication(")
 q(t[C],C,e,"route",
  '"/v1/offline/kagemusha/lifecycle-v4/transactions"',
  "AuthenticationPolicy::CanonicalSignedBody")
 q(t[U],U,e,"route",
  "fn ordinary_kagemusha_lifecycle_has_one_dedicated_canonical_signed_route(",
  "assert_eq!(crate::uri::KAGEMUSHA_LIFECYCLE_TRANSACTION, route.path());")
def _isi_contracts(texts,errors):
 model_isi = texts[MODEL_ISI]
 q(
  model_isi,
  MODEL_ISI,
  errors,
  "stable canonical lifecycle instruction wires",
  '"iroha.offline.kagemusha.recursive_release.enable.v1"',
  '"iroha.offline.kagemusha.recursive_release.cancel.v1"',
  '"iroha.offline.kagemusha.recursive_release.deactivate.v1"',
  "impl crate::seal::Instruction for EnableKagemushaRecursiveIssuanceV4",
  "impl crate::seal::Instruction for CancelKagemushaRecursiveReleaseV4",
  "impl crate::seal::Instruction for DeactivateKagemushaRecursiveIssuanceV4",
  "impl<'a> norito::core::DecodeFromSlice<'a> for EnableKagemushaRecursiveIssuanceV4",
  "impl_decode_one_canonical_offline_field!(CancelKagemushaRecursiveReleaseV4",
  "impl_decode_one_canonical_offline_field!(DeactivateKagemushaRecursiveIssuanceV4",
  "fn release_lifecycle_terminal_isis_validate_box_and_decode_both_layouts()",
  "fn release_lifecycle_enable_isi_boxes_and_rejects_trailing_bytes_in_both_layouts()",
 )
 for instruction in (
  "EnableKagemushaRecursiveIssuanceV4",
  "CancelKagemushaRecursiveReleaseV4",
  "DeactivateKagemushaRecursiveIssuanceV4",
 ):
  q(
   texts[MODEL_ISI_REGISTRY],
   MODEL_ISI_REGISTRY,
   errors,
   "direct lifecycle instruction boxing",
   f"impl_direct_instruction_box!(crate::isi::offline::{instruction});",
  )
  q(
   texts[CORE_ISI_REGISTRY],
   CORE_ISI_REGISTRY,
   errors,
   "direct lifecycle executor dispatch",
   f"dispatch_instruction::<iroha_data_model::isi::offline::{instruction}>",
  )
def _carrier_contracts(texts,errors):
 lifecycle = texts[CORE_LIFECYCLE]
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "affine direct lifecycle transaction carrier",
  "transaction.admission_intent() != TransactionAdmissionIntent::Ordinary",
  "fn require_no_proof_attachments(",
  "transaction.attachments().is_some()",
  "require_no_proof_attachments(transaction)?;",
  "require_distinct_governance_signers(transaction, kind)?;",
  "let [instruction] = instructions.as_ref() else",
  "transaction_intent: transaction.hash()",
  "instruction_digest: Hash::new(&norito::encode_canonical(instruction)",
  "let context = context.take()",
  "Kagemusha V4 lifecycle mutation requires one exact direct External instruction",
  "fn direct_lifecycle_context_binds_exact_instruction_and_transaction()",
  "fn lifecycle_context_rejects_multi_instruction_and_nonordinary_carriers()",
  "fn direct_lifecycle_context_is_consumed_exactly_once()",
 )
 tx = texts[CORE_TX]
 q(
  tx,
  CORE_TX,
  errors,
  "narrow verified-multisig lifecycle admission wiring",
  "pub(crate) use authority_admission::{",
  "instructions_allow_direct_kagemusha_lifecycle_authority,",
  "let lifecycle_entrypoint =",
  "signed_lifecycle_entrypoint_context(tx)",
  "let allows_direct_kagemusha_lifecycle_authority = lifecycle_entrypoint.is_some()",
  "if instructions_allow_direct_kagemusha_lifecycle_authority(instructions)",
  "&& !allows_direct_kagemusha_lifecycle_authority",
 )
 f(
  tx,
  CORE_TX,
  errors,
  "unverified direct lifecycle authority exception",
  "let allows_direct_kagemusha_lifecycle_authority = tx.multisig_signatures().is_some()",
 )
 q(
  texts[CORE_TX_AUTHORITY_ADMISSION],
  CORE_TX_AUTHORITY_ADMISSION,
  errors,
  "one-exact-instruction lifecycle authority classifier",
  "pub(crate) fn instructions_allow_direct_kagemusha_lifecycle_authority(",
  "let [instruction] = instructions else",
  "direct_lifecycle_entrypoint_kind(instruction).is_some()",
 )
 q(
  texts[CORE_TX_LIFECYCLE_TESTS],
  CORE_TX_LIFECYCLE_TESTS,
  errors,
  "narrow verified-multisig lifecycle admission regressions",
  "fn direct_kagemusha_lifecycle_authority_requires_one_exact_instruction()",
  "fn exact_kagemusha_lifecycle_accepts_verified_multisig_authority_at_stateful_admission()",
  "fn kagemusha_v4_non_lifecycle_proof_attachments_remain_outside_the_lifecycle_gate()",
  "fn kagemusha_v4_lifecycle_proof_attachments_fail_closed_at_stateful_admission()",
 )
 state = texts[CORE_STATE]
 q(
  state,
  CORE_STATE,
  errors,
  "one-shot lifecycle state carrier",
  "pub(crate) kagemusha_release_lifecycle_entrypoint: Option<LifecycleEntrypointContext>",
  "kagemusha_release_lifecycle_entrypoint: None",
 )
 committed = texts[CORE_COMMITTED_CONTEXT]
 o(
  committed,
  CORE_COMMITTED_CONTEXT,
  errors,
  "committed lifecycle context reset and External-only derivation",
  "state_transaction.kagemusha_release_lifecycle_entrypoint = None",
  "TransactionEntrypoint::External(transaction)",
  "if state_transaction.kagemusha_taira_canary_external_entrypoint",
  "signed_lifecycle_entrypoint_context(transaction)",
 )
 block = texts[CORE_BLOCK]
 o(
  block,
  CORE_BLOCK,
  errors,
  "block-admission lifecycle context derivation",
  "let lifecycle_entrypoint =",
  "signed_lifecycle_entrypoint_context(tx)",
  "state_tx.kagemusha_release_lifecycle_entrypoint = lifecycle_entrypoint",
  "StateBlock::validate_stateful_admission",
 )
 executor = texts[CORE_EXECUTOR]
 execute = s(executor, CORE_EXECUTOR, errors, "pub fn execute_transaction(")
 o(
  execute,
  CORE_EXECUTOR,
  errors,
  "executor lifecycle reset and direct-carrier derivation",
  "state_transaction.kagemusha_release_lifecycle_entrypoint = None",
  "if state_transaction.kagemusha_taira_canary_external_entrypoint",
  "signed_lifecycle_entrypoint_context(&transaction)?",
  "state_transaction.current_tx_hash = Some(tx_hash.clone())",
 )
 q(
  execute,
  CORE_EXECUTOR,
  errors,
  "verified-multisig lifecycle execution exception",
  "let exact_kagemusha_release_lifecycle = state_transaction",
  ".kagemusha_release_lifecycle_entrypoint",
  ".is_some()",
  "&& transaction.multisig_signatures().is_some()",
 )
def _transition_contracts(texts,errors):
 activation = texts[CORE_ACTIVATION]
 o(
  activation,
  CORE_ACTIVATION,
  errors,
  "stage validation before atomic lifecycle mutation",
  "kagemusha_release_lifecycle::require_direct_stage(&self, state_transaction)?",
  "ensure_kagemusha_recursive_release_v4_activation_authorized",
  "validate_offline_attestation_policy_for_release_activation(",
  "validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?",
  "kagemusha_release_lifecycle::plan_staged(",
  ".insert(release_key, release_record_bytes)",
  "kagemusha_release_lifecycle::commit_staged(lifecycle_plan, state_transaction)",
 )
 f(
  activation,
  CORE_ACTIVATION,
  errors,
  "premature singleton policy installation during staging",
  "OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY",
 )
 lifecycle = texts[CORE_LIFECYCLE]
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "manifest-addressed fail-closed lifecycle state",
  "load_lifecycle_by_manifest(world, &binding.manifest_sha256)?",
  "state.artifact_binding.manifest_sha256 != *manifest_sha256",
  "state.promotion_binding.manifest_sha256 != *manifest_sha256",
  "Kagemusha V4 lifecycle state differs from its manifest-addressed key",
  "pub(super) fn issuance_enabled(",
  "loaded.state.issuance_enabled()",
  "require_bound_consensus_artifacts(world, &loaded.state, true)?",
  "pub(super) fn require_staged(",
  "require_bound_consensus_artifacts(&state_transaction.world, &loaded.state, false)",
  "pub(super) fn plan_staged(",
  "device_attestation_policy: OfflineDeviceAttestationPolicy",
  "phase: KagemushaV4ReleaseLifecyclePhaseV1::Staged",
 )
 commit = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "fn commit_transition(",
  "impl Execute for EnableKagemushaRecursiveIssuanceV4",
 )
 o(
  commit,
  CORE_LIFECYCLE,
  errors,
  "final loaded-bytes CAS before lifecycle/replay commit",
  ".get(&loaded.key)",
  "!= Some(&loaded.bytes)",
  "next.validate()",
  ".insert(loaded.key, bytes)",
  RK,
  ".insert(marker, ())",
 )
 enable = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "impl Execute for EnableKagemushaRecursiveIssuanceV4",
  "struct CancelledVerifierWithdrawalPlan",
 )
 q(
  enable,
  CORE_LIFECYCLE,
  errors,
  "evidence-backed enable transition",
  "LifecycleEntrypointKind::Enable",
  "only the exact staged release may be enabled",
  "predecessor != witness.expected_predecessor_lifecycle",
  ".verify_exact(&expectations_bytes, controller, &reservation_bytes)",
  ".stage_finality_receipt",
  "verified_receipt.activation_transaction_intent()",
  "loaded.state.stage_transaction_intent",
  "validate_offline_attestation_policy_transition_from_state(",
  "require_v4_taira_canary_consumed(",
  ".validator_liveness_evidence",
  ".verify_exact(",
  "liveness_terminal_is_current_parent(",
  "validator_set_matches(&current_topology, &expected_topology)",
  "canonical_genesis_hash != Some(runtime.genesis_expected_hash)",
  "current_height < manifest.activation_height",
  "current_height >= manifest.withdrawal_height",
  "now_ms >= challenge.expires_at_unix_ms",
  "KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled))",
  "(*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY).clone()",
  "commit_transition(marker, loaded, next, state_transaction)",
 )
 verifier_auth = s(
  lifecycle, CORE_LIFECYCLE, errors, "fn exact_lifecycle_verifier_record",
  "fn require_bound_consensus_artifacts",
 )
 q(
  verifier_auth, CORE_LIFECYCLE, errors,
  "borrowed exact release-qualified Eq/Ep verifier authentication",
  "Result<&'world VerifyingKeyRecord, String>", ".verifying_keys()", ".get(id)",
  "ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;",
  "kagemusha_v4_circuit_id(parity).to_owned()", "state.verifier_version",
  "record.version != state.verifier_version",
  "record.status != ConfidentialStatus::Active",
  "world.verifying_keys_by_circuit().get(&expected_index) != Some(id)",
  "Result<[&'world VerifyingKeyRecord; 2], String>",
  "binding.manifest_sha256", "state.step_eq_verifier_key_id != expected_eq",
  "state.step_ep_verifier_key_id != expected_ep", '"Eq"', '"Ep"',
 )
 f(
  verifier_auth, CORE_LIFECYCLE, errors,
  "cloned verifier records on the issuance/readiness path", ".cloned()", ".clone()",
 )
 qualified = s(texts[CORE], CORE, errors,
  "fn exact_kagemusha_v4_release_verifier_identity(",
  "fn ensure_release_qualified_kagemusha_v4_verifier_id(")
 q(
  qualified, CORE, errors, "release owner/circuit-qualified verifier identity",
  "verifier_owner_manifest_sha256(record, role)?",
  "kagemusha_recursive_spend_verifier_key_id_v4(", "parity", "manifest_sha256",
  "kagemusha_v4_parity_for_circuit(&record.circuit_id).ok_or_else",
  "!id.is_portable_registry_id()", "id != &expected",
 )
 withdrawal = s(
  lifecycle, CORE_LIFECYCLE, errors, "struct CancelledVerifierWithdrawalPlan",
  CX,
 )
 q(
  withdrawal, CORE_LIFECYCLE, errors, "atomic two-verifier cancellation withdrawal plan",
  "records: [(VerifyingKeyId, VerifyingKeyRecord); 2]",
  "let [step_eq, step_ep] = exact_lifecycle_verifier_records(world, state)?;",
  "state.step_eq_verifier_key_id.clone()", "state.step_ep_verifier_key_id.clone()",
  "record.status = ConfidentialStatus::Withdrawn", "record.activation_height = None",
  "record.withdraw_height = Some(current_height)",
  "record.key = None", "record.vk_len = 0", "for (id, record) in self.records",
  "state_transaction.world.verifying_keys.remove(id.clone())",
  "state_transaction.world.verifying_keys.insert(id, record)",
 )
 o(
  withdrawal, CORE_LIFECYCLE, errors,
  "pre-activation cancellation clears the never-reached boundary before withdrawal",
  "record.status = ConfidentialStatus::Withdrawn", "record.activation_height = None",
  "record.withdraw_height = Some(current_height)", "record.key = None", "record.vk_len = 0",
 )
 cancel_execute = s(
  lifecycle, CORE_LIFECYCLE, errors, CX,
  "impl Execute for DeactivateKagemushaRecursiveIssuanceV4",
 )
 o(
  cancel_execute, CORE_LIFECYCLE, errors,
  "full verifier validation and planning before lifecycle/replay commit and tombstone apply",
  "next.validate()", "plan_cancelled_verifier_withdrawal(",
  "commit_transition(marker, loaded, next, state_transaction)?;",
  "withdrawal.apply(state_transaction);", "Ok(())",
 )
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "authenticated cancel/deactivate transitions",
  CX,
  "LifecycleEntrypointKind::Cancel",
  "load_lifecycle_by_manifest(&state_transaction.world, &cancellation.manifest_sha256)",
  "cancellation.expected_predecessor_lifecycle",
  "KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(Box::new(cancelled))",
  "impl Execute for DeactivateKagemushaRecursiveIssuanceV4",
  "LifecycleEntrypointKind::Deactivate",
  "load_lifecycle_by_manifest(&state_transaction.world, &deactivation.manifest_sha256)",
  "only enabled Kagemusha V4 issuance may be deactivated",
  "deactivation.expected_predecessor_lifecycle",
  "KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(Box::new(deactivated))",
  "fn liveness_terminal_must_be_the_exact_canonical_parent()",
  "fn runtime_validator_set_is_exact_but_rotation_independent()",
 )
 q(
  texts[CORE],
  CORE,
  errors,
  "lifecycle-gated readiness surface",
  "kagemusha_release_lifecycle::issuance_enabled(world, &lifecycle_binding)?",
  "kagemusha_release_lifecycle::issuance_enabled(world, binding).unwrap_or(false)",
 )
def _redemption_contracts(texts,errors):
 lifecycle = texts[CORE_LIFECYCLE]
 redemption_policy = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "pub(super) fn redemption_policy(",
  "/// Require the exact promotion to remain staged",
 )
 q(
  redemption_policy,
  CORE_LIFECYCLE,
  errors,
  "terminal release-scoped redemption policy",
  "load_lifecycle(world, binding)?",
  "Kagemusha V4 release lifecycle is absent",
  "loaded.state.device_attestation_policy",
  ".device_attestation_policy_norito",
  ".matches_bytes(&policy_bytes)",
 )
 f(
  redemption_policy,
  CORE_LIFECYCLE,
  errors,
  "issuance-phase gating of full redemption",
  "issuance_enabled",
  "require_bound_consensus_artifacts",
 )
 policy = texts[CORE_REDEMPTION_POLICY]
 q(
  policy,
  CORE_REDEMPTION_POLICY,
  errors,
  "release-scoped redemption with live emergency trust compatibility",
  "fn ensure_redemption_registration_policy_compatibility(",
  "release_roots != admission_roots",
  "!release_revocations.is_subset(&admission_revocations)",
  "validate_android_attestation_status_transition(",
  "!release_non_valid.is_subset(&admission_non_valid)",
  "fn ensure_existing_release_registration_trust_is_unchanged(",
  "release_revocations != current_revocations || android_status_changed",
  "let current_policy = effective_offline_device_attestation_policy(state_transaction)?",
  "validate_offline_attestation_policy(",
  "state.admission_policy_hash != release_policy_hash",
  "state.admission_policy_hash != current_policy_hash",
  "if current_policy_hash != release_policy_hash",
  "if state.admission_policy_hash == release_policy_hash",
  "ensure_android_app_allowed_by_policy(release_policy",
  "ensure_ios_app_allowed_by_policy(",
  "release_policy,",
  "let policy = effective_offline_device_attestation_policy(state_transaction)?",
 )
 redeem = s(
  texts[CORE],
  CORE,
  errors,
  "impl Execute for RedeemKagemushaRecursiveV4",
 )
 o(
  redeem,
  CORE,
  errors,
  "release-policy lookup before redemption authentication and replay",
  "kagemusha_release_lifecycle::redemption_policy(",
  "authenticate_kagemusha_v4_redeem_submission_before_replay(",
  "let replay_markers = match replay_status",
 )
 tests = texts[CORE_REDEMPTION_POLICY_TESTS]
 q(
  tests,
  CORE_REDEMPTION_POLICY_TESTS,
  errors,
  "active release-scoped redemption policy regressions",
  "fn release_scoped_redemption_survives_singleton_policy_rotation()",
  "new issuance must remain bound to the current singleton policy",
  "fn release_scoped_registration_rejects_incompatible_live_trust_rotation()",
  "an old compact registration cannot bypass a live emergency trust update",
  "fn release_scoped_redemption_accepts_only_compatible_current_policy_reregistration()",
  "a current admission cannot replace a stricter historical trust basis",
 )
 f(
  tests,
  CORE_REDEMPTION_POLICY_TESTS,
  errors,
  "disabled redemption policy regression",
  "#[ignore]",
  "#[cfg",
 )
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "terminal redemption policy regression",
  "fn redemption_policy_is_available_in_terminal_lifecycle_state()",
 )
def _runtime_projection_contracts(texts,errors):
 model_isi = texts[MODEL_ISI]
 q(
  model_isi,
  MODEL_ISI,
  errors,
  "activation-wire runtime projection identity",
  "pub runtime_effective_config_sha256: [u8; 32]",
  "|| self.runtime_effective_config_sha256 == [0; 32]",
  "let runtime_effective_config_sha256 = super::decode_aos_canonical_field::<[u8; 32]>(",
  "runtime_effective_config_sha256,",
 )
 receipt = texts[MODEL_PROMOTION_RECEIPT]
 o(
  receipt,
  MODEL_PROMOTION_RECEIPT,
  errors,
  "activation digest against unanimous validator projection",
  "let activation = direct_activation_instruction(&self.activation_transaction)?",
  "if activation.runtime_effective_config_sha256()",
  "validator_bodies[0]",
  ".runtime_effective_config",
  ".consensus_sha256()?",
  "KagemushaPromotionReceiptValidationError::ActivationPayload",
 )
 q(
  texts[MODEL_LIFECYCLE],
  MODEL_LIFECYCLE,
  errors,
  "persisted nonzero runtime projection identity",
  "pub runtime_effective_config_sha256: [u8; 32]",
  "|| self.runtime_effective_config_sha256 == [0; 32]",
 )
 activation = texts[CORE_ACTIVATION]
 o(
  activation,
  CORE_ACTIVATION,
  errors,
  "signed activation digest retained by staging",
  "kagemusha_release_lifecycle::require_direct_stage(&self, state_transaction)?",
  "let runtime_effective_config_sha256 = self.runtime_effective_config_sha256",
  "kagemusha_release_lifecycle::plan_staged(",
  "runtime_effective_config_sha256,",
  "kagemusha_release_lifecycle::commit_staged(lifecycle_plan, state_transaction)",
 )
 lifecycle = texts[CORE_LIFECYCLE]
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "active lifecycle runtime lock",
  "fn active_runtime_effective_config_sha256(",
  "KagemushaV4ReleaseLifecyclePhaseV1::Staged",
  "KagemushaV4ReleaseLifecyclePhaseV1::Enabled(_)",
  ".replace(state.runtime_effective_config_sha256)",
  "multiple active Kagemusha V4 release lifecycle records exist",
  "pub(crate) fn require_local_runtime_effective_config(",
  "if local_runtime_effective_config_sha256 != Some(expected)",
  "pub(crate) fn runtime_consensus_parameters_frozen(",
  "another Kagemusha V4 release is already staged or enabled",
  "loaded.state.runtime_effective_config_sha256 != runtime_effective_config_sha256",
  "fn runtime_projection_gate_is_active_only_for_staged_or_enabled_state()",
 )
 active_scan = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "fn active_runtime_effective_config_sha256(",
  "/// Fail closed unless every active lifecycle",
 )
 o(
  active_scan,
  CORE_LIFECYCLE,
  errors,
  "prefix-bounded active lifecycle scan",
  "let range_start: StatePath = KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1",
  ".range(range_start..)",
  ".starts_with(KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_KEY_PREFIX_V1)",
  "break;",
  "if key != &lifecycle_key(&state.artifact_binding.manifest_sha256)?",
 )
 f(
  active_scan,
  CORE_LIFECYCLE,
  errors,
  "whole-WSV active lifecycle scan",
  ".iter()",
 )
 state = texts[CORE_STATE]
 q(
  state,
  CORE_STATE,
  errors,
  "immutable process-local runtime projection",
  "kagemusha_runtime_effective_config_sha256: SyncOnceCell<[u8; 32]>",
 )
 runtime_state = texts[CORE_STATE_RUNTIME_CONFIG]
 q(
  runtime_state,
  CORE_STATE_RUNTIME_CONFIG,
  errors,
  "immutable process-local runtime projection",
  "pub fn install_kagemusha_runtime_effective_config_sha256(",
  "if digest == [0; 32]",
  "match self.kagemusha_runtime_effective_config_sha256.set(digest)",
  "Ok(()) => Ok(())",
  "pub(crate) fn require_kagemusha_runtime_effective_config_for_world(",
  "require_local_runtime_effective_config(",
  "pub fn require_committed_kagemusha_runtime_effective_config(",
 )
 runtime_install = s(
  runtime_state,
  CORE_STATE_RUNTIME_CONFIG,
  errors,
  "pub fn install_kagemusha_runtime_effective_config_sha256(",
  "/// Check one committed or prospective world",
 )
 o(
  runtime_install,
  CORE_STATE_RUNTIME_CONFIG,
  errors,
  "atomic install-once runtime projection identity",
  "match self.kagemusha_runtime_effective_config_sha256.set(digest)",
  "Ok(()) => Ok(())",
  "Err(digest)",
  "self.kagemusha_runtime_effective_config_sha256.get() == Some(&digest)",
  "Err(_) => {",
  "Kagemusha runtime-effective config digest is already installed",
 )
 q(
  texts[CORE_STATE_RUNTIME_CONFIG_TESTS],
  CORE_STATE_RUNTIME_CONFIG_TESTS,
  errors,
  "sequential and concurrent runtime projection install regressions",
  "kagemusha_runtime_projection_identity_is_install_once",
  "fn concurrent_runtime_projection_install_accepts_only_one_distinct_digest()",
  "only the installed digest may succeed",
  "a distinct concurrent digest was accepted",
  "the winning digest remains idempotent",
 )
 runtime_config = texts[CORE_RUNTIME_CONFIG]
 q(
  runtime_config,
  CORE_RUNTIME_CONFIG,
  errors,
  "authenticated complete local runtime derivation",
  "pub fn derive_from_signed_genesis(",
  "pub fn derive_from_authenticated_snapshot(",
  "signed_genesis_context: SumeragiV2GenesisContextParameters",
  "invalid authenticated snapshot bootstrap",
  "bootstrap.context.network_id.clone()",
  "bootstrap.context.mode",
  "signed_genesis_context,",
  "fn derive_from_authenticated_parts(",
  "config.sumeragi.role != NodeRole::Validator",
  "mode != ConsensusMode::Permissioned",
  "network_id != NetworkId::from_genesis_hash(config.genesis.expected_hash)",
  "validator_pops.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT",
  "trusted.pops.len() == validator_pops.len()",
  "bls_normal_pop_verify(validator_id.public_key(), pop).is_ok()",
  "config.network.public_address.value().clone()",
  ".v2_config(block_cadence, mode)",
  "projection.validate().map_err(|error| error.to_string())?",
 )
 seal_reader = texts[IROHAD_VALIDATOR_SEAL_READER]
 q(
  seal_reader,
  IROHAD_VALIDATOR_SEAL_READER,
  errors,
  "root-owned exact verified local validator seal",
  "pub fn read_configured_kagemusha_validator_qualification_seal(",
  ".kagemusha_validator_qualification_seal_path",
  "RootOwnedNoReplaceArtifactPublicationTarget::read_root_owned_bounded(",
  "KAGEMUSHA_VALIDATOR_QUALIFICATION_SEAL_MAX_BYTES_V1",
  "decode_exact_kagemusha_validator_qualification_seal(&exact)",
 )
 exact_seal = s(
  seal_reader,
  IROHAD_VALIDATOR_SEAL_READER,
  errors,
  "fn decode_exact_kagemusha_validator_qualification_seal(",
  "/// Prepared root-owned, no-replace destination",
 )
 q(
  exact_seal,
  IROHAD_VALIDATOR_SEAL_READER,
  errors,
  "bounded canonical signature-verified validator seal",
  "norito::decode_canonical_with_limits::<KagemushaV4ValidatorQualificationSealV1>(",
  "seal.verify()",
  "norito::encode_canonical(&seal)",
  "Kagemusha validator qualification seal is not exact canonical Norito",
 )
 node = texts[IROHAD_STARTUP]
 o(
  node,
  IROHAD_STARTUP,
  errors,
  "startup-authenticated local runtime projection installation",
  "if !state.kagemusha_release_catalog.is_configured()",
  "if let Some(bootstrap) = authenticated_snapshot_bootstrap",
  "let Some(_) = config",
  ".kagemusha_validator_qualification_seal_path",
  ".as_ref()",
  "else {",
  '"authenticated-snapshot Kagemusha startup has no configured local validator qualification seal; staging and active-lifecycle output remain fail-closed"',
  "return Ok(())",
  "read_configured_kagemusha_validator_qualification_seal(",
  "seal.body.validator_id != config.common.peer.id",
  "derive_from_authenticated_snapshot(",
  "seal.body.runtime_effective_config.genesis_context",
  "if derived.projection() != &seal.body.runtime_effective_config",
  "        derived\n    } else {",
  "derive_from_signed_genesis(",
  "let digest = runtime_effective_config",
  ".projection()",
  ".consensus_sha256()",
  ".install_kagemusha_runtime_effective_config_sha256(",
 )
 q(
  texts[IROHAD],
  IROHAD,
  errors,
  "startup runtime projection installer call",
  "kagemusha_startup::install_runtime_effective_config(",
 )
 q(
  node,
  IROHAD_STARTUP,
  errors,
  "injectable exact validator-seal reader seam",
  "pub fn install_runtime_effective_config_with_validator_seal_reader(",
  "read_configured_kagemusha_validator_qualification_seal: impl FnOnce(",
  "kagemusha_validator_qualification_command::read_configured_kagemusha_validator_qualification_seal,",
  "let seal = read_configured_kagemusha_validator_qualification_seal(config)?;",
 )
 startup_tests = texts[IROHAD_STARTUP_TESTS]
 q(
  startup_tests,
  IROHAD_STARTUP_TESTS,
  errors,
  "authenticated snapshot startup install and fail-closed regressions",
  "fn authenticated_snapshot_with_valid_local_seal_installs_runtime_digest()",
  "fn authenticated_snapshot_rejects_wrong_local_peer_without_installing()",
  "fn authenticated_snapshot_rejects_projection_mismatch_without_installing()",
  "fn authenticated_snapshot_without_configured_seal_does_not_install()",
  "install_runtime_effective_config_with_validator_seal_reader(",
  'assert!(error.contains("different local peer"));',
  'assert!(error.contains("effective snapshot runtime differs"));',
  'panic!("an absent configured seal must not invoke the reader")',
  "assert_runtime_digest_absent(&state, 0xa3);",
 )
 f(
  startup_tests,
  IROHAD_STARTUP_TESTS,
  errors,
  "disabled authenticated snapshot startup regression",
  "#[ignore]",
 )
 lifecycle = texts[CORE_LIFECYCLE]
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "catalog-or-lifecycle consensus-parameter freeze",
  "pub(crate) fn validate_runtime_consensus_parameter_update(",
  "SumeragiParameter::MaxClockDriftMs(_)",
  "SumeragiNposParameters::parameter_id()",
  "let lifecycle_frozen = runtime_consensus_parameters_frozen(world)",
  "if catalog_configured || lifecycle_frozen",
  "authenticated Kagemusha qualification freezes consensus runtime parameters",
  "fn authenticated_catalog_closes_consensus_parameter_drift_before_stage()",
  "an authenticated catalog must lock {parameter:?} before Stage",
 )
 o(
  texts[CORE_WORLD],
  CORE_WORLD,
  errors,
  "world parameter execution runtime lock",
  "crate::smartcontracts::isi::offline::validate_runtime_consensus_parameter_update(",
  "self.inner()",
  "state_transaction.kagemusha_release_catalog.is_configured()",
  "super::parameter_validation::validate_ivm_heap_parameter(self.inner())?;",
 )
 apply = texts[CORE_SUMERAGI_APPLY]
 for start, end, state_block in (
  ("pub(crate) fn validate_candidate(", "pub(crate) fn revalidate_recovered_candidate(", "valid"),
  ("fn validate_and_apply(", "fn persist_post_apply_metadata(", "valid_block"),
 ):
  section = s(apply, CORE_SUMERAGI_APPLY, errors, start, end)
  o(
   section,
   CORE_SUMERAGI_APPLY,
   errors,
   "prospective state runtime check before witness/Kura effects",
   "ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(",
   f"self.validate_prospective_autoscale_retirement_queue({state_block}.as_ref(), &state_block)?",
   ".require_kagemusha_runtime_effective_config_for_world(state_block.world())",
   "let witness = state_block",
   ".take_exec_witness()",
  )
 apply_tests = texts[CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS]
 q(
  apply_tests,
  CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS,
  errors,
  "production proposal and Commit runtime-projection regressions",
  "fn production_proposal_validation_enforces_kagemusha_runtime_projection()",
  "fn production_commit_apply_enforces_kagemusha_runtime_projection()",
  '[(None, "missing"), (Some([0x56; 32]), "mismatched")]',
  ".validate_candidate(&fixture.context, &fixture.body)",
  ".execute(&fixture.context, &mut store, &fixture.task)",
  "assert_eq!(state.committed_height(), 0);",
  "assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);",
  'expect("the exact startup projection must permit Commit apply")',
 )
 f(
  apply_tests,
  CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS,
  errors,
  "disabled production apply runtime-projection regression",
  "#[ignore]",
 )
 worker = texts[CORE_SUMERAGI_WORKER]
 n(
  worker,
  CORE_SUMERAGI_WORKER,
  errors,
  RC,
  2,
  "ordinary and recovered pre-sign runtime recheck",
 )
 o(
  worker,
  CORE_SUMERAGI_WORKER,
  errors,
  "ordinary and recovered signing runtime checks",
  "V2IoCommand::Sign {",
  RC,
  "sign_consensus_task(",
  "V2IoCommand::RecoveredLifecycleSign(task)",
  RC,
  "sign_recovered_lifecycle_task(",
 )
 worker_tests = texts[CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS]
 q(
  worker_tests,
  CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS,
  errors,
  "production Prepare and Commit signing runtime-projection regressions",
  "fn production_vote_worker_rejects_missing_and_mismatched_kagemusha_projection()",
  "fn production_vote_worker_signs_prepare_and_commit_for_exact_kagemusha_projection()",
  "for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit]",
  '[(None, "missing"), (Some([0x56; 32]), "mismatched")]',
  "V2IoCompletion::RecoveryRequired(reason)",
  "run_kagemusha_runtime_gated_vote(Some([0x55; 32]), phase)",
  "V2IoCompletion::Signature { signature, .. }",
 )
 f(
  worker_tests,
  CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS,
  errors,
  "disabled production signing runtime-projection regression",
  "#[ignore]",
 )
 o(
  texts[CORE_SUMERAGI_RUNNER],
  CORE_SUMERAGI_RUNNER,
  errors,
  "normal replay runtime check after startup reconstruction",
  "if pending_kura_apply.is_none()",
  ".require_committed_kagemusha_runtime_effective_config()",
  "match pending_kura_apply",
 )
 o(
  texts[CORE_SUMERAGI_PENDING_KURA],
  CORE_SUMERAGI_PENDING_KURA,
  errors,
  "pending-tip runtime check after reconstruction",
  '"finished lifecycle-owned interrupted-tip local Apply recovery"',
  ".require_committed_kagemusha_runtime_effective_config()",
  "reconcile_pending_lane_startup(",
 )
def _signature_floor_contracts(texts,errors):
 receipt = texts[MODEL_PROMOTION_RECEIPT]
 q(
  receipt,
  MODEL_PROMOTION_RECEIPT,
  errors,
  "distinct-signer governance policy floor",
  "fn kagemusha_v4_governance_policy_requires_distinct_signers(",
  "usize::from(policy.threshold()) >= KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS",
  "policy.members().len() >= KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS",
  ".all(|member| member.weight() < policy.threshold())",
  "if !kagemusha_v4_governance_policy_requires_distinct_signers(policy)",
 )
 lifecycle = texts[CORE_LIFECYCLE]
 signer_gate = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "fn require_distinct_governance_signers(",
  "/// Derive lifecycle context only from one ordinary",
 )
 o(
  signer_gate,
  CORE_LIFECYCLE,
  errors,
  "verified canonical distinct transaction-signature floor",
  "transaction.verify_signature()",
  ".multisig_signatures()",
  ".map_or(0, |bundle| bundle.signatures.len())",
  "if signer_count < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS",
  "verified distinct governance signers",
 )
 carrier = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "pub(crate) fn signed_lifecycle_entrypoint_context(",
  "struct LoadedLifecycle",
 )
 o(
  carrier,
  CORE_LIFECYCLE,
  errors,
  "signature floor before direct lifecycle context creation",
  "direct_lifecycle_entrypoint_kind(instruction)",
  "transaction.admission_intent() != TransactionAdmissionIntent::Ordinary",
  "require_distinct_governance_signers(transaction, kind)?",
  "Ok(Some(LifecycleEntrypointContext",
 )
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "all-four-kind distinct-signer regressions",
  "fn lifecycle_state_rejects_a_policy_with_one_threshold_weight_member()",
  "fn every_lifecycle_kind_rejects_one_threshold_weight_signer()",
  "for kind in [\n            LifecycleEntrypointKind::Stage,\n            LifecycleEntrypointKind::Enable,\n            LifecycleEntrypointKind::Cancel,\n            LifecycleEntrypointKind::Deactivate,\n        ]",
  "the direct carrier must apply the distinct-signer gate",
 )
 q(
  texts[CORE_TX_LIFECYCLE_TESTS],
  CORE_TX_LIFECYCLE_TESTS,
  errors,
  "stateful-admission distinct-signer regression",
  "fn exact_kagemusha_lifecycle_rejects_one_threshold_weight_signer_at_stateful_admission()",
  "the generic weighted threshold accepts member A alone",
  "Kagemusha lifecycle admission requires two distinct signers",
  "requires at least 2 verified distinct governance signers",
 )
 direct_fixture = s(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "fn lifecycle_transaction(",
  "fn lifecycle_state(",
 )
 o(
  direct_fixture,
  CORE_LIFECYCLE,
  errors,
  "direct ordinary multisig lifecycle execution fixture",
  "TransactionBuilder::new(",
  ".with_instructions([instruction])",
  ".with_admission_intent(TransactionAdmissionIntent::Ordinary)",
  ".sign_multisig(keys.iter().map(KeyPair::private_key))",
 )
 q(
  lifecycle,
  CORE_LIFECYCLE,
  errors,
  "direct signed ordinary Cancel and Deactivate transition regressions",
  "fn direct_ordinary_multisig_cancel_executes_exact_staged_transition()",
  "fn direct_ordinary_multisig_deactivate_executes_exact_enabled_transition()",
  "signed_lifecycle_entrypoint_context(&signed)",
  '.expect("execute exact cancellation transition")',
  "KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled)",
  "assert_eq!(cancelled.cancellation_transaction_intent, signed.hash());",
  '.expect("execute exact deactivation transition")',
  "KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated)",
  "assert_eq!(deactivated.deactivation_transaction_intent, signed.hash());",
  "assert!(transaction.kagemusha_release_lifecycle_entrypoint.is_none());",
 )
 fixture_state = s(
  lifecycle, CORE_LIFECYCLE, errors, "fn lifecycle_state(",
  "fn enabled_lifecycle_for_test(",
 )
 o(
  fixture_state, CORE_LIFECYCLE, errors,
  "two owned active indexed nonempty lifecycle verifier records in the execution fixture",
  "verifier_owner_manifest_id(&lifecycle.artifact_binding)",
  "for (id, parity, curve, key_byte) in [", EQ,
  "KagemushaPastaCycleParityV1::StepEq", "0x41", EP,
  "KagemushaPastaCycleParityV1::StepEp", "0x42", "let key_bytes = vec![key_byte; 32]",
  "VerifyingKeyBox::new(", "let commitment = crate::zk::hash_vk(&key)",
  "VerifyingKeyRecord::new_with_owner(", "Some(owner.clone())",
  "record.vk_len = u32::try_from(key.bytes.len())", "record.activation_height = Some(",
  ".checked_add(10)", 'expect("future verifier activation height")', "record.key = Some(key)",
  "record.status = ConfidentialStatus::Active;", VK,
  ".insert((record.circuit_id.clone(), record.version), id.clone())",
  "world.verifying_keys.insert(id.clone(), record);",
 )
 corrupt = s(
  lifecycle, CORE_LIFECYCLE, errors, "enum CancelVerifierCorruption",
  CT,
 )
 q(
  corrupt, CORE_LIFECYCLE, errors,
  "atomic missing/substituted/owner/version/status/index cancellation verifier regressions",
  "MissingRecord", "SubstitutedLifecycleId", "OwnerMismatch", "VersionMismatch",
  "StatusMismatch", "IndexMismatch", "let lifecycle_before =", "let verifiers_before =",
  "let indexes_before =", ".expect_err(\"unqualified cancellation verifier state must fail closed\")",
  LB, ".get(&marker)", ".is_none()", "verifiers_before", "indexes_before",
 )
 corrupt_tests = s(
  lifecycle, CORE_LIFECYCLE, errors, "fn cancellation_rejects_missing_verifier_record_atomically(",
  DX,
 )
 q(
  corrupt_tests, CORE_LIFECYCLE, errors, "cancellation verifier tests",
  "fn cancellation_rejects_missing_verifier_record_atomically()",
  "fn cancellation_rejects_substituted_lifecycle_verifier_id_atomically()",
  "fn cancellation_rejects_verifier_owner_mismatch_atomically()",
  "fn cancellation_rejects_verifier_version_mismatch_atomically()",
  "fn cancellation_rejects_inactive_verifier_atomically()",
  "fn cancellation_rejects_verifier_index_mismatch_atomically()",
 )
 cancel = s(
  lifecycle, CORE_LIFECYCLE, errors,
  CT,
  DX,
 )
 o(
  cancel, CORE_LIFECYCLE, errors,
  "Cancel verifier tombstones",
  "for id in [", EQ, EP,
  'expect("active release verifier fixture")',
  "assert_eq!(verifier.status, ConfidentialStatus::Active);",
  "assert!(verifier.activation_height.is_some_and(|height| height > 3));",
  "assert!(verifier.key.is_some());", "assert!(verifier.vk_len > 0);",
  ".execute(&lifecycle.governance_authority, &mut transaction)",
  "let marker = kagemusha_v2_marker(", RK,
  ".get(&marker)", ".is_some()", "for id in [",
  EQ, EP,
  "assert_eq!(verifier.status, ConfidentialStatus::Withdrawn);",
    "assert_eq!(verifier.activation_height, None);",
    "assert_eq!(verifier.withdraw_height, Some(3));",
    "assert!(verifier.key.is_none());", "assert_eq!(verifier.vk_len, 0);",
    VK, "Some(id)",
    'cancellation must retain the original release verifier index',
 )
 deactivate = s(
  lifecycle, CORE_LIFECYCLE, errors,
  DX,
  "fn terminal_lifecycle_rejects_repeated_and_cross_terminal_transitions(",
 )
 o(
  deactivate, CORE_LIFECYCLE, errors, "Deactivate retains verifiers",
  "let marker = kagemusha_v2_marker(", RK,
  ".get(&marker)", ".is_some()", "for id in [",
  EQ, EP,
  'expect("deactivated release retains its verifier")',
  "assert_eq!(verifier.status, ConfidentialStatus::Active);",
  "assert_eq!(verifier.withdraw_height, None);", "assert!(verifier.key.is_some());",
  "assert!(verifier.vk_len > 0);", VK, "Some(id)",
 )
 terminal = s(
  lifecycle, CORE_LIFECYCLE, errors,
  "fn terminal_lifecycle_rejects_repeated_and_cross_terminal_transitions(",
  "fn redemption_policy_is_available_in_terminal_lifecycle_state(",
 )
 o(
  terminal, CORE_LIFECYCLE, errors,
  "terminal repeat and cross-terminal no-effects rejection",
  "let lifecycle_before =", 'expect_err("cancelled lifecycle cannot be cancelled again")',
  'contains("exact staged state")',
  WS,
  LB, "let cancellation_marker =",
  ".get(&cancellation_marker)", ".is_none()", "let deactivation =",
  'expect_err("cancelled lifecycle cannot be deactivated")', 'contains("only enabled")',
  WS,
  LB, "let deactivation_marker =",
  ".get(&deactivation_marker)", ".is_none()",
 )
 q(lifecycle,CORE_LIFECYCLE,errors,"r","fn sealed_reveal_cannot_gain_direct_external_lifecycle_provenance(")
def lifecycle_source_contract_errors(
 root,overrides=None
):
 errors=[]
 overrides = overrides or {}
 paths = (MODEL, CORE, CORE_ISI_TESTS, *LIFECYCLE_SOURCE_PATHS)
 texts = {path: _read(root, path, overrides, errors) for path in paths}
 provider = overrides.get(PROVIDER, _SOURCE)
 if re.search(
  r'(?m)^if globals\(\)\.get\("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_CONTEXT_V1"\) is not True:$',
  provider,
 ) is None:
  errors.append(f"{PROVIDER}: missing authenticated lifecycle source-provider boundary")
 if len(re.findall(r"(?m)^def lifecycle_source_contract_errors\($", provider)) != 1:
  errors.append(f"{PROVIDER}: lifecycle source-contract evaluator is not sole")
 for relative, text in (*texts.items(), (PROVIDER, provider)):
  if re.search(r"(?m)^(?:<<<<<<<(?: .*)?|=======|>>>>>>>(?: .*)?)$", text):
   errors.append(f"{relative}: unresolved merge-conflict marker")
 _topology(texts, errors)
 _model_contracts(texts, errors)
 _internal_validation_trust_contracts(texts, errors)
 _isi_contracts(texts, errors)
 _carrier_contracts(texts, errors)
 _transition_contracts(texts, errors)
 _redemption_contracts(texts, errors)
 _runtime_projection_contracts(texts, errors)
 _signature_floor_contracts(texts, errors)
 _native_namespace_contracts(texts, errors)
 _native_verifier_hydration_contracts(texts, errors)
 _ordinary_lifecycle_transport_contracts(texts, errors)
 return errors
