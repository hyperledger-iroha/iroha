from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from scripts import publish_plan


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
    path.write_text(json.dumps(manifest), encoding="utf-8")


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
    plan = publish_plan.build_publish_plan(
        manifest_path=manifest_path,
        artifacts_dir=artifact_dir,
        target_map={"iroha2": "sorafs://releases/iroha2/v2.0.0-rc.3", "iroha3": "sorafs://releases/iroha3/v2.0.0-rc.3"},
    )
    outputs = publish_plan.write_plan_files(plan, tmp_path)
    assert outputs["json"].exists()
    shell_body = outputs["sh"].read_text(encoding="utf-8")
    assert "upload" in shell_body
    assert "iroha2-linux.tar.zst" in shell_body
    report = publish_plan.validate_publish_plan(outputs["json"])
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
    )
    plan_paths = publish_plan.write_plan_files(plan, tmp_path)
    previous = copy.deepcopy(plan)
    previous["artifacts"][0]["sha256"] = "cafebabe"
    prev_path = tmp_path / "previous_plan.json"
    prev_path.write_text(json.dumps(previous), encoding="utf-8")

    report = publish_plan.validate_publish_plan(
        plan_path=plan_paths["json"], previous_plan_path=prev_path
    )
    assert report["status"] == "ok"
    assert report["diff"]["changed"] == [file_path.name]
