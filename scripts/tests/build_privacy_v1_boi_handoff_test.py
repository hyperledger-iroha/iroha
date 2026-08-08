"""Hostile source-only tests for the Privacy v1 BOI handoff assembler."""

from __future__ import annotations

import dataclasses
import hashlib
import json
import stat
import subprocess
import zipfile
from dataclasses import dataclass
from pathlib import Path

import pytest

from scripts import build_privacy_v1_boi_handoff as boi
from scripts import check_native_sdk_abi22_artifact as abi22
from scripts import release_artifact_contract as contract
from scripts import taira_privacy_protocol_receipt as privacy_evidence
from scripts import taira_rollout_admission as admission


ROOT = Path(__file__).resolve().parents[2]
SOURCE_COMMIT = "1" * 40
DPN_COMMIT = "2" * 40
SOURCE_MANIFEST = "3" * 64
VALIDATOR_SHA256 = "4" * 64
QUALIFICATION_ID = "5" * 64
PROTOCOL_ID = "6" * 64
RELEASE_MANIFEST_SHA256 = "7" * 64
NATIVE_VALIDATOR_SHA256 = "b" * 64
MACOS_HANDOFF_SHA256 = "c" * 64


@dataclass
class Fixture:
    root: Path
    output: Path
    candidate: boi.AuthenticatedCandidate


def _sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _elf_aarch64() -> bytes:
    payload = bytearray(96)
    payload[:7] = b"\x7fELF\x02\x01\x01"
    payload[16:18] = (3).to_bytes(2, "little")
    payload[18:20] = (183).to_bytes(2, "little")
    payload[20:24] = (1).to_bytes(4, "little")
    payload[64:] = b"native-aarch64-test-artifact!!!!"
    return bytes(payload)


def _write(root: Path, relative: str, payload: bytes) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)


def _wheel(path: Path, *, native: bool = True, pure: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, mode="w", compression=zipfile.ZIP_DEFLATED) as wheel:
        if native:
            wheel.writestr(
                "iroha_python/_crypto.cpython-312-aarch64-linux-gnu.so",
                _elf_aarch64(),
            )
        wheel.writestr(
            "iroha_python/privacy_wallet_worker.py",
            b'"""Thin IPWW controller fixture."""\n',
        )
        wheel.writestr(
            "iroha_python_privacy_v1-1.0.dist-info/WHEEL",
            (
                "Wheel-Version: 1.0\n"
                "Generator: iroha-test\n"
                f"Root-Is-Purelib: {'true' if pure else 'false'}\n"
                "Tag: cp312-cp312-manylinux_2_17_aarch64\n"
            ).encode("ascii"),
        )
        wheel.writestr(
            "iroha_python_privacy_v1-1.0.dist-info/METADATA",
            b"Metadata-Version: 2.1\nName: iroha-python\nVersion: 1.0\n",
        )
        wheel.writestr(
            "iroha_python_privacy_v1-1.0.dist-info/RECORD",
            b"iroha_python/privacy_wallet_worker.py,,\n",
        )


def _schema(identifier: str) -> bytes:
    return contract.canonical_json_bytes(
        {
            "$id": identifier,
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "additionalProperties": False,
            "properties": {},
            "type": "object",
        }
    )


def _source() -> dict[str, object]:
    return {
        "cargo_lock_sha256": boi.FIXED_CARGO_LOCK_SHA256,
        "commit": SOURCE_COMMIT,
        "dpn_validator_release_commit": DPN_COMMIT,
        "workspace_source_manifest_sha256": SOURCE_MANIFEST,
    }


def _refresh_manifest(root: Path) -> str:
    rows = []
    for relative in sorted(boi.SOURCE_ARTIFACT_PATHS):
        payload = (root / relative).read_bytes()
        rows.append({"path": relative, "sha256": _sha(payload), "size": len(payload)})
    payload = (
        json.dumps(
            {
                "files": rows,
                "kind": boi.SOURCE_HANDOFF_KIND,
                "schema": boi.SOURCE_HANDOFF_SCHEMA,
                "schema_version": 1,
            },
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    _write(root, boi.SOURCE_HANDOFF_MANIFEST, payload)
    return _sha(payload)


def _receipt_payloads(
    source: dict[str, object], artifact_handoff_sha256: str, matrix_sha256: str
) -> tuple[bytes, bytes]:
    qualification = contract.canonical_json_bytes(
        {
            "artifact_handoff_sha256": artifact_handoff_sha256,
            "receipt_id": QUALIFICATION_ID,
            "schema": admission.MACOS_RECEIPT_SCHEMA,
            "schema_version": admission.MACOS_RECEIPT_SCHEMA_VERSION,
            "source": source,
            "validator_binary_sha256": VALIDATOR_SHA256,
        }
    )
    protocol = contract.canonical_json_bytes(
        {
            "candidate": {
                "artifact_handoff_sha256": artifact_handoff_sha256,
                "exact12_matrix_sha256": matrix_sha256,
                "source": source,
                "validator_binary_sha256": VALIDATOR_SHA256,
            },
            "receipt_id": PROTOCOL_ID,
            "schema": privacy_evidence.RECEIPT_SCHEMA,
            "schema_version": privacy_evidence.RECEIPT_SCHEMA_VERSION,
        }
    )
    return qualification, protocol


def _candidate(
    root: Path, boi_artifact_inventory_sha256: str, matrix_sha256: str
) -> boi.AuthenticatedCandidate:
    archive = root.parent / "admitted-candidate.tar.gz"
    if not archive.exists():
        archive.write_bytes(b"signed-admitted-candidate-fixture\n")
    source = _source()
    qualification, protocol = _receipt_payloads(
        source, MACOS_HANDOFF_SHA256, matrix_sha256
    )
    pair = {"json_sha256": "8" * 64, "norito_sha256": "9" * 64}
    native_json = {
        "all_native_stages_passed": True,
        "build_profile": "release",
        "cargo_lock_sha256": boi.FIXED_CARGO_LOCK_SHA256,
        "command_manifest": pair,
        "contains_canonical_proof_artifacts": True,
        "contains_witnesses": False,
        "exact12_matrix_sha256": matrix_sha256,
        "expectations": pair,
        "fixed_stage_count": 48,
        "isolation_policy_enforced": True,
        "runner_binary_sha256": "a" * 64,
        "schema_version": 1,
        "source_sha256": SOURCE_MANIFEST,
        "stage_artifacts": pair,
        "validator_binary_sha256": NATIVE_VALIDATOR_SHA256,
        "x509_resource": pair,
    }
    return boi.AuthenticatedCandidate(
        source=source,
        artifact_handoff_sha256=MACOS_HANDOFF_SHA256,
        boi_artifact_inventory_sha256=boi_artifact_inventory_sha256,
        boi_artifact_inventory=(
            root / boi.SOURCE_HANDOFF_MANIFEST
        ).read_bytes(),
        archive=archive,
        archive_info=contract.stable_hash_path(archive),
        release_manifest_sha256=RELEASE_MANIFEST_SHA256,
        native_validator_binary_sha256=NATIVE_VALIDATOR_SHA256,
        validator_binary_sha256=VALIDATOR_SHA256,
        exact12_matrix_sha256=matrix_sha256,
        qualification_receipt_id=QUALIFICATION_ID,
        privacy_protocol_receipt_id=PROTOCOL_ID,
        qualification_receipt=qualification,
        privacy_protocol_receipt=protocol,
        native_receipt_norito=b"NRT0" + b"\x11" * 28,
        native_receipt_json=contract.canonical_json_bytes(native_json),
    )


def _fixture(tmp_path: Path) -> Fixture:
    root = tmp_path / "artifact-handoff"
    root.mkdir(parents=True)
    capability = b"NRT0" + b"\x23" * 60
    _write(root, boi.CAPABILITY_PATH, capability)
    _wheel(root / boi.WHEEL_PATH)
    _write(root, boi.WORKER_PATH, _elf_aarch64())
    library = _elf_aarch64()
    _write(root, boi.ABI_LIBRARY_PATH, library)
    header = "".join(
        f"int32_t {name}(void);\n" for name in abi22.APPROVED_PRIVACY_C_EXPORTS
    ).encode("ascii")
    _write(root, boi.ABI_HEADER_PATH, header)
    _write(
        root,
        boi.ABI_SYMBOLS_PATH,
        "".join(f"{name}\n" for name in abi22.APPROVED_PRIVACY_C_EXPORTS).encode(
            "ascii"
        ),
    )
    abi_manifest = {
        "artifact_sha256": _sha(library),
        "artifact_size": len(library),
        "bridge_abi_version": 22,
        "privacy_c_exports": list(abi22.APPROVED_PRIVACY_C_EXPORTS),
        "privacy_c_exports_inspected": True,
        "required_symbols": list(abi22.REQUIRED_SYMBOLS["c-jni"]),
        "schema": abi22.SCHEMA,
        "sdk": "c-jni",
        "source_commit": SOURCE_COMMIT,
        "source_tree_clean": True,
        "target": "aarch64-unknown-linux-gnu",
        "workspace_source_manifest_sha256": SOURCE_MANIFEST,
    }
    _write(
        root,
        boi.ABI_EVIDENCE_PATH,
        abi22.canonical_manifest_bytes(abi_manifest),
    )
    _write(root, boi.CAPABILITY_SCHEMA_PATH, _schema(boi.CAPABILITY_SCHEMA_ID))
    _write(root, boi.WORKER_SCHEMA_PATH, _schema(boi.WORKER_SCHEMA_ID))
    cargo_lock = (ROOT / "Cargo.lock").read_bytes()
    assert _sha(cargo_lock) == boi.FIXED_CARGO_LOCK_SHA256
    _write(root, boi.CARGO_LOCK_PATH, cargo_lock)
    matrix = (ROOT / "fixtures/privacy/exact12_v1.tsv").read_bytes()
    _write(root, boi.MATRIX_PATH, matrix)
    _write(
        root,
        boi.SOURCE_MANIFEST_PATH,
        f"{SOURCE_MANIFEST}\n".encode("ascii"),
    )
    wheel_sha = _sha((root / boi.WHEEL_PATH).read_bytes())
    config = (
        "[privacy_v1]\n"
        f'abi22_library = "{boi.ABI_LIBRARY_PATH}"\n'
        f'abi22_sha256 = "{_sha(library)}"\n'
        f'capability_manifest = "{boi.CAPABILITY_PATH}"\n'
        'network_availability_source = "torii-committed-capability-manifest"\n'
        f'python_wheel = "{boi.WHEEL_PATH}"\n'
        f'python_wheel_sha256 = "{wheel_sha}"\n'
        "witness_crosses_ffi = false\n"
        f'worker = "{boi.WORKER_PATH}"\n'
        f'worker_sha256 = "{_sha(_elf_aarch64())}"\n'
    ).encode("utf-8")
    _write(root, boi.CONFIG_PATH, config)
    handoff_sha = _refresh_manifest(root)
    return Fixture(
        root=root,
        output=tmp_path / "boi-output",
        candidate=_candidate(root, handoff_sha, _sha(matrix)),
    )


def _refresh_candidate(fixture: Fixture) -> None:
    handoff_sha = _refresh_manifest(fixture.root)
    fixture.candidate = _candidate(
        fixture.root,
        handoff_sha,
        _sha((fixture.root / boi.MATRIX_PATH).read_bytes()),
    )


def _assemble(
    fixture: Fixture,
    *,
    wheel_probe: object | None = None,
    abi_probe: object | None = None,
) -> dict[str, object]:
    return boi.assemble_boi_handoff(
        fixture.root.resolve(),
        fixture.output.resolve(),
        fixture.candidate,
        python="python3",
        wheel_probe=(wheel_probe if callable(wheel_probe) else lambda *_: None),
        abi_runtime_validator=(
            abi_probe if callable(abi_probe) else lambda _path: None
        ),
    )


def test_assembles_one_closed_source_and_candidate_bound_bundle(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    calls: list[str] = []

    def wheel_probe(*_args: object) -> None:
        calls.append("wheel")

    def abi_probe(_path: Path) -> None:
        calls.append("abi")

    result = _assemble(fixture, wheel_probe=wheel_probe, abi_probe=abi_probe)
    assert result["ready"] is True
    assert calls == ["abi", "wheel"]
    inventory_path = fixture.output / boi.OUTPUT_INVENTORY
    inventory = json.loads(inventory_path.read_bytes())
    assert inventory["schema"] == boi.SCHEMA
    assert inventory["source"] == _source()
    assert inventory["candidate"]["artifact_handoff_sha256"] == (
        fixture.candidate.artifact_handoff_sha256
    )
    assert inventory["candidate"]["boi_artifact_inventory_sha256"] == (
        fixture.candidate.boi_artifact_inventory_sha256
    )
    assert inventory["candidate"]["linux_validator_binary_sha256"] == (
        NATIVE_VALIDATOR_SHA256
    )
    assert inventory["candidate"]["macos_validator_binary_sha256"] == (VALIDATOR_SHA256)
    assert inventory["contract"]["privacy_c_exports"] == list(
        abi22.APPROVED_PRIVACY_C_EXPORTS
    )
    assert inventory["contract"]["jindo_assurance"] == "available-experimental"
    assert inventory["contract"]["witness_crosses_ffi"] is False
    assert (fixture.output / boi.QUALIFICATION_RECEIPT_PATH).read_bytes() == (
        fixture.candidate.qualification_receipt
    )
    assert stat.S_IMODE(fixture.output.stat().st_mode) == 0o555
    assert stat.S_IMODE((fixture.output / boi.WORKER_PATH).stat().st_mode) == 0o555
    assert stat.S_IMODE(inventory_path.stat().st_mode) == 0o444


def test_candidate_authority_static_rebind_covers_the_exact_inventory(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)

    captured = boi.validate_candidate_boi_artifact_handoff(
        fixture.root.resolve(),
        source=fixture.candidate.source,
        exact12_matrix_sha256=fixture.candidate.exact12_matrix_sha256,
        inventory_sha256=fixture.candidate.boi_artifact_inventory_sha256,
        inventory_payload=fixture.candidate.boi_artifact_inventory,
    )

    assert tuple(sorted(captured)) == tuple(sorted(boi.SOURCE_ARTIFACT_PATHS))
    assert len(captured) == 13


@pytest.mark.parametrize(
    "relative",
    [
        boi.CAPABILITY_PATH,
        boi.WHEEL_PATH,
        boi.WORKER_PATH,
        boi.ABI_LIBRARY_PATH,
        boi.ABI_HEADER_PATH,
        boi.ABI_SYMBOLS_PATH,
        boi.ABI_EVIDENCE_PATH,
        boi.CAPABILITY_SCHEMA_PATH,
        boi.WORKER_SCHEMA_PATH,
        boi.CONFIG_PATH,
        boi.CARGO_LOCK_PATH,
        boi.MATRIX_PATH,
        boi.SOURCE_MANIFEST_PATH,
    ],
)
def test_missing_required_artifact_fails_before_output(
    tmp_path: Path, relative: str
) -> None:
    fixture = _fixture(tmp_path)
    (fixture.root / relative).unlink()
    with pytest.raises(boi.BoiHandoffError, match="exact first-release file set"):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_extra_artifact_and_symlink_fail_closed(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    _write(fixture.root, "unreviewed.txt", b"rogue\n")
    with pytest.raises(boi.BoiHandoffError, match="exact first-release file set"):
        _assemble(fixture)
    assert not fixture.output.exists()

    (fixture.root / "unreviewed.txt").unlink()
    (fixture.root / boi.CAPABILITY_PATH).unlink()
    (fixture.root / boi.CAPABILITY_PATH).symlink_to(
        fixture.root / boi.SOURCE_MANIFEST_PATH
    )
    with pytest.raises(
        (boi.BoiHandoffError, contract.ReleaseArtifactError), match="symlink"
    ):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_recomputed_handoff_cannot_substitute_frozen_lock(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    _write(fixture.root, boi.CARGO_LOCK_PATH, b"forged lock\n")
    _refresh_candidate(fixture)
    with pytest.raises(
        boi.BoiHandoffError, match="frozen release lock|different Cargo.lock"
    ):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_recomputed_handoff_cannot_substitute_source_or_matrix(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    _write(fixture.root, boi.SOURCE_MANIFEST_PATH, f"{'9' * 64}\n".encode())
    _refresh_candidate(fixture)
    with pytest.raises(
        boi.BoiHandoffError,
        match="source-manifest file differs|different source manifest",
    ):
        _assemble(fixture)
    assert not fixture.output.exists()

    fixture = _fixture(tmp_path / "matrix-case")
    matrix = (
        (fixture.root / boi.MATRIX_PATH)
        .read_bytes()
        .replace(b"matrix-version\t1", b"matrix-version\t2", 1)
    )
    _write(fixture.root, boi.MATRIX_PATH, matrix)
    _refresh_candidate(fixture)
    with pytest.raises(
        boi.BoiHandoffError, match="canonical v1 row|different Exact12 matrix"
    ):
        _assemble(fixture)
    assert not fixture.output.exists()


@pytest.mark.parametrize("attack", ["symbols", "header", "evidence", "library"])
def test_recomputed_handoff_cannot_expand_or_substitute_abi22(
    tmp_path: Path, attack: str
) -> None:
    fixture = _fixture(tmp_path)
    if attack == "symbols":
        path = boi.ABI_SYMBOLS_PATH
        payload = (fixture.root / path).read_bytes() + b"iroha_privacy_rogue_v23\n"
    elif attack == "header":
        path = boi.ABI_HEADER_PATH
        payload = (
            fixture.root / path
        ).read_bytes() + b"int iroha_privacy_rogue_v23(void);\n"
    elif attack == "evidence":
        path = boi.ABI_EVIDENCE_PATH
        value = json.loads((fixture.root / path).read_bytes())
        value["workspace_source_manifest_sha256"] = "8" * 64
        payload = abi22.canonical_manifest_bytes(value)
    else:
        path = boi.ABI_LIBRARY_PATH
        payload = b"not-elf\n"
    _write(fixture.root, path, payload)
    _refresh_candidate(fixture)
    with pytest.raises(boi.BoiHandoffError, match="ABI22"):
        _assemble(fixture)
    assert not fixture.output.exists()


@pytest.mark.parametrize("attack", ["pure", "missing-native"])
def test_python_wheel_must_be_native_aarch64(tmp_path: Path, attack: str) -> None:
    fixture = _fixture(tmp_path)
    _wheel(
        fixture.root / boi.WHEEL_PATH,
        native=attack != "missing-native",
        pure=attack == "pure",
    )
    _refresh_candidate(fixture)
    with pytest.raises(boi.BoiHandoffError, match="wheel|Wheel"):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_native_wheel_probe_stages_only_authenticated_inputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    captured, _manifest = boi._validate_source_handoff(
        fixture.root.resolve(),
        source=fixture.candidate.source,
        exact12_matrix_sha256=fixture.candidate.exact12_matrix_sha256,
        inventory_sha256=fixture.candidate.boi_artifact_inventory_sha256,
        inventory_payload=fixture.candidate.boi_artifact_inventory,
    )
    native_member, _controller_member = boi._wheel_layout(
        fixture.root.resolve(), captured
    )
    capability = (fixture.root / boi.CAPABILITY_PATH).read_bytes()
    expected_worker = (fixture.root / boi.WORKER_PATH).read_bytes()
    observed: list[str] = []

    def run(
        arguments: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        observed.append(arguments[0])
        assert Path(arguments[5]).read_bytes() == _elf_aarch64()
        assert Path(arguments[6]).read_bytes() == capability
        assert (
            Path(arguments[7]).read_bytes() == b'"""Thin IPWW controller fixture."""\n'
        )
        assert Path(arguments[8]).read_bytes() == expected_worker
        assert arguments[9] == _sha(expected_worker)
        return subprocess.CompletedProcess(arguments, 0)

    monkeypatch.setattr(boi.subprocess, "run", run)
    boi._probe_native_wheel(
        fixture.root.resolve(),
        captured,
        native_member,
        capability,
        "python-native-fixture",
    )
    assert observed == ["python-native-fixture"]


@pytest.mark.parametrize(
    "relative", [boi.CAPABILITY_SCHEMA_PATH, boi.WORKER_SCHEMA_PATH]
)
def test_schema_substitution_fails_with_recomputed_inventory(
    tmp_path: Path, relative: str
) -> None:
    fixture = _fixture(tmp_path)
    _write(fixture.root, relative, _schema("iroha://schemas/privacy/foreign-v1"))
    _refresh_candidate(fixture)
    with pytest.raises(
        boi.BoiHandoffError, match="wrong first-release schema identity"
    ):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_sample_config_cannot_enable_witness_ffi_or_stale_worker(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.root / boi.CONFIG_PATH
    payload = path.read_text().replace(
        "witness_crosses_ffi = false", "witness_crosses_ffi = true"
    )
    path.write_text(payload)
    _refresh_candidate(fixture)
    with pytest.raises(boi.BoiHandoffError, match="sample configuration"):
        _assemble(fixture)
    assert not fixture.output.exists()


@pytest.mark.parametrize("which", ["wheel", "abi"])
def test_native_replay_failure_emits_no_bundle(tmp_path: Path, which: str) -> None:
    fixture = _fixture(tmp_path)

    def reject(*_args: object) -> None:
        raise boi.BoiHandoffError(f"{which} native replay rejected")

    with pytest.raises(boi.BoiHandoffError, match="native replay rejected"):
        _assemble(
            fixture,
            wheel_probe=reject if which == "wheel" else None,
            abi_probe=reject if which == "abi" else None,
        )
    assert not fixture.output.exists()


@pytest.mark.parametrize("relative", [boi.WHEEL_PATH, boi.WORKER_PATH])
def test_changed_artifact_after_preflight_never_publishes_ready_output(
    tmp_path: Path, relative: str
) -> None:
    fixture = _fixture(tmp_path)

    def mutate_after_preflight(*_args: object) -> None:
        (fixture.root / relative).write_bytes(b"changed after preflight\n")

    with pytest.raises(
        (boi.BoiHandoffError, contract.ReleaseArtifactError),
        match="changed during replay|no longer matches",
    ):
        _assemble(fixture, wheel_probe=mutate_after_preflight)
    assert not fixture.output.exists()


@pytest.mark.parametrize(
    "attack",
    [
        "extra-field",
        "failed-stage",
        "contains-witnesses",
        "source",
        "cargo-lock",
        "matrix",
        "native-validator",
        "zero-runner",
        "pair-shape",
        "zero-pair",
    ],
)
def test_native_release_receipt_substitution_fails_closed(
    tmp_path: Path, attack: str
) -> None:
    fixture = _fixture(tmp_path)
    receipt = json.loads(fixture.candidate.native_receipt_json)
    if attack == "extra-field":
        receipt["untrusted_pass_marker"] = True
    elif attack == "failed-stage":
        receipt["all_native_stages_passed"] = False
    elif attack == "contains-witnesses":
        receipt["contains_witnesses"] = True
    elif attack == "source":
        receipt["source_sha256"] = "c" * 64
    elif attack == "cargo-lock":
        receipt["cargo_lock_sha256"] = "c" * 64
    elif attack == "matrix":
        receipt["exact12_matrix_sha256"] = "c" * 64
    elif attack == "native-validator":
        receipt["validator_binary_sha256"] = "c" * 64
    elif attack == "zero-runner":
        receipt["runner_binary_sha256"] = "0" * 64
    elif attack == "pair-shape":
        receipt["x509_resource"]["self_reported_pass"] = True
    else:
        receipt["expectations"]["json_sha256"] = "0" * 64
    fixture.candidate = dataclasses.replace(
        fixture.candidate,
        native_receipt_json=contract.canonical_json_bytes(receipt),
    )
    with pytest.raises(boi.BoiHandoffError):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_native_receipt_accepts_canonical_json_byte_array_digests(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    receipt = json.loads(fixture.candidate.native_receipt_json)
    for name in (
        "cargo_lock_sha256",
        "exact12_matrix_sha256",
        "runner_binary_sha256",
        "source_sha256",
        "validator_binary_sha256",
    ):
        receipt[name] = list(bytes.fromhex(receipt[name]))
    for name in (
        "command_manifest",
        "expectations",
        "stage_artifacts",
        "x509_resource",
    ):
        for digest in ("json_sha256", "norito_sha256"):
            receipt[name][digest] = list(bytes.fromhex(receipt[name][digest]))
    fixture.candidate = dataclasses.replace(
        fixture.candidate,
        native_receipt_json=contract.canonical_json_bytes(receipt),
    )
    result = _assemble(fixture)
    assert result["ready"] is True


def test_candidate_or_receipt_substitution_fails_before_output(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.candidate = dataclasses.replace(
        fixture.candidate, artifact_handoff_sha256="8" * 64
    )
    with pytest.raises(boi.BoiHandoffError, match="qualification receipt differs"):
        _assemble(fixture)
    assert not fixture.output.exists()

    fixture = _fixture(tmp_path / "receipt-case")
    fixture.candidate = dataclasses.replace(
        fixture.candidate, native_receipt_norito=b"fabricated"
    )
    with pytest.raises(boi.BoiHandoffError, match="not a Norito archive"):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_macos_handoff_digest_cannot_substitute_signed_boi_inventory(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    fixture.candidate = dataclasses.replace(
        fixture.candidate,
        boi_artifact_inventory_sha256=fixture.candidate.artifact_handoff_sha256,
    )

    with pytest.raises(boi.BoiHandoffError, match="BOI inventory admitted"):
        _assemble(fixture)
    assert not fixture.output.exists()


def test_candidate_archive_toctou_never_publishes_output(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)

    def mutate_candidate(_path: Path) -> None:
        fixture.candidate.archive.write_bytes(b"candidate changed after admission\n")

    with pytest.raises(boi.BoiHandoffError, match="candidate archive changed"):
        _assemble(fixture, abi_probe=mutate_candidate)
    assert not fixture.output.exists()


def test_raced_output_is_never_replaced(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    publish = boi._publish_directory_noreplace

    def race(staging: Path, destination: Path, parent_fd: int) -> None:
        destination.mkdir()
        (destination / "foreign-owner-marker").write_bytes(b"preserve me\n")
        publish(staging, destination, parent_fd)

    monkeypatch.setattr(boi, "_publish_directory_noreplace", race)
    with pytest.raises(boi.BoiHandoffError, match="appeared before"):
        _assemble(fixture)
    assert (fixture.output / "foreign-owner-marker").read_bytes() == b"preserve me\n"
    assert not (fixture.output / boi.OUTPUT_INVENTORY).exists()


def test_noncanonical_and_duplicate_handoff_json_are_rejected(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    manifest = fixture.root / boi.SOURCE_HANDOFF_MANIFEST
    value = json.loads(manifest.read_bytes())
    manifest.write_text(json.dumps(value) + "\n")
    fixture.candidate = dataclasses.replace(
        fixture.candidate,
        boi_artifact_inventory_sha256=_sha(manifest.read_bytes()),
        boi_artifact_inventory=manifest.read_bytes(),
    )
    with pytest.raises(boi.BoiHandoffError, match="not canonical"):
        _assemble(fixture)
    assert not fixture.output.exists()

    fixture = _fixture(tmp_path / "duplicate-case")
    original = (fixture.root / boi.SOURCE_HANDOFF_MANIFEST).read_text()
    duplicate = original.replace(
        '"kind":', '"kind": "privacy-v1-boi-artifacts",\n  "kind":', 1
    )
    (fixture.root / boi.SOURCE_HANDOFF_MANIFEST).write_text(duplicate)
    handoff_sha = _sha(duplicate.encode())
    fixture.candidate = _candidate(
        fixture.root,
        handoff_sha,
        fixture.candidate.exact12_matrix_sha256,
    )
    with pytest.raises(boi.BoiHandoffError, match="duplicate JSON object key"):
        _assemble(fixture)
    assert not fixture.output.exists()
