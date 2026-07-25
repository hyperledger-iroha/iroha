"""Regression tests for release script profile validation."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
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
    common = [
        str(manifest_path),
        "iroha2",
        "single",
        "1.2.3",
        "abcdef0",
        "2026-07-22T00:00:00Z",
        "mac",
        "arm64",
    ]
    if script.name == "build_release_bundle.sh":
        arguments = [
            *common,
            unusual,
            "dist/archive.tar.zst",
            "aa" * 32,
            "",
            "",
            "",
        ]
    else:
        arguments = [
            *common,
            unusual,
            "",
            "",
            "registry.example/iroha:quoted",
            "sha256:image-id",
            "dist/image.tar",
            "bb" * 32,
            "",
            "",
            "",
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
    assert artifact["signature_algorithm"] is None
    assert artifact["public_key_format"] is None
    assert artifact["signer_fingerprint_sha256"] is None
    assert not sentinel.exists()


def test_bundle_profile_values_are_toml_escaped(tmp_path: Path) -> None:
    script = RELEASE_SCRIPTS[0]
    program = _heredoc_program(script.read_text(encoding="utf-8"), "PROFILE_PY")
    assert "${" not in program

    sentinel = tmp_path / "unexpected-profile-side-effect"
    unusual = f'path-"\\\nvalue; open("{sentinel}", "w")'
    profile_path = tmp_path / 'PROFILE-"quoted".toml'
    arguments = [
        str(profile_path),
        "iroha2",
        unusual,
        "1.2.3",
        "abcdef0",
        "2026-07-22T00:00:00Z",
        "mac",
        "arm64",
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


def _signing_program(script: Path) -> str:
    source = script.read_text(encoding="utf-8")
    return _heredoc_program(source, "SIGNING_PY")


def _run_openssl(*arguments: str) -> subprocess.CompletedProcess[str]:
    openssl = shutil.which("openssl")
    if openssl is None:
        pytest.skip("openssl is required for release-signing tests")
    return subprocess.run(
        [openssl, *arguments],
        text=True,
        capture_output=True,
        check=True,
    )


def _ed25519_material(tmp_path: Path) -> tuple[Path, Path, str]:
    private_key = tmp_path / "ed25519-private.pem"
    public_der = tmp_path / "ed25519-public.der"
    public_raw = tmp_path / "ed25519-public.raw"
    _run_openssl("genpkey", "-algorithm", "Ed25519", "-out", str(private_key))
    private_key.chmod(0o600)
    _run_openssl(
        "pkey",
        "-in",
        str(private_key),
        "-pubout",
        "-outform",
        "DER",
        "-out",
        str(public_der),
    )
    public_der_bytes = public_der.read_bytes()
    assert public_der_bytes.startswith(bytes.fromhex("302a300506032b6570032100"))
    public_raw.write_bytes(public_der_bytes[-32:])
    public_raw.chmod(0o644)
    fingerprint = hashlib.sha256(public_raw.read_bytes()).hexdigest()
    return private_key, public_raw, fingerprint


def _ed25519_signer(tmp_path: Path, private_key: Path) -> Path:
    openssl = shutil.which("openssl")
    assert openssl is not None
    signer = tmp_path / "hsm-ed25519-signer"
    signer.write_text(
        "#!/bin/sh\n"
        f"exec {shlex.quote(openssl)} pkeyutl -sign "
        f"-inkey {shlex.quote(str(private_key))} "
        '-rawin -in "$1" -out "$2"\n',
        encoding="utf-8",
    )
    signer.chmod(0o700)
    return signer


def _fixed_signature_signer(tmp_path: Path, size: int, byte: int = 0x5A) -> Path:
    signer = tmp_path / f"fixed-signature-{size}"
    signer.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"Path(sys.argv[2]).write_bytes(bytes([{byte}]) * {size})\n",
        encoding="utf-8",
    )
    signer.chmod(0o700)
    return signer


def _run_signing_program(
    script: Path,
    artifact: Path,
    signer: Path,
    public_key: Path,
    fingerprint: str,
    signature_out: Path,
    public_out: Path,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-",
            str(artifact),
            str(signer),
            str(public_key),
            fingerprint,
            str(signature_out),
            str(public_out),
        ],
        input=_signing_program(script),
        text=True,
        capture_output=True,
        check=False,
    )


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_signing_contract_accepts_only_verified_ed25519(
    tmp_path: Path,
    script: Path,
) -> None:
    private_key, public_key, fingerprint = _ed25519_material(tmp_path)
    signer = _ed25519_signer(tmp_path, private_key)
    artifact = tmp_path / "release-artifact"
    artifact.write_bytes(b"canonical release bytes\n")
    artifact.chmod(0o644)
    signature_out = tmp_path / "release-artifact.sig"
    public_out = tmp_path / "release-artifact.pub"

    result = _run_signing_program(
        script,
        artifact,
        signer,
        public_key,
        fingerprint,
        signature_out,
        public_out,
    )

    assert result.returncode == 0, result.stderr
    assert len(signature_out.read_bytes()) == 64
    _run_openssl(
        "pkeyutl",
        "-verify",
        "-pubin",
        "-inkey",
        str(public_out),
        "-rawin",
        "-in",
        str(artifact),
        "-sigfile",
        str(signature_out),
    )


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_signing_contract_rejects_untrusted_fingerprint(
    tmp_path: Path,
    script: Path,
) -> None:
    private_key, public_key, _fingerprint = _ed25519_material(tmp_path)
    signer = _ed25519_signer(tmp_path, private_key)
    artifact = tmp_path / "release-artifact"
    artifact.write_bytes(b"release")

    result = _run_signing_program(
        script,
        artifact,
        signer,
        public_key,
        "0" * 64,
        tmp_path / "artifact.sig",
        tmp_path / "artifact.pub",
    )

    assert result.returncode != 0
    assert "does not match the reviewed fingerprint" in result.stderr
    assert not (tmp_path / "artifact.sig").exists()
    assert not (tmp_path / "artifact.pub").exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
@pytest.mark.parametrize(
    ("signature_size", "expected"),
    (
        (63, "must contain exactly 64 raw bytes"),
        (64, "signature verification failed"),
    ),
)
def test_release_signing_contract_rejects_malformed_or_forged_signature(
    tmp_path: Path,
    script: Path,
    signature_size: int,
    expected: str,
) -> None:
    _private_key, public_key, fingerprint = _ed25519_material(tmp_path)
    signer = _fixed_signature_signer(tmp_path, signature_size)
    artifact = tmp_path / "release-artifact"
    artifact.write_bytes(b"release")

    result = _run_signing_program(
        script,
        artifact,
        signer,
        public_key,
        fingerprint,
        tmp_path / "artifact.sig",
        tmp_path / "artifact.pub",
    )

    assert result.returncode != 0
    assert expected in result.stderr
    assert not (tmp_path / "artifact.sig").exists()
    assert not (tmp_path / "artifact.pub").exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
@pytest.mark.parametrize("unsafe_input", ("signer-symlink", "public-key-mode"))
def test_release_signing_contract_rejects_symlink_and_unsafe_permissions(
    tmp_path: Path,
    script: Path,
    unsafe_input: str,
) -> None:
    private_key, public_key, fingerprint = _ed25519_material(tmp_path)
    signer = _ed25519_signer(tmp_path, private_key)
    if unsafe_input == "signer-symlink":
        signer_link = tmp_path / "signer-link"
        signer_link.symlink_to(signer)
        signer = signer_link
    else:
        public_key.chmod(0o666)
    artifact = tmp_path / "release-artifact"
    artifact.write_bytes(b"release")

    result = _run_signing_program(
        script,
        artifact,
        signer,
        public_key,
        fingerprint,
        tmp_path / "artifact.sig",
        tmp_path / "artifact.pub",
    )

    assert result.returncode != 0
    if unsafe_input == "signer-symlink":
        assert "must not contain a symlink path component" in result.stderr
    else:
        assert "must not be group- or world-writable" in result.stderr
    assert not (tmp_path / "artifact.sig").exists()
    assert not (tmp_path / "artifact.pub").exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_signing_contract_rejects_incompatible_rsa_signer(
    tmp_path: Path,
    script: Path,
) -> None:
    _private_key, public_key, fingerprint = _ed25519_material(tmp_path)
    rsa_private = tmp_path / "rsa-private.pem"
    _run_openssl("genpkey", "-algorithm", "RSA", "-out", str(rsa_private))
    rsa_private.chmod(0o600)
    signer = _ed25519_signer(tmp_path, rsa_private)
    artifact = tmp_path / "release-artifact"
    artifact.write_bytes(b"release")

    result = _run_signing_program(
        script,
        artifact,
        signer,
        public_key,
        fingerprint,
        tmp_path / "artifact.sig",
        tmp_path / "artifact.pub",
    )

    assert result.returncode != 0
    assert "must contain exactly 64 raw bytes" in result.stderr
    assert not (tmp_path / "artifact.sig").exists()
    assert not (tmp_path / "artifact.pub").exists()


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_signing_contract_never_accepts_private_key_option(script: Path) -> None:
    source = script.read_text(encoding="utf-8")
    assert "--signing-key" not in source
    assert "openssl rsa" not in source
    assert "openssl dgst" not in source
    assert "signature_algorithm" in source
    assert '"ed25519"' in source


@pytest.mark.parametrize("script", RELEASE_SCRIPTS, ids=lambda path: path.stem)
def test_release_builders_emit_portable_basename_checksum_sidecars(
    script: Path,
) -> None:
    source = script.read_text(encoding="utf-8")
    assert 'checksum_dir="$(dirname "$tarball")"' in source
    assert 'checksum_name="$(basename "$tarball")"' in source
    assert (
        '(cd "$checksum_dir" && sha256sum "$checksum_name") '
        '> "${tarball}.sha256"'
    ) in source
    assert (
        '(cd "$checksum_dir" && shasum -a 256 "$checksum_name") '
        '> "${tarball}.sha256"'
    ) in source


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


def test_release_pipeline_signs_final_manifest_before_publish_plan() -> None:
    source = (REPO_ROOT / "scripts" / "run_release_pipeline.py").read_text(
        encoding="utf-8"
    )
    main_source = source.split("def main() -> int:", 1)[1]
    update_evidence = main_source.index("update_release_manifest_evidence(")
    sign_manifest = main_source.index("sign_release_manifest(")
    build_plan = main_source.index("build_publish_plan(")
    assert update_evidence < sign_manifest < build_plan
    assert "--development-allow-unsigned-publish-plan" in main_source
    assert "production publish plans require --external-signer" in main_source


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
    for canonical in RELEASE_DOCUMENT_FAMILIES:
        guarded_paths.append(canonical)
        guarded_paths.extend(_localized_release_documents(canonical))
    for path in guarded_paths:
        source = path.read_text(encoding="utf-8")
        for stale_claim in STALE_RELEASE_SIGNING_CLAIMS:
            assert stale_claim not in source, f"{path}: stale claim {stale_claim!r}"


def test_release_signing_docs_bind_fingerprint_key_and_signature() -> None:
    expected_markers = {
        "release_dual_track_automation_plan.md": (
            "--external-signer",
            "--signing-public-key",
            "--trusted-signing-fingerprint",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "signature_algorithm=ed25519",
            "public_key_format=pem-spki-ed25519",
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
            "exactly 32 raw Ed25519 public-key bytes",
            "openssl pkeyutl -verify -pubin -rawin",
            "raw 32-byte",
            "--manifest-signature",
            "--development-allow-unsigned-manifest",
            "OIDC/cosign",
        ),
        "release_artifact_selection.md": (
            "signer_fingerprint_sha256",
            "public_key_format' \"$MANIFEST\")\" = pem-spki-ed25519",
            "302a300506032b6570032100",
            'test "$ACTUAL_SIGNING_FINGERPRINT" = "$TRUSTED_SIGNING_FINGERPRINT"',
            "openssl pkeyutl -verify -pubin -rawin",
            "scripts/release_manifest_signing.py verify",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "exactly 32 raw Ed25519 public-key bytes",
            "--development-allow-unsigned-manifest",
            "PKCS#11/HSM",
        ),
        "sora_nexus_operator_onboarding.md": (
            "signer_fingerprint_sha256",
            "302a300506032b6570032100",
            'test "$ACTUAL_SIGNING_FINGERPRINT" = "$TRUSTED_SIGNING_FINGERPRINT"',
            "openssl pkeyutl -verify -pubin -rawin",
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
            "signer_fingerprint_sha256",
            "302a300506032b6570032100",
            'test "$ACTUAL_SIGNING_FINGERPRINT" = "$TRUSTED_SIGNING_FINGERPRINT"',
            "openssl pkeyutl -verify -pubin -rawin",
            "scripts/release_manifest_signing.py verify",
            "--release-manifest-verifier",
            "--trusted-release-manifest-verifier-sha256",
            "sorafs-validate release-manifest",
            "exactly 32 raw Ed25519 bytes",
            "release_manifest.json.sig",
            "PKCS#11/HSM",
            "OIDC/cosign",
        ),
    }
    for canonical in RELEASE_DOCUMENT_FAMILIES:
        source = canonical.read_text(encoding="utf-8")
        for marker in expected_markers[canonical.name]:
            assert marker in source, f"{canonical}: missing {marker!r}"


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
