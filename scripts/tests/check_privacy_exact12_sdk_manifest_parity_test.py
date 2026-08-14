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
    return tmp_path


def test_missing_sdk_paths_report_not_ready_but_remain_fail_closed(tmp_path: Path) -> None:
    report = MODULE.audit(_minimal_safe_tree(tmp_path))
    assert report["ready"] is False
    assert report["evidence_level"] == "source-prerequisite-not-native-release-authority"
    assert report["local_catalog_authorizes_network"] is False
    assert report["blockers"] == [contract.name for contract in MODULE.SDK_CONTRACTS]
    for result in report["sdk"].values():
        assert result["gates"]["fail_closed_without_admission"] is True


def test_sixth_privacy_export_is_rejected(tmp_path: Path) -> None:
    root = _minimal_safe_tree(tmp_path)
    bridge = root / MODULE.RUST_BRIDGE
    bridge.write_text(
        bridge.read_text(encoding="utf-8")
        + '\n#[unsafe(no_mangle)] pub extern "C" fn iroha_privacy_capabilities_v1() {}\n',
        encoding="utf-8",
    )
    with pytest.raises(MODULE.AuditError, match="exact approved five"):
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


def test_live_jvm_cutover_satisfies_strict_source_contract() -> None:
    gates = MODULE._jvm_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
    }


def test_live_javascript_cutover_uses_only_authenticated_native_authority() -> None:
    gates = MODULE._javascript_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
        "authenticated_native_authority": True,
        "browser_fail_closed": True,
    }


def test_live_python_pyo3_admission_is_included_in_source_parity() -> None:
    gates = MODULE._python_cutover_gates(ROOT)
    assert gates == {
        "canonical_manifest_model": True,
        "native_canonical_manifest_validation": True,
        "exact_native_local_tuple_match": True,
        "transaction_admission_guard": True,
    }
    report = MODULE.audit(ROOT)
    assert report["sdk"]["python-pyo3"]["ready"] is True


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
    }


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
            *MODULE._RUST_BRIDGE_SOURCE_FILES,
        )
    }
    assert needle in sources[relative]
    sources[relative] = sources[relative].replace(needle, replacement)
    monkeypatch.setattr(MODULE, "_read", lambda _root, path: sources.get(path, ""))
    assert MODULE._jvm_cutover_gates(ROOT)[failed_gate] is False
