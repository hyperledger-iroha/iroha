from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "check_privacy_exact12_sdk_manifest_parity.py"
SPEC = importlib.util.spec_from_file_location("privacy_exact12_sdk_manifest_parity", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _minimal_safe_tree(tmp_path: Path) -> Path:
    exports = "\n".join(
        f'#[unsafe(no_mangle)] pub extern "C" fn {name}() {{}}'
        for name in sorted(MODULE.APPROVED_PRIVACY_EXPORTS)
    )
    _write(
        tmp_path / MODULE.RUST_BRIDGE,
        "mod platform_jni;\n"
        "compiled_privacy_profile_catalog_v1\n"
        "The catalog contains no committed height.\n"
        + exports,
    )
    _write(
        tmp_path / MODULE._RUST_BRIDGE_PLATFORM_JNI,
        "".join(
            f'include!("{path}");\n'
            for path in MODULE._RUST_BRIDGE_PLATFORM_JNI_INCLUDES
        ),
    )
    for path in MODULE._RUST_BRIDGE_PLATFORM_JNI_PARTS:
        _write(tmp_path / path, "// authenticated test bridge part\n")
    declarations = "\n".join(
        f"void {name}(void);" for name in sorted(MODULE.APPROVED_PRIVACY_EXPORTS)
    )
    _write(
        tmp_path / MODULE.C_HEADER,
        "The catalog contains no committed height.\n" + declarations,
    )
    _write(
        tmp_path / "crates/iroha_data_model/src/privacy/capability_manifest.rs",
        " ".join(
            (
                "PrivacyExact12CapabilityManifestV1",
                "manifest_digest",
                "operation_schema",
                "execution_mode",
                "privacy_feature_mask",
                "readiness",
                "activation_state",
                "MissingDistributionWideKnowledgeSoundnessEvidence",
            )
        ),
    )
    _write(
        tmp_path / "crates/iroha_data_model/src/privacy/protocol.rs",
        "validate_privacy_capability_archive_v1",
    )
    _write(
        tmp_path / "crates/iroha_torii/src/runtime.rs",
        "exact12_capability_manifest_v1",
    )
    _write(
        tmp_path / MODULE._RUST_EXACT12_INTEGRATION_LIB,
        "pub mod privacy_exact12_controller;\n",
    )
    _write(
        tmp_path / MODULE._RUST_EXACT12_CONTROLLER,
        "pub fn submit_signed_privacy_action_and_wait_v1(\n"
        "    client: &Client,\n"
        ") -> Result<AuthenticatedPrivacyActionHandleV1> {\n"
        "    let mut handle = client.submit_signed_privacy_action_v1(request)?;\n"
        "    client.get_privacy_action_status_v1(&mut handle)?;\n"
        "    client.get_privacy_action_status_v1(&mut handle)?;\n"
        "    return Ok(handle);\n"
        "}\n"
        "pub fn require_applied_privacy_action_v1() {\n"
        "    PrivacyActionTerminalChainStateV1::Applied;\n"
        "    view.committed_height().is_some();\n"
        "    view.execution_receipt_finalized_height().is_some();\n"
        "}\n"
        "pub fn require_privacy_action_receipt_on_peer_v1() {\n"
        "    FindPrivacyActionExecutionReceiptV1::new(protocol, hash, 0);\n"
        "}\n",
    )
    _write(
        tmp_path / MODULE._RUST_EXACT12_ACTION_DRIVER,
        "//! One-shot, non-networked Exact12 action-construction driver.\n"
        'const QUALIFICATION_SCOPE: &str = "native-action-construction-only";\n'
        'const MISSING_CONTROLLER_CASE_EVIDENCE: &str = '
        '"MissingSealedControllerProtocolCaseEvidence";\n'
        "fn response() {\n"
        "    BuildActionResponseV1 {\n"
        "        network_outcome_authoritative: false,\n"
        "        qualification_scope: QUALIFICATION_SCOPE.to_owned(),\n"
        "    };\n"
        "}\n",
    )
    _write(
        tmp_path / MODULE._RUST_ZK_ACE_LOCALNET,
        "use integration_tests::privacy_exact12_controller::"
        "submit_signed_privacy_action_and_wait_v1;\n"
        "fn execute_zk_ace_network_semantic_flow() {\n"
        "    let canonical_handle = submit_signed_privacy_action_and_wait_v1(\n"
        "        client, PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,\n"
        "        &canonical.transaction, timeout, poll_interval,\n"
        "    );\n"
        "    let replay_handle = submit_signed_privacy_action_and_wait_v1(\n"
        "        client, PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,\n"
        "        &replay.transaction, timeout, poll_interval,\n"
        "    );\n"
        "}\n"
        "#[test]\n"
        "fn pre_activation_and_adversarial_raw_paths_remain_separate() {}\n",
    )
    for path, markers in MODULE._RUST_EXACT12_NETWORK_SEMANTIC_MARKERS:
        destination = tmp_path / path
        existing = destination.read_text(encoding="utf-8") if destination.is_file() else ""
        _write(destination, existing + "\n" + "\n".join(markers) + "\n")
    _write(
        tmp_path / MODULE._PRIVACY_SDK_WORKFLOW,
        'on:\n  pull_request:\n    paths:\n      - "integration_tests/**"\n'
        "  workflow_dispatch:\n"
        "jobs:\n  guard: {}\n",
    )
    return tmp_path


def test_missing_sdk_paths_report_not_ready_but_remain_fail_closed(tmp_path: Path) -> None:
    report = MODULE.audit(_minimal_safe_tree(tmp_path))
    assert report["ready"] is False
    assert report["evidence_level"] == "source-prerequisite-not-native-release-authority"
    assert report["local_catalog_authorizes_network"] is False
    assert report["rust_controller_live_zk_ace_consumer"] is True
    assert report["blockers"] == [contract.name for contract in MODULE.SDK_CONTRACTS]
    for result in report["sdk"].values():
        assert result["gates"]["fail_closed_without_admission"] is True


def test_unapproved_twenty_fifth_privacy_export_is_rejected(tmp_path: Path) -> None:
    root = _minimal_safe_tree(tmp_path)
    bridge = root / MODULE.RUST_BRIDGE
    bridge.write_text(
        bridge.read_text(encoding="utf-8")
        + '\n#[unsafe(no_mangle)] pub extern "C" fn iroha_privacy_capabilities_v1() {}\n',
        encoding="utf-8",
    )
    with pytest.raises(MODULE.AuditError, match="exact approved twenty-four"):
        MODULE.audit(root)


def test_incomplete_rust_bridge_platform_closure_is_rejected(tmp_path: Path) -> None:
    root = _minimal_safe_tree(tmp_path)
    platform_jni = root / MODULE._RUST_BRIDGE_PLATFORM_JNI
    platform_jni.write_text(
        'include!("platform_jni/part_1.rs");\n',
        encoding="utf-8",
    )
    with pytest.raises(MODULE.AuditError, match="exact three-part inventory"):
        MODULE.audit(root)


def test_retained_builder_without_admission_guard_is_rejected(tmp_path: Path) -> None:
    root = _minimal_safe_tree(tmp_path)
    transaction = root / MODULE.SDK_CONTRACTS[0].transaction_files[0]
    _write(transaction, "export function buildZkAmsTransaction() {}\n")
    with pytest.raises(MODULE.AuditError, match="without an Exact12 capability-admission guard"):
        MODULE.audit(root)


def test_retained_builder_with_explicit_admission_stays_fail_closed(tmp_path: Path) -> None:
    root = _minimal_safe_tree(tmp_path)
    transaction = root / MODULE.SDK_CONTRACTS[0].transaction_files[0]
    _write(
        transaction,
        "function requireExact12CapabilityAdmission() {}\n"
        "export function buildZkAmsTransaction() { "
        "requireExact12CapabilityAdmission(); }\n",
    )
    report = MODULE.audit(root)
    assert report["sdk"]["javascript-napi"]["gates"]["fail_closed_without_admission"]
    assert report["ready"] is False


def test_live_rust_controller_is_the_zk_ace_semantic_consumer() -> None:
    assert MODULE._rust_controller_live_zk_ace_consumer_gate(ROOT) is True
    report = MODULE.audit(ROOT)
    assert report["rust_controller_live_zk_ace_consumer"] is True
    assert "rust-controller-live-zk-ace-consumer" not in report["blockers"]


def test_core_action_driver_is_rejected_as_network_release_evidence() -> None:
    assert MODULE._rust_action_driver_network_authority_separation_gate(ROOT) is True
    report = MODULE.audit(ROOT)
    assert report["action_driver_accepted_as_network_evidence"] is False
    assert (
        report["network_execution_authority"]
        == "authenticated-client-controller-terminal-id105-and-typed-native-state"
    )
    assert report["rust_action_driver_network_authority_separation"] is True
    assert "rust-action-driver-network-authority-separation" not in report["blockers"]


@pytest.mark.parametrize(
    ("relative", "needle", "replacement"),
    (
        (
            MODULE._RUST_EXACT12_ACTION_DRIVER,
            "network_outcome_authoritative: false,",
            "network_outcome_authoritative: true,",
        ),
        (
            MODULE._RUST_EXACT12_ACTION_DRIVER,
            'const QUALIFICATION_SCOPE: &str = "native-action-construction-only";',
            'const QUALIFICATION_SCOPE: &str = "network-release-authority";',
        ),
        (
            MODULE._RUST_EXACT12_ACTION_DRIVER,
            "fn run() -> Result<(), String> {",
            "fn run() -> Result<(), String> { client.submit_signed_privacy_action_v1(request);",
        ),
        (
            MODULE._RUST_EXACT12_CONTROLLER,
            "FindPrivacyActionExecutionReceiptV1::new(",
            "FindTransactions::new(",
        ),
        (
            MODULE._RUST_EXACT12_CONTROLLER,
            "view.execution_receipt_finalized_height().is_some()",
            "true",
        ),
    ),
)
def test_action_driver_network_authority_hostile_mutations_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
) -> None:
    paths = (
        MODULE._RUST_EXACT12_ACTION_DRIVER,
        MODULE._RUST_EXACT12_CONTROLLER,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement, 1)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._rust_action_driver_network_authority_separation_gate(ROOT) is False


@pytest.mark.parametrize(
    ("relative", "needle", "replacement"),
    (
        (
            MODULE._RUST_EXACT12_CONTROLLER,
            ".submit_signed_privacy_action_v1(request)",
            ".submit_transaction(request)",
        ),
        (
            MODULE._RUST_EXACT12_CONTROLLER,
            ".get_privacy_action_status_v1(&mut handle)",
            ".get_pipeline_transaction_status(&handle)",
        ),
        (
            MODULE._RUST_EXACT12_CONTROLLER,
            "Result<AuthenticatedPrivacyActionHandleV1>",
            "Result<PrivacyActionOperationViewV1>",
        ),
        (
            MODULE._RUST_ZK_ACE_LOCALNET,
            "let canonical_handle = submit_signed_privacy_action_and_wait_v1(",
            "let canonical_handle = client.submit_transaction(",
        ),
        (
            MODULE._RUST_ZK_ACE_LOCALNET,
            "let replay_handle = submit_signed_privacy_action_and_wait_v1(",
            "let replay_handle = client.submit_transaction(",
        ),
        (
            MODULE._RUST_ZK_ACE_LOCALNET,
            "let canonical_view = canonical_handle.view();",
            "client.submit_transaction(&canonical.transaction)?;\n"
            "    let canonical_view = canonical_handle.view();",
        ),
        (
            MODULE._RUST_ZK_ACE_LOCALNET,
            "let replay_view = replay_handle.view();",
            "client.submit_transaction_blocking(&replay_submit_transaction)?;\n"
            "    let replay_view = replay_handle.view();",
        ),
        (
            MODULE._PRIVACY_SDK_WORKFLOW,
            '      - "integration_tests/**"\n',
            "",
        ),
    ),
)
def test_rust_controller_consumer_hostile_mutations_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
) -> None:
    paths = (
        MODULE._RUST_EXACT12_INTEGRATION_LIB,
        MODULE._RUST_EXACT12_CONTROLLER,
        MODULE._RUST_ZK_ACE_LOCALNET,
        MODULE._PRIVACY_SDK_WORKFLOW,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._rust_controller_live_zk_ace_consumer_gate(ROOT) is False


def test_adversarial_and_pre_activation_raw_submissions_remain_allowed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    paths = (
        MODULE._RUST_EXACT12_INTEGRATION_LIB,
        MODULE._RUST_EXACT12_CONTROLLER,
        MODULE._RUST_ZK_ACE_LOCALNET,
        MODULE._PRIVACY_SDK_WORKFLOW,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    sources[MODULE._RUST_ZK_ACE_LOCALNET] = sources[
        MODULE._RUST_ZK_ACE_LOCALNET
    ].replace(
        "    let canonical_handle = submit_signed_privacy_action_and_wait_v1(",
        "    client.submit_transaction(&adversarial_transaction)?;\n"
        "    let canonical_handle = submit_signed_privacy_action_and_wait_v1(",
    )
    sources[MODULE._RUST_ZK_ACE_LOCALNET] += (
        "\nfn pre_activation_raw_submission(client: &Client, transaction: &SignedTransaction) {\n"
        "    client.submit_transaction(transaction);\n"
        "}\n"
    )
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._rust_controller_live_zk_ace_consumer_gate(ROOT) is True


def test_rust_controller_consumer_failure_blocks_release_report(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    original_read = MODULE._read

    def mutated_read(root: Path, relative: str) -> str:
        source = original_read(root, relative)
        if relative == MODULE._RUST_EXACT12_CONTROLLER:
            return source.replace(
                ".submit_signed_privacy_action_v1(request)",
                ".submit_transaction(request)",
            )
        return source

    monkeypatch.setattr(MODULE, "_read", mutated_read)
    report = MODULE.audit(ROOT)
    assert report["rust_controller_live_zk_ace_consumer"] is False
    assert report["ready"] is False
    assert "rust-controller-live-zk-ace-consumer" in report["blockers"]


def test_live_jvm_cutover_satisfies_strict_source_contract() -> None:
    gates = MODULE._jvm_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
        "authenticated_exact12_action_flow": True,
        "authenticated_finalized_state_queries": True,
    }


def test_live_jvm_manifest_report_advertises_authenticated_action_and_state() -> None:
    jvm = MODULE.audit(ROOT)["sdk"]["jvm-android"]
    assert jvm["ready"] is True
    assert jvm["gates"]["authenticated_exact12_action_flow"] is True
    assert jvm["gates"]["authenticated_finalized_state_queries"] is True
    assert "authenticated_exact12_action_flow" not in jvm["blockers"]
    assert "authenticated_finalized_state_queries" not in jvm["blockers"]


def test_live_javascript_cutover_uses_only_authenticated_native_authority() -> None:
    gates = MODULE._javascript_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
        "authenticated_native_authority": True,
        "browser_fail_closed": True,
        "authenticated_exact12_action_flow": True,
    }


def test_live_csharp_action_flow_requires_authenticated_committed_evidence() -> None:
    assert MODULE._csharp_action_flow_gate(ROOT) is True


def test_live_python_pyo3_admission_is_included_in_source_parity() -> None:
    gates = MODULE._python_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
        "authenticated_exact12_action_flow": True,
        "authenticated_finalized_state_queries": True,
    }
    report = MODULE.audit(ROOT)
    python = report["sdk"]["python-pyo3"]
    assert python["ready"] is True
    assert python["gates"]["authenticated_exact12_action_flow"] is True
    assert python["gates"]["authenticated_finalized_state_queries"] is True


@pytest.mark.parametrize(
    ("relative", "needle", "replacement", "failed_gate"),
    (
        (
            MODULE._JAVASCRIPT_CAPABILITIES,
            "native = getNativeBinding();",
            "native = globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();",
            "authenticated_native_authority",
        ),
        (
            MODULE._JAVASCRIPT_CAPABILITIES,
            "native = getNativeBinding();",
            "native = fakeNativeBinding ?? getNativeBinding();",
            "authenticated_native_authority",
        ),
        (
            MODULE._JAVASCRIPT_NATIVE_BROWSER,
            'throw nativeBindingError("iroha_js_host is unavailable in browser builds.");',
            "return globalThis.__IROHA_NATIVE_BINDING__;",
            "browser_fail_closed",
        ),
        (
            MODULE._JAVASCRIPT_PACKAGE,
            '"./dist/native.js": "./dist/native.browser.js"',
            '"./dist/native.js": "./dist/native.js"',
            "browser_fail_closed",
        ),
        (
            MODULE._JAVASCRIPT_CAPABILITIES,
            "admitted !== true",
            "false",
            "exact_native_local_tuple_match",
        ),
    ),
)
def test_javascript_native_authority_regressions_fail_source_parity(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
    failed_gate: str,
) -> None:
    paths = (
        MODULE._JAVASCRIPT_CAPABILITIES,
        MODULE._JAVASCRIPT_NATIVE,
        MODULE._JAVASCRIPT_NATIVE_BROWSER,
        MODULE._JAVASCRIPT_PACKAGE,
        MODULE._JAVASCRIPT_TRANSACTION,
        MODULE._JAVASCRIPT_TEST,
        MODULE._JAVASCRIPT_ACTION_MODELS,
        MODULE._JAVASCRIPT_TORII,
        MODULE._JAVASCRIPT_ACTION_NATIVE,
        MODULE._JAVASCRIPT_DETAILS_NATIVE,
        MODULE._JAVASCRIPT_ACTION_TEST,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._javascript_cutover_gates(ROOT)[failed_gate] is False


@pytest.mark.parametrize(
    ("relative", "needle", "replacement", "failed_gate"),
    (
        (
            MODULE._PYTHON_RUST_MANIFEST,
            "if !row.is_network_available()",
            "if false",
            "exact_native_local_tuple_match",
        ),
        (
            MODULE._PYTHON_RUST_BRIDGE,
            "manifest.require_network_profile(protocol_id)?",
            "drop(manifest);",
            "transaction_admission_guard",
        ),
        (
            MODULE._PYTHON_CRYPTO,
            "manifest = decoder(canonical)",
            "manifest = object()",
            "native_canonical_manifest_validation",
        ),
        (
            MODULE._PYTHON_CLIENT,
            'terminal_kind in {"Queued", "Approved", "Committed"}',
            'terminal_kind in {"Queued", "Approved"}',
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_CLIENT,
            'receipt["admitted_at_height"] != authenticated_height',
            'receipt["admitted_at_height"] == authenticated_height',
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_CLIENT,
            '"submit_signed_privacy_action_v1.local_signing_context"',
            '"submit_signed_privacy_action_v1"',
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_RUST_BRIDGE,
            ")) => rejection.detail.clone(),",
            ")) => reason.to_string(),",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_RUST_BRIDGE,
            "while let Some(current) = source {",
            "if let Some(current) = source {",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_RUST_BRIDGE,
            "message.chars().any(char::is_control)",
            "false",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_RUST_BRIDGE,
            "pipeline_transaction_details_rejection_projection_rejects_noncanonical_text",
            "pipeline_transaction_details_rejection_projection_accepts_noncanonical_text",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._PYTHON_CRYPTO,
            "_crypto.build_privacy_finalized_state_query_with_signer(",
            "_crypto.build_unbound_privacy_state_query(",
            "authenticated_finalized_state_queries",
        ),
        (
            MODULE._PYTHON_STATE_TEST,
            "test_only_404_is_a_not_found_result",
            "test_any_error_is_a_not_found_result",
            "authenticated_finalized_state_queries",
        ),
    ),
)
def test_python_pyo3_admission_regressions_fail_source_parity(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
    failed_gate: str,
) -> None:
    paths = (
        MODULE._PYTHON_CRYPTO,
        MODULE._PYTHON_CLIENT,
        MODULE._PYTHON_TRANSACTION,
        MODULE._PYTHON_RUST_MANIFEST,
        MODULE._PYTHON_RUST_BRIDGE,
        MODULE._PYTHON_ACTION_TEST,
        MODULE._PYTHON_STATE_TEST,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._python_cutover_gates(ROOT)[failed_gate] is False


def test_live_swift_cutover_satisfies_strict_source_contract() -> None:
    gates = MODULE._swift_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
        "authenticated_exact12_action_flow": True,
        "authenticated_finalized_state_queries": True,
    }
    swift = MODULE.audit(ROOT)["sdk"]["swift"]
    assert swift["ready"] is True
    assert swift["gates"]["authenticated_exact12_action_flow"] is True
    assert swift["gates"]["authenticated_finalized_state_queries"] is True


@pytest.mark.parametrize(
    ("relative", "needle", "replacement", "failed_gate"),
    (
        (
            MODULE._SWIFT_MODEL,
            "maxStatementAndEncryptedOutputBytesPerTransaction",
            "droppedConsensusField",
            "canonical_manifest_model",
        ),
        (
            MODULE._SWIFT_MODEL,
            "assuranceExperimental",
            "droppedActivationAssurance",
            "canonical_manifest_model",
        ),
        (
            MODULE._SWIFT_BRIDGE,
            "localCatalog = try compiledProfileCatalogV1()",
            "localCatalog = Data()",
            "native_canonical_manifest_validation",
        ),
        (
            MODULE._SWIFT_BRIDGE,
            "return try requireCompiledProfileCatalogV1(archive)",
            "return Data(archive)",
            "native_canonical_manifest_validation",
        ),
        (
            MODULE._SWIFT_MODEL,
            "guard compiledBytes == localCompiledProfile",
            "guard !compiledBytes.isEmpty",
            "exact_native_local_tuple_match",
        ),
        (
            MODULE._SWIFT_MODEL,
            "private static let authenticSeal",
            "public static let authenticSeal",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_TRANSACTION,
            "TransactionInstructionFrame: Equatable, Sendable",
            "TransactionInstructionFrame: Equatable, Codable, Sendable",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_TRANSACTION,
            "wireName != PrivacyExact12FixtureCodecV1.submitProofWireId",
            "!wireName.isEmpty",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_TRANSACTION,
            "PrivacyExact12CapabilityAdmissionV1.requireForConstruction(",
            "acceptWithoutExact12Admission(",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_TORII,
            "_ = try PrivacyNativeBridge.compiledProfileCatalogV1()",
            "// native preflight removed",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_ENCODER,
            "try frame.compactInstructionBoxPayload()",
            "frame.framedPayload",
            "transaction_admission_guard",
        ),
        (
            MODULE._SWIFT_TORII,
            "case .queued, .approved, .committed:",
            "case .queued, .approved:",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._SWIFT_BRIDGE,
            "authenticatedActionReceiptProjectResultV1(",
            "uncheckedActionReceiptProjectResultV1(",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._SWIFT_STATE_MODEL,
            "case zkX509CertificateNullifier = 104",
            "case zkX509CertificateNullifier = 105",
            "authenticated_finalized_state_queries",
        ),
        (
            MODULE._SWIFT_STATE_TEST,
            "testProjectionRejectsNoncanonicalHashAndNumericLeaves",
            "testProjectionAcceptsNoncanonicalHashAndNumericLeaves",
            "authenticated_finalized_state_queries",
        ),
    ),
)
def test_swift_cutover_hostile_source_regressions_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
    failed_gate: str,
) -> None:
    paths = (
        MODULE._SWIFT_MODEL,
        MODULE._SWIFT_BRIDGE,
        MODULE._SWIFT_NATIVE,
        MODULE._SWIFT_TRANSACTION,
        MODULE._SWIFT_ENCODER,
        MODULE._SWIFT_TORII,
        MODULE._SWIFT_TEST,
        MODULE._SWIFT_ACTION_MODEL,
        MODULE._SWIFT_STATE_MODEL,
        MODULE._SWIFT_ACTION_TEST,
        MODULE._SWIFT_STATE_TEST,
    )
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._swift_cutover_gates(ROOT)[failed_gate] is False


@pytest.mark.parametrize(
    ("relative", "needle", "replacement", "failed_gate"),
    (
        (
            MODULE._RUST_BRIDGE_PLATFORM_JNI_PARTS[1],
            "committed.compiled_profile == local.compiled_profile",
            "committed.compiled_profile != local.compiled_profile",
            "exact_native_local_tuple_match",
        ),
        (
            MODULE._RUST_BRIDGE_PLATFORM_JNI_PARTS[1],
            "validate_privacy_capability_archive_v1(archive)",
            "accept_unchecked_privacy_capability_archive_v1(archive)",
            "native_canonical_manifest_validation",
        ),
        (
            MODULE._JVM_MODEL,
            "canonicalArchive.copyOf()",
            "canonicalArchive",
            "canonical_manifest_model",
        ),
        (
            MODULE._JVM_MODEL,
            "require(row.isNetworkAvailable())",
            "check(true)",
            "transaction_admission_guard",
        ),
        (
            MODULE._JVM_KOTLIN_TRANSPORT,
            "class HttpClientTransport(",
            "// PrivacyCapabilitySnapshotJsonV1\nclass HttpClientTransport(",
            "transaction_admission_guard",
        ),
        (
            MODULE._JVM_KOTLIN_TRANSACTION_ADAPTER,
            "value.requirePrivacyExact12ConstructionAdmission()",
            "Unit",
            "transaction_admission_guard",
        ),
        (
            MODULE._JVM_KOTLIN_RECEIPT_BRIDGE,
            "nativeProjectPrivacyActionReceiptV1(",
            "removedNativeProjectPrivacyActionReceiptV1(",
            "authenticated_exact12_action_flow",
        ),
        (
            MODULE._JVM_KOTLIN_STATE_BRIDGE,
            "nativeProjectPrivacyStateQueryV1(",
            "removedNativeProjectPrivacyStateQueryV1(",
            "authenticated_finalized_state_queries",
        ),
        (
            MODULE._JVM_CI,
            "check_privacy_finalized_state_jvm_parity.py",
            "removed_privacy_finalized_state_jvm_parity.py",
            "authenticated_finalized_state_queries",
        ),
        (
            MODULE._JVM_KOTLIN_TRANSPORT,
            "if (response.statusCode == 404) {",
            "if (response.statusCode == 404 || response.statusCode == 204) {",
            "authenticated_finalized_state_queries",
        ),
        (
            MODULE._JVM_TORII_QUERY_ROUTING,
            "SingularQueryBox::FindPrivacyZkX509CertificateNullifierV1(_)",
            "SingularQueryBox::RemovedPrivacyZkX509CertificateNullifierV1(_)",
            "authenticated_finalized_state_queries",
        ),
    ),
)
def test_jvm_cutover_hostile_source_regressions_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    needle: str,
    replacement: str,
    failed_gate: str,
) -> None:
    sources = {
        path: (ROOT / path).read_text(encoding="utf-8")
        for path in (
            MODULE._JVM_MODEL,
            MODULE._JVM_KOTLIN_BRIDGE,
            MODULE._JVM_JAVA_BRIDGE,
            MODULE._JVM_KOTLIN_TRANSPORT,
            MODULE._JVM_JAVA_TRANSPORT,
            MODULE._JVM_KOTLIN_INSTRUCTION,
            MODULE._JVM_KOTLIN_TRANSACTION_ADAPTER,
            MODULE._JVM_JAVA_INSTRUCTION,
            MODULE._JVM_JAVA_TRANSACTION_ADAPTER,
            *MODULE._JVM_AUTHENTICATED_SOURCE_FILES,
            *MODULE._RUST_BRIDGE_SOURCE_FILES,
        )
    }
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._jvm_cutover_gates(ROOT)[failed_gate] is False
