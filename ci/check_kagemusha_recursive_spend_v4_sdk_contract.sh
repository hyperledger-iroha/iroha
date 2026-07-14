#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_V4_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

if [[ $# -ne 0 ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh" >&2
  exit 2
fi

python3 - "$ROOT_DIR" <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path


root = Path(sys.argv[1]).resolve()
paths = {
    "data_model": Path("crates/iroha_data_model/src/offline/mod.rs"),
    "rust": Path("crates/connect_norito_bridge/src/lib.rs"),
    "header": Path("crates/connect_norito_bridge/include/connect_norito_bridge.h"),
    "swift": Path("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"),
    "swift_v4": Path("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift"),
    "swift_v4_codecs": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4Codecs.swift"
    ),
    "swift_native": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
    ),
    "swift_bridge": Path("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    "swift_coordinator": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaArtifactCoordinator.swift"
    ),
    "kagami": Path("crates/iroha_kagami/src/kagemusha.rs"),
    "bundle": Path("crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs"),
    "core_artifact": Path("crates/iroha_core/src/zk/kagemusha_artifact_v4.rs"),
    "kotlin": Path(
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/"
        "KagemushaRecursiveSpendProver.kt"
    ),
    "java": Path(
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/"
        "KagemushaRecursiveSpendProver.java"
    ),
    "xcframework_build": Path("scripts/build_norito_xcframework.sh"),
    "mobile_check": Path("scripts/check_mobile_sdk_artifacts.sh"),
    "mobile_check_test": Path("scripts/check_mobile_sdk_artifacts_test.sh"),
}

texts: dict[str, str] = {}
for label, relative in paths.items():
    absolute = root / relative
    if not absolute.is_file():
        raise SystemExit(f"required ABI20 contract file is missing: {relative}")
    texts[label] = absolute.read_text(encoding="utf-8")

# The check is dormant on branches that have no ABI20 SDK work. As soon as a
# V4 lifecycle method or carrier is introduced, the entire boundary must land
# atomically instead of relying on symbol presence or an ABI19 fallback.
v4_markers = (
    "nativeInitSpendV4",
    "KagemushaRecursiveSpendInitLocalRequestV4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
)
if not any(
    marker in texts[label]
    for marker in v4_markers
    for label in ("rust", "header", "swift_v4", "kotlin", "java")
):
    print("Kagemusha ABI20 SDK contract is not exposed; fail-closed pre-V4 state accepted.")
    raise SystemExit(0)

errors: list[str] = []


def require(label: str, needle: str) -> None:
    if needle not in texts[label]:
        errors.append(f"{paths[label]}: missing {needle!r}")


def require_regex(label: str, pattern: str, description: str) -> None:
    if re.search(pattern, texts[label], re.MULTILINE | re.DOTALL) is None:
        errors.append(f"{paths[label]}: missing {description}")


v4_files = (
    "step-eq.parameters.krv4",
    "step-eq.circuit-params.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.parameters.krv4",
    "step-ep.circuit-params.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
v4_roles = (
    "step_eq_parameters",
    "step_eq_circuit_params",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_parameters",
    "step_ep_circuit_params",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
v4_case_names = (
    "stepEqParameters",
    "stepEqCircuitParams",
    "stepEqProvingKey",
    "stepEqVerifyingKey",
    "stepEqBootstrapWitness",
    "stepEpParameters",
    "stepEpCircuitParams",
    "stepEpProvingKey",
    "stepEpVerifyingKey",
    "stepEpBootstrapWitness",
)
sdk_schema_types = {
    "InitRequestV4": "KagemushaRecursiveSpendInitLocalRequestV4",
    "AppendRequestV4": "KagemushaRecursiveSpendAppendLocalRequestV4",
    "VerifyRequestV4": "KagemushaRecursiveSpendVerifyLocalRequestV4",
    "RedeemRequestV4": "KagemushaRecursiveSpendRedeemLocalRequestV4",
    "InitResultV4": "KagemushaRecursiveSpendInitResultV4",
    "SplitResultV4": "KagemushaRecursiveSpendSplitResultV4",
    "VerifyResultV4": "KagemushaRecursiveSpendVerifyResultV4",
    "RedeemBuildResultV4": "KagemushaRecursiveSpendRedeemBuildResultV4",
}

native_methods = (
    "nativePastaCycleV4BackendAvailable",
    "nativeArtifactBeginV4",
    "nativeArtifactWriteV4",
    "nativeArtifactFinalizeV4",
    "nativeArtifactCancelV4",
    "nativeArtifactSetInstallV4",
    "nativeArtifactSetIsInstalledV4",
    "nativeArtifactSetUninstallV4",
    "nativeBuildOutputMembershipPathsV4",
    "nativeBuildInitRequestV4",
    "nativeBuildAppendRequestV4",
    "nativeBuildVerifyRequestV4",
    "nativeBuildRedeemRequestV4",
    "nativeInitSpendV4",
    "nativeAppendSpendV4",
    "nativeVerifySpendV4",
    "nativeBuildRedeemV4",
)

for label in ("kotlin", "java"):
    require(label, "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION")
    require(label, "V4_ARTIFACT_MANIFEST_SCHEMA")
    require(label, "V4_ARTIFACT_COUNT")
    require(label, "kagemusha.offline.recursive_spend.artifact_manifest.v4")
    require(label, "requireV4ArtifactBridge")
    require(label, "requireV4ProofBackend")
    for file_name in v4_files:
        require(label, file_name)
    for type_name, schema in sdk_schema_types.items():
        require_regex(
            label,
            rf"class\s+{re.escape(type_name)}\b[\s\S]{{0,700}}?\"{re.escape(schema)}\"",
            f"distinct {type_name} wrapper bound to {schema}",
        )
    for method in ("initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"):
        require_regex(label, rf"\b{method}\s*\(", f"distinct public V4 lifecycle method {method}")

swift_v4_types = (
    "KagemushaRecursiveSpendArtifactBindingV4",
    "KagemushaRecursiveSpendBundleV4",
    "KagemushaRecursiveSpendTopUpAnchorV4",
    "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
    "KagemushaOutputMembershipLeafPathsV4",
    "KagemushaOutputMembershipPathsV4",
    "KagemushaRecursiveSpendInitRequestV4",
    "KagemushaRecursiveSpendInitLocalRequestV4",
    "KagemushaRecursiveSpendAppendInputV4",
    "KagemushaRecursiveSpendSpendableBranchV4",
    "KagemushaRecursiveSpendAppendLocalRequestV4",
    "KagemushaRecursiveSpendVerifyRequestV4",
    "KagemushaRecursiveSpendVerifyLocalRequestV4",
    "KagemushaRecursiveSpendRedeemLocalRequestV4",
    "KagemushaRecursiveSpendInitResultV4",
    "KagemushaRecursiveSpendSplitResultV4",
    "KagemushaRecursiveSpendVerifyResultV4",
    "KagemushaRecursiveSpendRedeemBuildResultV4",
)
for type_name in swift_v4_types:
    require_regex(
        "swift_v4",
        rf"public\s+struct\s+{re.escape(type_name)}\b",
        f"distinct Swift carrier {type_name}",
    )

for method, native_call, result_type in (
    ("initSpendV4", "kagemushaRecursiveSpendInitV4", "KagemushaRecursiveSpendInitResultV4"),
    ("appendSpendV4", "kagemushaRecursiveSpendAppendV4", "KagemushaRecursiveSpendSplitResultV4"),
    ("verifySpendV4", "kagemushaRecursiveSpendVerifyV4", "KagemushaRecursiveSpendVerifyResultV4"),
    ("buildRedeemV4", "kagemushaRecursiveSpendRedeemV4", "KagemushaRecursiveSpendRedeemBuildResultV4"),
):
    require_regex(
        "swift_v4",
        rf"static\s+func\s+{method}\s*\([\s\S]{{0,1800}}?\.{native_call}\s*\([\s\S]{{0,900}}?{result_type}\s*\(",
        f"direct Swift ABI20 lifecycle {method}",
    )

if texts["swift_v4"].count(
    "catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable"
) != 4:
    errors.append(
        f"{paths['swift_v4']}: all four V4 lifecycle calls must catch the explicit V4-unavailable error"
    )
if "kagemushaRecursiveSpendV2Unavailable" in texts["swift_v4"]:
    errors.append(
        f"{paths['swift_v4']}: V4 lifecycle must not catch the ABI19 unavailable error"
    )
for needle in (
    "case kagemushaRecursiveSpendV4Unavailable",
    "case kagemushaRecursiveSpendV4Artifact",
    "case -316: return .kagemushaRecursiveSpendV4Unavailable",
    "case -317: return .kagemushaRecursiveSpendV4Artifact",
):
    require("swift_bridge", needle)
require_regex(
    "swift",
    r"copyKagemushaV4Output\([\s\S]{0,650}?NativeBridgeError\.fromStatus\(status\)",
    "Swift V4 status mapping through the explicit bridge error table",
)

for encoder, wire_name in (
    ("encodeInitLocalRequest", "initLocalRequestWireNameV4"),
    ("encodeAppendLocalRequest", "appendLocalRequestWireNameV4"),
    ("encodeVerifyLocalRequest", "verifyLocalRequestWireNameV4"),
    ("encodeRedeemLocalRequest", "redeemLocalRequestWireNameV4"),
):
    require_regex(
        "swift_v4_codecs",
        rf"static\s+func\s+{encoder}\s*\([\s\S]{{0,2600}}?localWitnessVersionV4[\s\S]{{0,2600}}?{wire_name}",
        f"field-for-field Swift V4 encoder {encoder}",
    )

require_regex(
    "swift",
    r"public\s+struct\s+KagemushaRecursiveSpendInstalledArtifactSetV4\b[\s\S]{0,500}?KagemushaRecursiveSpendArtifactBindingV4",
    "Swift V4-only installed artifact set",
)
require_regex(
    "swift_coordinator",
    r"KagemushaRecursiveSpendArtifactBindingV4[\s\S]{0,800}?KagemushaRecursiveSpendInstalledArtifactSetV4",
    "Swift V4-only artifact coordinator",
)

for label in ("rust", "kotlin", "java"):
    for forbidden in ("nativeWrapInitRequestV4", "nativeWrapAppendRequestV4",
                      "nativeWrapVerifyRequestV4", "nativeWrapRedeemRequestV4"):
        if forbidden in texts[label]:
            errors.append(f"{paths[label]}: forbidden ABI19-to-ABI20 wrapper {forbidden!r}")

for label in ("swift", "swift_v4", "swift_v4_codecs", "swift_native"):
    for forbidden in (
        "legacyRequest",
        "encodeInitLocalRequestV4",
        "encodeAppendLocalRequestV4",
        "encodeVerifyLocalRequestV4",
        "encodeRedeemLocalRequestV4",
    ):
        if forbidden in texts[label]:
            errors.append(f"{paths[label]}: forbidden shared V3/V4 alias {forbidden!r}")

if re.search(
    r"static\s+func\s+(?:initSpendV4|appendSpendV4|verifySpendV4|buildRedeemV4)\b"
    r"[\s\S]{0,2600}?(?:decode(?:Init|Split|Verify|Redeem)[A-Za-z]*\(|"
    r"kagemushaRecursiveSpend(?:Init|Append|Verify|Redeem)V[23]\s*\()",
    texts["swift_v4"],
):
    errors.append(
        f"{paths['swift_v4']}: Swift V4 lifecycle must not invoke a V2/V3 native call or decoder"
    )

if re.search(
    r"static\s+func\s+(?:initSpend|appendSpend|verifySpend|buildRedeem)\b"
    r"[\s\S]{0,2600}?kagemushaRecursiveSpend(?:Init|Append|Verify|Redeem)V4\s*\(",
    texts["swift"],
):
    errors.append(
        f"{paths['swift']}: frozen Swift lifecycle must not invoke an ABI20 symbol"
    )

for label in ("kotlin", "java"):
    if "legacyRequest" in texts[label]:
        errors.append(f"{paths[label]}: V4 SDK must not route through a legacy request archive")
    require_regex(
        label,
        r"nativeBuildInitRequestV4\s*\(\s*(?:final\s+)?(?:byte\[\]|anchor\s*:\s*ByteArray)[\s\S]{0,450}?(?:proof|topUpFinalityProof)[\s\S]{0,450}?roster",
        "genuine V4 init builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildAppendRequestV4\s*\([\s\S]{0,350}?bundles[\s\S]{0,350}?openings[\s\S]{0,350}?(?:witnesses|membershipWitnesses)",
        "genuine V4 append builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildVerifyRequestV4\s*\([\s\S]{0,350}?bundle[\s\S]{0,350}?(?:recipientRequest|recipient_request)[\s\S]{0,350}?roster",
        "genuine V4 verify builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildRedeemRequestV4\s*\([\s\S]{0,350}?bundle[\s\S]{0,350}?opening[\s\S]{0,350}?(?:membershipWitness|membership_witness)",
        "genuine V4 redeem builder inputs",
    )
    require_regex(
        label,
        r"class\s+BundleV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendBundleV4\"",
        "distinct BundleV4 wrapper",
    )
    require_regex(
        label,
        r"class\s+TopUpAnchorV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendTopUpAnchorV4\"",
        "distinct TopUpAnchorV4 wrapper",
    )
    require_regex(
        label,
        r"class\s+TopUpFinalityEvidenceV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendTopUpFinalityEvidenceV4\"",
        "distinct TopUpFinalityEvidenceV4 wrapper",
    )
    if re.search(
        r"project(?:Split|Verify|Redeem)[A-Za-z]*V4\s*\([\s\S]{0,1800}?nativeProject[A-Za-z]*V2",
        texts[label],
    ):
        errors.append(f"{paths[label]}: V4 result projection must not invoke a V2 decoder")

require_regex(
    "kotlin",
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*:\s*Int\s*=\s*20\b",
    "exact ABI20 Kotlin constant",
)


def quoted_inventory(label: str, pattern: str, declaration: str) -> tuple[str, ...]:
    match = re.search(pattern, texts[label], re.MULTILINE | re.DOTALL)
    if match is None:
        errors.append(f"{paths[label]}: missing ordered {declaration} inventory")
        return ()
    return tuple(re.findall(r'"([^"]+)"', match.group(1)))


data_roles_match = re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4\s*:\s*\[&str;\s*10\]\s*=\s*\[(.*?)\];",
    texts["data_model"],
    re.MULTILINE | re.DOTALL,
)
if data_roles_match is None:
    errors.append(f"{paths['data_model']}: missing exact [&str; 10] V4 role inventory")
else:
    data_roles = tuple(re.findall(r'"([^"]+)"', data_roles_match.group(1)))
    if data_roles != v4_roles:
        errors.append(f"{paths['data_model']}: V4 roles are not canonical: {data_roles!r}")

artifact_kind_match = re.search(
    r"pub\s+enum\s+KagemushaPastaCycleArtifactKindV4\s*\{(.*?)\n\s*\}",
    texts["data_model"],
    re.MULTILINE | re.DOTALL,
)
if artifact_kind_match is None:
    errors.append(f"{paths['data_model']}: missing V4 artifact-kind enum")
else:
    kind_cases = tuple(re.findall(r"^\s*([A-Z][A-Za-z0-9]*)\s*,", artifact_kind_match.group(1), re.MULTILINE))
    if kind_cases != (
        "Parameters", "CircuitParams", "ProvingKey", "VerifyingKey", "BootstrapWitness"
    ):
        errors.append(f"{paths['data_model']}: V4 artifact kinds are not canonical: {kind_cases!r}")

require_regex(
    "rust",
    r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4\s*:\s*usize\s*=\s*10\s*;",
    "exact ten-artifact bridge inventory",
)
require_regex(
    "bundle",
    r"const\s+INPUTS\s*:\s*\[InputSpec;\s*10\]\s*=\s*\[(.*?)\n\s*\];",
    "exact [InputSpec; 10] bundle inventory",
)
bundle_inputs_match = re.search(
    r"const\s+INPUTS\s*:\s*\[InputSpec;\s*10\]\s*=\s*\[(.*?)\n\s*\];",
    texts["bundle"],
    re.MULTILINE | re.DOTALL,
)
if bundle_inputs_match is not None:
    bundle_kinds = tuple(re.findall(r"kind:\s*KagemushaPastaCycleArtifactKindV4::([A-Za-z0-9]+)", bundle_inputs_match.group(1)))
    expected_kinds = (
        "Parameters", "CircuitParams", "ProvingKey", "VerifyingKey", "BootstrapWitness",
        "Parameters", "CircuitParams", "ProvingKey", "VerifyingKey", "BootstrapWitness",
    )
    if bundle_kinds != expected_kinds:
        errors.append(f"{paths['bundle']}: V4 bundle kind order is not canonical: {bundle_kinds!r}")

require_regex(
    "kagami",
    r"REPORT_ARTIFACT_PURPOSES_V4\s*:\s*\[&str;\s*10\]",
    "exact Kagami [&str; 10] report inventory",
)
require_regex(
    "kagami",
    r"\]:\s*\[KagemushaValidatedArtifactPayloadV4;\s*10\]\s*=",
    "exact Kagami [PayloadV4; 10] validated inventory",
)

swift_roles = quoted_inventory(
    "swift",
    r"public\s+static\s+let\s+artifactRolesV4\s*=\s*\[(.*?)\n\s*\]",
    "artifactRolesV4",
)
if swift_roles != v4_roles:
    errors.append(f"{paths['swift']}: V4 roles are not canonical: {swift_roles!r}")
swift_files = quoted_inventory(
    "swift",
    r"public\s+static\s+let\s+artifactFileNamesV4\s*=\s*\[(.*?)\n\s*\]",
    "artifactFileNamesV4",
)
if swift_files != v4_files:
    errors.append(f"{paths['swift']}: V4 files are not canonical: {swift_files!r}")
kotlin_files = quoted_inventory(
    "kotlin",
    r"V4_ARTIFACT_FILES\s*:\s*List<String>\s*=\s*listOf\((.*?)\n\s*\)",
    "V4_ARTIFACT_FILES",
)
if kotlin_files != v4_files:
    errors.append(f"{paths['kotlin']}: V4 files are not canonical: {kotlin_files!r}")
java_files = quoted_inventory(
    "java",
    r"V4_ARTIFACT_FILES\s*=\s*Collections\.unmodifiableList\(Arrays\.asList\((.*?)\)\s*\);",
    "V4_ARTIFACT_FILES",
)
if java_files != v4_files:
    errors.append(f"{paths['java']}: V4 files are not canonical: {java_files!r}")

swift_role_enum = re.search(
    r"public\s+enum\s+KagemushaRecursiveSpendArtifactRoleV4\b(.*?)public\s+var\s+fileName",
    texts["swift"],
    re.MULTILINE | re.DOTALL,
)
if swift_role_enum is None:
    errors.append(f"{paths['swift']}: missing V4 artifact-role enum")
else:
    swift_cases = tuple(re.findall(r"\bcase\s+([A-Za-z][A-Za-z0-9]*)", swift_role_enum.group(1)))
    if swift_cases != v4_case_names:
        errors.append(f"{paths['swift']}: V4 enum order is not canonical: {swift_cases!r}")

for label, pattern in (
    ("kotlin", r"enum\s+class\s+ArtifactRoleV4\b.*?\{(.*?)\n\s*\}"),
    ("java", r"enum\s+ArtifactRoleV4\b.*?\{(.*?)\n\s*\}"),
):
    enum_match = re.search(pattern, texts[label], re.MULTILINE | re.DOTALL)
    if enum_match is None:
        errors.append(f"{paths[label]}: missing V4 artifact-role enum")
        continue
    enum_files = tuple(re.findall(r'\b[A-Z][A-Z0-9_]*\s*\(\s*"([^"]+)"\s*\)', enum_match.group(1)))
    if enum_files != v4_files:
        errors.append(f"{paths[label]}: V4 enum files are not canonical: {enum_files!r}")

release_pairs = tuple(re.findall(
    r'"role":\s*"(step_(?:eq|ep)_[^"]+)"[\s\S]{0,220}?"file_name":\s*"([^"]+)"',
    texts["xcframework_build"],
))
if release_pairs != tuple(zip(v4_roles, v4_files)):
    errors.append(
        f"{paths['xcframework_build']}: ABI20 release artifact metadata is not canonical: "
        f"{release_pairs!r}"
    )

for label in ("data_model", "rust", "kagami", "bundle", "core_artifact"):
    for required in ("step_eq_circuit_params", "step_ep_circuit_params"):
        require(label, required)

for label in ("data_model", "swift", "kotlin", "java", "xcframework_build"):
    for required in (
        "step-eq.circuit-params.krv4",
        "step-ep.circuit-params.krv4",
    ):
        require(label, required)

for label in ("data_model", "rust", "bundle", "core_artifact"):
    require(label, "CircuitParams")

require_regex(
    "header",
    r"exact\s+ten-artifact\s+KRV4\s+inventory",
    "exact-ten ABI20 inventory documentation",
)
require_regex(
    "java",
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*=\s*20\s*;",
    "exact ABI20 Java constant",
)
require_regex(
    "kotlin",
    r"V4_ARTIFACT_COUNT\s*:\s*Int\s*=\s*10\b",
    "exact ten-artifact Kotlin inventory",
)
require_regex(
    "java",
    r"V4_ARTIFACT_COUNT\s*=\s*10\s*;",
    "exact ten-artifact Java inventory",
)

for method in native_methods:
    require_regex(
        "kotlin",
        rf"external\s+fun\s+{re.escape(method)}\s*\(",
        f"Kotlin native declaration {method}",
    )
    require_regex(
        "java",
        rf"native\s+[^;\n]+\s+{re.escape(method)}\s*\(",
        f"Java native declaration {method}",
    )
    for package in (
        "org_hyperledger_iroha_sdk_offline",
        "org_hyperledger_iroha_android_offline",
    ):
        require_regex(
            "rust",
            rf"fn\s+Java_{package}_KagemushaRecursiveSpendProver_{re.escape(method)}\s*\(",
            f"Rust JNI export {package}.{method}",
        )

c_symbols = (
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
)

require_regex(
    "rust",
    r"read_kagemusha_pasta_cycle_artifact_v4\s*\([\s\S]{0,220}?&self\.authenticated_release",
    "authenticated-release V4 artifact reads",
)
require_regex(
    "rust",
    r"KagemushaPastaCycleProverArtifactsV4::new\s*\(\s*&self\.authenticated_release,"
    r"[\s\S]{0,260}?Kind::Parameters\)\?,"
    r"[\s\S]{0,180}?Kind::CircuitParams\)\?,"
    r"[\s\S]{0,260}?Kind::ProvingKey\)\?,"
    r"[\s\S]{0,260}?Kind::VerifyingKey\)\?,"
    r"[\s\S]{0,260}?Kind::BootstrapWitness\)\?,"
    r"[\s\S]{0,260}?Kind::Parameters\)\?,"
    r"[\s\S]{0,180}?Kind::CircuitParams\)\?,"
    r"[\s\S]{0,260}?Kind::ProvingKey\)\?,"
    r"[\s\S]{0,260}?Kind::VerifyingKey\)\?,"
    r"[\s\S]{0,260}?Kind::BootstrapWitness\)\?,",
    "authenticated-release exact-ten V4 prover construction",
)
require_regex(
    "rust",
    r"for\s+profile\s+in\s+&manifest\.profiles[\s\S]{0,500}?authenticated_payload",
    "sequential authentication of all ten installed artifacts",
)

for symbol in c_symbols:
    require_regex("rust", rf"fn\s+{re.escape(symbol)}\s*\(", f"Rust C export {symbol}")
    require_regex("header", rf"\b{re.escape(symbol)}\s*\(", f"C header declaration {symbol}")
    for label in ("xcframework_build", "mobile_check", "mobile_check_test"):
        require(label, symbol)

for macro in (
    "CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_UNAVAILABLE",
    "CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_ARTIFACT",
):
    require("header", macro)

if errors:
    print("Kagemusha ABI20 SDK contract failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)

print(
    "Kagemusha ABI20 SDK contract passed: exact10 DM/Kagami/bundle inventory and "
    "distinct direct C/JNI/Swift/Kotlin/Java lifecycle parity are complete."
)
PY
