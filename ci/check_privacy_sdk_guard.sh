#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"
PYTHON_BIN="${PRIVACY_SDK_GUARD_PYTHON_BIN:-python3}"

"${PYTHON_BIN}" - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

import ast
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]

EXPECTED_IDS = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
)

RETIRED_IDS = (
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
)

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

    js_source = read("javascript/iroha_js/src/privacyCapabilities.js", overrides)
    js_dist = read("javascript/iroha_js/dist/privacyCapabilities.js", overrides)
    py_catalog = read(
        "python/iroha_python/src/iroha_python/privacy_catalog.py", overrides
    )
    rust_model = read("crates/iroha_data_model/src/privacy.rs", overrides)

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
        == {"iroha_privacy_capabilities_v1", "iroha_privacy_free_buffer"},
        "C privacy ABI must contain only capability snapshot and zeroizing free",
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

    if errors:
        raise GuardFailure("\n".join(f"- {error}" for error in errors))


if mode:
    if not mode.startswith("--negative-control-"):
        raise SystemExit(f"unknown mode: {mode}")
    selector = sum(mode.encode("utf-8")) % 4
    overrides: dict[str, str] = {}
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

bash "${ROOT_DIR}/ci/check_privacy_js_sdk.sh"
bash "${ROOT_DIR}/ci/check_privacy_python_sdk.sh"
