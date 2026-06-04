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
CORRIDOR_SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
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
EVM_EVIDENCE_SCRIPT_FRAGMENTS = (
    "pytests/scripts/sccp_eth_source_bridge_evidence_test.py",
    "pytests/scripts/sccp_bsc_source_bridge_evidence_test.py",
    "pytests/scripts/sccp_evm_destination_evidence_test.py",
    "pytests/scripts/sccp_evm_live_evidence_test.py",
    "pytests/scripts/sccp_evm_receipt_proof_evidence_test.py",
    "pytests/scripts/sccp_evm_source_live_evidence_test.py",
)
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
    "remote_prover": re.compile(r"\bremote_prover\b", re.IGNORECASE),
    "remote-prover": re.compile(r"\bremote-prover\b", re.IGNORECASE),
    "proverUrl": re.compile(r"\bproverUrl\b"),
    "proverURL": re.compile(r"\bproverURL\b"),
    "prover_url": re.compile(r"\bprover_url\b", re.IGNORECASE),
    "proverEndpoint": re.compile(r"\bproverEndpoint\b"),
    "prover_endpoint": re.compile(r"\bprover_endpoint\b", re.IGNORECASE),
}
ETHEREUM_DATA_COLLECTION_FORBIDDEN_PATTERNS = {
    "Torii": re.compile(r"\bTorii\b"),
    "torii": re.compile(r"\btorii\b"),
    "proxy": re.compile(r"\bproxy\b", re.IGNORECASE),
    "embedded HTTP client": re.compile(
        r"\b(fetch|XMLHttpRequest|requests|URLSession|HttpURLConnection|HttpClient)\b"
    ),
}
ETHEREUM_DATA_COLLECTION_REGIONS = {
    "js-sdk": (
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
        "  async validateExecutionProviderMainnet",
        "  async submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "js-dist": (
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
        "  async validateExecutionProviderMainnet",
        "  async submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "python-sdk": (
        ROOT / "python" / "iroha_torii_client" / "sccp.py",
        "    async def validate_execution_provider_mainnet",
        "    async def submit_inbound_to_iroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "_evm_facade_collect_finality",
        ),
    ),
    "swift-sdk": (
        ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
        "    public func validateExecutionProviderMainnet",
        "    public func submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
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
        / "EvmSccpProver.kt",
        "    fun validateExecutionProviderMainnet",
        "    fun submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
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
        / "EthereumMainnetSccp.java",
        "  public Object validateExecutionProviderMainnet()",
        "  public Object submitInboundToIroha",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "collectFinalityEvidence",
        ),
    ),
    "dotnet-sdk": (
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs",
        "    public static async ValueTask<object?> ValidateExecutionProviderMainnetAsync",
        "    public static async ValueTask<object?> SubmitInboundToIrohaAsync",
        (
            "eth_chainId",
            "eth_getTransactionReceipt",
            "eth_getBlockByHash",
            "CollectFinalityEvidenceAsync",
        ),
    ),
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


def corridor_evidence_script_tests() -> tuple[str, ...]:
    """Return pytest files listed by the production corridor evidence phase."""

    script = CORRIDOR_SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"phase_evidence_scripts\(\) \{\n"
        r"\s+local tests=\(\n"
        r"(?P<body>.*?)"
        r"\n\s+\)\n"
        r"\s+run_cmd python3 -m pytest -q \"\$\{tests\[@\]\}\"",
        script,
        re.DOTALL,
    )
    assert match is not None, "phase_evidence_scripts test inventory not found"
    tests = []
    for raw_line in match.group("body").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        tests.append(line)
    return tuple(tests)


def corridor_android_harness_mains() -> tuple[str, ...]:
    """Return Java/Android harness mains listed by the production corridor."""

    script = CORRIDOR_SCRIPT.read_text(encoding="utf-8")
    match = re.search(r'android_harness_mains="(?P<body>[^"]+)"', script)
    assert match is not None, "java-android harness inventory not found"
    return tuple(match.group("body").split(","))


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


def source_region(path: Path, start_marker: str, end_marker: str) -> str:
    """Return the source region delimited by two stable markers."""

    source = path.read_text(encoding="utf-8")
    start = source.find(start_marker)
    if start == -1:
        raise AssertionError(
            f"{path.relative_to(ROOT)} missing start marker: {start_marker}"
        )
    end = source.find(end_marker, start + len(start_marker))
    if end == -1:
        raise AssertionError(
            f"{path.relative_to(ROOT)} missing end marker: {end_marker}"
        )
    return source[start:end]


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


def active_evm_live_chain_id(report):
    """Return the decimal EVM chain id required by the active launch lane."""

    return {
        "eth": "1",
        "bsc": "56",
    }.get(report.ACTIVE_LAUNCH_CHAIN)


def write_complete_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write a complete synthetic all-lanes evidence bundle for report tests."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    report = load_report_module()
    records = helpers.complete_bundle(evidence_module)
    evm_chain_id = active_evm_live_chain_id(report)
    if evm_chain_id is not None:
        for record in records["sccp_source_verifier_materials"]:
            if record.get("source_domain") == report.ACTIVE_LAUNCH_DOMAIN:
                record["_comment_evm_source_rpc_chain_id"] = evm_chain_id
                record["_comment_evm_source_block_tag"] = "finalized"
        for record in records["sccp_destination_rollouts"]:
            if record.get("domain") == report.ACTIVE_LAUNCH_DOMAIN:
                record["_comment_evm_rpc_chain_id"] = evm_chain_id
                record["_comment_evm_block_tag"] = "finalized"
    evidence = tmp_path / "complete.toml"
    evidence_payload = helpers.render_records(records)
    evidence.write_text(evidence_payload, encoding="utf-8")
    return evidence, evidence_payload


def test_release_readiness_active_launch_policy_is_ethereum_mainnet() -> None:
    """The release-readiness script must advertise the Ethereum launch lane."""

    report = load_report_module()

    assert report.ACTIVE_LAUNCH_DOMAIN == 1
    assert report.ACTIVE_LAUNCH_CHAIN == "eth"
    assert report.ACTIVE_LAUNCH_POLICY == "EthereumMainnetLane"
    assert report.ACTIVE_LAUNCH_DISPLAY == "Ethereum mainnet"


def test_release_readiness_evidence_phase_requires_evm_script_suites() -> None:
    """The evidence phase transcript must prove the EVM evidence suites ran."""

    report = load_report_module()
    required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]

    for fragment in EVM_EVIDENCE_SCRIPT_FRAGMENTS:
        assert fragment in required_fragments


def test_release_readiness_evidence_phase_inventory_matches_corridor_runner() -> None:
    """The evidence transcript gate must track the runner's pytest inventory."""

    report = load_report_module()
    required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]

    for test_path in corridor_evidence_script_tests():
        assert any(test_path in fragment for fragment in required_fragments)


def test_release_readiness_java_android_phase_requires_source_proof_harness() -> None:
    """Android readiness evidence must prove source-proof hardening ran."""

    report = load_report_module()
    source_harness = "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"

    assert source_harness in corridor_android_harness_mains()
    assert source_harness in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "java-android"
    ]


def test_release_readiness_report_requires_evm_evidence_script_transcript(
    tmp_path: Path,
) -> None:
    """The report must reject evidence phase logs missing EVM evidence tests."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    omitted_fragment = "pytests/scripts/sccp_evm_live_evidence_test.py"
    assert omitted_fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
        "evidence-scripts"
    ]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[
            "evidence-scripts"
        ]
        if fragment != omitted_fragment
    ]
    corridor_log = tmp_path / "evidence-scripts-without-evm-live.log"
    corridor_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: evidence-scripts",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["evidence-scripts"],
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
            "evidence-scripts=passed",
            "--phase-evidence",
            f"evidence-scripts={corridor_log}",
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
        "production corridor phase evidence-scripts evidence artifact is "
        f"missing expected phase-block command: {omitted_fragment}"
    ) in completed.stdout


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
    evm_chain_id = active_evm_live_chain_id(report)
    if evm_chain_id is not None:
        for record in records["sccp_source_verifier_materials"]:
            record["_comment_evm_source_rpc_chain_id"] = evm_chain_id
            record["_comment_evm_source_block_tag"] = "finalized"
        for record in records["sccp_destination_rollouts"]:
            record["_comment_evm_rpc_chain_id"] = evm_chain_id
            record["_comment_evm_block_tag"] = "finalized"
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


def test_release_readiness_ethereum_data_collection_has_no_proxy_fallback() -> None:
    """Ethereum evidence collection must use app-owned providers."""

    violations: list[str] = []
    for sdk, region_config in ETHEREUM_DATA_COLLECTION_REGIONS.items():
        path, start_marker, end_marker, required_markers = region_config
        if not path.is_file():
            violations.append(
                f"{sdk} missing Ethereum data-collection source file: "
                f"{path.relative_to(ROOT)}"
            )
            continue
        region = source_region(path, start_marker, end_marker)
        for marker in required_markers:
            if marker not in region:
                violations.append(
                    f"{sdk} {path.relative_to(ROOT)} missing provider marker {marker}"
                )
        for label, pattern in ETHEREUM_DATA_COLLECTION_FORBIDDEN_PATTERNS.items():
            if pattern.search(region):
                violations.append(
                    f"{sdk} {path.relative_to(ROOT)} collection path contains forbidden {label}"
                )

    assert violations == []


def test_release_readiness_ethereum_js_dist_keeps_receipt_admission_guards() -> None:
    """Published JS must keep source receipt-proof admission checks in dist."""

    required_markers = (
        "eth_getBlockReceipts target receipt must match transactionHash",
        "eth_getBlockReceipts target receipt blockHash must match receipt",
        "eth_getBlockReceipts target receipt blockNumber must match receipt",
        "eth_getBlockReceipts target receipt RLP must match receipt",
        "typed receipt type is not supported for Ethereum mainnet receipt proofs",
        "const receiptTransactionHash = requireEthereumRpcHexData(",
        'const blockHash = requireEthereumRpcHexData(block.hash, "block.hash", 32);',
        "const executionBlockHash = nonZeroHex32Bytes(",
        "const executionReceiptsRoot = nonZeroHex32Bytes(",
        "const beaconFinalizedRoot = nonZeroHex32Bytes(",
        "const syncCommitteeRoot = nonZeroHex32Bytes(",
    )
    checked_paths = (
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
    )
    violations: list[str] = []
    for path in checked_paths:
        source = path.read_text(encoding="utf-8")
        for marker in required_markers:
            if marker not in source:
                violations.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_ethereum_sdks_keep_receipt_metadata_guards() -> None:
    """Ethereum SDK receipt-proof builders must reject block-receipt metadata drift."""

    sdk_markers = {
        "js-src": ((
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            (
                "eth_getBlockReceipts target receipt must match transactionHash",
                "eth_getBlockReceipts target receipt blockHash must match receipt",
                "eth_getBlockReceipts target receipt blockNumber must match receipt",
                "eth_getBlockReceipts target receipt RLP must match receipt",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
        ),),
        "js-dist": ((
            ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
            (
                "eth_getBlockReceipts target receipt must match transactionHash",
                "eth_getBlockReceipts target receipt blockHash must match receipt",
                "eth_getBlockReceipts target receipt blockNumber must match receipt",
                "eth_getBlockReceipts target receipt RLP must match receipt",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs",
            ),
        ),),
        "swift-sdk": (
            (
                ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
                (
                    '"blockReceipts.transactionHash"',
                    '"blockReceipts.blockHash"',
                    '"blockReceipts.blockNumber"',
                    '"blockReceipts.receiptRlp"',
                    "canonicalEvmReceiptRlp(currentReceipt)",
                ),
            ),
            (
                ROOT
                / "IrohaSwift"
                / "Sources"
                / "IrohaSwift"
                / "SccpSourceProofHashes.swift",
                (
                    "receiptType <= 0x7f",
                    "let admittedType = UInt8(receiptType)",
                    "(0x01...0x04).contains(admittedType)",
                ),
            ),
        ),
        "kotlin-sdk": (
            (
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
                / "EvmSccpProver.kt",
                (
                    "eth_getBlockReceipts target receipt must match transactionHash",
                    "eth_getBlockReceipts target receipt blockHash must match receipt",
                    "eth_getBlockReceipts target receipt blockNumber must match receipt",
                    "eth_getBlockReceipts target receipt RLP must match receipt",
                    "SccpSourceProofs.canonicalEvmReceiptRlp(receipt)",
                ),
            ),
            (
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
                / "SourceSccpProofHashes.kt",
                (
                    "typed receipt type must fit one byte below 0x80",
                    "val admittedType = receiptType.toInt()",
                    "typed receipt type is not supported for Ethereum mainnet receipt proofs",
                ),
            ),
        ),
        "java-android": (
            (
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
                / "EthereumMainnetSccp.java",
                (
                    "eth_getBlockReceipts target receipt must match transactionHash",
                    "eth_getBlockReceipts target receipt blockHash must match receipt",
                    "eth_getBlockReceipts target receipt blockNumber must match receipt",
                    "eth_getBlockReceipts target receipt RLP must match receipt",
                    "SourceSccpProofs.canonicalEvmReceiptRlp(receipt)",
                ),
            ),
            (
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
                / "SourceSccpProofs.java",
                (
                    "typed receipt type must fit one byte below 0x80",
                    "final int admittedType = receiptType.intValue()",
                    "typed receipt type is not supported for Ethereum mainnet receipt proofs",
                ),
            ),
        ),
        "dotnet-sdk": ((
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "EthereumMainnetSccp.cs",
            (
                "blockReceipts.transactionHash must match transactionHash.",
                "blockReceipts.blockHash must match receipt.",
                "blockReceipts.blockNumber must match receipt.",
                "blockReceipts.receiptRlp must match receipt.",
                "typed receipt type is not supported for Ethereum mainnet receipt proofs.",
            ),
        ),),
    }

    violations: list[str] = []
    for sdk, guarded_files in sdk_markers.items():
        for path, markers in guarded_files:
            source = path.read_text(encoding="utf-8")
            for marker in markers:
                if marker not in source:
                    violations.append(f"{sdk} {path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_ethereum_sdks_validate_provider_before_outbound_submitter() -> None:
    """Ethereum outbound submitter paths must honor configured mainnet providers."""

    sdk_markers = {
        "js-src": (
            ROOT / "javascript" / "iroha_js" / "src" / "sccp.js",
            (
                "let providerValidated = false;",
                "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
                "if (typeof submit === \"function\")",
            ),
        ),
        "js-dist": (
            ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js",
            (
                "let providerValidated = false;",
                "await this.validateExecutionProviderMainnet({ executionProvider: provider });",
                "if (typeof submit === \"function\")",
            ),
        ),
        "swift-sdk": (
            ROOT / "IrohaSwift" / "Sources" / "IrohaSwift" / "SccpEvmProver.swift",
            (
                "if let executionProvider {",
                "_ = try await validateExecutionProviderMainnet(executionProvider)",
                "return try await outboundSubmitFunction(submission)",
            ),
        ),
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
            / "EvmSccpProver.kt",
            (
                "executionProvider?.let { validateExecutionProviderMainnet(it) }",
                "return submitter.submit(buildEthereumCalldata(input))",
            ),
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
            / "EthereumMainnetSccp.java",
            (
                "if (executionProvider != null) {",
                "validateExecutionProviderMainnet(executionProvider);",
                "return outboundSubmitter.submit(buildEthereumCalldata(input));",
            ),
        ),
        "dotnet-sdk": (
            ROOT
            / "csharp"
            / "src"
            / "Hyperledger.Iroha.Sdk"
            / "Sccp"
            / "EthereumMainnetSccp.cs",
            (
                "IEthereumMainnetExecutionProvider? executionProvider",
                "ValidateExecutionProviderMainnetAsync(",
                "return await outboundSubmitter.SubmitAsync(submission, cancellationToken)",
            ),
        ),
    }

    violations: list[str] = []
    for sdk, (path, markers) in sdk_markers.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                violations.append(f"{sdk} {path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_evm_evidence_keeps_block_tag_metadata_guards() -> None:
    """Ethereum production evidence must keep finalized block-tag tripwires."""

    guarded_files = {
        ROOT / "scripts" / "sccp_evm_source_live_evidence.py": (
            'sccp_evm_source_block_tag = "',
            "--block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
            'sccp_evm_block_tag = "',
            "--block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_eth_source_bridge_evidence.py": (
            'sccp_evm_source_block_tag = "',
            "Ethereum source TOML requires --block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_evm_destination_evidence.py": (
            'sccp_evm_block_tag = "',
            "Ethereum destination TOML requires --block-tag finalized",
        ),
        ROOT / "scripts" / "sccp_bsc_source_bridge_evidence.py": (
            'sccp_evm_source_block_tag = "',
            '"latest"',
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            '"sccp_evm_source_rpc_chain_id": "_comment_evm_source_rpc_chain_id"',
            '"sccp_evm_source_block_tag": "_comment_evm_source_block_tag"',
            '"sccp_evm_rpc_chain_id": "_comment_evm_rpc_chain_id"',
            '"sccp_evm_block_tag": "_comment_evm_block_tag"',
            "EVM source live RPC chain-id must be canonical for {profile.chain}",
            "EVM live RPC chain-id must be canonical for {profile.chain}",
            "Ethereum source live block-tag metadata must be finalized",
            "Ethereum destination live block-tag metadata must be finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_source_live_evidence_test.py": (
            "test_evm_source_live_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_source_block_tag = "finalized"',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
            "test_live_evm_eth_toml_requires_finalized_block_tag",
            '# sccp_evm_block_tag = "finalized"',
        ),
        ROOT / "pytests" / "scripts" / "sccp_eth_source_bridge_evidence_test.py": (
            "test_eth_source_toml_rejects_nonfinalized_block_tag",
            "Ethereum source TOML requires --block-tag finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_destination_evidence_test.py": (
            "test_evm_destination_eth_toml_rejects_nonfinalized_block_tag",
            "Ethereum destination TOML requires --block-tag finalized",
        ),
        ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py": (
            "test_all_lanes_rejects_ethereum_nonfinalized_evm_live_metadata",
            '# sccp_evm_source_block_tag = "finalized"',
            '# sccp_evm_block_tag = "finalized"',
        ),
    }

    violations: list[str] = []
    for path, markers in guarded_files.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                violations.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert violations == []


def test_release_readiness_guards_evm_source_live_production_surface() -> None:
    """Ethereum source evidence must keep live production deployment guards."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_source_live_evidence.py": (
            'return "finalized" if domain == SCCP_DOMAIN_ETH else "latest"',
            "eth_chainId for {chain} lane must be canonical mainnet chain id",
            "deployment transaction receipt status must be 0x1",
            "deployment receipt contractAddress does not match source bridge",
            "deployment transaction hash does not match requested deployment transaction",
            "deployment transaction to must be null for contract creation",
            "deployment transaction input must not be empty or zero",
            "deployment receipt blockHash does not match eth_getBlockByNumber",
            "eth_getBlockByNumber receiptsRoot",
            "source bridge code hash at deployment receipt block does not",
            "source bridge runtime bytecode at deployment receipt block does",
            "deployment receipt block is newer than the finalized execution block",
            "deployment receipt block hash does not match the finalized execution block",
            "source bridge runtime bytecode hash must match bridge_code_hash",
            "deployment receipt block receiptsRoot metadata must be verified",
            "Ethereum source deployment receipt block finality metadata must be verified",
            "source verifier material hash metadata must match canonical inputs",
            "source adapter engine deployment hash metadata must match canonical inputs",
            "expected source verifier material hash argument must match ",
            "expected source adapter engine deployment hash argument must match ",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_source_live_evidence_test.py": (
            "test_evm_source_live_evidence_rejects_rpc_and_code_hash_drift",
            "test_evm_source_live_rejects_deployment_transaction_readback_drift",
            "test_evm_source_live_rejects_missing_or_drifted_receipt_contract_address",
            "test_evm_source_live_rejects_receipt_block_hash_drift",
            "test_evm_source_live_rejects_receipt_block_number_drift",
            "test_evm_source_live_rejects_unfinalized_deployment_receipt_block",
            "test_evm_source_live_rejects_finalized_deployment_receipt_hash_drift",
            "test_evm_source_live_rejects_zero_receipt_block_receipts_root",
            "test_evm_source_live_rejects_receipt_block_code_hash_drift",
            "test_evm_source_live_toml_revalidates_imported_summary_metadata",
            "test_evm_source_live_toml_requires_independent_pins",
        ),
    }

    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_evm_live_destination_production_surface() -> None:
    """Ethereum destination evidence must keep live production binding guards."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
            "eth_chainId for {chain} lane must be canonical mainnet chain id",
            "verifierCodeHash() does not match eth_getCode runtime bytecode",
            "verifierKeyHash() does not match verifier verifyingKeyHash()",
            "destinationBindingHash() does not match canonical live deployment inputs",
            "bridge runtime bytecode hash must match bridge_code_hash",
            "verifier runtime bytecode hash must match verifier_code_hash",
            "verifier key hash metadata must match verifyingKeyHash",
            "destination binding hash metadata must match canonical live inputs",
            "destination binding key metadata must match canonical inputs",
            "route-canary MessageProofAccepted destinationBindingHash does not",
            "route-canary MessageProofAccepted verifierBackendHash does not",
            "route-canary MessageProofAccepted proofFamilyHash does not match",
            "route-canary MessageProofAccepted networkId does not match networkId()",
            "route-canary transaction calldata must call",
            "submitSccpMessageProof(bytes,bytes32[6],bytes32)",
            "route-canary proofBytes must be a 384-byte Groth16 tuple",
            "route-canary proofBytes must not be all zero",
            "route-canary proof version must be 1",
            "route-canary proof sourceDomain does not match expectedSourceDomain()",
            "usedMessageProofs(bytes32) is false",
            'and transaction.get("message_proof_used") is True',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
            "test_live_evm_evidence_rejects_verifier_code_hash_drift",
            "test_live_evm_evidence_rejects_bridge_code_hash_drift",
            "test_live_evm_evidence_rejects_bridge_destination_binding_drift",
            "test_live_evm_full_toml_revalidates_imported_summary_metadata",
            "test_live_evm_route_canary_rejects_unverified_transaction_metadata",
            "route_canary_call_data_mutator",
            "proofBytes offset must be 256 bytes",
            "publicInputs[0] must match event messageId",
            "targetDomain does not match expectedTargetDomain()",
            "publicInputs[3] must match event commitmentRoot",
            "statementHash must match accepted event",
            "proofBytes must be a 384-byte Groth16 tuple",
            "proofBytes must not be all zero",
            "proof version must be 1",
            "proof sourceDomain does not match expectedSourceDomain()",
            "usedMessageProofs(bytes32) is false",
        ),
    }

    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_evm_route_canary_finalized_receipt_block() -> None:
    """Ethereum route canaries must bind receipt blocks to finalized execution heads."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_live_evidence.py": (
            "def _route_canary_finalized_block_summary(",
            '"eth_getBlockByNumber"',
            '["finalized", False]',
            "route-canary receipt block is newer than the finalized execution block",
            "route-canary receipt block hash does not match the finalized execution block",
            '"receipt_block_finalized": True',
            'and transaction.get("receipt_block_finalized") is True',
            'route_canary_transaction.get("receipt_block_finalized") is True',
            'receipt_block_finalized=finalized_block["receipt_block_finalized"]',
        ),
        ROOT / "scripts" / "sccp_evm_destination_evidence.py": (
            'EVM_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:evm-route-canary-evidence:v4"',
            "receipt_block_finalized: bool",
            "receipt_block_finalized must be a boolean for EVM route canaries",
            'receipt_block_finalized=values["receipt_block_finalized"]',
            "route_canary_receipt_block_finalized",
            "--route-canary-receipt-block-finalized",
            "from finalized live reads",
            "evm_route_canary_receipt_block_finalized",
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            "evm_route_canary_receipt_block_finalized",
            "_comment_evm_route_canary_receipt_block_finalized",
            "EVM route canary receipt block finalized metadata must be true",
            "receipt_block_finalized=receipt_block_finalized",
            'canary["receipt_block_finalized"] = True',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_live_evidence_test.py": (
            "route_canary_finalized_block_number",
            'params[0] == "finalized"',
            '"receipt_block_finalized"] is True',
            '"receipt_block_finalized"] is False',
            "evm_route_canary_receipt_block_finalized = true",
            "receipt_block_finalized=True",
            "receipt_block_finalized=False",
            "transaction to does not match destination bridge",
            'block_tag="finalized" if finality_expected else "latest"',
            "test_live_evm_bsc_default_latest_route_canary_stays_diagnostic",
            "receipt block is newer than the finalized execution block",
            "receipt block hash does not match the finalized execution block",
        ),
        ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py": (
            "test_all_lanes_rejects_evm_route_canary_missing_finalized_receipt_state",
            "_comment_evm_route_canary_receipt_block_finalized",
            "receipt_block_finalized=True",
            "receipt block finalized metadata must be true",
        ),
        ROOT / "crates" / "iroha_sccp" / "src" / "lib.rs": (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            'b"iroha:sccp:evm-route-canary-evidence:v4"',
            "push_u8(&mut out, u8::from(receipt_block_finalized));",
            "|| !receipt_block_finalized",
            "allowlist.evm_route_canary_receipt_block_finalized = Some(true);",
            "non-finalized diagnostic EVM route canary hash",
            "evm_route_canary_evidence_hash_matches_destination_script_vector",
            "84b93b0050b6bc9696ba55d56a8c957171e6a4ebd2f242b683762d52d88db9d7",
        ),
        ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs": (
            "pub evm_route_canary_receipt_block_finalized: Option<bool>",
            "evm_route_canary_receipt_block_finalized: self.evm_route_canary_receipt_block_finalized",
        ),
        ROOT / "crates" / "iroha_core" / "src" / "smartcontracts" / "isi" / "world.rs": (
            "evm_route_canary_receipt_block_finalized: configured",
            "configured_sccp_all_lanes_launch_rejects_evm_non_finalized_route_canary",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


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


def test_release_readiness_native_local_prover_guard_covers_identifier_variants() -> None:
    """The native/local-prover guard must catch common remote-prover spellings."""

    samples = {
        "WebAssembly": "const engine = WebAssembly.compile(bytes)",
        "wasm": "import './prover.wasm'",
        "snarkjs": "import snarkjs from 'snarkjs'",
        "remoteProver": "const remoteProver = endpoint",
        "remote prover": "fall back to a remote prover",
        "remote_prover": "remote_prover = 'https://example.invalid'",
        "remote-prover": "remote-prover endpoint",
        "proverUrl": "const proverUrl = config.prover",
        "proverURL": "const proverURL = config.prover",
        "prover_url": "prover_url = config.prover",
        "proverEndpoint": "const proverEndpoint = config.prover",
        "prover_endpoint": "prover_endpoint = config.prover",
    }

    missing = [
        label
        for label, sample in samples.items()
        if not BSC_FORBIDDEN_PROVER_DEPENDENCY_PATTERNS[label].search(sample)
    ]

    assert missing == []


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
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["swift-sdk"]
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
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
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
    for symbol in (
        "EthereumMainnetSccp.buildOutboundProofRequest",
        "EthereumMainnetSccp.proveOutboundToEthereum",
        "EthereumMainnetSccp.buildEthereumCalldata",
    ):
        assert symbol in surfaces["eth,bsc"]["sdk_helper_symbols_by_sdk"]["java-android"]
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
    assert "EVM Source Chain ID | EVM Source Tag | EVM Destination Chain ID" in (
        completed.stdout
    )
    assert "`eth` | `1` | `finalized` | `1` | `finalized`" in completed.stdout
    assert "`bsc` | `56` | `latest` | `56` | `latest`" in completed.stdout
    assert "Source Material | Source Deployment | Destination Binding" in (
        completed.stdout
    )
    assert "Source Gate | Source Gate Audits | Route Allowlist" in completed.stdout
    assert "Canary Tx | Canary Receipt Block | Canary Receipt Hash" in completed.stdout
    assert "Canary Receipt Finalized | Canary Receipts Root" in completed.stdout
    assert "Canary Receipts Root | Canary Message ID | Canary Block" in (
        completed.stdout
    )
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
    expected_chain_id = active_evm_live_chain_id(report)
    assert active_crypto["domain"] == active_domain
    assert active_crypto["evm_source_rpc_chain_id"] == expected_chain_id
    assert active_crypto["evm_source_block_tag"] == "finalized"
    assert active_crypto["evm_destination_rpc_chain_id"] == expected_chain_id
    assert active_crypto["evm_destination_block_tag"] == "finalized"
    assert isinstance(active_crypto["route_canary_transaction_hash"], str)
    assert active_crypto["route_canary_transaction_hash"].startswith("0x")
    assert type(active_crypto["route_canary_receipt_block_number"]) is int
    assert active_crypto["route_canary_receipt_block_number"] > 0
    assert isinstance(active_crypto["route_canary_receipt_block_hash"], str)
    assert active_crypto["route_canary_receipt_block_hash"].startswith("0x")
    assert active_crypto["route_canary_receipt_block_finalized"] is True
    assert isinstance(active_crypto["route_canary_block_receipts_root"], str)
    assert active_crypto["route_canary_block_receipts_root"].startswith("0x")
    assert isinstance(active_crypto["route_canary_message_id"], str)
    assert active_crypto["route_canary_message_id"].startswith("0x")
    blocked_future_lanes = [
        lane
        for lane in payload["evidence"]["lanes"]
        if lane["domain"] != active_domain and not lane["production_ready"]
    ]
    assert blocked_future_lanes


def test_release_readiness_report_blocks_active_launch_evm_live_metadata_drift(
    tmp_path: Path,
) -> None:
    """Active Ethereum launch readiness must surface mainnet/finalized live-read drift."""

    evidence, evidence_payload = write_active_launch_evidence(tmp_path)
    replacements = (
        (
            '# sccp_evm_source_rpc_chain_id = "1"',
            '# sccp_evm_source_rpc_chain_id = "2"',
        ),
        (
            '# sccp_evm_source_block_tag = "finalized"',
            '# sccp_evm_source_block_tag = "latest"',
        ),
        ('# sccp_evm_rpc_chain_id = "1"', '# sccp_evm_rpc_chain_id = "2"'),
        ('# sccp_evm_block_tag = "finalized"', '# sccp_evm_block_tag = "latest"'),
    )
    for expected, replacement in replacements:
        assert expected in evidence_payload
        evidence_payload = evidence_payload.replace(expected, replacement, 1)
    evidence.write_text(evidence_payload, encoding="utf-8")

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
    payload = json.loads(completed.stdout)
    checklist = {
        item["id"]: item for item in payload["release_checklist"]["items"]
    }
    governed = checklist["governed_deployment_evidence"]
    assert governed["ready"] is False
    assert (
        "domain 1 (eth): Ethereum mainnet source live eth_chainId must be 1 (0x1)"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet destination live eth_chainId must be 1 (0x1)"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet source live block tag must be finalized"
        in governed["blockers"]
    )
    assert (
        "domain 1 (eth): Ethereum mainnet destination live block tag must be finalized"
        in governed["blockers"]
    )


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


def test_release_readiness_rejects_java_android_log_without_source_harness(
    tmp_path: Path,
) -> None:
    """The Android phase log must include source-proof harness selection."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "java-android-without-source-harness.log"
    source_harness = "org.hyperledger.iroha.android.sccp.SourceSccpProofsTests"
    corridor_log.write_text(
        "==> SCCP production corridor: java-android\n"
        "+ ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.sccp.EvmSccpProverTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests\n"
        "+ ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests\n"
        "BUILD SUCCESSFUL\n"
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
            f"java-android={corridor_log}",
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
        "production corridor phase java-android evidence artifact is missing "
        f"expected phase-block command: {source_harness}"
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


def test_release_readiness_report_requires_ethereum_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the browser Ethereum path stayed native JS."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    ethereum_no_wasm_marker = (
        "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert ethereum_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != ethereum_no_wasm_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-ethereum-no-wasm-marker.log"
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
        f"expected phase-block success marker: {ethereum_no_wasm_marker}"
    ) in completed.stdout


def test_release_readiness_sccp_allow_unready_transparent_proofs_is_config_only() -> None:
    """SCCP unready transparent-proof bypasses must be sourced from TOML config."""

    user_config = ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs"
    service = ROOT / "configs" / "soranexus" / "taira" / "taira-irohad.service"
    bootstrap = (
        ROOT / "configs" / "soranexus" / "taira" / "bootstrap_kaigi_localnet.sh"
    )
    taira_config = ROOT / "configs" / "soranexus" / "taira" / "config.toml"

    for path in (user_config, service, bootstrap):
        assert "ZK_SCCP_ALLOW_UNREADY_TRANSPARENT_PROOFS" not in path.read_text(
            encoding="utf-8"
        )
    assert (
        "pub sccp_allow_unready_transparent_proofs: bool"
        in user_config.read_text(encoding="utf-8")
    )
    assert (
        "sccp_allow_unready_transparent_proofs = true"
        in bootstrap.read_text(encoding="utf-8")
    )
    assert (
        "sccp_allow_unready_transparent_proofs = false"
        in taira_config.read_text(encoding="utf-8")
    )


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


def test_release_readiness_report_requires_ethereum_facade_declaration_marker(
    tmp_path: Path,
) -> None:
    """The JS phase evidence must prove the Ethereum facade declarations were tested."""

    evidence, _ = write_complete_evidence(tmp_path)
    report = load_report_module()
    declaration_marker = "package declarations expose Ethereum mainnet SCCP facade methods"
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    corridor_log = tmp_path / "js-sdk-without-ethereum-facade-declaration-marker.log"
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


def test_release_readiness_guards_ethereum_inbound_adversarial_sdk_tests() -> None:
    """Native/browser Ethereum inbound tests must retain adversarial evidence cases."""

    guarded_tests = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "EthereumMainnetSccp rejects failed or drifted receipt evidence before proving",
            "receipt status must be 0x1",
            "beaconFinality.executionReceiptsRoot",
            "EthereumMainnetSccp validates source bridge logs in receipt evidence",
            "sourceEventLog(), sourceEventLog()",
            "/exactly 2 topics/u",
            "/source event log data must be 0x/u",
            "/source event digest must not be zero/u",
            "/removed logs/u",
            'for (const missingField of ["transactionHash", "blockHash", "blockNumber"])',
            '["transaction_hash", hex32("ab"), "receipt.logs[0].transactionHash"]',
            '["block_hash", hex32("ac"), "receipt.logs[0].blockHash"]',
            '["block_number", "0x1235", "receipt.logs[0].blockNumber"]',
            "must not use multiple aliases",
            "receipt_proof_hash: receiptProofHash",
            'receiptProofHash: hex32("00")',
            "receiptProofHash: evmSccpReceiptProofHash(sampleReceiptProof)",
            "/requires receiptProof/u",
            "/transactionHash must not be zero/u",
            "/blockHash must not be zero/u",
            "/receipt\\.transactionHash must not be zero/u",
            "/receipt\\.blockHash must not be zero/u",
            "/block\\.hash must not be zero/u",
            "/block\\.receiptsRoot must not be zero/u",
            "EthereumMainnetSccp requires linked local prover functions",
            "ERR_SCCP_ETH_INBOUND_PROVER_UNAVAILABLE",
            "assert.equal(executionRequests, 0)",
            "/requires receipt source event validation/u",
            '["executionBlockHash", /executionBlockHash must not be zero/u]',
            '["executionReceiptsRoot", /executionReceiptsRoot must not be zero/u]',
            '["beaconFinalizedRoot", /beaconFinalizedRoot must not be zero/u]',
            '["syncCommitteeRoot", /syncCommitteeRoot must not be zero/u]',
            "SAMPLE_SYNC_COMMITTEE_BITS",
            "LOW_SYNC_COMMITTEE_BITS",
            "/beaconFinality\\.finalityBranch/u",
            "/beaconFinality\\.syncCommitteeBits/u",
            "/beaconFinality\\.syncCommitteeBits must contain Ethereum sync committee supermajority/u",
            "/beaconFinality\\.syncCommitteeParticipation must match syncCommitteeBits/u",
            "/beaconFinality\\.syncSignatureSlot must cover beaconFinality\\.beaconSlot/u",
            "/beaconFinality\\.syncCommitteeSignature must not be zero/u",
            "/sync_committee_bits must contain Ethereum sync committee supermajority/u",
            "/sync_committee_signature must not be zero/u",
            "aliasOnlyProverCalls",
            "assert.equal(alias in evidence.beaconFinality, false)",
            "Ethereum receipt-proof transcript rejects empty trie and finality branches",
            'fullReceipt(0, { transaction_index: "0x0" })',
            '["transaction_hash", hex32("ab"), "receipt.transactionHash"]',
            '["receipts_root", hex32("ab"), "block.receiptsRoot"]',
            '["block_hash", hex32("ab"), "blockReceipts.blockHash"]',
            '["cumulative_gas_used", "0x5208", "receipt.cumulativeGasUsed"]',
            '["logs_bloom", `0x${"11".repeat(256)}`, "receipt.logsBloom"]',
            "receiptTrieProofNodes: []",
            "inclusionBranch: []",
            "sourceDomain: SCCP_DOMAIN_BSC",
            "/sourceDomain must be ETH/u",
        ),
        ROOT / "python" / "iroha_torii_client" / "sccp.py": (
            "_normalize_ethereum_mainnet_finality_branch",
            "beaconFinality.finalityBranch",
            "must contain 6 siblings",
            "_normalize_ethereum_mainnet_finality_sync_committee_bits",
            "must contain Ethereum sync committee supermajority",
            "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot",
            "receiptProof.syncCommitteeRoot must match beaconFinality.syncCommitteeRoot",
            "receiptProof.beaconSlot must match beaconFinality.beaconSlot",
        ),
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
            "ETHEREUM_FINALITY_BRANCH",
            "LOW_ETHEREUM_SYNC_COMMITTEE_BITS",
            'evidence["beacon_finality"]["finality_branch"]',
            "finalityBranch must contain 6 siblings",
            "beaconFinality.syncCommitteeParticipation",
            "receiptProof.beaconFinalizedRoot",
            "receiptProof.syncCommitteeRoot",
            "receiptProof.beaconSlot",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            'invalidPublicInputs("receipt.status")',
            'invalidPublicInputs("beaconFinality.executionReceiptsRoot")',
            "wrongTopicReceipt",
            "extraTopicReceipt",
            'invalidPublicInputs("receipt.logs[0].topics")',
            "nonEmptyDataReceipt",
            'invalidPublicInputs("receipt.logs[0].data")',
            "zeroDigestReceipt",
            'invalidPublicInputs("receipt.logs[0].topics[1]")',
            "duplicateLogReceipt",
            "removedLogReceipt",
            'invalidPublicInputs("receipt.logs")',
            'for missingField in ["transactionHash", "blockHash", "blockNumber"]',
            "EthereumMainnetInboundEvidence(receiptProofHash: receiptProofHash)",
            'String(repeating: "00", count: 32)',
            'receiptProofHash + " "',
            'XCTFail("prover callback must not run without receiptProof")',
            'XCTFail("prover callback must not run without source event validation")',
            'invalidPublicInputs("receiptProof")',
            'missingFinalityBranchFinality.removeValue(forKey: "finalityBranch")',
            'invalidPublicInputs("beaconFinality.finalityBranch")',
            'invalidPublicInputs("beaconFinality.syncCommitteeBits")',
            'mismatchedSyncParticipationFinality["syncCommitteeParticipation"] = "341"',
            'underQuorumSyncBitsFinality["syncCommitteeBits"] = "0x01" + String(repeating: "00", count: 63)',
            '.invalidPublicInputs("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")',
            'staleSyncSignatureSlotFinality["syncSignatureSlot"] = "31"',
            'zeroSyncCommitteeSignatureFinality["syncCommitteeSignature"] = "0x" + String(repeating: "00", count: 96)',
            '.zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature")',
            "let aliasOnlyFinality: [String: Any]",
            "XCTAssertFalse(finality.keys.contains(alias))",
            '"finalized_header_root", "0x" + String(repeating: "13", count: 32)',
            '"sync_committee_root", "0x" + String(repeating: "14", count: 32)',
            '"beacon_slot", "33", "beaconFinality.beaconSlot"',
            '"transaction_hash", "0x" + String(repeating: "ab", count: 32)',
            '"block_hash", "0x" + String(repeating: "ac", count: 32)',
            '"block_number", "0x1235", "receipt.logs[0].blockNumber"',
            '.invalidRlp("blockReceipts[0].transactionHash")',
            '.invalidRlp("receipt.cumulativeGasUsed")',
            '.invalidRlp("receipt.logsBloom")',
            'receiptTrieProofNodes: []',
            '.invalidValidatorSet("receiptTrieProofNodes")',
            'inclusionBranch: []',
            '.invalidBranch("inclusionBranch")',
            "sourceDomain: sccpDomainBsc",
            "sourceDomain: sccpDomainEthereum",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "emptyEvmReceiptNodes",
            "receiptTrieProofNodes = emptyList()",
            "emptyEvmInclusionBranch",
            "inclusionBranch = emptyList()",
            "inclusionBranch must not be empty",
            "emptyBscInclusionBranch",
            "bscDomainEvmReceiptProof",
            "sourceDomain must be ETH",
            "ethDomainBscReceiptProof",
            "sourceDomain must be BSC",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            'receipt + ("status" to "0x0")',
            'beaconFinality + ("executionReceiptsRoot"',
            '"logs" to listOf(sourceEventLog, sourceEventLog)',
            '"0x" + "66".repeat(32)',
            '"data" to "0x01"',
            '"0x" + "00".repeat(32)',
            'sourceEventLog + ("removed" to true)',
            "SccpEthereumMainnet.sourceEventTopic()",
            "receiptProof.executionReceiptsRoot",
            'for (missingField in listOf("transactionHash", "blockHash", "blockNumber"))',
            "EthereumMainnetInboundEvidence(receiptProofHash = receiptProofHash)",
            "receiptProofHash must not be zero",
            'receiptProofHash + " "',
            "val missingReceiptProof = assertFailsWith<IllegalArgumentException>",
            'missingReceiptProof.message?.contains("receiptProof")',
            "prebuiltProofOnlyProverCalls",
            'prebuiltProofWithoutSourceEvent.message?.contains("receipt source event validation")',
            'missingFinalityBranch.message?.contains("beaconFinality.finalityBranch")',
            'missingSyncBits.message?.contains("beaconFinality.syncCommitteeBits")',
            'mismatchedSyncParticipation.message?.contains("beaconFinality.syncCommitteeParticipation")',
            'underQuorumSyncBits.message?.contains("beaconFinality.syncCommitteeBits")',
            'sync_committee_bits must contain Ethereum sync committee supermajority',
            'staleSyncSignatureSlot.message?.contains("beaconFinality.syncSignatureSlot")',
            'zeroSyncCommitteeSignature.message?.contains("beaconFinality.syncCommitteeSignature")',
            'beaconFinalityUpdateJson(syncCommitteeSignature = "0x" + "00".repeat(96))',
            "val aliasOnlyFinality = mapOf<String, Any?>",
            "assertTrue(alias !in finality)",
            'Triple("finalized_header_root", "0x" + "13".repeat(32), "beaconFinality.finalizedHeaderRoot")',
            'Triple("sync_committee_root", "0x" + "14".repeat(32), "beaconFinality.syncCommitteeRoot")',
            'Triple("beacon_slot", "33", "beaconFinality.beaconSlot")',
            'Triple("transaction_hash", "0x" + "ab".repeat(32), "receipt.logs[0].transactionHash")',
            'Triple("block_hash", "0x" + "ac".repeat(32), "receipt.logs[0].blockHash")',
            'Triple("block_number", "0x1235", "receipt.logs[0].blockNumber")',
            'Triple("transaction_hash", "0x" + "ac".repeat(32), "receipt.transactionHash")',
            'blockReceipts[0].transactionHash',
            'receipt + ("cumulative_gas_used" to "0x5208")',
            'receipt + ("logs_bloom" to ("0x" + "00".repeat(256)))',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "Ethereum inbound collection must reject failed receipts",
            "beaconFinality.executionReceiptsRoot",
            'duplicateReceipt.put("logs"',
            "source-event validation must reject duplicate matching events",
            "Ethereum source-event validation must reject extra source-event topics",
            "Ethereum source-event validation must reject non-empty source-event data",
            "Ethereum source-event validation must reject zero source-event digest",
            "Ethereum source-event validation must reject removed logs",
            "EthereumMainnetSccp.sourceEventTopic()",
            'Arrays.asList("transactionHash", "blockHash", "blockNumber")',
            "hash-only receiptProofHash evidence",
            '"0x" + repeat("00", 32)',
            'receiptProofHash + " "',
            "Ethereum inbound proving must reject hash-only receipt proof evidence",
            "Ethereum inbound prover must not run without receipt proof material",
            "prebuiltProofOnlyProverCalls",
            "Ethereum inbound proving must reject proof-only evidence without source event validation",
            "Ethereum inbound proving must reject missing finality branch",
            "Ethereum inbound proving must reject missing sync-committee bits",
            'mismatchedSyncParticipationFinality.put("syncCommitteeParticipation", "341")',
            "Ethereum inbound proving must reject under-quorum sync-committee bits",
            "Beacon REST provider must reject under-quorum sync committee aggregate bits",
            'staleSyncSignatureSlotFinality.put("syncSignatureSlot", "31")',
            "Ethereum inbound proving must reject zero sync-committee signatures",
            "Beacon REST provider must reject zero sync committee aggregate signatures",
            'aliasOnlyFinality.put("execution_block_number", "0x1234")',
            "callback finality must not retain alias",
            "final Object[][] conflictingFinalityAliases",
            '"finalized_header_root", "0x" + repeat("13", 32), "beaconFinality.finalizedHeaderRoot"',
            '"sync_committee_root", "0x" + repeat("14", 32), "beaconFinality.syncCommitteeRoot"',
            "final Object[][] conflictingLogAliases",
            '"transaction_hash", "0x" + repeat("ab", 32), "receipt.logs[0].transactionHash"',
            '"block_hash", "0x" + repeat("ac", 32), "receipt.logs[0].blockHash"',
            "final String[][] receiptAliasConflicts",
            "blockReceipts[0].transactionHash",
            'conflictingGas.put("cumulative_gas_used", "0x5208")',
            'conflictingBloom.put("logs_bloom", "0x" + repeat("00", 256))',
            "Ethereum receipt-proof transcript must reject empty receiptTrieProofNodes",
            "Ethereum receipt-proof transcript must reject empty inclusionBranch",
            "Ethereum receipt-proof transcript must reject BSC sourceDomain",
            "BSC receipt-proof transcript must reject ETH sourceDomain",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "failedReceipt",
            "receiptProof.executionReceiptsRoot",
            "driftedFinalityReceiptsRoot",
            "wrongTopicLog",
            "extraTopicReceipt",
            'Assert.Contains("exactly 2 topics", extraTopic.Message)',
            "nonEmptyDataReceipt",
            'Assert.Contains("data must be 0x", nonEmptyData.Message)',
            "zeroDigestReceipt",
            'Assert.Contains("digest must not be zero", zeroDigest.Message)',
            "duplicateReceipt",
            'Assert.Contains("removed logs", removedSourceEventLog.Message)',
            'foreach (var missingField in new[] { "transactionHash", "blockHash", "blockNumber" })',
            "Assert.Null(receiptProofHashOnlyEvidence.ReceiptProof)",
            "ReceiptProofHash must not be zero",
            'ExpectedReceiptProofHash + " "',
            "missingReceiptProofProver",
            'Assert.Contains("receiptProof", missingReceiptProof.Message)',
            "unanchoredReceiptProofProver",
            'Assert.Contains("receipt source event validation", unanchoredReceiptProof.Message)',
            'missingFinalityBranchFinality.Remove("finalityBranch")',
            'Assert.Contains("beaconFinality.finalityBranch", missingFinalityBranch.Message)',
            'Assert.Contains("beaconFinality.syncCommitteeBits", missingSyncBits.Message)',
            "mismatchedSyncParticipationFinality",
            'Assert.Contains("beaconFinality.syncCommitteeParticipation", mismatchedSyncParticipation.Message)',
            "underQuorumSyncBitsFinality",
            'Assert.Contains("beaconFinality.syncCommitteeBits", underQuorumSyncBits.Message)',
            'Assert.Contains("sync_committee_bits must contain Ethereum sync committee supermajority", underQuorumSyncAggregate.Message)',
            '["syncSignatureSlot"] = "31"',
            'Assert.Contains("beaconFinality.syncSignatureSlot", staleSyncSignatureSlot.Message)',
            'Assert.Contains("beaconFinality.syncCommitteeSignature", zeroSyncCommitteeSignature.Message)',
            'Assert.Contains("sync_committee_signature must not be zero", zeroSyncAggregateSignature.Message)',
            "var aliasOnlyFinality = new Dictionary<string, object?>",
            "Assert.False(finality.ContainsKey(alias))",
            '("finalized_header_root", "0x" + string.Concat(Enumerable.Repeat("13", 32)), "beaconFinality.finalizedHeaderRoot")',
            '("sync_committee_root", "0x" + string.Concat(Enumerable.Repeat("14", 32)), "beaconFinality.syncCommitteeRoot")',
            '("beacon_slot", "33", "beaconFinality.beaconSlot")',
            '("transaction_hash", "0x" + new string(\'d\', 64), "receipt.logs[0].transactionHash")',
            '("block_hash", "0x" + new string(\'a\', 64), "receipt.logs[0].blockHash")',
            '("block_number", "0x1235", "receipt.logs[0].blockNumber")',
            '("transaction_hash", "0x" + new string(\'a\', 64), "receipt.transactionHash")',
            'Assert.Contains("blockReceipts[0].transactionHash", indexedHashAliasConflict.Message);',
            '["cumulative_gas_used"] = "0x5208"',
            '["logs_bloom"] = logsBloom',
            "Assert.Throws<ArgumentException>(() => BuildBytes(sourceDomain: 2));",
            "Assert.Throws<ArgumentException>(() => BuildBytes(nodes: Array.Empty<byte[]>()));",
            "Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: Array.Empty<byte[]>()));",
        ),
    }
    missing: list[str] = []
    for path, markers in guarded_tests.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_outbound_precallback_sdk_tests() -> None:
    """Ethereum outbound facades must reject foreign lanes before callbacks."""

    guarded_tests = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert.equal(outboundProverCalled, false)",
            "ERR_SCCP_ETH_OUTBOUND_PROVER_UNAVAILABLE",
            "local JS\\/native EVM prover",
            "Ethereum mainnet SCCP outbound from",
            "submittedTxs[3].from",
        ),
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
            "destinationBindingHash must match destinationBinding",
            "outbound_prover_called = False",
            "assert not outbound_prover_called",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "Ethereum outbound prover callback must not see BSC requests",
            "XCTAssertFalse(outboundProverCalled)",
            "Ethereum outbound facade must reject forged destinationBindingHash before returning request",
            "forgedBindingHashRequest",
            "String(repeating: \"99\", count: 32)",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "Ethereum outbound prover callback must not see BSC requests",
            "outboundProverCalled",
            'request.copy(destinationBindingHash = "0x" + "99".repeat(32))',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "Ethereum outbound prover callback must not see BSC requests",
            "assert !outboundProverCalled[0]",
            "Ethereum wrapProofResult must reject forged destinationBindingHash",
            "evmRequestWithDestinationBindingHash",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "Ethereum outbound prover callback must not see BSC requests",
            "Assert.Null(guardedProver.Request)",
            "request with { DestinationBindingHash = \"0x\" + new string('9', 64) }",
        ),
    }
    missing = []
    for path, markers in guarded_tests.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_root_zero_sdk_tests() -> None:
    """Ethereum SDK receipt-root helpers must reject zero typed MPT roots."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "export function canonicalEvmReceiptRootMptValue(receiptRoot)",
            'const root = nonZeroHex32Bytes(receiptRoot, "receiptRoot");',
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "canonicalEvmReceiptRootMptValue(SCCP_ZERO_HASH_V1)",
            "must not be zero",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "package_dist.test.js": (
            'canonicalEvmReceiptRootMptValue(`0x${"00".repeat(32)}`)',
            "must not be zero",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "public func canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "canonicalEvmReceiptRootMptValue(receiptRoot: zeroHash)",
            "XCTAssertThrowsError",
        ),
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
        / "SourceSccpProofHashes.kt": (
            "fun canonicalEvmReceiptRootMptValue(receiptRoot: String)",
            'rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot"))',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "SccpSourceProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "assertFailsWith<IllegalArgumentException>",
        ),
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
        / "SourceSccpProofs.java": (
            "public static byte[] canonicalEvmReceiptRootMptValue(final String receiptRoot)",
            'fields.add(rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot")))',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "SourceSccpProofs.canonicalEvmReceiptRootMptValue(zeroHash)",
            "expectThrows",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "public static byte[] CanonicalEvmSccpReceiptProofBytes",
            "payload.Write(RpcHexToBytes(executionReceiptsRoot, nameof(executionReceiptsRoot), 32));",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "BuildBytes(executionReceiptsRoot: zeroRoot)",
            "BuildBytes(syncCommitteeRoot: zeroRoot)",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rlp_zero_topic_tests() -> None:
    """Ethereum receipt RLP builders must allow zero log topics."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "`receipt.logs[${index}].topics[${topicIndex}]`",
            "{ nonzero: false }",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "zeroTopicReceiptTrieProof",
            'topics: [hex32("00")]',
        ),
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{log_index}].topics[{topic_index}]\"",
            "nonzero=False",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_accepts_zero_log_topic_in_receipt_rlp",
            '"topics": ["0x" + "00" * 32]',
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            'field: "receipt.logs[\\(index)].topics[\\(topicIndex)]"',
            "nonzero: false",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "zeroTopicProof",
            '"topics": ["0x" + String(repeating: "00", count: 32)]',
        ),
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
        / "SourceSccpProofHashes.kt": (
            '"receipt.logs[$index].topics[$topicIndex]"',
            "nonzero = false",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "zeroTopicProof",
            '"topics" to listOf("0x" + "00".repeat(32))',
        ),
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
        / "SourceSccpProofs.java": (
            '"receipt.logs[" + index + "].topics[" + topicIndex + "]"',
            "false,\n                    false)))",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "zeroTopicProof",
            "generic Ethereum receipt RLP must allow zero log topics",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            '$"receipt.logs[{index}].topics[{topicIndex}]"',
            "nonZero: false",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "zeroTopicProof",
            '["topics"] = new object?[] { "0x" + new string(\'0\', 64) }',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rlp_zero_address_tests() -> None:
    """Ethereum receipt RLP builders must allow zero log addresses."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "`receipt.logs[${index}].address`",
            "{ nonzero: false }",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "zeroAddressReceiptTrieProof",
            'address: `0x${"00".repeat(20)}`',
        ),
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{log_index}].address\"",
            "nonzero=False",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_accepts_zero_log_address_in_receipt_rlp",
            '"address": "0x" + "00" * 20',
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            'field: "receipt.logs[\\(index)].address"',
            "nonzero: false",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "zeroAddressProof",
            '"address": "0x" + String(repeating: "00", count: 20)',
        ),
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
        / "SourceSccpProofHashes.kt": (
            '"receipt.logs[$index].address"',
            "nonzero = false",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "zeroAddressProof",
            '"address" to "0x" + "00".repeat(20)',
        ),
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
        / "SourceSccpProofs.java": (
            '"receipt.logs[" + index + "].address"',
            "false,\n                          false))",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "zeroAddressProof",
            "generic Ethereum receipt RLP must allow zero log addresses",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            '$"receipt.logs[{index}].address"',
            "nonZero: false",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "zeroAddressProof",
            '["address"] = "0x" + new string(\'0\', 40)',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_context_tests() -> None:
    """Ethereum source-event evidence must bind logs to receipt/block context."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "log_transaction_hash = _rpc_fixed_hex_data(",
            "log_block_hash = _rpc_fixed_hex_data(",
            "log_block_number = _rpc_quantity(",
            "source event log transactionHash does not match receipt",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_source_event_missing_context_fields",
            'for field in ("transactionHash", "blockHash", "blockNumber")',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_mode_tests() -> None:
    """Ethereum source-event evidence must be the default receipt collector mode."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "allow_receipt_only_evidence: bool = False",
            "source_bridge_address is required for SCCP source-event evidence",
            "--allow-receipt-only-evidence",
            '"evidence_mode": (',
            '"source_event_validated": source_event_digest is not None',
            '"receipt_only_evidence": source_event_digest is None',
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_requires_explicit_receipt_only_mode_without_source_bridge",
            "test_collect_receipt_proof_allows_explicit_receipt_only_mode",
            "test_cli_requires_source_bridge_or_explicit_receipt_only_mode",
            "test_cli_exposes_explicit_receipt_only_mode",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_event_zero_digest_tests() -> None:
    """Ethereum source-event evidence must reject zero event digests."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "method=f\"receipt.logs[{index}].topics[1]\"",
            "raise RuntimeError(f\"{method} returned zero data\")",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_zero_source_event_digest",
            '"topics": [module.EVM_SOURCE_EVENT_TOPIC, "0x" + "00" * 32]',
            "zero source event digest was accepted",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_receipt_rpc_duplicate_json_tests() -> None:
    """Ethereum receipt evidence RPC parsing must reject duplicate JSON keys."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "_json_object_without_duplicate_keys",
            "JSON-RPC returned duplicate JSON key",
            "object_pairs_hook=_json_object_without_duplicate_keys",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "FakeRawResponse",
            "test_collect_receipt_proof_rejects_duplicate_json_rpc_result_keys",
            "test_collect_receipt_proof_rejects_duplicate_json_receipt_fields",
            "duplicate JSON-RPC result keys were accepted",
            "duplicate JSON receipt fields were accepted",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_block_receipt_transaction_hash_tests() -> None:
    """Ethereum block receipt proof inputs must reject duplicate tx hashes."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_evm_receipt_proof_evidence.py": (
            "seen_transaction_hashes: set[bytes] = set()",
            'method=f"block receipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_receipt_trie_builder_rejects_duplicate_transaction_hashes",
            'receipts[1]["transactionHash"] = receipts[0]["transactionHash"]',
            "duplicate block receipt transaction hashes were accepted",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const seenTransactionHashes = new Set();",
            "`blockReceipts[${index}].transactionHash`",
            "block receipt transactionHash values must be unique",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "fullReceipt(1, { transactionHash: TX_HASH })",
            "transactionHash values must be unique",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "var seenTransactionHashes = Set<Data>()",
            'field: "blockReceipts[\\(index)].transactionHash"',
            "blockReceipts.transactionHash",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "duplicateHashReceipt",
            '.invalidRlp("blockReceipts.transactionHash")',
        ),
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
        / "SourceSccpProofHashes.kt": (
            "val seenTransactionHashes = HashSet<String>(receipts.size)",
            '"blockReceipts[$index].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "duplicateHashReceipt",
            "transactionHash values must be unique",
        ),
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
        / "SourceSccpProofs.java": (
            "final Set<String> seenTransactionHashes = new HashSet<String>();",
            '"blockReceipts[" + index + "].transactionHash"',
            "block receipt transactionHash values must be unique",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "duplicateHashReceipt",
            "receipt proof builder must reject duplicate block receipt transaction hashes",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "var seenTransactionHashes = new HashSet<string>(StringComparer.Ordinal);",
            '$"blockReceipts[{index}].transactionHash"',
            "block receipt transactionHash values must be unique.",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "duplicateTransactionHashReceipt",
            "transactionHash values must be unique",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_noncanonical_chain_id_tests() -> None:
    """Ethereum mainnet collectors must reject noncanonical eth_chainId values."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            'for (const chainId of ["1", 1, "0x01", "0X1", " 0x1", "0x1 "])',
            "canonical JSON-RPC quantity",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            'chainId: "0x01"',
            '.invalidPublicInputs("eth_chainId")',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            'EthereumMainnetExecutionProvider { _, _ -> "0x01" }',
            "EthereumMainnetInboundEvidence(receipt = receipt)",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            '(method, params) -> "0x01"',
            "leading-zero eth_chainId RPC",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            'new ExecutionProviderStub("0x01", receipt, block)',
            "ValidateExecutionProviderMainnetAsync",
        ),
        ROOT / "pytests" / "scripts" / "sccp_evm_receipt_proof_evidence_test.py": (
            "test_collect_receipt_proof_rejects_noncanonical_chain_id_quantity",
            'rpc_response("0x01")',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_beacon_rest_header_shape_tests() -> None:
    """Beacon REST providers must require finalized-header roots and signature."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "`${label}.data.header.message.${field}`",
            "`${label}.data.header.signature`",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            'for (const field of ["parent_root", "state_root", "body_root"])',
            "/body_root must be 32 bytes/u",
            "/signature must be 96 bytes/u",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
            'for field in ["parent_root", "state_root", "body_root"]',
            '"\\(label).data.header.message.\\(field)"',
            '"\\(label).data.header.signature"',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            '("parent_root", String(repeating: "01", count: 32))',
            'invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.data.header.signature")',
            'String(repeating: "12", count: 95)',
        ),
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
        / "EvmSccpProver.kt": (
            'for (field in listOf("parent_root", "state_root", "body_root"))',
            '"$label.data.header.message.$field"',
            '"$label.data.header.signature"',
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            '"parent_root" to "01"',
            '"body_root" to "03"',
            '"12".repeat(95)',
        ),
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
        / "EthereumMainnetSccp.java": (
            'Arrays.asList("parent_root", "state_root", "body_root")',
            'label + ".data.header.message." + field',
            'label + ".data.header.signature"',
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            '{"parent_root", "01"}',
            'repeat("12", 95)',
            "Beacon REST provider must reject malformed finalized header signatures",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            'foreach (var field in new[] { "parent_root", "state_root", "body_root" })',
            '"{label}.data.header.message.{field}"',
            '"{label}.data.header.signature"',
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            '("parent_root", "01")',
            'string.Concat(Enumerable.Repeat("12", 95))',
            'Assert.Contains("signature", malformedSignature.Message)',
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_beacon_rest_execution_payload_tests() -> None:
    """Beacon REST providers must bind finalized execution payload fields."""

    guarded_sources = {
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "ethereumMainnetBeaconRestBlockIdForTarget",
            "/eth/v1/beacon/headers/${encodeURIComponent(targetBlockId.id)}",
            "/eth/v1/beacon/blocks/${encodeURIComponent(targetBlockId.id)}/root",
            "/eth/v2/beacon/blocks/${encodeURIComponent(targetBlockId.id)}",
            "execution_payload",
            "const executionBlockHash = requireEthereumRpcHexData(",
            "const executionReceiptsRoot = requireEthereumRpcHexData(",
            "const finalizedBlockRoot = requireEthereumRpcHexData(",
            "const finalizedCheckpointRoot = requireEthereumRpcHexData(",
            "const syncCommitteeRoot = requireEthereumRpcHexData(",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumMainnetBeaconRestFinalityUpdateSummary",
            "normalizeEthereumMainnetFinalityBranch",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "must contain 6 siblings",
            "must contain at least one participant",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "ethereumMainnetBeaconRestBlockIdForTarget",
            "/eth/v1/beacon/headers/${encodeURIComponent(targetBlockId.id)}",
            "/eth/v1/beacon/blocks/${encodeURIComponent(targetBlockId.id)}/root",
            "/eth/v2/beacon/blocks/${encodeURIComponent(targetBlockId.id)}",
            "execution_payload",
            "const executionBlockHash = requireEthereumRpcHexData(",
            "const executionReceiptsRoot = requireEthereumRpcHexData(",
            "const finalizedBlockRoot = requireEthereumRpcHexData(",
            "const finalizedCheckpointRoot = requireEthereumRpcHexData(",
            "const syncCommitteeRoot = requireEthereumRpcHexData(",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumMainnetBeaconRestFinalityUpdateSummary",
            "normalizeEthereumMainnetFinalityBranch",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "must contain 6 siblings",
            "must contain at least one participant",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpEthereumMainnet.test.js": (
            "/eth/v1/beacon/genesis",
            "/eth/v1/beacon/headers/64",
            "/eth/v1/beacon/blocks/64/root",
            "/eth/v2/beacon/blocks/64",
            "/eth/v1/beacon/light_client/finality_update",
            "validFinalityUpdate",
            "SAMPLE_FINALITY_BRANCH",
            "assert.deepEqual(evidence.beaconFinality.finalityBranch, SAMPLE_FINALITY_BRANCH)",
            "syncCommitteeParticipation",
            "/sync_committee_bits must contain at least one participant/u",
            "/finality_branch is required/u",
            "/finality_branch must contain 6 siblings/u",
            "/finalizedHeaderRoot must not be zero/u",
            "/finalizedBlockRoot must not be zero/u",
            "/finalizedCheckpointRoot must not be zero/u",
            "/syncCommitteeRoot must not be zero/u",
            "/requires beaconSlot, beaconBlockRoot, or block\\.timestamp/u",
            "/finalized target header must be finalized/u",
            "/historical target blocks require an ancestry proof/u",
            "/beaconFinality\\.executionBlockHash must not be zero/u",
            "/beaconFinality\\.executionReceiptsRoot must not be zero/u",
            "/beaconFinality\\.finalizedHeaderRoot must not be zero/u",
            "/beaconFinality\\.syncCommitteeRoot must not be zero/u",
            "/finalized block root must match finalized header root/u",
            "/execution payload block_hash must match block.hash/u",
            "/execution payload block_number must match block.number/u",
            "/execution payload receipts_root must match block.receiptsRoot/u",
        ),
        ROOT / "javascript" / "iroha_js" / "index.d.ts": (
            "syncCommitteeBits?: string;",
            "syncCommitteeSignature?: string;",
            "syncSignatureSlot?: string | number | bigint;",
            "signatureSlot?: string | number | bigint;",
            "finalityBranch?: readonly string[];",
            "finality_branch?: readonly string[];",
            "syncCommitteeParticipation?: string | number | bigint;",
            "readonly finalityBranch?: readonly string[];",
            "readonly syncCommitteeBits?: string;",
            "readonly syncCommitteeSignature?: string;",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "package_dist.test.js": (
            "syncCommitteeBits\\?: string;",
            "syncCommitteeSignature\\?: string;",
            "syncSignatureSlot\\?: string \\| number \\| bigint;",
            "finalityBranch\\?: readonly string\\[\\];",
            "syncCommitteeParticipation\\?: string \\| number \\| bigint;",
            "readonly finalityBranch\\?: readonly string\\[\\];",
            "readonly syncCommitteeBits\\?: string;",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpEvmProver.swift": (
            "beaconRestBlockIdForTarget",
            'path: "/eth/v1/beacon/headers/\\(targetBlockId.id)"',
            'path: "/eth/v1/beacon/blocks/\\(targetBlockId.id)/root"',
            'path: "/eth/v2/beacon/blocks/\\(targetBlockId.id)"',
            'path: "/eth/v1/beacon/light_client/finality_update"',
            "BeaconRestFinalityUpdateSummary",
            "Ethereum mainnet Beacon REST light-client finality update.data.finality_branch",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits",
            "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            "normalizeFinalityBranch(",
            '"finalityBranch": finalityUpdate.finalityBranch',
            "syncCommitteeParticipation",
            "public let syncCommitteeBits: String?",
            'value["syncCommitteeBits"] = syncCommitteeBits',
            "strictFirstPresent(",
            "normalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            'invalidPublicInputs("beaconRest.targetHeader.slot")',
            'invalidPublicInputs("beaconRest.targetHeader.finalizedSlot")',
            'invalidPublicInputs("beaconRest.targetHeader.ancestryProof")',
            'invalidPublicInputs("beaconRest.finalizedBlockRoot")',
            'invalidPublicInputs("beaconRest.executionPayload.blockHash")',
            'invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "testEthereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "testEthereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "/eth/v1/beacon/genesis",
            "/eth/v1/beacon/headers/32",
            "/eth/v1/beacon/blocks/64/root",
            "/eth/v2/beacon/blocks/64",
            "/eth/v1/beacon/light_client/finality_update",
            "ethereumBeaconFinalityUpdateJson(",
            "ethereumFinalityBranch",
            'XCTAssertEqual(finality["finalityBranch"] as? [String], Self.ethereumFinalityBranch)',
            "includeFinalityBranch: false",
            "finalityBranch: Array(Self.ethereumFinalityBranch.prefix(5))",
            "syncCommitteeParticipation",
            'syncCommitteeBits: "0x01" + String(repeating: "00", count: 63)',
            'conflictingSyncBitsFinality["sync_committee_bits"]',
            '"finalized_header_root", "0x" + String(repeating: "13", count: 32)',
            '.zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")',
            '"timestamp": "0x364"',
            "ethereumBeaconBlockRootJson(",
            "ethereumBeaconBlockJson(",
            'invalidPublicInputs("beaconRest.targetHeader.ancestryProof")',
            'invalidPublicInputs("beaconRest.finalizedBlockRoot")',
            'invalidPublicInputs("beaconRest.executionPayload.blockHash")',
            'invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")',
        ),
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
        / "EvmSccpProver.kt": (
            "beaconRestBlockIdForTarget",
            '"/eth/v1/beacon/headers/${targetBlockId.id}"',
            '"/eth/v1/beacon/blocks/${targetBlockId.id}/root"',
            '"/eth/v2/beacon/blocks/${targetBlockId.id}"',
            '"/eth/v1/beacon/light_client/finality_update"',
            "ethereumBeaconRestFinalityUpdateSummary",
            "normalizeEthereumBeaconRestFinalityBranch",
            '"finalityBranch" to finalityUpdate.finalityBranch',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "ethereumBeaconRestSyncCommitteeParticipation",
            "val syncCommitteeBits: String? = null",
            'syncCommitteeBits?.let { "syncCommitteeBits" to it }',
            "strictFirstPresent(",
            "execution_payload",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "EvmSccpProverTest.kt": (
            "ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "beaconFinalityUpdateJson(",
            "ethereumFinalityBranch",
            'assertEquals(ethereumFinalityBranch, evidence.beaconFinality?.get("finalityBranch"))',
            "includeFinalityBranch = false",
            "finalityBranch = ethereumFinalityBranch.take(5)",
            "syncCommitteeParticipation",
            "ethereumSyncCommitteeSupermajorityBits",
            '"sync_committee_bits" to ("0x02" + "00".repeat(63))',
            'Triple("finalized_header_root", "0x" + "13".repeat(32), "beaconFinality.finalizedHeaderRoot")',
            "sync_committee_bits must contain at least one participant",
            '"timestamp" to "0x364"',
            "beaconBlockRootJson(",
            "beaconBlockJson(",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
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
        / "EthereumMainnetSccp.java": (
            "beaconRestBlockIdForTarget",
            '"/eth/v1/beacon/headers/" + targetBlockId.id',
            '"/eth/v1/beacon/blocks/" + targetBlockId.id + "/root"',
            '"/eth/v2/beacon/blocks/" + targetBlockId.id',
            '"/eth/v1/beacon/light_client/finality_update"',
            "beaconRestFinalityUpdateSummary",
            "normalizeBeaconRestFinalityBranch",
            'evidence.put("finalityBranch", finalityUpdate.finalityBranch)',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "beaconRestSyncCommitteeParticipation",
            "String syncCommitteeBits,",
            'value.put("syncCommitteeBits", syncCommitteeBits)',
            "strictFirstPresent(",
            "normalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "EvmSccpProverTests.java": (
            "ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "beaconFinalityUpdateJson(",
            "ETHEREUM_FINALITY_BRANCH",
            "Beacon REST provider must reject missing finality branch",
            "Beacon REST provider must reject malformed finality branch",
            "syncCommitteeParticipation",
            "ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS",
            'conflictingSyncBitsFinality.put("sync_committee_bits"',
            "final Object[][] conflictingFinalityAliases",
            "sync_committee_bits must contain at least one participant",
            '"timestamp", "0x364"',
            "beaconBlockRootJson(",
            "beaconBlockJson(",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "BeaconRestBlockIdForTargetAsync",
            '$"/eth/v1/beacon/headers/{targetBlockId.Id}"',
            '$"/eth/v1/beacon/blocks/{targetBlockId.Id}/root"',
            '$"/eth/v2/beacon/blocks/{targetBlockId.Id}"',
            '"/eth/v1/beacon/light_client/finality_update"',
            "BeaconRestFinalityUpdateSummary",
            "NormalizeFinalityBranch(",
            '["finalityBranch"] = finalityUpdate.FinalityBranch',
            "sync_aggregate",
            "finality_branch",
            "sync_committee_bits",
            "sync_committee_signature",
            "SyncCommitteeParticipation",
            "string? SyncCommitteeBits = null",
            'value["syncCommitteeBits"] = SyncCommitteeBits',
            "StrictFirstPresent(",
            "NormalizeFinalitySyncCommitteeBits(",
            "execution_payload",
            "EthExecutionPayloadHeaderRootFromRlp",
            "EthBeaconBodyRootFromExecutionPayloadBranch",
            "EthBeaconBlockHeaderRoot",
            "SszMerkleRootFromBranch(",
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot",
            "Ethereum mainnet Beacon REST target block is newer than the finalized header",
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof",
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root",
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash",
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "BeaconRestConsensusProviderCollectsFinalizedTargetEvidence",
            "BeaconRestConsensusProviderDerivesTargetSlotFromTimestamp",
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/32",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            "BeaconFinalityUpdateJson(",
            "EthereumFinalityBranch",
            'Assert.Equal(EthereumFinalityBranch, Assert.IsAssignableFrom<IReadOnlyList<string>>(evidence.BeaconFinality?["finalityBranch"]))',
            "includeFinalityBranch: false",
            "finalityBranch: EthereumFinalityBranch.Take(5).ToArray()",
            "syncCommitteeParticipation",
            "EthereumSyncCommitteeSupermajorityBits",
            '["sync_committee_bits"] = "0x02" + string.Concat(Enumerable.Repeat("00", 63))',
            "sync_committee_bits must contain at least one participant",
            '["timestamp"] = "0x364"',
            "BeaconBlockRootJson(",
            "BeaconBlockJson(",
            "BeaconExecutionPayloadSszRootsMatchSharedVector",
            "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
            "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
            "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
            "historical target blocks require an ancestry proof",
            "finalized block root must match finalized header root",
            "execution payload block_hash must match block.hash",
            "execution payload receipts_root must match block.receiptsRoot",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_sync_committee_roster_tests() -> None:
    """Ethereum mainnet sync-committee helpers must reject compressed rosters."""

    guarded_sources = {
        ROOT / "crates" / "iroha_sccp" / "src" / "lib.rs": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: usize = 512;",
            ".all(|weight| *weight == 1)",
            "eth_sync_committee_transition_transcript_requires_mainnet_rosters",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512;",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[${index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "syncCommitteeFixture(0x11, 0xaa)",
            "assert.equal(nextSyncCommitteePayload.length, 81925)",
            "signersBitmap(342)",
        ),
        ROOT / "python" / "iroha_torii_client" / "sccp.py": (
            "_SCCP_ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "ETH sync committee must contain exactly",
            "syncCommitteeWeights[{index}] must be 1 for Ethereum mainnet",
        ),
        ROOT / "python" / "iroha_torii_client" / "tests" / "sccp_test.py": (
            "sync_committee_fixture(0x11, 0xAA)",
            "assert len(next_payload) == 81925",
            "signers_bitmap(342)",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "sccpEthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteeWeights[index] == 1",
            "signersBitmap.count == (syncCommitteePublicKeys.count + 7) / 8",
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "ethereumSyncCommitteeBytes(_ byte: UInt8, count: Int)",
            "XCTAssertEqual(nextSyncPayload.count, 81_925)",
            "Self.ethereumSyncCommitteeSignersBitmap(342)",
        ),
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
        / "SourceSccpProofHashes.kt": (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES: Int = 512",
            "syncCommitteeWeights[$index] must be 1 for Ethereum mainnet",
            "signersBitmap.size == (syncCommitteePublicKeys.size + 7) / 8",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "List(512) { index ->",
            "assertEquals(81925, nextSyncPayload.size)",
            "syncCommitteeSignersBitmap(342)",
        ),
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
        / "SourceSccpProofs.java": (
            "ETH_MAINNET_SYNC_COMMITTEE_AUTHORITIES = 512",
            "must be 1 for Ethereum mainnet",
            "(syncCommitteePublicKeys.size() + 7) / 8",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "syncCommitteeBytes(0x11, 48)",
            "nextSyncPayload.length == 81925",
            "syncCommitteeSignersBitmap(342)",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "EthMainnetSyncCommitteeAuthorities = 512",
            "syncCommitteePayload must contain exactly",
            "must be 1 for Ethereum mainnet",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "Assert.Equal(81925, syncCommitteePayload.Length)",
            "CompressedSyncCommitteePayload()",
            "WeightedSyncCommitteePayload()",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


def test_release_readiness_guards_ethereum_source_bridge_config_tests() -> None:
    """Ethereum source bridge material must bind mainnet config hashes."""

    guarded_sources = {
        ROOT / "scripts" / "sccp_eth_source_bridge_evidence.py": (
            "def eth_source_bridge_config_hash(",
            "source_bridge_network_id must be Ethereum mainnet chain id 1",
            "ETH_SOURCE_BRIDGE_CONFIG_PREFIX",
        ),
        ROOT / "scripts" / "sccp_all_lanes_evidence.py": (
            "def _check_eth_source_bridge_config_hash(",
            "source_bridge_config_hash does not match ETH bridge address",
        ),
        ROOT / "pytests" / "scripts" / "sccp_eth_source_bridge_evidence_test.py": (
            "test_eth_source_bridge_config_hash_binds_mainnet_lane_and_code_hash",
            "invalid ETH source bridge config hash input was accepted",
        ),
        ROOT / "javascript" / "iroha_js" / "src" / "sccp.js": (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT / "javascript" / "iroha_js" / "dist" / "sccp.js": (
            "const rejectMismatchedEthSourceBridgeConfigHash = (material) =>",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT / "javascript" / "iroha_js" / "test" / "sccpSolanaProver.test.js": (
            "sourceBridgeNetworkId must be Ethereum mainnet chain id",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "IrohaSwift"
        / "Sources"
        / "IrohaSwift"
        / "SccpSourceProofHashes.swift": (
            "ethSourceBridgeConfigHash(",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
        ROOT
        / "IrohaSwift"
        / "Tests"
        / "IrohaSwiftTests"
        / "SccpSolanaProverTests.swift": (
            "sourceBridgeNetworkId",
            '.invalidSourceMaterial("sourceBridgeConfigHash")',
        ),
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
        / "SourceSccpProofHashes.kt": (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "kotlin"
        / "core-jvm"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "hyperledger"
        / "iroha"
        / "sdk"
        / "sccp"
        / "SourceSccpProofHashesTest.kt": (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
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
        / "SourceSccpProofs.java": (
            "ethSourceBridgeConfigHash(",
            "sourceBridgeConfigHash must match ETH source bridge config fields",
        ),
        ROOT
        / "java"
        / "iroha_android"
        / "src"
        / "test"
        / "java"
        / "org"
        / "hyperledger"
        / "iroha"
        / "android"
        / "sccp"
        / "SourceSccpProofsTests.java": (
            "sourceBridgeNetworkId",
            "sourceBridgeConfigHash",
        ),
        ROOT
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Sccp"
        / "EthereumMainnetSccp.cs": (
            "SourceBridgeConfigHash must match the Ethereum mainnet source bridge config fields.",
            "NormalizeEthereumMainnetNetworkId(input.NetworkId)",
        ),
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SccpEthereumMainnetTests.cs": (
            "ExpectedSourceBridgeConfigHash",
            "SourceBridgeConfigHash = \"0x\" + new string('9', 64)",
        ),
    }
    missing = []
    for path, markers in guarded_sources.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in source:
                missing.append(f"{path.relative_to(ROOT)} missing `{marker}`")

    assert missing == []


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
