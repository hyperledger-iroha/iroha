#!/usr/bin/env python3
"""Audit fail-closed Exact12 capability-manifest admission across SDKs.

ABI22 intentionally has exactly five privacy C exports.  Its no-argument
compiled-profile getter can expose only immutable local build metadata; it
cannot manufacture Torii's committed height, lifecycle, or activation state.
Consequently an SDK is release-ready only when it preserves Torii's canonical
manifest bytes, validates them, and compares the selected row's complete
compiled-profile tuple with the native local catalog before constructing a
privacy transaction.

The default mode reports source readiness without weakening the build.  Pass
``--require-ready`` as a prerequisite in a qualification lane to fail until
every SDK has the complete admission path.  This source audit is never native
execution evidence or release authority.  Structural safety violations always
fail, including a sixth ABI export or a retained-protocol builder which lacks
an explicit capability-admission guard.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


APPROVED_PRIVACY_EXPORTS = frozenset(
    {
        "iroha_privacy_compiled_profile_catalog_v1",
        "iroha_privacy_validate_compiled_profile_catalog_v1",
        "iroha_privacy_exact12_fixture_bundle_v1",
        "iroha_privacy_validate_exact12_fixture_bundle_v1",
        "iroha_privacy_free_buffer",
    }
)

RUST_BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
C_HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
_JAVASCRIPT_CAPABILITIES = "javascript/iroha_js/src/privacyCapabilities.js"
_JAVASCRIPT_NATIVE = "javascript/iroha_js/src/native.js"
_JAVASCRIPT_NATIVE_BROWSER = "javascript/iroha_js/src/native.browser.js"
_JAVASCRIPT_PACKAGE = "javascript/iroha_js/package.json"
_JAVASCRIPT_TRANSACTION = "javascript/iroha_js/src/transaction.js"
_JAVASCRIPT_TEST = (
    "javascript/iroha_js/test/privacyExact12CapabilityManifest.test.js"
)
_PYTHON_CRYPTO = "python/iroha_python/src/iroha_python/crypto.py"
_PYTHON_CLIENT = "python/iroha_python/src/iroha_python/client.py"
_PYTHON_TRANSACTION = "python/iroha_python/src/iroha_python/tx.py"
_PYTHON_RUST_MANIFEST = (
    "python/iroha_python/iroha_python_rs/src/privacy_capability_manifest.rs"
)
_PYTHON_RUST_BRIDGE = "python/iroha_python/iroha_python_rs/src/lib.rs"


class AuditError(RuntimeError):
    """The source tree violates a fail-closed release invariant."""


@dataclass(frozen=True)
class SdkContract:
    name: str
    model_files: tuple[str, ...]
    native_files: tuple[str, ...]
    transaction_files: tuple[str, ...]
    manifest_markers: tuple[str, ...]
    native_markers: tuple[str, ...]
    tuple_markers: tuple[str, ...]


SDK_CONTRACTS = (
    SdkContract(
        "javascript-napi",
        (_JAVASCRIPT_CAPABILITIES,),
        (
            _JAVASCRIPT_NATIVE,
            _JAVASCRIPT_CAPABILITIES,
            "crates/iroha_js_host/src/lib.rs",
        ),
        (_JAVASCRIPT_TRANSACTION,),
        (
            "PrivacyExact12CapabilityManifestV1",
            "manifest_digest",
            "operation_schema",
            "execution_mode",
            "privacy_feature_mask",
            "activation_state",
        ),
        (
            "privacyValidateExact12CapabilityManifestV1",
            "validate_privacy_capability_archive_v1",
        ),
        ("requirePrivacyExact12CapabilityTupleV1", "compiledProfileCatalogV1"),
    ),
    SdkContract(
        "python-pyo3",
        (_PYTHON_CRYPTO, _PYTHON_RUST_MANIFEST),
        (_PYTHON_CRYPTO, _PYTHON_RUST_MANIFEST, _PYTHON_RUST_BRIDGE),
        (_PYTHON_CLIENT, _PYTHON_TRANSACTION, _PYTHON_RUST_BRIDGE),
        (
            "PrivacyExact12CapabilityManifestV1",
            "canonical_archive",
            "manifest_digest",
            "operation_schema",
            "execution_mode",
            "privacy_feature_mask",
            "activation_state",
        ),
        (
            "privacy_validate_exact12_capability_manifest_v1",
            "validate_privacy_capability_archive_v1",
        ),
        ("require_network_profile", "compiled_privacy_profile_v1"),
    ),
    SdkContract(
        "jvm-android",
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyCapabilitiesV1.kt",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyExact12CapabilityManifestV1.kt",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            RUST_BRIDGE,
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/InstructionBox.kt",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/tx/norito/TransactionPayloadAdapter.kt",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/InstructionBox.java",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/norito/TransactionPayloadAdapter.java",
        ),
        (
            "PrivacyExact12CapabilityManifestV1",
            "manifestDigest",
            "operationSchema",
            "executionMode",
            "privacyFeatureMask",
            "activationState",
        ),
        (
            "nativeValidateExact12CapabilityManifest",
            "validate_privacy_capability_archive_v1",
        ),
        ("requireExact12CapabilityTupleV1", "compiledProfileCatalogTypedV1"),
    ),
    SdkContract(
        "csharp",
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyExact12CapabilityManifestV1.cs",
        ),
        ("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
        ),
        (
            "PrivacyExact12CapabilityManifestV1",
            "ManifestDigest",
            "OperationSchema",
            "ExecutionMode",
            "PrivacyFeatureMask",
            "ActivationState",
        ),
        ("ValidateExact12CapabilityManifestV1", "ValidateCompiledProfileCatalogV1"),
        ("RequireExact12CapabilityTupleV1", "CompiledProfileCatalogV1"),
    ),
    SdkContract(
        "swift",
        (
            "IrohaSwift/Sources/IrohaSwift/PrivacyExact12CapabilityManifestV1.swift",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        ),
        ("IrohaSwift/Sources/IrohaSwift/TxBuilder.swift",),
        (
            "PrivacyExact12CapabilityManifestV1",
            "manifestDigest",
            "operationSchema",
            "executionMode",
            "privacyFeatureMask",
            "activationState",
        ),
        ("validateExact12CapabilityManifestV1", "validateCompiledProfileCatalogV1"),
        ("requireExact12CapabilityTupleV1", "compiledProfileCatalogV1"),
    ),
)

_RETAINED_BUILDER = re.compile(
    r"\b(?:build|construct|sign|submit)\w*"
    r"(?:Exact12|ZkAce|AnonymousPgc|VeRange|ZkAms|ZkX509|Jindo|"
    r"Bootle|Lantern|Orchard|Fcmp|PrivateNote|PqMasp)\w*\b",
    re.IGNORECASE,
)
_ADMISSION_MARKER = re.compile(r"Exact12Capability(?:Tuple)?Admission", re.IGNORECASE)

_JVM_MODEL = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyExact12CapabilityManifestV1.kt"
)
_JVM_KOTLIN_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"
)
_JVM_JAVA_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/"
    "PrivacyNativeBridge.java"
)
_JVM_KOTLIN_TRANSPORT = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt"
)
_JVM_JAVA_TRANSPORT = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "HttpClientTransport.java"
)
_JVM_KOTLIN_INSTRUCTION = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/InstructionBox.kt"
)
_JVM_KOTLIN_TRANSACTION_ADAPTER = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/tx/norito/"
    "TransactionPayloadAdapter.kt"
)
_JVM_JAVA_INSTRUCTION = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/InstructionBox.java"
)
_JVM_JAVA_TRANSACTION_ADAPTER = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/norito/"
    "TransactionPayloadAdapter.java"
)
_SWIFT_MODEL = "IrohaSwift/Sources/IrohaSwift/PrivacyExact12CapabilityManifestV1.swift"
_SWIFT_BRIDGE = "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"
_SWIFT_NATIVE = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
_SWIFT_TRANSACTION = "IrohaSwift/Sources/IrohaSwift/TxBuilder.swift"
_SWIFT_ENCODER = "IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift"
_SWIFT_TORII = "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift"
_SWIFT_TEST = (
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyExact12CapabilityManifestV1Tests.swift"
)


def _read(root: Path, relative: str) -> str:
    path = root / relative
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ""


def _combined(root: Path, files: Iterable[str]) -> str:
    return "\n".join(_read(root, relative) for relative in files)


def _rust_exports(source: str) -> frozenset[str]:
    return frozenset(
        re.findall(
            r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+(iroha_privacy_[A-Za-z0-9_]+)',
            source,
        )
    )


def _header_exports(source: str) -> frozenset[str]:
    return frozenset(
        re.findall(
            r"\b(iroha_privacy_[A-Za-z0-9_]+)\s*\(",
            re.sub(r"//[^\n]*|/\*.*?\*/", "", source, flags=re.DOTALL),
        )
    )


def _require_exact_abi22(root: Path) -> None:
    rust = _rust_exports(_read(root, RUST_BRIDGE))
    header = _header_exports(_read(root, C_HEADER))
    if rust != APPROVED_PRIVACY_EXPORTS:
        raise AuditError(
            "Rust ABI22 privacy exports differ from the exact approved five: "
            f"found {sorted(rust)}"
        )
    if header != APPROVED_PRIVACY_EXPORTS:
        raise AuditError(
            "C ABI22 privacy declarations differ from the exact approved five: "
            f"found {sorted(header)}"
        )


def _require_authority_boundary(root: Path) -> None:
    bridge = _read(root, RUST_BRIDGE)
    header = _read(root, C_HEADER)
    combined = bridge + "\n" + header
    forbidden = (
        "iroha_privacy_capabilities_v1",
        "iroha_privacy_validate_capabilities_v1",
        "iroha_privacy_exact12_capability_manifest_v1",
    )
    if any(symbol in combined for symbol in forbidden):
        raise AuditError("ABI22 added a sixth capability export")
    if "compiled_privacy_profile_catalog_v1" not in bridge:
        raise AuditError("ABI22 local catalog is no longer derived from native Rust profiles")
    if "contains no committed height" not in combined.lower():
        raise AuditError("ABI22 local catalog lost its explicit non-authority contract")


def _require_rust_manifest_contract(root: Path) -> None:
    model = _read(root, "crates/iroha_data_model/src/privacy/capability_manifest.rs")
    protocol = _read(root, "crates/iroha_data_model/src/privacy/protocol.rs")
    torii = _read(root, "crates/iroha_torii/src/runtime.rs")
    required = (
        "PrivacyExact12CapabilityManifestV1",
        "manifest_digest",
        "operation_schema",
        "execution_mode",
        "privacy_feature_mask",
        "readiness",
        "activation_state",
        "MissingDistributionWideKnowledgeSoundnessEvidence",
    )
    if not all(marker in model for marker in required):
        raise AuditError("Rust canonical Exact12 manifest contract is incomplete")
    if "validate_privacy_capability_archive_v1" not in protocol:
        raise AuditError("Rust canonical Exact12 manifest archive validator is absent")
    if "exact12_capability_manifest_v1" not in torii:
        raise AuditError("Torii does not project committed state into the Exact12 manifest")


def _javascript_cutover_gates(root: Path) -> dict[str, bool]:
    """Require Exact12 to use only the authenticated N-API loader."""

    capabilities = _read(root, _JAVASCRIPT_CAPABILITIES)
    native = _read(root, _JAVASCRIPT_NATIVE)
    native_browser = _read(root, _JAVASCRIPT_NATIVE_BROWSER)
    package = _read(root, _JAVASCRIPT_PACKAGE)
    transaction = _read(root, _JAVASCRIPT_TRANSACTION)
    tests = _read(root, _JAVASCRIPT_TEST)
    authority_start = capabilities.find("function requirePrivacyExact12NativeV1()")
    authority_end = capabilities.find(
        "function callPrivacyExact12NativeV1(", authority_start
    )
    authority = (
        capabilities[authority_start:authority_end]
        if authority_start >= 0 and authority_end > authority_start
        else ""
    )
    browser_start = native_browser.find("export function getNativeBinding()")
    browser_end = native_browser.find(
        "/**\n * Native binding verification", browser_start
    )
    browser_loader = (
        native_browser[browser_start:browser_end]
        if browser_start >= 0 and browser_end > browser_start
        else ""
    )

    canonical_model = all(
        marker in capabilities
        for marker in (
            "class PrivacyExact12CapabilityManifestV1",
            "PRIVACY_EXACT12_MANIFEST_CONSTRUCTOR",
            "privacyExact12ManifestState",
            "canonicalArchive: Uint8Array.from(canonicalArchive)",
            "manifest_digest",
            "operation_schema",
            "execution_mode",
            "privacy_feature_mask",
            "activation_state",
            "missing-distribution-wide-knowledge-soundness-evidence",
        )
    )
    authenticated_native_authority = all(
        (
            'import { getNativeBinding } from "./native.js";' in capabilities,
            "native = getNativeBinding();" in authority,
            authority.count("getNativeBinding()") == 1,
            authority.count("native =") == 1,
            authority.count("return native;") == 1,
            "??" not in authority,
            "globalThis" not in authority,
            "__IROHA_NATIVE_BINDING__" not in capabilities,
            "verifyNativeBindingInternal(" in native,
            "assertLoadableSourceProvenance(" in native,
            "materializeVerifiedSnapshot(" in native,
            "cachedBinding = require(snapshot.path)" in native,
        )
    )
    native_validation = authenticated_native_authority and all(
        marker in capabilities + "\n" + native
        for marker in (
            "privacyValidateExact12CapabilityManifestV1",
            "privacyExact12CapabilityManifestJsonV1",
            "privacyRequireExact12CapabilityTupleV1",
            "requires exact ABI22",
        )
    )
    exact_tuple_match = all(
        marker in capabilities
        for marker in (
            "row.activation_state.activation_state !== \"active\"",
            "row.compiled_profile.status !== \"available\"",
            "compiledProfileCatalogFromNativeV1(native)",
            '"privacyRequireExact12CapabilityTupleV1"',
            "admitted !== true",
        )
    )
    transaction_admission = all(
        (
            "bindPrivacyExact12CapabilityAdmissionV1(" in capabilities,
            "admitPrivacyExact12CapabilityTupleV1(this, protocolId)" in capabilities,
            "privacyExact12ManifestState.get(manifest)" in capabilities,
            "requirePrivacyExact12CapabilityAdmissionV1" in transaction,
        )
    )
    browser_fail_closed = all(
        (
            '"./dist/native.js": "./dist/native.browser.js"' in package,
            "export function getNativeBinding()" in browser_loader,
            'throw nativeBindingError("iroha_js_host is unavailable in browser builds.")'
            in browser_loader,
            "return" not in browser_loader,
            "globalThis" not in browser_loader,
            "mutable global bindings cannot authorize Exact12 native admission" in tests,
            "browser Exact12 exports fail closed even when a fake global binding exists"
            in tests,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
        "authenticated_native_authority": authenticated_native_authority,
        "browser_fail_closed": browser_fail_closed,
    }


def _python_cutover_gates(root: Path) -> dict[str, bool]:
    """Include the Python/PyO3 admission path in Exact12 source parity."""

    crypto = _read(root, _PYTHON_CRYPTO)
    client = _read(root, _PYTHON_CLIENT)
    transaction = _read(root, _PYTHON_TRANSACTION)
    manifest = _read(root, _PYTHON_RUST_MANIFEST)
    bridge = _read(root, _PYTHON_RUST_BRIDGE)

    canonical_model = all(
        marker in crypto + "\n" + manifest
        for marker in (
            "PyPrivacyExact12CapabilityManifestV1",
            "canonical_archive",
            "manifest_digest",
            "protocol_tuples",
            "operation_schema",
            "execution_mode",
            "privacy_feature_mask",
            "activation_state",
            "MissingDistributionWideKnowledgeSoundnessEvidence",
        )
    )
    native_validation = all(
        (
            "_crypto = load_crypto_extension()" in crypto,
            "if not _has_privacy_bridge_abi(_crypto):" in crypto,
            "privacy_validate_exact12_capability_manifest_v1(canonical)" in crypto,
            "manifest = decoder(canonical)" in crypto,
            "if bytes(returned) != canonical:" in crypto,
            "validate_privacy_capability_archive_v1(archive)" in manifest,
            "canonical_archive.as_slice() != archive" in manifest,
        )
    )
    exact_tuple_match = all(
        marker in manifest
        for marker in (
            "if !row.is_network_available()",
            "compiled_privacy_profile_v1(protocol_id)",
            "if network_profile != local_snapshot",
            "self.require_network_profile(protocol_id)?",
        )
    )
    transaction_admission = all(
        (
            "manifest must be a native PrivacyExact12CapabilityManifestV1" in transaction,
            "builder.bind_privacy_exact12_capability_manifest_v1(" in transaction,
            "Option<privacy_capability_manifest::PyPrivacyExact12CapabilityManifestV1>"
            in bridge,
            "manifest.require_network_profile(protocol_id)?" in bridge,
            "requires a validated Torii Exact12 capability manifest" in bridge,
            "PyRef<'_, privacy_capability_manifest::PyPrivacyExact12CapabilityManifestV1>"
            in bridge,
            'headers={"Accept": "application/x-norito"}' in client,
            'media_type != "application/x-norito"' in client,
            "privacy_exact12_capability_manifest_v1(response.content)" in client,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
    }


def _jvm_cutover_gates(root: Path) -> dict[str, bool]:
    """Audit the JVM cutover's authority-bearing statements, not documentation markers."""

    model = _read(root, _JVM_MODEL)
    kotlin_bridge = _read(root, _JVM_KOTLIN_BRIDGE)
    java_bridge = _read(root, _JVM_JAVA_BRIDGE)
    rust_bridge = _read(root, RUST_BRIDGE)
    kotlin_transport = _read(root, _JVM_KOTLIN_TRANSPORT)
    java_transport = _read(root, _JVM_JAVA_TRANSPORT)
    kotlin_instruction = _read(root, _JVM_KOTLIN_INSTRUCTION)
    kotlin_adapter = _read(root, _JVM_KOTLIN_TRANSACTION_ADAPTER)
    java_instruction = _read(root, _JVM_JAVA_INSTRUCTION)
    java_adapter = _read(root, _JVM_JAVA_TRANSACTION_ADAPTER)
    transports = kotlin_transport + "\n" + java_transport

    canonical_model = all(
        marker in model
        for marker in (
            "class PrivacyExact12CapabilityManifestV1 internal constructor",
            "canonicalArchive.copyOf()",
            "fun canonicalBytes(): ByteArray = archive.copyOf()",
            "protocols.size == expected.size",
            "row.protocolId == expected[index]",
            "PrivacyOperationSchemaV1",
            "PrivacyExecutionModeV1",
            "privacyFeatureMask",
            "compiledProfile",
            "manifestDigest",
            "MISSING_DISTRIBUTION_WIDE_KNOWLEDGE_SOUNDNESS_EVIDENCE",
        )
    )
    native_validation = all(
        (
            "nativeValidateExact12CapabilityManifest" in kotlin_bridge,
            "nativeInspectExact12CapabilityManifest" in kotlin_bridge,
            "check(nativeAvailable)" in kotlin_bridge,
            "nativeValidateExact12CapabilityManifest" in java_bridge,
            "if (!NATIVE_AVAILABLE)" in java_bridge,
            "validate_privacy_capability_archive_v1(archive)" in rust_bridge,
            "PrivacyExact12CapabilityManifestV1>(archive)" in rust_bridge,
            "Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_"
            "nativeValidateExact12CapabilityManifest" in rust_bridge,
            "Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_"
            "nativeValidateExact12CapabilityManifest" in rust_bridge,
        )
    )
    exact_tuple_match = all(
        (
            "committed.protocol_id == local.protocol_id" in rust_bridge,
            "committed.compiled_profile == local.compiled_profile" in rust_bridge,
            '"local_compiled_tuple_matches"' in rust_bridge,
            "require(row.localCompiledTupleMatches)" in model,
            "compiledProfileCatalogTypedV1" in model,
        )
    )
    admission_guard = all(
        (
            "class PrivacyExact12CapabilityTupleAdmissionV1 private constructor" in model,
            "private val SEAL = Any()" in model,
            "require(row.isNetworkAvailable())" in model,
            "fun requireForConstruction(" in model,
            "PrivacyNativeBridge.requireExact12CapabilityTuple(" in model,
            "PrivacyNativeBridge.requireExact12SubmitProofConstruction(" in model,
            "nativeRequireExact12CapabilityTuple" in kotlin_bridge,
            "nativeValidateExact12SubmitProofConstruction" in kotlin_bridge,
            "nativeRequireExact12CapabilityTuple" in java_bridge,
            "nativeValidateExact12SubmitProofConstruction" in java_bridge,
            "fromPrivacyExact12WirePayload" in kotlin_instruction,
            "requirePrivacyExact12ConstructionAdmission" in kotlin_instruction,
            "value.requirePrivacyExact12ConstructionAdmission()" in kotlin_adapter,
            "fromPrivacyExact12WirePayload" in java_instruction,
            "requirePrivacyExact12ConstructionAdmission" in java_instruction,
            "value.requirePrivacyExact12ConstructionAdmission();" in java_adapter,
            "requirePrivacyExact12CapabilityAdmission" in kotlin_transport,
            "requirePrivacyExact12CapabilityAdmission" in java_transport,
            "buildExactNoritoGetRequest(" in kotlin_transport,
            "buildExactNoritoGetRequest(" in java_transport,
            "PrivacyNativeBridge::decodeExact12CapabilityManifestV1" in kotlin_transport,
            "PrivacyNativeBridge::decodeExact12CapabilityManifestV1" in java_transport,
            "application/x-norito" in kotlin_transport,
            "application/x-norito" in java_transport,
            "PrivacyCapabilitySnapshotJsonV1" not in transports,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": admission_guard,
    }


def _swift_cutover_gates(root: Path) -> dict[str, bool]:
    """Audit Swift's managed semantics plus its mandatory ABI22 catalog anchor.

    The fixed five-export C ABI has no Rust manifest validator.  This gate is
    therefore true only when Swift strictly validates the Torii bytes and every
    fetch, admission, construction, and final encode necessarily re-enters the
    native catalog getter and validator before exact tuple comparison.
    """

    model = _read(root, _SWIFT_MODEL)
    bridge = _read(root, _SWIFT_BRIDGE)
    native = _read(root, _SWIFT_NATIVE)
    transaction = _read(root, _SWIFT_TRANSACTION)
    encoder = _read(root, _SWIFT_ENCODER)
    torii = _read(root, _SWIFT_TORII)
    tests = _read(root, _SWIFT_TEST)
    admission_start = model.find(
        "public final class PrivacyExact12CapabilityTupleAdmissionV1"
    )
    admission_end = model.find(
        "/// The sole path from a committed manifest",
        admission_start,
    )
    admission = (
        model[admission_start:admission_end]
        if admission_start >= 0 and admission_end > admission_start
        else ""
    )

    canonical_model = all(
        marker in model
        for marker in (
            "public final class PrivacyExact12CapabilityManifestV1",
            "fileprivate init(",
            "private let archive: Data",
            "public func canonicalBytes() -> Data",
            "PrivacyConsensusPolicyV1",
            "maxActionsPerTransaction",
            "maxActionsPerBlock",
            "maxProofBytesPerAction",
            "maxActionBytes",
            "maxPrivacyBytesPerTransaction",
            "maxPrivacyBytesPerBlock",
            "maxStatementAndEncryptedOutputBytesPerTransaction",
            "maxNullifiersPerAction",
            "maxCommitmentsPerAction",
            "retainedRootCount",
            "pendingTightening",
            "PrivacyProtocolActivationRecordV1",
            "proofSystemId",
            "engineId",
            "parameterId",
            "parameterDigest",
            "verifierDigest",
            "statementSchemaDigest",
            "engineManifestDigest",
            "lifecycle",
            "protocolLimits",
            "pendingProtocolLimitsTightening",
            "assuranceExperimental",
            "canonicalNorito",
            "protocols must contain exactly 12 rows",
            "protocol rows are missing, duplicated, or reordered",
            "manifest digest does not bind the canonical archive",
            "missingDistributionWideKnowledgeSoundnessEvidence",
            "strictFrame(",
        )
    )
    native_backed_validation = all(
        (
            "validateExact12CapabilityManifestV1" in bridge,
            "localCatalog = try compiledProfileCatalogV1()" in bridge,
            "let archive = try NoritoNativeBridge.shared.privacyCompiledProfileCatalogV1()"
            in bridge,
            "return try requireCompiledProfileCatalogV1(archive)" in bridge,
            "privacyCompiledProfileCatalogValidationStatusV1(archive)" in bridge,
            "PrivacyExact12CapabilityManifestCodecV1.decode(" in bridge,
            "privacyCompiledProfileCatalogV1()" in bridge,
            "requiredBridgeABIVersion: UInt32 = 22" in bridge,
            "loadedBridgeAbiVersion == PrivacyNativeBridge.requiredBridgeABIVersion"
            in native,
            "privacyNativeProbeOk" in native,
            all(symbol in native for symbol in APPROVED_PRIVACY_EXPORTS),
        )
    )
    exact_tuple_match = all(
        marker in model
        for marker in (
            "guard compiledBytes == localCompiledProfile",
            "activation proof system differs from the compiled tuple",
            "profile.engineManifestDigest",
            "guard binding == expectedBindings[index]",
            "submit-proof envelope differs from the admitted compiled profile tuple",
            "PrivacyNativeBridge.validateExact12CapabilityManifestV1(",
            "row.localCompiledTupleMatches",
        )
    )
    transaction_admission = all(
        (
            "private init(" in admission,
            "private static let authenticSeal" in admission,
            not re.search(r"\b(?:Codable|Decodable)\b", admission),
            "public static func requireExact12CapabilityTupleV1" in model,
            model.count("PrivacyNativeBridge.validateExact12CapabilityManifestV1(") >= 2,
            re.search(
                r"public struct TransactionInstructionFrame:[^\n]*"
                r"\b(?:Codable|Decodable)\b",
                transaction,
            )
            is None,
            "wireName != PrivacyExact12FixtureCodecV1.submitProofWireId" in transaction,
            "public static func privacyExact12SubmitProof" in transaction,
            "private let privacyAdmission" in transaction,
            "func compactInstructionBoxPayload() throws" in transaction,
            transaction.count(
                "PrivacyExact12CapabilityAdmissionV1.requireForConstruction("
            ) >= 2,
            "try frame.compactInstructionBoxPayload()" in encoder,
            "getPrivacyExact12CapabilityManifestV1(" in torii,
            "canonicalAuth: ToriiCanonicalRequestAuth" in torii,
            'baseURL.scheme?.lowercased() == "https"' in torii,
            'path: "/v1/privacy/capabilities"' in torii,
            "try applyCanonicalAuth(canonicalAuth" in torii,
            "_ = try PrivacyNativeBridge.compiledProfileCatalogV1()" in torii,
            'contentType == "application/x-norito"' in torii,
            "ToriiRejectRedirectTaskDelegate.shared" in torii,
            "validatedSccpContentLength(" in torii,
            "testEveryTruncationAndOneByteSuffixFailClosed" in tests,
            "testGenericInstructionConstructionCannotBypassPrivacyAdmission" in tests,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_backed_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
    }


def _sdk_result(root: Path, contract: SdkContract) -> dict[str, object]:
    model = _combined(root, contract.model_files)
    native = _combined(root, contract.native_files)
    transactions = _combined(root, contract.transaction_files)
    manifest_model = all(marker in model for marker in contract.manifest_markers)
    native_validation = all(marker in native for marker in contract.native_markers)
    tuple_match = all(marker in model + "\n" + native for marker in contract.tuple_markers)
    transaction_admission = bool(_ADMISSION_MARKER.search(transactions))
    extra_gates: dict[str, bool] = {}
    if contract.name == "javascript-napi" and _read(root, _JAVASCRIPT_CAPABILITIES):
        javascript = _javascript_cutover_gates(root)
        manifest_model = manifest_model and javascript["canonical_manifest_model"]
        native_validation = (
            native_validation
            and javascript["native_canonical_manifest_validation"]
        )
        tuple_match = tuple_match and javascript["exact_native_local_tuple_match"]
        transaction_admission = javascript["transaction_admission_guard"]
        extra_gates = {
            "authenticated_native_authority": javascript[
                "authenticated_native_authority"
            ],
            "browser_fail_closed": javascript["browser_fail_closed"],
        }
    if contract.name == "python-pyo3" and _read(root, _PYTHON_RUST_MANIFEST):
        python = _python_cutover_gates(root)
        manifest_model = manifest_model and python["canonical_manifest_model"]
        native_validation = python["native_canonical_manifest_validation"]
        tuple_match = tuple_match and python["exact_native_local_tuple_match"]
        transaction_admission = python["transaction_admission_guard"]
    if contract.name == "jvm-android":
        jvm = _jvm_cutover_gates(root)
        manifest_model = manifest_model and jvm["canonical_manifest_model"]
        native_validation = (
            native_validation and jvm["native_canonical_manifest_validation"]
        )
        tuple_match = tuple_match and jvm["exact_native_local_tuple_match"]
        transaction_admission = (
            transaction_admission and jvm["transaction_admission_guard"]
        )
    if contract.name == "swift":
        swift = _swift_cutover_gates(root)
        manifest_model = manifest_model and swift["canonical_manifest_model"]
        native_validation = (
            native_validation and swift["native_canonical_manifest_validation"]
        )
        tuple_match = tuple_match and swift["exact_native_local_tuple_match"]
        transaction_admission = (
            transaction_admission and swift["transaction_admission_guard"]
        )
    retained_builders = sorted(set(_RETAINED_BUILDER.findall(transactions)))
    fail_closed = not retained_builders or transaction_admission
    if not fail_closed:
        raise AuditError(
            f"{contract.name} exposes a retained-protocol builder without an "
            "Exact12 capability-admission guard"
        )
    gates = {
        "canonical_manifest_model": manifest_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": tuple_match,
        "transaction_admission_guard": transaction_admission,
        "fail_closed_without_admission": fail_closed,
        **extra_gates,
    }
    blockers = [name for name, passed in gates.items() if not passed]
    return {
        "ready": not blockers,
        "gates": gates,
        "blockers": blockers,
    }


def audit(root: Path) -> dict[str, object]:
    root = root.resolve()
    _require_exact_abi22(root)
    _require_authority_boundary(root)
    _require_rust_manifest_contract(root)
    sdks = {contract.name: _sdk_result(root, contract) for contract in SDK_CONTRACTS}
    blockers = [name for name, result in sdks.items() if not result["ready"]]
    return {
        "schema_version": 1,
        "evidence_level": "source-prerequisite-not-native-release-authority",
        "abi22_privacy_exports": sorted(APPROVED_PRIVACY_EXPORTS),
        "authority": "torii-committed-canonical-manifest-bytes",
        "local_catalog_authorizes_network": False,
        "ready": not blockers,
        "sdk": sdks,
        "blockers": blockers,
    }


def _format_human(report: dict[str, object]) -> str:
    lines = [
        "Exact12 cross-SDK capability-manifest parity: "
        + ("READY" if report["ready"] else "NOT READY"),
        "ABI22 privacy exports: exact five",
        "Network authority: Torii committed canonical manifest bytes",
    ]
    sdks = report["sdk"]
    assert isinstance(sdks, dict)
    for name, result in sdks.items():
        assert isinstance(result, dict)
        state = "ready" if result["ready"] else "blocked"
        lines.append(f"- {name}: {state}")
        for blocker in result["blockers"]:
            lines.append(f"  - missing {blocker}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--require-ready", action="store_true")
    args = parser.parse_args(argv)
    try:
        report = audit(args.root)
    except AuditError as error:
        print(f"privacy Exact12 SDK manifest safety violation: {error}", file=sys.stderr)
        return 2
    if args.as_json:
        print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    else:
        print(_format_human(report))
    return 1 if args.require_ready and not report["ready"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
