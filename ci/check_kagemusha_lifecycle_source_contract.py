"""Static source contract for the Kagemusha V4 release lifecycle."""

import re
from pathlib import Path

PROVIDER = "ci/check_kagemusha_lifecycle_source_contract.py"
MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_KAGEMUSHA_MODEL = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_KAGEMUSHA_MODEL_INCLUDE = 'include!("kagemusha_model.rs");'
MODEL_RELEASE_V5 = "crates/iroha_data_model/src/offline/kagemusha_release_v5.rs"
MODEL_RELEASE_V5_INCLUDE = 'include!("kagemusha_release_v5.rs");'
MODEL_LIFECYCLE = "crates/iroha_data_model/src/offline/kagemusha_release_lifecycle.rs"
MODEL_LIFECYCLE_MODULE = "mod kagemusha_release_lifecycle;"
MODEL_TAIL_TESTS = "crates/iroha_data_model/src/offline/kagemusha_v4_release_tail_inline_tests.rs"
MODEL_TAIL_TESTS_INCLUDE = 'include!("kagemusha_v4_release_tail_inline_tests.rs");'
MODEL_PROMOTION_RECEIPT_TESTS = (
    "crates/iroha_data_model/src/offline/kagemusha_promotion_receipt_inline_tests.rs"
)
MODEL_PROMOTION_RECEIPT_TESTS_INCLUDE = 'include!("kagemusha_promotion_receipt_inline_tests.rs");'
MODEL_PROMOTION_RECEIPT = "crates/iroha_data_model/src/offline/kagemusha_promotion_receipt.rs"
MODEL_ISI = "crates/iroha_data_model/src/isi/offline.rs"
MODEL_ISI_REGISTRY = "crates/iroha_data_model/src/isi/mod.rs"
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
CORE_ACTIVATION = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_activation.rs"
CORE_LIFECYCLE = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_release_lifecycle.rs"
CORE_LIFECYCLE_MODULE = "mod kagemusha_release_lifecycle;"
CORE_RUNTIME_CONFIG = (
    "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_runtime_effective_config.rs"
)
CORE_REDEMPTION_POLICY = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "kagemusha_redemption_policy_validation.rs"
)
CORE_REDEMPTION_POLICY_INCLUDE = (
    'include!("offline/kagemusha_redemption_policy_validation.rs");'
)
CORE_ISI_TESTS = "crates/iroha_core/src/smartcontracts/isi/offline/isi_tests.rs"
CORE_REDEMPTION_POLICY_TESTS = (
    "crates/iroha_core/src/smartcontracts/isi/offline/"
    "isi_kagemusha_redemption_policy_tests.rs"
)
CORE_REDEMPTION_POLICY_TESTS_INCLUDE = (
    'include!("isi_kagemusha_redemption_policy_tests.rs");'
)
CORE_ISI_REGISTRY = "crates/iroha_core/src/smartcontracts/isi/mod.rs"
CORE_TX = "crates/iroha_core/src/tx.rs"
CORE_TX_AUTHORITY_ADMISSION = "crates/iroha_core/src/tx/authority_admission.rs"
CORE_TX_AUTHORITY_ADMISSION_MODULE = "mod authority_admission;"
CORE_TX_LIFECYCLE_TESTS = "crates/iroha_core/src/tx/kagemusha_lifecycle_admission_tests.rs"
CORE_TX_LIFECYCLE_TESTS_INCLUDE = 'include!("tx/kagemusha_lifecycle_admission_tests.rs");'
CORE_STATE = "crates/iroha_core/src/state.rs"
CORE_STATE_RUNTIME_CONFIG = "crates/iroha_core/src/state/runtime_configuration.rs"
CORE_STATE_RUNTIME_CONFIG_INCLUDE = 'include!("state/runtime_configuration.rs");'
CORE_STATE_TESTS = "crates/iroha_core/src/state/tests.rs"
CORE_STATE_RUNTIME_CONFIG_TESTS = (
    "crates/iroha_core/src/state/tests/kagemusha_runtime_effective_config_tests.rs"
)
CORE_STATE_RUNTIME_CONFIG_TESTS_INCLUDE = (
    'include!("tests/kagemusha_runtime_effective_config_tests.rs");'
)
CORE_COMMITTED_CONTEXT = "crates/iroha_core/src/state/committed_transaction_context.rs"
CORE_BLOCK = "crates/iroha_core/src/block.rs"
CORE_EXECUTOR = "crates/iroha_core/src/executor.rs"
CORE_WORLD = "crates/iroha_core/src/smartcontracts/isi/world.rs"
CORE_IVM_HOST = "crates/iroha_core/src/smartcontracts/ivm/host.rs"
CORE_SUMERAGI_APPLY = "crates/iroha_core/src/sumeragi/v2_apply.rs"
CORE_SUMERAGI_APPLY_TESTS = "crates/iroha_core/src/sumeragi/v2_apply_tests.rs"
CORE_SUMERAGI_APPLY_TESTS_MODULE = '#[path = "v2_apply_tests.rs"]\nmod tests;'
CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS = (
    "crates/iroha_core/src/sumeragi/tests/v2_apply_kagemusha_runtime_gate.rs"
)
CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS_INCLUDE = (
    'include!("tests/v2_apply_kagemusha_runtime_gate.rs");'
)
CORE_SUMERAGI_WORKER = "crates/iroha_core/src/sumeragi/v2_worker_completion.rs"
CORE_SUMERAGI_WORKER_ROOT = "crates/iroha_core/src/sumeragi/v2_worker.rs"
CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS = (
    "crates/iroha_core/src/sumeragi/tests/v2_worker_kagemusha_runtime_gate.rs"
)
CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS_INCLUDE = (
    'include!("tests/v2_worker_kagemusha_runtime_gate.rs");'
)
CORE_SUMERAGI_RUNNER = "crates/iroha_core/src/sumeragi/v2_runner.rs"
CORE_SUMERAGI_PENDING_KURA = (
    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
)
IROHAD = "crates/irohad/src/main.rs"
IROHAD_STARTUP = "crates/irohad/src/main/kagemusha_startup.rs"
IROHAD_STARTUP_MODULE = '#[path = "main/kagemusha_startup.rs"]\nmod kagemusha_startup;'
IROHAD_STARTUP_TESTS = (
    "crates/irohad/src/main/kagemusha_runtime_effective_config_projection_tests.rs"
)
IROHAD_STARTUP_TESTS_INCLUDE = (
    'include!("main/kagemusha_runtime_effective_config_projection_tests.rs");'
)
IROHAD_VALIDATOR_SEAL_READER = (
    "crates/irohad/src/main/kagemusha_validator_qualification_command.rs"
)
LIFECYCLE_SOURCE_PATHS = (
    MODEL_KAGEMUSHA_MODEL,
    MODEL_RELEASE_V5,
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

if globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_CONTEXT_V1") is not True:
    raise RuntimeError("lifecycle source-contract provider must run inside the authenticated gate")
_SOURCE = globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_SOURCE_V1")
if not isinstance(_SOURCE, str) or not _SOURCE:
    raise RuntimeError("lifecycle source-contract provider requires its exact loaded bytes")


def _read(
    root: Path, relative: str, overrides: dict[str, str], errors: list[str]
) -> str:
    if relative in overrides:
        return overrides[relative]
    try:
        return (root / relative).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"{relative}: could not read lifecycle source input: {error}")
        return ""


def _require(
    text: str, relative: str, errors: list[str], label: str, *needles: str
) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {label}: {needle!r}")


def _forbid(
    text: str, relative: str, errors: list[str], label: str, *needles: str
) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: forbidden {label}: {needle!r}")


def _count(
    text: str,
    relative: str,
    errors: list[str],
    needle: str,
    expected: int,
    label: str,
) -> None:
    observed = text.count(needle)
    if observed != expected:
        errors.append(
            f"{relative}: {label} count for {needle!r} is {observed}, expected {expected}"
        )


def _section(
    text: str,
    relative: str,
    errors: list[str],
    start: str,
    end: str | None = None,
) -> str:
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
    text: str, relative: str, errors: list[str], label: str, *needles: str
) -> None:
    offsets: list[int] = []
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


def _topology(texts: dict[str, str], errors: list[str]) -> None:
    for parent, relative, marker, component in (
        (texts[MODEL], MODEL, MODEL_KAGEMUSHA_MODEL_INCLUDE, MODEL_KAGEMUSHA_MODEL),
        (texts[MODEL], MODEL, MODEL_RELEASE_V5_INCLUDE, MODEL_RELEASE_V5),
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
        _count(
            parent,
            relative,
            errors,
            marker,
            1,
            f"exactly one authenticated {Path(component).name} attachment",
        )
    _require(
        texts[MODEL],
        MODEL,
        errors,
        "public lifecycle model export",
        "kagemusha_release_lifecycle::*",
    )


def _model_contracts(texts: dict[str, str], errors: list[str]) -> None:
    lifecycle = texts[MODEL_LIFECYCLE]
    _require(
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
    _require(
        texts[MODEL_TAIL_TESTS],
        MODEL_TAIL_TESTS,
        errors,
        "active lifecycle transition and retained-policy regressions",
        "fn release_lifecycle_state_enforces_exact_predecessors_and_terminal_phases()",
        "the retained redemption policy must match its signed promotion identity",
        "deactivation must retain the exact policy required for full redemption",
        "deactivation cannot name the staged state instead of the exact enabled predecessor",
    )
    _require(
        texts[MODEL_PROMOTION_RECEIPT_TESTS],
        MODEL_PROMOTION_RECEIPT_TESTS,
        errors,
        "promotion identity and exact reservation-generation regressions",
        "fn github_promotion_id_derivation_matches_known_vector()",
        "fn validator_seals_reject_mixed_exact_reservation_generations()",
    )
    _require(
        lifecycle,
        MODEL_LIFECYCLE,
        errors,
        "active bounded lifecycle decoder regressions",
        "fn lifecycle_state_key_is_exact_and_rejects_zero_manifest_digest()",
        "fn terminal_transitions_are_bounded_canonical_and_reason_closed()",
        "fn bounded_decoders_reject_empty_and_trailing_input()",
    )
    _forbid(
        lifecycle + texts[MODEL_TAIL_TESTS] + texts[MODEL_PROMOTION_RECEIPT_TESTS],
        "lifecycle model regressions",
        errors,
        "disabled lifecycle regression",
        "#[ignore]",
        "#[cfg(any())]",
    )


def _internal_validation_trust_contracts(
    texts: dict[str, str], errors: list[str]
) -> None:
    model = _section(
        texts[MODEL_KAGEMUSHA_MODEL],
        MODEL_KAGEMUSHA_MODEL,
        errors,
        "pub struct KagemushaRecursiveSpendReleasePolicyV1 {",
        "/// Verified signer identity retained",
    )
    _require(
        model,
        MODEL_KAGEMUSHA_MODEL,
        errors,
        "policy-owned internal-validation runner trust root",
        "pub internal_validation_runner_identity_sha256: [u8; 32]",
        "Domain-separated identity of the only internal-validation runner authorized by policy",
    )
    release = texts[MODEL_RELEASE_V5]
    policy_validation = _section(
        release,
        MODEL_RELEASE_V5,
        errors,
        "impl KagemushaRecursiveSpendReleasePolicyV1 {",
        "impl KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {",
    )
    _require(
        policy_validation,
        MODEL_RELEASE_V5,
        errors,
        "nonzero internal-validation runner trust root",
        "self.internal_validation_runner_identity_sha256 == [0; 32]",
        "KagemushaReleaseVerificationError::InvalidPolicy",
    )
    receipt_validation = _section(
        release,
        MODEL_RELEASE_V5,
        errors,
        "fn validate_internal_validation_receipt_v4(",
        "impl KagemushaAuthenticatedReleaseV4 {",
    )
    _ordered(
        receipt_validation,
        MODEL_RELEASE_V5,
        errors,
        "receipt runner identity against the policy trust root",
        "expected_runner_identity_sha256: Option<[u8; 32]>",
        "let body = &receipt.body",
        "expected_runner_identity_sha256",
        ".is_some_and(|expected| body.validation_runner_identity_sha256 != expected)",
        "KagemushaReleaseVerificationError::InvalidInternalValidationReceipt",
    )
    authenticated_v4 = _section(
        release,
        MODEL_RELEASE_V5,
        errors,
        "impl KagemushaAuthenticatedReleaseV4 {",
        "impl KagemushaAuthenticatedReleaseV5 {",
    )
    _ordered(
        authenticated_v4,
        MODEL_RELEASE_V5,
        errors,
        "authenticated V4 internal-validation trust-root forwarding",
        "validate_internal_validation_receipt_v4(",
        "internal_validation_receipt",
        "Some(policy.internal_validation_runner_identity_sha256)",
    )
    _require(
        texts[MODEL],
        MODEL,
        errors,
        "hostile runner-root release regressions",
        "wrong_runner_policy.internal_validation_runner_identity_sha256[0] ^= 1",
        "a valid self-declared runner signature is not an authorization root",
        "unpinned_runner_policy.internal_validation_runner_identity_sha256 = [0; 32]",
        "an authenticated V4 release policy must name a runner trust root",
    )


def _native_namespace_contracts(texts: dict[str, str], errors: list[str]) -> None:
    host = texts[CORE_IVM_HOST]
    read_only = _section(
        host,
        CORE_IVM_HOST,
        errors,
        "const READ_ONLY_SYSTEM_CONTRACT_STATE_PREFIXES: &[&str] = &[",
        "];",
    )
    _count(
        read_only,
        CORE_IVM_HOST,
        errors,
        '"kagemusha",',
        1,
        "exact delimiter-aware native Kagemusha namespace root",
    )
    _forbid(
        read_only,
        CORE_IVM_HOST,
        errors,
        "narrow or delimiter-bearing Kagemusha namespace root",
        '"kagemusha_",',
        '"kagemusha_online_registration_",',
    )
    classifier = _section(
        host,
        CORE_IVM_HOST,
        errors,
        "fn contract_state_key_matches_namespace(",
        "fn scoped_durable_state_path(",
    )
    _require(
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
    classifier_test = _section(
        host,
        CORE_IVM_HOST,
        errors,
        "fn contract_state_namespace_access_covers_consensus_owned_prefixes()",
        "fn state_syscalls_cannot_forge_delete_or_disclose_queue_plan_admission_marker()",
    )
    _require(
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


def _isi_contracts(texts: dict[str, str], errors: list[str]) -> None:
    model_isi = texts[MODEL_ISI]
    _require(
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
        _require(
            texts[MODEL_ISI_REGISTRY],
            MODEL_ISI_REGISTRY,
            errors,
            "direct lifecycle instruction boxing",
            f"impl_direct_instruction_box!(crate::isi::offline::{instruction});",
        )
        _require(
            texts[CORE_ISI_REGISTRY],
            CORE_ISI_REGISTRY,
            errors,
            "direct lifecycle executor dispatch",
            f"dispatch_instruction::<iroha_data_model::isi::offline::{instruction}>",
        )


def _carrier_contracts(texts: dict[str, str], errors: list[str]) -> None:
    lifecycle = texts[CORE_LIFECYCLE]
    _require(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "affine direct lifecycle transaction carrier",
        "transaction.admission_intent() != TransactionAdmissionIntent::Ordinary",
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
    _require(
        tx,
        CORE_TX,
        errors,
        "narrow verified-multisig lifecycle admission wiring",
        "pub(crate) use authority_admission::{",
        "instructions_allow_direct_kagemusha_lifecycle_authority,",
        "let allows_direct_kagemusha_lifecycle_authority = tx.multisig_signatures().is_some()",
        "&& !allows_direct_kagemusha_lifecycle_authority",
    )
    _require(
        texts[CORE_TX_AUTHORITY_ADMISSION],
        CORE_TX_AUTHORITY_ADMISSION,
        errors,
        "one-exact-instruction lifecycle authority classifier",
        "pub(crate) fn instructions_allow_direct_kagemusha_lifecycle_authority(",
        "let [instruction] = instructions else",
        "direct_lifecycle_entrypoint_kind(instruction).is_some()",
    )
    _require(
        texts[CORE_TX_LIFECYCLE_TESTS],
        CORE_TX_LIFECYCLE_TESTS,
        errors,
        "narrow verified-multisig lifecycle admission regressions",
        "fn direct_kagemusha_lifecycle_authority_requires_one_exact_instruction()",
        "fn exact_kagemusha_lifecycle_accepts_verified_multisig_authority_at_stateful_admission()",
    )
    state = texts[CORE_STATE]
    _require(
        state,
        CORE_STATE,
        errors,
        "one-shot lifecycle state carrier",
        "pub(crate) kagemusha_release_lifecycle_entrypoint: Option<LifecycleEntrypointContext>",
        "kagemusha_release_lifecycle_entrypoint: None",
    )
    committed = texts[CORE_COMMITTED_CONTEXT]
    _ordered(
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
    _ordered(
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
    execute = _section(executor, CORE_EXECUTOR, errors, "pub fn execute_transaction(")
    _ordered(
        execute,
        CORE_EXECUTOR,
        errors,
        "executor lifecycle reset and direct-carrier derivation",
        "state_transaction.kagemusha_release_lifecycle_entrypoint = None",
        "if state_transaction.kagemusha_taira_canary_external_entrypoint",
        "signed_lifecycle_entrypoint_context(&transaction)?",
        "state_transaction.current_tx_hash = Some(tx_hash.clone())",
    )
    _require(
        execute,
        CORE_EXECUTOR,
        errors,
        "verified-multisig lifecycle execution exception",
        "let exact_kagemusha_release_lifecycle = state_transaction",
        ".kagemusha_release_lifecycle_entrypoint",
        ".is_some()",
        "&& transaction.multisig_signatures().is_some()",
    )


def _transition_contracts(texts: dict[str, str], errors: list[str]) -> None:
    activation = texts[CORE_ACTIVATION]
    _ordered(
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
    _forbid(
        activation,
        CORE_ACTIVATION,
        errors,
        "premature singleton policy installation during staging",
        "OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY",
    )
    lifecycle = texts[CORE_LIFECYCLE]
    _require(
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
    commit = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "fn commit_transition(",
        "impl Execute for EnableKagemushaRecursiveIssuanceV4",
    )
    _ordered(
        commit,
        CORE_LIFECYCLE,
        errors,
        "final loaded-bytes CAS before lifecycle/replay commit",
        ".get(&loaded.key)",
        "!= Some(&loaded.bytes)",
        "next.validate()",
        ".insert(loaded.key, bytes)",
        ".kagemusha_replay_keys",
        ".insert(marker, ())",
    )
    enable = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "impl Execute for EnableKagemushaRecursiveIssuanceV4",
        "fn withdraw_cancelled_verifiers(",
    )
    _require(
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
    _require(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "authenticated cancel/deactivate transitions",
        "impl Execute for CancelKagemushaRecursiveReleaseV4",
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
    _require(
        texts[CORE],
        CORE,
        errors,
        "lifecycle-gated readiness surface",
        "kagemusha_release_lifecycle::issuance_enabled(world, &lifecycle_binding)?",
        "kagemusha_release_lifecycle::issuance_enabled(world, binding).unwrap_or(false)",
    )


def _redemption_contracts(texts: dict[str, str], errors: list[str]) -> None:
    lifecycle = texts[CORE_LIFECYCLE]
    redemption_policy = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "pub(super) fn redemption_policy(",
        "/// Require the exact promotion to remain staged",
    )
    _require(
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
    _forbid(
        redemption_policy,
        CORE_LIFECYCLE,
        errors,
        "issuance-phase gating of full redemption",
        "issuance_enabled",
        "require_bound_consensus_artifacts",
    )
    policy = texts[CORE_REDEMPTION_POLICY]
    _require(
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
    redeem = _section(
        texts[CORE],
        CORE,
        errors,
        "impl Execute for RedeemKagemushaRecursiveV4",
    )
    _ordered(
        redeem,
        CORE,
        errors,
        "release-policy lookup before redemption authentication and replay",
        "kagemusha_release_lifecycle::redemption_policy(",
        "authenticate_kagemusha_v4_redeem_submission_before_replay(",
        "let replay_markers = match replay_status",
    )
    tests = texts[CORE_REDEMPTION_POLICY_TESTS]
    _require(
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
    _forbid(
        tests,
        CORE_REDEMPTION_POLICY_TESTS,
        errors,
        "disabled redemption policy regression",
        "#[ignore]",
        "#[cfg",
    )
    _require(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "terminal redemption policy regression",
        "fn redemption_policy_is_available_in_terminal_lifecycle_state()",
    )


def _runtime_projection_contracts(texts: dict[str, str], errors: list[str]) -> None:
    model_isi = texts[MODEL_ISI]
    _require(
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
    _ordered(
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
    _require(
        texts[MODEL_LIFECYCLE],
        MODEL_LIFECYCLE,
        errors,
        "persisted nonzero runtime projection identity",
        "pub runtime_effective_config_sha256: [u8; 32]",
        "|| self.runtime_effective_config_sha256 == [0; 32]",
    )
    activation = texts[CORE_ACTIVATION]
    _ordered(
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
    _require(
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
    active_scan = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "fn active_runtime_effective_config_sha256(",
        "/// Fail closed unless every active lifecycle",
    )
    _ordered(
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
    _forbid(
        active_scan,
        CORE_LIFECYCLE,
        errors,
        "whole-WSV active lifecycle scan",
        ".iter()",
    )
    state = texts[CORE_STATE]
    _require(
        state,
        CORE_STATE,
        errors,
        "immutable process-local runtime projection",
        "kagemusha_runtime_effective_config_sha256: SyncOnceCell<[u8; 32]>",
    )
    runtime_state = texts[CORE_STATE_RUNTIME_CONFIG]
    _require(
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
    runtime_install = _section(
        runtime_state,
        CORE_STATE_RUNTIME_CONFIG,
        errors,
        "pub fn install_kagemusha_runtime_effective_config_sha256(",
        "/// Check one committed or prospective world",
    )
    _ordered(
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
    _require(
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
    _require(
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
    _require(
        seal_reader,
        IROHAD_VALIDATOR_SEAL_READER,
        errors,
        "root-owned exact verified local validator seal",
        "pub(super) fn read_configured_kagemusha_validator_qualification_seal(",
        ".kagemusha_validator_qualification_seal_path",
        "RootOwnedNoReplaceArtifactPublicationTarget::read_root_owned_bounded(",
        "KAGEMUSHA_VALIDATOR_QUALIFICATION_SEAL_MAX_BYTES_V1",
        "decode_exact_kagemusha_validator_qualification_seal(&exact)",
    )
    exact_seal = _section(
        seal_reader,
        IROHAD_VALIDATOR_SEAL_READER,
        errors,
        "fn decode_exact_kagemusha_validator_qualification_seal(",
        "/// Prepared root-owned, no-replace destination",
    )
    _require(
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
    _ordered(
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
    _require(
        texts[IROHAD],
        IROHAD,
        errors,
        "startup runtime projection installer call",
        "kagemusha_startup::install_runtime_effective_config(",
    )
    _require(
        node,
        IROHAD_STARTUP,
        errors,
        "injectable exact validator-seal reader seam",
        "pub(super) fn install_runtime_effective_config_with_validator_seal_reader(",
        "read_configured_kagemusha_validator_qualification_seal: impl FnOnce(",
        "kagemusha_validator_qualification_command::read_configured_kagemusha_validator_qualification_seal,",
        "let seal = read_configured_kagemusha_validator_qualification_seal(config)?;",
    )
    startup_tests = texts[IROHAD_STARTUP_TESTS]
    _require(
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
    _forbid(
        startup_tests,
        IROHAD_STARTUP_TESTS,
        errors,
        "disabled authenticated snapshot startup regression",
        "#[ignore]",
    )
    lifecycle = texts[CORE_LIFECYCLE]
    _require(
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
    _ordered(
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
        section = _section(apply, CORE_SUMERAGI_APPLY, errors, start, end)
        _ordered(
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
    _require(
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
    _forbid(
        apply_tests,
        CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS,
        errors,
        "disabled production apply runtime-projection regression",
        "#[ignore]",
    )
    worker = texts[CORE_SUMERAGI_WORKER]
    _count(
        worker,
        CORE_SUMERAGI_WORKER,
        errors,
        ".require_committed_kagemusha_runtime_effective_config(",
        2,
        "ordinary and recovered pre-sign runtime recheck",
    )
    _ordered(
        worker,
        CORE_SUMERAGI_WORKER,
        errors,
        "ordinary and recovered signing runtime checks",
        "V2IoCommand::Sign {",
        ".require_committed_kagemusha_runtime_effective_config(",
        "sign_consensus_task(",
        "V2IoCommand::RecoveredLifecycleSign(task)",
        ".require_committed_kagemusha_runtime_effective_config(",
        "sign_recovered_lifecycle_task(",
    )
    worker_tests = texts[CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS]
    _require(
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
    _forbid(
        worker_tests,
        CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS,
        errors,
        "disabled production signing runtime-projection regression",
        "#[ignore]",
    )
    _ordered(
        texts[CORE_SUMERAGI_RUNNER],
        CORE_SUMERAGI_RUNNER,
        errors,
        "normal replay runtime check after startup reconstruction",
        "if pending_kura_apply.is_none()",
        ".require_committed_kagemusha_runtime_effective_config()",
        "match pending_kura_apply",
    )
    _ordered(
        texts[CORE_SUMERAGI_PENDING_KURA],
        CORE_SUMERAGI_PENDING_KURA,
        errors,
        "pending-tip runtime check after reconstruction",
        '"finished lifecycle-owned interrupted-tip local Apply recovery"',
        ".require_committed_kagemusha_runtime_effective_config()",
        "reconcile_pending_lane_startup(",
    )


def _signature_floor_contracts(texts: dict[str, str], errors: list[str]) -> None:
    receipt = texts[MODEL_PROMOTION_RECEIPT]
    _require(
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
    signer_gate = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "fn require_distinct_governance_signers(",
        "/// Derive lifecycle context only from one ordinary",
    )
    _ordered(
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
    carrier = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "pub(crate) fn signed_lifecycle_entrypoint_context(",
        "struct LoadedLifecycle",
    )
    _ordered(
        carrier,
        CORE_LIFECYCLE,
        errors,
        "signature floor before direct lifecycle context creation",
        "direct_lifecycle_entrypoint_kind(instruction)",
        "transaction.admission_intent() != TransactionAdmissionIntent::Ordinary",
        "require_distinct_governance_signers(transaction, kind)?",
        "Ok(Some(LifecycleEntrypointContext",
    )
    _require(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "all-four-kind distinct-signer regressions",
        "fn lifecycle_state_rejects_a_policy_with_one_threshold_weight_member()",
        "fn every_lifecycle_kind_rejects_one_threshold_weight_signer()",
        "for kind in [\n            LifecycleEntrypointKind::Stage,\n            LifecycleEntrypointKind::Enable,\n            LifecycleEntrypointKind::Cancel,\n            LifecycleEntrypointKind::Deactivate,\n        ]",
        "the direct carrier must apply the distinct-signer gate",
    )
    _require(
        texts[CORE_TX_LIFECYCLE_TESTS],
        CORE_TX_LIFECYCLE_TESTS,
        errors,
        "stateful-admission distinct-signer regression",
        "fn exact_kagemusha_lifecycle_rejects_one_threshold_weight_signer_at_stateful_admission()",
        "the generic weighted threshold accepts member A alone",
        "Kagemusha lifecycle admission requires two distinct signers",
        "requires at least 2 verified distinct governance signers",
    )
    direct_fixture = _section(
        lifecycle,
        CORE_LIFECYCLE,
        errors,
        "fn lifecycle_transaction(",
        "fn lifecycle_state(",
    )
    _ordered(
        direct_fixture,
        CORE_LIFECYCLE,
        errors,
        "direct ordinary multisig lifecycle execution fixture",
        "TransactionBuilder::new(",
        ".with_instructions([instruction])",
        ".with_admission_intent(TransactionAdmissionIntent::Ordinary)",
        ".sign_multisig(keys.iter().map(KeyPair::private_key))",
    )
    _require(
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


def lifecycle_source_contract_errors(
    root: Path, overrides: dict[str, str] | None = None
) -> list[str]:
    """Return lifecycle source-contract violations for the reviewed tree."""
    errors: list[str] = []
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
    return errors
