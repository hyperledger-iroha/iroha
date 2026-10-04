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
from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import sys
import time
import uuid

if __package__:
    from . import release_artifact_contract as custody
else:
    import release_artifact_contract as custody


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
MAX_PTX_BYTES = 8 * 1024 * 1024
MAX_MANIFEST_BYTES = 16 * 1024
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_TOOL_BYTES = 512 * 1024 * 1024
MAX_LOG_BYTES = 4 * 1024 * 1024
MAX_EVIDENCE_BYTES = 128 * 1024
TOOL_TIMEOUT_SECONDS = 1200
TARGET_RE = re.compile(r"arch=compute_[0-9]+,code=sm_[0-9]+\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")


class BundleError(custody.ReleaseArtifactError):
    """The candidate cannot satisfy the signed V1 bundle contract."""


def sha256_hex(data: bytes) -> str:
    """Return the lowercase SHA-256 of exact bytes."""
    return hashlib.sha256(data).hexdigest()


def read_regular_file(path: Path, maximum: int | None = None) -> bytes:
    """Read one no-follow regular inode, retaining explicit artifact bounds."""
    if not hasattr(os, "O_NOFOLLOW"):
        raise BundleError("no-follow file admission is unavailable on this host")
    flags = os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BundleError(f"{path} must be a bounded regular file: {error.strerror}") from error
    with os.fdopen(descriptor, "rb") as handle:
        metadata = os.fstat(handle.fileno())
        if not stat.S_ISREG(metadata.st_mode) or (maximum is not None and metadata.st_size > maximum):
            raise BundleError(f"{path} must be a bounded regular file")
        data = handle.read(metadata.st_size + 1)
        after = os.fstat(handle.fileno())
        try:
            after_path = path.lstat()
        except OSError as error:
            raise BundleError(f"{path} changed while its inode was read") from error
        if maximum is not None and len(data) > maximum:
            raise BundleError(f"{path} grew beyond its byte limit")
        fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
        if (
            len(data) != metadata.st_size
            or not stat.S_ISREG(after_path.st_mode)
            or any(
                getattr(metadata, field) != getattr(observed, field)
                for observed in (after, after_path)
                for field in fields
            )
        ):
            raise BundleError(f"{path} changed while its inode was read")
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
        stem: read_regular_file(source_dir / f"{stem}.cu", MAX_SOURCE_BYTES)
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
    check_custody,
) -> tuple[dict[str, bytes], list[dict[str, object]]]:
    """Compile the fixed inventory in a clean output directory."""
    check_custody()
    custody.create_fresh_directory(output, mode=0o700)
    result = {}
    records = []
    for stem in STEMS:
        target = output / f"{stem}.ptx"
        command = [str(nvcc), "-ptx", f"{stem}.cu", "-o", str(target.relative_to(sources)), "-std=c++14"]
        if host_compiler is not None:
            command.append(f"-ccbin={host_compiler}")
        command.extend(("-gencode", target_profile, *extra))
        _, _, record = run_tool(command, sources, output / stem, check_custody)
        data = read_regular_file(target, MAX_PTX_BYTES)
        validate_ptx(stem, data)
        result[stem] = data
        record["source"] = {"path": f"{stem}.cu", "sha256": sha256_hex(read_regular_file(sources / f"{stem}.cu", MAX_SOURCE_BYTES))}
        record["output"] = {"path": str(target.relative_to(sources)), "sha256": sha256_hex(data), "size": len(data)}
        records.append(record)
    return result, records


def run_tool(command: list[str], directory: Path, log: Path, check_custody, *, public_command=None):
    """Retain bounded exact stdout/stderr and invocation evidence for one owned child."""
    directory_fd, launcher, tool_owners = check_custody()
    streams = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + TOOL_TIMEOUT_SECONDS
    # The child resolves relative source/output paths from the held directory,
    # even if an ancestor pathname is exchanged after the preflight check.
    # macOS does not permit chdir through /dev/fd. A fresh isolated Python
    # process performs fchdir and exec, avoiding Python's unsafe threaded preexec_fn.
    launch = [str(launcher), "-I", "-S", "-c",
              "import os,sys; fd=int(sys.argv[1]); os.fchdir(fd); os.close(fd); os.execv(sys.argv[2],sys.argv[2:])",
              str(directory_fd), *command]
    with ExitStack() as retained:
        for path, info in tool_owners.items():
            retained.enter_context(custody.pin_path_ancestors(path))
            retained.enter_context(custody.stable_open_relative(path.parent, path.name, expected=info))
        child = retained.enter_context(subprocess.Popen(
            launch, pass_fds=(directory_fd,), stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        ))
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ, "stdout")
                selector.register(child.stderr, selectors.EVENT_READ, "stderr")
                while selector.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise BundleError("CUDA producer tool exceeded its time limit")
                    for key, _ in selector.select(min(remaining, 0.25)):
                        chunk = os.read(key.fd, 64 * 1024)
                        if not chunk:
                            selector.unregister(key.fileobj)
                        elif sum(map(len, streams.values())) + len(chunk) > MAX_LOG_BYTES:
                            raise BundleError("CUDA producer tool exceeded its combined log byte limit")
                        else:
                            streams[key.data].extend(chunk)
            code = child.wait(timeout=max(0.001, deadline - time.monotonic()))
        except subprocess.TimeoutExpired as error:
            raise BundleError("CUDA producer tool exceeded its time limit") from error
        finally:
            if child.poll() is None:
                # This is only the producer's own compiler child, never another build.
                child.kill()
                child.wait()
    check_custody()
    outputs = {}
    for name, data in streams.items():
        path = log.with_suffix(f".{name}")
        custody.exclusive_write_bytes(path, bytes(data), mode=0o600)
        outputs[name] = {"path": str(path.relative_to(directory)), "sha256": sha256_hex(data), "size": len(data)}
    record = {"command": public_command if public_command is not None else command,
              "working_directory": ".", "exit_code": code, **outputs}
    command_bytes = custody.canonical_json_bytes(record)
    command_path = log.with_suffix(".command.json")
    custody.exclusive_write_bytes(command_path, command_bytes, mode=0o600)
    record["invocation_record"] = {"path": str(command_path.relative_to(directory)), "sha256": sha256_hex(command_bytes), "size": len(command_bytes)}
    if code != 0:
        raise BundleError(f"CUDA producer tool failed with status {code}; bounded logs retained")
    return bytes(streams["stdout"]), bytes(streams["stderr"]), record


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


def sign_manifest(openssl: Path, key: Path, bundle: Path, manifest: bytes, check_custody):
    """Sign with an explicit external Ed25519 key and verify the raw signature."""
    manifest_path = bundle / "provenance.v1"
    signature_path = bundle / "provenance.v1.sig"
    public_path = bundle / "provenance.v1.pub"
    check_custody()
    custody.exclusive_write_bytes(manifest_path, manifest, mode=0o600)
    command = [str(openssl), "pkey", "-in", str(key), "-pubout", "-outform", "DER"]
    public_der, _, public_record = run_tool(
        command, bundle, bundle / "evidence" / "public-key", check_custody,
        public_command=["<runtime-signing-key>" if part == str(key) else part for part in command],
    )
    if len(public_der) != 44 or not public_der.startswith(ED25519_SPKI_PREFIX):
        raise BundleError("signing key is not a canonical Ed25519 private key")
    public_key = public_der[len(ED25519_SPKI_PREFIX) :]
    check_custody()
    custody.exclusive_write_bytes(public_path, public_key, mode=0o600)
    command = [
            str(openssl), "pkeyutl", "-sign", "-rawin", "-inkey", str(key),
            "-in", manifest_path.name, "-out", signature_path.name,
        ]
    _, _, sign_record = run_tool(
        command, bundle, bundle / "evidence" / "sign", check_custody,
        public_command=["<runtime-signing-key>" if part == str(key) else part for part in command],
    )
    signature = read_regular_file(signature_path, 64)
    if len(signature) != 64:
        raise BundleError("Ed25519 signature must be exactly 64 raw bytes")
    check_custody()
    public_der_path = bundle / "evidence" / "public.der"
    custody.exclusive_write_bytes(public_der_path, public_der, mode=0o600)
    _, _, verify_record = run_tool(
        [
            str(openssl), "pkeyutl", "-verify", "-rawin", "-pubin",
            "-inkey", str(public_der_path.relative_to(bundle)), "-keyform", "DER",
            "-sigfile", signature_path.name, "-in", manifest_path.name,
        ],
        bundle, bundle / "evidence" / "verify", check_custody,
    )
    check_custody()
    artifacts = [{"path": name, "sha256": sha256_hex(data), "size": len(data)} for name, data in (
        (public_path.name, public_key), (signature_path.name, signature),
        (str(public_der_path.relative_to(bundle)), public_der),
    )]
    return sha256_hex(public_key), [public_record, sign_record, verify_record], artifacts


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
    read_regular_file(signing_key, 64 * 1024)
    if not SHA256_RE.fullmatch(image_digest) or image_digest == "0" * 64:
        raise BundleError("CUDA image digest must be a nonzero lowercase SHA-256")
    if not TARGET_RE.fullmatch(target_profile):
        raise BundleError("CUDA target profile is not a canonical compute/sm pair")
    if (not output_dir.is_absolute() or str(output_dir) != os.path.abspath(output_dir)
            or ".." in output_dir.parts):
        raise BundleError("candidate output must be an absolute canonical path")
    output_parent = output_dir.parent
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
    nvcc = _regular_executable(nvcc, "nvcc")
    openssl = _regular_executable(openssl, "OpenSSL")
    if host_compiler is not None:
        host_compiler = _regular_executable(host_compiler, "host compiler")
    sources = validate_sources(source_dir)
    flags = compiler_flags(target_profile, host_compiler, extra)
    tool_paths = {"nvcc": nvcc, "openssl": openssl,
                  "producer_python": Path(sys.executable).resolve(strict=True)}
    if host_compiler is not None:
        tool_paths["host_compiler"] = host_compiler
    tool_identities = {name: custody.stable_hash_path(path, max_size=MAX_TOOL_BYTES)
                       for name, path in tool_paths.items()}
    producer_sources = [Path(__file__).resolve(strict=True), Path(custody.__file__).resolve(strict=True)]
    producer_identities = {path: custody.stable_hash_path(path, max_size=MAX_SOURCE_BYTES) for path in producer_sources}
    bundle = custody.create_fresh_directory(output_parent / (".ivm-cuda-incomplete-" + uuid.uuid4().hex), mode=0o700)
    parent_fd, _, parent_identity = custody._open_absolute_directory(output_parent, "CUDA output parent")
    stage_fd = -1
    try:
        stage_fd, _, stage_identity = custody._open_absolute_directory(bundle, "CUDA candidate")

        def check_custody():
            for path, original in ((output_parent, parent_identity), (bundle, stage_identity)):
                descriptor, _, current = custody._open_absolute_directory(path, "CUDA candidate custody")
                try:
                    fields = ("st_dev", "st_ino", "st_uid", "st_mode")
                    if any(getattr(original, field) != getattr(current, field) for field in fields):
                        raise BundleError("CUDA candidate directory custody changed")
                finally:
                    os.close(descriptor)
            return stage_fd, tool_paths["producer_python"], {
                path: tool_identities[name] for name, path in tool_paths.items()
            }

        evidence = custody.create_fresh_directory(bundle / "evidence", mode=0o700)
        stdout, stderr, version_record = run_tool([str(nvcc), "--version"], bundle, evidence / "nvcc-version", check_custody)
        version_digest = sha256_hex(stdout + stderr)
        for stem in STEMS:
            custody.exclusive_write_bytes(bundle / f"{stem}.cu", sources[stem], mode=0o600)
        first, first_records = compile_run(nvcc, bundle, evidence / "run-one", target_profile, host_compiler, extra, check_custody)
        second, second_records = compile_run(nvcc, bundle, evidence / "run-two", target_profile, host_compiler, extra, check_custody)
        first_digest = generation_sha256(first)
        second_digest = generation_sha256(second)
        if first != second or first_digest != second_digest:
            raise BundleError("two independent clean nvcc runs produced different PTX")
        for stem in STEMS:
            if read_regular_file(bundle / f"{stem}.cu") != sources[stem]:
                raise BundleError(f"source snapshot changed during generation: {stem}")
        for stem in STEMS:
            custody.exclusive_write_bytes(bundle / f"{stem}.ptx", first[stem], mode=0o600)
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
        fingerprint, signer_records, signer_artifacts = sign_manifest(openssl, signing_key, bundle, manifest, check_custody)
        for name, path in tool_paths.items():
            if custody.stable_hash_path(path, max_size=MAX_TOOL_BYTES) != tool_identities[name]:
                raise BundleError("CUDA producer executable changed during generation")
        for path, original in producer_identities.items():
            if custody.stable_hash_path(path, max_size=MAX_SOURCE_BYTES) != original:
                raise BundleError("CUDA producer source changed during generation")
        records = {
            "schema": "ivm.cuda-candidate-evidence.v1",
            "scope": "Local producer observations only; no image attestation or physical qualification.",
            "cuda_image_sha256_claim": image_digest,
            "image_attestation_verified": False,
            "hardware_qualified": False,
            "trusted_key_sha256": fingerprint,
            "manifest_sha256": sha256_hex(manifest),
            "tools": {name: {"path": str(tool_paths[name]), "sha256": info.sha256, "size": info.size}
                      for name, info in tool_identities.items()},
            "nvcc_version": version_record,
            "signer_commands": signer_records,
            "signer_artifacts": signer_artifacts,
            "producer_sources": [{"path": str(path), "sha256": info.sha256, "size": info.size}
                                 for path, info in producer_identities.items()],
            "sources": [{"path": f"{stem}.cu", "sha256": sha256_hex(sources[stem]), "size": len(sources[stem])} for stem in STEMS],
            "runs": [{"generation_sha256": digest, "compilations": rows}
                     for digest, rows in ((first_digest, first_records), (second_digest, second_records))],
        }
        payload = custody.canonical_json_bytes(records)
        if len(payload) > MAX_EVIDENCE_BYTES:
            raise BundleError("CUDA producer evidence exceeds its byte limit")
        custody.exclusive_write_bytes(evidence / "record.json", payload, mode=0o600)
        expected = {f"{stem}.cu": (sha256_hex(sources[stem]), len(sources[stem])) for stem in STEMS}
        expected.update({f"{stem}.ptx": (sha256_hex(first[stem]), len(first[stem])) for stem in STEMS})
        expected["provenance.v1"] = (sha256_hex(manifest), len(manifest))
        expected["evidence/record.json"] = (sha256_hex(payload), len(payload))
        expected.update({item["path"]: (item["sha256"], item["size"]) for item in signer_artifacts})
        for row in [version_record, *signer_records, *first_records, *second_records]:
            for key in ("stdout", "stderr", "invocation_record", "output"):
                if key in row:
                    item = row[key]
                    expected[item["path"]] = (item["sha256"], item["size"])
        if set(custody.scan_inventory_paths(bundle)) != set(expected):
            raise BundleError("CUDA candidate output inventory changed before publication")
        for path, (digest, size) in expected.items():
            actual = custody.stable_hash_path(bundle / path, max_size=max(MAX_SOURCE_BYTES, MAX_PTX_BYTES, MAX_LOG_BYTES), allow_empty=size == 0)
            if (actual.sha256, actual.size) != (digest, size):
                raise BundleError(f"CUDA candidate evidence changed before publication: {path}")
        check_custody()
        custody.publish_directory_noreplace(bundle, output_dir, parent_fd=parent_fd, stage_fd=stage_fd)
    finally:
        if stage_fd >= 0:
            os.close(stage_fd)
        os.close(parent_fd)
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
    except (custody.ReleaseArtifactError, OSError) as error:
        parser.error(str(error))
    print(f"candidate={output_dir}")
    print(f"trusted_key_sha256={fingerprint}")
    print(f"generation_sha256={generation}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
