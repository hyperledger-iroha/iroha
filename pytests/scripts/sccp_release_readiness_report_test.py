"""Tests for the SCCP release-readiness report renderer."""

import hashlib
import importlib
import json
import re
import subprocess
import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
ALL_LANES_TESTS = ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py"
PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
)
SDK_PHASES = ("js-sdk", "python-sdk", "swift-sdk", "kotlin-sdk", "java-android")
EVM_SDK_PHASES = (*SDK_PHASES, "dotnet-sdk")
JS_CALLBACK_HOOK_SYMBOLS = ("witnessProvider", "proveFn", "consensusProvider")
PYTHON_CALLBACK_HOOK_SYMBOLS = ("witness_provider", "prove", "consensus_provider")
BSC_MAINNET_SDK_SOURCE_PATHS = {
    "js-sdk": ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
    "python-sdk": ROOT / "python" / "iroha_torii_client" / "sccp.py",
    "swift-sdk": ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
    "kotlin-sdk": (
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProver.kt"
    ),
    "java-android": (
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "BscMainnetSccp.java"
    ),
    "dotnet-sdk": (
        (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "BscMainnetSccp.cs"
        ),
        (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "BscMainnetSccpOutbound.cs"
        ),
    ),
}
ETHEREUM_MAINNET_SDK_SOURCE_PATHS = {
    **{
        sdk: path
        for sdk, path in BSC_MAINNET_SDK_SOURCE_PATHS.items()
        if sdk not in {"java-android", "dotnet-sdk"}
    },
    "java-android": (
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "main"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EthereumMainnetSccp.java"
    ),
    "dotnet-sdk": (
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs"
    ),
}
BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS = {
    "WebAssembly": re.compile(r"\bWebAssembly\b"),
    "wasm": re.compile(r"\bwasm\b", re.IGNORECASE),
    "snarkjs": re.compile(r"\bsnarkjs\b", re.IGNORECASE),
    "remoteProver": re.compile(r"\bremoteProver\b"),
    "remote prover": re.compile(r"\bremote prover\b", re.IGNORECASE),
    "proverUrl": re.compile(r"\bproverUrl\b"),
    "proverEndpoint": re.compile(r"\bproverEndpoint\b"),
}
NATIVE_LOCAL_PROVER_SOURCE_GLOBS = {
    "js-sdk": (
        "javascript/iroha_js/src/sccp.js",
        "javascript/iroha_js/src/index.js",
        "javascript/iroha_js/dist/sccp.js",
        "javascript/iroha_js/dist/index.js",
        "javascript/iroha_js/index.d.ts",
    ),
    "python-sdk": (
        "python/iroha_torii_client/sccp.py",
        "python/iroha_torii_client/__init__.py",
    ),
    "swift-sdk": (
        "IrohaSwift/Sources/IrohaSwift/Sccp*.swift",
        "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    ),
    "kotlin-sdk": (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp/*.kt",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/BridgeProofSubmitRequest.kt",
    ),
    "java-android": (
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp/*.java",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/BridgeProofSubmitRequest.java",
    ),
    "dotnet-sdk": (
        "csharp/src/Hyperledger.Iroha.Sdk/Sccp/*.cs",
    ),
}


def phase_command_lines(fragments) -> list[str]:
    """Render required fragments as production-corridor traced commands."""

    return [f"+ {fragment}" for fragment in fragments]


def complete_corridor_log(phases: tuple[str, ...] = PHASES) -> str:
    """Return a synthetic successful SCCP production-corridor transcript."""

    report = load_report_module()
    lines: list[str] = []
    for phase in phases:
        lines.append(f"==> SCCP production corridor: {phase}")
        lines.extend(
            phase_command_lines(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase])
        )
        lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase])
    return "\n".join(
        [*lines, ""]
    ) + "SCCP production corridor completed.\n"


def native_local_prover_source_paths() -> dict[str, list[Path]]:
    """Return SDK source files that must not depend on WASM or remote provers."""

    paths_by_sdk: dict[str, list[Path]] = {}
    for sdk, patterns in NATIVE_LOCAL_PROVER_SOURCE_GLOBS.items():
        paths: list[Path] = []
        for pattern in patterns:
            matches = sorted(ROOT.glob(pattern))
            if not matches:
                raise AssertionError(f"{sdk} native SCCP source glob matched no files: {pattern}")
            paths.extend(path for path in matches if path.is_file())
        paths_by_sdk[sdk] = paths
    return paths_by_sdk


def write_downloaded_phase_artifacts(tmp_path: Path) -> Path:
    """Write synthetic downloaded CI artifacts for every corridor phase."""

    artifact_root = tmp_path / "phase-artifacts"
    for phase in PHASES:
        phase_dir = artifact_root / f"sccp-production-corridor-{phase}"
        phase_dir.mkdir(parents=True)
        (phase_dir / f"{phase}.log").write_text(
            complete_corridor_log((phase,)),
            encoding="utf-8",
        )
    return artifact_root


def load_all_lanes_helpers():
    """Load all-lanes fixture helpers without importing pytest test collection state."""

    spec = spec_from_file_location(
        "sccp_all_lanes_evidence_test_helpers",
        ALL_LANES_TESTS,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_report_module():
    """Load the readiness report module for structured helper assertions."""

    spec = spec_from_file_location("sccp_release_readiness_report_module", SCRIPT)
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def write_complete_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write a complete synthetic all-lanes evidence bundle for report tests."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    evidence = tmp_path / "complete.toml"
    evidence_payload = helpers.render_records(helpers.complete_bundle(evidence_module))
    evidence.write_text(evidence_payload, encoding="utf-8")
    return evidence, evidence_payload


def write_active_launch_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write only the active launch-lane evidence records."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    report = load_report_module()
    active_domain = report.ACTIVE_LAUNCH_DOMAIN
    records = helpers.complete_bundle(evidence_module)
    for section, domain_key in {
        "sccp_source_verifier_materials": "source_domain",
        "sccp_source_adapter_engine_deployments": "source_domain",
        "sccp_destination_rollouts": "domain",
        "sccp_route_allowlists": "domain",
    }.items():
        records[section] = [
            record
            for record in records[section]
            if record.get(domain_key) == active_domain
        ]
    evidence = tmp_path / f"{report.ACTIVE_LAUNCH_CHAIN}-launch.toml"
    evidence_payload = helpers.render_records(records)
    evidence.write_text(evidence_payload, encoding="utf-8")
    return evidence, evidence_payload


def sdk_source_text(sdk: str) -> str:
    """Return the SDK source text that must expose readiness helper symbols."""

    if sdk == "js-sdk":
        paths = [
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            ROOT / "javascript" / "iroha_js" / "src" / "index.js",
        ]
    elif sdk == "python-sdk":
        paths = [ROOT / "python" / "iroha_torii_client" / "sccp.py"]
    elif sdk == "swift-sdk":
        source_root = ROOT / "IrohaSwift" / "Sources" / "IrohaSwift"
        paths = sorted(source_root.glob("Sccp*.swift")) + [
            source_root / "ToriiClient.swift"
        ]
    elif sdk == "kotlin-sdk":
        source_root = (
            ROOT
            / "kotlin"
            / "core-jvm"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "sdk"
        )
        paths = sorted((source_root / "sccp").glob("*.kt")) + [
            source_root / "client" / "BridgeProofSubmitRequest.kt"
        ]
    elif sdk == "java-android":
        source_root = (
            ROOT
            / "java"
            / "iroha_android"
            / "src"
            / "main"
            / "java"
            / "org"
            / "hyperledger"
            / "iroha"
            / "android"
        )
        paths = sorted((source_root / "sccp").glob("*.java")) + [
            source_root / "client" / "BridgeProofSubmitRequest.java"
        ]
    elif sdk == "dotnet-sdk":
        paths = sorted((ROOT / "csharp" / "src" / "Hyperledger.Iroha.Sdk" / "Sccp").glob("*.cs"))
    else:
        raise AssertionError(f"unhandled SCCP SDK phase: {sdk}")
    return "\n".join(path.read_text(encoding="utf-8") for path in paths)


def sdk_symbol_tokens(symbol: str) -> tuple[str, ...]:
    """Return source tokens that must be present for a readiness helper symbol."""

    if ".init(" in symbol:
        owner, _, rest = symbol.partition(".init(")
        return owner, rest.rstrip(")").rstrip(":")
    if "." in symbol:
        owner, member = symbol.rsplit(".", 1)
        return owner, member
    return (symbol,)


def sdk_symbol_export_tokens(symbol: str) -> tuple[str, ...]:
    """Return package-root tokens needed to expose a readiness helper symbol."""

    if ".init(" in symbol:
        owner, _, _ = symbol.partition(".init(")
        return (owner,)
    if "." in symbol:
        owner, _ = symbol.rsplit(".", 1)
        return (owner,)
    return (symbol,)


def helper_matches_hook_marker(sdk: str, helper: str, marker: str) -> bool:
    """Return whether a helper symbol satisfies a UI-owned hook marker."""

    if sdk == "python-sdk":
        return helper == marker
    return marker in helper


def test_release_readiness_sdk_helper_symbols_exist_in_sdk_sources() -> None:
    """Readiness helper maps must name SDK symbols that exist in source."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    surfaces = report._submission_surfaces(passed_phases)
    sources = {
        sdk: sdk_source_text(sdk)
        for surface in surfaces
        for sdk in surface["sdk_helper_symbols_by_sdk"]
    }
    missing: list[str] = []

    for surface in surfaces:
        for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items():
            source = sources[sdk]
            for symbol in symbols:
                absent_tokens = [
                    token for token in sdk_symbol_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} {sdk} {symbol}: {absent_tokens}"
                    )

    assert missing == []


def test_release_readiness_bsc_sdk_sources_are_native_local_prover_only() -> None:
    """BSC SDK facades must stay native/local-prover owned, with no WASM fallback."""

    violations: list[str] = []
    for sdk, path_or_paths in BSC_MAINNET_SDK_SOURCE_PATHS.items():
        paths = path_or_paths if isinstance(path_or_paths, tuple) else (path_or_paths,)
        for path in paths:
            if not path.is_file():
                violations.append(f"{sdk} missing BSC source file: {path.relative_to(ROOT)}")
                continue
            source = path.read_text(encoding="utf-8")
            for label, pattern in BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS.items():
                if pattern.search(source):
                    violations.append(
                        f"{sdk} {path.relative_to(ROOT)} contains forbidden {label}"
                    )

    assert violations == []


def test_release_readiness_ethereum_sdk_sources_are_native_local_prover_only() -> None:
    """Ethereum SDK facades must stay native/local-prover owned, with no WASM fallback."""

    violations: list[str] = []
    for sdk, path in ETHEREUM_MAINNET_SDK_SOURCE_PATHS.items():
        if not path.is_file():
            violations.append(f"{sdk} missing Ethereum source file: {path.relative_to(ROOT)}")
            continue
        source = path.read_text(encoding="utf-8")
        for label, pattern in BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS.items():
            if pattern.search(source):
                violations.append(
                    f"{sdk} {path.relative_to(ROOT)} contains forbidden {label}"
                )

    assert violations == []


def test_release_readiness_all_public_sccp_sdk_sources_are_native_local_prover_only(
) -> None:
    """All public SCCP SDK artifacts must stay native/local-prover owned."""

    violations: list[str] = []
    for sdk, paths in native_local_prover_source_paths().items():
        for path in paths:
            source = path.read_text(encoding="utf-8")
            for label, pattern in BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS.items():
                if pattern.search(source):
                    violations.append(
                        f"{sdk} {path.relative_to(ROOT)} contains forbidden {label}"
                    )

    assert violations == []


def test_release_readiness_sdk_helper_symbols_are_unique() -> None:
    """Public user-prover helper rows must not hide missing hooks behind duplicates."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    duplicates: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        helper_symbols = surface["sdk_helper_symbols"]
        if len(helper_symbols) != len(set(helper_symbols)):
            duplicates.append(f"{surface['lanes']} default helper list")
        for sdk, symbols in surface["sdk_helper_symbols_by_sdk"].items():
            if len(symbols) != len(set(symbols)):
                duplicates.append(f"{surface['lanes']} {sdk}")

    assert duplicates == []


def test_release_readiness_js_helper_symbols_exist_in_portal_artifacts() -> None:
    """Web portal helper maps must exist in JS source, dist, and declarations."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    implementation_artifacts = {
        "src/sccp.js": ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
        "dist/sccp.js": ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
        "index.d.ts": ROOT / "javascript" / "iroha_js" / "index.d.ts",
    }
    package_entry_artifacts = {
        "src/index.js": ROOT / "javascript" / "iroha_js" / "src" / "index.js",
        "dist/index.js": ROOT / "javascript" / "iroha_js" / "dist" / "index.js",
    }
    implementation_artifact_text = {
        label: path.read_text(encoding="utf-8")
        for label, path in implementation_artifacts.items()
    }
    package_entry_artifact_text = {
        label: path.read_text(encoding="utf-8")
        for label, path in package_entry_artifacts.items()
    }
    missing: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        for symbol in surface["sdk_helper_symbols_by_sdk"]["js-sdk"]:
            for artifact, source in implementation_artifact_text.items():
                absent_tokens = [
                    token for token in sdk_symbol_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} js-sdk {symbol} missing from {artifact}: {absent_tokens}"
                    )
            if symbol in JS_CALLBACK_HOOK_SYMBOLS:
                continue
            for artifact, source in package_entry_artifact_text.items():
                absent_tokens = [
                    token for token in sdk_symbol_export_tokens(symbol) if token not in source
                ]
                if absent_tokens:
                    missing.append(
                        f"{surface['lanes']} js-sdk {symbol} missing from {artifact}: {absent_tokens}"
                    )

    assert missing == []


def test_release_readiness_user_prover_surfaces_name_ui_hook_symbols() -> None:
    """Every public user-prover row must include the app-owned prover hooks."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (
            *report.USER_PROVER_SDK_PHASES,
            report.EVM_NATIVE_DOTNET_PHASE,
            "contract-smoke",
            "core-admission",
        )
    }
    required_hook_markers = {
        "js-sdk": ("witnessProvider", "proveFn"),
        "python-sdk": ("witness_provider", "prove"),
        "swift-sdk": ("WitnessProvider", "ProveFunction"),
        "kotlin-sdk": ("WitnessProvider", "ProofEngine"),
        "java-android": ("WitnessProvider", "ProofEngine"),
        "dotnet-sdk": ("InboundProver", "InboundSubmitter"),
    }
    missing: list[str] = []

    for surface in report._submission_surfaces(passed_phases):
        for sdk, markers in required_hook_markers.items():
            symbols = surface["sdk_helper_symbols_by_sdk"].get(sdk)
            if symbols is None:
                continue
            for marker in markers:
                if not any(
                    helper_matches_hook_marker(sdk, symbol, marker)
                    for symbol in symbols
                ):
                    missing.append(f"{surface['lanes']} {sdk} missing {marker}")

    assert missing == []


def test_release_readiness_python_helper_symbols_are_package_root_exports() -> None:
    """Python app code must be able to import public SCCP helpers from the package root."""

    report = load_report_module()
    passed_phases = {
        phase: "passed"
        for phase in (*report.USER_PROVER_SDK_PHASES, "contract-smoke", "core-admission")
    }
    required_exports = sorted(
        {
            export
            for surface in report._submission_surfaces(passed_phases)
            for symbol in surface["sdk_helper_symbols_by_sdk"]["python-sdk"]
            for export in sdk_symbol_export_tokens(symbol)
            if symbol not in PYTHON_CALLBACK_HOOK_SYMBOLS
        }
    )

    original_path = sys.path[:]
    sys.path.insert(0, str(ROOT / "python"))
    try:
        package = importlib.import_module("iroha_torii_client")
    finally:
        sys.path[:] = original_path

    package_exports = set(getattr(package, "__all__", ()))
    missing_attrs = [
        symbol for symbol in required_exports if not hasattr(package, symbol)
    ]
    missing_all = [symbol for symbol in required_exports if symbol not in package_exports]

    assert missing_attrs == []
    assert missing_all == []


def test_release_readiness_report_blocks_without_evidence_or_corridor_results(
    tmp_path: Path,
) -> None:
    """A public readiness note must not pass without evidence and corridor proof."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        ["python3", str(SCRIPT), str(evidence)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "# SCCP Release Readiness Report" in completed.stdout
    assert "Status: NOT READY" in completed.stdout
    assert "| Path | Bytes | SHA-256 |" in completed.stdout
    assert hashlib.sha256(b"").hexdigest() in completed.stdout
    assert "## Release Checklist" in completed.stdout
    assert "## User Prover Submission Surfaces" in completed.stdout
    assert "`ton` | `ton-contract-v1`" in completed.stdout
    assert "buildTonSccpSubmission" in completed.stdout
    assert "`python-sdk`: `build_ton_sccp_proof_request`" in completed.stdout
    assert "`swift-sdk`: `buildTonSccpProofRequest`" in completed.stdout
    assert "ToriiBridgeProofSubmitRequest.init(evmSccpSubmission:)" in completed.stdout
    assert "TON internal message body BOC" in completed.stdout
    assert (
        "`js-sdk`, `python-sdk`, `swift-sdk`, `kotlin-sdk`, `java-android`"
        in completed.stdout
    )
    assert "blocked: js-sdk is missing<br>python-sdk is missing" in completed.stdout
    assert "`live_route_canary_evidence` | blocked" in completed.stdout
    assert "missing source verifier material" in completed.stdout
    assert "`contract-smoke` | missing" in completed.stdout
    assert "`core-admission`" in completed.stdout
    assert "packaged `dist`, and TypeScript declaration exports" in completed.stdout


def test_release_readiness_json_tracks_corridor_phase_results(tmp_path: Path) -> None:
    """JSON output must separate evidence blockers from validation corridor status."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    report = load_report_module()
    payload = json.loads(completed.stdout)
    assert payload["production_ready"] is False
    assert payload["corridor"]["production_ready"] is True
    assert payload["corridor"]["phases"]["contract-smoke"] == "passed"
    assert payload["corridor"]["evidence_artifacts"] == {}
    assert payload["corridor"]["require_phase_evidence"] is False
    assert payload["input_artifacts"] == [
        {
            "path": str(evidence),
            "bytes": 0,
            "sha256": hashlib.sha256(b"").hexdigest(),
        }
    ]
    assert "cryptographic_evidence" in payload
    assert payload["evidence"]["production_ready"] is False
    assert payload["release_checklist"]["ready"] is False
    assert any(
        item["id"] == "all_required_lane_records"
        for item in payload["release_checklist"]["items"]
    )
    surfaces = {
        surface["lanes"]: surface
        for surface in payload["user_prover_submission_surfaces"]
    }
    assert "ton" in surfaces
    assert surfaces["sol"]["proof_backend"] == "sccp-solana-recursive-mainnet-v1"
    assert surfaces["ton"]["proof_backend"] == "ton-contract-v1"
    assert surfaces["eth,bsc"]["proof_backend"] == "evm-groth16-bn254-v1"
    assert surfaces["tron"]["proof_backend"] == "tron-groth16-bn254-v1"
    assert surfaces["substrate"]["proof_backend"] == "substrate-runtime-v1"
    assert "canonicalEvmSccpReceiptProofBytes" in surfaces["eth,bsc"]["sdk_helpers"]
    assert "canonicalBscSccpReceiptProofBytes" in surfaces["eth,bsc"]["sdk_helpers"]
    assert surfaces["eth,bsc"]["sdk_helper_symbols"] == list(
        report.EVM_JS_USER_PROVER_HELPERS
    )
    assert surfaces["eth,bsc"]["sdk_helpers"] == ", ".join(
        surfaces["eth,bsc"]["sdk_helper_symbols"]
    )
    assert set(surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]) == set(EVM_SDK_PHASES)
    assert (
        surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["js-sdk"]
        == surfaces["eth,bsc"]["sdk_helper_symbols"]
    )
    assert (
        "build_evm_sccp_proof_request"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.build_ethereum_calldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submit_outbound_to_ethereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "BscMainnetSccp.build_bsc_calldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "BscMainnetSccp.submit_outbound_to_bsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.collect_inbound_evidence_from_receipt"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "EthereumMainnetSccp.prove_inbound_to_sora"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "consensus_provider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "ToriiBridgeProofSubmitRequest.init(evmSccpSubmission:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EvmSccpProver.ProveFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.init(beaconFinalityEvidence:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "EthereumMainnetSccp.OutboundSubmitFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetSccp.OutboundSubmitFunction"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.init(parliaFinalityEvidence:)"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "SccpEvm.buildProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EvmSccpProofEngine"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.withBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EthereumMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.withParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "EvmSccpProver.buildProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EvmSccpProver.ProofEngine"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.ConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.BeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.ReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "InboundEvidence.withBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.submitOutboundToEthereum"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.OutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.buildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.submitOutboundToBsc"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.OutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.ConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "BscMainnetSccp.ParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "InboundEvidence.withParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.BuildOutboundProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.ProveOutboundToEthereumAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.BuildEthereumCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccp.SubmitOutboundToEthereumAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetOutboundProofRequestInput"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetSccpSubmission"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetReceiptProof"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "EthereumMainnetInboundEvidence.WithBeaconFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetConsensusProvider"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetInboundEvidence.WithParliaFinalityEvidence"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.BuildOutboundProofRequest"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.ProveOutboundToBscAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.BuildBscCalldata"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccp.SubmitOutboundToBscAsync"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetOutboundProofRequestInput"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "BscMainnetSccpSubmission"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetInboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetOutboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IBscMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetOutboundProver"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert (
        "IEthereumMainnetOutboundSubmitter"
        in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    )
    assert "canonicalTronSccpReceiptStateProofBytes" in surfaces["tron"]["sdk_helpers"]
    assert (
        "canonicalTronSccpTransactionSourceProofBytes"
        in surfaces["tron"]["sdk_helpers"]
    )
    assert "TronSccpProver" in surfaces["tron"]["sdk_helper_symbols"]
    assert "witnessProvider" in surfaces["tron"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["tron"]["sdk_helper_symbols"]
    assert (
        "build_tron_sccp_bridge_proof_submit_payload"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "ToriiBridgeProofSubmitRequest.init(tronSccpSubmission:)"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    )
    assert (
        "SccpSourceProofs.tronTransactionSourceProofHash"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "TronSccpProofEngine"
        in surfaces["tron"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "buildSubstrateSccpRuntimeStorageProofRequest"
        in surfaces["substrate"]["sdk_helpers"]
    )
    assert "SubstrateSccpProver" in surfaces["substrate"]["sdk_helper_symbols"]
    assert (
        "build_substrate_sccp_runtime_storage_proof_request"
        in surfaces["substrate"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SccpSourceProofs.buildSubstrateRuntimeStorageProofRequest"
        in surfaces["substrate"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert "witnessProvider" in surfaces["substrate"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["substrate"]["sdk_helper_symbols"]
    assert (
        "SubstrateSccpProver.ProofEngine"
        in surfaces["substrate"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert "buildTonSccpSubmission" in surfaces["ton"]["sdk_helpers"]
    assert "TonSccpSourceStateProver" in surfaces["ton"]["sdk_helper_symbols"]
    assert (
        "build_ton_shard_state_proof_request"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SccpTon.buildShardStateProofRequest"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "buildSolanaSccpAccountsLtHashProofRequest"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert (
        "buildSolanaSccpFullLightClientAuditProofRequests"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert "SolanaSccpSourceStateProver" in surfaces["sol"]["sdk_helpers"]
    assert "SolanaSccpProver" in surfaces["sol"]["sdk_helper_symbols"]
    assert "witnessProvider" in surfaces["sol"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["sol"]["sdk_helper_symbols"]
    assert (
        "build_solana_sccp_accounts_lt_hash_proof_request"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SccpSolana.buildFullLightClientAuditProofRequests"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "SolanaSccpFullLightClientAuditProofEngine"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    )
    assert (
        "SolanaSccpProver.SourceStateProver"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert "TON internal message body BOC" in surfaces["ton"]["on_chain_submission"]
    assert "buildTonShardStateProofRequest" in surfaces["ton"]["sdk_helpers"]
    assert (
        "buildTonSccpFullLightClientAuditProofRequests"
        in surfaces["ton"]["sdk_helpers"]
    )
    assert (
        "buildTonSccpValidatorSetTransitionProofRequest"
        in surfaces["ton"]["sdk_helpers"]
    )
    assert (
        "build_ton_sccp_masterchain_config_proof_request"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "TonSccpProver.buildShardAccountsDictionaryProofRequest"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "TonSccpProver.FullLightClientAuditProofEngine"
        in surfaces["ton"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert "TonSccpSourceStateProver" in surfaces["ton"]["sdk_helpers"]
    assert "witnessProvider" in surfaces["ton"]["sdk_helper_symbols"]
    assert "proveFn" in surfaces["ton"]["sdk_helper_symbols"]
    assert (
        "buildSolanaSccpBankForkChoiceProofRequest"
        in surfaces["sol"]["sdk_helpers"]
    )
    assert (
        "build_solana_sccp_full_accountsdb_lattice_proof_request"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["python-sdk"]
    )
    assert (
        "SolanaSccpProver.buildTowerReplayProofRequest"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert (
        "SolanaSccpProver.FullLightClientAuditProofEngine"
        in surfaces["sol"]["sdk_helper_symbols_by_sdk"]["java-android"]
    )
    assert surfaces["ton"]["required_phases"] == [
        "js-sdk",
        "python-sdk",
        "swift-sdk",
        "kotlin-sdk",
        "java-android",
        "core-admission",
    ]
    assert surfaces["ton"]["validation_status"] == "passed"
    assert surfaces["ton"]["validation_blockers"] == []
    assert "eth,bsc" in surfaces
    assert (
        "buildEvmSccpBridgeProofSubmitPayload"
        in surfaces["eth,bsc"]["sdk_helpers"]
    )
    assert "dotnet-sdk" in surfaces["eth,bsc"]["required_phases"]
    assert "contract-smoke" in surfaces["eth,bsc"]["required_phases"]
    assert "core-admission" in surfaces["eth,bsc"]["required_phases"]
    assert any("missing source verifier material" in item for item in payload["blockers"])


def test_release_readiness_user_prover_surfaces_require_core_admission(
    tmp_path: Path,
) -> None:
    """User-side prover surfaces are blocked until on-chain admission is tested."""

    evidence = tmp_path / "empty.toml"
    evidence.write_text("", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            "--phase-result",
            "core-admission=missing",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    assert payload["corridor"]["production_ready"] is False
    assert "core-admission is missing" in payload["corridor"]["blockers"]
    for surface in payload["user_prover_submission_surfaces"]:
        assert "core-admission" in surface["required_phases"]
        assert surface["validation_status"] == "blocked"
        assert "core-admission is missing" in surface["validation_blockers"]


def test_release_readiness_report_strict_phase_evidence_blocks_missing_artifacts(
    tmp_path: Path,
) -> None:
    """Strict release notes require hashed proof for every passed corridor phase."""

    evidence, _ = write_complete_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert "| `rust-sccp` | passed | - | - |" in completed.stdout
    assert (
        "production corridor phase rust-sccp has no hashed evidence artifact"
        in completed.stdout
    )
    assert "`governed_deployment_evidence` | ready" in completed.stdout
    assert "`live_route_canary_evidence` | ready" in completed.stdout


def test_release_readiness_report_passes_for_complete_evidence_and_corridor(
    tmp_path: Path,
) -> None:
    """A complete all-lanes bundle plus passing corridor phases produces releasable notes."""

    evidence, evidence_payload = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "sccp-corridor.log"
    corridor_payload = complete_corridor_log()
    corridor_log.write_text(corridor_payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    assert "Status: READY" in completed.stdout
    assert (
        hashlib.sha256(evidence_payload.encode("utf-8")).hexdigest()
        in completed.stdout
    )
    assert (
        hashlib.sha256(corridor_payload.encode("utf-8")).hexdigest()
        in completed.stdout
    )
    assert f"| `rust-sccp` | passed | `{corridor_log}` |" in completed.stdout
    assert "## Release Checklist" in completed.stdout
    assert "## Cryptographic Evidence" in completed.stdout
    assert "Source Material | Source Deployment | Destination Binding" in (
        completed.stdout
    )
    assert "Source Gate | Source Gate Audits | Route Allowlist" in completed.stdout
    assert "Canary Block | Canary Timestamp" in completed.stdout
    assert "`evm_message_proof_accepted_transaction`" in completed.stdout
    assert "`tron_message_proof_accepted_transaction`" in completed.stdout
    assert "`10144`" in completed.stdout
    assert "`1700144`" in completed.stdout
    assert "`solana_live_programdata_snapshot`" in completed.stdout
    assert "`ton_live_account_snapshot`" in completed.stdout
    assert "`substrate_finalized_runtime_snapshot`" in completed.stdout
    assert "## User Prover Submission Surfaces" in completed.stdout
    assert "`substrate` | `substrate-runtime-v1`" in completed.stdout
    assert "Substrate runtime call envelope" in completed.stdout
    assert "| `substrate` | `substrate-runtime-v1`" in completed.stdout
    assert "| `ton` | `ton-contract-v1`" in completed.stdout
    assert " | passed |" in completed.stdout
    assert "`governed_deployment_evidence` | ready" in completed.stdout
    assert "`live_route_canary_evidence` | ready" in completed.stdout
    assert "## Blocking Items\n\n- None" in completed.stdout


def test_release_readiness_report_passes_with_only_active_launch_lane(
    tmp_path: Path,
) -> None:
    """Active launch readiness must not require future lanes to be complete."""

    report = load_report_module()
    active_domain = report.ACTIVE_LAUNCH_DOMAIN
    evidence, _ = write_active_launch_evidence(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    payload = json.loads(completed.stdout)
    assert payload["production_ready"] is True
    assert payload["release_checklist"]["ready"] is True
    assert payload["evidence"]["production_ready"] is False
    assert all(
        f"domain {active_domain}" not in blocker for blocker in payload["blockers"]
    )
    active_crypto = next(
        row
        for row in payload["cryptographic_evidence"]
        if row["domain"] == active_domain
    )
    assert active_crypto["domain"] == active_domain
    blocked_future_lanes = [
        lane
        for lane in payload["evidence"]["lanes"]
        if lane["domain"] != active_domain and not lane["production_ready"]
    ]
    assert blocked_future_lanes


def test_release_readiness_report_accepts_phase_evidence_dir(
    tmp_path: Path,
) -> None:
    """Strict reports can bind downloaded per-phase corridor log artifacts."""

    evidence, _ = write_complete_evidence(tmp_path)
    phase_artifacts = write_downloaded_phase_artifacts(tmp_path)
    js_log = (
        phase_artifacts
        / "sccp-production-corridor-js-sdk"
        / "js-sdk.log"
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(phase_artifacts),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0
    assert "Status: READY" in completed.stdout
    assert f"| `js-sdk` | passed | `{js_log}` |" in completed.stdout
    assert "## Blocking Items\n\n- None" in completed.stdout


def test_release_readiness_report_rejects_forged_phase_log(
    tmp_path: Path,
) -> None:
    """A hashed phase artifact must be an actual corridor transcript."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-corridor.log"
    corridor_log.write_text("SCCP production corridor completed.\n", encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase marker"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_expected_command(
    tmp_path: Path,
) -> None:
    """A phase artifact must contain the command for the claimed corridor phase."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-rust-sccp.log"
    corridor_log.write_text(
        "==> SCCP production corridor: rust-sccp\n"
        "phase rust-sccp passed\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_phase_completion(
    tmp_path: Path,
) -> None:
    """A phase artifact must prove completion in the claimed phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-no-phase-completion.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "==> SCCP production corridor: js-sdk",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "the phase-block completion sentinel"
    ) in completed.stdout


def test_release_readiness_report_rejects_output_only_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Required command fragments must come from traced corridor command lines."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "forged-rust-sccp-output-only.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"],
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["rust-sccp"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout


def test_release_readiness_report_requires_js_package_dist_transcript(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove source, dist, and package export tests ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != "javascript/iroha_js/test/package_dist.test.js"
    ]
    corridor_log = tmp_path / "js-sdk-without-package-dist.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        "expected phase-block command: javascript/iroha_js/test/package_dist.test.js"
    ) in completed.stdout


def test_release_readiness_report_requires_bsc_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the browser BSC path stayed native JS."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    bsc_no_wasm_marker = (
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert bsc_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != bsc_no_wasm_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-bsc-no-wasm-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {bsc_no_wasm_marker}"
    ) in completed.stdout


def test_release_readiness_report_requires_bsc_parlia_declaration_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the BSC Parlia declarations were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    declaration_marker = (
        "package declarations expose BSC mainnet Parlia finality evidence hooks"
    )
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-bsc-parlia-declaration-marker.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {declaration_marker}"
    ) in completed.stdout


def test_release_readiness_report_requires_js_package_export_transcript(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove package-root SCCP helpers were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_export_test = "javascript/iroha_js/test/sccpPackageExports.test.js"
    assert required_export_test in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != required_export_test
    ]
    corridor_log = tmp_path / "js-sdk-without-package-exports.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "js-sdk=passed",
            "--phase-evidence",
            f"js-sdk={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {required_export_test}"
    ) in completed.stdout


def test_release_readiness_report_requires_js_mainnet_facade_transcripts(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove ETH/BSC mainnet facade tests ran."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    required_facade_tests = (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
    )
    for required_facade_test in required_facade_tests:
        assert (
            required_facade_test
            in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        )
        required_fragments = [
            fragment
            for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
            if fragment != required_facade_test
        ]
        corridor_log = tmp_path / f"js-sdk-without-{Path(required_facade_test).stem}.log"
        corridor_log.write_text(
            "\n".join(
                (
                    "==> SCCP production corridor: js-sdk",
                    *phase_command_lines(required_fragments),
                    *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )

        completed = subprocess.run(
            [
                "python3",
                str(SCRIPT),
                "--require-phase-evidence",
                "--phase-result",
                "all=missing",
                "--phase-result",
                "js-sdk=passed",
                "--phase-evidence",
                f"js-sdk={corridor_log}",
                str(evidence),
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert completed.returncode == 1
        assert "Status: NOT READY" in completed.stdout
        assert (
            "production corridor phase js-sdk evidence artifact is missing "
            f"expected phase-block command: {required_facade_test}"
        ) in completed.stdout


def test_release_readiness_report_rejects_phase_command_outside_claimed_block(
    tmp_path: Path,
) -> None:
    """A full transcript must bind the command to the claimed phase block."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "forged-rust-sccp-block.log"
    corridor_log.write_text(
        "==> SCCP production corridor: rust-sccp\n"
        "phase rust-sccp passed\n"
        "==> SCCP production corridor: js-sdk\n"
        "+ cargo test -p iroha_sccp -- --nocapture\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block command: cargo test -p iroha_sccp -- --nocapture"
    ) in completed.stdout


def test_release_readiness_report_rejects_phase_log_without_success_marker(
    tmp_path: Path,
) -> None:
    """A passed phase artifact must contain a phase-local success marker."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    corridor_log = tmp_path / "command-only-rust-sccp.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: rust-sccp",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["rust-sccp"]
                ),
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=missing",
            "--phase-result",
            "rust-sccp=passed",
            "--phase-evidence",
            f"rust-sccp={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "Status: NOT READY" in completed.stdout
    assert (
        "production corridor phase rust-sccp evidence artifact is missing "
        "expected phase-block success marker: test result: ok"
    ) in completed.stdout


def test_release_readiness_report_rejects_symlinked_phase_evidence(
    tmp_path: Path,
) -> None:
    """Strict release notes must hash the actual phase artifact, not a symlink."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "sccp-corridor.log"
    corridor_log.write_text(complete_corridor_log(), encoding="utf-8")
    corridor_link = tmp_path / "sccp-corridor-link.log"
    corridor_link.symlink_to(corridor_log)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--require-phase-evidence",
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_link}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "release artifact path must not be a symlink" in completed.stderr


def test_release_readiness_rejects_control_character_artifact_paths(
    tmp_path: Path,
) -> None:
    """Release-readiness artifact paths must be printable reviewer text."""

    _, payload = write_complete_evidence(tmp_path)
    evidence = tmp_path / "complete\noperator.toml"
    evidence.write_text(payload, encoding="utf-8")

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--format",
            "json",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "release artifact path contains control character '\\n':" in completed.stderr
    assert "complete\\noperator.toml" in completed.stderr
