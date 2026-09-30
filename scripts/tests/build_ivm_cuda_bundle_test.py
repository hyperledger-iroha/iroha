"""Offline contract tests for the signed IVM CUDA PTX candidate builder."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import subprocess

import pytest

from scripts import build_ivm_cuda_bundle as bundle


OPENSSL = shutil.which("openssl")
pytestmark = pytest.mark.skipif(OPENSSL is None, reason="OpenSSL Ed25519 is unavailable")


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path]:
    source_dir = tmp_path / "sources"
    source_dir.mkdir()
    for stem in bundle.STEMS:
        (source_dir / f"{stem}.cu").write_bytes(f"// source for {stem}\n".encode())
    nvcc = tmp_path / "fake-nvcc"
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
    key = tmp_path / "signer.pem"
    subprocess.run(
        [OPENSSL, "genpkey", "-algorithm", "Ed25519", "-out", str(key)],
        check=True,
        capture_output=True,
    )
    return source_dir, nvcc, key


def _build(tmp_path: Path, *, extra: tuple[str, ...] = ()) -> tuple[Path, str, str]:
    source_dir, nvcc, key = _fixture(tmp_path)
    candidate = tmp_path / "candidate"
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


def test_two_run_candidate_matches_independent_manifest_and_signature(tmp_path: Path) -> None:
    """The published bundle is byte-exact and verifies without the builder parser."""
    candidate, fingerprint, generation = _build(tmp_path)
    assert {path.name for path in candidate.iterdir()} == {
        *(f"{stem}.cu" for stem in bundle.STEMS),
        *(f"{stem}.ptx" for stem in bundle.STEMS),
        "provenance.v1",
        "provenance.v1.sig",
        "provenance.v1.pub",
    }
    raw_public = (candidate / "provenance.v1.pub").read_bytes()
    assert len(raw_public) == 32
    assert fingerprint == hashlib.sha256(raw_public).hexdigest()
    assert len((candidate / "provenance.v1.sig").read_bytes()) == 64
    der_path = tmp_path / "independent-public.der"
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


def test_divergent_second_run_never_publishes_candidate(tmp_path: Path) -> None:
    """A compiler that emits different PTX on the second run fails closed."""
    source_dir, nvcc, key = _fixture(tmp_path)
    output = tmp_path / "candidate"
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


def test_source_snapshot_mutation_never_publishes_candidate(tmp_path: Path) -> None:
    """The PTX digest alone cannot hide a source modified by the compiler."""
    source_dir, nvcc, key = _fixture(tmp_path)
    output = tmp_path / "candidate"
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


def test_signature_and_ptx_corruption_are_independently_detectable(tmp_path: Path) -> None:
    """Manifest signing does not bless later signature or artifact changes."""
    candidate, _, generation = _build(tmp_path)
    raw_public = (candidate / "provenance.v1.pub").read_bytes()
    der_path = tmp_path / "independent-public.der"
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


def test_source_inventory_and_unreviewed_inputs_are_rejected(tmp_path: Path) -> None:
    """An extra family, symlink, or unspecified signing provenance cannot pass."""
    source_dir, nvcc, key = _fixture(tmp_path)
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
            output_dir=tmp_path / "candidate",
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest="0" * 64,
            target_profile="arch=compute_86,code=sm_86",
        )


def test_cli_requires_explicit_tools_and_key_and_reports_only_public_digests(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    """The command-line entry point never emits or copies the signing key."""
    source_dir, nvcc, key = _fixture(tmp_path)
    candidate = tmp_path / "candidate"
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


def test_broken_output_symlink_and_relative_tool_path_are_rejected(tmp_path: Path) -> None:
    """A stale candidate link cannot be silently replaced by publication."""
    source_dir, nvcc, key = _fixture(tmp_path)
    output = tmp_path / "candidate"
    output.symlink_to(tmp_path / "missing-target")
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


def test_non_ed25519_signer_cannot_publish_candidate(tmp_path: Path) -> None:
    """A valid but wrong-algorithm PEM key does not satisfy V1 admission."""
    source_dir, nvcc, key = _fixture(tmp_path)
    subprocess.run(
        [
            OPENSSL, "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048",
            "-out", str(key),
        ],
        check=True,
        capture_output=True,
    )
    output = tmp_path / "candidate"
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
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The original descriptor stays intact, but a replaced logical source is refused."""
    source = tmp_path / "source.cu"
    source.write_bytes(b"original source")
    replacement = tmp_path / "replacement.cu"
    replacement.write_bytes(b"different source")
    original_open = bundle.os.open
    retained = []

    def swap_after_open(path: Path, flags: int) -> int:
        descriptor = original_open(path, flags)
        if Path(path) == source:
            retained.append(os.dup(descriptor))
            source.rename(tmp_path / "old-source.cu")
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


def test_large_source_trivia_retains_signed_exact_candidate(tmp_path: Path) -> None:
    """Source comments do not change admission; exact source hashes remain signed."""
    source_dir, nvcc, key = _fixture(tmp_path)
    original = (source_dir / "vector.cu").read_bytes()
    source = original + b"//" + b"x" * (1024 * 1024) + b"\n"
    (source_dir / "vector.cu").write_bytes(source)
    candidate = tmp_path / "candidate"
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
    public_der = tmp_path / "independent-public.der"
    public_der.write_bytes(bytes.fromhex("302a300506032b6570032100") + (candidate / "provenance.v1.pub").read_bytes())
    assert _verify_signature(candidate, public_der).returncode == 0


def test_explicit_artifact_bound_still_refuses_large_regular_file(tmp_path: Path) -> None:
    """Artifact admission remains bounded independently of implementation size."""
    path = tmp_path / "artifact.ptx"
    path.write_bytes(b"x" * 65)
    with pytest.raises(bundle.BundleError, match="regular file"):
        bundle.read_regular_file(path, 64)


def test_signing_key_symlink_into_repository_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An external-looking link cannot move the private signer into source control."""
    source_dir, nvcc, key = _fixture(tmp_path)
    repository = tmp_path / "repository"
    repository.mkdir()
    inside = repository / "signer.pem"
    key.rename(inside)
    key.symlink_to(inside)
    monkeypatch.setattr(bundle, "ROOT", repository)
    with pytest.raises(bundle.BundleError, match="outside the repository"):
        bundle.build_candidate(
            source_dir=source_dir,
            output_dir=tmp_path / "candidate",
            nvcc=nvcc,
            openssl=Path(OPENSSL),
            signing_key=key,
            image_digest=hashlib.sha256(b"image").hexdigest(),
            target_profile="arch=compute_86,code=sm_86",
        )
