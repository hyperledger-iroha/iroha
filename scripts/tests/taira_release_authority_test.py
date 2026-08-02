from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import tarfile
from pathlib import Path

import pytest

from scripts import release_artifact_contract as contract
from scripts import taira_release_authority as authority


ROOT = Path(__file__).resolve().parents[2]
EXACT12 = ROOT / "fixtures" / "privacy" / "exact12_v1.tsv"
COMMIT = "1" * 40
FINGERPRINT = "2" * 64
VERIFIER_SHA = "3" * 64
SOURCE_SHA = "4" * 64
DPN_COMMIT = "5" * 40


def _evidence_root(tmp_path: Path) -> Path:
    root = tmp_path / "evidence"
    for index, relative in enumerate(authority.EVIDENCE_PATHS.values(), start=1):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if relative == authority.EVIDENCE_PATHS["exact12_matrix"]:
            path.write_bytes(EXACT12.read_bytes())
        elif relative == authority.EVIDENCE_PATHS["workspace_source_manifest"]:
            path.write_text(f"{SOURCE_SHA}\n", encoding="ascii")
        else:
            path.write_bytes(f"native-evidence-{index}\n".encode())
    cargo_sha256 = hashlib.sha256(
        (root / authority.EVIDENCE_PATHS["cargo_lock"]).read_bytes()
    ).hexdigest()
    provenance = {
        "dpn_validator_release_commit": DPN_COMMIT,
        "iroha_git_head": COMMIT,
        "iroha_source_attested": True,
        "iroha_source_bundle_provenance_sha256": "a" * 64,
        "iroha_source_tree_sha256": "b" * 64,
        "iroha_tracked_patch_sha256": "c" * 64,
        "iroha_worktree_clean": False,
        "schema_version": 1,
        "validator_lock_sha256": cargo_sha256,
        "workspace_source_manifest_sha256": SOURCE_SHA,
    }
    (
        root / authority.EVIDENCE_PATHS["dpn_validator_build_provenance"]
    ).write_bytes(contract.canonical_json_bytes(provenance))
    return root


def _archive(tmp_path: Path, evidence_root: Path) -> Path:
    path = tmp_path / "taira-rollout-test-release.tar.gz"
    prefix = path.name.removesuffix(".tar.gz")
    with tarfile.open(path, mode="w:gz") as archive:
        for relative in authority.EVIDENCE_PATHS.values():
            archive.add(
                evidence_root / relative,
                arcname=f"{prefix}/{relative}",
                recursive=False,
            )
    return path


def _args(
    tmp_path: Path,
    *,
    evidence_root: Path | None = None,
    archive: Path | None = None,
    image: bool = False,
) -> argparse.Namespace:
    root = evidence_root or _evidence_root(tmp_path)
    if image:
        tags = sorted(
            [
                f"hyperledger/iroha:taira-source-{SOURCE_SHA}",
                (
                    "docker.soramitsu.co.jp/iroha3/iroha:"
                    f"taira-source-{SOURCE_SHA}"
                ),
            ]
        )
        return argparse.Namespace(
            evidence_root=str(root),
            commit=COMMIT,
            dpn_validator_release_commit=DPN_COMMIT,
            signing_fingerprint=FINGERPRINT,
            native_verifier_sha256=VERIFIER_SHA,
            archive=None,
            image_manifest_digest=f"sha256:{'5' * 64}",
            image_id=f"sha256:{'6' * 64}",
            image_tag=tags,
        )
    return argparse.Namespace(
        evidence_root=str(root),
        commit=COMMIT,
        dpn_validator_release_commit=DPN_COMMIT,
        signing_fingerprint=FINGERPRINT,
        native_verifier_sha256=VERIFIER_SHA,
        archive=str(archive or _archive(tmp_path, root)),
        image_manifest_digest=None,
        image_id=None,
        image_tag=[],
    )


def test_archive_authority_is_canonical_portable_and_exact12(tmp_path: Path) -> None:
    args = _args(tmp_path)
    payload = authority.build_authority(args)
    encoded = contract.canonical_json_bytes(payload)
    decoded = json.loads(encoded)

    assert decoded["schema"] == authority.SCHEMA
    assert decoded["schema_version"] == 1
    assert decoded["release_profile"] == "release"
    assert decoded["dpn_validator_release_commit"] == DPN_COMMIT
    assert decoded["workspace_source_manifest_sha256"] == SOURCE_SHA
    assert decoded["exact12"] == {
        "protocol_count": 12,
        "protocol_labels": [label for label, _ in authority.PROTOCOLS],
        "registry_sha256": authority.REGISTRY_SHA256,
        "retired_labels": list(authority.RETIRED_LABELS),
        "stage_count": 48,
        "typed_envelope_count": 12,
    }
    assert decoded["subject"]["kind"] == "taira-rollout-tar-gzip-v1"
    assert decoded["subject"]["name"] == Path(args.archive).name
    assert len(decoded["native_release_evidence"]) == len(
        authority.EVIDENCE_PATHS
    )
    for row in decoded["native_release_evidence"]:
        assert not row["path"].startswith("/")
        assert ".." not in Path(row["path"]).parts
    assert str(tmp_path) not in encoded.decode()
    artifact_paths = {row["path"] for row in decoded["native_release_evidence"]}
    assert authority.EVIDENCE_PATHS["x509_resource_norito"] in artifact_paths
    assert authority.EVIDENCE_PATHS["x509_resource_json"] in artifact_paths


def test_authority_rejects_dpn_only_provenance_mismatch(tmp_path: Path) -> None:
    args = _args(tmp_path)
    args.dpn_validator_release_commit = "6" * 40

    with pytest.raises(
        authority.TairaReleaseAuthorityError,
        match="provenance release commit differs",
    ):
        authority.build_authority(args)


def test_create_then_verify_rebuilds_exact_subject(tmp_path: Path) -> None:
    args = _args(tmp_path)
    output = tmp_path / "taira-exact12-release-authority-v1.json"
    create_args = argparse.Namespace(**vars(args), command="create", output=str(output))
    assert authority.main(
        [
            "create",
            "--evidence-root",
            args.evidence_root,
            "--commit",
            COMMIT,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--signing-fingerprint",
            FINGERPRINT,
            "--native-verifier-sha256",
            VERIFIER_SHA,
            "--archive",
            args.archive,
            "--output",
            str(output),
        ]
    ) == 0
    assert output.read_bytes() == contract.canonical_json_bytes(
        authority.build_authority(create_args)
    )
    assert authority.main(
        [
            "verify",
            "--evidence-root",
            args.evidence_root,
            "--commit",
            COMMIT,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--signing-fingerprint",
            FINGERPRINT,
            "--native-verifier-sha256",
            VERIFIER_SHA,
            "--archive",
            args.archive,
            "--authority",
            str(output),
        ]
    ) == 0
    assert authority.main(
        [
            "create",
            "--evidence-root",
            args.evidence_root,
            "--commit",
            COMMIT,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--signing-fingerprint",
            FINGERPRINT,
            "--native-verifier-sha256",
            VERIFIER_SHA,
            "--archive",
            args.archive,
            "--output",
            str(output),
        ]
    ) == 1


@pytest.mark.parametrize(
    ("mutate", "message"),
    (
        (
            lambda payload: payload.replace(b"matrix-version\t1", b"matrix-version\t2"),
            "canonical v1",
        ),
        (
            lambda payload: payload.replace(
                b"protocol\t0\tzk-ace-pq-authorization-v0",
                b"protocol\t0\tlegacy-alias",
            ),
            "reordered, missing, aliased",
        ),
        (
            lambda payload: payload.replace(
                b"registry-sha256\t734e",
                b"registry-sha256\t0000",
            ),
            "wrong first-release registry",
        ),
        (
            lambda payload: payload.replace(
                b"retired\tsis-with-hints\n",
                b"",
            ),
            "retired-label",
        ),
        (
            lambda payload: payload.replace(b"\n", b"\r\n"),
            "LF-delimited",
        ),
        (
            lambda payload: payload.replace(
                b"protocol\t11\tpq-masp-stark-v0\tPqMaspStarkV0\tPqMaspStarkV0\n",
                b"",
            ),
            "exactly 12 protocol",
        ),
    ),
    ids=(
        "wrong-version",
        "active-alias",
        "registry-substitution",
        "retired-alias-removed",
        "crlf",
        "missing-route",
    ),
)
def test_matrix_adversarial_mutations_are_rejected(
    tmp_path: Path,
    mutate,
    message: str,
) -> None:
    root = _evidence_root(tmp_path)
    matrix = root / authority.EVIDENCE_PATHS["exact12_matrix"]
    matrix.write_bytes(mutate(matrix.read_bytes()))
    with pytest.raises(authority.TairaReleaseAuthorityError, match=message):
        authority.build_authority(_args(tmp_path, evidence_root=root))


def test_missing_symlink_hardlink_and_empty_evidence_fail_closed(
    tmp_path: Path,
) -> None:
    root = _evidence_root(tmp_path)
    target = root / authority.EVIDENCE_PATHS["receipt_norito"]
    target.unlink()
    with pytest.raises((FileNotFoundError, contract.ReleaseArtifactError)):
        authority.build_authority(_args(tmp_path, evidence_root=root))

    target.write_bytes(b"restored\n")
    alias = tmp_path / "alias"
    alias.write_bytes(b"alias\n")
    target.unlink()
    target.symlink_to(alias)
    with pytest.raises(
        contract.ReleaseArtifactError,
        match="symlink|symbolic links|regular file",
    ):
        authority.build_authority(_args(tmp_path, evidence_root=root))

    target.unlink()
    target.write_bytes(b"hard-linked\n")
    os.link(target, tmp_path / "second-hard-link")
    with pytest.raises(contract.ReleaseArtifactError, match="hard link"):
        authority.build_authority(_args(tmp_path, evidence_root=root))

    (tmp_path / "second-hard-link").unlink()
    target.write_bytes(b"")
    with pytest.raises(
        (authority.TairaReleaseAuthorityError, contract.ReleaseArtifactError),
        match="must not be empty",
    ):
        authority.build_authority(_args(tmp_path, evidence_root=root))


def test_source_archive_and_authority_mutations_are_rejected(tmp_path: Path) -> None:
    args = _args(tmp_path)
    output = tmp_path / "authority.json"
    assert authority.main(
        [
            "create",
            "--evidence-root",
            args.evidence_root,
            "--commit",
            COMMIT,
            "--dpn-validator-release-commit",
            DPN_COMMIT,
            "--signing-fingerprint",
            FINGERPRINT,
            "--native-verifier-sha256",
            VERIFIER_SHA,
            "--archive",
            args.archive,
            "--output",
            str(output),
        ]
    ) == 0
    Path(args.archive).write_bytes(b"substituted archive\n")
    verify = [
        "verify",
        "--evidence-root",
        args.evidence_root,
        "--commit",
        COMMIT,
        "--dpn-validator-release-commit",
        DPN_COMMIT,
        "--signing-fingerprint",
        FINGERPRINT,
        "--native-verifier-sha256",
        VERIFIER_SHA,
        "--archive",
        args.archive,
        "--authority",
        str(output),
    ]
    assert authority.main(verify) == 1

    Path(args.archive).unlink()
    _archive(tmp_path, Path(args.evidence_root))
    receipt = (
        Path(args.evidence_root)
        / authority.EVIDENCE_PATHS["receipt_json"]
    )
    receipt_original = receipt.read_bytes()
    receipt.write_bytes(b"substituted evidence\n")
    assert authority.main(verify) == 1

    receipt.write_bytes(receipt_original)
    parsed = json.loads(output.read_text())
    parsed["unexpected"] = True
    output.write_text(
        json.dumps(parsed, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    assert authority.main(verify) == 1


@pytest.mark.parametrize(
    "attack",
    ("traversal", "symlink", "fifo", "duplicate", "missing", "substituted"),
)
def test_archive_member_attacks_fail_closed(
    tmp_path: Path,
    attack: str,
) -> None:
    root = _evidence_root(tmp_path)
    archive_path = tmp_path / "taira-rollout-test-release.tar.gz"
    prefix = archive_path.name.removesuffix(".tar.gz")
    missing_path = authority.EVIDENCE_PATHS["receipt_norito"]
    with tarfile.open(archive_path, mode="w:gz") as archive:
        for relative in authority.EVIDENCE_PATHS.values():
            if attack == "missing" and relative == missing_path:
                continue
            source = root / relative
            if attack == "substituted" and relative == missing_path:
                info = tarfile.TarInfo(f"{prefix}/{relative}")
                payload = b"substituted-native-evidence\n"
                info.size = len(payload)
                archive.addfile(info, io.BytesIO(payload))
            else:
                archive.add(
                    source,
                    arcname=f"{prefix}/{relative}",
                    recursive=False,
                )
        if attack == "traversal":
            info = tarfile.TarInfo(f"{prefix}/../escape")
            info.size = 1
            archive.addfile(info, io.BytesIO(b"x"))
        elif attack == "symlink":
            info = tarfile.TarInfo(f"{prefix}/escape-link")
            info.type = tarfile.SYMTYPE
            info.linkname = "../../escape"
            archive.addfile(info)
        elif attack == "fifo":
            info = tarfile.TarInfo(f"{prefix}/release-fifo")
            info.type = tarfile.FIFOTYPE
            archive.addfile(info)
        elif attack == "duplicate":
            relative = authority.EVIDENCE_PATHS["receipt_json"]
            archive.add(
                root / relative,
                arcname=f"{prefix}/{relative}",
                recursive=False,
            )

    with pytest.raises(authority.TairaReleaseAuthorityError):
        authority.build_authority(
            _args(
                tmp_path,
                evidence_root=root,
                archive=archive_path,
            )
        )


@pytest.mark.parametrize(
    "evidence_key",
    ("validator_binary", "receipt_norito"),
)
def test_archive_expected_evidence_directory_substitution_fails_closed(
    tmp_path: Path,
    evidence_key: str,
) -> None:
    root = _evidence_root(tmp_path)
    archive_path = tmp_path / "taira-rollout-test-release.tar.gz"
    prefix = archive_path.name.removesuffix(".tar.gz")
    substituted_path = authority.EVIDENCE_PATHS[evidence_key]
    with tarfile.open(archive_path, mode="w:gz") as archive:
        for relative in authority.EVIDENCE_PATHS.values():
            if relative == substituted_path:
                info = tarfile.TarInfo(f"{prefix}/{relative}/")
                info.type = tarfile.DIRTYPE
                archive.addfile(info)
                continue
            archive.add(
                root / relative,
                arcname=f"{prefix}/{relative}",
                recursive=False,
            )

    with pytest.raises(
        authority.TairaReleaseAuthorityError,
        match="evidence must be a regular file",
    ):
        authority.build_authority(
            _args(
                tmp_path,
                evidence_root=root,
                archive=archive_path,
            )
        )


def test_image_authority_requires_exact_source_bound_registry_pair(
    tmp_path: Path,
) -> None:
    args = _args(tmp_path, image=True)
    payload = authority.build_authority(args)
    assert payload["subject"]["manifest_digest"] == f"sha256:{'5' * 64}"
    assert payload["subject"]["image_id"] == f"sha256:{'6' * 64}"

    for tags in (
        args.image_tag[:1],
        list(reversed(args.image_tag)),
        [*args.image_tag, args.image_tag[0]],
        [*args.image_tag, "hyperledger/iroha:taira-latest"],
        [*args.image_tag, "hyperledger/iroha:taira-source-unbound"],
        [*args.image_tag, "hyperledger/iroha:taira-latest\ninjected"],
    ):
        mutated = argparse.Namespace(**vars(args))
        mutated.image_tag = tags
        with pytest.raises(authority.TairaReleaseAuthorityError):
            authority.build_authority(mutated)


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("commit", "A" * 40),
        ("commit", "1" * 39),
        ("dpn_validator_release_commit", "A" * 40),
        ("dpn_validator_release_commit", "5" * 39),
        ("signing_fingerprint", "2" * 63),
        ("native_verifier_sha256", "g" * 64),
        ("image_manifest_digest", "sha256:" + "A" * 64),
        ("image_id", "6" * 64),
    ),
)
def test_noncanonical_identifiers_are_rejected(
    tmp_path: Path,
    field: str,
    value: str,
) -> None:
    args = _args(tmp_path, image=True)
    setattr(args, field, value)
    with pytest.raises(authority.TairaReleaseAuthorityError):
        authority.build_authority(args)
