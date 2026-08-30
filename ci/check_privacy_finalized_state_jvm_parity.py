#!/usr/bin/env python3
"""Lightweight source contract for authenticated ABI-22 JVM privacy flows.

This guard is deliberately importable. The authoritative Exact12 manifest-parity
checker calls :func:`audit` with its own source reader, so neither advertised JVM
gate can drift from the focused CI prerequisite or bypass hostile-source tests.
"""

from __future__ import annotations

import argparse
from collections.abc import Callable
from pathlib import Path


ACTION_GATE = "authenticated_exact12_action_flow"
STATE_GATE = "authenticated_finalized_state_queries"

QUERY_METHODS = (
    "getPrivacyZkAceReplayNullifierV1",
    "getPrivacyProofManagedPoolStateV1",
    "getPrivacyOrchardPoolStateV1",
    "getPrivacyOrchardNullifierV1",
    "getPrivacyAnonymousPgcPoolStateV1",
    "getPrivacyZkAmsAdmissionV1",
    "getPrivacyZkAmsProvisionV1",
    "getPrivacyZkX509CertificateNullifierV1",
)
STATE_NATIVE_METHODS = (
    "nativePreparePrivacyStateQueryV1",
    "nativeFinalizePrivacyStateQueryV1",
    "nativeProjectPrivacyStateQueryV1",
)
RECEIPT_NATIVE_METHODS = (
    "nativePreparePrivacyActionReceiptQueryV1",
    "nativeFinalizePrivacyActionReceiptQueryV1",
    "nativeProjectPrivacyActionReceiptV1",
)

ACTION_MODEL = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyExact12ActionModelsV1.kt"
)
STATE_MODEL = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyFinalizedStateModelsV1.kt"
)
KOTLIN_RECEIPT_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedPrivacyActionReceiptNativeBridge.kt"
)
JAVA_RECEIPT_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyActionReceiptNativeBridge.java"
)
KOTLIN_DETAILS_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedTransactionDetailsNativeBridge.kt"
)
JAVA_DETAILS_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedTransactionDetailsNativeBridge.java"
)
KOTLIN_STATE_BRIDGE = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
    "AuthenticatedPrivacyStateQueryNativeBridge.kt"
)
JAVA_STATE_BRIDGE = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyStateQueryNativeBridge.java"
)
KOTLIN_TRANSPORT = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt"
)
JAVA_TRANSPORT = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
    "HttpClientTransport.java"
)
JNI_HELPERS = "crates/connect_norito_bridge/src/platform_jni/part_2.rs"
JNI_EXPORTS = "crates/connect_norito_bridge/src/platform_jni/part_3.rs"
NATIVE_RECEIPT = (
    "crates/connect_norito_bridge/src/authenticated_privacy_action_receipt.rs"
)
NATIVE_STATE = "crates/connect_norito_bridge/src/authenticated_privacy_state_query.rs"
NATIVE_QUERY_ACCESS = "crates/iroha_core/src/executor.rs"
NATIVE_QUERY_MEMORY = "crates/iroha_core/src/smartcontracts/isi/query/ordinary_memory.rs"
TORII_QUERY_ROUTING = "crates/iroha_torii/src/lib.rs"
KOTLIN_ACTION_TEST = (
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyExact12ActionModelsV1Test.kt"
)
KOTLIN_STATE_TEST = (
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
    "PrivacyFinalizedStateModelsV1Test.kt"
)
JAVA_ACTION_TEST = (
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/"
    "PrivacyExact12ActionInspectionV1Tests.java"
)
JAVA_STATE_TEST = (
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/"
    "AuthenticatedPrivacyStateQueryNativeBridgeTests.java"
)
CSHARP_STATE_MODEL = (
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyFinalizedStateModelsV1.cs"
)
JVM_CI = "ci/check_privacy_jvm_sdk.sh"
GUARD_BASENAME = "check_privacy_finalized_state_jvm_parity.py"

SourceReader = Callable[[str], str]


def _root_reader(root: Path) -> SourceReader:
    def read(relative: str) -> str:
        try:
            return (root / relative).read_text(encoding="utf-8", errors="strict")
        except (OSError, UnicodeError):
            return ""

    return read


def require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def _method_body(source: str, start: str, end: str) -> str:
    start_index = source.find(start)
    end_index = source.find(end, start_index + len(start))
    if start_index < 0 or end_index <= start_index:
        return ""
    return source[start_index:end_index]


def _require_exact_query_transport(
    source: str,
    label: str,
    errors: list[str],
) -> None:
    if label == "Kotlin":
        post = _method_body(
            source,
            "private fun buildExactSignedQueryPostRequest(",
            "private fun requireCanonicalHeadersUnset(",
        )
        optional = _method_body(
            source,
            "internal fun fetchExactNoritoBytesAllowingNotFound(",
            "private fun requireExactHeader(",
        )
        exact_404 = "response.statusCode == 404" in optional
        exact_200 = "response.statusCode == 200" in optional
    else:
        post = _method_body(
            source,
            "private TransportRequest buildExactSignedQueryPostRequest(",
            "private TransportRequest buildExactNoritoPostRequest(",
        )
        optional = _method_body(
            source,
            "CompletableFuture<byte[]> fetchExactNoritoBytesAllowingNotFound(",
            "private static void requireExactHeader(",
        )
        exact_404 = "response.statusCode() == 404" in optional
        exact_200 = "response.statusCode() != 200" in optional
    require(
        '.setMethod("POST")' in post
        and '.addHeader("Accept", APPLICATION_NORITO)' in post
        and '.addHeader("Content-Type", APPLICATION_NORITO)' in post,
        f"{label} signed-query transport is not exact POST application/x-norito",
        errors,
    )
    require(
        'APPLICATION_NORITO = "application/x-norito"' in source,
        f"{label} transport does not pin the Norito media type",
        errors,
    )
    require(
        exact_404
        and exact_200
        and "future.complete(null)" in optional
        and "204" not in optional,
        f"{label} optional query transport must map only HTTP 404 to null",
        errors,
    )


def _audit_action_flow(read: SourceReader) -> tuple[str, ...]:
    errors: list[str] = []
    model = read(ACTION_MODEL)
    kotlin_receipt = read(KOTLIN_RECEIPT_BRIDGE)
    java_receipt = read(JAVA_RECEIPT_BRIDGE)
    kotlin_details = read(KOTLIN_DETAILS_BRIDGE)
    java_details = read(JAVA_DETAILS_BRIDGE)
    kotlin_transport = read(KOTLIN_TRANSPORT)
    java_transport = read(JAVA_TRANSPORT)
    jni_helpers = read(JNI_HELPERS)
    jni_exports = read(JNI_EXPORTS)
    native_receipt = read(NATIVE_RECEIPT)
    native_access = read(NATIVE_QUERY_ACCESS)
    memory = read(NATIVE_QUERY_MEMORY)
    torii = read(TORII_QUERY_ROUTING)
    kotlin_test = read(KOTLIN_ACTION_TEST)
    java_test = read(JAVA_ACTION_TEST)
    ci = read(JVM_CI)

    for marker in (
        "typealias PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1",
        "class PrivacyExact12ActionRequestV1",
        "class PrivacyActionOperationViewV1",
        "executionCapabilityManifestDigestValue",
        "executionCapabilityCommittedHeight",
        "executionReceiptFinalizedHeight",
        "executionReceiptFinalizedBlockHashValue",
        "requireAuthenticatedProvenanceV1",
    ):
        require(marker in model, f"Exact12 JVM action model lacks {marker}", errors)
    require(
        "assertEquals(13, operations.size)" in kotlin_test,
        "Kotlin regression does not pin the closed 13-operation union",
        errors,
    )
    require(
        "projectionIsExactAndSnapshotsBytes" in java_test
        and "malformedOrZeroDigestProjectionFailsClosed" in java_test,
        "Android regression does not pin native action-inspection projection",
        errors,
    )

    for bridge, label in (
        (kotlin_receipt, "Kotlin receipt"),
        (java_receipt, "Android receipt"),
    ):
        require(
            "IrohaQuerySignatureProvider" in bridge and "signQueryDigest" in bridge,
            f"{label} bridge does not keep the signer opaque",
            errors,
        )
        for method in RECEIPT_NATIVE_METHODS:
            require(method in bridge, f"{label} bridge lacks {method}", errors)
        require(
            "transactionIntentDigest" in bridge
            and "statementDigest" in bridge
            and "proofEnvelopeHash" in bridge,
            f"{label} bridge does not bind all action digests",
            errors,
        )
    for bridge, label in (
        (kotlin_details, "Kotlin transaction-details"),
        (java_details, "Android transaction-details"),
    ):
        require(
            "IrohaQuerySignatureProvider" in bridge and "signQueryDigest" in bridge,
            f"{label} bridge does not keep the signer opaque",
            errors,
        )
        for method in (
            "nativePrepareExactRejectedTransactionQueryV1",
            "nativeFinalizeExactRejectedTransactionQueryV1",
            "nativeProjectExactCommittedTransactionResultV1",
            "nativeProjectExactOfflineDeviceRegistrationResultV1",
        ):
            require(method in bridge, f"{label} bridge lacks {method}", errors)

    for transport, label in (
        (kotlin_transport, "Kotlin"),
        (java_transport, "Android"),
    ):
        _require_exact_query_transport(transport, label, errors)
        require(
            "getAuthenticatedOfflineDeviceRegistrationResultV1" in transport,
            f"{label} transport lacks typed offline-device registration results",
            errors,
        )
        submit_start = transport.find("submitSignedPrivacyActionV1(")
        status_start = transport.find("getPrivacyActionStatusV1(", submit_start)
        submit = (
            transport[submit_start:status_start]
            if submit_start >= 0 and status_start > submit_start
            else ""
        )
        submit_order = tuple(
            submit.find(marker)
            for marker in (
                "inspectSignedExact12ActionV1(",
                "getPrivacyCapabilities(",
                "requireExact12CapabilityTupleV1(",
                "submitExactPrivacyActionWire(",
            )
        )
        require(
            all(index >= 0 for index in submit_order)
            and submit_order == tuple(sorted(submit_order)),
            f"{label} submission does not inspect, fresh-admit, then dispatch",
            errors,
        )
        for marker in (
            '"/v1/pipeline/transactions"',
            '"/v1/pipeline/transactions/details"',
            '"/v1/query"',
            "buildExactSignedQueryPostRequest",
            "fetchExactNoritoBytesAllowingNotFound",
            "AuthenticatedPrivacyActionReceiptNativeBridge",
            "AuthenticatedTransactionDetailsNativeBridge",
            "details == null || receipt == null",
        ):
            require(marker in transport, f"{label} action flow lacks {marker}", errors)
        require(
            "receipt.admittedAtHeight == details.committedBlockHeight" in transport
            or "receipt.admittedAtHeight().equals(details.committedBlockHeight())"
            in transport,
            f"{label} action flow does not reconcile receipt and committed height",
            errors,
        )
        require(
            "Queued" in transport and "Approved" in transport and "Committed" in transport,
            f"{label} action flow does not preserve nonterminal pipeline states",
            errors,
        )
    require(
        "privacyActionQuerySignerV1(canonicalAuth" in kotlin_transport,
        "Kotlin action queries do not use the opaque canonical signer adapter",
        errors,
    )
    require(
        "canonicalAuth::sign" in java_transport,
        "Android action queries do not use canonicalAuth::sign",
        errors,
    )

    for helper in (
        "java_native_authenticated_transaction_details_prepare_v1",
        "java_native_authenticated_transaction_details_finalize_v1",
        "java_native_authenticated_transaction_details_project_result_v1",
        "java_native_authenticated_offline_device_registration_result_project_v1",
        "java_native_authenticated_privacy_action_receipt_prepare_v1",
        "java_native_authenticated_privacy_action_receipt_finalize_v1",
        "java_native_authenticated_privacy_action_receipt_project_v1",
    ):
        require(helper in jni_helpers, f"JNI helper lacks {helper}", errors)
        require(helper in jni_exports, f"paired JNI exports do not delegate {helper}", errors)
    for namespace in ("sdk", "android"):
        details_prefix = (
            f"Java_org_hyperledger_iroha_{namespace}_client_"
            "AuthenticatedTransactionDetailsNativeBridge_"
        )
        require(
            details_prefix + "nativeProjectExactOfflineDeviceRegistrationResultV1"
            in jni_exports,
            f"missing paired JNI device-registration export {namespace}",
            errors,
        )
        receipt_prefix = (
            f"Java_org_hyperledger_iroha_{namespace}_client_"
            "AuthenticatedPrivacyActionReceiptNativeBridge_"
        )
        for method in RECEIPT_NATIVE_METHODS:
            require(
                receipt_prefix + method in jni_exports,
                f"missing paired JNI receipt export {namespace}.{method}",
                errors,
            )

    for marker in (
        "norito::decode_canonical_with_limits",
        "norito::canonical_decode_limits(response.len())",
        "canonical != response",
        "receipt.transaction_intent_digest.as_bytes()",
        "receipt.statement_digest.as_bytes()",
        "receipt.proof_envelope_hash",
        ".validate()",
    ):
        require(marker in native_receipt, f"native receipt verifier lacks {marker}", errors)
    receipt_query = "FindPrivacyActionExecutionReceiptV1"
    require(
        f"105 => {receipt_query}: ProvenBounded" in memory,
        "native action receipt is not pinned to registered query ID 105",
        errors,
    )
    require(
        f"SingularQueryBox::{receipt_query}(" in native_access
        and "NativeQueryAccess::Registered" in native_access,
        "native action receipt is not Registered",
        errors,
    )
    require(
        torii.count(f"SingularQueryBox::{receipt_query}(") >= 2
        and "Some(SignedQueryScope::LocalReplicated)" in torii,
        "native action receipt is not LocalReplicated",
        errors,
    )
    require(
        GUARD_BASENAME in ci
        and "PrivacyExact12ActionModelsV1Test" in ci
        and "PrivacyExact12ActionInspectionV1Tests" in ci,
        "JVM CI does not execute the action-flow guard and focused regressions",
        errors,
    )
    return tuple(errors)


def _audit_finalized_state(read: SourceReader) -> tuple[str, ...]:
    errors: list[str] = []
    kotlin_models = read(STATE_MODEL)
    kotlin_bridge = read(KOTLIN_STATE_BRIDGE)
    java_bridge = read(JAVA_STATE_BRIDGE)
    kotlin_transport = read(KOTLIN_TRANSPORT)
    java_transport = read(JAVA_TRANSPORT)
    jni_helpers = read(JNI_HELPERS)
    jni_exports = read(JNI_EXPORTS)
    native_state = read(NATIVE_STATE)
    native_access = read(NATIVE_QUERY_ACCESS)
    memory = read(NATIVE_QUERY_MEMORY)
    torii = read(TORII_QUERY_ROUTING)
    kotlin_test = read(KOTLIN_STATE_TEST)
    java_test = read(JAVA_STATE_TEST)
    csharp_models = read(CSHARP_STATE_MODEL)
    ci = read(JVM_CI)

    for method in STATE_NATIVE_METHODS:
        require(method in kotlin_bridge, f"Kotlin bridge lacks {method}", errors)
        require(method in java_bridge, f"Android bridge lacks {method}", errors)
    for bridge, label in (
        (kotlin_bridge, "Kotlin"),
        (java_bridge, "Android"),
    ):
        require(
            "IrohaQuerySignatureProvider" in bridge and "signQueryDigest" in bridge,
            f"{label} state bridge does not keep the signer opaque",
            errors,
        )
        require(
            "PrivacyFinalizedStateProjectionV1.parse" in bridge,
            f"{label} bridge does not parse only native-verified projections",
            errors,
        )
    for helper in (
        "java_native_authenticated_privacy_state_query_prepare_v1",
        "java_native_authenticated_privacy_state_query_finalize_v1",
        "java_native_authenticated_privacy_state_query_project_v1",
    ):
        require(helper in jni_helpers, f"JNI helper lacks {helper}", errors)
        require(helper in jni_exports, f"JNI paired exports do not delegate {helper}", errors)
    for namespace in ("sdk", "android"):
        prefix = (
            f"Java_org_hyperledger_iroha_{namespace}_client_"
            "AuthenticatedPrivacyStateQueryNativeBridge_"
        )
        for method in STATE_NATIVE_METHODS:
            require(
                prefix + method in jni_exports,
                f"missing paired JNI state export {namespace}.{method}",
                errors,
            )

    for transport, label in ((kotlin_transport, "Kotlin"), (java_transport, "Android")):
        _require_exact_query_transport(transport, label, errors)
        for method in QUERY_METHODS:
            require(method in transport, f"{label} transport lacks {method}", errors)
        for marker in (
            '"/v1/query"',
            "buildExactSignedQueryPostRequest",
            "fetchExactNoritoBytesAllowingNotFound",
            "AuthenticatedPrivacyStateQueryNativeBridge.projectPrivacyStateQueryV1",
        ):
            require(marker in transport, f"{label} state queries lack {marker}", errors)
    require(
        "privacyActionQuerySignerV1(canonicalAuth" in kotlin_transport,
        "Kotlin state queries do not use the opaque canonical signer adapter",
        errors,
    )
    require(
        "canonicalAuth::sign" in java_transport,
        "Android state queries do not use canonicalAuth::sign",
        errors,
    )

    expected_selectors = {
        "PrivacyZkAceReplayNullifierRequestV1": (97, 64),
        "PrivacyProofManagedPoolStateRequestV1": (98, 32),
        "PrivacyOrchardPoolStateRequestV1": (99, 32),
        "PrivacyOrchardNullifierRequestV1": (100, 64),
        "PrivacyAnonymousPgcPoolStateRequestV1": (101, 32),
        "PrivacyZkAmsAdmissionRequestV1": (102, 128),
        "PrivacyZkAmsProvisionRequestV1": (103, 128),
        "PrivacyZkX509CertificateNullifierRequestV1": (104, 96),
    }
    for name, (query_id, _width) in expected_selectors.items():
        require(f"class {name}" in kotlin_models, f"missing immutable selector {name}", errors)
        require(
            f"override val queryId: Int = {query_id}" in kotlin_models,
            f"{name} is not bound to query ID {query_id}",
            errors,
        )
    require(
        "concat32V1(trustAnchor, policy, consumed)" in kotlin_models,
        "Kotlin ID104 selector is not exactly trust-anchor + policy + nullifier",
        errors,
    )
    require(
        "trustAnchorId,\n            policyId,\n            nullifier" in csharp_models
        and "trustAnchorId,\n            policyId,\n            policyId,\n            nullifier"
        not in csharp_models,
        "C# ID104 selector must not duplicate its policy chunk",
        errors,
    )
    require(
        "projectionFields(): Map<String, Any?> = freezeObjectV1(fields)" in kotlin_models,
        "finalized JVM projections must remain deeply immutable",
        errors,
    )
    for marker in (
        "decode_canonical_with_limits",
        "canonical_decode_limits(response.len())",
        "canonical != response",
        "parse_binding(",
        "view.validate()",
        "view.network_id",
    ):
        require(marker in native_state, f"native state verifier lacks {marker}", errors)
    require(
        "match (binding, decoded)" in native_state
        and native_state.count("differs from its request") == 8,
        "native state projection does not bind all eight selectors to typed outputs",
        errors,
    )
    native_query_names = (
        "FindPrivacyZkAceReplayNullifierV1",
        "FindPrivacyProofManagedPoolStateV1",
        "FindPrivacyOrchardPoolStateV1",
        "FindPrivacyOrchardNullifierV1",
        "FindPrivacyAnonymousPgcPoolStateV1",
        "FindPrivacyZkAmsAdmissionV1",
        "FindPrivacyZkAmsProvisionV1",
        "FindPrivacyZkX509CertificateNullifierV1",
    )
    for query_id, name in enumerate(native_query_names, start=97):
        require(
            f"{query_id} => {name}: ProvenBounded" in memory,
            f"native query ID {query_id} is not pinned to {name}",
            errors,
        )
        require(
            f"SingularQueryBox::{name}(" in native_access,
            f"native query ID {query_id} is not Registered",
            errors,
        )
        require(
            torii.count(f"SingularQueryBox::{name}(") >= 2,
            f"native query ID {query_id} is not LocalReplicated and registered for routing",
            errors,
        )
    require(
        "NativeQueryAccess::Registered" in native_access
        and "Some(SignedQueryScope::LocalReplicated)" in torii
        and "is_registered_exact12_local_query" in torii,
        "native IDs 97-104 lost Registered + LocalReplicated classification",
        errors,
    )
    require(
        "x509SelectorIsExactlyTrustAnchorPolicyAndNullifier" in kotlin_test
        and "projectionBindsNetworkSelectorAndImmutableFinality" in kotlin_test,
        "Kotlin finalized-state regressions are incomplete",
        errors,
    )
    require(
        "selectorsCoverExactlyIds97Through104" in java_test
        and "x509BindingContainsNoDuplicatedPolicyChunk" in java_test,
        "Android finalized-state regressions are incomplete",
        errors,
    )
    require(
        GUARD_BASENAME in ci
        and "PrivacyFinalizedStateModelsV1Test" in ci
        and "AuthenticatedPrivacyStateQueryNativeBridgeTests" in ci,
        "JVM CI does not execute the finalized-state guard and focused regressions",
        errors,
    )
    return tuple(errors)


def audit(
    root: Path,
    reader: SourceReader | None = None,
) -> dict[str, tuple[str, ...]]:
    """Return per-capability source-contract failures without running a build."""

    resolved = root.resolve()
    read = reader if reader is not None else _root_reader(resolved)
    return {
        ACTION_GATE: _audit_action_flow(read),
        STATE_GATE: _audit_finalized_state(read),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    results = audit(args.root)
    errors = [
        f"{gate}: {error}"
        for gate, gate_errors in results.items()
        for error in gate_errors
    ]
    if errors:
        print("authenticated privacy JVM parity check failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("authenticated privacy JVM parity check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
