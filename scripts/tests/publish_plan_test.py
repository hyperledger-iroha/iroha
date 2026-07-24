from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from scripts import publish_plan, release_manifest_signing


def _sha256_bytes(data: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(data)
    return digest.hexdigest()


def test_parse_target_map_supports_default_and_profile_specific() -> None:
    assert publish_plan.parse_target_map(["sorafs://releases"]) == {
        "iroha2": "sorafs://releases",
        "iroha3": "sorafs://releases",
    }
    assert publish_plan.parse_target_map(["iroha2=sorafs://i2", "iroha3=sorafs://i3"]) == {
        "iroha2": "sorafs://i2",
        "iroha3": "sorafs://i3",
    }
    with pytest.raises(publish_plan.PublishPlanError):
        publish_plan.parse_target_map([])
    with pytest.raises(publish_plan.PublishPlanError):
        publish_plan.parse_target_map(["iroha2="])


def _write_manifest(path: Path, artifacts: list[dict[str, object]]) -> None:
    manifest = {
        "version": "2.0.0-rc.3",
        "commit": "abcdef0",
        "built_at": "2026-03-01T00:00:00Z",
        "os": "linux",
        "arch": "x86_64",
        "artifacts": artifacts,
    }
    path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _sign_manifest(
    tmp_path: Path,
    manifest_path: Path,
) -> tuple[Path, Path, str, Path, str]:
    raw = bytes.fromhex(
        "2152f8d19b791d24453242e15f2eab6c"
        "b7cffa7b6a5ed30097960e069881db12"
    )
    raw_public_key = tmp_path / "manifest-signing-public.raw"
    signer = tmp_path / "external-ed25519-signer.sh"
    raw_public_key.write_bytes(raw)
    raw_public_key.chmod(0o600)
    signer.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"public_key = bytes.fromhex({raw.hex()!r})\n"
        "manifest = Path(sys.argv[1]).read_bytes()\n"
        "Path(sys.argv[2]).write_bytes(hashlib.sha512(public_key + manifest).digest())\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)
    fingerprint = hashlib.sha256(raw).hexdigest()
    verifier = tmp_path / "sorafs-validate"
    verifier.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        "args = sys.argv[1:]\n"
        "if len(args) != 9 or args[0] != 'release-manifest':\n"
        "    raise SystemExit(4)\n"
        "options = dict(zip(args[1::2], args[2::2]))\n"
        "manifest = Path(options['--manifest']).read_bytes()\n"
        "public_key = Path(options['--public-key']).read_bytes()\n"
        "signature = Path(options['--signature']).read_bytes()\n"
        "if len(public_key) != 32 or len(signature) != 64:\n"
        "    raise SystemExit(2)\n"
        "if hashlib.sha256(public_key).hexdigest() != "
        "options['--public-key-fingerprint']:\n"
        "    raise SystemExit(2)\n"
        "if signature != hashlib.sha512(public_key + manifest).digest():\n"
        "    raise SystemExit(2)\n",
        encoding="utf-8",
    )
    verifier.chmod(0o700)
    verifier_digest = hashlib.sha256(verifier.read_bytes()).hexdigest()
    signature = tmp_path / "release_manifest.json.sig"
    public_key = tmp_path / "release_manifest.json.pub"
    release_manifest_signing.sign_release_manifest(
        manifest_path,
        signer,
        raw_public_key,
        fingerprint,
        signature,
        public_key,
        verifier,
        verifier_digest,
    )
    return signature, public_key, fingerprint, verifier, verifier_digest


def test_build_and_validate_publish_plan(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    i2 = artifact_dir / "iroha2-linux.tar.zst"
    i3 = artifact_dir / "iroha3-linux.tar.zst"
    i2.write_bytes(b"i2-bytes")
    i3.write_bytes(b"i3-bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": i2.name,
                "sha256": _sha256_bytes(b"i2-bytes"),
            },
            {
                "profile": "iroha3",
                "kind": "bundle",
                "format": "tar.zst",
                "path": i3.name,
                "sha256": _sha256_bytes(b"i3-bytes"),
            },
        ],
    )
    signature, public_key, fingerprint, verifier, verifier_digest = _sign_manifest(
        tmp_path,
        manifest_path,
    )
    plan = publish_plan.build_publish_plan(
        manifest_path=manifest_path,
        artifacts_dir=artifact_dir,
        target_map={"iroha2": "sorafs://releases/iroha2/v2.0.0-rc.3", "iroha3": "sorafs://releases/iroha3/v2.0.0-rc.3"},
        manifest_signature_path=signature,
        manifest_public_key_path=public_key,
        trusted_signing_fingerprint=fingerprint,
        release_manifest_verifier_path=verifier,
        trusted_release_manifest_verifier_sha256=verifier_digest,
    )
    assert plan["manifest_verification_mode"] == "ed25519"
    assert plan["manifest_signature_verified"] is True
    assert plan["manifest_signer_fingerprint_sha256"] == fingerprint
    assert plan["manifest_public_key_format"] == "raw-ed25519-32"
    assert plan["manifest_native_verifier_sha256"] == verifier_digest
    outputs = publish_plan.write_plan_files(plan, tmp_path)
    assert outputs["json"].exists()
    shell_body = outputs["sh"].read_text(encoding="utf-8")
    assert "upload" in shell_body
    assert "iroha2-linux.tar.zst" in shell_body
    report = publish_plan.validate_publish_plan(
        outputs["json"],
        trusted_signing_fingerprint=fingerprint,
        release_manifest_verifier_path=verifier,
        trusted_release_manifest_verifier_sha256=verifier_digest,
    )
    assert report["status"] == "ok"
    assert all(result["local_status"] == "ok" for result in report["results"])


def test_probe_command_is_used_for_non_http_targets(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    i2 = artifact_dir / "iroha2-linux.tar.zst"
    data = b"bytes"
    i2.write_bytes(data)
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": i2.name,
                "sha256": _sha256_bytes(data),
            }
        ],
    )
    plan = publish_plan.build_publish_plan(
        manifest_path=manifest_path,
        artifacts_dir=artifact_dir,
        target_map={"iroha2": "sorafs://releases/iroha2/v2.0.0-rc.3"},
        development_allow_unsigned_manifest=True,
    )
    plan_paths = publish_plan.write_plan_files(plan, tmp_path)

    probe_script = tmp_path / "probe.sh"
    probe_script.write_text(
        "#!/usr/bin/env bash\n"
        "echo '{\"size\": " + str(len(data)) + "}'\n",
        encoding="utf-8",
    )
    probe_script.chmod(0o755)

    report = publish_plan.validate_publish_plan(
        plan_path=plan_paths["json"],
        probe_remote=True,
        probe_command=f"{probe_script} {{destination}}",
        development_allow_unsigned_manifest=True,
    )
    assert report["status"] == "ok"
    assert report["results"][0]["remote_status"] == "ok"


def test_build_publish_plan_rejects_sha_mismatch(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    file_path = artifact_dir / "iroha2-linux.tar.zst"
    file_path.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": file_path.name,
                "sha256": "deadbeef",
            }
        ],
    )
    with pytest.raises(publish_plan.PublishPlanError):
        publish_plan.build_publish_plan(
            manifest_path=manifest_path,
            artifacts_dir=artifact_dir,
            target_map={"iroha2": "sorafs://releases/iroha2"},
            development_allow_unsigned_manifest=True,
        )


def test_validate_publish_plan_reports_diff(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    file_path = artifact_dir / "iroha2-linux.tar.zst"
    file_path.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    sha = _sha256_bytes(b"bytes")
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": file_path.name,
                "sha256": sha,
            }
        ],
    )
    plan = publish_plan.build_publish_plan(
        manifest_path=manifest_path,
        artifacts_dir=artifact_dir,
        target_map={"iroha2": "sorafs://releases/iroha2/v2.0.0-rc.3"},
        development_allow_unsigned_manifest=True,
    )
    plan_paths = publish_plan.write_plan_files(plan, tmp_path)
    previous = copy.deepcopy(plan)
    previous["artifacts"][0]["sha256"] = "cafebabe"
    prev_path = tmp_path / "previous_plan.json"
    prev_path.write_text(json.dumps(previous), encoding="utf-8")

    report = publish_plan.validate_publish_plan(
        plan_path=plan_paths["json"],
        previous_plan_path=prev_path,
        development_allow_unsigned_manifest=True,
    )
    assert report["status"] == "ok"
    assert report["diff"]["changed"] == [file_path.name]


def test_unsigned_manifest_is_rejected_for_production_plan(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    artifact = artifact_dir / "iroha2-linux.tar.zst"
    artifact.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": artifact.name,
                "sha256": _sha256_bytes(b"bytes"),
            }
        ],
    )

    with pytest.raises(
        publish_plan.PublishPlanError,
        match="production publish plans require",
    ):
        publish_plan.build_publish_plan(
            manifest_path,
            artifact_dir,
            {"iroha2": "sorafs://releases/iroha2"},
        )


def test_unsigned_plan_validation_requires_explicit_development_mode(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    artifact = artifact_dir / "iroha2-linux.tar.zst"
    artifact.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": artifact.name,
                "sha256": _sha256_bytes(b"bytes"),
            }
        ],
    )
    plan = publish_plan.build_publish_plan(
        manifest_path,
        artifact_dir,
        {"iroha2": "sorafs://releases/iroha2"},
        development_allow_unsigned_manifest=True,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]

    with pytest.raises(
        publish_plan.PublishPlanError,
        match="test/development-only",
    ):
        publish_plan.validate_publish_plan(plan_path)
    assert (
        publish_plan.validate_publish_plan(
            plan_path,
            development_allow_unsigned_manifest=True,
        )["status"]
        == "ok"
    )


def test_signed_plan_detects_manifest_tampering(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    artifact = artifact_dir / "iroha2-linux.tar.zst"
    artifact.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": artifact.name,
                "sha256": _sha256_bytes(b"bytes"),
            }
        ],
    )
    signature, public_key, fingerprint, verifier, verifier_digest = _sign_manifest(
        tmp_path,
        manifest_path,
    )
    plan = publish_plan.build_publish_plan(
        manifest_path,
        artifact_dir,
        {"iroha2": "sorafs://releases/iroha2"},
        manifest_signature_path=signature,
        manifest_public_key_path=public_key,
        trusted_signing_fingerprint=fingerprint,
        release_manifest_verifier_path=verifier,
        trusted_release_manifest_verifier_sha256=verifier_digest,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]
    manifest_path.write_bytes(manifest_path.read_bytes() + b" ")

    report = publish_plan.validate_publish_plan(
        plan_path,
        trusted_signing_fingerprint=fingerprint,
        release_manifest_verifier_path=verifier,
        trusted_release_manifest_verifier_sha256=verifier_digest,
    )
    assert report["status"] == "failed"
    assert "signature verification failed" in report["local_failures"][0]


def test_signed_plan_validation_requires_independent_trusted_fingerprint(
    tmp_path: Path,
) -> None:
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    artifact = artifact_dir / "iroha2-linux.tar.zst"
    artifact.write_bytes(b"bytes")
    manifest_path = tmp_path / "release_manifest.json"
    _write_manifest(
        manifest_path,
        [
            {
                "profile": "iroha2",
                "kind": "bundle",
                "format": "tar.zst",
                "path": artifact.name,
                "sha256": _sha256_bytes(b"bytes"),
            }
        ],
    )
    signature, public_key, fingerprint, verifier, verifier_digest = _sign_manifest(
        tmp_path,
        manifest_path,
    )
    plan = publish_plan.build_publish_plan(
        manifest_path,
        artifact_dir,
        {"iroha2": "sorafs://releases/iroha2"},
        manifest_signature_path=signature,
        manifest_public_key_path=public_key,
        trusted_signing_fingerprint=fingerprint,
        release_manifest_verifier_path=verifier,
        trusted_release_manifest_verifier_sha256=verifier_digest,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]

    with pytest.raises(
        publish_plan.PublishPlanError,
        match="independently trusted signing fingerprint",
    ):
        publish_plan.validate_publish_plan(plan_path)
    with pytest.raises(
        publish_plan.PublishPlanError,
        match="pinned native release-manifest verifier",
    ):
        publish_plan.validate_publish_plan(
            plan_path,
            trusted_signing_fingerprint=fingerprint,
        )
    wrong_fingerprint = "0" * 64 if fingerprint != "0" * 64 else "1" * 64
    with pytest.raises(
        publish_plan.PublishPlanError,
        match="does not match the independently trusted",
    ):
        publish_plan.validate_publish_plan(
            plan_path,
            trusted_signing_fingerprint=wrong_fingerprint,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=verifier_digest,
        )


def test_production_plan_generation_requires_complete_signer_and_verifier_contracts(
    tmp_path: Path,
) -> None:
    manifest = tmp_path / "release_manifest.json"
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()

    common = {
        "manifest_path": manifest,
        "artifacts_dir": artifacts,
        "target_map": {"iroha2": "sorafs://releases/iroha2"},
    }
    with pytest.raises(
        publish_plan.PublishPlanError,
        match="must be supplied together",
    ):
        publish_plan.build_publish_plan(
            **common,
            manifest_signature_path=tmp_path / "release_manifest.json.sig",
            manifest_public_key_path=tmp_path / "release_manifest.json.pub",
            trusted_signing_fingerprint="a" * 64,
        )
    with pytest.raises(
        publish_plan.PublishPlanError,
        match="must be supplied together",
    ):
        publish_plan.build_publish_plan(
            **common,
            release_manifest_verifier_path=tmp_path / "sorafs-validate",
            trusted_release_manifest_verifier_sha256="b" * 64,
        )
