"""Hostile source-only tests for the Privacy v1 BOI handoff assembler."""

from __future__ import annotations

import dataclasses
import ctypes
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
COMPILED_CATALOG = b"NRT0canonical-compiled-profile-catalog-v1"
QUALIFICATION_PUBLIC_KEY = b"q" * 32
QUALIFICATION_FINGERPRINT = hashlib.sha256(QUALIFICATION_PUBLIC_KEY).hexdigest()
QUALIFICATION_NOW = 1_800_000_000


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
    authority = root.parent / "candidate-authority"
    authority.mkdir(exist_ok=True)
    authority_payloads = {
        "release_manifest.json": b"signed candidate manifest fixture\n",
        "release_manifest.json.pub": b"p" * 32,
        "release_manifest.json.sig": b"s" * 64,
    }
    for relative, payload in authority_payloads.items():
        path = authority / relative
        if not path.exists():
            path.write_bytes(payload)
    authority_files = {
        relative: contract.stable_hash_relative(authority, relative)
        for relative in admission.FINAL_AUTHORITY_FILES
    }
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
        authority_dir=authority,
        authority_files=authority_files,
        release_manifest_sha256=authority_files["release_manifest.json"].sha256,
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
    qualification_key = fixture.root.parent / "qualification-signing.pub"
    if not qualification_key.exists():
        qualification_key.write_bytes(QUALIFICATION_PUBLIC_KEY)

    def qualification_signer(
        manifest: Path,
        _external_signer: Path,
        raw_public_key: Path,
        fingerprint: str,
        signature_output: Path,
        public_key_output: Path,
        _verifier: Path,
        _verifier_sha256: str,
    ) -> dict[str, object]:
        assert fingerprint == QUALIFICATION_FINGERPRINT
        public_key_output.write_bytes(raw_public_key.read_bytes())
        signature_output.write_bytes(b"s" * 64)
        return {"manifest_sha256": _sha(manifest.read_bytes())}

    return boi.assemble_boi_handoff(
        fixture.root.resolve(),
        fixture.output.resolve(),
        fixture.candidate,
        python="python3",
        wheel_probe=(wheel_probe if callable(wheel_probe) else lambda *_: None),
        abi_runtime_validator=(
            abi_probe if callable(abi_probe) else lambda _path: COMPILED_CATALOG
        ),
        qualification_external_signer=fixture.root.parent / "hsm-signer",
        qualification_signing_public_key=qualification_key,
        trusted_qualification_signing_fingerprint=QUALIFICATION_FINGERPRINT,
        qualification_host_id="boi-host-v1",
        qualification_installation_id="boi-installation-v1",
        controller_closure_digest="d" * 64,
        workflow_run_id=101,
        workflow_run_attempt=2,
        release_manifest_verifier_path=fixture.root.parent / "native-verifier",
        trusted_release_manifest_verifier_sha256="e" * 64,
        qualification_issued_at_unix=QUALIFICATION_NOW,
        qualification_signer=qualification_signer,
    )


def test_assembles_one_closed_source_and_candidate_bound_bundle(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    calls: list[str] = []

    def wheel_probe(
        _root: Path,
        _captured: object,
        _native_member: str,
        _capability: bytes,
        compiled_catalog: bytes,
        _python: str,
    ) -> None:
        assert compiled_catalog == COMPILED_CATALOG
        calls.append("wheel")

    def abi_probe(_path: Path) -> bytes:
        calls.append("abi")
        return COMPILED_CATALOG

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
    transport = json.loads(
        (fixture.output / boi.QUALIFIED_HANDOFF_MANIFEST).read_bytes()
    )
    assert transport["kind"] == boi.QUALIFIED_HANDOFF_KIND
    assert transport["files"] == sorted(
        transport["files"], key=lambda row: row["path"]
    )
    archive_relative = inventory["candidate"]["archive_path"]
    assert (fixture.output / archive_relative).read_bytes() == (
        fixture.candidate.archive.read_bytes()
    )
    assert {
        path.name for path in (fixture.output / boi.CANDIDATE_AUTHORITY_DIRECTORY).iterdir()
    } == set(admission.FINAL_AUTHORITY_FILES)
    assert stat.S_IMODE(fixture.output.stat().st_mode) == 0o555
    assert stat.S_IMODE((fixture.output / boi.WORKER_PATH).stat().st_mode) == 0o555
    assert stat.S_IMODE(inventory_path.stat().st_mode) == 0o444
    assert result["qualification_receipt_id"] == boi._qualification_receipt_id(
        (fixture.output / boi.QUALIFICATION_ENVELOPE_PATH).read_bytes()
    )
    envelope = json.loads(
        (fixture.output / boi.QUALIFICATION_ENVELOPE_PATH).read_bytes()
    )
    assert envelope["controller"] == {
        "closure_digest": "d" * 64,
        "host_id": "boi-host-v1",
        "installation_id": "boi-installation-v1",
        "role": "linux-boi-qualification",
    }
    assert envelope["workflow"] == {"run_attempt": 2, "run_id": 101}
    assert (
        fixture.output / boi.QUALIFICATION_PUBLIC_KEY_PATH
    ).read_bytes() == QUALIFICATION_PUBLIC_KEY


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
        probe_source = arguments[4]
        assert "privacy_compiled_profile_catalog_v1" in probe_source
        assert "privacy_validate_compiled_profile_catalog_v1(catalog)" in probe_source
        assert (
            "privacy_validate_exact12_capability_manifest_v1(archive)"
            in probe_source
        )
        assert "privacy_exact12_capability_manifest_v1(archive)" in probe_source
        assert "bytes(canonical_archive) != archive" in probe_source
        assert Path(arguments[5]).read_bytes() == _elf_aarch64()
        assert Path(arguments[6]).read_bytes() == capability
        assert Path(arguments[7]).read_bytes() == COMPILED_CATALOG
        assert (
            Path(arguments[8]).read_bytes() == b'"""Thin IPWW controller fixture."""\n'
        )
        assert Path(arguments[9]).read_bytes() == expected_worker
        assert arguments[10] == _sha(expected_worker)
        return subprocess.CompletedProcess(arguments, 0)

    monkeypatch.setattr(boi.subprocess, "run", run)
    boi._probe_native_wheel(
        fixture.root.resolve(),
        captured,
        native_member,
        capability,
        COMPILED_CATALOG,
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


def test_abi_runtime_executes_catalog_getter_and_validator_on_exact_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    catalog = b"NRT0" + b"canonical-compiled-profile-catalog"
    calls: list[tuple[str, bytes | None]] = []

    class Function:
        def __init__(self, callback):
            self.callback = callback
            self.argtypes = None
            self.restype = None

        def __call__(self, *args):
            return self.callback(*args)

    class Library:
        def __init__(self) -> None:
            self.buffers: list[object] = []

            def getter(out_pointer, out_length):
                buffer = (ctypes.c_uint8 * len(catalog)).from_buffer_copy(catalog)
                self.buffers.append(buffer)
                ctypes.cast(
                    out_pointer,
                    ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
                )[0] = ctypes.cast(buffer, ctypes.POINTER(ctypes.c_uint8))
                ctypes.cast(out_length, ctypes.POINTER(ctypes.c_ulong))[0] = len(catalog)
                calls.append(("get", None))
                return 0

            def validator(pointer, length):
                payload = ctypes.string_at(pointer, int(length))
                calls.append(("validate", payload))
                return 0 if payload == catalog else 5

            def free(_pointer):
                calls.append(("free", None))

            self.iroha_privacy_compiled_profile_catalog_v1 = Function(getter)
            self.iroha_privacy_validate_compiled_profile_catalog_v1 = Function(
                validator
            )
            self.iroha_privacy_free_buffer = Function(free)

    monkeypatch.setattr(boi.ctypes, "CDLL", lambda _path: Library())
    monkeypatch.setattr(abi22, "probe_artifact", lambda *_args: 22)
    monkeypatch.setattr(
        abi22,
        "inspect_exported_symbols",
        lambda *_args, **_kwargs: abi22.APPROVED_PRIVACY_C_EXPORTS,
    )
    monkeypatch.setattr(
        abi22,
        "validate_privacy_c_exports",
        lambda *_args, **_kwargs: abi22.APPROVED_PRIVACY_C_EXPORTS,
    )

    returned = boi._validate_abi_runtime(tmp_path / "libbridge.so")

    assert returned == catalog
    assert calls.count(("get", None)) == 2
    assert calls.count(("validate", catalog)) == 2
    assert calls.count(("free", None)) == 2


def test_mismatched_standalone_and_wheel_catalogs_never_publish(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    standalone_catalog = b"NRT0standalone-compiled-profile-catalog"
    wheel_catalog = b"NRT0different-wheel-compiled-profile-catalog"

    def abi_probe(_path: Path) -> bytes:
        return standalone_catalog

    def wheel_probe(
        _root: Path,
        _captured: object,
        _native_member: str,
        _capability: bytes,
        supplied_catalog: bytes,
        _python: str,
    ) -> None:
        assert supplied_catalog == standalone_catalog
        if supplied_catalog != wheel_catalog:
            raise boi.BoiHandoffError(
                "native wheel compiled catalog differs from the standalone ABI"
            )

    with pytest.raises(boi.BoiHandoffError, match="compiled catalog differs"):
        _assemble(fixture, wheel_probe=wheel_probe, abi_probe=abi_probe)
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

    def mutate_candidate(_path: Path) -> bytes:
        fixture.candidate.archive.write_bytes(b"candidate changed after admission\n")
        return COMPILED_CATALOG

    with pytest.raises(
        boi.BoiHandoffError,
        match="candidate archive changed|no longer matches its stable capture",
    ):
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


def _verify_qualified(
    fixture: Fixture,
    monkeypatch: pytest.MonkeyPatch,
    *,
    archive: Path | None = None,
    expected_host_id: str = "boi-host-v1",
    expected_run_id: int = 101,
    now_unix: int = QUALIFICATION_NOW,
    reject_signature: bool = False,
) -> boi.QualifiedBoiSnapshot:
    monkeypatch.setattr(
        boi, "authenticate_candidate", lambda _args: fixture.candidate
    )
    source = admission.SourceIdentity(
        commit=SOURCE_COMMIT,
        dpn_validator_release_commit=DPN_COMMIT,
        cargo_lock_sha256=boi.FIXED_CARGO_LOCK_SHA256,
        workspace_source_manifest_sha256=SOURCE_MANIFEST,
    )
    replay_ledger = fixture.root.parent / "unused-ledger.json"
    if not replay_ledger.exists():
        replay_ledger.write_bytes(admission.canonical_replay_ledger_bytes([]))

    def verify_signature(
        manifest: Path,
        _signature: Path,
        _public_key: Path,
        fingerprint: str,
        _verifier: Path,
        _verifier_sha256: str,
    ) -> dict[str, object]:
        if reject_signature:
            raise boi.ReleaseManifestSignatureError("injected invalid signature")
        return {
            "manifest_sha256": _sha(manifest.read_bytes()),
            "signature_verified": True,
            "signer_fingerprint_sha256": fingerprint,
        }

    return boi.verify_qualified_boi_handoff(
        fixture.output.resolve(),
        candidate_archive=archive or fixture.candidate.archive,
        candidate_authority_dir=fixture.candidate.authority_dir,
        expected_source=source,
        expected_receipt_id=QUALIFICATION_ID,
        replay_ledger_path=replay_ledger,
        trusted_signing_fingerprint="a" * 64,
        trusted_qualification_public_key_path=(
            fixture.root.parent / "qualification-signing.pub"
        ),
        trusted_qualification_signing_fingerprint=QUALIFICATION_FINGERPRINT,
        expected_qualification_host_id=expected_host_id,
        expected_qualification_installation_id="boi-installation-v1",
        expected_controller_closure_digest="d" * 64,
        expected_workflow_run_id=expected_run_id,
        expected_workflow_run_attempt=2,
        release_manifest_verifier_path=fixture.root.parent / "unused-verifier",
        trusted_release_manifest_verifier_sha256="b" * 64,
        now_unix=now_unix,
        qualification_signature_verifier=verify_signature,
    )


def test_qualified_handoff_independently_binds_signed_candidate(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)
    snapshot = _verify_qualified(fixture, monkeypatch)

    assert snapshot.candidate_archive_sha256 == fixture.candidate.archive_info.sha256
    assert snapshot.candidate_release_manifest_sha256 == (
        fixture.candidate.release_manifest_sha256
    )
    assert snapshot.boi_inventory_sha256 == _sha(
        (fixture.output / boi.OUTPUT_INVENTORY).read_bytes()
    )
    assert snapshot.qualification_signer_fingerprint_sha256 == (
        QUALIFICATION_FINGERPRINT
    )


def test_qualified_handoff_rejects_invalid_external_signature(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)

    with pytest.raises(boi.BoiHandoffError, match="signature is invalid"):
        _verify_qualified(fixture, monkeypatch, reject_signature=True)


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("expected_host_id", "different-host", "controller identity differs"),
        ("expected_run_id", 102, "workflow identity differs"),
        (
            "now_unix",
            QUALIFICATION_NOW + boi.QUALIFICATION_LIFETIME_SECONDS + 1,
            "expired or outside policy",
        ),
    ],
)
def test_qualified_handoff_rejects_wrong_policy_or_expiry(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    field: str,
    value: object,
    message: str,
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)

    with pytest.raises(boi.BoiHandoffError, match=message):
        _verify_qualified(fixture, monkeypatch, **{field: value})


def test_qualified_handoff_rejects_signed_receipt_replay(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    result = _assemble(fixture)
    (fixture.root.parent / "unused-ledger.json").write_bytes(
        admission.canonical_replay_ledger_bytes(
            [str(result["qualification_receipt_id"])]
        )
    )

    with pytest.raises(boi.BoiHandoffError, match="already consumed"):
        _verify_qualified(fixture, monkeypatch)


def test_unsigned_self_hashed_qualification_tree_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)
    signature = fixture.output / boi.QUALIFICATION_SIGNATURE_PATH
    transport = fixture.output / boi.QUALIFIED_HANDOFF_MANIFEST
    signature.parent.chmod(0o755)
    fixture.output.chmod(0o755)
    signature.unlink()
    value = json.loads(transport.read_bytes())
    value["files"] = [
        row
        for row in value["files"]
        if row["path"] != boi.QUALIFICATION_SIGNATURE_PATH
    ]
    transport.chmod(0o644)
    transport.write_bytes(
        (
            json.dumps(
                value,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    )
    transport.chmod(0o444)
    signature.parent.chmod(0o555)
    fixture.output.chmod(0o555)

    with pytest.raises(boi.BoiHandoffError):
        _verify_qualified(fixture, monkeypatch)


@pytest.mark.parametrize("attack", ["wrong", "missing", "reordered"])
def test_qualified_transport_inventory_attacks_fail_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, attack: str
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)
    manifest = fixture.output / boi.QUALIFIED_HANDOFF_MANIFEST
    if attack == "missing":
        fixture.output.chmod(0o755)
        (fixture.output / boi.OUTPUT_INVENTORY).unlink()
        fixture.output.chmod(0o555)
    else:
        value = json.loads(manifest.read_bytes())
        if attack == "wrong":
            value["files"][0]["sha256"] = "0" * 64
        else:
            value["files"].reverse()
        manifest.chmod(0o644)
        manifest.write_bytes(
            (
                json.dumps(
                    value,
                    ensure_ascii=True,
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii")
        )
        manifest.chmod(0o444)

    with pytest.raises(boi.BoiHandoffError):
        _verify_qualified(fixture, monkeypatch)


def test_qualified_handoff_rejects_a_different_candidate_archive(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)
    other = tmp_path / "other" / fixture.candidate.archive.name
    other.parent.mkdir()
    other.write_bytes(b"different signed candidate\n")

    with pytest.raises(boi.BoiHandoffError, match="different signed candidate"):
        _verify_qualified(fixture, monkeypatch, archive=other)


def test_qualified_handoff_toctou_recheck_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture(tmp_path)
    _assemble(fixture)
    snapshot = _verify_qualified(fixture, monkeypatch)
    target = fixture.output / boi.QUALIFICATION_RECEIPT_PATH
    target.chmod(0o644)
    target.write_bytes(target.read_bytes() + b"stale")
    target.chmod(0o444)

    with pytest.raises(boi.BoiHandoffError, match="changed"):
        boi.recheck_qualified_boi_handoff(snapshot)
