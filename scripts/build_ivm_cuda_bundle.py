#!/usr/bin/env python3
"""Build a signed, two-run IVM CUDA PTX candidate outside the shipping build.

Requires Python 3.10+, a pinned ``nvcc`` executable, OpenSSL with Ed25519
support, an independently measured CUDA image SHA-256, and an explicitly
provided PEM signing key outside this repository. No environment variables are
required. The key is never copied into the output or printed. This tool only
produces a candidate: signer review, image attestation, and GPU qualification
are separate release gates.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "crates" / "ivm" / "cuda"
STEMS = (
    "aes",
    "bitonic_sort",
    "bn254",
    "poseidon",
    "sha256",
    "sha256_leaves",
    "sha256_pairs_reduce",
    "sha3",
    "signature",
    "vector",
)
HEADER = "ivm-cuda-ptx-provenance-v1"
GENERATION_DOMAIN = b"ivm-cuda-ptx-generation-v1\0"
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
MAX_SOURCE_BYTES = 1024 * 1024
MAX_PTX_BYTES = 8 * 1024 * 1024
MAX_MANIFEST_BYTES = 16 * 1024
TARGET_RE = re.compile(r"arch=compute_[0-9]+,code=sm_[0-9]+\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")


class BundleError(ValueError):
    """The candidate cannot satisfy the signed V1 bundle contract."""


def sha256_hex(data: bytes) -> str:
    """Return the lowercase SHA-256 of exact bytes."""
    return hashlib.sha256(data).hexdigest()


def read_regular_bounded(path: Path, maximum: int) -> bytes:
    """Bound a regular inode and read it through one no-follow file descriptor."""
    if not hasattr(os, "O_NOFOLLOW"):
        raise BundleError("no-follow file admission is unavailable on this host")
    flags = os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BundleError(f"{path} must be a bounded regular file: {error.strerror}") from error
    with os.fdopen(descriptor, "rb") as handle:
        metadata = os.fstat(handle.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > maximum:
            raise BundleError(f"{path} must be a bounded regular file")
        data = handle.read(maximum + 1)
        if len(data) > maximum:
            raise BundleError(f"{path} grew beyond its byte limit")
        return data


def validate_sources(source_dir: Path) -> dict[str, bytes]:
    """Pin exactly the ten source families in the verifier's order."""
    observed = {path.stem for path in source_dir.glob("*.cu")}
    if observed != set(STEMS):
        raise BundleError(
            "CUDA source inventory differs from the ten pinned families: "
            f"missing={sorted(set(STEMS) - observed)}, "
            f"unexpected={sorted(observed - set(STEMS))}"
        )
    return {
        stem: read_regular_bounded(source_dir / f"{stem}.cu", MAX_SOURCE_BYTES)
        for stem in STEMS
    }


def validate_ptx(stem: str, data: bytes) -> None:
    """Apply the build script's bounded structural PTX admission check."""
    if len(data) > MAX_PTX_BYTES:
        raise BundleError(f"{stem}.ptx exceeds its byte limit")
    try:
        tokens = data.decode("utf-8").split()
    except UnicodeDecodeError as error:
        raise BundleError(f"{stem}.ptx is not UTF-8") from error
    for directive in (".version", ".target", ".address_size", ".entry"):
        if directive not in tokens:
            raise BundleError(f"{stem}.ptx is missing {directive}")


def generation_sha256(artifacts: dict[str, bytes]) -> str:
    """Hash one run using the exact Rust verifier's length-framed preimage."""
    digest = hashlib.sha256(GENERATION_DOMAIN)
    for stem in STEMS:
        name = stem.encode("ascii")
        data = artifacts[stem]
        digest.update(len(name).to_bytes(2, "little"))
        digest.update(name)
        digest.update(len(data).to_bytes(8, "little"))
        digest.update(data)
    return digest.hexdigest()


def compiler_flags(target_profile: str, host_compiler: Path | None, extra: tuple[str, ...]) -> str:
    """Render the exact compiler flags checked by ``ivm/build.rs``."""
    if any(not flag or any(char.isspace() for char in flag) for flag in extra):
        raise BundleError("each extra nvcc flag must be one nonempty argument")
    flags = ["-ptx", "-std=c++14"]
    if host_compiler is not None:
        flags.append(f"-ccbin={host_compiler}")
    flags.extend(("-gencode", target_profile))
    flags.extend(extra)
    rendered = " ".join(flags)
    if len(rendered) > 512 or any(ord(char) < 32 or ord(char) > 126 for char in rendered):
        raise BundleError("nvcc flags are not canonical printable ASCII")
    return rendered


def compile_run(
    nvcc: Path,
    sources: Path,
    output: Path,
    target_profile: str,
    host_compiler: Path | None,
    extra: tuple[str, ...],
) -> dict[str, bytes]:
    """Compile the fixed inventory in a clean output directory."""
    output.mkdir()
    result = {}
    for stem in STEMS:
        target = output / f"{stem}.ptx"
        command = [str(nvcc), "-ptx", f"{stem}.cu", "-o", str(target), "-std=c++14"]
        if host_compiler is not None:
            command.append(f"-ccbin={host_compiler}")
        command.extend(("-gencode", target_profile, *extra))
        subprocess.run(command, cwd=sources, check=True)
        data = read_regular_bounded(target, MAX_PTX_BYTES)
        validate_ptx(stem, data)
        result[stem] = data
    return result


def manifest_bytes(
    *,
    image_digest: str,
    version_digest: str,
    flags: str,
    target_profile: str,
    first_generation: str,
    second_generation: str,
    sources: dict[str, bytes],
    artifacts: dict[str, bytes],
) -> bytes:
    """Encode the signed V1 manifest in the verifier's exact field order."""
    lines = [
        HEADER,
        f"cuda_image_sha256={image_digest}",
        f"nvcc_version_sha256={version_digest}",
        f"nvcc_flags={flags}",
        f"target_profile={target_profile}",
        f"generation_1_sha256={first_generation}",
        f"generation_2_sha256={second_generation}",
    ]
    for stem in STEMS:
        lines.append(f"artifact.{stem}.source_sha256={sha256_hex(sources[stem])}")
        lines.append(f"artifact.{stem}.ptx_sha256={sha256_hex(artifacts[stem])}")
    encoded = ("\n".join(lines) + "\n").encode("ascii")
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise BundleError("CUDA manifest exceeds its byte limit")
    return encoded


def sign_manifest(openssl: Path, key: Path, bundle: Path, manifest: bytes) -> str:
    """Sign with an explicit external Ed25519 key and verify the raw signature."""
    manifest_path = bundle / "provenance.v1"
    signature_path = bundle / "provenance.v1.sig"
    public_path = bundle / "provenance.v1.pub"
    manifest_path.write_bytes(manifest)
    public_der = subprocess.run(
        [str(openssl), "pkey", "-in", str(key), "-pubout", "-outform", "DER"],
        check=True,
        capture_output=True,
    ).stdout
    if len(public_der) != 44 or not public_der.startswith(ED25519_SPKI_PREFIX):
        raise BundleError("signing key is not a canonical Ed25519 private key")
    public_key = public_der[len(ED25519_SPKI_PREFIX) :]
    public_path.write_bytes(public_key)
    subprocess.run(
        [
            str(openssl), "pkeyutl", "-sign", "-rawin", "-inkey", str(key),
            "-in", str(manifest_path), "-out", str(signature_path),
        ],
        check=True,
        capture_output=True,
    )
    signature = read_regular_bounded(signature_path, 64)
    if len(signature) != 64:
        raise BundleError("Ed25519 signature must be exactly 64 raw bytes")
    public_der_path = bundle.parent / "public.der"
    public_der_path.write_bytes(public_der)
    subprocess.run(
        [
            str(openssl), "pkeyutl", "-verify", "-rawin", "-pubin",
            "-inkey", str(public_der_path), "-keyform", "DER",
            "-sigfile", str(signature_path), "-in", str(manifest_path),
        ],
        check=True,
        capture_output=True,
    )
    return sha256_hex(public_key)


def build_candidate(
    *,
    source_dir: Path,
    output_dir: Path,
    nvcc: Path,
    openssl: Path,
    signing_key: Path,
    image_digest: str,
    target_profile: str,
    host_compiler: Path | None = None,
    extra: tuple[str, ...] = (),
) -> tuple[str, str]:
    """Publish an atomic candidate only after two equal runs and a valid signature."""
    signing_key = signing_key.resolve(strict=True)
    read_regular_bounded(signing_key, 64 * 1024)
    if not SHA256_RE.fullmatch(image_digest) or image_digest == "0" * 64:
        raise BundleError("CUDA image digest must be a nonzero lowercase SHA-256")
    if not TARGET_RE.fullmatch(target_profile):
        raise BundleError("CUDA target profile is not a canonical compute/sm pair")
    output_parent = output_dir.parent.resolve(strict=False)
    if (
        output_dir.exists()
        or output_dir.is_symlink()
        or output_dir == source_dir
        or output_parent == source_dir
        or source_dir in output_parent.parents
    ):
        raise BundleError("candidate output must be fresh and outside the CUDA source directory")
    if ROOT == signing_key or ROOT in signing_key.parents:
        raise BundleError("signing key must remain outside the repository")
    sources = validate_sources(source_dir)
    flags = compiler_flags(target_profile, host_compiler, extra)
    version = subprocess.run([str(nvcc), "--version"], check=True, capture_output=True)
    version_digest = sha256_hex(version.stdout + version.stderr)
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".ivm-cuda-", dir=output_dir.parent) as temporary:
        work = Path(temporary)
        bundle = work / "candidate"
        bundle.mkdir()
        for stem in STEMS:
            (bundle / f"{stem}.cu").write_bytes(sources[stem])
        first = compile_run(nvcc, bundle, work / "run-one", target_profile, host_compiler, extra)
        second = compile_run(nvcc, bundle, work / "run-two", target_profile, host_compiler, extra)
        first_digest = generation_sha256(first)
        second_digest = generation_sha256(second)
        if first != second or first_digest != second_digest:
            raise BundleError("two independent clean nvcc runs produced different PTX")
        for stem in STEMS:
            if read_regular_bounded(bundle / f"{stem}.cu", MAX_SOURCE_BYTES) != sources[stem]:
                raise BundleError(f"source snapshot changed during generation: {stem}")
        for stem in STEMS:
            (bundle / f"{stem}.ptx").write_bytes(first[stem])
        manifest = manifest_bytes(
            image_digest=image_digest,
            version_digest=version_digest,
            flags=flags,
            target_profile=target_profile,
            first_generation=first_digest,
            second_generation=second_digest,
            sources=sources,
            artifacts=first,
        )
        fingerprint = sign_manifest(openssl, signing_key, bundle, manifest)
        if output_dir.exists() or output_dir.is_symlink():
            raise BundleError("candidate output appeared during generation")
        bundle.rename(output_dir)
    return fingerprint, first_digest


def _regular_executable(path: Path, label: str) -> Path:
    """Resolve a named executable without looking it up through PATH."""
    if not path.is_absolute():
        raise BundleError(f"{label} path must be absolute")
    resolved = path.resolve(strict=True)
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise BundleError(f"{label} must be an executable regular file")
    return resolved


def default_host_compiler() -> Path | None:
    """Mirror the Linux host-compiler selection in ``ivm/build.rs``."""
    if sys.platform != "linux" or any(
        key == "CXX" or key == "HOST_CXX" or key.startswith("CXX_")
        for key in os.environ
    ):
        return None
    for candidate in ("/usr/bin/g++-12", "/usr/local/bin/g++-12", "/bin/g++-12"):
        path = Path(candidate)
        if path.exists():
            return path
    return None


def main(argv: list[str] | None = None) -> int:
    """Parse explicit release inputs and print only public candidate digests."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--nvcc", type=Path, required=True, help="absolute pinned nvcc path")
    parser.add_argument("--openssl", type=Path, required=True, help="absolute OpenSSL path")
    parser.add_argument("--signing-key", type=Path, required=True, help="external Ed25519 PEM key")
    parser.add_argument("--cuda-image-sha256", required=True, help="independent toolkit image digest")
    parser.add_argument("--target-profile", default="arch=compute_86,code=sm_86")
    parser.add_argument("--host-compiler", type=Path)
    parser.add_argument("--extra-flag", action="append", default=[])
    arguments = parser.parse_args(argv)
    try:
        source_dir = SOURCE_DIR.resolve(strict=True)
        output_dir = Path(os.path.abspath(arguments.output_dir))
        nvcc = _regular_executable(arguments.nvcc, "nvcc")
        openssl = _regular_executable(arguments.openssl, "OpenSSL")
        signing_key = arguments.signing_key.resolve(strict=True)
        host_compiler = (
            _regular_executable(arguments.host_compiler, "host compiler")
            if arguments.host_compiler is not None else default_host_compiler()
        )
        fingerprint, generation = build_candidate(
            source_dir=source_dir,
            output_dir=output_dir,
            nvcc=nvcc,
            openssl=openssl,
            signing_key=signing_key,
            image_digest=arguments.cuda_image_sha256,
            target_profile=arguments.target_profile,
            host_compiler=host_compiler,
            extra=tuple(arguments.extra_flag),
        )
    except subprocess.CalledProcessError as error:
        parser.error(f"external compiler or signer command failed with status {error.returncode}")
    except (BundleError, OSError) as error:
        parser.error(str(error))
    print(f"candidate={output_dir}")
    print(f"trusted_key_sha256={fingerprint}")
    print(f"generation_sha256={generation}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
