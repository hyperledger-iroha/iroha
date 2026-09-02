from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tarfile
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "build_release_image.sh"
VERSION = "2.0.0-rc.2.0"
EPOCH = 1_234_567_890
BUILDX_VERSION = "github.com/docker/buildx v0.99.0 reviewed"
BUILDX_BUILDER = "reviewed-builder"
BUILDER_INSPECT = (
    "Name: reviewed-builder\n"
    "Driver: docker-container\n"
    "BuildKit version: v0.99.0\n"
    "Platforms: linux/amd64\n"
)
BUILDER_INSPECT_SHA256 = hashlib.sha256(BUILDER_INSPECT.encode()).hexdigest()
BUILDER_BASE = f"registry.example/iroha-builder@sha256:{'1' * 64}"
RUNTIME_BASE = f"registry.example/iroha-runtime@sha256:{'2' * 64}"


def _write_executable(path: Path, payload: str) -> Path:
    path.write_text(payload, encoding="utf-8")
    path.chmod(0o755)
    return path


def _write_clean_git_shim(directory: Path) -> None:
    real_git = shutil.which("git")
    assert real_git is not None
    _write_executable(
        directory / "git",
        "#!/usr/bin/env python3\n"
        "import os, sys\n"
        f"real_git = {real_git!r}\n"
        "arguments = sys.argv[1:]\n"
        "if arguments and arguments[0] == 'diff':\n"
        "    counter_path = os.environ.get('IROHA_TEST_GIT_DIFF_COUNTER')\n"
        "    count = 1\n"
        "    if counter_path:\n"
        "        try:\n"
        "            count = int(open(counter_path, encoding='ascii').read()) + 1\n"
        "        except FileNotFoundError:\n"
        "            pass\n"
        "        with open(counter_path, 'w', encoding='ascii') as counter:\n"
        "            counter.write(str(count))\n"
        "    dirty_after = int(os.environ.get('IROHA_TEST_GIT_DIRTY_AFTER', '0'))\n"
        "    if os.environ.get('IROHA_TEST_GIT_ALWAYS_DIRTY') == '1' or (\n"
        "        dirty_after and count > dirty_after\n"
        "    ):\n"
        "        raise SystemExit(1)\n"
        "if arguments == ['ls-files', '--cached', '-z']:\n"
        "    arguments = ['ls-files', '--cached', '--others', '-z']\n"
        "os.execv(real_git, [real_git, *arguments])\n",
    )


def _fixture(tmp_path: Path) -> tuple[Path, Path, str, Path, str, Path]:
    binaries = tmp_path / "binaries"
    binaries.mkdir()
    for name in (
        "iroha3d",
        "iroha3d_taira",
        "sorafs_governance_dag",
        "iroha",
        "kagami",
        "attachment_sanitizer",
        "sorafs_external_software_signer",
    ):
        _write_executable(
            binaries / name,
            f"#!/bin/sh\nprintf '%s\\n' {name}\n",
        )
    docker = _write_executable(
        tmp_path / "docker",
        "#!/usr/bin/env python3\n"
        "import hashlib, json, os, pathlib, sys\n"
        "args = sys.argv[1:]\n"
        "with pathlib.Path(os.environ['FAKE_DOCKER_LOG']).open('a', encoding='utf-8') as log:\n"
        "    log.write(json.dumps(args, separators=(',', ':')) + '\\n')\n"
        "if args == ['buildx', 'version']:\n"
        "    print(os.environ['FAKE_BUILDX_VERSION'])\n"
        "    raise SystemExit(0)\n"
        "if args == ['buildx', 'inspect', '--builder', os.environ['FAKE_BUILDX_BUILDER'], '--bootstrap']:\n"
        "    print(os.environ['FAKE_BUILDER_INSPECT'], end='')\n"
        "    raise SystemExit(0)\n"
        "if args[:2] != ['buildx', 'build']:\n"
        "    raise SystemExit(91)\n"
        "output = args[args.index('--output') + 1]\n"
        "fields = dict(item.split('=', 1) for item in output.split(','))\n"
        "root = pathlib.Path(fields['dest'])\n"
        "root.mkdir(parents=True)\n"
        "(root / 'blobs' / 'sha256').mkdir(parents=True)\n"
        "platform = args[args.index('--platform') + 1]\n"
        "os_name, arch = platform.split('/', 1)\n"
        "def encoded(value):\n"
        "    return (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\\n').encode()\n"
        "def blob(payload):\n"
        "    digest = hashlib.sha256(payload).hexdigest()\n"
        "    (root / 'blobs' / 'sha256' / digest).write_bytes(payload)\n"
        "    return digest, len(payload)\n"
        "layer_digest, layer_size = blob(b'deterministic-layer')\n"
        "config_payload = encoded({'architecture': arch, 'os': os_name, 'rootfs': {'type': 'layers', 'diff_ids': ['sha256:' + layer_digest]}})\n"
        "config_digest, config_size = blob(config_payload)\n"
        "manifest_payload = encoded({'schemaVersion': 2, 'mediaType': 'application/vnd.oci.image.manifest.v1+json', 'config': {'mediaType': 'application/vnd.oci.image.config.v1+json', 'digest': 'sha256:' + config_digest, 'size': config_size}, 'layers': [{'mediaType': 'application/vnd.oci.image.layer.v1.tar', 'digest': 'sha256:' + layer_digest, 'size': layer_size}]})\n"
        "manifest_digest, manifest_size = blob(manifest_payload)\n"
        "index = {'schemaVersion': 2, 'mediaType': 'application/vnd.oci.image.index.v1+json', 'manifests': [{'mediaType': 'application/vnd.oci.image.manifest.v1+json', 'digest': 'sha256:' + manifest_digest, 'size': manifest_size, 'annotations': {'org.opencontainers.image.ref.name': fields['name']}, 'platform': {'architecture': arch, 'os': os_name}}]}\n"
        "(root / 'index.json').write_bytes(encoded(index))\n"
        "(root / 'oci-layout').write_bytes(encoded({'imageLayoutVersion': '1.0.0'}))\n"
        "if os.environ.get('FAKE_OCI_EXTRA_BLOB') == '1':\n"
        "    (root / 'blobs' / 'sha256' / ('f' * 64)).write_bytes(b'extra')\n",
    )
    buildx = _write_executable(
        tmp_path / "docker-buildx",
        "#!/bin/sh\nexit 0\n",
    )
    docker_digest = hashlib.sha256(docker.read_bytes()).hexdigest()
    buildx_digest = hashlib.sha256(buildx.read_bytes()).hexdigest()
    log = tmp_path / "docker-calls.jsonl"
    _write_clean_git_shim(tmp_path)
    return binaries, docker, docker_digest, buildx, buildx_digest, log


def _authenticated_prebuilt(
    binaries: Path, *, destination: Path, commit: str
) -> tuple[Path, str]:
    package_by_binary = {
        "iroha3d": "irohad",
        "iroha3d_taira": "irohad",
        "sorafs_governance_dag": "irohad",
        "iroha": "iroha_cli",
        "kagami": "iroha_kagami",
        "attachment_sanitizer": "iroha_torii",
        "sorafs_external_software_signer": "irohad",
    }
    destination.mkdir()
    rows = []
    for name in sorted(package_by_binary):
        source = binaries / name
        binary = destination / name
        payload = source.read_bytes()
        binary.write_bytes(payload)
        binary.chmod(0o755)
        if source.stat().st_nlink != 1:
            os.link(binary, destination.with_name(f"{destination.name}-{name}.link"))
        rows.append(
            {
                "name": name,
                "package": package_by_binary[name],
                "sha256": hashlib.sha256(payload).hexdigest(),
                "size": len(payload),
            }
        )
    manifest = {
        "schema": "iroha.release_prebuilt_provenance",
        "schema_version": 1,
        "source_commit": commit,
        "cargo_lock_sha256": hashlib.sha256(
            (REPO_ROOT / "Cargo.lock").read_bytes()
        ).hexdigest(),
        "target": "x86_64-unknown-linux-gnu",
        "cargo_profile": "deploy",
        "default_features": True,
        "selected_features": ["irohad/external-software-signer-bin"],
        "binaries": rows,
    }
    payload = (
        json.dumps(
            manifest,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode()
    path = destination / "release-prebuilt-provenance.json"
    path.write_bytes(payload)
    path.chmod(0o644)
    return destination, hashlib.sha256(payload).hexdigest()


def _run(
    output: Path,
    binaries: Path,
    docker: Path,
    docker_digest: str,
    buildx: Path,
    buildx_digest: str,
    log: Path,
    *,
    epoch: str = str(EPOCH),
    builder_base: str = BUILDER_BASE,
    runtime_base: str = RUNTIME_BASE,
    buildx_version: str = BUILDX_VERSION,
    builder_inspect_sha256: str = BUILDER_INSPECT_SHA256,
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    output.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["PATH"] = (
        f"{binaries.parent}{os.pathsep}{environment['PATH']}"
    )
    environment.update(
        {
            "FAKE_BUILDX_VERSION": BUILDX_VERSION,
            "FAKE_BUILDX_BUILDER": BUILDX_BUILDER,
            "FAKE_BUILDER_INSPECT": BUILDER_INSPECT,
            "FAKE_DOCKER_LOG": str(log),
        }
    )
    environment.update(extra_env or {})
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    authenticated_binaries, provenance_digest = _authenticated_prebuilt(
        binaries,
        destination=output.with_name(f".{output.name}-prebuilt"),
        commit=commit,
    )
    return subprocess.run(
        [
            str(SCRIPT),
            "--source-commit",
            commit,
            "--source-date-epoch",
            epoch,
            "--platform",
            "linux/amd64",
            "--builder-base-image",
            builder_base,
            "--runtime-base-image",
            runtime_base,
            "--docker",
            str(docker),
            "--trusted-docker-sha256",
            docker_digest,
            "--buildx-plugin",
            str(buildx),
            "--trusted-buildx-sha256",
            buildx_digest,
            "--trusted-buildx-version",
            buildx_version,
            "--buildx-builder",
            BUILDX_BUILDER,
            "--trusted-buildx-builder-inspect-sha256",
            builder_inspect_sha256,
            "--prebuilt-bin-dir",
            str(authenticated_binaries),
            "--trusted-prebuilt-provenance-sha256",
            provenance_digest,
            "--artifacts-dir",
            str(output),
        ],
        cwd=REPO_ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )


def _outputs(root: Path) -> dict[str, Path]:
    stem = f"iroha3-{VERSION}-linux-amd64-image.oci.tar"
    return {
        "archive": root / stem,
        "checksum": root / f"{stem}.sha256",
        "manifest": root / f"iroha3-{VERSION}-linux-amd64-image.json",
    }


def test_image_replay_is_byte_identical_and_oci_archive_is_normalized(
    tmp_path: Path,
) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first = _run(
        first_root,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
    )
    second = _run(
        second_root,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
    )
    assert first.returncode == second.returncode == 0, first.stderr + second.stderr
    for key, first_path in _outputs(first_root).items():
        assert first_path.read_bytes() == _outputs(second_root)[key].read_bytes()

    outputs = _outputs(first_root)
    with tarfile.open(outputs["archive"], "r:") as archive:
        members = archive.getmembers()
        assert [member.name for member in members] == sorted(
            member.name for member in members
        )
        assert {"index.json", "oci-layout"} <= {
            member.name for member in members
        }
        assert all(member.mtime == EPOCH for member in members)
        assert all(member.uid == member.gid == 0 for member in members)
        assert all(member.uname == member.gname == "" for member in members)
        assert all(member.mode in {0o644, 0o755} for member in members)

    manifest = json.loads(outputs["manifest"].read_text(encoding="utf-8"))
    assert manifest["profile"] == "iroha3"
    assert manifest["config"] == "nexus"
    assert len(manifest["commit"]) == 40
    assert manifest["source_date_epoch"] == EPOCH
    assert manifest["built_at"] == "2009-02-13T23:31:30Z"
    assert manifest["platform"] == "linux/amd64"
    assert manifest["target"] == "x86_64-unknown-linux-gnu"
    assert manifest["source_context"]["kind"] == "closed-prebuilt"
    assert manifest["source_context"]["file_count"] > 0
    assert manifest["external_software_signer"] == {
        "backend": "software",
        "binary": "/usr/local/bin/sorafs_external_software_signer",
        "broker_alias": "/usr/local/libexec/iroha-runtime-provider-broker-v1",
        "smoke": "native-build-stage",
        "windows_supported": False,
    }
    assert manifest["base_images"] == {
        "builder": BUILDER_BASE,
        "runtime": RUNTIME_BASE,
    }
    assert manifest["builder"]["docker"]["sha256"] == docker_digest
    assert manifest["builder"]["buildx"]["sha256"] == buildx_digest
    assert manifest["builder"]["buildx"]["version"] == BUILDX_VERSION
    assert manifest["builder"]["buildx"]["builder"] == BUILDX_BUILDER
    assert (
        manifest["builder"]["buildx"]["builder_inspect_sha256"]
        == BUILDER_INSPECT_SHA256
    )
    assert manifest["builder"]["network"] == "none"
    assert manifest["artifacts"][0]["file"] == outputs["archive"].name
    assert outputs["checksum"].read_text(encoding="ascii").endswith(
        f"  {outputs['archive'].name}\n"
    )

    calls = [
        json.loads(line)
        for line in log.read_text(encoding="utf-8").splitlines()
    ]
    builds = [call for call in calls if call[:2] == ["buildx", "build"]]
    assert len(builds) == 2
    for call in builds:
        assert "--provenance=false" in call
        assert "--sbom=false" in call
        assert "--no-cache" in call
        assert call[call.index("--builder") + 1] == BUILDX_BUILDER
        assert call[call.index("--network") + 1] == "none"
        output = call[call.index("--output") + 1]
        assert output.startswith("type=oci,")
        assert "tar=false" in output
        assert "rewrite-timestamp=true" in output
        assert "save" not in call
        assert (
            "BINARIES=iroha3d iroha3d_taira sorafs_governance_dag iroha kagami "
            "attachment_sanitizer sorafs_external_software_signer"
        ) in call


def test_image_refuses_stale_output_without_replacement(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    output = tmp_path / "out"
    output.mkdir()
    archive = _outputs(output)["archive"]
    archive.write_bytes(b"preserve")
    result = _run(
        output,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
    )
    assert result.returncode != 0
    assert "refusing stale reuse" in result.stderr
    assert archive.read_bytes() == b"preserve"


def test_image_rejects_dirty_reviewed_source_before_outputs(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    output = tmp_path / "out"
    result = _run(
        output,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        extra_env={"IROHA_TEST_GIT_ALWAYS_DIRTY": "1"},
    )
    assert result.returncode != 0
    assert "tracked working-tree drift" in result.stderr
    assert not _outputs(output)["archive"].exists()


def test_image_rechecks_reviewed_source_after_manifest(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    output = tmp_path / "out"
    result = _run(
        output,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        extra_env={
            "IROHA_TEST_GIT_DIFF_COUNTER": str(tmp_path / "git-diff-count"),
            "IROHA_TEST_GIT_DIRTY_AFTER": "1",
        },
    )
    assert result.returncode != 0
    assert "tracked working-tree drift" in result.stderr
    assert _outputs(output)["manifest"].is_file()


def test_image_rejects_untrusted_docker_before_invocation(tmp_path: Path) -> None:
    binaries, docker, _, buildx, buildx_digest, log = _fixture(tmp_path)
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        "0" * 64,
        buildx,
        buildx_digest,
        log,
    )
    assert result.returncode != 0
    assert "Docker CLI SHA256 is not trusted" in result.stderr
    assert not log.exists()
    assert not _outputs(tmp_path / "out")["archive"].exists()


def test_image_rejects_wrong_buildx_version(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        buildx_version="different reviewed version",
    )
    assert result.returncode != 0
    assert "does not match the reviewed exact version" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


def test_image_rejects_unreviewed_buildx_builder_state(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        builder_inspect_sha256="0" * 64,
    )
    assert result.returncode != 0
    assert "does not match the reviewed exact state" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


@pytest.mark.parametrize(
    "builder_base,runtime_base",
    (
        ("registry.example/builder:latest", RUNTIME_BASE),
        (BUILDER_BASE, "registry.example/runtime:latest"),
    ),
)
def test_image_requires_digest_pinned_base_images(
    tmp_path: Path,
    builder_base: str,
    runtime_base: str,
) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        builder_base=builder_base,
        runtime_base=runtime_base,
    )
    assert result.returncode != 0
    assert "exact sha256 digest" in result.stderr
    assert not log.exists()


def test_image_rejects_hardlinked_prebuilt_binary(tmp_path: Path) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    os.link(binaries / "iroha", tmp_path / "binary-hardlink")
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
    )
    assert result.returncode != 0
    assert "exactly one hard link" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


def test_image_rejects_unreachable_oci_blob_and_scrubs_archive(
    tmp_path: Path,
) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    output = tmp_path / "out"
    result = _run(
        output,
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        extra_env={"FAKE_OCI_EXTRA_BLOB": "1"},
    )
    assert result.returncode != 0
    assert "not exactly the reachable image graph" in result.stderr
    assert not _outputs(output)["archive"].exists()


@pytest.mark.parametrize("epoch", ("", "-1", "+1", "01", "4294967296"))
def test_image_rejects_invalid_epoch(tmp_path: Path, epoch: str) -> None:
    binaries, docker, docker_digest, buildx, buildx_digest, log = _fixture(
        tmp_path
    )
    result = _run(
        tmp_path / "out",
        binaries,
        docker,
        docker_digest,
        buildx,
        buildx_digest,
        log,
        epoch=epoch,
    )
    assert result.returncode != 0
    assert "source-date-epoch" in result.stderr or "SOURCE_DATE_EPOCH" in result.stderr
    assert not _outputs(tmp_path / "out")["archive"].exists()


def test_image_source_has_no_nondeterministic_docker_archive_path() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    dockerfile = (REPO_ROOT / "Dockerfile").read_text(encoding="utf-8")
    for marker in (
        "docker save",
        "docker image inspect",
        "git rev-parse --short",
        "date -u",
        "sha256sum",
        "shasum",
        "--use-target-prebuilt",
    ):
        assert marker not in source
    assert "build_release_oci_archive.py" in source
    assert "write_release_checksum.py" in source
    assert "rewrite-timestamp=true" in source
    assert "--network none" in source
    assert "network default" not in source
    assert "IROHA_VALIDATOR_RELEASE_VERIFIED" not in source
    assert "configs/sorafs/external_software_signer" in source
    assert "configs/sorafs/runtime_provider_broker" in source
    assert "sorafs_external_software_signer" in dockerfile
    assert 'ARG BINARIES="iroha3d iroha3d_taira ' in dockerfile
    assert 'test -x "${BIN_PATH}/iroha3d_taira"' in dockerfile
    assert "/usr/local/libexec/iroha-runtime-provider-broker-v1" in dockerfile
    assert "ARG IROHA_RUST_BUILDER_IMAGE\n" in dockerfile
    assert "ARG IROHA_RUNTIME_IMAGE\n" in dockerfile
    assert "rust:slim-bookworm" not in dockerfile
    assert "debian:bookworm-slim" not in dockerfile
