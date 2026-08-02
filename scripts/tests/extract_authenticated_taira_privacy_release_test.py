"""Adversarial tests for authenticated Taira privacy-release extraction."""

from __future__ import annotations

import argparse
from io import BytesIO
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
from typing import Callable

import pytest

from scripts import extract_authenticated_taira_privacy_release as MODULE
from scripts import taira_rollout_admission as admission


SOURCE = admission.SourceIdentity("ab" * 20, "12" * 20, "cd" * 32, "ef" * 32)
SCRIPT = Path(MODULE.__file__).resolve()


def test_isolated_cli_loads_only_its_trusted_sibling_modules(
    tmp_path: Path,
) -> None:
    result = subprocess.run(
        [sys.executable, "-I", "-S", str(SCRIPT), "--help"],
        cwd=tmp_path,
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "--trusted-release-manifest-verifier-sha256" in result.stdout


def _payloads() -> dict[str, bytes]:
    return {
        "privacy_bootstrap_plan.json": b'{"plan":true}\n',
        "config.toml": b'chain = "taira"\n',
        "genesis.json": b'{"transactions":[]}\n',
        "bootle_lantern_broker_public.json": b'{"broker":true}\n',
    }


def _rollout_manifest(
    payloads: dict[str, bytes],
    *,
    mutate: Callable[[dict[str, object]], None] | None = None,
) -> bytes:
    release: dict[str, object] = {
        "schema": "iroha.taira.privacy-bootstrap-release-bundle.v1",
        "native_recomposition_passed": True,
        "bundled_release_validation_passed": True,
        "secret_free": True,
    }
    for output_name, contract in MODULE.PRIVACY_INPUTS.items():
        key = str(contract["manifest_key"])
        row: dict[str, object] = {
            "path": contract["rollout_path"],
            "sha256": hashlib.sha256(payloads[output_name]).hexdigest(),
        }
        if key == "peer_1_config":
            row.update(
                operator_copy=contract["operator_copy"],
                designated_validator="taira-validator-1",
            )
        elif contract["operator_copy"] is not None:
            row["operator_copy"] = contract["operator_copy"]
        else:
            row["bound_by_plan_sha256"] = True
        release[key] = row
    manifest: dict[str, object] = {
        "dpn_validator_release_commit": SOURCE.dpn_validator_release_commit,
        "git_head": SOURCE.commit,
        "validator_lock_sha256": SOURCE.cargo_lock_sha256,
        "workspace_source_manifest_sha256": SOURCE.workspace_source_manifest_sha256,
        "cargo_profile": "release",
        "privacy_bootstrap_release": release,
    }
    if mutate is not None:
        mutate(manifest)
    return (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()


def _archive(
    root: Path,
    *,
    mutate_manifest: Callable[[dict[str, object]], None] | None = None,
    duplicate: str | None = None,
    omit: str | None = None,
    operator_mismatch: str | None = None,
    extra_member: tarfile.TarInfo | None = None,
) -> Path:
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    archive_path = root / "taira-rollout-test-linux-aarch64.tar.gz"
    prefix = archive_path.name.removesuffix(".tar.gz")
    payloads = _payloads()
    members: list[tuple[str, bytes]] = [
        (MODULE.ROLLOUT_MANIFEST_PATH, _rollout_manifest(payloads, mutate=mutate_manifest))
    ]
    for output_name, contract in MODULE.PRIVACY_INPUTS.items():
        payload = payloads[output_name]
        members.append((str(contract["rollout_path"]), payload))
        if contract["operator_copy"] is not None:
            operator_payload = (
                b"substituted\n" if operator_mismatch == output_name else payload
            )
            members.append((str(contract["operator_copy"]), operator_payload))
    if duplicate is not None:
        contract = MODULE.PRIVACY_INPUTS[duplicate]
        members.append((str(contract["rollout_path"]), payloads[duplicate]))
    with tarfile.open(archive_path, "w:gz") as archive:
        for relative, payload in members:
            if relative == omit:
                continue
            info = tarfile.TarInfo(f"{prefix}/{relative}")
            info.size = len(payload)
            info.mode = 0o600
            archive.addfile(info, BytesIO(payload))
        if extra_member is not None:
            archive.addfile(extra_member)
    archive_path.chmod(0o600)
    return archive_path


def test_extracts_only_exact_manifest_bound_release_inputs(tmp_path: Path) -> None:
    archive = _archive(tmp_path)
    info = MODULE.stable_hash_path(archive)

    payloads, rollout, rows = MODULE.extract_privacy_release(
        archive, info, source=SOURCE
    )

    assert payloads == _payloads()
    assert hashlib.sha256(rollout).hexdigest()
    assert set(rows) == set(MODULE.PRIVACY_INPUTS)
    assert all(rows[name]["size"] == len(payloads[name]) for name in rows)


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (
            lambda manifest: manifest["privacy_bootstrap_release"]["plan"].update(
                sha256="00" * 32
            ),
            "digest mismatch",
        ),
        (
            lambda manifest: manifest["privacy_bootstrap_release"].update(
                secret_free=False
            ),
            "not natively recomposed",
        ),
        (
            lambda manifest: manifest["privacy_bootstrap_release"][
                "peer_1_config"
            ].update(designated_validator="taira-validator-2"),
            "not designated",
        ),
        (
            lambda manifest: manifest["privacy_bootstrap_release"]["genesis"].update(
                path="configs/soranexus/taira/genesis.json"
            ),
            "changed path",
        ),
        (
            lambda manifest: manifest["privacy_bootstrap_release"].update(
                unknown=True
            ),
            "fields are not exact",
        ),
        (
            lambda manifest: manifest.update(git_head="12" * 20),
            "differs from the authenticated release source",
        ),
        (
            lambda manifest: manifest.update(
                dpn_validator_release_commit="34" * 20
            ),
            "differs from the authenticated release source",
        ),
    ),
)
def test_rejects_adversarial_rollout_manifest_mutations(
    tmp_path: Path, mutation, message: str
) -> None:
    archive = _archive(tmp_path, mutate_manifest=mutation)

    with pytest.raises(MODULE.PrivacyReleaseExtractionError, match=message):
        MODULE.extract_privacy_release(
            archive, MODULE.stable_hash_path(archive), source=SOURCE
        )


def test_rejects_operator_copy_substitution(tmp_path: Path) -> None:
    archive = _archive(tmp_path, operator_mismatch="genesis.json")

    with pytest.raises(
        MODULE.PrivacyReleaseExtractionError, match="operator copy differs"
    ):
        MODULE.extract_privacy_release(
            archive, MODULE.stable_hash_path(archive), source=SOURCE
        )


def test_rejects_duplicate_and_missing_privacy_members(tmp_path: Path) -> None:
    duplicate = _archive(tmp_path / "duplicate", duplicate="config.toml")
    with pytest.raises(MODULE.PrivacyReleaseExtractionError, match="repeats member"):
        MODULE.extract_privacy_release(
            duplicate, MODULE.stable_hash_path(duplicate), source=SOURCE
        )

    missing = _archive(
        tmp_path / "missing",
        omit=str(MODULE.PRIVACY_INPUTS["config.toml"]["rollout_path"]),
    )
    with pytest.raises(MODULE.PrivacyReleaseExtractionError, match="omits"):
        MODULE.extract_privacy_release(
            missing, MODULE.stable_hash_path(missing), source=SOURCE
        )


@pytest.mark.parametrize("kind", ("symlink", "hardlink", "traversal"))
def test_rejects_links_and_noncanonical_archive_paths(
    tmp_path: Path, kind: str
) -> None:
    prefix = "taira-rollout-test-linux-aarch64"
    if kind == "traversal":
        member = tarfile.TarInfo(f"{prefix}/../escape")
        member.size = 0
    else:
        member = tarfile.TarInfo(f"{prefix}/unsafe-{kind}")
        member.type = tarfile.SYMTYPE if kind == "symlink" else tarfile.LNKTYPE
        member.linkname = "target"
        member.size = 0
    archive = _archive(tmp_path, extra_member=member)

    with pytest.raises(MODULE.PrivacyReleaseExtractionError):
        MODULE.extract_privacy_release(
            archive, MODULE.stable_hash_path(archive), source=SOURCE
        )


def test_run_authenticates_before_extraction_and_publishes_all_or_none(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    private = tmp_path / "private"
    private.mkdir(mode=0o700)
    archive = private / "release.tar.gz"
    archive.write_bytes(b"authenticated archive")
    archive.chmod(0o600)
    authority = private / "authority"
    authority.mkdir(mode=0o700)
    verifier = private / "verifier"
    verifier.write_bytes(b"native verifier")
    verifier.chmod(0o700)
    output = private / "release"
    verifier_sha = hashlib.sha256(verifier.read_bytes()).hexdigest()
    info = MODULE.stable_hash_path(archive)
    events: list[str] = []
    payloads = _payloads()
    rows = {
        name: {
            "rollout_path": contract["rollout_path"],
            "sha256": hashlib.sha256(payloads[name]).hexdigest(),
            "size": len(payloads[name]),
        }
        for name, contract in MODULE.PRIVACY_INPUTS.items()
    }

    def authenticate(*_args, **_kwargs):
        events.append("authenticate")
        return info, {"manifest_sha256": "11" * 32}

    def extract(*_args, **_kwargs):
        assert events == ["authenticate"]
        events.append("extract")
        return payloads, b"rollout\n", rows

    monkeypatch.setattr(MODULE, "authenticate_linux_release", authenticate)
    monkeypatch.setattr(MODULE, "extract_privacy_release", extract)
    args = argparse.Namespace(
        archive=archive,
        authority_dir=authority,
        source_commit=SOURCE.commit,
        dpn_validator_release_commit=SOURCE.dpn_validator_release_commit,
        cargo_lock_sha256=SOURCE.cargo_lock_sha256,
        workspace_source_manifest_sha256=SOURCE.workspace_source_manifest_sha256,
        trusted_signing_fingerprint="22" * 32,
        release_manifest_verifier=verifier,
        trusted_release_manifest_verifier_sha256=verifier_sha,
        output_dir=output,
    )

    result = MODULE.run(args)

    assert events == ["authenticate", "extract"]
    assert result["output_dir"] == str(output)
    assert MODULE.scan_inventory_paths(output) == sorted(
        {MODULE.OUTPUT_MANIFEST, *MODULE.PRIVACY_INPUTS}
    )


def test_run_leaves_no_partial_output_when_extraction_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    private = tmp_path / "private"
    private.mkdir(mode=0o700)
    archive = private / "release.tar.gz"
    archive.write_bytes(b"authenticated archive")
    archive.chmod(0o600)
    authority = private / "authority"
    authority.mkdir(mode=0o700)
    verifier = private / "verifier"
    verifier.write_bytes(b"native verifier")
    verifier.chmod(0o700)
    output = private / "release"
    verifier_sha = hashlib.sha256(verifier.read_bytes()).hexdigest()
    info = MODULE.stable_hash_path(archive)
    monkeypatch.setattr(
        MODULE,
        "authenticate_linux_release",
        lambda *_args, **_kwargs: (info, {"manifest_sha256": "11" * 32}),
    )
    monkeypatch.setattr(
        MODULE,
        "extract_privacy_release",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(
            MODULE.PrivacyReleaseExtractionError("malicious tar")
        ),
    )

    with pytest.raises(MODULE.PrivacyReleaseExtractionError, match="malicious"):
        MODULE.run(
            argparse.Namespace(
                archive=archive,
                authority_dir=authority,
                source_commit=SOURCE.commit,
                dpn_validator_release_commit=(
                    SOURCE.dpn_validator_release_commit
                ),
                cargo_lock_sha256=SOURCE.cargo_lock_sha256,
                workspace_source_manifest_sha256=SOURCE.workspace_source_manifest_sha256,
                trusted_signing_fingerprint="22" * 32,
                release_manifest_verifier=verifier,
                trusted_release_manifest_verifier_sha256=verifier_sha,
                output_dir=output,
            )
        )
    assert not output.exists()
