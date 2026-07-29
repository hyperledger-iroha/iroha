#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"
PYTHON_BIN="${PRIVACY_SDK_GUARD_PYTHON_BIN:-python3}"

"${PYTHON_BIN}" - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

import ast
import hashlib
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]

MATRIX_RELATIVE = "fixtures/privacy/exact12_v1.tsv"
MATRIX_BYTES = (root / MATRIX_RELATIVE).read_bytes()
MATRIX_TEXT = MATRIX_BYTES.decode("utf-8", errors="strict")


def matrix_rows(kind: str) -> tuple[tuple[str, ...], ...]:
    rows = []
    for line_number, line in enumerate(MATRIX_TEXT.splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = tuple(line.split("\t"))
        if fields[0] == kind:
            rows.append(fields)
        elif fields[0] not in {
            "matrix-version",
            "registry-sha256",
            "protocol",
            "typed-envelope",
            "retired",
        }:
            raise RuntimeError(f"unknown matrix row {line_number}: {fields[0]}")
    return tuple(rows)


PROTOCOL_ROWS = matrix_rows("protocol")
TYPED_ENVELOPE_ROWS = matrix_rows("typed-envelope")
EXPECTED_IDS = tuple(row[2] for row in PROTOCOL_ROWS)
RETIRED_IDS = tuple(row[1] for row in matrix_rows("retired"))

RETIRED_PUBLIC_SYMBOLS = (
    "privacyProofRequestV1",
    "privacyBuildProofV1",
    "privacyVerifyProofV1",
    "privacy_proof_request_v1",
    "privacy_build_proof_v1",
    "privacy_verify_proof_v1",
    "getPrivacyAlgorithmDescriptor",
    "getPrivacyAlgorithmDescriptors",
    "getPrivacyCapabilities",
    "getPrivacyCriteria",
    "buildPrivacyProofEnvelope",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "PrivacyProofRequestV1",
    "PrivacyProofResultV1",
    "nativeProofRequest",
    "nativeBuildProof",
    "nativeVerifyProof",
)

DELETED_PYTHON_MODULES = (
    "anonymous_pgc.py",
    "jindo.py",
    "research_adapters.py",
    "silent_threshold.py",
    "sis_hints.py",
    "vega.py",
    "verange.py",
    "zk_ams.py",
    "zk_x509.py",
    "zkat.py",
)


class GuardFailure(RuntimeError):
    pass


def read(relative: str, overrides: dict[str, str]) -> str:
    if relative in overrides:
        return overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def literal_assignment(source: str, name: str):
    tree = ast.parse(source)
    for node in tree.body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            target = node.targets[0] if isinstance(node, ast.Assign) else node.target
            if isinstance(target, ast.Name) and target.id == name:
                return ast.literal_eval(node.value)
    raise GuardFailure(f"missing Python assignment {name}")


def js_protocol_ids(source: str) -> tuple[str, ...]:
    match = re.search(
        r"export const PRIVACY_PROTOCOL_IDS_V1 = Object\.freeze\(\[([\s\S]*?)\]\);",
        source,
    )
    if match is None:
        raise GuardFailure("missing JavaScript PRIVACY_PROTOCOL_IDS_V1")
    return tuple(re.findall(r'"([^"]+)"', match.group(1)))


def unique_expected_ids_in_source(source: str) -> tuple[str, ...]:
    matches = re.findall(
        r'"(' + "|".join(re.escape(value) for value in EXPECTED_IDS) + r')"',
        source,
    )
    return tuple(dict.fromkeys(matches))


def check(overrides: dict[str, str] | None = None) -> None:
    overrides = overrides or {}
    errors: list[str] = []

    version_rows = matrix_rows("matrix-version")
    registry_rows = matrix_rows("registry-sha256")
    require(
        MATRIX_TEXT.endswith("\n")
        and "\r" not in MATRIX_TEXT
        and all(MATRIX_TEXT.split("\n")[:-1]),
        "exact12 matrix must use non-empty canonical LF lines and end with LF",
        errors,
    )
    require(
        version_rows == (("matrix-version", "1"),),
        "exact12 matrix must declare only version 1",
        errors,
    )
    require(
        len(PROTOCOL_ROWS) == 12
        and all(
            len(row) == 5 and row[1] == str(index)
            for index, row in enumerate(PROTOCOL_ROWS)
        )
        and len(set(EXPECTED_IDS)) == 12,
        "exact12 matrix must contain exactly 12 unique indexed protocol routes",
        errors,
    )
    registry_preimage = "".join(f"{protocol_id}\n" for protocol_id in EXPECTED_IDS)
    registry_digest = hashlib.sha256(registry_preimage.encode("utf-8")).hexdigest()
    require(
        registry_rows == (("registry-sha256", registry_digest),),
        "exact12 matrix registry digest does not bind its ordered protocol rows",
        errors,
    )
    require(
        len(TYPED_ENVELOPE_ROWS) == 12
        and all(len(row) == 6 for row in TYPED_ENVELOPE_ROWS)
        and tuple(row[1:4] for row in TYPED_ENVELOPE_ROWS)
        == tuple(row[2:5] for row in PROTOCOL_ROWS)
        and all(
            re.fullmatch(r"[0-9a-f]{64}", digest) is not None
            and digest != "0" * 64
            for row in TYPED_ENVELOPE_ROWS
            for digest in row[4:]
        ),
        "exact12 matrix must bind non-zero typed envelopes for all 12 canonical routes",
        errors,
    )
    require(
        len(RETIRED_IDS) == len(set(RETIRED_IDS))
        and all(protocol_id not in EXPECTED_IDS for protocol_id in RETIRED_IDS),
        "exact12 matrix retired IDs must be unique and outside the registry",
        errors,
    )

    js_source = read("javascript/iroha_js/src/privacyCapabilities.js", overrides)
    js_dist = read("javascript/iroha_js/dist/privacyCapabilities.js", overrides)
    py_catalog = read(
        "python/iroha_python/src/iroha_python/privacy_catalog.py", overrides
    )
    rust_model = read("crates/iroha_data_model/src/privacy.rs", overrides)
    js_crypto = read("javascript/iroha_js/src/crypto.js", overrides)
    py_crypto = read(
        "python/iroha_python/src/iroha_python/crypto.py", overrides
    )
    py_native = read(
        "python/iroha_python/iroha_python_rs/src/lib.rs", overrides
    )

    for relative, source, markers in (
        (
            "crates/iroha_data_model/src/privacy.rs",
            rust_model,
            (
                "TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_ACTION_BYTES_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1: u32 = 18 * 1024 * 1024",
            ),
        ),
        (
            "javascript/iroha_js/src/privacyCapabilities.js",
            js_source,
            (
                "max_proof_bytes_per_action: 9 * 1024 * 1024",
                "max_action_bytes: 9 * 1024 * 1024",
                "max_privacy_bytes_per_transaction: 9 * 1024 * 1024",
                "max_privacy_bytes_per_block: 18 * 1024 * 1024",
            ),
        ),
        (
            "python/iroha_python/src/iroha_python/privacy_catalog.py",
            py_catalog,
            (
                '"max_proof_bytes_per_action": 9 * 1024 * 1024',
                '"max_action_bytes": 9 * 1024 * 1024',
                '"max_privacy_bytes_per_transaction": 9 * 1024 * 1024',
                '"max_privacy_bytes_per_block": 18 * 1024 * 1024',
            ),
        ),
    ):
        require(
            all(marker in source for marker in markers),
            f"{relative} must pin the first-release 9 MiB action/transaction and 18 MiB block privacy ceilings",
            errors,
        )

    require(
        re.search(
            r"PRIVACY_REQUIRED_BRIDGE_ABI_VERSION\s*=\s*21\s*;",
            js_crypto,
        )
        is not None
        and "abiVersion === PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" in js_crypto
        and "abiVersion >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" not in js_crypto,
        "JavaScript privacy bridge must require exact first-release ABI 21",
        errors,
    )
    require(
        literal_assignment(py_crypto, "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION") == 21
        and "version == PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" in py_crypto
        and "version >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" not in py_crypto,
        "Python privacy bridge must require exact first-release ABI 21",
        errors,
    )
    require(
        "fn privacy_bridge_abi_version_py() -> u32" in py_native
        and "PRIVACY_BRIDGE_ABI_VERSION_V1" in py_native,
        "Python native privacy bridge must report first-release ABI 21",
        errors,
    )
    for relative, marker in (
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            "REQUIRED_BRIDGE_ABI_VERSION = 21",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "REQUIRED_BRIDGE_ABI_VERSION: Int = 21",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
            "requiredBridgeABIVersion: UInt32 = 21",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
            "RequiredBridgeAbiVersion = 21",
        ),
    ):
        require(
            marker in read(relative, overrides),
            f"{relative} must require exact first-release ABI 21",
            errors,
        )

    require(
        js_protocol_ids(js_source) == EXPECTED_IDS,
        "JavaScript source capability registry must contain the exact 12 IDs in order",
        errors,
    )
    require(
        js_protocol_ids(js_dist) == EXPECTED_IDS,
        "JavaScript dist capability registry must contain the exact 12 IDs in order",
        errors,
    )
    require(
        tuple(literal_assignment(py_catalog, "PRIVACY_PROTOCOL_IDS_V1"))
        == EXPECTED_IDS,
        "Python capability registry must contain the exact 12 IDs in order",
        errors,
    )
    require(
        rust_model.count("pub const COUNT: usize = 12;") == 1,
        "Rust PrivacyProtocolIdV1::COUNT must remain exactly 12",
        errors,
    )
    positions = [rust_model.find(f'"{protocol_id}"') for protocol_id in EXPECTED_IDS]
    require(
        all(position >= 0 for position in positions)
        and positions == sorted(positions),
        "Rust canonical privacy labels must include the exact IDs in order",
        errors,
    )

    for marker in (
        "objectWithExactKeys",
        "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
        "PRIVACY_PROTOCOL_IDS_V1.length",
        "Object.freeze",
    ):
        require(marker in js_source, f"JavaScript strict parser lost {marker}", errors)
    for marker in (
        "_exact_object",
        "reject_duplicate_pairs",
        "PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1",
        "type(value) is not int",
    ):
        require(marker in py_catalog, f"Python strict parser lost {marker}", errors)

    require(
        read("javascript/iroha_js/src/privacyCapabilities.js", overrides)
        == read("javascript/iroha_js/dist/privacyCapabilities.js", overrides),
        "JavaScript capability source and dist must be byte-identical",
        errors,
    )
    for name in ("index.js", "crypto.js", "crypto.browser.js"):
        require(
            read(f"javascript/iroha_js/src/{name}", overrides)
            == read(f"javascript/iroha_js/dist/{name}", overrides),
            f"JavaScript {name} source and dist must be byte-identical",
            errors,
        )

    public_files = (
        "javascript/iroha_js/src/index.js",
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/dist/index.js",
        "javascript/iroha_js/dist/crypto.js",
        "javascript/iroha_js/dist/crypto.browser.js",
        "javascript/iroha_js/index.d.ts",
        "python/iroha_python/src/iroha_python/__init__.py",
        "python/iroha_python/src/iroha_python/crypto.py",
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    )
    for relative in public_files:
        source = read(relative, overrides)
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} must not expose retired generic symbol {symbol}",
                errors,
            )

    mobile_capability_files = (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
        "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
        "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    )
    for relative in mobile_capability_files:
        source = read(relative, overrides)
        require(
            unique_expected_ids_in_source(source) == EXPECTED_IDS,
            f"{relative} must expose the exact 12 canonical IDs in order",
            errors,
        )
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} must not expose retired generic symbol {symbol}",
                errors,
            )
        for protocol_id in RETIRED_IDS:
            require(
                protocol_id not in source,
                f"{relative} must not accept retired ID {protocol_id}",
                errors,
            )
        for marker in (
            "capabilities",
            "ProtocolIdV1",
            "unknown",
        ):
            require(
                marker.lower() in source.lower(),
                f"{relative} lost closed capability marker {marker}",
                errors,
            )

    validator_markers = (
        (
            "javascript/iroha_js/src/crypto.js",
            "privacyValidateCapabilitiesV1",
        ),
        (
            "python/iroha_python/src/iroha_python/crypto.py",
            "privacy_validate_capabilities_v1",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            "nativeValidateCapabilities",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "nativeValidateCapabilities",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
            "iroha_privacy_validate_capabilities_v1",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
            "iroha_privacy_validate_capabilities_v1",
        ),
    )
    for relative, marker in validator_markers:
        source = read(relative, overrides)
        require(
            marker in source,
            f"{relative} must call the shared Rust typed capability validator",
            errors,
        )
        require(
            "0x50" not in source
            and "CAPABILITY_SCHEMA_BYTE" not in source
            and "capabilitySchemaByte" not in source
            and "SchemaByte = 0x50" not in source,
            f"{relative} must not retain the fabricated repeated-byte schema gate",
            errors,
        )

    require(
        "pub fn validate_privacy_capability_archive_v1" in rust_model
        and "decode_canonical_with_limits::<PrivacyCapabilitySnapshotV1>" in rust_model
        and "PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024" in rust_model
        and "snapshot.validate().is_err()" in rust_model,
        "Rust data model must own the bounded canonical typed capability validator",
        errors,
    )
    for relative in (
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
        "crates/connect_norito_bridge/src/lib.rs",
    ):
        source = read(relative, overrides)
        require(
            "validate_privacy_capability_archive_v1" in source,
            f"{relative} must call the shared Rust capability validator directly",
            errors,
        )
        require(
            "PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE" not in source
            and "privacy_patch_archive_schema_hash" not in source
            and "privacy_patch_archive_repeated_schema_byte" not in source,
            f"{relative} must not rewrite the canonical Norito schema hash",
            errors,
        )

    capability_only_native_files = (
        "crates/connect_norito_bridge/src/lib.rs",
        "crates/connect_norito_bridge/include/connect_norito_bridge.h",
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyConfidentialWitness.java",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyConfidentialWitness.kt",
        "IrohaSwift/Sources/IrohaSwift/PrivacyConfidentialWitness.swift",
        "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    )
    for relative in capability_only_native_files:
        source = read(relative, overrides)
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} retains retired generic privacy route {symbol}",
                errors,
            )

    c_header = read(
        "crates/connect_norito_bridge/include/connect_norito_bridge.h", overrides
    )
    require(
        set(re.findall(r"\b(iroha_privacy_[a-z0-9_]+)\s*\(", c_header))
        == {
            "iroha_privacy_capabilities_v1",
            "iroha_privacy_validate_capabilities_v1",
            "iroha_privacy_free_buffer",
        },
        "C privacy ABI must contain only capability snapshot, typed validator, and zeroizing free",
        errors,
    )
    cli_root = root / "crates/iroha_cli"
    if cli_root.exists():
        cli_source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in cli_root.rglob("*.rs")
        )
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", cli_source) is None,
                f"Rust CLI must fail closed instead of exposing {symbol}",
                errors,
            )

    for relative in (
        "javascript/iroha_js/src/privacyCapabilities.js",
        "javascript/iroha_js/dist/privacyCapabilities.js",
        "python/iroha_python/src/iroha_python/privacy_catalog.py",
    ):
        source = read(relative, overrides)
        for protocol_id in RETIRED_IDS:
            require(
                protocol_id not in source,
                f"{relative} must not accept retired ID {protocol_id}",
                errors,
            )

    for relative in (
        "crates/connect_norito_bridge/src/lib.rs",
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    ):
        source = read(relative, overrides)
        for marker in ("PrivacyCapabilitySnapshotV1", "PrivacyProtocolIdV1::ALL"):
            require(marker in source, f"{relative} lost typed snapshot marker {marker}", errors)
        for marker in (
            "struct PrivacyAlgorithmEntry",
            "struct PrivacyCapabilitiesV1",
            "PRIVACY_ALGORITHM_ENTRIES",
        ):
            require(marker not in source, f"{relative} retains legacy catalog {marker}", errors)

    require(
        "mod privacy_production;" not in read(
            "crates/connect_norito_bridge/src/lib.rs", overrides
        ),
        "connect bridge must not compile the retired generic production dispatcher",
        errors,
    )
    require(
        not (root / "crates/connect_norito_bridge/src/privacy_production.rs").exists(),
        "retired connect privacy_production.rs must remain deleted",
        errors,
    )
    require(
        not (root / "javascript/iroha_js/src/privacyAlgorithms.js").exists()
        and not (root / "javascript/iroha_js/dist/privacyAlgorithms.js").exists(),
        "retired JavaScript editorial privacy catalog must remain deleted",
        errors,
    )
    for module in DELETED_PYTHON_MODULES:
        require(
            not (
                root / "python/iroha_python/src/iroha_python" / module
            ).exists(),
            f"retired Python module {module} must remain deleted",
            errors,
        )

    js_tests = read("javascript/iroha_js/test/privacyCatalogParity.test.js", overrides)
    py_tests = read("python/iroha_python/tests/privacy_catalog_test.py", overrides)
    matrix_consumers = (
        "crates/iroha_data_model/src/privacy.rs",
        "javascript/iroha_js/test/privacyCatalogParity.test.js",
        "python/iroha_python/tests/privacy_catalog_test.py",
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
        "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
    )
    for relative in matrix_consumers:
        require(
            "exact12_v1.tsv" in read(relative, overrides),
            f"{relative} must consume the shared exact12 matrix",
            errors,
        )
    for marker in ("unknown fields", "aliases", "canonical 12"):
        require(
            marker.lower() in js_tests.lower() or marker.lower() in py_tests.lower(),
            f"strict SDK tests must retain {marker} coverage",
            errors,
        )
    require(
        "duplicate" in py_tests.lower() and "NaN" in py_tests,
        "Python tests must retain duplicate-key and non-finite-number rejection",
        errors,
    )

    workflow_source = read(
        ".github/workflows/pr_privacy_sdk_guard.yml", overrides
    )
    lock_helper_source = read("ci/privacy_sdk_cargo_lockfile.sh", overrides)
    cargo_wrapper_source = read("ci/privacy_sdk_cargo_wrapper.sh", overrides)
    cargo_jobs = {
        "privacy_native_bridge_tests": (
            "run: cargo test -p connect_norito_bridge privacy_ --lib -- --test-threads=1"
        ),
        "privacy_python_sdk_tests": "run: ci/check_privacy_python_sdk.sh",
        "privacy-sdk-guard": "run: ci/check_privacy_sdk_guard.sh",
    }
    require(
        workflow_source.count(
            "ci/privacy_sdk_cargo_lockfile.sh provision-ci"
        )
        == len(cargo_jobs)
        and workflow_source.count(
            "ci/privacy_sdk_cargo_lockfile.sh verify-ci"
        )
        == 2 * len(cargo_jobs),
        "every privacy workflow Cargo job must provision once and verify twice",
        errors,
    )
    for job, consumer in cargo_jobs.items():
        match = re.search(
            rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            workflow_source,
        )
        block = "" if match is None else match.group(1)
        markers = (
            "Provision private privacy SDK Cargo lock",
            "Swatinem/rust-cache@",
            "Verify privacy SDK Cargo lock isolation",
            consumer,
            "Verify final privacy SDK Cargo lock isolation",
        )
        positions = tuple(block.find(marker) for marker in markers)
        require(
            match is not None
            and block.count("provision-ci") == 1
            and block.count("verify-ci") == 2
            and -1 not in positions
            and positions == tuple(sorted(positions))
            and "if: always()" in block[positions[-1] :],
            f"{job} must bracket rust-cache and Cargo work with external-lock isolation",
            errors,
        )
    require(
        'RUSTC_BOOTSTRAP=1 "${real_cargo}" -Z unstable-options generate-lockfile'
        in lock_helper_source
        and '--lockfile-path "${lock_path}"' in lock_helper_source
        and "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE=absent"
        in lock_helper_source
        and 'printf \'%s\\n\' "${cargo_wrapper_directory}" >>"${github_path_path}"'
        in lock_helper_source,
        "privacy SDK CI helper must generate only the authenticated external lock and install its wrapper",
        errors,
    )
    require(
        "run_real_cargo_and_verify_locks" in cargo_wrapper_source
        and cargo_wrapper_source.count("assert_authenticated_cargo_lock_state")
        >= 3
        and "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE"
        in cargo_wrapper_source,
        "privacy SDK Cargo wrapper must verify external and workspace locks before and after Cargo",
        errors,
    )

    if errors:
        raise GuardFailure("\n".join(f"- {error}" for error in errors))


if mode:
    if not mode.startswith("--negative-control-"):
        raise SystemExit(f"unknown mode: {mode}")
    try:
        check()
    except (GuardFailure, SyntaxError, ValueError) as error:
        raise SystemExit(
            "negative control requires a valid canonical baseline:\n" + str(error)
        ) from error
    overrides: dict[str, str] = {}
    if mode in {
        "--negative-control-cargo-lock-native-workflow",
        "--negative-control-cargo-lock-python-workflow",
        "--negative-control-cargo-lock-guard-workflow",
    }:
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        job = {
            "--negative-control-cargo-lock-native-workflow": "privacy_native_bridge_tests",
            "--negative-control-cargo-lock-python-workflow": "privacy_python_sdk_tests",
            "--negative-control-cargo-lock-guard-workflow": "privacy-sdk-guard",
        }[mode]
        source = read(path, {})
        match = re.search(
            rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            source,
        )
        if match is None:
            raise SystemExit(f"negative control cannot find workflow job: {job}")
        block = match.group(0).replace("provision-ci", "bypassed-provision", 1)
        overrides[path] = source[: match.start()] + block + source[match.end() :]
    elif mode == "--negative-control-cargo-lock-helper-generation":
        path = "ci/privacy_sdk_cargo_lockfile.sh"
        overrides[path] = read(path, {}).replace(
            "generate-lockfile", "generate-workspace-lockfile", 1
        )
    elif mode == "--negative-control-js-privacy-abi-drift":
        path = "javascript/iroha_js/src/crypto.js"
        overrides[path] = read(path, {}).replace(
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 21",
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 20",
            1,
        )
    elif mode == "--negative-control-python-privacy-abi-drift":
        path = "python/iroha_python/src/iroha_python/crypto.py"
        overrides[path] = read(path, {}).replace(
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 21",
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 22",
            1,
        )
    else:
        selector = sum(mode.encode("utf-8")) % 4
        if selector == 0:
            path = "javascript/iroha_js/src/privacyCapabilities.js"
            overrides[path] = read(path, {}).replace(
                '"iroha-zk-ams-v1"', '"zk-ams-recursive-admission-v0"', 1
            )
        elif selector == 1:
            path = "python/iroha_python/src/iroha_python/privacy_catalog.py"
            overrides[path] = read(path, {}).replace(
                '"iroha-zk-x509-stark-p256-v0",',
                '"iroha-jindo-polynomial-commitment-v0",',
            )
        elif selector == 2:
            path = "javascript/iroha_js/src/index.js"
            overrides[path] = read(path, {}) + "\nexport const privacyBuildProofV1 = null;\n"
        else:
            path = "crates/iroha_js_host/src/lib.rs"
            overrides[path] = read(path, {}).replace("PrivacyProtocolIdV1::ALL", "[]")
    try:
        check(overrides)
    except (GuardFailure, SyntaxError, ValueError):
        print(f"negative control rejected canonical privacy SDK drift: {mode}")
        raise SystemExit(0)
    raise SystemExit(f"negative control was not detected: {mode}")

try:
    check()
except (GuardFailure, SyntaxError, ValueError) as error:
    print("privacy SDK canonical cutover guard failed:", file=sys.stderr)
    print(error, file=sys.stderr)
    raise SystemExit(1)

print("privacy SDK canonical cutover guard passed")
PY

if [[ -n "${MODE}" || "${PRIVACY_SDK_GUARD_SKIP_RUNTIME:-0}" == "1" ]]; then
  exit 0
fi

# Runtime SDK checks are allowed to resolve Rust dependencies only through an
# explicitly selected lock outside the repository. Normalize the shared
# selection once and pass the same canonical path to every SDK-specific guard.
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${ROOT_DIR}/ci/privacy_sdk_cargo_lockfile.sh"
PRIVACY_SDK_CARGO_LOCKFILE="$(
  privacy_sdk_resolve_cargo_lockfile "${ROOT_DIR}" "${PYTHON_BIN}"
)"
export IROHA_PRIVACY_CARGO_LOCKFILE_PATH="${PRIVACY_SDK_CARGO_LOCKFILE}"
export IROHA_JS_CARGO_LOCKFILE_PATH="${PRIVACY_SDK_CARGO_LOCKFILE}"

PRIVACY_SDK_CARGO_LOCK_SEAL="$(
  privacy_sdk_file_seal "${PRIVACY_SDK_CARGO_LOCKFILE}" "${PYTHON_BIN}"
)"
PRIVACY_SDK_WORKSPACE_LOCK="${ROOT_DIR}/Cargo.lock"
PRIVACY_SDK_WORKSPACE_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${PRIVACY_SDK_WORKSPACE_LOCK}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"
)"

assert_privacy_sdk_guard_lock_state() {
  local status=0
  privacy_sdk_assert_file_seal \
    "${PRIVACY_SDK_CARGO_LOCKFILE}" \
    "${PRIVACY_SDK_CARGO_LOCK_SEAL}" \
    "selected external Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${PRIVACY_SDK_WORKSPACE_LOCK}" \
    "${PRIVACY_SDK_WORKSPACE_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

cleanup_privacy_sdk_guard_lock_state() {
  local status=$?
  trap - EXIT HUP INT TERM
  if ! assert_privacy_sdk_guard_lock_state; then
    status=1
  fi
  exit "${status}"
}
trap cleanup_privacy_sdk_guard_lock_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

bash "${ROOT_DIR}/ci/check_privacy_js_sdk.sh"
assert_privacy_sdk_guard_lock_state
bash "${ROOT_DIR}/ci/check_privacy_python_sdk.sh"
assert_privacy_sdk_guard_lock_state
