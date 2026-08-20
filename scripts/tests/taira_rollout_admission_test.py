"""Post-provisioning archive validation tests.

Production entry points are unconditionally closed by the controller-origin
authority barrier.  This module replaces only that barrier so the independent
archive, signature, mutation, and TOCTOU checks behind the trust boundary keep
their focused coverage.  The production barrier itself is exercised without
replacement in ``taira_privacy_protocol_provenance_gate_test.py``.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import tarfile
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import pytest

from scripts import build_taira_rollout_candidate as candidate_builder
from scripts import release_artifact_contract as contract
from scripts import release_manifest_signing as signing
from scripts import render_taira_validator_bundle as receipt_renderer
from scripts import seal_taira_release_controllers as controller_seal
from scripts import taira_privacy_protocol_receipt as privacy_evidence
from scripts import taira_release_authority as linux_authority
from scripts import taira_rollout_admission as admission
from scripts.tests.taira_privacy_protocol_receipt_test import (
    DRIVER_BYTES,
    build_valid_evidence,
)

ROOT = Path(__file__).resolve().parents[2]
EXACT12 = ROOT / "fixtures" / "privacy" / "exact12_v1.tsv"
COMMIT = "1" * 40
DPN_COMMIT = "d" * 40
WORKSPACE_SHA = "2" * 64
CARGO_LOCK = b"first-release-lock\n"
CARGO_LOCK_SHA = hashlib.sha256(CARGO_LOCK).hexdigest()
SOURCE = admission.SourceIdentity(COMMIT, DPN_COMMIT, CARGO_LOCK_SHA, WORKSPACE_SHA)
TEST_PUBLIC_KEY = bytes.fromhex(
    "2152f8d19b791d24453242e15f2eab6cb7cffa7b6a5ed30097960e069881db12"
)
TEST_FINGERPRINT = hashlib.sha256(TEST_PUBLIC_KEY).hexdigest()
SUBSTITUTE_PUBLIC_KEY = bytes.fromhex(
    "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00"
)
SOURCE_DATE_EPOCH = 1_700_000_000


def _receipt_signers() -> dict[str, dict[str, object]]:
    result: dict[str, dict[str, object]] = {}
    for number, slug in enumerate(admission.SLUGS, start=1):
        private_payload = number.to_bytes(32, "big")
        public_payload = receipt_renderer._secp256k1_public_payload(private_payload)
        public_key = receipt_renderer.RECEIPT_PUBLIC_KEY_PREFIX + (
            public_payload.hex().upper()
        )
        result[slug] = {
            "node_id": receipt_renderer.receipt_node_id(public_key),
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": public_payload.hex(),
            },
        }
    return result


@pytest.fixture(autouse=True)
def _exercise_checks_behind_unprovisioned_authority_barrier(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client = linux_authority.taira_authority_client
    records: dict[
        str,
        tuple[
            bytes,
            tuple[dict[str, object], ...],
            dict[str, object],
            dict[str, object],
        ],
    ] = {}

    def artifact_manifest(
        artifacts: tuple[client.Artifact, ...],
    ) -> tuple[dict[str, object], ...]:
        return tuple(
            {
                "name": artifact.name,
                "ordinal": ordinal,
                "sha256": hashlib.sha256(artifact.path.read_bytes()).hexdigest(),
                "size": artifact.path.stat().st_size,
            }
            for ordinal, artifact in enumerate(artifacts)
        )

    def authorize(
        role: str,
        subject: dict[str, object],
        *,
        artifacts: tuple[client.Artifact, ...] = (),
        **_kwargs: object,
    ) -> client.AuthorityResult:
        manifest = artifact_manifest(artifacts)
        run_id = client.derive_run_id(role, subject)
        operation_id = client._operation_id(role, run_id, subject, manifest)
        envelope = {
            "operation_id": operation_id,
            "role": role,
            "schema": "iroha.taira.test-native-evidence-envelope.v1",
        }
        receipt = {
            "operation_id": operation_id,
            "role": role,
            "schema": "iroha.taira.test-native-evidence-receipt.v1",
        }
        records[operation_id] = (
            client.canonical_json_bytes(subject),
            manifest,
            envelope,
            receipt,
        )
        return client.AuthorityResult(
            role=role,
            operation_id=operation_id,
            run_id=run_id,
            status="authorized",
            authority_envelope=envelope,
            durable_receipt=receipt,
            artifact_manifest=manifest,
        )

    def verify_receipt(
        role: str,
        subject: dict[str, object],
        *,
        authority_envelope: dict[str, object],
        durable_receipt: dict[str, object],
        artifacts: tuple[client.Artifact, ...] = (),
        **_kwargs: object,
    ) -> client.AuthorityResult:
        manifest = artifact_manifest(artifacts)
        run_id = client.derive_run_id(role, subject)
        operation_id = client._operation_id(role, run_id, subject, manifest)
        expected = records.get(operation_id)
        observed = (
            client.canonical_json_bytes(subject),
            manifest,
            authority_envelope,
            durable_receipt,
        )
        if expected != observed:
            raise client.TairaAuthorityClientError(
                "test native-evidence authority binding differs"
            )
        return client.AuthorityResult(
            role=role,
            operation_id=operation_id,
            run_id=run_id,
            status="valid",
            authority_envelope=authority_envelope,
            durable_receipt=durable_receipt,
            artifact_manifest=manifest,
        )

    monkeypatch.setattr(
        privacy_evidence,
        "require_controller_origin_authority_provisioned",
        lambda: None,
    )
    monkeypatch.setattr(
        client,
        "preflight",
        lambda role, *, require_signing=True: {"role": role},
    )
    monkeypatch.setattr(client, "authorize", authorize)
    monkeypatch.setattr(client, "verify_receipt", verify_receipt)


ReceiptMutation = Callable[[dict[str, object]], None]
PrivacyReceiptMutation = Callable[[dict[str, object]], None]
ManifestMutation = Callable[[dict[str, object]], None]
ArtifactMutation = Callable[[dict[str, bytes]], None]


@dataclass(frozen=True)
class Candidate:
    archive: Path
    authority_dir: Path
    boi_artifact_handoff: Path
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
    provenance = {
        "dpn_validator_release_commit": DPN_COMMIT,
        "iroha_git_head": COMMIT,
        "iroha_source_attested": True,
        "iroha_source_bundle_provenance_sha256": "a" * 64,
        "iroha_source_tree_sha256": "b" * 64,
        "iroha_tracked_patch_sha256": "c" * 64,
        "iroha_worktree_clean": False,
        "schema_version": 1,
        "validator_lock_sha256": CARGO_LOCK_SHA,
        "workspace_source_manifest_sha256": WORKSPACE_SHA,
    }
    (
        evidence
        / linux_authority.EVIDENCE_PATHS["dpn_validator_build_provenance"]
    ).write_bytes(contract.canonical_json_bytes(provenance))
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
) -> linux_authority.AuthorizedAuthority:
    return linux_authority.build_authority(
        argparse.Namespace(
            archive=str(archive),
            commit=COMMIT,
            dpn_validator_release_commit=DPN_COMMIT,
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
    receipt_signers = _receipt_signers()
    body: dict[str, object] = {
        "artifact_handoff_sha256": "2" * 64,
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
                "receipt_signer_node_id": receipt_signers[
                    f"taira-validator-{number}"
                ]["node_id"],
                "restart_proof": "passed",
                "source_commit": COMMIT,
                "validator_binary_sha256": validator_sha,
                "validator_config_sha256": config_digests[f"taira-validator-{number}"],
            }
            for number in range(1, 5)
        ],
        "platform": {"arch": "arm64", "os": "macos"},
        "receipt_signers": receipt_signers,
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


def _swap_receipt_signer_associations(receipt: dict[str, object]) -> None:
    signers = receipt["receipt_signers"]
    assert isinstance(signers, dict)
    first, second = admission.SLUGS[:2]
    signers[first], signers[second] = signers[second], signers[first]


def _privacy_protocol_evidence(
    root: Path,
    now_unix: int,
    linux_archive_sha256: str,
    validator_binary_sha256: str,
    artifact_handoff_sha256: str,
    mutation: PrivacyReceiptMutation | None,
) -> tuple[dict[str, object], dict[str, bytes]]:
    evidence_root = root / "privacy-protocol-four-peer-v2"
    build_valid_evidence(
        evidence_root,
        source=SOURCE.as_dict(),
        bindings={
            "artifact_handoff_sha256": artifact_handoff_sha256,
            "exact12_matrix_sha256": hashlib.sha256(EXACT12.read_bytes()).hexdigest(),
            "linux_release_archive_sha256": linux_archive_sha256,
            "validator_binary_sha256": validator_binary_sha256,
        },
        now_unix=now_unix - 30,
    )
    receipt_path = evidence_root / privacy_evidence.RECEIPT_NAME
    body = json.loads(receipt_path.read_bytes())
    assert isinstance(body, dict)
    body.pop("receipt_id")
    if mutation is not None:
        mutation(body)
    receipt = {
        **body,
        "receipt_id": privacy_evidence.compute_receipt_id(body),
    }
    receipt_path.write_bytes(contract.canonical_json_bytes(receipt))
    files = {
        f"{admission.PRIVACY_PROTOCOL_EVIDENCE_DIRECTORY}/{name}": (
            evidence_root / name
        ).read_bytes()
        for name in privacy_evidence.EVIDENCE_NAMES
    }
    return receipt, files


def _boi_artifact_handoff(
    root: Path,
    mutation: ManifestMutation | None = None,
) -> tuple[Path, bytes]:
    handoff = root / "privacy-v1-boi-artifacts"
    handoff.mkdir(mode=0o700)
    rows: list[dict[str, object]] = []
    for relative in admission.BOI_SOURCE_ARTIFACT_PATHS:
        if relative == "source/Cargo.lock":
            payload = CARGO_LOCK
        elif relative == "source/exact12-v1.tsv":
            payload = EXACT12.read_bytes()
        elif relative == "source/workspace-source-manifest.sha256":
            payload = f"{WORKSPACE_SHA}\n".encode("ascii")
        else:
            payload = f"BOI artifact fixture: {relative}\n".encode("ascii")
        path = handoff / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        rows.append(
            {
                "path": relative,
                "sha256": hashlib.sha256(payload).hexdigest(),
                "size": len(payload),
            }
        )
    manifest: dict[str, object] = {
        "files": rows,
        "kind": admission.BOI_SOURCE_HANDOFF_KIND,
        "schema": admission.BOI_SOURCE_HANDOFF_SCHEMA,
        "schema_version": 1,
    }
    if mutation is not None:
        mutation(manifest)
    payload = (
        json.dumps(
            manifest,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    (handoff / admission.BOI_SOURCE_HANDOFF_MANIFEST).write_bytes(payload)
    return handoff, payload


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
    privacy_receipt_mutation: PrivacyReceiptMutation | None = None,
    boi_inventory_mutation: ManifestMutation | None = None,
    admission_mutation: ManifestMutation | None = None,
    nested_authority_mutation: ManifestMutation | None = None,
    nested_artifact_mutation: ArtifactMutation | None = None,
    nested_manifest_mutation: ManifestMutation | None = None,
    controller_manifest_mutation: ManifestMutation | None = None,
    outer_manifest_mutation: ManifestMutation | None = None,
    outer_key: bytes = TEST_PUBLIC_KEY,
    nested_key: bytes = TEST_PUBLIC_KEY,
    receipt_id_override: str | None = None,
    omit_boi_inventory: bool = False,
    archive_attack: str | None = None,
    add_extra_file: bool = False,
    replayed: bool = False,
) -> Candidate:
    now_unix = int(time.time())
    root = tmp_path / "candidate-inputs"
    root.mkdir(mode=0o700)
    verifier = root / "trusted-sorafs-validate"
    verifier_sha256 = _write_fake_native_verifier(verifier)
    boi_artifact_handoff, boi_inventory_payload = _boi_artifact_handoff(
        root, boi_inventory_mutation
    )
    evidence = _evidence_root(root)
    linux_archive = _linux_archive(root, evidence)
    authorized_authority = _authority_payload(
        evidence, linux_archive, verifier_sha256
    )
    authority_payload = authorized_authority.subject
    if nested_authority_mutation is not None:
        nested_authority_mutation(authority_payload)

    linux_controller_manifest = contract.canonical_json_bytes(
        {
            "files": [
                {
                    "path": "scripts/linux-controller.py",
                    "sha256": hashlib.sha256(b"linux-controller").hexdigest(),
                    "size": len(b"linux-controller"),
                }
            ],
            "platform": "linux",
            "schema": "iroha.taira.release_controller_closure",
            "schema_version": 1,
            "source_commit": COMMIT,
        }
    )
    nested_artifacts: dict[str, bytes] = {
        "authority-controller-v1.json": linux_controller_manifest,
        "release_artifact_contract.py": b"trusted contract helper\n",
        "sorafs-validate": verifier.read_bytes(),
        admission.LINUX_AUTHORITY_SUBJECT: (
            contract.canonical_json_bytes(authority_payload)
        ),
        admission.LINUX_AUTHORITY_ENVELOPE: (
            authorized_authority.authority_envelope
        ),
        admission.LINUX_AUTHORITY_DURABLE_RECEIPT: (
            authorized_authority.durable_receipt
        ),
        "taira_release_authority.py": b"trusted exact12 helper\n",
    }
    if nested_artifact_mutation is not None:
        nested_artifact_mutation(nested_artifacts)
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
    privacy_receipt, privacy_evidence_files = _privacy_protocol_evidence(
        root,
        now_unix,
        hashlib.sha256(linux_archive.read_bytes()).hexdigest(),
        str(receipt["validator_binary_sha256"]),
        str(receipt["artifact_handoff_sha256"]),
        privacy_receipt_mutation,
    )
    privacy_receipt_id = str(privacy_receipt["receipt_id"])
    macos_controller_rows = []
    for relative in admission.MACOS_CONTROLLER_FILES:
        reviewed_payload = f"reviewed controller: {relative}\n".encode("ascii")
        macos_controller_rows.append(
            {
                "path": relative,
                "sha256": hashlib.sha256(reviewed_payload).hexdigest(),
                "size": len(reviewed_payload),
            }
        )
    macos_controller_manifest_object: dict[str, object] = {
        "files": macos_controller_rows,
        "platform": "macos",
        "schema": "iroha.taira.release_controller_closure",
        "schema_version": 1,
        "source_commit": COMMIT,
    }
    if controller_manifest_mutation is not None:
        controller_manifest_mutation(macos_controller_manifest_object)
    macos_controller_manifest = contract.canonical_json_bytes(
        macos_controller_manifest_object
    )
    macos_controller_digest = hashlib.sha256(
        b"iroha.taira.release-controller-closure.v1\0" + macos_controller_manifest
    ).hexdigest()
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
        admission.BOI_ARTIFACT_INVENTORY_PATH: boi_inventory_payload,
        **privacy_evidence_files,
        admission.CONTROLLER_MANIFEST_PATH: macos_controller_manifest,
    }
    if omit_boi_inventory:
        del outer_files[admission.BOI_ARTIFACT_INVENTORY_PATH]
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
        "boi_privacy_v1": {
            "artifact_count": len(admission.BOI_SOURCE_ARTIFACT_PATHS),
            "inventory_path": admission.BOI_ARTIFACT_INVENTORY_PATH,
            "inventory_sha256": hashlib.sha256(
                boi_inventory_payload
            ).hexdigest(),
        },
        "controller": {
            "digest": macos_controller_digest,
            "manifest_path": admission.CONTROLLER_MANIFEST_PATH,
            "platform": "macos",
            "source_commit": COMMIT,
        },
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
            "privacy_protocol_receipt_id": privacy_receipt_id,
            "privacy_protocol_receipt_path": admission.PRIVACY_PROTOCOL_RECEIPT_PATH,
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
        boi_artifact_handoff=boi_artifact_handoff,
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


def _assembler_inputs(
    base: Candidate, root: Path
) -> tuple[Path, Path, Path, Path, Path, Path]:
    linux_authority_dir = root / "linux-authority"
    linux_authority_dir.mkdir(parents=True, mode=0o700)
    receipt_path = root / "macos-receipt.json"
    privacy_evidence_dir = root / "privacy-protocol-four-peer-v2"
    privacy_evidence_dir.mkdir(mode=0o700)
    controller_manifest = root / "authority-controller-v1.json"
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
        for name in privacy_evidence.EVIDENCE_NAMES:
            privacy_member = archive.extractfile(
                members[
                    f"{prefix}/{admission.PRIVACY_PROTOCOL_EVIDENCE_DIRECTORY}/{name}"
                ]
            )
            assert privacy_member is not None
            (privacy_evidence_dir / name).write_bytes(privacy_member.read())
        controller_member = archive.extractfile(
            members[f"{prefix}/{admission.CONTROLLER_MANIFEST_PATH}"]
        )
        assert controller_member is not None
        controller_manifest.write_bytes(controller_member.read())
    return (
        linux_archive,
        linux_authority_dir,
        base.boi_artifact_handoff,
        receipt_path,
        privacy_evidence_dir,
        controller_manifest,
    )


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
    assert result["receipt_signers"] == _receipt_signers()
    assert result["boi_artifact_inventory_sha256"] == hashlib.sha256(
        (
            candidate.boi_artifact_handoff
            / admission.BOI_SOURCE_HANDOFF_MANIFEST
        ).read_bytes()
    ).hexdigest()
    assert set(result["validator_config_sha256"]) == {
        f"taira-validator-{number}" for number in range(1, 5)
    }


@pytest.mark.parametrize(
    "sidecar",
    (
        admission.LINUX_AUTHORITY_ENVELOPE,
        admission.LINUX_AUTHORITY_DURABLE_RECEIPT,
    ),
)
def test_signed_candidate_requires_both_native_authority_sidecars(
    tmp_path: Path,
    sidecar: str,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        nested_artifact_mutation=lambda artifacts: artifacts.pop(sidecar),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="does not contain the exact first-release inventory",
    ):
        _verify(candidate)


def _replace_boi_row_digest(
    manifest: dict[str, object], relative: str, digest: str
) -> None:
    rows = manifest["files"]
    assert isinstance(rows, list)
    row = next(value for value in rows if value["path"] == relative)
    row["sha256"] = digest


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (lambda value: value["files"].pop(), "exactly thirteen"),
        (lambda value: value["files"].reverse(), "exact.*sequence"),
        (
            lambda value: _replace_boi_row_digest(
                value, "source/Cargo.lock", "9" * 64
            ),
            "different Cargo.lock",
        ),
        (
            lambda value: _replace_boi_row_digest(
                value, "source/workspace-source-manifest.sha256", "9" * 64
            ),
            "different source manifest",
        ),
        (
            lambda value: _replace_boi_row_digest(
                value, "source/exact12-v1.tsv", "9" * 64
            ),
            "different Exact12 matrix",
        ),
    ),
    ids=("missing-row", "reordered", "wrong-lock", "stale-source", "wrong-matrix"),
)
def test_signed_candidate_rejects_hostile_boi_inventory(
    tmp_path: Path,
    mutation: ManifestMutation,
    message: str,
) -> None:
    candidate = _build_candidate(tmp_path, boi_inventory_mutation=mutation)

    with pytest.raises(admission.TairaRolloutAdmissionError, match=message):
        _verify(candidate)


def test_signed_candidate_requires_boi_inventory_not_macos_handoff_substitution(
    tmp_path: Path,
) -> None:
    missing_root = tmp_path / "missing"
    missing_root.mkdir()
    candidate = _build_candidate(
        missing_root,
        omit_boi_inventory=True,
    )
    with pytest.raises(admission.TairaRolloutAdmissionError):
        _verify(candidate)

    substitution_root = tmp_path / "macos-substitution"
    substitution_root.mkdir()
    candidate = _build_candidate(
        substitution_root,
        admission_mutation=lambda manifest: manifest["boi_privacy_v1"].__setitem__(
            "inventory_sha256", "2" * 64
        ),
    )
    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="BOI artifact inventory differs|BOI artifact inventory.*binding",
    ):
        _verify(candidate)


def test_signed_candidate_rejects_non_integer_boi_artifact_count(
    tmp_path: Path,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        admission_mutation=lambda manifest: manifest["boi_privacy_v1"].__setitem__(
            "artifact_count", 13.0
        ),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="artifact count must be an integer",
    ):
        _verify(candidate)


def test_candidate_authority_rejects_boi_bytes_not_named_by_inventory(
    tmp_path: Path,
) -> None:
    candidate = _build_candidate(tmp_path)
    target = candidate.boi_artifact_handoff / "worker/iroha_privacy_wallet_worker"
    target.write_bytes(b"wrong worker bytes\n")

    with pytest.raises(
        candidate_builder.TairaCandidateBuildError,
        match="differs from inventory",
    ):
        candidate_builder._capture_boi_artifact_handoff(
            candidate.boi_artifact_handoff.resolve(),
            source=SOURCE,
            expected_exact12_matrix_sha256=hashlib.sha256(
                EXACT12.read_bytes()
            ).hexdigest(),
        )


@pytest.mark.parametrize(
    "mutation, message",
    (
        (lambda value: value["outcomes"].pop(), "exactly twelve"),
        (
            lambda value: value["outcomes"].append(dict(value["outcomes"][0])),
            "exactly twelve",
        ),
        (
            lambda value: value["outcomes"].__setitem__(
                slice(0, 2), [value["outcomes"][1], value["outcomes"][0]]
            ),
            "substituted|noncanonical",
        ),
        (
            lambda value: value["outcomes"][0].__setitem__(
                "profile", "engine-unavailable"
            ),
            "unavailable|substituted",
        ),
        (
            lambda value: value["candidate"].__setitem__(
                "linux_release_archive_sha256", "9" * 64
            ),
            "stale linux_release_archive_sha256",
        ),
        (
            lambda value: value["candidate"]["source"].__setitem__(
                "commit", "e" * 40
            ),
            "different source identity",
        ),
        (
            lambda value: value["cases"][0].__setitem__(
                "transcript_sha256", "9" * 64
            ),
            "stored bytes differ",
        ),
        (
            lambda value: value["cases"][1].__setitem__(
                "transcript_id", value["cases"][0]["transcript_id"]
            ),
            "IDs differ|repeats",
        ),
        (
            lambda value: value.__setitem__("expires_at_unix", 1),
            "lifetime|stale",
        ),
    ),
    ids=(
        "missing-row",
        "unknown-extra-row",
        "outcome-swap",
        "availability-outcome-substitution",
        "stale-candidate-archive",
        "stale-candidate-source",
        "per-case-candidate-substitution",
        "shared-case-digest-substitution",
        "stale-receipt",
    ),
)
def test_privacy_protocol_receipt_rejects_hostile_exact12_mutations(
    tmp_path: Path,
    mutation: PrivacyReceiptMutation,
    message: str,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        privacy_receipt_mutation=mutation,
    )

    with pytest.raises(admission.TairaRolloutAdmissionError, match=message):
        _verify(candidate)


def test_fail_closed_protocol_receipt_cannot_replace_native_release_authority(
    tmp_path: Path,
) -> None:
    def remove_exact12_native_evidence(authority: dict[str, object]) -> None:
        rows = authority["native_release_evidence"]
        assert isinstance(rows, list)
        authority["native_release_evidence"] = [
            row
            for row in rows
            if not (
                isinstance(row, dict)
                and row.get("path")
                == linux_authority.EVIDENCE_PATHS["exact12_matrix"]
            )
        ]

    candidate = _build_candidate(
        tmp_path,
        nested_authority_mutation=remove_exact12_native_evidence,
    )

    with pytest.raises(admission.TairaRolloutAdmissionError):
        _verify(candidate)


def test_admission_controller_closure_matches_the_root_sealer() -> None:
    assert admission.MACOS_CONTROLLER_FILES == tuple(
        sorted(controller_seal.MACOS_FILES)
    )
    assert {
        "scripts/deploy_taira_v21_reset_authority.py",
        "scripts/deploy_taira_v21_reset_health.py",
    } <= set(admission.MACOS_CONTROLLER_FILES)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda manifest: manifest["files"].pop(),
        lambda manifest: manifest["files"].append(
            {
                "path": "zz-unreviewed-controller.py",
                "sha256": "9" * 64,
                "size": 1,
            }
        ),
    ),
    ids=("missing-controller", "extra-controller"),
)
def test_controller_manifest_rejects_a_digest_consistent_noncanonical_closure(
    tmp_path: Path,
    mutation: ManifestMutation,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        controller_manifest_mutation=mutation,
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="exact release closure",
    ):
        _verify(candidate)


def test_controller_digest_substitution_is_rejected_even_when_candidate_is_resigned(
    tmp_path: Path,
) -> None:
    candidate = _build_candidate(
        tmp_path,
        admission_mutation=lambda manifest: manifest["controller"].__setitem__(
            "digest", "0" * 64
        ),
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="differs from its bound digest",
    ):
        _verify(candidate)


def test_candidate_builder_reconstructs_the_same_admitted_archive_deterministically(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    base_root = tmp_path / "base"
    base_root.mkdir()
    base = _build_candidate(base_root)
    input_root = tmp_path / "assembler-inputs"
    input_root.mkdir()
    (
        linux_archive,
        linux_authority_dir,
        boi_artifact_handoff,
        receipt,
        privacy_evidence_dir,
        controller_manifest,
    ) = (
        _assembler_inputs(base, input_root)
    )
    controller_digest = hashlib.sha256(
        b"iroha.taira.release-controller-closure.v1\0"
        + controller_manifest.read_bytes()
    ).hexdigest()
    monkeypatch.setattr(
        candidate_builder,
        "_sealed_controller_manifest_path",
        lambda: controller_manifest,
    )
    static_rebinds: list[Path] = []

    def accept_fixture_boi(root: Path, **_kwargs) -> dict[str, contract.StableFile]:
        static_rebinds.append(root)
        return {
            relative: contract.stable_hash_relative(root, relative)
            for relative in admission.BOI_SOURCE_ARTIFACT_PATHS
        }

    monkeypatch.setattr(
        candidate_builder.qualification_handoff,
        "validate_candidate_artifact_handoff",
        accept_fixture_boi,
    )
    public_key = input_root / "release.pub"
    public_key.write_bytes(TEST_PUBLIC_KEY)
    signer = input_root / "external-signer"
    _write_fake_external_signer(signer)

    common = {
        "cargo_lock_sha256": CARGO_LOCK_SHA,
        "controller_digest": controller_digest,
        "controller_manifest": controller_manifest,
        "dpn_validator_release_commit": DPN_COMMIT,
        "expected_receipt_id": base.receipt_id,
        "external_signer": signer,
        "boi_artifact_handoff_dir": boi_artifact_handoff,
        "linux_archive": linux_archive,
        "linux_authority_dir": linux_authority_dir,
        "macos_receipt": receipt,
        "privacy_protocol_evidence_dir": privacy_evidence_dir,
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
    assert static_rebinds == [boi_artifact_handoff.resolve()] * 2
    prefix = first_archive.name.removesuffix(".tar.gz")
    with tarfile.open(first_archive, mode="r:gz") as archive:
        members = {member.name: member for member in archive.getmembers()}
        boi_member = archive.extractfile(
            members[f"{prefix}/{admission.BOI_ARTIFACT_INVENTORY_PATH}"]
        )
        assert boi_member is not None
        assert boi_member.read() == (
            boi_artifact_handoff / admission.BOI_SOURCE_HANDOFF_MANIFEST
        ).read_bytes()
        for driver_name, payload in DRIVER_BYTES.items():
            relative = (
                f"{prefix}/{admission.PRIVACY_PROTOCOL_EVIDENCE_DIRECTORY}/"
                f"{privacy_evidence.DRIVER_EVIDENCE_NAMES[driver_name]}"
            )
            member = archive.extractfile(members[relative])
            assert member is not None
            assert member.read() == payload
    first_authority = Path(str(first["authority_dir"]))
    second_authority = Path(str(second["authority_dir"]))
    for name in admission.FINAL_AUTHORITY_FILES:
        assert (first_authority / name).read_bytes() == (
            second_authority / name
        ).read_bytes()


@pytest.mark.parametrize(
    "relative",
    (
        admission.BOI_SOURCE_HANDOFF_MANIFEST,
        "sdk/iroha_python_privacy_v1.whl",
    ),
    ids=("inventory", "artifact"),
)
def test_candidate_builder_rejects_boi_inventory_toctou(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
) -> None:
    candidate = _build_candidate(tmp_path)
    monkeypatch.setattr(
        candidate_builder.qualification_handoff,
        "validate_candidate_artifact_handoff",
        lambda root, **_kwargs: {
            relative: contract.stable_hash_relative(root, relative)
            for relative in admission.BOI_SOURCE_ARTIFACT_PATHS
        },
    )
    snapshot = candidate_builder._capture_boi_artifact_handoff(
        candidate.boi_artifact_handoff.resolve(),
        source=SOURCE,
        expected_exact12_matrix_sha256=hashlib.sha256(EXACT12.read_bytes()).hexdigest(),
    )
    target = candidate.boi_artifact_handoff / relative
    target.write_bytes(b"substituted after candidate preflight\n")

    with pytest.raises(
        candidate_builder.TairaCandidateBuildError,
        match="changed during signing|cannot recheck",
    ):
        candidate_builder._recheck_boi_artifact_handoff(snapshot)


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
            lambda receipt: receipt.__setitem__("peer_count", 5),
            "exactly four peers",
        ),
        (
            lambda receipt: receipt["peers"].pop(),
            "exactly four peer rows",
        ),
        (
            lambda receipt: receipt["peers"].append(
                {
                    **receipt["peers"][-1],
                    "number": 5,
                    "slug": "taira-validator-5",
                }
            ),
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
            lambda receipt: receipt["source"].__setitem__(
                "dpn_validator_release_commit", "e" * 40
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
        (
            lambda receipt: receipt.pop("receipt_signers"),
            "fields are not exact",
        ),
        (
            lambda receipt: receipt["receipt_signers"].pop(admission.SLUGS[-1]),
            "exact ordered four validator slugs",
        ),
        (
            lambda receipt: receipt["receipt_signers"][
                admission.SLUGS[0]
            ].__setitem__(
                "node_id", admission.RECEIPT_NODE_ID_PREFIX + "0" * 64
            ),
            "not derived from its receipt key",
        ),
        (
            _swap_receipt_signer_associations,
            "exact receipt signer node ID",
        ),
        (
            lambda receipt: receipt["receipt_signers"][
                admission.SLUGS[0]
            ].__setitem__("receipt_private_key", "812620" + "01" * 32),
            "fields are not exact",
        ),
        (
            lambda receipt: receipt["receipt_signers"][admission.SLUGS[0]][
                "public_key"
            ].__setitem__("payload_hex", "00" * 33),
            "payload is noncanonical",
        ),
        (
            lambda receipt: receipt["receipt_signers"].__setitem__(
                admission.SLUGS[1],
                dict(receipt["receipt_signers"][admission.SLUGS[0]]),
            ),
            "aliases receipt signer identities",
        ),
    ],
    ids=(
        "wrong-arch",
        "declared-three",
        "declared-five",
        "three-rows",
        "five-rows",
        "extra-field",
        "source-drift",
        "dpn-source-drift",
        "missing-reset-manifest-binding",
        "missing-config-binding",
        "peer-config-substitution",
        "invalid-supervisor-binding",
        "missing-receipt-signer-map",
        "missing-receipt-signer",
        "receipt-node-mismatch",
        "receipt-signer-association-swap",
        "receipt-private-key-leak",
        "non-secp-receipt-key",
        "duplicate-receipt-signer",
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


def test_receipt_id_cannot_be_replayed_across_a_dpn_only_change() -> None:
    receipt = _receipt(1_000, None)
    original_receipt_id = str(receipt["receipt_id"])
    receipt["source"]["dpn_validator_release_commit"] = "e" * 40
    changed_source = admission.SourceIdentity(
        COMMIT,
        "e" * 40,
        CARGO_LOCK_SHA,
        WORKSPACE_SHA,
    )

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="canonical receipt body",
    ):
        admission._validate_macos_receipt(
            contract.canonical_json_bytes(receipt),
            expected_source=changed_source,
            expected_receipt_id=original_receipt_id,
            consumed_receipt_ids=set(),
            now_unix=1_000,
        )


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


def test_empty_replay_ledger_initializer_is_create_new_and_canonical(
    tmp_path: Path,
) -> None:
    output = tmp_path / "empty-replay-ledger.json"

    result = admission.initialize_empty_replay_ledger(output)

    assert output.read_bytes() == admission.canonical_replay_ledger_bytes([])
    assert output.stat().st_mode & 0o777 == 0o600
    assert result["sha256"] == hashlib.sha256(output.read_bytes()).hexdigest()
    with pytest.raises(contract.ReleaseArtifactError):
        admission.initialize_empty_replay_ledger(output)


def test_empty_replay_ledger_initializer_rejects_symlink_output(
    tmp_path: Path,
) -> None:
    victim = tmp_path / "victim"
    victim.write_bytes(b"unchanged")
    output = tmp_path / "empty-replay-ledger.json"
    output.symlink_to(victim.name)

    with pytest.raises(contract.ReleaseArtifactError):
        admission.initialize_empty_replay_ledger(output)

    assert victim.read_bytes() == b"unchanged"


def test_replay_ledger_inode_swap_during_verification_is_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidate = _build_candidate(tmp_path)
    original = admission._verify_closed_linux_authority

    def swap_ledger(*args, **kwargs):
        result = original(*args, **kwargs)
        replacement = candidate.replay_ledger.with_suffix(".replacement")
        replacement.write_bytes(admission.canonical_replay_ledger_bytes([]))
        os.replace(replacement, candidate.replay_ledger)
        return result

    monkeypatch.setattr(admission, "_verify_closed_linux_authority", swap_ledger)

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="replay ledger changed during admission verification",
    ):
        _verify(candidate)


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


def test_reviewed_signer_fingerprint_substitution_is_rejected(tmp_path: Path) -> None:
    candidate = _build_candidate(tmp_path)

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="public key does not match the reviewed fingerprint",
    ):
        admission.verify_admission(
            archive_path=candidate.archive,
            authority_dir=candidate.authority_dir,
            expected_source=SOURCE,
            expected_receipt_id=candidate.receipt_id,
            replay_ledger_path=candidate.replay_ledger,
            trusted_signing_fingerprint="0" * 64,
            release_manifest_verifier_path=candidate.verifier,
            trusted_release_manifest_verifier_sha256=candidate.verifier_sha256,
            now_unix=candidate.now_unix,
        )


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
            "--expected-dpn-validator-release-commit",
            DPN_COMMIT,
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
