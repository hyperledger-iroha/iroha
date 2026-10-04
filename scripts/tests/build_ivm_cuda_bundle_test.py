"""Offline contract tests for the signed IVM CUDA PTX candidate builder."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import pytest

from scripts import build_ivm_cuda_bundle as bundle


OPENSSL = shutil.which("openssl")
pytestmark = pytest.mark.skipif(OPENSSL is None, reason="OpenSSL Ed25519 is unavailable")


@pytest.fixture
def workspace():
    """Keep each tool fixture beneath an owned workspace with stable ancestors."""
    parent = Path(__file__).resolve().parents[2] / "target" / "unit-tests" / "ivm-cuda-bundle"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="case-", dir=parent) as directory:
        yield Path(directory)


@pytest.fixture(autouse=True)
def synthetic_repository(workspace: Path, monkeypatch: pytest.MonkeyPatch):
    """Keep ephemeral test keys under target, outside the synthetic production root."""
    repository = workspace / "test-repository"
    repository.mkdir()
    monkeypatch.setattr(bundle, "ROOT", repository)


def _fixture(workspace: Path) -> tuple[Path, Path, Path]:
    source_dir = workspace / "sources"
    source_dir.mkdir()
    for stem in bundle.STEMS:
        (source_dir / f"{stem}.cu").write_bytes(f"// source for {stem}\n".encode())
    nvcc = workspace / "fake-nvcc"
    nvcc.write_text(
        "#!/usr/bin/env python3\n"
        "from pathlib import Path\n"
        "import sys\n"
        "if sys.argv[1:] == ['--version']:\n"
        "    sys.stdout.buffer.write(b'Pinned nvcc test output\\n')\n"
        "    sys.stderr.buffer.write(b'Pinned nvcc stderr\\n')\n"
        "    raise SystemExit(0)\n"
        "args = sys.argv[1:]\n"
        "source = Path(args[args.index('-ptx') + 1])\n"
        "target = Path(args[args.index('-o') + 1])\n"
        "assert source.read_bytes().startswith(b'// source for ')\n"
        "if '--mutate-source' in args and source.stem == 'vector':\n"
        "    source.write_bytes(source.read_bytes() + b'// changed\\n')\n"
        "marker = b'// divergent\\n' if '--diverge' in args and 'run-two' in str(target) else b''\n"
        "target.write_bytes((\n"
        "    '.version 7.8\\n.target sm_86\\n.address_size 64\\n'\n"
        "    f'.visible .entry {source.stem}() {{ ret; }}\\n'\n"
        ").encode() + marker)\n"
    )
    nvcc.chmod(0o755)
    key = workspace / "signer.pem"
    subprocess.run(
        [OPENSSL, "genpkey", "-algorithm", "Ed25519", "-out", str(key)],
        check=True,
        capture_output=True,
    )
    return source_dir, nvcc, key


def _build(workspace: Path, *, extra: tuple[str, ...] = ()) -> tuple[Path, str, str]:
    source_dir, nvcc, key = _fixture(workspace)
    candidate = workspace / "candidate"
    fingerprint, generation = bundle.build_candidate(
        source_dir=source_dir,
        output_dir=candidate,
        nvcc=nvcc,
        openssl=Path(OPENSSL),
        signing_key=key,
        image_digest=hashlib.sha256(b"independently measured image").hexdigest(),
        target_profile="arch=compute_86,code=sm_86",
        extra=extra,
    )
    return candidate, fingerprint, generation


def _independent_generation(candidate: Path) -> str:
    digest = hashlib.sha256(b"ivm-cuda-ptx-generation-v1\0")
    for stem in bundle.STEMS:
        name = stem.encode("ascii")
        ptx = (candidate / f"{stem}.ptx").read_bytes()
        digest.update(len(name).to_bytes(2, "little"))
        digest.update(name)
        digest.update(len(ptx).to_bytes(8, "little"))
        digest.update(ptx)
    return digest.hexdigest()


def _verify_signature(candidate: Path, public_der: Path) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [
            OPENSSL,
            "pkeyutl", "-verify", "-rawin", "-pubin", "-inkey", str(public_der),
            "-keyform", "DER", "-sigfile", str(candidate / "provenance.v1.sig"),
            "-in", str(candidate / "provenance.v1"),
        ],
        capture_output=True,
    )


def test_two_run_candidate_matches_independent_manifest_and_signature(workspace: Path) -> None:
    """The published bundle is byte-exact and verifies without the builder parser."""
    candidate, fingerprint, generation = _build(workspace)
    assert {path.name for path in candidate.iterdir()} == {
        *(f"{stem}.cu" for stem in bundle.STEMS),
        *(f"{stem}.ptx" for stem in bundle.STEMS),
        "provenance.v1",
        "provenance.v1.sig",
        "provenance.v1.pub",
        "evidence",
    }
    raw_public = (candidate / "provenance.v1.pub").read_bytes()
    assert len(raw_public) == 32
    assert fingerprint == hashlib.sha256(raw_public).hexdigest()
    assert len((candidate / "provenance.v1.sig").read_bytes()) == 64
    der_path = workspace / "independent-public.der"
    der_path.write_bytes(bytes.fromhex("302a300506032b6570032100") + raw_public)
    assert _verify_signature(candidate, der_path).returncode == 0

    raw_manifest = (candidate / "provenance.v1").read_bytes()
    assert raw_manifest.endswith(b"\n") and b"\r" not in raw_manifest
    lines = raw_manifest.decode("ascii").splitlines()
    version_bytes = b"Pinned nvcc test output\nPinned nvcc stderr\n"
    assert lines[:7] == [
        "ivm-cuda-ptx-provenance-v1",
        f"cuda_image_sha256={hashlib.sha256(b'independently measured image').hexdigest()}",
        f"nvcc_version_sha256={hashlib.sha256(version_bytes).hexdigest()}",
        "nvcc_flags=-ptx -std=c++14 -gencode arch=compute_86,code=sm_86",
        "target_profile=arch=compute_86,code=sm_86",
        f"generation_1_sha256={generation}",
        f"generation_2_sha256={generation}",
    ]
    assert generation == _independent_generation(candidate)
    expected = lines[:7]
    for stem in bundle.STEMS:
        expected.extend(
            (
                f"artifact.{stem}.source_sha256="
                + hashlib.sha256((candidate / f"{stem}.cu").read_bytes()).hexdigest(),
                f"artifact.{stem}.ptx_sha256="
                + hashlib.sha256((candidate / f"{stem}.ptx").read_bytes()).hexdigest(),
            )
        )
    assert lines == expected
    evidence = json.loads((candidate / "evidence/record.json").read_bytes())
    assert evidence["image_attestation_verified"] is False
    assert evidence["hardware_qualified"] is False
    assert evidence["manifest_sha256"] == hashlib.sha256(raw_manifest).hexdigest()
    assert len(evidence["runs"]) == 2
    for run in evidence["runs"]:
        assert run["generation_sha256"] == generation
        assert len(run["compilations"]) == 10
        for invocation in run["compilations"]:
            assert invocation["exit_code"] == 0
            for field in ("output", "stdout", "stderr", "invocation_record"):
                item = invocation[field]
                data = (candidate / item["path"]).read_bytes()
                assert len(data) == item["size"]
                assert hashlib.sha256(data).hexdigest() == item["sha256"]
    assert str(workspace / "signer.pem") not in (candidate / "evidence/record.json").read_text()


def test_divergent_second_run_never_publishes_candidate(workspace: Path) -> None:
    """A compiler that emits different PTX on the second run fails closed."""
    source_dir, nvcc, key = _fixture(workspace)
    output = workspace / "candidate"
    with pytest.raises(bundle.BundleError, match="different PTX"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=output,
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
            extra=("--diverge",),
        )
    assert not output.exists()


def test_source_snapshot_mutation_never_publishes_candidate(workspace: Path) -> None:
    """The PTX digest alone cannot hide a source modified by the compiler."""
    source_dir, nvcc, key = _fixture(workspace)
    output = workspace / "candidate"
    with pytest.raises(bundle.BundleError, match="source snapshot changed"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=output,
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
            extra=("--mutate-source",),
        )
    assert not output.exists()


def test_signature_and_ptx_corruption_are_independently_detectable(workspace: Path) -> None:
    """Manifest signing does not bless later signature or artifact changes."""
    candidate, _, generation = _build(workspace)
    raw_public = (candidate / "provenance.v1.pub").read_bytes()
    der_path = workspace / "independent-public.der"
    der_path.write_bytes(bytes.fromhex("302a300506032b6570032100") + raw_public)
    signature = candidate / "provenance.v1.sig"
    original = signature.read_bytes()
    signature.write_bytes(bytes([original[0] ^ 1]) + original[1:])
    assert _verify_signature(candidate, der_path).returncode != 0
    signature.write_bytes(original)
    ptx = candidate / "poseidon.ptx"
    ptx.write_bytes(ptx.read_bytes() + b"// corruption\n")
    assert _verify_signature(candidate, der_path).returncode == 0
    assert _independent_generation(candidate) != generation
    expected = dict(line.split("=", 1) for line in (candidate / "provenance.v1").read_text().splitlines()[1:])
    assert hashlib.sha256(ptx.read_bytes()).hexdigest() != expected["artifact.poseidon.ptx_sha256"]


def test_source_inventory_and_unreviewed_inputs_are_rejected(workspace: Path) -> None:
    """An extra family, symlink, or unspecified signing provenance cannot pass."""
    source_dir, nvcc, key = _fixture(workspace)
    (source_dir / "float_diagnostic.cu").write_bytes(b"// retired\n")
    with pytest.raises(bundle.BundleError, match="unexpected"):
        bundle.validate_sources(source_dir)
    (source_dir / "float_diagnostic.cu").unlink()
    (source_dir / "aes.cu").unlink()
    (source_dir / "aes.cu").symlink_to(source_dir / "vector.cu")
    with pytest.raises(bundle.BundleError, match="regular file"):
        bundle.validate_sources(source_dir)
    with pytest.raises(bundle.BundleError, match="image digest"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=workspace / "candidate",
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest="0" * 64,
            target_profile="arch=compute_86,code=sm_86",
        )


def test_cli_requires_explicit_tools_and_key_and_reports_only_public_digests(
    workspace: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    """The command-line entry point never emits or copies the signing key."""
    source_dir, nvcc, key = _fixture(workspace)
    candidate = workspace / "candidate"
    monkeypatch.setattr(bundle, "default_host_compiler", lambda: None)
    monkeypatch.setattr(bundle, "SOURCE_DIR", source_dir)
    assert bundle.main(
        [
            "--output-dir", str(candidate),
            "--nvcc", str(nvcc),
            "--openssl", str(OPENSSL),
            "--signing-key", str(key),
            "--cuda-image-sha256", hashlib.sha256(b"image").hexdigest(),
        ]
    ) == 0
    output = capsys.readouterr().out
    assert "trusted_key_sha256=" in output and "generation_sha256=" in output
    assert str(key) not in output and key.name not in {path.name for path in candidate.iterdir()}


def test_flag_and_host_compiler_selection_match_build_script(monkeypatch: pytest.MonkeyPatch) -> None:
    """The manifest flags retain one token per explicit option and CXX disables auto-selection."""
    assert bundle.compiler_flags("arch=compute_86,code=sm_86", None, ("--fmad=false",)) == (
        "-ptx -std=c++14 -gencode arch=compute_86,code=sm_86 --fmad=false"
    )
    with pytest.raises(bundle.BundleError, match="one nonempty argument"):
        bundle.compiler_flags("arch=compute_86,code=sm_86", None, ("-Xfoo two",))
    monkeypatch.setattr(bundle.sys, "platform", "linux")
    monkeypatch.setenv("CXX", "/private/pinned/compiler")
    assert bundle.default_host_compiler() is None


def test_broken_output_symlink_and_relative_tool_path_are_rejected(workspace: Path) -> None:
    """A stale candidate link cannot be silently replaced by publication."""
    source_dir, nvcc, key = _fixture(workspace)
    output = workspace / "candidate"
    output.symlink_to(workspace / "missing-target")
    with pytest.raises(bundle.BundleError, match="fresh"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=output,
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
        )
    with pytest.raises(bundle.BundleError, match="absolute"):
        bundle._regular_executable(Path("nvcc"), "nvcc")


def test_non_ed25519_signer_cannot_publish_candidate(workspace: Path) -> None:
    """A valid but wrong-algorithm PEM key does not satisfy V1 admission."""
    source_dir, nvcc, key = _fixture(workspace)
    subprocess.run(
        [
            OPENSSL, "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048",
            "-out", str(key),
        ],
        check=True,
        capture_output=True,
    )
    output = workspace / "candidate"
    with pytest.raises(bundle.BundleError, match="Ed25519"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=output,
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
        )
    assert not output.exists()


def test_no_follow_read_retains_inode_and_refuses_replaced_path(
    workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The original descriptor stays intact, but a replaced logical source is refused."""
    source = workspace / "source.cu"
    source.write_bytes(b"original source")
    replacement = workspace / "replacement.cu"
    replacement.write_bytes(b"different source")
    original_open = bundle.os.open
    retained = []

    def swap_after_open(path: Path, flags: int) -> int:
        descriptor = original_open(path, flags)
        if Path(path) == source:
            retained.append(os.dup(descriptor))
            source.rename(workspace / "old-source.cu")
            source.symlink_to(replacement)
        return descriptor

    monkeypatch.setattr(bundle.os, "open", swap_after_open)
    with pytest.raises(bundle.BundleError, match="changed while"):
        bundle.read_regular_file(source, 1024)
    try:
        assert os.pread(retained[0], 1024, 0) == b"original source"
    finally:
        os.close(retained[0])
    with pytest.raises(bundle.BundleError, match="regular file"):
        bundle.read_regular_file(source, 1024)


def test_large_source_trivia_retains_signed_exact_candidate(workspace: Path) -> None:
    """Source comments do not change admission; exact source hashes remain signed."""
    source_dir, nvcc, key = _fixture(workspace)
    original = (source_dir / "vector.cu").read_bytes()
    source = original + b"//" + b"x" * (1024 * 1024) + b"\n"
    (source_dir / "vector.cu").write_bytes(source)
    candidate = workspace / "candidate"
    _, generation = bundle.build_candidate(
        source_dir=source_dir, output_dir=candidate, nvcc=nvcc,
        openssl=Path(OPENSSL), signing_key=key,
        image_digest=hashlib.sha256(b"image").hexdigest(),
        target_profile="arch=compute_86,code=sm_86",
    )
    assert (candidate / "vector.cu").read_bytes() == source
    manifest = (candidate / "provenance.v1").read_text()
    assert f"artifact.vector.source_sha256={hashlib.sha256(source).hexdigest()}" in manifest
    assert generation == _independent_generation(candidate)
    public_der = workspace / "independent-public.der"
    public_der.write_bytes(bytes.fromhex("302a300506032b6570032100") + (candidate / "provenance.v1.pub").read_bytes())
    assert _verify_signature(candidate, public_der).returncode == 0


def test_explicit_artifact_bound_still_refuses_large_regular_file(workspace: Path) -> None:
    """Artifact admission remains bounded independently of implementation size."""
    path = workspace / "artifact.ptx"
    path.write_bytes(b"x" * 65)
    with pytest.raises(bundle.BundleError, match="regular file"):
        bundle.read_regular_file(path, 64)


def test_signing_key_symlink_into_repository_is_rejected(
    workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An external-looking link cannot move the private signer into source control."""
    source_dir, nvcc, key = _fixture(workspace)
    repository = workspace / "repository"
    repository.mkdir()
    inside = repository / "signer.pem"
    key.rename(inside)
    key.symlink_to(inside)
    monkeypatch.setattr(bundle, "ROOT", repository)
    with pytest.raises(bundle.BundleError, match="outside the repository"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=workspace / "candidate",
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
        )


@pytest.mark.parametrize("populated", [False, True])
def test_existing_destination_cannot_be_replaced_at_publication(workspace, monkeypatch, populated):
    original = bundle.custody.publish_directory_noreplace

    def race(stage, destination, **kwargs):
        destination.mkdir()
        if populated:
            (destination / "sentinel").write_bytes(b"other owner")
        original(stage, destination, **kwargs)

    monkeypatch.setattr(bundle.custody, "publish_directory_noreplace", race)
    with pytest.raises(bundle.custody.ReleaseArtifactError, match="publication failed"):
        _build(workspace)
    if populated:
        assert (workspace / "candidate/sentinel").read_bytes() == b"other owner"
    else:
        assert not list((workspace / "candidate").iterdir())
    assert not (workspace / "candidate/provenance.v1").exists()


def test_output_parent_symlink_is_rejected_before_compiler(workspace):
    source_dir, nvcc, key = _fixture(workspace)
    real = workspace / "real"
    real.mkdir()
    alias = workspace / "alias"
    alias.symlink_to(real, target_is_directory=True)
    with pytest.raises(bundle.custody.ReleaseArtifactError, match="directory"):
        bundle.build_candidate(source_dir=source_dir, output_dir=alias / "candidate",
                               nvcc=nvcc, openssl=Path(OPENSSL), signing_key=key,
                               image_digest=hashlib.sha256(b"image").hexdigest(),
                               target_profile="arch=compute_86,code=sm_86")
    assert not list(real.iterdir())


def test_replaced_parent_retains_original_stage_and_never_publishes(workspace, monkeypatch):
    source_dir, nvcc, key = _fixture(workspace)
    parent = workspace / "output"
    parent.mkdir()
    moved = workspace / "original-output"
    original = bundle.sign_manifest

    def replace_parent(*args, **kwargs):
        result = original(*args, **kwargs)
        parent.rename(moved)
        parent.mkdir()
        return result

    monkeypatch.setattr(bundle, "sign_manifest", replace_parent)
    with pytest.raises(bundle.custody.ReleaseArtifactError):
        bundle.build_candidate(source_dir=source_dir, output_dir=parent / "candidate",
                               nvcc=nvcc, openssl=Path(OPENSSL), signing_key=key,
                               image_digest=hashlib.sha256(b"image").hexdigest(),
                               target_profile="arch=compute_86,code=sm_86")
    assert not list(parent.iterdir())
    assert list(moved.glob(".ivm-cuda-incomplete-*"))


def test_tool_output_bound_rejects_without_publication(workspace, monkeypatch):
    monkeypatch.setattr(bundle, "MAX_LOG_BYTES", 1)
    with pytest.raises(bundle.BundleError, match="log byte limit"):
        _build(workspace)
    assert not (workspace / "candidate").exists()


def test_tool_replacement_during_generation_is_not_signed_evidence(workspace, monkeypatch):
    original = bundle.compile_run
    changed = False

    def replace(*args, **kwargs):
        nonlocal changed
        result = original(*args, **kwargs)
        if not changed:
            changed = True
            nvcc = args[0]
            nvcc.write_bytes(nvcc.read_bytes() + b"\n# replaced executable\n")
        return result

    monkeypatch.setattr(bundle, "compile_run", replace)
    with pytest.raises(bundle.custody.ReleaseArtifactError, match="no longer matches its stable capture"):
        _build(workspace)
    assert not (workspace / "candidate").exists()


def test_alternate_compiler_really_executes_but_parent_restore_cannot_publish(workspace, monkeypatch):
    source_dir, nvcc, key = _fixture(workspace)
    parent = workspace / "compiler"
    parent.mkdir()
    nvcc.rename(parent / "nvcc")
    nvcc = parent / "nvcc"
    before = bundle.custody.stable_hash_path(nvcc)
    alternate = workspace / "alternate"
    alternate.mkdir()
    marker = workspace / "executed"
    marker.write_bytes(b"")
    (alternate / "nvcc").write_text(
        f"#!{sys.executable}\nfrom pathlib import Path\n"
        f"Path({str(marker)!r}).write_bytes(b'alternate compiler executed')\n"
        "print('Pinned nvcc test output')\n"
    )
    (alternate / "nvcc").chmod(0o755)
    saved = workspace / "original-compiler"
    popen = bundle.subprocess.Popen

    def substitute(command, **kwargs):
        assert str(nvcc) in command
        parent.rename(saved)
        alternate.rename(parent)
        try:
            child = popen(command, **kwargs)
            assert child.wait(timeout=5) == 0
            assert marker.read_bytes() == b"alternate compiler executed"
            return child
        finally:
            parent.rename(alternate)
            saved.rename(parent)

    monkeypatch.setattr(bundle.subprocess, "Popen", substitute)
    with pytest.raises(bundle.custody.ReleaseArtifactError, match="ancestor changed"):
        bundle.build_candidate(source_dir=source_dir, output_dir=workspace / "candidate",
                               nvcc=nvcc, openssl=Path(OPENSSL), signing_key=key,
                               image_digest=hashlib.sha256(b"image").hexdigest(),
                               target_profile="arch=compute_86,code=sm_86")
    assert marker.read_bytes() == b"alternate compiler executed"
    assert bundle.custody.stable_hash_path(nvcc) == before
    assert not (workspace / "candidate").exists()
