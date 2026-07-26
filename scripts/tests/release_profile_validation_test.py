"""Regression tests for release script profile validation."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import tarfile
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python < 3.11
    import tomli as tomllib

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_SCRIPTS = (
    REPO_ROOT / "scripts" / "build_release_bundle.sh",
    REPO_ROOT / "scripts" / "build_release_image.sh",
)
RELEASE_MANIFEST_SIGNING_HELPER = (
    REPO_ROOT / "scripts" / "release_manifest_signing.py"
)
RELEASE_DOCUMENT_FAMILIES = (
    REPO_ROOT / "docs" / "source" / "release_dual_track_automation_plan.md",
    REPO_ROOT / "docs" / "source" / "release_dual_track_runbook.md",
    REPO_ROOT / "docs" / "source" / "release_artifact_selection.md",
    REPO_ROOT / "docs" / "source" / "sora_nexus_operator_onboarding.md",
    REPO_ROOT
    / "docs"
    / "portal"
    / "docs"
    / "nexus"
    / "nexus-operator-onboarding.md",
)
ADDITIONAL_ACTIVE_RELEASE_DOCUMENT_FAMILIES = (
    REPO_ROOT / "docs" / "source" / "sorafs_reference_sdk_plan.md",
    REPO_ROOT / "docs" / "source" / "sorafs_release_pipeline_plan.md",
)
ACTIVE_RELEASE_DOCUMENT_FAMILIES = (
    *RELEASE_DOCUMENT_FAMILIES,
    *ADDITIONAL_ACTIVE_RELEASE_DOCUMENT_FAMILIES,
)
RELEASE_DOCUMENT_LOCALES = {
    "am",
    "ar",
    "az",
    "ba",
    "dz",
    "es",
    "fr",
    "he",
    "hy",
    "ja",
    "ka",
    "kk",
    "mn",
    "my",
    "pt",
    "ru",
    "ur",
    "uz",
    "zh-hans",
    "zh-hant",
}
STALE_RELEASE_SIGNING_CLAIMS = (
    "--signing-key",
    "RELEASE_SIGNING_KEY",
    "openssl dgst",
    "openssl rsa",
    "openssl pkeyutl",
    "openssl pkey -pubin",
    "pem-spki-ed25519",
    "302a300506032b6570032100",
    "generated Ed25519 SPKI PEM",
    "--manifest-signature-in",
    "--development-local-signing",
    "--manifest-signing-key",
    ".manifest.json.sig",
    "sign published archives or binaries",
    "release-artifacts.yml",
    "release-pipeline.yml",
    "ci/verify_release_assets.sh",
    "scripts/sdk_release_smoke.sh",
    "ci/release_metrics_check.sh",
    "--publish-bucket",
)


def _heredoc_program(source: str, delimiter: str) -> str:
    marker = f"<<'{delimiter}'\n"
    assert marker in source
    return source.split(marker, 1)[1].split(f"\n{delimiter}", 1)[0]


def _split_markdown_frontmatter(text: str) -> tuple[dict[str, str], str]:
    lines = text.splitlines()
    if not lines or lines[0] != "---":
        return {}, text
    assert lines.count("---") == 2
    end = lines.index("---", 1)
    metadata: dict[str, str] = {}
    for line in lines[1:end]:
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        metadata[key.strip()] = value.strip().strip('"').strip("'")
    body = "\n".join(lines[end + 1 :]).lstrip("\n")
    if text.endswith("\n"):
        body += "\n"
    return metadata, body


def _localized_release_documents(canonical: Path) -> list[Path]:
    return sorted(canonical.parent.glob(f"{canonical.stem}.*.md"))


def _fake_tool(directory: Path, name: str) -> None:
    tool = directory / name
    tool.write_text(
        "#!/bin/sh\n"
        'printf \'%s\\n\' "${0##*/}" >>"$RELEASE_TOOL_CALLS"\n'
        "exit 97\n",
        encoding="utf-8",
    )
    tool.chmod(0o700)


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
@pytest.mark.parametrize("profile", ("iroha4", "../escaped-release"))
def test_release_script_rejects_invalid_profile_before_tools_or_outputs(
    tmp_path: Path,
    script: Path,
    profile: str,
) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    for name in ("cargo", "docker", "zstd"):
        _fake_tool(fake_bin, name)

    tool_calls = tmp_path / "tool-calls.txt"
    artifacts_dir = tmp_path / "artifacts"
    environment = dict(os.environ)
    environment.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
            "RELEASE_TOOL_CALLS": str(tool_calls),
        }
    )

    result = subprocess.run(
        [
            "bash",
            str(script),
            "--profile",
            profile,
            "--config",
            "single",
            "--artifacts-dir",
            str(artifacts_dir),
        ],
        cwd=REPO_ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert (
        f"Unsupported profile value: {profile} (expected iroha2 or iroha3)"
        in result.stderr
    )
    assert not tool_calls.exists()
    assert not artifacts_dir.exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_manifest_values_are_passed_as_data(tmp_path: Path, script: Path) -> None:
    source = script.read_text(encoding="utf-8")
    program = _heredoc_program(source, "MANIFEST_PY")
    assert "${" not in program

    sentinel = tmp_path / "unexpected-side-effect"
    unusual = f'feature-");open("{sentinel}","w").write("bad");#\nnext'
    manifest_path = tmp_path / 'manifest-"quoted".json'
    if script.name == "build_release_bundle.sh":
        arguments = [
            str(REPO_ROOT / "scripts"),
            str(manifest_path),
            "iroha2",
            "single",
            "1.2.3",
            "a" * 40,
            "1",
            "mac",
            "arm64",
            "aarch64-apple-darwin",
            unusual,
            str(tmp_path / "archive.tar.zst"),
            hashlib.sha256(b"archive").hexdigest(),
            "bb" * 32,
        ]
        (tmp_path / "archive.tar.zst").write_bytes(b"archive")
    else:
        image_archive = tmp_path / "image.oci.tar"
        image_archive.write_bytes(b"image")
        arguments = [
            str(REPO_ROOT / "scripts"),
            str(manifest_path),
            "iroha2",
            "single",
            "1.2.3",
            "a" * 40,
            "1",
            "linux",
            "amd64",
            "x86_64-unknown-linux-gnu",
            "linux/amd64",
            unusual,
            "irohad iroha kagami",
            "closed-prebuilt",
            json.dumps({"file_count": 1, "sha256": "b" * 64}),
            f"registry.example/builder@sha256:{'c' * 64}",
            f"registry.example/runtime@sha256:{'d' * 64}",
            "e" * 64,
            "f" * 64,
            "buildx reviewed",
            "reviewed-builder",
            "1" * 64,
            "registry.example/iroha:quoted",
            json.dumps(
                {
                    "config_digest": f"sha256:{'1' * 64}",
                    "file_count": 4,
                    "layout_sha256": "2" * 64,
                    "manifest_digest": f"sha256:{'3' * 64}",
                }
                ),
                str(image_archive),
                hashlib.sha256(b"image").hexdigest(),
            ]

    result = subprocess.run(
        [sys.executable, "-", *arguments],
        input=program,
        text=True,
        capture_output=True,
        check=False,
        cwd=tmp_path,
    )

    assert result.returncode == 0, result.stderr
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["features"] == unusual
    assert manifest["profile"] == "iroha2"
    artifact = manifest["artifacts"][0]
    expected_artifact_fields = (
        {"file", "sha256", "size"}
    )
    assert set(artifact) == expected_artifact_fields
    assert not sentinel.exists()


def test_bundle_profile_values_are_toml_escaped(tmp_path: Path) -> None:
    script = RELEASE_SCRIPTS[0]
    program = _heredoc_program(script.read_text(encoding="utf-8"), "PROFILE_PY")
    assert "${" not in program

    sentinel = tmp_path / "unexpected-profile-side-effect"
    unusual = f'path-"\\\nvalue; open("{sentinel}", "w")'
    profile_path = tmp_path / 'PROFILE-"quoted".toml'
    arguments = [
        str(REPO_ROOT / "scripts"),
        str(profile_path),
        "iroha2",
        unusual,
        "1.2.3",
        "a" * 40,
        "1",
        "mac",
        "arm64",
        "aarch64-apple-darwin",
        unusual,
    ]

    result = subprocess.run(
        [sys.executable, "-", *arguments],
        input=program,
        text=True,
        capture_output=True,
        check=False,
        cwd=tmp_path,
    )

    assert result.returncode == 0, result.stderr
    with profile_path.open("rb") as profile_file:
        profile = tomllib.load(profile_file)
    assert profile["config"] == unusual
    assert profile["features"] == unusual
    assert not sentinel.exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_builders_have_no_competing_signature_format(script: Path) -> None:
    source = script.read_text(encoding="utf-8")
    for retired_marker in (
        "--external-signer",
        "--signing-public-key",
        "--trusted-signing-fingerprint",
        "--signing-key",
        "SIGNING_PY",
        "openssl",
        "BEGIN PUBLIC KEY",
        "pem-spki-ed25519",
        "signature_algorithm",
        "public_key_format",
        "signer_fingerprint_sha256",
    ):
        assert retired_marker not in source


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
@pytest.mark.parametrize(
    "retired_option",
    (
        "--external-signer",
        "--signing-public-key",
        "--trusted-signing-fingerprint",
        "--signing-key",
    ),
)
def test_release_builders_reject_retired_signing_options_before_outputs(
    tmp_path: Path,
    script: Path,
    retired_option: str,
) -> None:
    artifacts_dir = tmp_path / "artifacts"
    result = subprocess.run(
        [
            "bash",
            str(script),
            "--profile",
            "iroha2",
            "--config",
            "single",
            "--artifacts-dir",
            str(artifacts_dir),
            retired_option,
            str(tmp_path / "retired-key-or-signer"),
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 1
    assert f"Unknown argument: {retired_option}" in result.stderr
    assert not artifacts_dir.exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_builders_emit_portable_basename_checksum_sidecars(
    script: Path,
) -> None:
    source = script.read_text(encoding="utf-8")
    if script.name == "build_release_bundle.sh":
        assert "write_release_checksum.py" in source
        assert '--listed-name "$archive_name"' in source
    else:
        assert "write_release_checksum.py" in source
        assert '--listed-name "$archive_name"' in source


def test_release_pipeline_requires_complete_ed25519_signer_contract() -> None:
    scripts_dir = REPO_ROOT / "scripts"
    sys.path.insert(0, str(scripts_dir))
    try:
        import run_release_pipeline as pipeline
    finally:
        sys.path.pop(0)

    assert pipeline.release_signing_cli_args(None, None, None) == []
    with pytest.raises(pipeline.PipelineError, match="must be supplied together"):
        pipeline.release_signing_cli_args("/signer", None, None)
    with pytest.raises(pipeline.PipelineError, match="64 lowercase"):
        pipeline.release_signing_cli_args("/signer", "/public", "AA" * 32)
    assert pipeline.release_signing_cli_args(
        "/signer",
        "/public",
        "ab" * 32,
    ) == [
        "--external-signer",
        "/signer",
        "--signing-public-key",
        "/public",
        "--trusted-signing-fingerprint",
        "ab" * 32,
    ]
    rendered = pipeline.render_command(
        ["publisher", "--password", "secret\nvalue", "--label", "line\nbreak"]
    )
    assert "secret" not in rendered
    assert "\n" not in rendered
    assert "<redacted>" in rendered
    assert "line\\x0abreak" in rendered


def test_release_pipeline_signs_final_manifest_before_publish_plan() -> None:
    source = (REPO_ROOT / "scripts" / "run_release_pipeline.py").read_text(
        encoding="utf-8"
    )
    main_source = source.split("def main() -> int:", 1)[1]
    close_evidence = main_source.index("build_evidence_artifacts(")
    generate_manifest = main_source.index("generate_release_manifest.py")
    sign_manifest = main_source.index("sign_release_manifest(")
    build_plan = main_source.index("build_publish_plan(")
    assert close_evidence < generate_manifest < sign_manifest < build_plan
    assert "update_release_manifest_evidence(" not in main_source
    assert "--source-date-epoch" in main_source
    assert '["git", "rev-parse", "HEAD"]' in main_source
    assert "bundle_cmd.extend(signing_cli_args)" not in main_source
    assert "image_cmd.extend(signing_cli_args)" not in main_source
    assert "--development-allow-unsigned-publish-plan" in main_source
    assert "production publish plans require --external-signer" in main_source
    assert '"oci-archive"' in main_source
    assert '"--source-commit"' in main_source
    assert '"--image-builder-base-image"' in main_source
    assert '"--trusted-buildx-sha256"' in main_source
    assert '"--trusted-buildx-builder-inspect-sha256"' in main_source
    assert '"--image-prebuilt-bin-dir"' in main_source
    assert '"--bundle-prebuilt-bin-dir"' in main_source
    assert "RELEASE_TARGETS" in source
    assert "host_target_triple" not in source
    assert "detect_os_tag" not in source
    assert '"taira"' not in source


def test_jenkins_has_no_competing_promotable_release_path() -> None:
    source = (REPO_ROOT / "Jenkinsfile").read_text(encoding="utf-8")
    assert "Jenkins does not create promotable release artifacts" in source
    for retired in (
        "build_release_bundle.sh",
        "generate_release_manifest.py",
        "git rev-parse --short",
        "date -u",
        "sha256sum",
        "shasum",
        "archiveArtifacts artifacts: 'artifacts/**/*'",
    ):
        assert retired not in source


def test_docker_workflows_pin_buildx_tool_version() -> None:
    workflows = sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml"))
    buildx_workflows = []
    for workflow in workflows:
        source = workflow.read_text(encoding="utf-8")
        if "docker/setup-buildx-action@" not in source:
            continue
        buildx_workflows.append(workflow.name)
        assert "version: latest" not in source
        assert "version: v0.34.1" in source
    assert buildx_workflows


def test_root_dockerfile_workflows_require_digest_pinned_base_refs() -> None:
    expected_root_builds = {
        "publish_dev.yml": 1,
        "publish.yml": 3,
        "pr_docker_compose.yml": 1,
        "publish_taira_validator.yml": 2,
        # One additional custom build uses Dockerfile.musl.
        "publish_custom.yml": 2,
    }
    for name, expected in expected_root_builds.items():
        source = (
            REPO_ROOT / ".github" / "workflows" / name
        ).read_text(encoding="utf-8")
        assert "scripts/validate_release_image_bases.py" in source
        assert (
            source.count(
                '"IROHA_RUST_BUILDER_IMAGE=${{ env.IROHA_RUST_BUILDER_IMAGE }}"'
            )
            == expected
        )
        assert (
            source.count(
                '"IROHA_RUNTIME_IMAGE=${{ env.IROHA_RUNTIME_IMAGE }}"'
            )
            == expected
        )


def test_release_and_evidence_dockerfiles_have_no_mutable_base_defaults() -> None:
    expected_base_args = {
        "Dockerfile": (
            "IROHA_RUST_BUILDER_IMAGE",
            "IROHA_RUNTIME_IMAGE",
        ),
        "Dockerfile.musl": (
            "IROHA_MUSL_BUILDER_IMAGE",
            "IROHA_MUSL_RUNTIME_IMAGE",
        ),
        "Dockerfile.cross": (
            "IROHA_CROSS_XX_IMAGE",
            "IROHA_CROSS_RUST_IMAGE",
            "IROHA_CROSS_RUNTIME_IMAGE",
        ),
        "Dockerfile.build": ("IROHA_CI_BUILDER_IMAGE",),
        "scripts/fastpq/docker/Dockerfile.cpu": ("RUST_IMAGE",),
        "scripts/fastpq/docker/Dockerfile.gpu": (
            "CUDA_IMAGE",
            "RUST_IMAGE",
        ),
    }
    for relative, argument_names in expected_base_args.items():
        source = (REPO_ROOT / relative).read_text(encoding="utf-8")
        for argument_name in argument_names:
            assert f"ARG {argument_name}\n" in source
            assert f"${{{argument_name}}}" in source
            assert f"ARG {argument_name}=" not in source
        for mutable_ref in (
            r"(?m)^FROM(?:\s+--platform=\S+)?\s+archlinux:",
            r"(?m)^FROM(?:\s+--platform=\S+)?\s+alpine(?::|\s|$)",
            r"(?m)^FROM(?:\s+--platform=\S+)?\s+rust:",
            r"(?m)^FROM(?:\s+--platform=\S+)?\s+tonistiigi/",
            r"(?m)^FROM(?:\s+--platform=\S+)?\s+nvidia/",
        ):
            assert re.search(mutable_ref, source) is None


def test_legacy_image_workflows_pass_validated_digest_base_refs() -> None:
    custom = (
        REPO_ROOT / ".github" / "workflows" / "publish_custom.yml"
    ).read_text(encoding="utf-8")
    assert "Validate digest-pinned musl base images" in custom
    for retired_taira_path in (
        "iroha3-taira",
        "taira_image:",
        "validator_release_ref",
        "CONFIG_PROFILE=taira",
    ):
        assert retired_taira_path not in custom
    dedicated_taira = (
        REPO_ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    assert "environment: taira-validator-publish" in dedicated_taira
    for argument_name in (
        "IROHA_MUSL_BUILDER_IMAGE",
        "IROHA_MUSL_RUNTIME_IMAGE",
    ):
        assignment = f'"{argument_name}=${{{{ env.{argument_name} }}}}"'
        assert custom.count(assignment) == 1

    cross = (
        REPO_ROOT / ".github" / "workflows" / "publish_xx.yml"
    ).read_text(encoding="utf-8")
    assert "Validate digest-pinned cross-build base images" in cross
    for argument_name in (
        "IROHA_CROSS_XX_IMAGE",
        "IROHA_CROSS_RUST_IMAGE",
        "IROHA_CROSS_RUNTIME_IMAGE",
    ):
        assignment = f'"{argument_name}=${{{{ env.{argument_name} }}}}"'
        assert cross.count(assignment) == 2

    ci_image = (
        REPO_ROOT / ".github" / "workflows" / "ci_image.yml"
    ).read_text(encoding="utf-8")
    assert "Validate digest-pinned CI base image" in ci_image
    assert "github.event.inputs.IROHA2_CI_DOCKERFILE" not in ci_image
    assert "file: Dockerfile.build" in ci_image
    assert (
        '"IROHA_CI_BUILDER_IMAGE=${{ env.IROHA_CI_BUILDER_IMAGE }}"'
        in ci_image
    )


def test_release_build_workflow_containers_use_validated_digest_refs() -> None:
    for name in (
        "publish.yml",
        "publish_custom.yml",
        "publish_dev.yml",
        "publish_taira_validator.yml",
    ):
        source = (
            REPO_ROOT / ".github" / "workflows" / name
        ).read_text(encoding="utf-8")
        assert "image: hyperledger/iroha2-ci:" not in source
        assert "image: ${{ vars.IROHA_CI_IMAGE }}" in source
        assert "IROHA_CI_IMAGE: ${{ vars.IROHA_CI_IMAGE }}" in source
        assert '--builder "$IROHA_CI_IMAGE"' in source
        assert '--runtime "$IROHA_CI_IMAGE"' in source


def test_fastpq_repro_builder_rejects_mutable_or_missing_base_refs(
    tmp_path: Path,
) -> None:
    script = REPO_ROOT / "scripts" / "fastpq" / "repro_build.sh"
    env = os.environ.copy()
    env.pop("FASTPQ_RUST_IMAGE", None)
    env.pop("FASTPQ_CUDA_IMAGE", None)
    common = [
        "bash",
        str(script),
        "--skip-build-image",
        "--container-runtime",
        "true",
        "--output",
        str(tmp_path / "unused"),
    ]

    missing = subprocess.run(
        common,
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert missing.returncode != 0
    assert "Rust base image must be a bounded lowercase ref@sha256 digest" in (
        missing.stderr
    )

    mutable = subprocess.run(
        [*common, "--rust-image", "rust:1.88.0-slim-bookworm"],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert mutable.returncode != 0
    assert "Rust base image must be a bounded lowercase ref@sha256 digest" in (
        mutable.stderr
    )

    digest_rust = f"registry.example/rust@sha256:{'a' * 64}"
    missing_cuda = subprocess.run(
        [*common, "--mode", "gpu", "--rust-image", digest_rust],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert missing_cuda.returncode != 0
    assert "CUDA base image must be a bounded lowercase ref@sha256 digest" in (
        missing_cuda.stderr
    )

    accepted = subprocess.run(
        [*common, "--rust-image", digest_rust],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert accepted.returncode == 0, accepted.stderr


def test_release_evidence_producers_write_only_to_fresh_explicit_roots(
    tmp_path: Path,
) -> None:
    pipeline_source = (
        REPO_ROOT / "scripts" / "run_release_pipeline.py"
    ).read_text(encoding="utf-8")
    assert 'dp_env["SORANET_PRIVACY_DP_ARTIFACT_DIR"]' in pipeline_source
    assert 'smoke_env["NEXUS_LANE_SMOKE_EVIDENCE_DIR"]' in pipeline_source
    assert (
        'REPO_ROOT / "artifacts" / "soranet_privacy_dp"'
        not in pipeline_source
    )
    assert 'REPO_ROOT / "artifacts" / "nx18"' not in pipeline_source

    privacy_output = tmp_path / "privacy"
    privacy_env = os.environ.copy()
    privacy_env["SORANET_PRIVACY_DP_ARTIFACT_DIR"] = str(privacy_output)
    privacy_result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "telemetry" / "run_privacy_dp.py"),
        ],
        cwd=REPO_ROOT,
        env=privacy_env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert privacy_result.returncode == 0, privacy_result.stderr
    assert sorted(path.name for path in privacy_output.iterdir()) == [
        "summary.json",
        "suppression_matrix.csv",
    ]

    notebook = json.loads(
        (
            REPO_ROOT / "notebooks" / "soranet_privacy_dp.ipynb"
        ).read_text(encoding="utf-8")
    )
    notebook_source = "\n".join(
        "".join(cell.get("source", []))
        for cell in notebook.get("cells", [])
    )
    assert "SORANET_PRIVACY_DP_ARTIFACT_DIR" in notebook_source
    normalizer = (
        REPO_ROOT
        / "scripts"
        / "telemetry"
        / "normalize_executed_notebook.py"
    )
    normalized_paths = []
    for index, timestamp in enumerate(
        ("2026-01-01T00:00:00Z", "2026-07-25T12:34:56Z")
    ):
        candidate = tmp_path / f"executed-{index}.ipynb"
        candidate.write_text(
            json.dumps(
                {
                    "cells": [
                        {
                            "cell_type": "code",
                            "execution_count": 1,
                            "metadata": {
                                "execution": {"iopub.execute_input": timestamp},
                                "papermill": {
                                    "start_time": timestamp,
                                    "duration": index + 0.5,
                                },
                            },
                            "outputs": [{"output_type": "stream", "text": ["ok\n"]}],
                            "source": ["print('ok')"],
                        }
                    ],
                    "metadata": {
                        "papermill": {
                            "input_path": f"/host-{index}/input.ipynb",
                            "start_time": timestamp,
                        }
                    },
                    "nbformat": 4,
                    "nbformat_minor": 5,
                }
            ),
            encoding="utf-8",
        )
        normalize_result = subprocess.run(
            [
                sys.executable,
                str(normalizer),
                "--notebook",
                str(candidate),
                "--source-date-epoch",
                "1",
            ],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        assert normalize_result.returncode == 0, normalize_result.stderr
        normalized_paths.append(candidate)
    assert normalized_paths[0].read_bytes() == normalized_paths[1].read_bytes()

    existing = tmp_path / "existing"
    existing.mkdir()
    privacy_wrapper = (
        REPO_ROOT / "scripts" / "telemetry" / "run_privacy_dp_notebook.sh"
    )
    wrapper_env = os.environ.copy()
    wrapper_env["SORANET_PRIVACY_DP_ARTIFACT_DIR"] = str(existing)
    wrapper_result = subprocess.run(
        ["bash", str(privacy_wrapper)],
        cwd=REPO_ROOT,
        env=wrapper_env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert wrapper_result.returncode != 0
    assert "Refusing existing privacy DP artifact directory" in (
        wrapper_result.stderr
    )

    smoke_script = REPO_ROOT / "ci" / "check_nexus_lane_smoke.sh"
    smoke_env = os.environ.copy()
    smoke_env["NEXUS_LANE_SMOKE_EVIDENCE_DIR"] = str(existing)
    smoke_result = subprocess.run(
        ["bash", str(smoke_script)],
        cwd=REPO_ROOT,
        env=smoke_env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert smoke_result.returncode != 0
    assert "refusing existing Nexus lane evidence directory" in (
        smoke_result.stderr
    )

    fresh_smoke = tmp_path / "nx18"
    smoke_env["NEXUS_LANE_SMOKE_EVIDENCE_DIR"] = str(fresh_smoke)
    smoke_env["SOURCE_DATE_EPOCH"] = "1"
    smoke_result = subprocess.run(
        ["bash", str(smoke_script)],
        cwd=REPO_ROOT,
        env=smoke_env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert smoke_result.returncode == 0, smoke_result.stderr
    assert {
        "nx18_acceptance.json",
        "slot_bundle_manifest.json",
        "slot_summary.json",
    }.issubset(path.name for path in fresh_smoke.iterdir())
    replay_smoke = tmp_path / "nx18-replay"
    smoke_env["NEXUS_LANE_SMOKE_EVIDENCE_DIR"] = str(replay_smoke)
    replay_result = subprocess.run(
        ["bash", str(smoke_script)],
        cwd=REPO_ROOT,
        env=smoke_env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert replay_result.returncode == 0, replay_result.stderr
    first_files = {
        path.name: path.read_bytes()
        for path in fresh_smoke.iterdir()
        if path.is_file()
    }
    replay_files = {
        path.name: path.read_bytes()
        for path in replay_smoke.iterdir()
        if path.is_file()
    }
    assert replay_files == first_files


def test_release_pipeline_requires_explicit_image_contract_before_outputs(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "release-output"
    version = tomllib.loads(
        (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    )["workspace"]["package"]["version"]
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "run_release_pipeline.py"),
            "--version",
            version,
            "--output-dir",
            str(output_dir),
            "--skip-bundles",
            "--skip-privacy-dp",
            "--skip-nexus-lane-smoke",
            "--skip-nexus-cross-dataspace-proof",
            "--skip-fastpq-rollout-check",
            "--skip-cbdc-rollout-check",
            "--dry-run",
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 1
    assert "image lanes require explicit reviewed controls" in result.stderr
    assert not output_dir.exists()


def test_release_pipeline_dry_run_uses_closed_oci_image_contract(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "release-output"
    version = tomllib.loads(
        (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    )["workspace"]["package"]["version"]
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "run_release_pipeline.py"),
            "--version",
            version,
            "--source-commit",
            commit,
            "--source-date-epoch",
            "1",
            "--output-dir",
            str(output_dir),
            "--git-cliff",
            "/reviewed/git-cliff",
            "--trusted-git-cliff-sha256",
            "a" * 64,
            "--skip-bundles",
            "--image-platform",
            "linux/amd64",
            "--image-platform",
            "linux/arm64",
            "--image-builder-base-image",
            f"registry.example/builder@sha256:{'b' * 64}",
            "--image-runtime-base-image",
            f"registry.example/runtime@sha256:{'c' * 64}",
            "--image-docker",
            "/reviewed/docker",
            "--trusted-docker-sha256",
            "d" * 64,
            "--image-buildx-plugin",
            "/reviewed/docker-buildx",
            "--trusted-buildx-sha256",
            "e" * 64,
            "--trusted-buildx-version",
            "reviewed buildx version",
            "--image-buildx-builder",
            "reviewed-builder",
            "--trusted-buildx-builder-inspect-sha256",
            "f" * 64,
            "--image-prebuilt-bin-dir",
            "iroha2:linux/amd64=/reviewed/iroha2-amd64-bin",
            "--image-prebuilt-bin-dir",
            "iroha2:linux/arm64=/reviewed/iroha2-arm64-bin",
            "--image-prebuilt-bin-dir",
            "iroha3:linux/amd64=/reviewed/iroha3-amd64-bin",
            "--image-prebuilt-bin-dir",
            "iroha3:linux/arm64=/reviewed/iroha3-arm64-bin",
            "--skip-privacy-dp",
            "--skip-nexus-lane-smoke",
            "--skip-nexus-cross-dataspace-proof",
            "--skip-fastpq-rollout-check",
            "--skip-cbdc-rollout-check",
            "--dry-run",
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert "-linux-amd64-image.oci.tar" in result.stdout
    assert "-linux-arm64-image.oci.tar" in result.stdout
    assert "oci-archive" in result.stdout
    assert "--source-commit" in result.stdout
    assert "--trusted-buildx-sha256" in result.stdout
    assert "CHANGELOG-" in result.stdout
    assert not output_dir.exists()


def test_release_pipeline_dry_run_uses_complete_bundle_target_matrix(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "release-output"
    version = tomllib.loads(
        (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    )["workspace"]["package"]["version"]
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        text=True,
    ).strip()
    targets = (
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    )
    command = [
        sys.executable,
        str(REPO_ROOT / "scripts" / "run_release_pipeline.py"),
        "--version",
        version,
        "--source-commit",
        commit,
        "--source-date-epoch",
        "1",
        "--output-dir",
        str(output_dir),
        "--git-cliff",
        "/reviewed/git-cliff",
        "--trusted-git-cliff-sha256",
        "a" * 64,
        "--trusted-zstd-sha256",
        "b" * 64,
        "--zstd",
        "/reviewed/zstd",
        "--skip-images",
        "--skip-privacy-dp",
        "--skip-nexus-lane-smoke",
        "--skip-nexus-cross-dataspace-proof",
        "--skip-fastpq-rollout-check",
        "--skip-cbdc-rollout-check",
        "--dry-run",
    ]
    for profile in ("iroha2", "iroha3"):
        for target in targets:
            command.extend(
                [
                    "--bundle-prebuilt-bin-dir",
                    f"{profile}:{target}=/reviewed/{profile}/{target}",
                ]
            )
    result = subprocess.run(
        command,
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.count("--prebuilt-bin-dir") == 10
    for target in targets:
        assert target in result.stdout
    for name in (
        "dual_profile_matrix-linux-x86_64.json",
        "dual_profile_matrix-linux-aarch64.json",
        "dual_profile_matrix-mac-x86_64.json",
        "dual_profile_matrix-mac-aarch64.json",
        "dual_profile_matrix-win-x86_64.json",
    ):
        assert name in result.stdout
    assert not output_dir.exists()


def test_release_pipeline_closes_evidence_before_manifest_inventory(
    tmp_path: Path,
) -> None:
    scripts_dir = REPO_ROOT / "scripts"
    sys.path.insert(0, str(scripts_dir))
    try:
        import run_release_pipeline as pipeline
    finally:
        sys.path.pop(0)

    release_root = tmp_path / "release"
    artifact_dir = release_root / "artifacts"
    evidence_stage = release_root / ".evidence-staging"
    artifact_dir.mkdir(parents=True)
    evidence_file = evidence_stage / "orderbook" / "summary.json"
    evidence_file.parent.mkdir(parents=True)
    evidence_file.write_text('{"status":"ready"}\n', encoding="utf-8")
    evidence_file.chmod(0o644)
    zstd = tmp_path / "zstd"
    zstd.write_text(
        "#!/usr/bin/env python3\n"
        "import shutil, sys\n"
        "shutil.copyfileobj(sys.stdin.buffer, sys.stdout.buffer)\n",
        encoding="utf-8",
    )
    zstd.chmod(0o755)
    zstd_digest = hashlib.sha256(zstd.read_bytes()).hexdigest()

    specs = pipeline.build_evidence_artifacts(
        evidence_stage=evidence_stage,
        release_root=release_root,
        artifact_dir=artifact_dir,
        version="1.2.3",
        commit="a" * 40,
        source_date_epoch=1,
        zstd_path=str(zstd),
        trusted_zstd_sha256=zstd_digest,
        dry_run=False,
    )

    archive = artifact_dir / "release-evidence-1.2.3.tar.zst"
    inventory_path = artifact_dir / "release-evidence-1.2.3.json"
    checksum_path = artifact_dir / "release-evidence-1.2.3.tar.zst.sha256"
    assert archive.is_file()
    assert inventory_path.is_file()
    assert checksum_path.is_file()
    assert not evidence_stage.exists()
    assert not (release_root / ".evidence-normalized").exists()
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    assert inventory["file_count"] == 1
    assert inventory["files"][0]["path"] == "evidence/orderbook/summary.json"
    with tarfile.open(archive, "r:") as handle:
        assert "release-evidence-1.2.3/evidence/orderbook/summary.json" in (
            member.name for member in handle.getmembers()
        )
    assert len(specs) == 3
    assert any(":release-evidence:tar.zst:" in spec for spec in specs)


def test_release_pipeline_rejects_unsigned_production_plan_before_outputs(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "release-output"
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "run_release_pipeline.py"),
            "--version",
            "0.0.0-test",
            "--output-dir",
            str(output_dir),
            "--publish-target",
            "sorafs://release-test",
            "--dry-run",
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 1
    assert "production publish plans require --external-signer" in result.stderr
    assert not output_dir.exists()


@pytest.mark.parametrize(
    "contract_args",
    [
        [
            "--external-signer",
            "/reviewed/external-signer",
            "--signing-public-key",
            "/reviewed/release-public.raw",
            "--trusted-signing-fingerprint",
            "a" * 64,
        ],
        [
            "--release-manifest-verifier",
            "/reviewed/sorafs-validate",
            "--trusted-release-manifest-verifier-sha256",
            "b" * 64,
        ],
    ],
)
def test_release_pipeline_rejects_unpaired_signer_and_verifier_contracts(
    tmp_path: Path,
    contract_args: list[str],
) -> None:
    output_dir = tmp_path / "release-output"
    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "run_release_pipeline.py"),
            "--version",
            "0.0.0-test",
            "--output-dir",
            str(output_dir),
            "--publish-target",
            "sorafs://release-test",
            "--dry-run",
            *contract_args,
        ],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 1
    assert (
        "aggregate signing requires both the complete external signer contract "
        "and the pinned native release-manifest verifier contract"
    ) in result.stderr
    assert not output_dir.exists()


def test_aggregate_manifest_signing_helper_is_strict_ed25519_only() -> None:
    source = RELEASE_MANIFEST_SIGNING_HELPER.read_text(encoding="utf-8")
    for marker in (
        "sorafs-validate",
        "release-manifest",
        "--public-key-fingerprint",
        "raw-ed25519-32",
        "native release-manifest verifier",
        "trusted native verifier SHA256",
        "exactly one hard link",
        "O_NOFOLLOW",
        "trusted signing fingerprint",
        "external Ed25519 signer",
    ):
        assert marker in source
    assert "openssl" not in source.lower()
    assert "BEGIN PUBLIC KEY" not in source


def test_release_signing_docs_are_schema_closed_and_mirrors_require_review() -> None:
    for canonical in RELEASE_DOCUMENT_FAMILIES:
        canonical_bytes = canonical.read_bytes()
        canonical_text = canonical_bytes.decode("utf-8")
        canonical_metadata, canonical_body = _split_markdown_frontmatter(
            canonical_text
        )
        if canonical_metadata:
            expected_body = canonical_body
        else:
            expected_body = canonical_text
        expected_hash = hashlib.sha256(canonical_bytes).hexdigest()
        mirrors = _localized_release_documents(canonical)
        assert {
            mirror.name.removeprefix(f"{canonical.stem}.").removesuffix(".md")
            for mirror in mirrors
        } == RELEASE_DOCUMENT_LOCALES
        for mirror in mirrors:
            mirror_text = mirror.read_text(encoding="utf-8")
            metadata, body = _split_markdown_frontmatter(mirror_text)
            assert metadata["source"] == canonical.relative_to(REPO_ROOT).as_posix()
            assert metadata["status"] == "needs-review"
            assert metadata["generator"] == "scripts/sync_docs_i18n.py"
            assert re.fullmatch(r"[0-9a-f]{64}", metadata["source_hash"])
            assert metadata["source_hash"] != expected_hash
            assert re.fullmatch(
                r"[0-9]{4}-[0-9]{2}-[0-9]{2}",
                metadata["translation_last_reviewed"],
            )
            assert body == expected_body
            if canonical_metadata:
                for key in ("id", "title", "description"):
                    assert metadata[key] == canonical_metadata[key]


def test_release_signing_docs_reject_stale_rsa_and_private_key_claims() -> None:
    guarded_paths = [
        *RELEASE_SCRIPTS,
        RELEASE_MANIFEST_SIGNING_HELPER,
        REPO_ROOT / "scripts" / "run_release_pipeline.py",
    ]
    for canonical in ACTIVE_RELEASE_DOCUMENT_FAMILIES:
        guarded_paths.append(canonical)
        guarded_paths.extend(_localized_release_documents(canonical))
    for path in guarded_paths:
        source = path.read_text(encoding="utf-8")
        for stale_claim in STALE_RELEASE_SIGNING_CLAIMS:
            assert stale_claim.casefold() not in source.casefold(), (
                f"{path}: stale claim {stale_claim!r}"
            )


def test_release_signing_docs_bind_fingerprint_key_and_signature() -> None:
    expected_markers = {
        "release_dual_track_automation_plan.md": (
            "--external-signer",
            "--signing-public-key",
            "--trusted-signing-fingerprint",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "builders do not sign artifacts",
            "public_key_format=raw-ed25519-32",
            "sorafs-validate release-manifest",
            "release_manifest.json.sig",
            "--development-allow-unsigned-publish-plan",
            "PKCS#11/HSM",
            "OIDC/cosign",
        ),
        "release_dual_track_runbook.md": (
            "--external-signer",
            "--signing-public-key",
            "--trusted-signing-fingerprint",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "Ed25519 public-key bytes",
            "Builders do not invoke signers",
            "canonical raw Ed25519 bytes",
            "--manifest-signature",
            "--development-allow-unsigned-manifest",
            "OIDC/cosign",
        ),
        "release_artifact_selection.md": (
            "builders deliberately expose no signing interface",
            "release_manifest.json.sig",
            "EXPECTED_SHA256",
            "scripts/release_manifest_signing.py verify",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "exactly 32 raw Ed25519 public-key bytes",
            "--development-allow-unsigned-manifest",
            "PKCS#11/HSM",
        ),
        "sora_nexus_operator_onboarding.md": (
            "Builders emit no per-artifact key or signature sidecars",
            "EXPECTED_SHA256",
            "scripts/release_manifest_signing.py verify",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "exactly 32 raw Ed25519 bytes",
            "release_manifest.json.sig",
            "PKCS#11/HSM",
            "OIDC/cosign",
        ),
        "nexus-operator-onboarding.md": (
            "Builders emit no per-artifact key or signature sidecars",
            "EXPECTED_SHA256",
            "scripts/release_manifest_signing.py verify",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "exactly 32 raw Ed25519 bytes",
            "release_manifest.json.sig",
            "PKCS#11/HSM",
            "OIDC/cosign",
        ),
        "sorafs_reference_sdk_plan.md": (
            "artifact/checksum producer",
            "canonical aggregate `release_manifest.json`",
            "scripts/release_manifest_signing.py",
            "PKCS#11/HSM",
            "sorafs-validate release-manifest",
        ),
        "sorafs_release_pipeline_plan.md": (
            "deterministic unsigned artifact/checksum producer",
            "canonical aggregate `release_manifest.json`",
            "scripts/release_manifest_signing.py",
            "PKCS#11/HSM",
            "aggregate-manifest signature tuple",
        ),
    }
    for canonical in ACTIVE_RELEASE_DOCUMENT_FAMILIES:
        source = canonical.read_text(encoding="utf-8")
        for marker in expected_markers[canonical.name]:
            assert marker in source, f"{canonical}: missing {marker!r}"


def test_release_artifact_selection_documents_current_matrix_names() -> None:
    canonical = REPO_ROOT / "docs" / "source" / "release_artifact_selection.md"
    guarded_paths = [canonical, *_localized_release_documents(canonical)]
    for path in guarded_paths:
        source = path.read_text(encoding="utf-8")
        for marker in (
            "<profile>-<version>-<os>-<arch>.tar.zst",
            "<profile>-<version>-<os>-<arch>-manifest.json",
            "<profile>-<version>-linux-<arch>-image.oci.tar",
            "explicit `linux/amd64` and `linux/arm64` platform matrix",
            "Taira is intentionally not a",
        ):
            assert marker in source, f"{path}: missing {marker!r}"
        for stale in (
            "<profile>-<version>-<os>.tar.zst",
            "<profile>-<version>-manifest.json",
            "<profile>-<version>-<os>-image.tar",
        ):
            assert stale not in source, f"{path}: stale {stale!r}"


def test_sorafs_release_gate_runs_generic_release_signing_guard() -> None:
    gate = (REPO_ROOT / "ci" / "check_sorafs_cli_release.sh").read_text(
        encoding="utf-8"
    )
    assert "scripts/tests/release_profile_validation_test.py" in gate
    assert "scripts/tests/release_manifest_signing_test.py" in gate
    assert "scripts/tests/release_manifest_signing_test.sh" in gate
    assert "scripts/tests/generate_release_manifest_test.py" in gate
    assert "scripts/tests/publish_plan_test.py" in gate
    assert "scripts/build_release_bundle.sh" in gate
    assert "scripts/build_release_image.sh" in gate
