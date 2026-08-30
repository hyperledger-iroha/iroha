#!/usr/bin/env python3
"""Audit fail-closed Exact12 capability-manifest admission across SDKs.

ABI22 intentionally has exactly twenty-four privacy C exports.  Its no-argument
compiled-profile getter can expose only immutable local build metadata, while
the action and transaction-details helpers authenticate caller-selected wire
bytes without manufacturing Torii's committed height, lifecycle, activation
state, or independent block finality.
Consequently an SDK is release-ready only when it preserves Torii's canonical
manifest bytes, validates them, and compares the selected row's complete
compiled-profile tuple with the native local catalog before constructing a
privacy transaction.

The same prerequisite binds the live Rust ZK-ACE semantic consumer to the
opaque authenticated controller handle.  Canonical and replay submissions may
not bypass the controller's signed submit and authenticated status methods.

The default mode reports source readiness without weakening the build.  Pass
``--require-ready`` as a prerequisite in a qualification lane to fail until
every SDK has the complete admission path.  This source audit is never native
execution evidence or release authority.  Structural safety violations always
fail, including an unapproved ABI export or a retained-protocol builder which
lacks an explicit capability-admission guard.
"""

from __future__ import annotations

import argparse
import importlib.util
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
        "iroha_privacy_inspect_signed_exact12_action_v1",
        "iroha_privacy_authenticated_transaction_details_prepare_v1",
        "iroha_privacy_authenticated_transaction_details_finalize_v1",
        "iroha_privacy_authenticated_transaction_details_project_result_v1",
        "iroha_privacy_authenticated_transaction_details_prepare_v2",
        "iroha_privacy_authenticated_transaction_details_finalize_v2",
        "iroha_privacy_authenticated_transaction_details_project_result_v2",
        "iroha_privacy_authenticated_finality_proof_page_bind_v1",
        "iroha_privacy_authenticated_finality_page_verify_v1",
        "iroha_privacy_authenticated_finalized_kagemusha_outcome_project_v1",
        "iroha_privacy_authenticated_finalized_action_rejection_project_v1",
        "iroha_privacy_kagemusha_topup_finality_project_v4",
        "iroha_privacy_authenticated_offline_device_registration_result_project_v1",
        "iroha_privacy_authenticated_action_receipt_prepare_v1",
        "iroha_privacy_authenticated_action_receipt_finalize_v1",
        "iroha_privacy_authenticated_action_receipt_project_result_v1",
        "iroha_privacy_authenticated_state_query_prepare_v1",
        "iroha_privacy_authenticated_state_query_finalize_v1",
        "iroha_privacy_authenticated_state_query_project_result_v1",
        "iroha_privacy_free_buffer",
    }
)

RUST_BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
_RUST_BRIDGE_PLATFORM_JNI = "crates/connect_norito_bridge/src/platform_jni.rs"
_RUST_BRIDGE_PLATFORM_JNI_PARTS = (
    "crates/connect_norito_bridge/src/platform_jni/part_1.rs",
    "crates/connect_norito_bridge/src/platform_jni/part_2.rs",
    "crates/connect_norito_bridge/src/platform_jni/part_3.rs",
)
_RUST_BRIDGE_SOURCE_FILES = (
    RUST_BRIDGE,
    _RUST_BRIDGE_PLATFORM_JNI,
    *_RUST_BRIDGE_PLATFORM_JNI_PARTS,
)
_RUST_BRIDGE_PLATFORM_JNI_INCLUDES = (
    "platform_jni/part_1.rs",
    "platform_jni/part_2.rs",
    "platform_jni/part_3.rs",
)
C_HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
_JAVASCRIPT_CAPABILITIES = "javascript/iroha_js/src/privacyCapabilities.js"
_JAVASCRIPT_NATIVE = "javascript/iroha_js/src/native.js"
_JAVASCRIPT_NATIVE_BROWSER = "javascript/iroha_js/src/native.browser.js"
_JAVASCRIPT_PACKAGE = "javascript/iroha_js/package.json"
_JAVASCRIPT_TRANSACTION = "javascript/iroha_js/src/transaction.js"
_JAVASCRIPT_ACTION_MODELS = "javascript/iroha_js/src/privacyExact12ActionModels.js"
_JAVASCRIPT_TORII = "javascript/iroha_js/src/toriiClient.js"
_JAVASCRIPT_ACTION_NATIVE = "crates/iroha_js_host/src/privacy_exact12_action.rs"
_JAVASCRIPT_DETAILS_NATIVE = (
    "crates/iroha_js_host/src/authenticated_transaction_details.rs"
)
_JAVASCRIPT_RECEIPT_NATIVE = (
    "crates/iroha_js_host/src/authenticated_privacy_action_receipt.rs"
)
_JAVASCRIPT_ACTION_TEST = (
    "javascript/iroha_js/test/privacyExact12ActionFlow.source.test.js"
)
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
_PYTHON_ACTION_TEST = (
    "python/iroha_python/tests/privacy_exact12_action_transport_test.py"
)
_PYTHON_STATE_TEST = (
    "python/iroha_python/tests/privacy_finalized_state_queries_test.py"
)
_RUST_EXACT12_INTEGRATION_LIB = "integration_tests/src/lib.rs"
_RUST_EXACT12_CONTROLLER = "integration_tests/src/privacy_exact12_controller.rs"
_RUST_ZK_ACE_LOCALNET = "integration_tests/tests/zk_ace_localnet.rs"
_RUST_EXACT12_ACTION_DRIVER = "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
_PRIVACY_SDK_WORKFLOW = ".github/workflows/pr_privacy_sdk_guard.yml"
_RUST_EXACT12_NETWORK_SEMANTIC_MARKERS = (
    (
        "integration_tests/tests/zk_ace_localnet.rs",
        (
            "PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1",
            "submit_signed_privacy_action_and_wait_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
            "validate_finalized_replay_provenance(",
            "wait_for_asset_quantities(",
        ),
    ),
    (
        "integration_tests/tests/privacy_exact12_jindo_network.rs",
        (
            "PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1",
            "submit_signed_privacy_action_and_wait_async_v1(",
            "require_applied_privacy_action_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
        ),
    ),
    (
        "integration_tests/tests/privacy_exact12_retained_network.rs",
        (
            "PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1",
            "PrivacyOperationSchemaV1::VeRangeRangeProofV1",
            "PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1",
            "PrivacyOperationSchemaV1::FcmpMembershipPaymentV1",
            "PrivacyOperationSchemaV1::IvmPrivateNoteActionV1",
            "submit_signed_privacy_action_and_wait_async_v1(",
            "require_applied_privacy_action_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
            "FindPrivacyAnonymousPgcPoolStateV1::new(",
            "assert_proof_managed_transition_view(",
            ".state_query.clone()",
        ),
    ),
    (
        "integration_tests/tests/privacy_exact12_orchard_pq_masp_network.rs",
        (
            "PrivacyOperationSchemaV1::OrchardNoteActionV1",
            "PrivacyOperationSchemaV1::PqMaspNoteActionV1",
            "submit_signed_privacy_action_and_wait_async_v1(",
            "require_applied_privacy_action_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
            "assert_orchard_finalized_post_state(",
            "assert_pq_masp_transition_view(",
            ".state_query.clone()",
        ),
    ),
    (
        "integration_tests/tests/privacy_exact12_zk_ams_vega_network.rs",
        (
            "PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1",
            "PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1",
            "PrivacyOperationSchemaV1::VegaCredentialPresentationV1",
            "submit_signed_privacy_action_and_wait_async_v1(",
            "require_applied_privacy_action_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
            "FindPrivacyZkAmsAdmissionV1::new(",
            "FindPrivacyZkAmsProvisionV1::new(",
            "assert_zk_ams_admission_state(",
            "assert_zk_ams_provision_state(",
        ),
    ),
    (
        "integration_tests/tests/privacy_exact12_zk_x509_network.rs",
        (
            "PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1",
            "submit_signed_privacy_action_and_wait_async_v1(",
            "require_applied_privacy_action_v1(",
            "require_privacy_action_receipt_on_peer_v1(",
            "FindPrivacyZkX509CertificateNullifierV1::new(",
            "assert_zk_x509_nullifier_state(",
        ),
    ),
)


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
        (
            _JAVASCRIPT_TRANSACTION,
            _JAVASCRIPT_ACTION_MODELS,
            _JAVASCRIPT_TORII,
        ),
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
            *_RUST_BRIDGE_SOURCE_FILES,
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
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyExact12ActionModelsV1.cs",
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyFinalizedStateModelsV1.cs",
        ),
        ("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs",
            "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.PrivacyExact12Actions.cs",
            "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.PrivacyFinalizedStateQueries.cs",
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
_JVM_ACTION_MODEL = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyExact12ActionModelsV1.kt"
)
_JVM_KOTLIN_RECEIPT_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedPrivacyActionReceiptNativeBridge.kt"
)
_JVM_JAVA_RECEIPT_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyActionReceiptNativeBridge.java"
)
_JVM_KOTLIN_DETAILS_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedTransactionDetailsNativeBridge.kt"
)
_JVM_JAVA_DETAILS_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedTransactionDetailsNativeBridge.java"
)
_JVM_STATE_MODEL = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyFinalizedStateModelsV1.kt"
)
_JVM_KOTLIN_STATE_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedPrivacyStateQueryNativeBridge.kt"
)
_JVM_JAVA_STATE_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyStateQueryNativeBridge.java"
)
_JVM_NATIVE_RECEIPT = (
    "crates/connect_norito_bridge/src/authenticated_privacy_action_receipt.rs"
)
_JVM_NATIVE_STATE = (
    "crates/connect_norito_bridge/src/authenticated_privacy_state_query.rs"
)
_JVM_NATIVE_QUERY_ACCESS = "crates/iroha_core/src/executor.rs"
_JVM_NATIVE_QUERY_MEMORY = (
    "crates/iroha_core/src/smartcontracts/isi/query/ordinary_memory.rs"
)
_JVM_TORII_QUERY_ROUTING = "crates/iroha_torii/src/lib.rs"
_JVM_KOTLIN_ACTION_TEST = (
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyExact12ActionModelsV1Test.kt"
)
_JVM_KOTLIN_STATE_TEST = (
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyFinalizedStateModelsV1Test.kt"
)
_JVM_JAVA_ACTION_TEST = (
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/"
    "PrivacyExact12ActionInspectionV1Tests.java"
)
_JVM_JAVA_STATE_TEST = (
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyStateQueryNativeBridgeTests.java"
)
_JVM_SOURCE_GUARD = "ci/check_privacy_finalized_state_jvm_parity.py"
_JVM_CI = "ci/check_privacy_jvm_sdk.sh"
_JVM_AUTHENTICATED_SOURCE_FILES = (
    _JVM_ACTION_MODEL,
    _JVM_KOTLIN_RECEIPT_BRIDGE,
    _JVM_JAVA_RECEIPT_BRIDGE,
    _JVM_KOTLIN_DETAILS_BRIDGE,
    _JVM_JAVA_DETAILS_BRIDGE,
    _JVM_STATE_MODEL,
    _JVM_KOTLIN_STATE_BRIDGE,
    _JVM_JAVA_STATE_BRIDGE,
    _JVM_NATIVE_RECEIPT,
    _JVM_NATIVE_STATE,
    _JVM_NATIVE_QUERY_ACCESS,
    _JVM_NATIVE_QUERY_MEMORY,
    _JVM_TORII_QUERY_ROUTING,
    _JVM_KOTLIN_ACTION_TEST,
    _JVM_KOTLIN_STATE_TEST,
    _JVM_JAVA_ACTION_TEST,
    _JVM_JAVA_STATE_TEST,
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyFinalizedStateModelsV1.cs",
    _JVM_SOURCE_GUARD,
    _JVM_CI,
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
_SWIFT_ACTION_MODEL = (
    "IrohaSwift/Sources/IrohaSwift/PrivacyExact12ActionModelsV1.swift"
)
_SWIFT_STATE_MODEL = (
    "IrohaSwift/Sources/IrohaSwift/PrivacyFinalizedStateModelsV1.swift"
)
_SWIFT_ACTION_TEST = (
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyExact12ActionModelsV1Tests.swift"
)
_SWIFT_STATE_TEST = (
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyFinalizedStateModelsV1Tests.swift"
)
_CSHARP_ACTION_MODEL = (
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyExact12ActionModelsV1.cs"
)
_CSHARP_NATIVE = "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs"
_CSHARP_TORII = (
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.PrivacyExact12Actions.cs"
)
_CSHARP_ACTION_TEST = (
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiPrivacyExact12ActionFlowTests.cs"
)
_CSHARP_STATE_MODEL = (
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyFinalizedStateModelsV1.cs"
)
_CSHARP_STATE_TORII = (
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/"
    "ToriiClient.PrivacyFinalizedStateQueries.cs"
)
_CSHARP_STATE_TEST = (
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyFinalizedStateModelsV1Tests.cs"
)


def _read(root: Path, relative: str) -> str:
    path = root / relative
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ""


def _combined(root: Path, files: Iterable[str]) -> str:
    return "\n".join(_read(root, relative) for relative in files)


def _markers_are_strictly_ordered(source: str, markers: Iterable[str]) -> bool:
    """Return true only when every marker appears in the requested order."""

    cursor = 0
    for marker in markers:
        index = source.find(marker, cursor)
        if index < 0:
            return False
        cursor = index + len(marker)
    return True


def _read_required_source(root: Path, relative: str) -> str:
    path = root / relative
    if path.is_symlink() or not path.is_file():
        raise AuditError(f"required Rust bridge source is unavailable: {relative}")
    return _read(root, relative)


def _rust_bridge_source(root: Path) -> str:
    """Read the exact split Rust bridge closure after authenticating its includes."""

    bridge = _read_required_source(root, RUST_BRIDGE)
    if len(re.findall(r"^mod platform_jni;$", bridge, flags=re.MULTILINE)) != 1:
        raise AuditError("Rust bridge must own exactly one platform_jni module")
    platform_jni = _read_required_source(root, _RUST_BRIDGE_PLATFORM_JNI)
    observed_includes = tuple(
        re.findall(r'^include!\("([^"]+)"\);$', platform_jni, flags=re.MULTILINE)
    )
    if observed_includes != _RUST_BRIDGE_PLATFORM_JNI_INCLUDES:
        raise AuditError(
            "Rust bridge platform_jni include closure differs from the exact "
            f"three-part inventory: found {observed_includes}"
        )
    parts = tuple(
        _read_required_source(root, path) for path in _RUST_BRIDGE_PLATFORM_JNI_PARTS
    )
    return "\n".join((bridge, platform_jni) + parts)


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
    rust = _rust_exports(_rust_bridge_source(root))
    header = _header_exports(_read(root, C_HEADER))
    if rust != APPROVED_PRIVACY_EXPORTS:
        raise AuditError(
            "Rust ABI22 privacy exports differ from the exact approved twenty-four: "
            f"found {sorted(rust)}"
        )
    if header != APPROVED_PRIVACY_EXPORTS:
        raise AuditError(
            "C ABI22 privacy declarations differ from the exact approved twenty-four: "
            f"found {sorted(header)}"
        )


def _require_authority_boundary(root: Path) -> None:
    bridge = _rust_bridge_source(root)
    header = _read(root, C_HEADER)
    combined = bridge + "\n" + header
    forbidden = (
        "iroha_privacy_capabilities_v1",
        "iroha_privacy_validate_capabilities_v1",
        "iroha_privacy_exact12_capability_manifest_v1",
    )
    if any(symbol in combined for symbol in forbidden):
        raise AuditError("ABI22 added a forbidden capability-authority export")
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


def _rust_controller_live_zk_ace_consumer_gate(root: Path) -> bool:
    """Require the live ZK-ACE semantic flow to keep the opaque Rust controller handle."""

    integration_lib = _read(root, _RUST_EXACT12_INTEGRATION_LIB)
    controller = _read(root, _RUST_EXACT12_CONTROLLER)
    localnet = _read(root, _RUST_ZK_ACE_LOCALNET)
    workflow = _read(root, _PRIVACY_SDK_WORKFLOW)
    controller_code = re.sub(r"//[^\n]*|/\*.*?\*/", "", controller, flags=re.DOTALL)
    localnet_code = re.sub(r"//[^\n]*|/\*.*?\*/", "", localnet, flags=re.DOTALL)

    helper_start = controller_code.find("pub fn submit_signed_privacy_action_and_wait_v1(")
    helper_end = controller_code.find("\n}", helper_start)
    helper = (
        controller_code[helper_start : helper_end + 2]
        if helper_start >= 0 and helper_end > helper_start
        else ""
    )
    helper_compact = re.sub(r"\s+", "", helper)

    live_flow_start = localnet_code.find("fn execute_zk_ace_network_semantic_flow(")
    live_flow_end = localnet_code.find("\n#[test]", live_flow_start)
    live_flow = (
        localnet_code[live_flow_start:live_flow_end]
        if live_flow_start >= 0 and live_flow_end > live_flow_start
        else ""
    )
    live_flow_compact = re.sub(r"\s+", "", live_flow)
    import_prefix_compact = re.sub(r"\s+", "", localnet_code[:live_flow_start])
    controller_imported = (
        "privacy_exact12_controller::submit_signed_privacy_action_and_wait_v1"
        in import_prefix_compact
        or re.search(
            r"privacy_exact12_controller::\{[^}]*"
            r"submit_signed_privacy_action_and_wait_v1(?:,|\})",
            import_prefix_compact,
        )
        is not None
    )
    controller_call = "submit_signed_privacy_action_and_wait_v1("
    canonical_call = (
        "letcanonical_handle=submit_signed_privacy_action_and_wait_v1("
        "client,PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,"
        "&canonical.transaction,"
    )
    replay_call = (
        "letreplay_handle=submit_signed_privacy_action_and_wait_v1("
        "client,PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,"
        "&replay.transaction,"
    )
    direct_canonical_or_replay_submission = re.search(
        r"\.(?:submit_transaction|submit_transaction_blocking|submit_transaction_async)\("
        r"[^)]*(?:canonical|replay)(?:\.|[A-Za-z0-9_]*)transaction\b",
        live_flow_compact,
    )
    workflow_trigger_start = workflow.find("\n    paths:")
    workflow_trigger_end = workflow.find(
        "\n  workflow_dispatch:", workflow_trigger_start
    )
    workflow_trigger = (
        workflow[workflow_trigger_start:workflow_trigger_end]
        if workflow_trigger_start >= 0
        and workflow_trigger_end > workflow_trigger_start
        else ""
    )

    return all(
        (
            "pub mod privacy_exact12_controller;" in integration_lib,
            "Result<AuthenticatedPrivacyActionHandleV1>" in helper_compact,
            ".submit_signed_privacy_action_v1(request)" in helper_compact,
            helper_compact.count(".get_privacy_action_status_v1(&muthandle)") >= 2,
            "returnOk(handle);" in helper_compact,
            controller_imported,
            live_flow_compact.count(controller_call) == 2,
            canonical_call in live_flow_compact,
            replay_call in live_flow_compact,
            direct_canonical_or_replay_submission is None,
            len(
                re.findall(
                    r'^\s+- ["\']integration_tests/\*\*["\']\s*$',
                    workflow_trigger,
                    flags=re.MULTILINE,
                )
            )
            == 1,
        )
    )


def _rust_action_driver_network_authority_separation_gate(root: Path) -> bool:
    """Keep the core action builder explicitly outside network release authority."""

    driver = _read(root, _RUST_EXACT12_ACTION_DRIVER)
    controller = _read(root, _RUST_EXACT12_CONTROLLER)
    driver_code = re.sub(r"//[^\n]*|/\*.*?\*/", "", driver, flags=re.DOTALL)
    network_semantics = all(
        all(marker in _read(root, path) for marker in markers)
        for path, markers in _RUST_EXACT12_NETWORK_SEMANTIC_MARKERS
    )
    return all(
        (
            "One-shot, non-networked Exact12 action-construction driver." in driver,
            'const QUALIFICATION_SCOPE: &str = "native-action-construction-only";'
            in driver_code,
            'const MISSING_CONTROLLER_CASE_EVIDENCE: &str = '
            '"MissingSealedControllerProtocolCaseEvidence";'
            in driver_code,
            "network_outcome_authoritative: false," in driver_code,
            "qualification_scope: QUALIFICATION_SCOPE.to_owned()," in driver_code,
            "network_outcome_authoritative: true" not in driver_code,
            "AuthenticatedPrivacyActionHandleV1" not in driver_code,
            ".submit_signed_privacy_action_v1(" not in driver_code,
            ".get_privacy_action_status_v1(" not in driver_code,
            "FindPrivacyActionExecutionReceiptV1" not in driver_code,
            "pub fn require_applied_privacy_action_v1(" in controller,
            "pub fn require_privacy_action_receipt_on_peer_v1(" in controller,
            "FindPrivacyActionExecutionReceiptV1::new(" in controller,
            "PrivacyActionTerminalChainStateV1::Applied" in controller,
            "view.committed_height().is_some()" in controller,
            "view.execution_receipt_finalized_height().is_some()" in controller,
            network_semantics,
        )
    )


def _javascript_cutover_gates(root: Path) -> dict[str, bool]:
    """Require Exact12 to use only the authenticated N-API loader."""

    capabilities = _read(root, _JAVASCRIPT_CAPABILITIES)
    native = _read(root, _JAVASCRIPT_NATIVE)
    native_browser = _read(root, _JAVASCRIPT_NATIVE_BROWSER)
    package = _read(root, _JAVASCRIPT_PACKAGE)
    transaction = _read(root, _JAVASCRIPT_TRANSACTION)
    tests = _read(root, _JAVASCRIPT_TEST)
    action_models = _read(root, _JAVASCRIPT_ACTION_MODELS)
    torii = _read(root, _JAVASCRIPT_TORII)
    action_native = _read(root, _JAVASCRIPT_ACTION_NATIVE)
    details_native = _read(root, _JAVASCRIPT_DETAILS_NATIVE)
    receipt_native = _read(root, _JAVASCRIPT_RECEIPT_NATIVE)
    action_test = _read(root, _JAVASCRIPT_ACTION_TEST)
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
    submit_start = torii.find("async submitSignedPrivacyActionV1(")
    submit_end = torii.find("async getPrivacyActionStatusV1(", submit_start)
    submit = (
        torii[submit_start:submit_end]
        if submit_start >= 0 and submit_end > submit_start
        else ""
    )
    submit_order = tuple(
        submit.find(marker)
        for marker in (
            "inspectSignedPrivacyActionNativeV1(",
            "getPrivacyExact12CapabilityManifestV1(",
            "requirePrivacyExact12CapabilityTupleV1(",
            "_submitSignedPrivacyActionWireV1(",
        )
    )
    authenticated_action_flow = all(
        (
            len(re.findall(r'^  "[a-z0-9_]+",$', action_models, re.MULTILINE))
            >= 13,
            "class PrivacyExact12ActionRequestV1" in action_models,
            "class PrivacyActionOperationViewV1" in action_models,
            all(index >= 0 for index in submit_order),
            submit_order == tuple(sorted(submit_order)),
            "timingSafeEqual" in submit,
            "privacyInspectSignedExact12ActionV1" in torii,
            "privacyBuildFindPrivacyActionExecutionReceiptQueryV1" in torii,
            "privacyInspectPrivacyActionExecutionReceiptResponseV1" in torii,
            "privacyBuildFindCommittedTransactionQueryV1" in torii,
            "privacyInspectPipelineTransactionDetailsV1" in torii,
            '"/v1/pipeline/transactions"' in torii,
            '"/v1/pipeline/transactions/details"' in torii,
            '"/v1/query"' in torii,
            "details.resultOk" in torii,
            "details.rejectionMessage" in torii,
            "receipt.admittedAtHeight !== details.committedHeight" in torii,
            "details === null || receipt === null" in torii,
            'kind === "Queued" || kind === "Approved" || kind === "Committed"'
            in torii,
            'resolution.resolvedFrom === "cache"' in torii,
            "executionCapabilityManifestDigest" in action_models,
            "executionReceiptFinalizedBlockHash" in action_models,
            "exactCommittedHeight < capabilityCommittedHeight" in action_models,
            "verify_signature()" in action_native,
            "canonical_authority(authority_literal)" in action_native,
            "signed.authority() != &expected_authority" in action_native,
            "validate_zk_x509_credential_proof_container_v1" in action_native,
            "verify_privacy_proof" not in action_native,
            "decode_canonical_with_limits" in details_native,
            "independent block finality" in details_native,
            "FindPrivacyActionExecutionReceiptV1" in receipt_native,
            "PrivacyActionExecutionReceiptViewV1" in receipt_native,
            "norito::decode_canonical_with_limits" in receipt_native,
            "norito::canonical_decode_limits(response.len())" in receipt_native,
            "canonical != response" in receipt_native,
            "receipt.transaction_intent_digest.as_bytes()" in receipt_native,
            "receipt.statement_digest.as_bytes()" in receipt_native,
            "receipt.proof_envelope_hash != expected_binding.proof_envelope_hash"
            in receipt_native,
            "receipt\n        .validate()" in receipt_native,
            "verify_privacy_proof" not in receipt_native,
            "terminal status requires committed result and finalized native receipt"
            in action_test,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
        "authenticated_native_authority": authenticated_native_authority,
        "browser_fail_closed": browser_fail_closed,
        "authenticated_exact12_action_flow": authenticated_action_flow,
    }


def _python_cutover_gates(root: Path) -> dict[str, bool]:
    """Include the Python/PyO3 admission path in Exact12 source parity."""

    crypto = _read(root, _PYTHON_CRYPTO)
    client = _read(root, _PYTHON_CLIENT)
    transaction = _read(root, _PYTHON_TRANSACTION)
    manifest = _read(root, _PYTHON_RUST_MANIFEST)
    bridge = _read(root, _PYTHON_RUST_BRIDGE)
    action_tests = _read(root, _PYTHON_ACTION_TEST)
    state_tests = _read(root, _PYTHON_STATE_TEST)

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
    operations_start = client.find("PRIVACY_EXACT12_ACTION_OPERATIONS_V1:")
    operations_end = client.find(
        "_PRIVACY_EXACT12_ACTION_OPERATION_INDEX_V1",
        operations_start,
    )
    operations = (
        client[operations_start:operations_end]
        if operations_start >= 0 and operations_end > operations_start
        else ""
    )
    submit_start = client.find("    def submit_signed_privacy_action_v1(")
    submit_end = client.find("    def get_privacy_action_status_v1(", submit_start)
    submit = (
        client[submit_start:submit_end]
        if submit_start >= 0 and submit_end > submit_start
        else ""
    )
    resolve_start = client.find("    def _resolve_privacy_action_status_v1(")
    resolve_end = client.find(
        "    def _wait_for_privacy_action_terminal_status_v1(",
        resolve_start,
    )
    resolve = (
        client[resolve_start:resolve_end]
        if resolve_start >= 0 and resolve_end > resolve_start
        else ""
    )
    receipt_start = client.find("    def get_privacy_action_execution_receipt_v1(")
    receipt_end = client.find(
        "    def _stabilize_privacy_action_terminal_view_v1(",
        receipt_start,
    )
    receipt = (
        client[receipt_start:receipt_end]
        if receipt_start >= 0 and receipt_end > receipt_start
        else ""
    )
    rejection_projection_start = bridge.find(
        "fn validated_pipeline_transaction_rejection_message_v1("
    )
    rejection_projection_end = bridge.find(
        "const PIPELINE_TRANSACTION_DETAILS_RESPONSE_MAX_BYTES",
        rejection_projection_start,
    )
    rejection_projection = (
        bridge[rejection_projection_start:rejection_projection_end]
        if rejection_projection_start >= 0
        and rejection_projection_end > rejection_projection_start
        else ""
    )
    authenticated_action_flow = all(
        (
            len(
                re.findall(
                    r'^\s{4}"[a-z0-9_]+",$',
                    operations,
                    flags=re.MULTILINE,
                )
            )
            == 13,
            "class PrivacyExact12ActionRequestV1" in client,
            "class PrivacyActionOperationViewV1" in client,
            _markers_are_strictly_ordered(
                submit,
                (
                    "inspector = getattr(crypto, spec.inspector, None)",
                    "envelope = crypto.signed_transaction_envelope_from_versioned_v1(",
                    "manifest = self.privacy_capabilities_v1(",
                    "capability = manifest.require_network_capability(spec.protocol_id)",
                    "self._submit_signed_privacy_action_wire_v1(",
                ),
            ),
            "hmac.compare_digest(" in submit,
            '"submit_signed_privacy_action_v1.local_signing_context"' in submit,
            "signed privacy transaction authority differs from canonical_auth account"
            in submit,
            '"/v1/pipeline/transactions"' in client,
            '"/v1/pipeline/transactions/details"' in client,
            '"/v1/query"' in receipt,
            "build_find_privacy_action_execution_receipt_query_with_signer(" in receipt,
            "inspect_privacy_action_execution_receipt_response(" in receipt,
            "if response.status_code == 404:" in receipt,
            "response.status_code == 204" not in receipt,
            'terminal_kind in {"Queued", "Approved", "Committed"}' in resolve,
            'terminal_kind not in {"Applied", "Rejected"}' in resolve,
            "get_pipeline_transaction_details_with_canonical_auth(" in resolve,
            "receipt = self.get_privacy_action_execution_receipt_v1(" in resolve,
            'receipt["admitted_at_height"] != authenticated_height' in resolve,
            'details.get("rejection_message")' in resolve,
            "InstructionExecutionError::OfflineDeviceEligibility(rejection)"
            in rejection_projection,
            "rejection.detail.clone()" in rejection_projection,
            "let mut source = reason.source();" in rejection_projection,
            "while let Some(current) = source" in rejection_projection,
            "message = current.to_string();" in rejection_projection,
            "PIPELINE_TRANSACTION_DETAILS_REJECTION_MESSAGE_MAX_BYTES_V1"
            in rejection_projection,
            "message.trim() != message" in rejection_projection,
            "message.chars().any(char::is_control)" in rejection_projection,
            '"terminal_chain_state": "Applied"' in resolve,
            '"terminal_chain_state": "Rejected"' in resolve,
            "test_every_exact12_operation_authenticates_gates_and_submits_once"
            in action_tests,
            "test_wait_path_requires_applied_details_and_finalized_receipt"
            in action_tests,
            "test_rejected_wait_fetches_authenticated_committed_reason"
            in action_tests,
            "test_id105_receipt_query_treats_only_404_as_retryable_absence"
            in action_tests,
            "pipeline_transaction_details_rejection_projection_matches_abi22" in bridge,
            "pipeline_transaction_details_rejection_projection_rejects_noncanonical_text"
            in bridge,
        )
    )

    state_ids_start = client.find(
        "_PRIVACY_FINALIZED_STATE_QUERY_SCHEMA_BY_ID_V1"
    )
    state_ids_end = client.find(
        "_PRIVACY_PROOF_MANAGED_QUERY_PROTOCOL_INDEX_V1",
        state_ids_start,
    )
    state_ids = (
        client[state_ids_start:state_ids_end]
        if state_ids_start >= 0 and state_ids_end > state_ids_start
        else ""
    )
    state_query_start = client.find("    def _get_privacy_finalized_state_v1(")
    state_query_end = client.find(
        "    def get_privacy_zk_ace_replay_nullifier_v1(",
        state_query_start,
    )
    state_query = (
        client[state_query_start:state_query_end]
        if state_query_start >= 0 and state_query_end > state_query_start
        else ""
    )
    state_methods = (
        "get_privacy_zk_ace_replay_nullifier_v1",
        "get_privacy_proof_managed_pool_state_v1",
        "get_privacy_orchard_pool_state_v1",
        "get_privacy_orchard_nullifier_v1",
        "get_privacy_anonymous_pgc_pool_state_v1",
        "get_privacy_zk_ams_admission_v1",
        "get_privacy_zk_ams_provision_v1",
        "get_privacy_zk_x509_certificate_nullifier_v1",
    )
    authenticated_finalized_state_queries = all(
        (
            tuple(
                int(query_id)
                for query_id in re.findall(
                    r"^\s{8}(97|98|99|100|101|102|103|104):",
                    state_ids,
                    flags=re.MULTILINE,
                )
            )
            == tuple(range(97, 105)),
            all(f"    def {method}(" in client for method in state_methods),
            "class PrivacyFinalizedStateViewV1" in client,
            'if "network_id" not in self.projection:' in client,
            'self.projection.get("finalized_height")' in client,
            'self.projection.get("finalized_block_hash")' in client,
            "build_privacy_finalized_state_query_with_signer(" in state_query,
            "inspect_privacy_finalized_state_query_response(" in state_query,
            '"POST",\n            "/v1/query"' in state_query,
            "allow_retry=False" in state_query,
            "allow_redirects=False" in state_query,
            "if response.status_code == 404:" in state_query,
            "response.status_code == 204" not in state_query,
            'content_type.strip().lower() != "application/x-norito"' in state_query,
            "_crypto.build_privacy_finalized_state_query_with_signer(" in crypto,
            "_crypto.inspect_privacy_finalized_state_query_response(" in crypto,
            "set(projection) != _PRIVACY_FINALIZED_STATE_PROJECTION_FIELDS_V1[query_id]"
            in crypto,
            "test_all_stable_state_queries_use_native_signer_and_exact_binding"
            in state_tests,
            "test_only_404_is_a_not_found_result" in state_tests,
            "test_python_native_state_query_boundary_is_sealed_to_ids_97_through_104"
            in state_tests,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
        "authenticated_exact12_action_flow": authenticated_action_flow,
        "authenticated_finalized_state_queries": authenticated_finalized_state_queries,
    }


def _jvm_authenticated_source_gates(root: Path) -> dict[str, bool]:
    """Run the focused JVM source guard through this checker's source reader.

    Passing ``_read`` through to the guard keeps the focused CI prerequisite and
    this authoritative manifest report on one contract. It also lets hostile
    source regressions prove that a removed binding cannot remain advertised.
    """

    gate_names = (
        "authenticated_exact12_action_flow",
        "authenticated_finalized_state_queries",
    )
    failed = {name: False for name in gate_names}
    guard_path = root / _JVM_SOURCE_GUARD
    guard_source = _read(root, _JVM_SOURCE_GUARD)
    if not guard_path.is_file() or not all(
        marker in guard_source
        for marker in (
            "def audit(",
            "ACTION_GATE",
            "STATE_GATE",
            "_audit_action_flow",
            "_audit_finalized_state",
        )
    ):
        return failed
    try:
        spec = importlib.util.spec_from_file_location(
            "_iroha_privacy_jvm_authenticated_source_guard",
            guard_path,
        )
        if spec is None or spec.loader is None:
            return failed
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        result = module.audit(
            root,
            reader=lambda relative: _read(root, relative),
        )
    except Exception:
        # A missing dependency or malformed focused guard is a failed source
        # prerequisite, never permission to advertise either capability.
        return failed
    if not isinstance(result, dict):
        return failed
    return {
        name: isinstance(result.get(name), (tuple, list))
        and len(result[name]) == 0
        for name in gate_names
    }


def _jvm_cutover_gates(root: Path) -> dict[str, bool]:
    """Audit the JVM cutover's authority-bearing statements, not documentation markers."""

    model = _read(root, _JVM_MODEL)
    kotlin_bridge = _read(root, _JVM_KOTLIN_BRIDGE)
    java_bridge = _read(root, _JVM_JAVA_BRIDGE)
    rust_bridge = _rust_bridge_source(root)
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
        **_jvm_authenticated_source_gates(root),
    }


def _swift_cutover_gates(root: Path) -> dict[str, bool]:
    """Audit Swift's managed semantics plus its mandatory ABI22 catalog anchor.

    The fixed twenty-four-export C ABI has no Rust manifest validator.  This gate is
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
    action_model = _read(root, _SWIFT_ACTION_MODEL)
    state_model = _read(root, _SWIFT_STATE_MODEL)
    action_tests = _read(root, _SWIFT_ACTION_TEST)
    state_tests = _read(root, _SWIFT_STATE_TEST)
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
    submit_start = torii.find("    public func submitSignedPrivacyActionV1(")
    submit_end = torii.find("    public func getPrivacyActionStatusV1(", submit_start)
    submit = (
        torii[submit_start:submit_end]
        if submit_start >= 0 and submit_end > submit_start
        else ""
    )
    resolve_start = torii.find("    static func resolvePrivacyActionStatusV1(")
    resolve_end = torii.find(
        "    public func getAuthenticatedCommittedTransactionResultV1(",
        resolve_start,
    )
    resolve = (
        torii[resolve_start:resolve_end]
        if resolve_start >= 0 and resolve_end > resolve_start
        else ""
    )
    receipt_start = torii.find(
        "    private func getAuthenticatedPrivacyActionExecutionReceiptV1("
    )
    receipt_end = torii.find(
        "    private func getAuthenticatedPrivacyActionPublicStatusV1(",
        receipt_start,
    )
    receipt = (
        torii[receipt_start:receipt_end]
        if receipt_start >= 0 and receipt_end > receipt_start
        else ""
    )
    authenticated_action_flow = all(
        (
            "public typealias PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1"
            in action_model,
            "public struct PrivacyExact12ActionRequestV1" in action_model,
            "public struct PrivacyActionOperationViewV1" in action_model,
            "private var authenticatedProvenance" in action_model,
            "case applied = \"Applied\"" in action_model,
            _markers_are_strictly_ordered(
                submit,
                (
                    "PrivacyNativeBridge.inspectSignedExact12ActionV1(",
                    "getPrivacyExact12CapabilityManifestV1(",
                    ".requireExact12CapabilityTupleV1(",
                    "makeCanonicalAccountRequest(",
                    "sendBoundedSccpResponse(",
                ),
            ),
            'path: "/v1/pipeline/transactions"' in submit,
            "canonicalAuth: canonicalAuth" in submit,
            "bindingAuthenticatedSubmission(" in submit,
            "case .queued, .approved, .committed:" in resolve,
            "case .applied, .rejected:" in resolve,
            "let details = try await loadDetails()" in resolve,
            "let receipt = try await loadReceipt()" in resolve,
            "guard receipt == nil else" in resolve,
            "details.rejectionMessage" in resolve,
            "guard let details, let receipt else" in resolve,
            "receipt.admittedAtHeight == details.committedBlockHeight" in resolve,
            "executionReceiptFinalizedBlockHash: receipt.finalizedBlockHash" in resolve,
            "prepareAuthenticatedTransactionDetailsV1(" in bridge,
            "finalizeAuthenticatedTransactionDetailsV1(" in bridge,
            "projectAuthenticatedTransactionDetailsResultV1(" in bridge,
            "prepareAuthenticatedActionReceiptV1(" in bridge,
            "finalizeAuthenticatedActionReceiptV1(" in bridge,
            "projectAuthenticatedActionReceiptResultV1(" in bridge,
            "authenticatedActionReceiptProjectResultV1(" in bridge,
            "authenticatedTransactionDetailsPrepareV1(" in native,
            "authenticatedActionReceiptPrepareV1(" in native,
            'path: "/v1/query"' in receipt,
            "if response.statusCode == 404 { return nil }" in receipt,
            "response.statusCode == 204" not in receipt,
            "projectAuthenticatedActionReceiptResultV1(" in receipt,
            "testClosedOperationProtocolAndEffectMappings" in action_tests,
            "XCTAssertEqual(operations.count, 13)" in action_tests,
            "testStatusResolverRequiresReceiptBeforeAppliedTerminalization"
            in action_tests,
            "testStatusResolverRejectsReceiptContradictions" in action_tests,
            "testStatusRejectsDetachedOperationViewBeforeNetwork" in action_tests,
        )
    )

    state_query_ids = tuple(
        int(query_id)
        for query_id in re.findall(
            r"^\s{4}case [A-Za-z0-9]+ = (97|98|99|100|101|102|103|104)$",
            state_model,
            flags=re.MULTILINE,
        )
    )
    state_start = torii.find("    private func getPrivacyFinalizedStateV1<")
    state_end = torii.find("    public func submitSignedPrivacyActionV1(", state_start)
    state_query = (
        torii[state_start:state_end]
        if state_start >= 0 and state_end > state_start
        else ""
    )
    state_methods = (
        "getPrivacyZkAceReplayNullifierV1",
        "getPrivacyProofManagedPoolStateV1",
        "getPrivacyOrchardPoolStateV1",
        "getPrivacyOrchardNullifierV1",
        "getPrivacyAnonymousPgcPoolStateV1",
        "getPrivacyZkAmsAdmissionV1",
        "getPrivacyZkAmsProvisionV1",
        "getPrivacyZkX509CertificateNullifierV1",
    )
    state_requests = (
        "PrivacyZkAceReplayNullifierRequestV1",
        "PrivacyProofManagedPoolStateRequestV1",
        "PrivacyOrchardPoolStateRequestV1",
        "PrivacyOrchardNullifierRequestV1",
        "PrivacyAnonymousPgcPoolStateRequestV1",
        "PrivacyZkAmsAdmissionRequestV1",
        "PrivacyZkAmsProvisionRequestV1",
        "PrivacyZkX509CertificateNullifierRequestV1",
    )
    authenticated_finalized_state_queries = all(
        (
            state_query_ids == tuple(range(97, 105)),
            all(f"public struct {request}" in state_model for request in state_requests),
            all(f"public func {method}(" in torii for method in state_methods),
            "PrivacyFinalizedStateRequestV1" in state_model,
            "@PrivacyFinalizedUInt64V1 public var finalizedHeight" in state_model,
            "@PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash"
            in state_model,
            'baseURL.scheme?.lowercased() == "https"' in state_query,
            "prepareAuthenticatedPrivacyStateQueryV1(" in state_query,
            "finalizeAuthenticatedPrivacyStateQueryV1(" in state_query,
            "makeCanonicalAccountRequest(" in state_query,
            'path: "/v1/query"' in state_query,
            "canonicalAuth: canonicalAuth" in state_query,
            "if response.statusCode == 404 { return nil }" in state_query,
            "response.statusCode == 204" not in state_query,
            'contentType == "application/x-norito"' in state_query,
            "projectAuthenticatedPrivacyStateQueryResultV1(" in state_query,
            "prepareAuthenticatedPrivacyStateQueryV1(" in bridge,
            "finalizeAuthenticatedPrivacyStateQueryV1(" in bridge,
            "projectAuthenticatedPrivacyStateQueryResultV1(" in bridge,
            "authenticatedPrivacyStateQueryPrepareV1(" in native,
            "authenticatedPrivacyStateQueryFinalizeV1(" in native,
            "authenticatedPrivacyStateQueryProjectResultV1(" in native,
            "testClosedQueryIdsAndSelectorOrder" in state_tests,
            "testRequestSelectorsRejectWrongWidthAndZero" in state_tests,
            "testProofManagedProtocolIndicesAreClosed" in state_tests,
            "testProjectionRejectsNoncanonicalHashAndNumericLeaves" in state_tests,
        )
    )
    return {
        "canonical_manifest_model": canonical_model,
        "native_canonical_manifest_validation": native_backed_validation,
        "exact_native_local_tuple_match": exact_tuple_match,
        "transaction_admission_guard": transaction_admission,
        "authenticated_exact12_action_flow": authenticated_action_flow,
        "authenticated_finalized_state_queries": authenticated_finalized_state_queries,
    }


def _csharp_action_flow_gate(root: Path) -> bool:
    """Require C# Exact12 submission to resolve through native committed evidence."""

    model = _read(root, _CSHARP_ACTION_MODEL)
    native = _read(root, _CSHARP_NATIVE)
    torii = _read(root, _CSHARP_TORII)
    tests = _read(root, _CSHARP_ACTION_TEST)
    submit_start = torii.find("SubmitSignedPrivacyActionV1Async(")
    submit_end = torii.find("GetPrivacyActionStatusV1Async(", submit_start)
    submit = (
        torii[submit_start:submit_end]
        if submit_start >= 0 and submit_end > submit_start
        else ""
    )
    submit_order = tuple(
        submit.find(marker)
        for marker in (
            "InspectSignedExact12ActionV1(",
            "GetPrivacyExact12CapabilityManifestV1Async(",
            "RequireExact12CapabilityTupleV1(",
            "SubmitPrivacyActionWireOnceV1Async(",
        )
    )
    native_symbols = (
        "iroha_privacy_inspect_signed_exact12_action_v1",
        "iroha_privacy_authenticated_transaction_details_prepare_v1",
        "iroha_privacy_authenticated_transaction_details_finalize_v1",
        "iroha_privacy_authenticated_transaction_details_project_result_v1",
        "iroha_privacy_authenticated_action_receipt_prepare_v1",
        "iroha_privacy_authenticated_action_receipt_finalize_v1",
        "iroha_privacy_authenticated_action_receipt_project_result_v1",
        "iroha_privacy_authenticated_state_query_prepare_v1",
        "iroha_privacy_authenticated_state_query_finalize_v1",
        "iroha_privacy_authenticated_state_query_project_result_v1",
    )
    return all(
        (
            "class PrivacyExact12ActionRequestV1" in model,
            "class PrivacyActionOperationViewV1" in model,
            "Enum.GetValues<PrivacyOperationSchemaV1>()" in model,
            all(index >= 0 for index in submit_order),
            submit_order == tuple(sorted(submit_order)),
            "CryptographicOperations.FixedTimeEquals" in submit,
            '"/v1/pipeline/transactions"' in torii,
            '"/v1/pipeline/transactions/details"' in torii,
            '"/v1/query"' in torii,
            "RequirePrivacyActionContextV1" in torii,
            "Options.CanonicalRequestCredentials" in torii,
            "Options.LocalSigningContext" in torii,
            "PrivacyNative.BuildAuthenticatedTransactionDetailsQueryV1" in torii,
            "PrivacyNative.ProjectAuthenticatedTransactionDetailsResultV1" in torii,
            "PrivacyNative.BuildAuthenticatedPrivacyActionReceiptQueryV1" in torii,
            "PrivacyNative.ProjectAuthenticatedPrivacyActionReceiptResultV1" in torii,
            "details.ResultOk" in torii,
            "details.RejectionMessage" in torii,
            "receipt.AdmittedAtHeight != details.CommittedBlockHeight" in torii,
            "PipelineTransactionState.Committed" in torii,
            'string.Equals(status.ResolvedFrom, "cache"' in torii,
            "Rejected Exact12 status contradicts an authenticated execution receipt" in torii,
            "HttpStatusCode.NotFound" in torii,
            "ExecutionCapabilityManifestDigest" in model,
            "ExecutionCapabilityCommittedHeight" in model,
            "ExecutionReceiptFinalizedHeight" in model,
            "ExecutionReceiptFinalizedBlockHash" in model,
            all(symbol in native for symbol in native_symbols),
            "expectedFields.SetEquals" not in native,
            "observedFields.SetEquals(expectedFields)" in native,
            '"transaction_authority"' in native,
            '"committed_block_height"' in native,
            '"capability_manifest_digest"' in native,
            '"finalized_block_hash"' in native,
            "Exact12FlowPinsEveryAbi22Entrypoint" in tests,
            "AuthenticatedResultProjectionRequiresExactEightFieldContract" in tests,
            "AuthenticatedReceiptProjectionRequiresExactBoundFifteenFieldContract" in tests,
            "CommittedAndCacheExpiryRemainNonterminal" in tests,
            "ContradictoryTerminalEvidenceFailsClosedAndTerminalEvidenceIsStable" in tests,
        )
    )


def _csharp_finalized_state_gate(root: Path) -> bool:
    """Require C# IDs 97-104 to use the authenticated native query union."""

    model = _read(root, _CSHARP_STATE_MODEL)
    native = _read(root, _CSHARP_NATIVE)
    torii = _read(root, _CSHARP_STATE_TORII)
    tests = _read(root, _CSHARP_STATE_TEST)
    methods = (
        "GetPrivacyZkAceReplayNullifierV1Async",
        "GetPrivacyProofManagedPoolStateV1Async",
        "GetPrivacyOrchardPoolStateV1Async",
        "GetPrivacyOrchardNullifierV1Async",
        "GetPrivacyAnonymousPgcPoolStateV1Async",
        "GetPrivacyZkAmsAdmissionV1Async",
        "GetPrivacyZkAmsProvisionV1Async",
        "GetPrivacyZkX509CertificateNullifierV1Async",
    )
    return all(
        (
            all(method in torii for method in methods),
            "BuildAuthenticatedPrivacyStateQueryV1(" in native,
            "ProjectAuthenticatedPrivacyStateQueryResultV1(" in native,
            "RequirePrivacyStateQueryBindingV1(" in native,
            all(f"{query_id} =>" in native for query_id in range(97, 105)),
            "StrictUtf8Bytes(credentials.AccountId" in native,
            'PrivacyActionReceiptQueryPathV1 = "/v1/query"' in _read(
                root, _CSHARP_TORII
            ),
            "HttpStatusCode.NotFound" in torii,
            "response.StatusCode != HttpStatusCode.OK" in torii,
            'PrivacyActionNoritoMediaTypeV1 = "application/x-norito"'
            in _read(root, _CSHARP_TORII),
            "PrivacyNative.BuildAuthenticatedPrivacyStateQueryV1(" in torii,
            "PrivacyNative.ProjectAuthenticatedPrivacyStateQueryResultV1(" in torii,
            "PrivacyFinalizedStateContractV1.ParseProjectionV1(" in torii,
            "CryptographicOperations.FixedTimeEquals" in model,
            "NetworkId.Parse(literal)" in model,
            'literal.StartsWith("hash:", StringComparison.Ordinal)' in model,
            "supplied != Crc16(" in model,
            "RequireCanonicalU64String(" in model,
            "PrivacyProofManagedPoolTransitionViewV1" in model,
            "PrivacyOrchardPoolTransitionViewV1" in model,
            "PrivacyAnonymousPgcPoolTransitionViewV1" in model,
            "RequestsExposeTheClosedNativeQueryUnionAndDefensiveBindings" in tests,
            "ProofManagedProjectionBindsNetworkSelectorAndCanonicalWireForms" in tests,
            "ProjectionRejectsHostileSchemaAndBindingMutations" in tests,
            "FinalityHashesRequireCanonicalChecksummedHashLiterals" in tests,
        )
    )


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
            "authenticated_exact12_action_flow": javascript[
                "authenticated_exact12_action_flow"
            ],
        }
    if contract.name == "python-pyo3" and _read(root, _PYTHON_RUST_MANIFEST):
        python = _python_cutover_gates(root)
        manifest_model = manifest_model and python["canonical_manifest_model"]
        native_validation = python["native_canonical_manifest_validation"]
        tuple_match = tuple_match and python["exact_native_local_tuple_match"]
        transaction_admission = python["transaction_admission_guard"]
        extra_gates = {
            "authenticated_exact12_action_flow": python[
                "authenticated_exact12_action_flow"
            ],
            "authenticated_finalized_state_queries": python[
                "authenticated_finalized_state_queries"
            ],
        }
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
        extra_gates = {
            "authenticated_exact12_action_flow": jvm[
                "authenticated_exact12_action_flow"
            ],
            "authenticated_finalized_state_queries": jvm[
                "authenticated_finalized_state_queries"
            ],
        }
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
        extra_gates = {
            "authenticated_exact12_action_flow": swift[
                "authenticated_exact12_action_flow"
            ],
            "authenticated_finalized_state_queries": swift[
                "authenticated_finalized_state_queries"
            ],
        }
    if contract.name == "csharp":
        extra_gates["authenticated_exact12_action_flow"] = (
            _csharp_action_flow_gate(root)
        )
        extra_gates["authenticated_finalized_state_queries"] = (
            _csharp_finalized_state_gate(root)
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
    rust_controller_live_zk_ace_consumer = (
        _rust_controller_live_zk_ace_consumer_gate(root)
    )
    rust_action_driver_network_authority_separation = (
        _rust_action_driver_network_authority_separation_gate(root)
    )
    sdks = {contract.name: _sdk_result(root, contract) for contract in SDK_CONTRACTS}
    blockers = [name for name, result in sdks.items() if not result["ready"]]
    if not rust_controller_live_zk_ace_consumer:
        blockers.append("rust-controller-live-zk-ace-consumer")
    if not rust_action_driver_network_authority_separation:
        blockers.append("rust-action-driver-network-authority-separation")
    return {
        "schema_version": 1,
        "evidence_level": "source-prerequisite-not-native-release-authority",
        "abi22_privacy_exports": sorted(APPROVED_PRIVACY_EXPORTS),
        "authority": "torii-committed-canonical-manifest-bytes",
        "local_catalog_authorizes_network": False,
        "action_driver_accepted_as_network_evidence": False,
        "network_execution_authority": (
            "authenticated-client-controller-terminal-id105-and-typed-native-state"
        ),
        "rust_controller_live_zk_ace_consumer": rust_controller_live_zk_ace_consumer,
        "rust_action_driver_network_authority_separation": (
            rust_action_driver_network_authority_separation
        ),
        "ready": not blockers,
        "sdk": sdks,
        "blockers": blockers,
    }


def _format_human(report: dict[str, object]) -> str:
    lines = [
        "Exact12 cross-SDK capability-manifest parity: "
        + ("READY" if report["ready"] else "NOT READY"),
        "ABI22 privacy exports: exact twenty-four",
        "Network authority: Torii committed canonical manifest bytes",
        "Rust authenticated Exact12 controller consumer: "
        + (
            "ready"
            if report["rust_controller_live_zk_ace_consumer"]
            else "blocked"
        ),
        "Core action driver network evidence: rejected (construction-only)",
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
