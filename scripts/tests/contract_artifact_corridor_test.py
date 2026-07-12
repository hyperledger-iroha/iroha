"""Adversarial tests for the authenticated SCCP contract artifact corridor."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import contract_artifact_corridor as corridor  # noqa: E402


EVM_COMPILER = b"deterministic fake EVM compiler"
TRON_COMPILER = b"deterministic fake TRON compiler"


def corridor_config() -> corridor.CorridorConfig:
    """Return a minimal two-compiler configuration for isolated tests."""

    compilers = {
        "evm": corridor.CompilerSpec(
            target="evm",
            identity="test-evm-solc",
            reported_version="0.7.4+commit.3f05b770.Emscripten.clang",
            url="https://example.invalid/evm-soljson.js",
            sha256=hashlib.sha256(EVM_COMPILER).hexdigest(),
        ),
        "tron": corridor.CompilerSpec(
            target="tron",
            identity="test-tron-solc",
            reported_version="0.7.4+commit.3f05b770.Emscripten.clang",
            url="https://example.invalid/tron-soljson.js",
            sha256=hashlib.sha256(TRON_COMPILER).hexdigest(),
        ),
    }
    settings = {
        "optimizer": {"enabled": True, "runs": 200},
        "evmVersion": "istanbul",
        "metadata": {
            "bytecodeHash": "ipfs",
            "useLiteralContent": True,
        },
        "outputSelection": {
            "*": {
                "*": [
                    "abi",
                    "metadata",
                    "evm.bytecode.object",
                    "evm.bytecode.linkReferences",
                    "evm.deployedBytecode.object",
                    "evm.deployedBytecode.immutableReferences",
                    "evm.deployedBytecode.linkReferences",
                ]
            }
        },
    }
    return corridor.CorridorConfig(
        compilers=compilers,
        settings=settings,
        sources={
            "evm": ("contracts/evm/Test.sol",),
            "tron": ("contracts/tron/Test.sol",),
        },
        size_limits={
            "evm": {"creation_bytecode_bytes": 128, "runtime_bytecode_bytes": 128},
            "tron": {"creation_bytecode_bytes": 128, "runtime_bytecode_bytes": 128},
        },
        tvm_runner={
            "image": "tronbox/tre@sha256:" + "11" * 32,
            "platform": "linux/amd64",
        },
        canonical_sha256="22" * 32,
    )


def write_test_sources(root: Path) -> None:
    """Create identical source bytes beneath an arbitrary checkout root."""

    for target in ("evm", "tron"):
        path = root / "contracts" / target / "Test.sol"
        path.parent.mkdir(parents=True)
        path.write_text(
            "// SPDX-License-Identifier: Apache-2.0\n"
            "pragma solidity 0.7.4;\n"
            "contract Test { function value() external pure returns(uint256) { return 7; } }\n",
            encoding="utf-8",
        )


def compiler_fetcher(url: str) -> bytes:
    """Return target-distinct fake compiler bytes."""

    if url.endswith("evm-soljson.js"):
        return EVM_COMPILER
    if url.endswith("tron-soljson.js"):
        return TRON_COMPILER
    raise AssertionError(url)


def fake_compiler_output(
    compiler_path: Path,
    spec: corridor.CompilerSpec,
    compiler_input: bytes,
    node_binary: str,
) -> dict[str, object]:
    """Return a deterministic standard-json result bound to the supplied target."""

    del compiler_path, node_binary
    parsed_input = json.loads(compiler_input)
    source_path = next(iter(parsed_input["sources"]))
    target_byte = "01" if spec.target == "evm" else "02"
    metadata = {
        "compiler": {"version": spec.reported_version.removesuffix(".Emscripten.clang")},
        "language": "Solidity",
        "output": {"abi": [{"type": "function", "name": "value", "inputs": []}]},
        "settings": parsed_input["settings"],
        "sources": parsed_input["sources"],
    }
    return {
        "errors": [],
        "contracts": {
            source_path: {
                "Test": {
                    "abi": [{"type": "function", "name": "value", "inputs": []}],
                    "metadata": json.dumps(metadata, sort_keys=True, separators=(",", ":")),
                    "evm": {
                        "bytecode": {
                            "object": "6000" + target_byte,
                            "linkReferences": {},
                        },
                        "deployedBytecode": {
                            "object": "6001" + target_byte,
                            "linkReferences": {},
                        },
                    },
                }
            }
        },
    }


def build_fake_manifest(root: Path) -> dict[str, object]:
    return dict(
        corridor.compile_corridor(
            root,
            corridor_config(),
            node_binary="unused-node",
            fetcher=compiler_fetcher,
            runner=fake_compiler_output,
        )
    )


def test_keccak_is_ethereum_keccak_not_nist_sha3() -> None:
    assert corridor.keccak256_hex(b"") == (
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    )
    assert corridor.keccak256_hex(b"abc") == (
        "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    )
    assert corridor.keccak256_hex(b"abc") != hashlib.sha3_256(b"abc").hexdigest()


def test_builds_in_separate_roots_are_byte_identical_and_complete(tmp_path: Path) -> None:
    first_root = tmp_path / "first-checkout"
    second_root = tmp_path / "different" / "second-checkout"
    write_test_sources(first_root)
    write_test_sources(second_root)
    first = build_fake_manifest(first_root)
    second = build_fake_manifest(second_root)
    assert corridor.canonical_json_bytes(first) == corridor.canonical_json_bytes(second)

    lock = corridor.artifact_lock_from_manifest(first)
    corridor.validate_manifest_integrity(first, corridor_config())
    corridor.validate_artifact_lock(second, lock)
    first_path = corridor.publish_manifest(tmp_path / "first-output", first)
    second_path = corridor.publish_manifest(tmp_path / "second-output", second)
    assert first_path.read_bytes() == second_path.read_bytes()

    for target in corridor.TARGETS:
        target_manifest = first["targets"][target]
        assert target_manifest["compiler"]["soljson_sha256_hex"]
        assert target_manifest["standard_json_input_sha256_hex"]
        artifact = target_manifest["contracts"][0]
        assert artifact["abi"]
        assert artifact["metadata"]
        for bytecode_name in ("creation_bytecode", "runtime_bytecode"):
            bytecode = artifact[bytecode_name]
            assert bytecode["hex"].startswith("0x")
            assert len(bytecode["sha256_hex"]) == 64
            assert len(bytecode["keccak256_hex"]) == 64


def test_tampered_compiler_bytes_and_digest_drift_fail_before_execution(
    tmp_path: Path,
) -> None:
    config = corridor_config()
    destination = tmp_path / "compiler.js"
    with pytest.raises(corridor.CorridorError, match="digest mismatch"):
        corridor.materialize_verified_compiler(
            config.compilers["evm"],
            destination,
            lambda _url: EVM_COMPILER + b"tampered",
        )
    assert not destination.exists()

    drifted = corridor.CompilerSpec(
        target="evm",
        identity=config.compilers["evm"].identity,
        reported_version=config.compilers["evm"].reported_version,
        url=config.compilers["evm"].url,
        sha256="00" * 32,
    )
    with pytest.raises(corridor.CorridorError, match="digest mismatch"):
        corridor.materialize_verified_compiler(
            drifted,
            destination,
            lambda _url: EVM_COMPILER,
        )


@pytest.mark.parametrize(
    "directive",
    (
        "pragma solidity 0.8.24;",
        "pragma solidity 0.7.6;",
        "pragma solidity ^0.7.4;",
        "pragma  solidity >=0.7.4 <0.9.0;",
        "pragma solidity /* hidden range */ >=0.7.4 <0.9.0;",
        "pragma solidity >=0.7.4 <0.9.0/* hidden terminator */;",
        "pragma solidity 0.7.4;\npragma solidity 0.7.4;",
        "pragma solidity 0.7.4;\npragma experimental ABIEncoderV1;",
        "pragma solidity 0.7.4;\npragma experimental ABIEncoderV2;",
        "pragma solidity 0.7.4;\npragma experimental ABIEncoderV2;\npragma experimental ABIEncoderV2;",
        "pragma solidity\n>=0.7.4 <0.9.0;",
        "pragma solidity >=0.7.4 <0.9.0",
    ),
)
def test_source_policy_rejects_noncanonical_duplicate_and_obfuscated_pragmas(
    tmp_path: Path, directive: str
) -> None:
    write_test_sources(tmp_path)
    source = tmp_path / "contracts" / "evm" / "Test.sol"
    source.write_text(
        "// SPDX-License-Identifier: Apache-2.0\n"
        f"{directive}\n"
        "contract Test {}\n",
        encoding="utf-8",
    )
    with pytest.raises(corridor.CorridorError, match="pragma|0.7.4"):
        corridor.standard_json_input(tmp_path, corridor_config(), "evm")


def test_source_policy_accepts_only_exact_first_release_pragma(tmp_path: Path) -> None:
    write_test_sources(tmp_path)
    source = tmp_path / "contracts" / "evm" / "Test.sol"
    source.write_text(
        "// SPDX-License-Identifier: Apache-2.0\n"
        "pragma solidity 0.7.4;\n"
        "contract Test {}\n",
        encoding="utf-8",
    )
    corridor.standard_json_input(tmp_path, corridor_config(), "evm")


def test_source_policy_ignores_fake_pragmas_in_comments_and_strings(tmp_path: Path) -> None:
    write_test_sources(tmp_path)
    source = tmp_path / "contracts" / "evm" / "Test.sol"
    source.write_text(
        "// pragma solidity 0.7.4;\n"
        "/* pragma experimental ABIEncoderV2; */\n"
        "pragma solidity 0.7.4;\n"
        'contract Test { string constant TEXT = "pragma solidity ^0.7.0;"; }\n',
        encoding="utf-8",
    )
    standard_input, _inventory = corridor.standard_json_input(
        tmp_path, corridor_config(), "evm"
    )
    assert standard_input["sources"]["contracts/evm/Test.sol"]["content"].startswith(
        "// pragma solidity 0.7.4;"
    )


def test_source_policy_requires_abi_encoder_v2_only_for_typed_deployments() -> None:
    typed_source = (
        "// SPDX-License-Identifier: Apache-2.0\n"
        "pragma solidity 0.7.4;\n"
        "pragma experimental ABIEncoderV2;\n"
        "contract TypedBridge {}\n"
    )
    typed_path = "contracts/evm/sccp/TairaXorExactEvmSccpBridge.sol"
    corridor.validate_solidity_source_policy(typed_source, typed_path)

    with pytest.raises(corridor.CorridorError, match="pragma sequence"):
        corridor.validate_solidity_source_policy(
            typed_source.replace("pragma experimental ABIEncoderV2;\n", ""),
            typed_path,
        )
    with pytest.raises(corridor.CorridorError, match="pragma sequence"):
        corridor.validate_solidity_source_policy(
            typed_source.replace(
                "contract TypedBridge {}",
                "pragma experimental ABIEncoderV2;\ncontract TypedBridge {}",
            ),
            typed_path,
        )


@pytest.mark.parametrize(
    ("target", "field", "replacement"),
    (
        ("evm", "identity", "solc-evm-0.8.24+commit.e11b9ed9"),
        ("evm", "reported_version", "0.8.24+commit.e11b9ed9.Emscripten.clang"),
        ("evm", "sha256", "00" * 32),
        ("evm", "url", "https://binaries.soliditylang.org/wasm/soljson-v0.8.24.js"),
        ("tron", "identity", "tron-solc-tvm-0.8.24+commit.7d902c66"),
        ("tron", "reported_version", "0.7.6+commit.7338295f.Emscripten.clang"),
        ("tron", "sha256", "11" * 32),
        ("tron", "url", "https://example.invalid/soljson.js"),
    ),
)
def test_compiler_lock_rejects_every_identity_or_digest_downgrade(
    tmp_path: Path, target: str, field: str, replacement: str
) -> None:
    lock = json.loads(corridor.DEFAULT_COMPILER_LOCK.read_text(encoding="utf-8"))
    lock["compilers"][target][field] = replacement
    lock_path = tmp_path / "compiler-lock.json"
    lock_path.write_text(json.dumps(lock), encoding="utf-8")
    with pytest.raises(corridor.CorridorError, match="exact Solidity 0.7.4"):
        corridor.load_corridor_config(lock_path)


def test_compiler_warning_is_a_hard_failure(tmp_path: Path) -> None:
    write_test_sources(tmp_path)

    def warning_runner(*args, **kwargs):
        output = fake_compiler_output(*args, **kwargs)
        output["errors"] = [
            {
                "severity": "warning",
                "formattedMessage": "Warning: target-dependent output changed",
            }
        ]
        return output

    compiler_path = tmp_path / "compiler.js"
    compiler_path.write_bytes(EVM_COMPILER)
    with pytest.raises(corridor.CorridorError, match="warning or error"):
        corridor.compile_target(
            tmp_path,
            corridor_config(),
            "evm",
            compiler_path,
            "unused-node",
            warning_runner,
        )


@pytest.mark.parametrize("case", ("placeholder", "link-reference"))
def test_unlinked_bytecode_is_rejected(tmp_path: Path, case: str) -> None:
    write_test_sources(tmp_path)

    def unlinked_runner(*args, **kwargs):
        output = fake_compiler_output(*args, **kwargs)
        artifact = next(iter(next(iter(output["contracts"].values())).values()))
        if case == "placeholder":
            artifact["evm"]["bytecode"]["object"] = "__$1234567890$__"
        else:
            artifact["evm"]["bytecode"]["linkReferences"] = {
                "Library.sol": {"Library": [{"start": 1, "length": 20}]}
            }
        return output

    compiler_path = tmp_path / "compiler.js"
    compiler_path.write_bytes(EVM_COMPILER)
    with pytest.raises(corridor.CorridorError, match="unresolved|noncanonical"):
        corridor.compile_target(
            tmp_path,
            corridor_config(),
            "evm",
            compiler_path,
            "unused-node",
            unlinked_runner,
        )


def test_aliased_evm_and_tvm_output_maps_are_rejected() -> None:
    shared_contracts: dict[str, object] = {"Source.sol": {"Test": {}}}
    shared_output: dict[str, object] = {"contracts": shared_contracts}
    evm = corridor.CompiledTarget(
        target="evm",
        source_paths=("evm.sol",),
        input_sha256="11" * 32,
        compiler_sha256="22" * 32,
        raw_output=shared_output,
        manifest={"target": "evm"},
    )
    tron = corridor.CompiledTarget(
        target="tron",
        source_paths=("tron.sol",),
        input_sha256="33" * 32,
        compiler_sha256="44" * 32,
        raw_output={"contracts": shared_contracts},
        manifest={"target": "tron"},
    )
    with pytest.raises(corridor.CorridorError, match="aliased"):
        corridor.validate_distinct_targets(evm, tron)


def test_size_and_full_artifact_digest_drift_are_rejected(tmp_path: Path) -> None:
    write_test_sources(tmp_path)
    manifest = build_fake_manifest(tmp_path)
    size_lock = corridor.artifact_lock_from_manifest(manifest)
    size_lock["targets"]["evm"]["contract_sizes"][
        "contracts/evm/Test.sol:Test"
    ]["runtime_bytecode_bytes"] += 1
    with pytest.raises(corridor.CorridorError, match="size drift"):
        corridor.validate_artifact_lock(manifest, size_lock)

    digest_lock = corridor.artifact_lock_from_manifest(manifest)
    digest_lock["corridor_manifest_sha256_hex"] = "ff" * 32
    with pytest.raises(corridor.CorridorError, match="artifact digest drift"):
        corridor.validate_artifact_lock(manifest, digest_lock)


def test_tampered_manifest_hashes_are_rejected(tmp_path: Path) -> None:
    write_test_sources(tmp_path)
    manifest = build_fake_manifest(tmp_path)
    tampered = copy.deepcopy(manifest)
    tampered["targets"]["tron"]["contracts"][0]["runtime_bytecode"][
        "sha256_hex"
    ] = "00" * 32
    with pytest.raises(corridor.CorridorError, match="SHA-256"):
        corridor.validate_manifest_integrity(tampered, corridor_config())

    unknown = copy.deepcopy(manifest)
    unknown["targets"]["evm"]["contracts"][0]["unreviewed"] = True
    with pytest.raises(corridor.CorridorError, match="missing or unknown"):
        corridor.validate_manifest_integrity(unknown, corridor_config())


@pytest.mark.parametrize(
    "references",
    (
        {"1": []},
        {"01": [{"start": 0, "length": 1}]},
        {"1": [{"start": -1, "length": 1}]},
        {"1": [{"start": 2, "length": 2}]},
        {"1": [{"start": 0, "length": 2}], "2": [{"start": 1, "length": 1}]},
        {"1": [{"start": 0, "length": 0}]},
    ),
)
def test_runtime_immutable_references_reject_malformed_or_overlapping_ranges(
    references: object,
) -> None:
    with pytest.raises(corridor.CorridorError, match="immutable"):
        corridor._runtime_immutable_references(references, 3, "test runtime")


def test_runtime_immutable_references_are_normalized_and_manifest_bound(
    tmp_path: Path,
) -> None:
    assert corridor._runtime_immutable_references(
        {"12": [{"start": 2, "length": 1}], "3": [{"start": 0, "length": 2}]},
        3,
        "test runtime",
    ) == [
        {"ast_id": "3", "start": 0, "length": 2},
        {"ast_id": "12", "start": 2, "length": 1},
    ]

    write_test_sources(tmp_path)
    manifest = build_fake_manifest(tmp_path)
    tampered = copy.deepcopy(manifest)
    tampered["targets"]["evm"]["contracts"][0]["runtime_immutable_references"] = [
        {"ast_id": "1", "start": 3, "length": 1}
    ]
    with pytest.raises(corridor.CorridorError, match="immutable"):
        corridor.validate_manifest_integrity(tampered, corridor_config())


def test_source_smoke_rejects_manifest_stale_for_current_checkout(tmp_path: Path) -> None:
    write_test_sources(tmp_path)
    manifest = build_fake_manifest(tmp_path)
    corridor.validate_manifest_source_inputs(manifest, corridor_config(), tmp_path)

    source = tmp_path / "contracts" / "tron" / "Test.sol"
    source.write_text(
        source.read_text(encoding="utf-8").replace("return 7", "return 8"),
        encoding="utf-8",
    )
    with pytest.raises(corridor.CorridorError, match="stale.*source input"):
        corridor.validate_manifest_source_inputs(manifest, corridor_config(), tmp_path)


def test_runtime_input_snapshot_is_private_read_only_and_exact(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    write_test_sources(tmp_path / "checkout")
    manifest_value = build_fake_manifest(tmp_path / "checkout")
    manifest = tmp_path / "manifest.json"
    manifest_bytes = corridor.canonical_json_bytes(manifest_value) + b"\n"
    manifest.write_bytes(manifest_bytes)
    vectors = tmp_path / "vectors.json"
    vector_bytes = b'{"version":1,"vectors":[]}\n'
    vectors.write_bytes(vector_bytes)

    snapshot_dir = tmp_path / "snapshot"
    manifest_copy, vector_copy = corridor.snapshot_runtime_inputs(
        manifest,
        vectors,
        snapshot_dir,
    )
    try:
        assert manifest_copy.read_bytes() == manifest_bytes
        assert vector_copy.read_bytes() == vector_bytes
        assert stat.S_IMODE(snapshot_dir.stat().st_mode) == 0o500
        assert stat.S_IMODE(manifest_copy.stat().st_mode) == 0o400
        assert stat.S_IMODE(vector_copy.stat().st_mode) == 0o400
    finally:
        snapshot_dir.chmod(0o700)


def test_runtime_input_snapshot_rejects_symlink_and_private_parent_drift(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    manifest = tmp_path / "manifest.json"
    manifest.write_text("{}\n", encoding="utf-8")
    vectors = tmp_path / "vectors.json"
    vectors.write_text("{}\n", encoding="utf-8")
    manifest_link = tmp_path / "manifest-link.json"
    manifest_link.symlink_to(manifest)
    with pytest.raises(corridor.CorridorError, match="direct regular file"):
        corridor.snapshot_runtime_inputs(manifest_link, vectors, tmp_path / "snapshot-link")

    tmp_path.chmod(0o755)
    with pytest.raises(corridor.CorridorError, match="owned private directory"):
        corridor.snapshot_runtime_inputs(manifest, vectors, tmp_path / "snapshot-public")


def test_runtime_input_snapshot_rejects_source_path_replacement_during_read(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    tmp_path.chmod(0o700)
    manifest = tmp_path / "manifest.json"
    original = b'{"generation":"original"}\n'
    replacement = b'{"generation":"replaced"}\n'
    manifest.write_bytes(original)
    vectors = tmp_path / "vectors.json"
    vectors.write_text("{}\n", encoding="utf-8")
    replacement_path = tmp_path / "replacement.json"
    replacement_path.write_bytes(replacement)

    real_read = corridor.os.read
    replaced = False

    def replacing_read(descriptor: int, count: int) -> bytes:
        nonlocal replaced
        payload = real_read(descriptor, count)
        if payload and not replaced:
            replaced = True
            os.replace(replacement_path, manifest)
        return payload

    monkeypatch.setattr(corridor.os, "read", replacing_read)
    with pytest.raises(corridor.CorridorError, match="changed while it was being read"):
        corridor.snapshot_runtime_inputs(manifest, vectors, tmp_path / "snapshot")
    assert replaced
    assert manifest.read_bytes() == replacement
    assert not (tmp_path / "snapshot").exists()


def test_runtime_input_snapshot_rejects_in_place_mutation_during_read(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    tmp_path.chmod(0o700)
    manifest = tmp_path / "manifest.json"
    manifest.write_bytes(b'{"generation":"original"}\n')
    vectors = tmp_path / "vectors.json"
    vectors.write_text("{}\n", encoding="utf-8")
    real_read = corridor.os.read
    mutated = False

    def mutating_read(descriptor: int, count: int) -> bytes:
        nonlocal mutated
        payload = real_read(descriptor, count)
        if payload and not mutated:
            mutated = True
            manifest.write_bytes(b'{"generation":"tampered"}\n')
        return payload

    monkeypatch.setattr(corridor.os, "read", mutating_read)
    with pytest.raises(corridor.CorridorError, match="changed while it was being read"):
        corridor.snapshot_runtime_inputs(manifest, vectors, tmp_path / "snapshot")
    assert mutated
    assert not (tmp_path / "snapshot").exists()


def test_source_and_output_path_collisions_fail_closed(tmp_path: Path) -> None:
    lock = json.loads(corridor.DEFAULT_COMPILER_LOCK.read_text(encoding="utf-8"))
    lock["sources"]["evm"].extend(
        [
            "contracts/COLLISION/Test.sol",
            "contracts/collision/test.sol",
        ]
    )
    lock["sources"]["evm"].sort()
    lock_path = tmp_path / "compiler-lock.json"
    lock_path.write_text(json.dumps(lock), encoding="utf-8")
    with pytest.raises(corridor.CorridorError, match="collision"):
        corridor.load_corridor_config(lock_path)

    write_test_sources(tmp_path / "checkout")
    manifest = build_fake_manifest(tmp_path / "checkout")
    output = tmp_path / "output"
    output.mkdir()
    (output / "stale.json").write_text("{}", encoding="utf-8")
    with pytest.raises(corridor.CorridorError, match="empty"):
        corridor.publish_manifest(output, manifest)


def test_requested_tvm_execution_fails_when_docker_is_unavailable(tmp_path: Path) -> None:
    manifest = tmp_path / "manifest.json"
    manifest.write_text("{}\n", encoding="utf-8")
    environment = os.environ.copy()
    environment["SCCP_TVM_DOCKER_BIN"] = "definitely-not-an-installed-docker-cli"
    result = subprocess.run(
        [
            "bash",
            str(ROOT / "scripts" / "contract_tvm_runner.sh"),
            "--manifest",
            str(manifest),
        ],
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert result.returncode != 0
    assert result.stdout == ""
    assert "Docker CLI is unavailable" in result.stderr
    assert "Ganache is not TVM evidence" in result.stderr


def test_tvm_image_is_an_immutable_official_tre_digest() -> None:
    config = corridor.load_corridor_config()
    assert config.tvm_runner == {
        "image": "tronbox/tre@sha256:e57deeb0d8201498549dbec28e7c329d8647ef0976b547cfbb6fa6a41a10f491",
        "platform": "linux/amd64",
    }


def test_committed_compiler_lock_is_the_two_exact_authenticated_0_8_24_builds() -> None:
    config = corridor.load_corridor_config()
    for target in corridor.TARGETS:
        expected = corridor.EXPECTED_COMPILERS[target]
        actual = config.compilers[target]
        assert actual.identity == expected["identity"]
        assert actual.reported_version == expected["reported_version"]
        assert actual.url == expected["url"]
        assert actual.sha256 == expected["sha256"]


def test_tvm_tooling_is_integrity_locked_and_audit_is_mandatory() -> None:
    tooling = ROOT / "scripts" / "contract_tooling"
    package = json.loads((tooling / "package.json").read_text(encoding="utf-8"))
    package_lock = json.loads((tooling / "package-lock.json").read_text(encoding="utf-8"))
    assert package["dependencies"] == {
        "@noble/hashes": "1.3.2",
        "tronweb": "6.4.0",
    }
    assert package["overrides"] == {"ws": "8.21.0"}
    assert all("ganache" not in name for name in package_lock["packages"])
    for name, value in package_lock["packages"].items():
        if name and "resolved" in value:
            assert value["resolved"].startswith("https://registry.npmjs.org/")
            assert value.get("integrity", "").startswith("sha512-")
    runner = (ROOT / "scripts" / "contract_tvm_runner.sh").read_text(encoding="utf-8")
    assert "audit --omit=dev --audit-level=low" in runner
    assert "--pull always" in runner
    assert "SCCP_TVM_STATIC_ONLY=1" in runner
    assert "contract_artifact_corridor.py\" snapshot" in runner
    assert "--check-source-inputs" in runner
    assert runner.count("$SNAPSHOT_MANIFEST\" \"$SNAPSHOT_VECTORS") == 2
    assert 'contract_tvm_smoke.mjs\" "$MANIFEST"' not in runner


def test_evm_tooling_is_locked_audited_hardhat_without_ganache() -> None:
    tooling = ROOT / "scripts" / "contract_tooling" / "evm-runtime"
    package = json.loads((tooling / "package.json").read_text(encoding="utf-8"))
    package_lock = json.loads((tooling / "package-lock.json").read_text(encoding="utf-8"))
    assert package["dependencies"] == {
        "ethers": "6.16.0",
        "hardhat": "3.9.1",
        "solc": "file:../authenticated-solc",
    }
    assert package["overrides"] == {"ws": "8.21.0"}
    assert all("ganache" not in name.casefold() for name in package_lock["packages"])
    for name, value in package_lock["packages"].items():
        if not name or "resolved" not in value:
            continue
        if value["resolved"] == "../authenticated-solc":
            assert name == "node_modules/solc"
            continue
        assert value["resolved"].startswith("https://registry.npmjs.org/")
        assert value.get("integrity", "").startswith("sha512-")

    provider = (tooling / "hardhat-provider.js").read_text(encoding="utf-8")
    assert 'rawRequest("eth_chainId", [])' in provider
    assert "reported the wrong chain id" in provider
    smoke = (ROOT / "scripts" / "sccp_evm_contract_smoke.sh").read_text(
        encoding="utf-8"
    )
    assert "audit --omit=dev --audit-level=low" in smoke
    assert "0.7.4+commit.3f05b770.Emscripten.clang" in smoke
    assert "SCCP_CONTRACT_ARTIFACT_MANIFEST" in smoke
    assert "soljson-v0.7.4+commit.3f05b770.js" in smoke
    assert "2b55ed5fec4d9625b6c7b3ab1abd2b7fb7dd2a9c68543bf0323db2c7e2d55af2" in smoke
    assert "ganache" not in smoke.casefold()


def test_tvm_smoke_probes_chain_before_real_deployment_and_covers_adversarial_state() -> None:
    script_path = ROOT / "scripts" / "contract_tvm_smoke.mjs"
    source = script_path.read_text(encoding="utf-8")
    syntax = subprocess.run(
        ["node", "--check", str(script_path)],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert syntax.returncode == 0, syntax.stderr
    assert "expectRejected" not in source
    assert "expectConfirmedTvmFailure" in source
    assert "utils.crypto.sha3" not in source
    assert "utils.ethersUtils.keccak256" in source
    assert "native_transfer_event_v1" not in source
    assert "validateNativeVectors" in source
    assert "independentPayloadHash" in source
    assert "decodeSccpTransferLog" in source
    assert "[0, 1, 127, 128, 129, 255, 256, 257, 511]" in source
    assert "TRON_MAINNET_PROFILE = 10" in source
    assert "TRON_MAINNET_CHAIN_ID = 0x2b6653dcn" in source
    probe = source.index("await assertMainnetChainId(endpoint);")
    account_read = source.index('fetchJson(endpoint, "/admin/accounts-json")')
    valid_deploy = source.index("const verifier = await deploy(")
    assert probe < account_read < valid_deploy
    assert (
        "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol:"
        "SccpTronGroth16Bn254MessageVerifier"
    ) in source
    assert "contracts/tron/sccp/TairaXorSccpBridge.sol:TairaXorSccpBridge" in source
    for required_case in (
        "wrong-chain verifier",
        "wrong-chain bridge",
        "forged EXTCODEHASH policy",
        "unauthorized direct token mint",
        "hostile invalid BN254 proof",
        "wrong route revision rollback",
        "destination replay",
        "noncanonical Taira recipient burn",
        "unaligned Taira burn amount",
        "unauthorized direct token burn",
    ):
        assert required_case in source


def test_evm_runtime_is_locked_and_mines_reverting_transactions() -> None:
    tooling = ROOT / "scripts" / "contract_tooling" / "evm-runtime"
    package = json.loads((tooling / "package.json").read_text(encoding="utf-8"))
    package_lock = json.loads((tooling / "package-lock.json").read_text(encoding="utf-8"))
    assert package["dependencies"] == {
        "ethers": "6.16.0",
        "hardhat": "3.9.1",
        "solc": "file:../authenticated-solc",
    }
    assert package_lock["packages"]["node_modules/solc"]["resolved"] == "../authenticated-solc"
    assert all("ganache" not in name.lower() for name in package_lock["packages"])
    for name, value in package_lock["packages"].items():
        if name and "resolved" in value and not value.get("link"):
            assert value["resolved"].startswith("https://registry.npmjs.org/")
            assert value.get("integrity", "").startswith("sha512-")

    provider = (tooling / "hardhat-provider.js").read_text(encoding="utf-8")
    assert "throwOnTransactionFailures: true" in provider
    assert "allowUnlimitedContractSize: false" in provider
    assert "this.child.kill(\"SIGTERM\")" in provider

    runner = (ROOT / "scripts" / "sccp_evm_contract_smoke.sh").read_text(encoding="utf-8")
    assert '"$NPM_BIN" ci --ignore-scripts --no-audit --no-fund' in runner
    assert '"$NPM_BIN" audit --omit=dev --audit-level=low' in runner
    assert "SCCP_EXPECTED_SOLC_BUILD" in runner
    assert "0.7.4+commit.3f05b770.Emscripten.clang" in runner
    assert "--check-source-inputs" in runner
