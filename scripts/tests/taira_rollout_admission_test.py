from __future__ import annotations

import argparse
import hashlib
import io
import json
import tarfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import pytest

from scripts import build_taira_rollout_candidate as candidate_builder
from scripts import release_artifact_contract as contract
from scripts import release_manifest_signing as signing
from scripts import taira_release_authority as linux_authority
from scripts import taira_rollout_admission as admission


ROOT = Path(__file__).resolve().parents[2]
EXACT12 = ROOT / "fixtures" / "privacy" / "exact12_v1.tsv"
COMMIT = "1" * 40
WORKSPACE_SHA = "2" * 64
CARGO_LOCK = b"first-release-lock\n"
CARGO_LOCK_SHA = hashlib.sha256(CARGO_LOCK).hexdigest()
SOURCE = admission.SourceIdentity(COMMIT, CARGO_LOCK_SHA, WORKSPACE_SHA)
TEST_PUBLIC_KEY = bytes.fromhex(
    "2152f8d19b791d24453242e15f2eab6cb7cffa7b6a5ed30097960e069881db12"
)
TEST_FINGERPRINT = hashlib.sha256(TEST_PUBLIC_KEY).hexdigest()
SUBSTITUTE_PUBLIC_KEY = bytes.fromhex(
    "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00"
)
SOURCE_DATE_EPOCH = 1_700_000_000


ReceiptMutation = Callable[[dict[str, object]], None]
ManifestMutation = Callable[[dict[str, object]], None]


@dataclass(frozen=True)
class Candidate:
    archive: Path
    authority_dir: Path
    replay_ledger: Path
    verifier: Path
    verifier_sha256: str
    receipt_id: str
    now_unix: int


def _signature(payload: bytes, key: bytes) -> bytes:
    encoded_r = bytearray(hashlib.sha256(b"r\0" + key + payload).digest())
    encoded_r[-1] &= 0x7F
    scalar = (
        int.from_bytes(hashlib.sha256(b"s\0" + key + payload).digest(), "little")
        % signing.ED25519_SCALAR_ORDER
    )
    if scalar == 0:
        scalar = 1
    return bytes(encoded_r) + scalar.to_bytes(32, "little")


def _write_fake_native_verifier(path: Path) -> str:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"ORDER = {signing.ED25519_SCALAR_ORDER}\n"
        "args = sys.argv[1:]\n"
        "if len(args) != 9 or args[0] != 'release-manifest':\n"
        "    raise SystemExit(3)\n"
        "options = dict(zip(args[1::2], args[2::2]))\n"
        "if set(options) != {'--manifest', '--public-key', "
        "'--public-key-fingerprint', '--signature'}:\n"
        "    raise SystemExit(4)\n"
        "payload = Path(options['--manifest']).read_bytes()\n"
        "key = Path(options['--public-key']).read_bytes()\n"
        "encoded_r = bytearray(hashlib.sha256(b'r\\0' + key + payload).digest())\n"
        "encoded_r[-1] &= 0x7f\n"
        "scalar = int.from_bytes(hashlib.sha256(b's\\0' + key + payload).digest(), "
        "'little') % ORDER\n"
        "if scalar == 0:\n"
        "    scalar = 1\n"
        "expected = bytes(encoded_r) + scalar.to_bytes(32, 'little')\n"
        "if hashlib.sha256(key).hexdigest() != "
        "options['--public-key-fingerprint']:\n"
        "    raise SystemExit(5)\n"
        "if Path(options['--signature']).read_bytes() != expected:\n"
        "    raise SystemExit(6)\n",
        encoding="utf-8",
    )
    path.chmod(0o700)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_fake_external_signer(path: Path) -> None:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"KEY = bytes.fromhex('{TEST_PUBLIC_KEY.hex()}')\n"
        f"ORDER = {signing.ED25519_SCALAR_ORDER}\n"
        "if len(sys.argv) != 3:\n"
        "    raise SystemExit(2)\n"
        "payload = Path(sys.argv[1]).read_bytes()\n"
        "encoded_r = bytearray(hashlib.sha256(b'r\\0' + KEY + payload).digest())\n"
        "encoded_r[-1] &= 0x7f\n"
        "scalar = int.from_bytes(hashlib.sha256(b's\\0' + KEY + payload).digest(), 'little') % ORDER\n"
        "if scalar == 0:\n"
        "    scalar = 1\n"
        "with Path(sys.argv[2]).open('xb') as output:\n"
        "    output.write(bytes(encoded_r) + scalar.to_bytes(32, 'little'))\n",
        encoding="utf-8",
    )
    path.chmod(0o700)


def _canonical_manifest(
    *,
    version: str,
    commit: str,
    os_tag: str,
    arch: str,
    artifacts: list[dict[str, object]],
) -> bytes:
    manifest = contract.validate_release_manifest(
        {
            "arch": arch,
            "artifacts": artifacts,
            "built_at": contract.format_source_date_epoch(SOURCE_DATE_EPOCH),
            "commit": commit,
            "os": os_tag,
            "schema": contract.RELEASE_MANIFEST_SCHEMA,
            "schema_version": contract.RELEASE_MANIFEST_SCHEMA_VERSION,
            "source_date_epoch": SOURCE_DATE_EPOCH,
            "version": version,
        }
    )
    return contract.canonical_json_bytes(manifest)


def _release_row(
    path: str,
    payload: bytes,
    *,
    target: str,
    kind: str,
    fmt: str,
) -> dict[str, object]:
    return {
        "format": fmt,
        "kind": kind,
        "path": path,
        "profile": "iroha3",
        "sha256": hashlib.sha256(payload).hexdigest(),
        "size": len(payload),
        "target": target,
    }


def _evidence_root(root: Path) -> Path:
    evidence = root / "linux-evidence"
    for index, relative in enumerate(linux_authority.EVIDENCE_PATHS.values()):
        path = evidence / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if relative == linux_authority.EVIDENCE_PATHS["exact12_matrix"]:
            payload = EXACT12.read_bytes()
        elif relative == linux_authority.EVIDENCE_PATHS["workspace_source_manifest"]:
            payload = f"{WORKSPACE_SHA}\n".encode("ascii")
        elif relative == linux_authority.EVIDENCE_PATHS["cargo_lock"]:
            payload = CARGO_LOCK
        else:
            payload = f"native-evidence-{index}\n".encode()
        path.write_bytes(payload)
    return evidence


def _linux_archive(root: Path, evidence: Path) -> Path:
    path = root / "taira-rollout-linux-arm64.tar.gz"
    prefix = path.name.removesuffix(".tar.gz")
    with tarfile.open(path, mode="w:gz") as archive:
        for relative in linux_authority.EVIDENCE_PATHS.values():
            archive.add(
                evidence / relative,
                arcname=f"{prefix}/{relative}",
                recursive=False,
            )
    return path


def _authority_payload(
    evidence: Path,
    archive: Path,
    verifier_sha256: str,
) -> dict[str, object]:
    return linux_authority.build_authority(
        argparse.Namespace(
            archive=str(archive),
            commit=COMMIT,
            evidence_root=str(evidence),
            image_id=None,
            image_manifest_digest=None,
            image_tag=[],
            native_verifier_sha256=verifier_sha256,
            signing_fingerprint=TEST_FINGERPRINT,
        )
    )


def _receipt(now_unix: int, mutation: ReceiptMutation | None) -> dict[str, object]:
    validator_sha = "3" * 64
    supervisor_sha = "7" * 64
    reset_manifest_sha = "8" * 64
    config_digests = {
        f"taira-validator-{number}": hashlib.sha256(
            f"validator-config-{number}".encode("ascii")
        ).hexdigest()
        for number in range(1, 5)
    }
    start_hash = "4" * 64
    end_hash = "5" * 64
    body: dict[str, object] = {
        "end": {"block_hash": end_hash, "height": 102},
        "expires_at_unix": now_unix + 900,
        "issued_at_unix": now_unix - 30,
        "peer_count": 4,
        "peers": [
            {
                "final_block_hash": end_hash,
                "final_height": 102,
                "label": f"taira-validator-{number}",
                "number": number,
                "restart_proof": "passed",
                "source_commit": COMMIT,
                "validator_binary_sha256": validator_sha,
                "validator_config_sha256": config_digests[f"taira-validator-{number}"],
            }
            for number in range(1, 5)
        ],
        "platform": {"arch": "arm64", "os": "macos"},
        "reset_manifest_sha256": reset_manifest_sha,
        "restart_generation": "6" * 64,
        "schema": admission.MACOS_RECEIPT_SCHEMA,
        "schema_version": admission.MACOS_RECEIPT_SCHEMA_VERSION,
        "source": SOURCE.as_dict(),
        "start": {"block_hash": start_hash, "height": 101},
        "supervisor_sha256": supervisor_sha,
        "validator_binary_sha256": validator_sha,
        "validator_config_sha256": config_digests,
    }
    if mutation is not None:
        mutation(body)
    receipt_id = admission.compute_macos_receipt_id(body)
    return {**body, "receipt_id": receipt_id}


def _tar_bytes(
    destination: Path,
    files: dict[str, bytes],
    *,
    attack: str | None,
) -> None:
    prefix = destination.name.removesuffix(".tar.gz")
    with tarfile.open(destination, mode="w:gz") as archive:
        for relative, payload in sorted(files.items()):
            info = tarfile.TarInfo(f"{prefix}/{relative}")
            info.size = len(payload)
            info.mode = 0o600
            archive.addfile(info, io.BytesIO(payload))
        if attack == "traversal":
            info = tarfile.TarInfo(f"{prefix}/../escape")
            info.size = 1
            archive.addfile(info, io.BytesIO(b"x"))
        elif attack == "symlink":
            info = tarfile.TarInfo(f"{prefix}/escape-link")
            info.type = tarfile.SYMTYPE
            info.linkname = "../../escape"
            archive.addfile(info)
        elif attack == "duplicate":
            payload = files[admission.MACOS_RECEIPT_PATH]
            info = tarfile.TarInfo(f"{prefix}/{admission.MACOS_RECEIPT_PATH}")
            info.size = len(payload)
            archive.addfile(info, io.BytesIO(payload))
        elif attack == "extra-directory":
            info = tarfile.TarInfo(f"{prefix}/unexpected/")
            info.type = tarfile.DIRTYPE
            archive.addfile(info)
        elif attack is not None:
            raise AssertionError(f"unsupported archive attack: {attack}")


def _build_candidate(
    tmp_path: Path,
    *,
    receipt_mutation: ReceiptMutation | None = None,
    admission_mutation: ManifestMutation | None = None,
    nested_authority_mutation: ManifestMutation | None = None,
    nested_manifest_mutation: ManifestMutation | None = None,
    outer_manifest_mutation: ManifestMutation | None = None,
    outer_key: bytes = TEST_PUBLIC_KEY,
    nested_key: bytes = TEST_PUBLIC_KEY,
    receipt_id_override: str | None = None,
    archive_attack: str | None = None,
    add_extra_file: bool = False,
    replayed: bool = False,
) -> Candidate:
    now_unix = int(time.time())
    root = tmp_path / "candidate-inputs"
    root.mkdir(mode=0o700)
    verifier = root / "trusted-sorafs-validate"
    verifier_sha256 = _write_fake_native_verifier(verifier)
    evidence = _evidence_root(root)
    linux_archive = _linux_archive(root, evidence)
    authority_payload = _authority_payload(evidence, linux_archive, verifier_sha256)
    if nested_authority_mutation is not None:
        nested_authority_mutation(authority_payload)

    nested_artifacts: dict[str, bytes] = {
        "release_artifact_contract.py": b"trusted contract helper\n",
        "sorafs-validate": verifier.read_bytes(),
        "taira-exact12-release-authority-v1.json": (
            contract.canonical_json_bytes(authority_payload)
        ),
        "taira_release_authority.py": b"trusted exact12 helper\n",
    }
    nested_checksums = "".join(
        f"{hashlib.sha256(payload).hexdigest()}  {name}\n"
        for name, payload in sorted(nested_artifacts.items())
    ).encode("ascii")
    nested_rows = []
    for name, payload in sorted(nested_artifacts.items()):
        profile, target, kind, fmt = admission._artifact_descriptor(name)
        assert profile == "iroha3"
        nested_rows.append(
            _release_row(
                name,
                payload,
                target=target,
                kind=kind,
                fmt=fmt,
            )
        )
    nested_manifest_object = json.loads(
        _canonical_manifest(
            version=f"taira-{WORKSPACE_SHA[:16]}",
            commit=COMMIT,
            os_tag="linux",
            arch="aarch64",
            artifacts=nested_rows,
        )
    )
    if nested_manifest_mutation is not None:
        nested_manifest_mutation(nested_manifest_object)
    nested_manifest = contract.canonical_json_bytes(nested_manifest_object)
    nested_signature = _signature(nested_manifest, nested_key)

    receipt = _receipt(now_unix, receipt_mutation)
    if receipt_id_override is not None:
        receipt["receipt_id"] = receipt_id_override
    receipt_payload = contract.canonical_json_bytes(receipt)
    receipt_id = str(receipt["receipt_id"])
    outer_files: dict[str, bytes] = {
        "linux/authority/artifacts/SHA256SUMS": nested_checksums,
        **{
            f"linux/authority/artifacts/{name}": payload
            for name, payload in nested_artifacts.items()
        },
        "linux/authority/release_manifest.json": nested_manifest,
        "linux/authority/release_manifest.json.pub": nested_key,
        "linux/authority/release_manifest.json.sig": nested_signature,
        f"linux/{linux_archive.name}": linux_archive.read_bytes(),
        admission.MACOS_RECEIPT_PATH: receipt_payload,
    }
    if add_extra_file:
        outer_files["unexpected.txt"] = b"unexpected\n"
    inventory = [
        {
            "path": path,
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size": len(payload),
        }
        for path, payload in sorted(outer_files.items())
    ]
    admission_manifest: dict[str, object] = {
        "inventory": inventory,
        "linux_arm64": {
            "arch": "aarch64",
            "archive_path": f"linux/{linux_archive.name}",
            "authority_directory": admission.LINUX_AUTHORITY_DIRECTORY,
            "authority_manifest_sha256": hashlib.sha256(nested_manifest).hexdigest(),
            "authority_native_verifier_sha256": verifier_sha256,
            "os": "linux",
        },
        "macos_arm64": {
            "arch": "arm64",
            "os": "macos",
            "receipt_id": receipt_id,
            "receipt_path": admission.MACOS_RECEIPT_PATH,
        },
        "schema": admission.ADMISSION_SCHEMA,
        "schema_version": admission.ADMISSION_SCHEMA_VERSION,
        "source": SOURCE.as_dict(),
        "trust": {
            "release_manifest_verifier_sha256": verifier_sha256,
            "signer_fingerprint_sha256": TEST_FINGERPRINT,
        },
    }
    if admission_mutation is not None:
        admission_mutation(admission_manifest)
    outer_files[admission.ADMISSION_MANIFEST_PATH] = contract.canonical_json_bytes(
        admission_manifest
    )

    archive = tmp_path / "taira-dual-target-test.tar.gz"
    _tar_bytes(archive, outer_files, attack=archive_attack)
    archive_payload = archive.read_bytes()
    outer_rows = [
        _release_row(
            archive.name,
            archive_payload,
            target="taira-rollout-admission-v1",
            kind="reference-validator",
            fmt="tar.gz",
        )
    ]
    outer_manifest_object = json.loads(
        _canonical_manifest(
            version=f"taira-admission-{WORKSPACE_SHA[:16]}",
            commit=COMMIT,
            os_tag="macos",
            arch="arm64",
            artifacts=outer_rows,
        )
    )
    if outer_manifest_mutation is not None:
        outer_manifest_mutation(outer_manifest_object)
    outer_manifest = contract.canonical_json_bytes(outer_manifest_object)
    authority_dir = tmp_path / "taira-dual-target-test.authority"
    authority_dir.mkdir(mode=0o700)
    (authority_dir / "release_manifest.json").write_bytes(outer_manifest)
    (authority_dir / "release_manifest.json.pub").write_bytes(outer_key)
    (authority_dir / "release_manifest.json.sig").write_bytes(
        _signature(outer_manifest, outer_key)
    )

    replay_ledger = tmp_path / "replay-ledger.json"
    replay_ledger.write_bytes(
        admission.canonical_replay_ledger_bytes([receipt_id] if replayed else [])
    )
    return Candidate(
        archive=archive,
        authority_dir=authority_dir,
        replay_ledger=replay_ledger,
        verifier=verifier,
        verifier_sha256=verifier_sha256,
        receipt_id=receipt_id,
        now_unix=now_unix,
    )


def _verify(candidate: Candidate) -> dict[str, object]:
    return admission.verify_admission(
        archive_path=candidate.archive,
        authority_dir=candidate.authority_dir,
        expected_source=SOURCE,
        expected_receipt_id=candidate.receipt_id,
        replay_ledger_path=candidate.replay_ledger,
        trusted_signing_fingerprint=TEST_FINGERPRINT,
        release_manifest_verifier_path=candidate.verifier,
        trusted_release_manifest_verifier_sha256=candidate.verifier_sha256,
        now_unix=candidate.now_unix,
    )


def _assembler_inputs(base: Candidate, root: Path) -> tuple[Path, Path, Path]:
    linux_authority_dir = root / "linux-authority"
    linux_authority_dir.mkdir(parents=True, mode=0o700)
    receipt_path = root / "macos-receipt.json"
    with tarfile.open(base.archive, mode="r:gz") as archive:
        prefix = base.archive.name.removesuffix(".tar.gz")
        members = {member.name: member for member in archive.getmembers()}
        linux_names = [
            name
            for name in members
            if name.startswith(f"{prefix}/linux/") and name.endswith(".tar.gz")
        ]
        assert len(linux_names) == 1
        linux_member = archive.extractfile(members[linux_names[0]])
        assert linux_member is not None
        linux_archive = root / Path(linux_names[0]).name
        linux_archive.write_bytes(linux_member.read())
        for relative in admission.LINUX_AUTHORITY_FILES:
            member = archive.extractfile(
                members[f"{prefix}/{admission.LINUX_AUTHORITY_DIRECTORY}/{relative}"]
            )
            assert member is not None
            destination = linux_authority_dir / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(member.read())
        receipt_member = archive.extractfile(
            members[f"{prefix}/{admission.MACOS_RECEIPT_PATH}"]
        )
        assert receipt_member is not None
        receipt_path.write_bytes(receipt_member.read())
    return linux_archive, linux_authority_dir, receipt_path


def test_valid_dual_target_archive_is_verified_without_deployment(
    tmp_path: Path,
) -> None:
    candidate = _build_candidate(tmp_path)

    result = _verify(candidate)

    assert result["verified"] is True
    assert result["deployment_performed"] is False
    assert result["peer_count"] == 4
    assert result["receipt_id"] == candidate.receipt_id
    assert result["source"] == SOURCE.as_dict()
    assert result["reset_manifest_sha256"] == "8" * 64
    assert result["supervisor_sha256"] == "7" * 64
    assert result["validator_binary_sha256"] == "3" * 64
    assert set(result["validator_config_sha256"]) == {
        f"taira-validator-{number}" for number in range(1, 5)
    }


def test_candidate_builder_reconstructs_the_same_admitted_archive_deterministically(
    tmp_path: Path,
) -> None:
    base_root = tmp_path / "base"
    base_root.mkdir()
    base = _build_candidate(base_root)
    input_root = tmp_path / "assembler-inputs"
    input_root.mkdir()
    linux_archive, linux_authority_dir, receipt = _assembler_inputs(base, input_root)
    public_key = input_root / "release.pub"
    public_key.write_bytes(TEST_PUBLIC_KEY)
    signer = input_root / "external-signer"
    _write_fake_external_signer(signer)

    common = {
        "cargo_lock_sha256": CARGO_LOCK_SHA,
        "expected_receipt_id": base.receipt_id,
        "external_signer": signer,
        "linux_archive": linux_archive,
        "linux_authority_dir": linux_authority_dir,
        "macos_receipt": receipt,
        "now_unix": base.now_unix,
        "release_manifest_verifier": base.verifier,
        "signing_public_key": public_key,
        "source_commit": COMMIT,
        "source_date_epoch": SOURCE_DATE_EPOCH,
        "trusted_release_manifest_verifier_sha256": base.verifier_sha256,
        "trusted_signing_fingerprint": TEST_FINGERPRINT,
        "workspace_source_manifest_sha256": WORKSPACE_SHA,
    }
    first_args = argparse.Namespace(
        **common, output_directory=tmp_path / "assembled-one"
    )
    second_args = argparse.Namespace(
        **common, output_directory=tmp_path / "assembled-two"
    )

    first = candidate_builder.assemble_candidate(first_args)
    second = candidate_builder.assemble_candidate(second_args)

    first_archive = Path(str(first["archive"]))
    second_archive = Path(str(second["archive"]))
    assert first_archive.read_bytes() == second_archive.read_bytes()
    assert first["verified"] is True
    assert first["receipt_id"] == base.receipt_id
    assert second["archive_sha256"] == first["archive_sha256"]
    first_authority = Path(str(first["authority_dir"]))
    second_authority = Path(str(second["authority_dir"]))
    for name in admission.FINAL_AUTHORITY_FILES:
        assert (first_authority / name).read_bytes() == (
            second_authority / name
        ).read_bytes()


@pytest.mark.parametrize("field", ["schema", "source", "trust"])
def test_admission_manifest_rejects_missing_fields(tmp_path: Path, field: str) -> None:
    candidate = _build_candidate(
        tmp_path,
        admission_mutation=lambda manifest: manifest.pop(field),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="fields are not exact"
    ):
        _verify(candidate)


def test_admission_manifest_rejects_extra_field(tmp_path: Path) -> None:
    candidate = _build_candidate(
        tmp_path,
        admission_mutation=lambda manifest: manifest.__setitem__("legacy", True),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="fields are not exact"
    ):
        _verify(candidate)


@pytest.mark.parametrize(
    "mutation, message",
    [
        (
            lambda receipt: receipt["platform"].__setitem__("arch", "x86_64"),
            "exactly macos/arm64",
        ),
        (
            lambda receipt: receipt.__setitem__("peer_count", 3),
            "exactly four peers",
        ),
        (
            lambda receipt: receipt["peers"].pop(),
            "exactly four peer rows",
        ),
        (
            lambda receipt: receipt.__setitem__("legacy", True),
            "fields are not exact",
        ),
        (
            lambda receipt: receipt["source"].__setitem__(
                "cargo_lock_sha256", "9" * 64
            ),
            "source identity differs",
        ),
        (
            lambda receipt: receipt.pop("reset_manifest_sha256"),
            "fields are not exact",
        ),
        (
            lambda receipt: receipt["validator_config_sha256"].pop("taira-validator-4"),
            "exact four validator config digests",
        ),
        (
            lambda receipt: receipt["peers"][0].__setitem__(
                "validator_config_sha256", "9" * 64
            ),
            "exact validator config",
        ),
        (
            lambda receipt: receipt.__setitem__("supervisor_sha256", "NOT-A-DIGEST"),
            "supervisor digest",
        ),
    ],
    ids=(
        "wrong-arch",
        "declared-three",
        "three-rows",
        "extra-field",
        "source-drift",
        "missing-reset-manifest-binding",
        "missing-config-binding",
        "peer-config-substitution",
        "invalid-supervisor-binding",
    ),
)
def test_macos_receipt_rejects_malformed_or_cross_source_rows(
    tmp_path: Path,
    mutation: ReceiptMutation,
    message: str,
) -> None:
    candidate = _build_candidate(tmp_path, receipt_mutation=mutation)

    with pytest.raises(admission.TairaRolloutAdmissionError, match=message):
        _verify(candidate)


def test_stale_receipt_is_rejected(tmp_path: Path) -> None:
    def stale(receipt: dict[str, object]) -> None:
        receipt["issued_at_unix"] = 100
        receipt["expires_at_unix"] = 200

    candidate = _build_candidate(tmp_path, receipt_mutation=stale)

    with pytest.raises(admission.TairaRolloutAdmissionError, match="stale"):
        _verify(candidate)


def test_receipt_id_must_be_derived_from_the_exact_body(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path, receipt_id_override="0" * 64)

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="canonical receipt body"
    ):
        _verify(candidate)


def test_boolean_receipt_schema_version_is_not_integer_one(tmp_path: Path) -> None:
    candidate = _build_candidate(
        tmp_path,
        receipt_mutation=lambda receipt: receipt.__setitem__("schema_version", True),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="must be an integer"
    ):
        _verify(candidate)


def test_replayed_receipt_is_rejected_from_read_only_ledger(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path, replayed=True)

    before = candidate.replay_ledger.read_bytes()
    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="already been consumed"
    ):
        _verify(candidate)
    assert candidate.replay_ledger.read_bytes() == before


@pytest.mark.parametrize(
    "receipt_ids",
    [
        ["b" * 64, "a" * 64],
        ["a" * 64, "a" * 64],
        ["A" * 64],
    ],
    ids=("unsorted", "duplicate", "noncanonical-digest"),
)
def test_replay_ledger_encoder_rejects_noncanonical_ids(
    receipt_ids: list[str],
) -> None:
    with pytest.raises(admission.TairaRolloutAdmissionError):
        admission.canonical_replay_ledger_bytes(receipt_ids)


@pytest.mark.parametrize("which", ["outer", "nested"])
def test_signer_key_substitution_is_rejected(tmp_path: Path, which: str) -> None:
    candidate = _build_candidate(
        tmp_path,
        outer_key=(SUBSTITUTE_PUBLIC_KEY if which == "outer" else TEST_PUBLIC_KEY),
        nested_key=(SUBSTITUTE_PUBLIC_KEY if which == "nested" else TEST_PUBLIC_KEY),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="public key does not match the reviewed fingerprint",
    ):
        _verify(candidate)


def test_wrong_native_verifier_pin_is_rejected(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path)

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="native release-manifest verifier does not match",
    ):
        admission.verify_admission(
            archive_path=candidate.archive,
            authority_dir=candidate.authority_dir,
            expected_source=SOURCE,
            expected_receipt_id=candidate.receipt_id,
            replay_ledger_path=candidate.replay_ledger,
            trusted_signing_fingerprint=TEST_FINGERPRINT,
            release_manifest_verifier_path=candidate.verifier,
            trusted_release_manifest_verifier_sha256="0" * 64,
            now_unix=candidate.now_unix,
        )


def test_nested_authority_mutation_is_rejected_by_existing_verifier(
    tmp_path: Path,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        nested_authority_mutation=lambda payload: payload.__setitem__(
            "substituted", True
        ),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="existing exact-12 release-authority verification rejected",
    ):
        _verify(candidate)


def test_nested_linux_wrong_architecture_is_rejected(tmp_path: Path) -> None:
    candidate = _build_candidate(
        tmp_path,
        nested_manifest_mutation=lambda manifest: manifest.__setitem__(
            "arch", "x86_64"
        ),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="wrong source or platform"
    ):
        _verify(candidate)


def test_final_inventory_digest_mismatch_is_rejected(tmp_path: Path) -> None:
    def mutate(manifest: dict[str, object]) -> None:
        inventory = manifest["inventory"]
        assert isinstance(inventory, list)
        inventory[0]["sha256"] = "0" * 64

    candidate = _build_candidate(tmp_path, admission_mutation=mutate)

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="digest/size mismatch"
    ):
        _verify(candidate)


def test_final_archive_byte_mutation_breaks_its_signed_subject(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path)
    with candidate.archive.open("ab") as stream:
        stream.write(b"substituted-after-signing\n")

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="exact macOS archive"
    ):
        _verify(candidate)


def test_final_archive_extra_file_is_rejected(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path, add_extra_file=True)

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="exact first-release inventory",
    ):
        _verify(candidate)


@pytest.mark.parametrize(
    "attack", ["traversal", "symlink", "duplicate", "extra-directory"]
)
def test_final_archive_member_attacks_fail_closed(tmp_path: Path, attack: str) -> None:
    candidate = _build_candidate(tmp_path, archive_attack=attack)

    with pytest.raises(admission.TairaRolloutAdmissionError):
        _verify(candidate)


def test_outer_signed_manifest_wrong_platform_is_rejected(tmp_path: Path) -> None:
    candidate = _build_candidate(
        tmp_path,
        outer_manifest_mutation=lambda manifest: manifest.__setitem__("arch", "x86_64"),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="exact macOS archive"
    ):
        _verify(candidate)


def test_outer_signed_manifest_rejects_image_subject_fallback(tmp_path: Path) -> None:
    def image_subject(manifest: dict[str, object]) -> None:
        artifacts = manifest["artifacts"]
        assert isinstance(artifacts, list)
        artifacts[0]["kind"] = "image"
        artifacts[0]["format"] = "oci-archive"
        artifacts[0]["path"] = "taira-dual-target-test.tar"

    candidate = _build_candidate(tmp_path, outer_manifest_mutation=image_subject)

    with pytest.raises(
        admission.TairaRolloutAdmissionError, match="exact macOS archive"
    ):
        _verify(candidate)


def test_cli_emits_only_verification_summary(tmp_path: Path, capsys) -> None:
    candidate = _build_candidate(tmp_path)

    result = admission.main(
        [
            "verify",
            "--archive",
            str(candidate.archive),
            "--authority-dir",
            str(candidate.authority_dir),
            "--expected-source-commit",
            COMMIT,
            "--expected-cargo-lock-sha256",
            CARGO_LOCK_SHA,
            "--expected-workspace-source-manifest-sha256",
            WORKSPACE_SHA,
            "--expected-receipt-id",
            candidate.receipt_id,
            "--replay-ledger",
            str(candidate.replay_ledger),
            "--trusted-signing-fingerprint",
            TEST_FINGERPRINT,
            "--release-manifest-verifier",
            str(candidate.verifier),
            "--trusted-release-manifest-verifier-sha256",
            candidate.verifier_sha256,
        ]
    )

    captured = capsys.readouterr()
    assert result == 0, captured.err
    summary = json.loads(captured.out)
    assert summary["verified"] is True
    assert summary["deployment_performed"] is False
