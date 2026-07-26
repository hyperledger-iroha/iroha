from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path

import pytest

from scripts import (
    publish_plan,
    release_artifact_contract,
    release_manifest_signing,
)


TARGETS = {
    "iroha2": "sorafs://releases/iroha2/v1.0.0",
    "iroha3": "sorafs://releases/iroha3/v1.0.0",
}


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _write_release(
    tmp_path: Path,
    files: dict[str, tuple[str, bytes]] | None = None,
) -> tuple[Path, Path, dict[str, str]]:
    inventory = files or {
        "iroha2-linux.tar.zst": ("iroha2", b"i2-bytes"),
        "iroha3-linux.tar.zst": ("iroha3", b"i3-bytes"),
    }
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    rows: list[dict[str, object]] = []
    checksums: list[str] = []
    targets: dict[str, str] = {}
    for relative, (profile, payload) in sorted(inventory.items()):
        path = artifacts / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        path.chmod(0o644)
        digest = _sha256(payload)
        checksums.append(f"{digest}  {relative}\n")
        rows.append(
            {
                "profile": profile,
                "target": "x86_64-unknown-linux-gnu",
                "kind": "bundle",
                "format": "tar.zst",
                "path": relative,
                "sha256": digest,
                "size": len(payload),
            }
        )
        targets[profile] = TARGETS[profile]
    (artifacts / "SHA256SUMS").write_text("".join(checksums), encoding="ascii")
    manifest = {
        "schema": release_artifact_contract.RELEASE_MANIFEST_SCHEMA,
        "schema_version": (
            release_artifact_contract.RELEASE_MANIFEST_SCHEMA_VERSION
        ),
        "version": "1.0.0",
        "commit": "a" * 40,
        "source_date_epoch": 1,
        "built_at": "1970-01-01T00:00:01Z",
        "os": "linux",
        "arch": "x86_64",
        "artifacts": rows,
    }
    manifest_path = tmp_path / "release_manifest.json"
    manifest_path.write_bytes(
        release_artifact_contract.canonical_json_bytes(manifest)
    )
    manifest_path.chmod(0o644)
    return artifacts, manifest_path, targets


def _sign_manifest(
    tmp_path: Path,
    manifest_path: Path,
) -> tuple[Path, Path, str, Path, str]:
    public = bytes.fromhex(
        "2152f8d19b791d24453242e15f2eab6c"
        "b7cffa7b6a5ed30097960e069881db12"
    )
    public_input = tmp_path / "manifest-signing-public.raw"
    signer = tmp_path / "external-ed25519-signer"
    verifier = tmp_path / "sorafs-validate"
    public_input.write_bytes(public)
    public_input.chmod(0o600)
    signer.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"public_key = bytes.fromhex({public.hex()!r})\n"
        "payload = Path(sys.argv[1]).read_bytes()\n"
        "digest = hashlib.sha512(public_key + payload).digest()\n"
        "r = (int.from_bytes(digest[:32], 'little') % "
        "((1 << 255) - 19)).to_bytes(32, 'little')\n"
        "s = (int.from_bytes(digest[32:], 'little') % "
        "((1 << 252) + 27742317777372353535851937790883648493))"
        ".to_bytes(32, 'little')\n"
        "Path(sys.argv[2]).write_bytes(r + s)\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)
    verifier.write_text(
        "#!/usr/bin/env python3\n"
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        "args = sys.argv[1:]\n"
        "if len(args) != 9 or args[0] != 'release-manifest':\n"
        "    raise SystemExit(4)\n"
        "options = dict(zip(args[1::2], args[2::2]))\n"
        "payload = Path(options['--manifest']).read_bytes()\n"
        "public_key = Path(options['--public-key']).read_bytes()\n"
        "signature = Path(options['--signature']).read_bytes()\n"
        "if hashlib.sha256(public_key).hexdigest() != "
        "options['--public-key-fingerprint']:\n"
        "    raise SystemExit(2)\n"
        "digest = hashlib.sha512(public_key + payload).digest()\n"
        "r = (int.from_bytes(digest[:32], 'little') % "
        "((1 << 255) - 19)).to_bytes(32, 'little')\n"
        "s = (int.from_bytes(digest[32:], 'little') % "
        "((1 << 252) + 27742317777372353535851937790883648493))"
        ".to_bytes(32, 'little')\n"
        "if signature != r + s:\n"
        "    raise SystemExit(2)\n",
        encoding="utf-8",
    )
    verifier.chmod(0o700)
    fingerprint = _sha256(public)
    verifier_digest = _sha256(verifier.read_bytes())
    signature = tmp_path / "release_manifest.json.sig"
    public_output = tmp_path / "release_manifest.json.pub"
    release_manifest_signing.sign_release_manifest(
        manifest_path,
        signer,
        public_input,
        fingerprint,
        signature,
        public_output,
        verifier,
        verifier_digest,
    )
    return signature, public_output, fingerprint, verifier, verifier_digest


def _signed_inputs(
    tmp_path: Path,
    manifest_path: Path,
) -> dict[str, object]:
    signature, public_key, fingerprint, verifier, verifier_digest = (
        _sign_manifest(tmp_path, manifest_path)
    )
    return {
        "manifest_signature_path": signature,
        "manifest_public_key_path": public_key,
        "trusted_signing_fingerprint": fingerprint,
        "release_manifest_verifier_path": verifier,
        "trusted_release_manifest_verifier_sha256": verifier_digest,
    }


def _validate(
    plan_path: Path,
    artifacts: Path,
    manifest: Path,
    targets: dict[str, str],
    **kwargs: object,
) -> dict[str, object]:
    return publish_plan.validate_publish_plan(
        plan_path,
        manifest_path=manifest,
        artifacts_dir=artifacts,
        target_map=targets,
        **kwargs,
    )


def test_parse_target_map_is_canonical_and_duplicate_closed() -> None:
    assert publish_plan.parse_target_map(["sorafs://releases"]) == {
        "iroha2": "sorafs://releases",
        "iroha3": "sorafs://releases",
        "shared": "sorafs://releases",
    }
    assert publish_plan.parse_target_map(
        [
            "iroha2=sorafs://releases/i2",
            "iroha3=https://gateway.example/releases/i3",
        ]
    ) == {
        "iroha2": "sorafs://releases/i2",
        "iroha3": "https://gateway.example/releases/i3",
    }
    for values in (
        [],
        ["iroha2="],
        ["iroha2=sorafs://one", "iroha2=sorafs://two"],
        ["unknown=sorafs://one"],
        ["http://gateway.example/releases"],
        ["sorafs://releases/../escape"],
        ["sorafs://releases/%2fescape"],
        ["sorafs://releases?query=1"],
        ["sorafs://releases/\nattack"],
    ):
        with pytest.raises(publish_plan.PublishPlanError):
            publish_plan.parse_target_map(values)


def test_signed_plan_build_write_and_independent_replay(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    signed = _signed_inputs(tmp_path, manifest)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        **signed,
    )
    assert plan["generated_at"] == "1970-01-01T00:00:01Z"
    assert plan["manifest_verification_mode"] == "ed25519"
    assert plan["manifest_signature_verified"] is True
    outputs = publish_plan.write_plan_files(plan, tmp_path)
    assert outputs["json"].read_bytes() == (
        release_artifact_contract.canonical_json_bytes(plan)
    )
    assert "printf" in outputs["sh"].read_text(encoding="utf-8")
    report = _validate(
        outputs["json"],
        artifacts,
        manifest,
        targets,
        **signed,
    )
    assert report["status"] == "ok"
    assert report["plan_sha256"] == _sha256(outputs["json"].read_bytes())


def test_plan_generation_is_byte_identical_for_same_inputs(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    first = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    second = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    assert release_artifact_contract.canonical_json_bytes(first) == (
        release_artifact_contract.canonical_json_bytes(second)
    )


def test_unsigned_plan_requires_explicit_development_mode(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    with pytest.raises(publish_plan.PublishPlanError, match="production publish"):
        publish_plan.build_publish_plan(manifest, artifacts, targets)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    path = publish_plan.write_plan_files(plan, tmp_path)["json"]
    with pytest.raises(publish_plan.PublishPlanError):
        _validate(path, artifacts, manifest, targets)
    assert (
        _validate(
            path,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )["status"]
        == "ok"
    )


@pytest.mark.parametrize("kind", ("symlink", "hardlink", "unsafe", "fifo"))
def test_plan_rejects_linked_or_special_artifacts(
    tmp_path: Path,
    kind: str,
) -> None:
    artifacts, manifest, targets = _write_release(
        tmp_path,
        {"iroha2-linux.tar.zst": ("iroha2", b"bytes")},
    )
    artifact = artifacts / "iroha2-linux.tar.zst"
    if kind == "symlink":
        artifact.unlink()
        target = tmp_path / "outside"
        target.write_bytes(b"bytes")
        artifact.symlink_to(target)
    elif kind == "hardlink":
        os.link(artifact, tmp_path / "copy")
    elif kind == "unsafe":
        artifact.chmod(0o666)
    else:
        artifact.unlink()
        os.mkfifo(artifact)
    with pytest.raises(publish_plan.PublishPlanError):
        publish_plan.build_publish_plan(
            manifest,
            artifacts,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_plan_rejects_missing_extra_or_tampered_checksum_inventory(
    tmp_path: Path,
) -> None:
    artifacts, manifest, targets = _write_release(
        tmp_path,
        {"iroha2-linux.tar.zst": ("iroha2", b"bytes")},
    )
    (artifacts / "stale.tar.zst").write_bytes(b"stale")
    with pytest.raises(publish_plan.PublishPlanError, match="closed publish"):
        publish_plan.build_publish_plan(
            manifest,
            artifacts,
            targets,
            development_allow_unsigned_manifest=True,
        )
    (artifacts / "stale.tar.zst").unlink()
    (artifacts / "SHA256SUMS").write_text(
        f"{'0' * 64}  iroha2-linux.tar.zst\n",
        encoding="ascii",
    )
    with pytest.raises(publish_plan.PublishPlanError, match="SHA256 mismatch"):
        publish_plan.build_publish_plan(
            manifest,
            artifacts,
            targets,
            development_allow_unsigned_manifest=True,
        )


@pytest.mark.parametrize(
    "field",
    ("destination", "source", "sha256", "size", "target"),
)
def test_replay_rejects_plan_artifact_tampering(
    tmp_path: Path,
    field: str,
) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    row = plan["artifacts"][0]
    assert isinstance(row, dict)
    if field == "size":
        row[field] = int(row[field]) + 1
    elif field == "sha256":
        row[field] = "0" * 64
    elif field == "target":
        row[field] = "aarch64-unknown-linux-gnu"
    else:
        row[field] = str(row[field]) + "-tampered"
    plan_path = tmp_path / "publish_plan.json"
    plan_path.write_bytes(release_artifact_contract.canonical_json_bytes(plan))
    with pytest.raises(publish_plan.PublishPlanError):
        _validate(
            plan_path,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_replay_requires_independent_roots_targets_and_manifest(
    tmp_path: Path,
) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]
    with pytest.raises(publish_plan.PublishPlanError, match="independent"):
        publish_plan.validate_publish_plan(
            plan_path,
            development_allow_unsigned_manifest=True,
        )
    wrong_targets = dict(targets)
    wrong_targets["iroha2"] = "sorafs://other/i2"
    with pytest.raises(publish_plan.PublishPlanError, match="independently"):
        _validate(
            plan_path,
            artifacts,
            manifest,
            wrong_targets,
            development_allow_unsigned_manifest=True,
        )
    alias = tmp_path / "artifact-alias"
    alias.symlink_to(artifacts, target_is_directory=True)
    with pytest.raises(publish_plan.PublishPlanError):
        _validate(
            plan_path,
            alias,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_replay_rejects_noncanonical_or_duplicate_key_plan(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    compact = tmp_path / "compact.json"
    compact.write_text(
        json.dumps(plan, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(publish_plan.PublishPlanError, match="not canonical"):
        _validate(
            compact,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )
    duplicate = tmp_path / "duplicate.json"
    payload = release_artifact_contract.canonical_json_bytes(plan)
    duplicate.write_bytes(payload.replace(b"{\n", b'{\n  "schema": "duplicate",\n', 1))
    with pytest.raises(publish_plan.PublishPlanError, match="duplicate JSON"):
        _validate(
            duplicate,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_replay_rejects_hardlinked_or_substituted_plan_path(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    first_dir = tmp_path / "first"
    first_dir.mkdir()
    plan_path = publish_plan.write_plan_files(plan, first_dir)["json"]
    os.link(plan_path, tmp_path / "plan-hardlink")
    with pytest.raises(publish_plan.PublishPlanError, match="hard link"):
        _validate(
            plan_path,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )
    (tmp_path / "plan-hardlink").unlink()

    original_hash = publish_plan.stable_hash_path
    replaced = tmp_path / "replaced-plan.json"
    raced = False

    def substitute(path: Path):
        nonlocal raced
        if path == plan_path and not raced:
            raced = True
            path.rename(replaced)
            path.write_bytes(replaced.read_bytes())
        return original_hash(path)

    monkeypatch.setattr(publish_plan, "stable_hash_path", substitute)
    with pytest.raises(publish_plan.PublishPlanError, match="changed while"):
        _validate(
            plan_path,
            artifacts,
            manifest,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_write_plan_files_refuses_existing_or_linked_outputs(
    tmp_path: Path,
) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    output = tmp_path / "out"
    output.mkdir()
    sentinel = output / "publish_plan.json"
    sentinel.write_text("preserve", encoding="utf-8")
    with pytest.raises(publish_plan.PublishPlanError, match="must all be new"):
        publish_plan.write_plan_files(plan, output)
    assert sentinel.read_text(encoding="utf-8") == "preserve"


def test_plan_rejects_control_character_paths_and_targets(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    bad_targets = dict(targets)
    bad_targets["iroha2"] = "sorafs://releases/i2\nprintf-pwned"
    with pytest.raises(publish_plan.PublishPlanError):
        publish_plan.build_publish_plan(
            manifest,
            artifacts,
            bad_targets,
            development_allow_unsigned_manifest=True,
        )
    newline_root = tmp_path / "bad\nroot"
    artifacts.rename(newline_root)
    with pytest.raises(publish_plan.PublishPlanError, match="control"):
        publish_plan.build_publish_plan(
            manifest,
            newline_root,
            targets,
            development_allow_unsigned_manifest=True,
        )


def test_probe_command_does_not_shell_expand_destination(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(
        tmp_path,
        {"iroha2-linux.tar.zst": ("iroha2", b"bytes")},
    )
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]
    probe = tmp_path / "probe"
    observed = tmp_path / "observed"
    probe.write_text(
        "#!/usr/bin/env python3\n"
        "import json, sys\n"
        "from pathlib import Path\n"
        f"Path({str(observed)!r}).write_text(sys.argv[1])\n"
        "print(json.dumps({'size': 5}))\n",
        encoding="utf-8",
    )
    probe.chmod(0o755)
    report = _validate(
        plan_path,
        artifacts,
        manifest,
        targets,
        probe_remote=True,
        probe_command=f"{probe} {{destination}}",
        development_allow_unsigned_manifest=True,
    )
    assert report["status"] == "ok"
    assert observed.read_text(encoding="utf-8").startswith("sorafs://")


def test_previous_plan_diff_requires_canonical_plan(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        development_allow_unsigned_manifest=True,
    )
    current_dir = tmp_path / "current"
    current_dir.mkdir()
    plan_path = publish_plan.write_plan_files(plan, current_dir)["json"]
    previous = copy.deepcopy(plan)
    previous["artifacts"][0]["sha256"] = "f" * 64
    previous_path = tmp_path / "previous.json"
    previous_path.write_bytes(
        release_artifact_contract.canonical_json_bytes(previous)
    )
    report = _validate(
        plan_path,
        artifacts,
        manifest,
        targets,
        previous_plan_path=previous_path,
        development_allow_unsigned_manifest=True,
    )
    assert report["diff"]["changed"] == ["iroha2-linux.tar.zst"]


def test_signed_replay_requires_independent_trust_tuple(tmp_path: Path) -> None:
    artifacts, manifest, targets = _write_release(tmp_path)
    signed = _signed_inputs(tmp_path, manifest)
    plan = publish_plan.build_publish_plan(
        manifest,
        artifacts,
        targets,
        **signed,
    )
    plan_path = publish_plan.write_plan_files(plan, tmp_path)["json"]
    with pytest.raises(publish_plan.PublishPlanError, match="supplied together"):
        _validate(
            plan_path,
            artifacts,
            manifest,
            targets,
            trusted_signing_fingerprint=signed[
                "trusted_signing_fingerprint"
            ],
        )
    wrong = dict(signed)
    wrong["trusted_signing_fingerprint"] = "0" * 64
    with pytest.raises(publish_plan.PublishPlanError):
        _validate(plan_path, artifacts, manifest, targets, **wrong)
