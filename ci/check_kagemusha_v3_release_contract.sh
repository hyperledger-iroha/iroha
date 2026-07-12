#!/usr/bin/env bash
set -euo pipefail

# Fail-before-build source gate for the one first-release Kagemusha corridor.
# It performs no network access and mutates no repository files. `--self-test`
# exercises adversarial in-memory mutations against every release invariant.

ROOT_DIR="${KAGEMUSHA_V3_RELEASE_CONTRACT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "$ROOT_DIR" "$MODE" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
overrides: dict[str, str] = {}

MODEL = "crates/iroha_data_model/src/offline/mod.rs"
PACKAGER = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v3_bundle.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
SWIFT_PROVER = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift"
SWIFT_V2 = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
SWIFT_NATIVE = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
SWIFT_BRIDGE = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
KOTLIN_PROVER = (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/"
    "KagemushaRecursiveSpendProver.kt"
)
JAVA_PROVER = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/"
    "KagemushaRecursiveSpendProver.java"
)
BUILD_XCFRAMEWORK = "scripts/build_norito_xcframework.sh"
CHECK_MOBILE_ARTIFACTS = "scripts/check_mobile_sdk_artifacts.sh"
CHECK_MOBILE_ARTIFACTS_TEST = "scripts/check_mobile_sdk_artifacts_test.sh"
SWIFT_README = "IrohaSwift/README.md"
V2_CONTRACT_DOC = "docs/source/offline_kagemusha_v2_contract.md"
RECURSION_DOC = "docs/source/offline_kagemusha_recursion_adapter.md"
WORKFLOW = ".github/workflows/pr_kagemusha_payload_bench.yml"

V3_C_SYMBOLS = (
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
)

RELEASE_SYMBOLS = (
    "connect_norito_bridge_abi_version",
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_topup_finality_verify_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
    "connect_norito_kagemusha_recursive_spend_init_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
    "connect_norito_kagemusha_recursive_spend_verify_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v2",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
    "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
    "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
)

ARTIFACT_CONSTANTS = frozenset(
    (
        "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3",
    )
)

RELEASE_FILES = frozenset(
    (
        "manifest.json",
        "manifest.norito",
        "manifest.norito.sha256",
        "transition-eq.parameters.krv3",
        "transition-eq.proving-key.krv3",
        "transition-eq.verifying-key.krv3",
        "state-ep.parameters.krv3",
        "state-ep.proving-key.krv3",
        "state-ep.verifying-key.krv3",
        "topup-finality-roster.norito",
    )
)


def read(relative: str) -> str:
    if relative in overrides:
        return overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def rust_string_constant(text: str, name: str) -> str | None:
    match = re.search(
        rf"\b(?:pub\s+)?const\s+{re.escape(name)}\s*:[^=]+\s*=\s*\"([^\"]+)\"\s*;",
        text,
        re.MULTILINE,
    )
    return None if match is None else match.group(1)


def add_missing(errors: list[str], relative: str, text: str, needles: tuple[str, ...]) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing `{needle}`")


def enum_body(text: str, pattern: str) -> str | None:
    match = re.search(pattern, text, re.DOTALL)
    return None if match is None else match.group("body")


def quoted_symbols(text: str) -> tuple[str, ...]:
    return tuple(re.findall(r'"(connect_norito_[a-z0-9_]+)"', text))


def check_release_contract() -> list[str]:
    errors: list[str] = []
    model = read(MODEL)
    packager = read(PACKAGER)
    bridge = read(BRIDGE)
    header = read(HEADER)

    if re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3\s*:\s*u32\s*=\s*18\s*;",
        model,
    ) is None:
        errors.append(f"{MODEL}: native bridge ABI must be exactly 18")
    if re.search(
        r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*18\s*;",
        bridge,
    ) is None:
        errors.append(f"{BRIDGE}: native bridge ABI must be exactly 18")
    if re.search(
        r"KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE\s*:\s*bool\s*=\s*false\s*;",
        model,
    ) is None:
        errors.append(f"{MODEL}: unavailable proof backend must remain truthfully false")
    for gate in (
        "KAGEMUSHA_RECURSIVE_SPEND_AUTHENTICATED_RELEASE_ENVELOPE_WIRED_V3",
        "KAGEMUSHA_RECURSIVE_SPEND_INIT_BINDS_TOPUP_FINALITY_V2",
    ):
        if re.search(rf"const\s+{gate}\s*:\s*bool\s*=\s*false\s*;", bridge) is None:
            errors.append(f"{BRIDGE}: first-release trust gate `{gate}` must remain fail-closed")
    add_missing(
        errors,
        BRIDGE,
        bridge,
        (
            "KAGEMUSHA_TOPUP_FINALITY_VERIFY_ENTRYPOINT_CALLABLE_V2: bool =",
            "kagemusha_topup_finality_entrypoint_callable_v2(",
            "authenticated_release_envelope && init_binds_topup_finality",
            "proof_backend",
            "&& kagemusha_topup_finality_entrypoint_callable_v2(",
            "if !KAGEMUSHA_TOPUP_FINALITY_VERIFY_ENTRYPOINT_CALLABLE_V2",
        ),
    )
    if rust_string_constant(model, "KAGEMUSHA_RECURSIVE_SPEND_MODE_V2") != "recursive_spend_v2":
        errors.append(f"{MODEL}: first-release mode must be exactly recursive_spend_v2")

    preferred = re.search(
        r"pub\s+const\s+fn\s+preferred_kagemusha_offline_spend_mode\(\s*"
        r"pasta_cycle_v3_backend_available:\s*bool,?\s*\)\s*"
        r"->\s*Option<&'static\s+str>\s*\{(?P<body>[\s\S]*?)\n\}",
        model,
    )
    if preferred is None:
        errors.append(f"{MODEL}: missing first-release mode selector")
    else:
        normalized = re.sub(r"\s+", " ", preferred.group("body")).strip()
        expected = (
            "if pasta_cycle_v3_backend_available { "
            "Some(KAGEMUSHA_RECURSIVE_SPEND_MODE_V2) } else { None }"
        )
        if normalized != expected:
            errors.append(f"{MODEL}: first-release selector must expose only recursive_spend_v2")

    input_match = re.search(
        r"const\s+INPUTS\s*:\s*&\[InputSpec\]\s*=\s*&\[(?P<body>[\s\S]*?)\n\];",
        packager,
    )
    artifact_constants = (
        set()
        if input_match is None
        else set(re.findall(r"file_name:\s*([A-Z][A-Z0-9_]+)", input_match.group("body")))
    )
    if artifact_constants != ARTIFACT_CONSTANTS:
        errors.append(f"{PACKAGER}: V3 INPUTS must contain exactly six canonical artifacts")
    files = {
        value
        for constant in artifact_constants
        if (value := rust_string_constant(model, constant)) is not None
    }
    for constant in (
        "MANIFEST_JSON_FILE_NAME",
        "MANIFEST_NORITO_FILE_NAME",
        "MANIFEST_NORITO_SHA256_FILE_NAME",
    ):
        value = rust_string_constant(packager, constant)
        if value is not None:
            files.add(value)
    roster = rust_string_constant(model, "KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2")
    if roster is not None:
        files.add(roster)
    if len(files) != 10 or files != RELEASE_FILES:
        errors.append(f"{PACKAGER}: release bundle must contain exactly ten canonical files")
    add_missing(
        errors,
        PACKAGER,
        packager,
        (
            "fn verify_inventory(&self) -> io::Result<()>",
            "if actual != expected.into_iter().map(str::to_owned).collect()",
            "publication file inventory is incomplete or excessive",
        ),
    )

    rust_v3 = set(
        re.findall(
            r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
            r'(connect_norito_kagemusha_recursive_spend_(?:capabilities_v1|artifact_[a-z0-9_]+_v3))\s*\(',
            bridge,
        )
    )
    header_v3 = set(
        re.findall(
            r'int32_t\s+'
            r'(connect_norito_kagemusha_recursive_spend_(?:capabilities_v1|artifact_[a-z0-9_]+_v3))\s*\(',
            header,
        )
    )
    if rust_v3 != set(V3_C_SYMBOLS):
        errors.append(f"{BRIDGE}: V3 C export inventory is not exact")
    if header_v3 != set(V3_C_SYMBOLS):
        errors.append(f"{HEADER}: V3 C declaration inventory is not exact")
    install_match = re.search(
        r"pub\s+unsafe\s+extern\s+\"C\"\s+fn\s+"
        r"connect_norito_kagemusha_recursive_spend_artifact_set_install_v3\s*\("
        r"(?P<body>[\s\S]*?)\n\}\n\n/// Report whether",
        bridge,
    )
    if install_match is None:
        errors.append(f"{BRIDGE}: missing atomic six-artifact install implementation")
    else:
        install_body = install_match.group("body")
        add_missing(
            errors,
            BRIDGE,
            install_body,
            (
                "handles_len != 6",
                "unique_handles.len() != 6",
                "expected_descriptors.len() != 6",
                "All fallible validation happens before any handle is removed",
                "validate_kagemusha_recursive_spend_artifact_spool_v3(&mut artifact_guard)?",
                "let removed = registry.remove(handle);",
                "*active = Some(installed);",
            ),
        )
        validation = install_body.find("validate_kagemusha_recursive_spend_artifact_spool_v3")
        removal = install_body.find("let removed = registry.remove(handle);")
        activation = install_body.find("*active = Some(installed);")
        if not (0 <= validation < removal < activation):
            errors.append(
                f"{BRIDGE}: install must validate all six files before consuming handles and activating"
            )
    retired_ingest = re.compile(
        r"connect_norito_kagemusha_recursive_spend_artifact_(?:begin|write|finalize|cancel)_v2"
    )
    if retired_ingest.search(bridge):
        errors.append(f"{BRIDGE}: retired V2 artifact ingest was reintroduced")
    if retired_ingest.search(header):
        errors.append(f"{HEADER}: retired V2 artifact ingest was reintroduced")

    retired_c_exports = (
        re.compile(r"connect_norito_kagemusha_[a-z0-9_]*compact[a-z0-9_]*\s*\("),
        re.compile(
            r"connect_norito_kagemusha_recursive_spend_"
            r"(?:init|topup|append|verify|redeem)\s*\("
        ),
    )
    for pattern in retired_c_exports:
        if pattern.search(bridge):
            errors.append(f"{BRIDGE}: retired compact or unsuffixed C export remains")
        if pattern.search(header):
            errors.append(f"{HEADER}: retired compact or unsuffixed C declaration remains")
    if "KagemushaRecursiveCompactPaymentTokenProver" in bridge:
        errors.append(f"{BRIDGE}: retired recursive-compact JNI exports remain")

    for package in ("sdk", "android"):
        for method in ("Begin", "Write", "Finalize", "Cancel"):
            symbol = (
                f"Java_org_hyperledger_iroha_{package}_offline_"
                f"KagemushaRecursiveSpendProver_nativeArtifact{method}V3"
            )
            if symbol not in bridge:
                errors.append(f"{BRIDGE}: missing JNI export `{symbol}`")
        for method in ("Install", "IsInstalled", "Uninstall"):
            symbol = (
                f"Java_org_hyperledger_iroha_{package}_offline_"
                f"KagemushaRecursiveSpendProver_nativeArtifactSet{method}V3"
            )
            if symbol not in bridge:
                errors.append(f"{BRIDGE}: missing JNI export `{symbol}`")

    swift_prover = read(SWIFT_PROVER)
    swift_mode = enum_body(
        swift_prover,
        r"public\s+enum\s+KagemushaOfflineSpendMode[^\{]*\{(?P<body>[\s\S]*?)\n\}",
    )
    swift_cases = set() if swift_mode is None else set(re.findall(r"case\s+(\w+)\s*=", swift_mode))
    if swift_cases != {"recursiveSpend"}:
        errors.append(f"{SWIFT_PROVER}: first-release mode enum must contain only recursiveSpend")
    add_missing(
        errors,
        SWIFT_PROVER,
        swift_prover,
        (
            "pastaCycleV3BackendAvailable ? .recursiveSpend : nil",
        ),
    )
    swift_v2 = read(SWIFT_V2)
    add_missing(
        errors,
        SWIFT_V2,
        swift_v2,
        (
            "requiredNativeBridgeAbiVersion: UInt32 = 18",
            'public static let mode = "recursive_spend_v2"',
            "public static func verifyTopUpFinality(",
            "anchor: KagemushaRecursiveSpendTopUpAnchor,",
            "anchorArchive: anchor.archive,",
            "case finalityTrustUnavailable",
            "catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable",
            "throw KagemushaRecursiveSpendError.finalityTrustUnavailable",
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
        ),
    )
    swift_native = read(SWIFT_NATIVE)
    add_missing(
        errors,
        SWIFT_NATIVE,
        swift_native,
        (
            "kagemushaRecursiveSpendArtifactBeginV3",
            "kagemushaRecursiveSpendArtifactWriteV3",
            "kagemushaRecursiveSpendArtifactFinalizeV3",
            "kagemushaRecursiveSpendArtifactCancelV3",
            "kagemushaRecursiveSpendArtifactSetInstallV3",
            "kagemushaRecursiveSpendArtifactSetIsInstalledV3",
            "kagemushaRecursiveSpendArtifactSetUninstallV3",
        ),
    )
    if re.search(
        r"func\s+kagemushaTopUpFinalityVerifyV2\(\s*"
        r"proofArchive:\s*Data,\s*rosterArtifactArchive:\s*Data,\s*"
        r"anchorArchive:\s*Data,\s*manifestArchive:\s*Data,\s*"
        r"expectedManifestSHA256:\s*Data\s*\)[\s\S]*?"
        r"anchorArchive\.withUnsafeBytes\s*\{\s*anchorBuffer\s+in[\s\S]*?"
        r"manifestArchive\.withUnsafeBytes\s*\{\s*manifestBuffer\s+in[\s\S]*?"
        r"anchorBuffer\.bindMemory\(to:\s*UInt8\.self\)\.baseAddress,[\s\S]*?"
        r"manifestBuffer\.bindMemory\(to:\s*UInt8\.self\)\.baseAddress,",
        swift_native,
        re.DOTALL,
    ) is None:
        errors.append(
            f"{SWIFT_NATIVE}: native finality wrapper must forward anchor before manifest"
        )
    swift_bridge = read(SWIFT_BRIDGE)
    bridge_abi_check = re.search(
        r"isKagemushaRecursiveSpendV2StubAvailable[\s\S]*?"
        r"loadedBridgeAbiVersion\s*(?P<operator>==|>=)\s*"
        r"KagemushaRecursiveSpend\.requiredNativeBridgeAbiVersion",
        swift_bridge,
    )
    if bridge_abi_check is None or bridge_abi_check.group("operator") != "==":
        errors.append(f"{SWIFT_BRIDGE}: recursive-spend native ABI check must be exact")

    kotlin = read(KOTLIN_PROVER)
    kotlin_mode = enum_body(
        kotlin,
        r"enum\s+class\s+Mode\([^\)]*\)\s*\{(?P<body>[\s\S]*?)\n\s*\}",
    )
    kotlin_cases = (
        set()
        if kotlin_mode is None
        else set(re.findall(r"^\s*([A-Z][A-Z0-9_]*)\(\"", kotlin_mode, re.MULTILINE))
    )
    if kotlin_cases != {"RECURSIVE_SPEND"}:
        errors.append(f"{KOTLIN_PROVER}: first-release mode enum must contain only RECURSIVE_SPEND")
    add_missing(
        errors,
        KOTLIN_PROVER,
        kotlin,
        (
            "REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 18",
            'MODE: String = "recursive_spend_v2"',
            "if (pastaCycleV3BackendAvailable) Mode.RECURSIVE_SPEND else null",
            "ArtifactIngest",
            "beginArtifactInstallSession",
            "ArtifactInstallSession",
            "nativeArtifactBeginV3",
            "nativeArtifactWriteV3",
            "nativeArtifactFinalizeV3",
            "nativeArtifactCancelV3",
            "nativeArtifactSetInstallV3",
            "nativeArtifactSetIsInstalledV3",
            "nativeArtifactSetUninstallV3",
        ),
    )

    java = read(JAVA_PROVER)
    java_mode = enum_body(
        java,
        r"public\s+enum\s+Mode\s*\{(?P<body>[\s\S]*?)\n\s*\}",
    )
    java_cases = (
        set()
        if java_mode is None
        else set(re.findall(r"^\s*([A-Z][A-Z0-9_]*)\(\"", java_mode, re.MULTILINE))
    )
    if java_cases != {"RECURSIVE_SPEND"}:
        errors.append(f"{JAVA_PROVER}: first-release mode enum must contain only RECURSIVE_SPEND")
    add_missing(
        errors,
        JAVA_PROVER,
        java,
        (
            "REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 18",
            'MODE = "recursive_spend_v2"',
            "return pastaCycleV3BackendAvailable ? Mode.RECURSIVE_SPEND : null;",
            "ArtifactIngest",
            "beginArtifactInstallSession",
            "ArtifactInstallSession",
            "nativeArtifactBeginV3",
            "nativeArtifactWriteV3",
            "nativeArtifactFinalizeV3",
            "nativeArtifactCancelV3",
            "nativeArtifactSetInstallV3",
            "nativeArtifactSetIsInstalledV3",
            "nativeArtifactSetUninstallV3",
        ),
    )

    build = read(BUILD_XCFRAMEWORK)
    build_inventory = re.search(
        r'"required_symbols"\s*:\s*\[(?P<body>[\s\S]*?)\n\s*\],',
        build,
    )
    build_symbols = () if build_inventory is None else quoted_symbols(build_inventory.group("body"))
    if build_symbols != RELEASE_SYMBOLS:
        errors.append(f"{BUILD_XCFRAMEWORK}: release symbol inventory is not exact")
    add_missing(
        errors,
        BUILD_XCFRAMEWORK,
        build,
        ('if [[ "$BRIDGE_ABI_VERSION" != "18" ]]',),
    )

    checker = read(CHECK_MOBILE_ARTIFACTS)
    checker_inventory = re.search(
        r"local\s+required_symbols=\((?P<body>[\s\S]*?)\n\s*\)",
        checker,
    )
    checker_symbols = (
        ()
        if checker_inventory is None
        else tuple(
            re.findall(r"^\s*(connect_norito_[a-z0-9_]+)\s*$", checker_inventory.group("body"), re.MULTILINE)
        )
    )
    if checker_symbols != RELEASE_SYMBOLS:
        errors.append(f"{CHECK_MOBILE_ARTIFACTS}: release symbol inventory is not exact")
    add_missing(
        errors,
        CHECK_MOBILE_ARTIFACTS,
        checker,
        ("exact first-release NoritoBridge ABI 18",),
    )

    checker_test = read(CHECK_MOBILE_ARTIFACTS_TEST)
    fixture_inventory = re.search(
        r'"required_symbols"\s*:\s*\[(?P<body>[\s\S]*?)\n\s*\],',
        checker_test,
    )
    fixture_symbols = () if fixture_inventory is None else quoted_symbols(fixture_inventory.group("body"))
    if fixture_symbols != RELEASE_SYMBOLS:
        errors.append(f"{CHECK_MOBILE_ARTIFACTS_TEST}: fixture symbol inventory is not exact")
    add_missing(
        errors,
        CHECK_MOBILE_ARTIFACTS_TEST,
        checker_test,
        (
            "wrong-bridge-abi",
            "legacy-symbol-reintroduced",
        ),
    )

    swift_readme = read(SWIFT_README)
    add_missing(
        errors,
        SWIFT_README,
        swift_readme,
        (
            "The first release exposes one Kagemusha production mode:",
            "`recursive_spend_v2`",
            "exact native bridge ABI 18",
            "The release directory has exactly ten files:",
            "`manifest.norito` as the canonical runtime object",
            "JSON is an operator view, not a trust anchor.",
            "KagemushaRecursiveSpendArtifactInstallSessionV3",
            "Successful ingestion alone does not advertise proof readiness.",
            "ambiguous terminal outcome rather than submitting the same operation again",
        ),
    )
    for retired in ("recursive_spend_v1", "recursive_compact_v1", "ABI 6", "ABI 7"):
        if retired in swift_readme:
            errors.append(f"{SWIFT_README}: publishes retired release selection `{retired}`")
    for relative in (V2_CONTRACT_DOC, RECURSION_DOC):
        text = read(relative)
        if "recursive_spend_v2" not in text:
            errors.append(f"{relative}: missing recursive_spend_v2 release mode")
        if "recursive_spend_v1" in text:
            errors.append(f"{relative}: publishes retired recursive_spend_v1 release mode")

    workflow = read(WORKFLOW)
    add_missing(
        errors,
        WORKFLOW,
        workflow,
        (
            '"ci/check_kagemusha_v3_release_contract.sh"',
            "ci/check_kagemusha_v3_release_contract.sh --self-test",
            "ci/check_kagemusha_v3_release_contract.sh\n",
        ),
    )
    return errors


def replace(relative: str, old: str, new: str) -> None:
    source = read(relative)
    if old not in source:
        raise SystemExit(f"self-test setup failed: `{old}` not found in {relative}")
    overrides[relative] = source.replace(old, new, 1)


def run_self_test() -> None:
    baseline = check_release_contract()
    if baseline:
        raise SystemExit("self-test requires a green baseline:\n" + "\n".join(baseline))
    cases = (
        (
            "bridge ABI downgrade",
            BRIDGE,
            "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 18;",
            "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 17;",
            "native bridge ABI must be exactly 18",
        ),
        (
            "premature proof-backend advertisement",
            MODEL,
            "KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE: bool = false;",
            "KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE: bool = true;",
            "unavailable proof backend must remain truthfully false",
        ),
        (
            "unauthenticated release activation",
            BRIDGE,
            "KAGEMUSHA_RECURSIVE_SPEND_AUTHENTICATED_RELEASE_ENVELOPE_WIRED_V3: bool = false;",
            "KAGEMUSHA_RECURSIVE_SPEND_AUTHENTICATED_RELEASE_ENVELOPE_WIRED_V3: bool = true;",
            "must remain fail-closed",
        ),
        (
            "unbound top-up finality activation",
            BRIDGE,
            "KAGEMUSHA_RECURSIVE_SPEND_INIT_BINDS_TOPUP_FINALITY_V2: bool = false;",
            "KAGEMUSHA_RECURSIVE_SPEND_INIT_BINDS_TOPUP_FINALITY_V2: bool = true;",
            "must remain fail-closed",
        ),
        (
            "mode substitution",
            MODEL,
            'KAGEMUSHA_RECURSIVE_SPEND_MODE_V2: &str = "recursive_spend_v2";',
            'KAGEMUSHA_RECURSIVE_SPEND_MODE_V2: &str = "unsupported_mode";',
            "first-release mode must be exactly recursive_spend_v2",
        ),
        (
            "legacy Swift mode reintroduction",
            SWIFT_PROVER,
            '    case recursiveSpend = "recursive_spend_v2"',
            '    case recursiveSpend = "recursive_spend_v2"\n'
            '    case recursiveSpendV1 = "recursive_spend_v1"',
            "mode enum must contain only recursiveSpend",
        ),
        (
            "missing Swift finality anchor forwarding",
            SWIFT_NATIVE,
            "anchorArchive.withUnsafeBytes { anchorBuffer in",
            "manifestArchive.withUnsafeBytes { anchorBuffer in",
            "native finality wrapper must forward anchor before manifest",
        ),
        (
            "eleventh release file",
            PACKAGER,
            'const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";',
            'const MANIFEST_JSON_FILE_NAME: &str = "manifest-extra.json";',
            "release bundle must contain exactly ten canonical files",
        ),
        (
            "missing V3 ingest symbol",
            HEADER,
            "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_finalize_removed",
            "V3 C declaration inventory is not exact",
        ),
        (
            "legacy release symbol",
            BUILD_XCFRAMEWORK,
            '    "connect_norito_kagemusha_recursive_spend_init_v2",',
            '    "connect_norito_kagemusha_recursive_spend_init",\n'
            '    "connect_norito_kagemusha_recursive_spend_init_v2",',
            "release symbol inventory is not exact",
        ),
        (
            "operator-view trust substitution",
            SWIFT_README,
            "JSON is an operator view, not a trust anchor.",
            "JSON is the runtime trust anchor.",
            "missing `JSON is an operator view, not a trust anchor.`",
        ),
    )
    for label, relative, old, new, expected in cases:
        overrides.clear()
        replace(relative, old, new)
        errors = check_release_contract()
        if not any(expected in error for error in errors):
            raise SystemExit(f"self-test failed to reject {label}: " + " | ".join(errors))
    overrides.clear()
    print(f"Kagemusha ABI-18/V3 release contract self-test rejected {len(cases)} adversarial mutations")


if mode == "--self-test":
    run_self_test()
elif mode:
    raise SystemExit(f"unknown mode: {mode}")
else:
    failures = check_release_contract()
    if failures:
        raise SystemExit("\n".join(failures))
    print("Kagemusha first-release contract is exact: ABI 18, recursive_spend_v2, ten files, V3 ingest only")
PY
