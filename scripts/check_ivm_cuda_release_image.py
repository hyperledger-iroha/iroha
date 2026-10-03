#!/usr/bin/env python3
"""Bind reviewed CUDA release inputs to a Docker image inspection and source bundle.

Requires Python 3.10+, an explicit digest-pinned image reference, independently
reviewed nonzero image/key SHA-256 values, and `docker image inspect --format
'{{json .}}'` output. Does not pull images, run builds, or assert a signature/GPU
qualification pass. Rust bundled admission still owns signature and PTX validation.
The create-only receipt is local input-binding evidence, not release approval.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

if __package__:
    from . import release_artifact_contract as custody
else:
    import release_artifact_contract as custody

SHA256 = re.compile(r"[0-9a-f]{64}\Z")
IMAGE = re.compile(r"[a-z0-9][a-z0-9./:_-]*@sha256:([0-9a-f]{64})\Z")


def configured_image(reference: str, key_digest: str, image_digest: str) -> str:
    """Require explicit reviewed inputs before any image pull or Cargo work."""
    for label, value in (("trusted CUDA key", key_digest), ("CUDA image", image_digest)):
        if not SHA256.fullmatch(value) or value == "0" * 64:
            raise custody.ReleaseArtifactError(f"{label} requires an explicit reviewed nonzero SHA256")
    match = IMAGE.fullmatch(reference)
    if match is None or match[1] != image_digest:
        raise custody.ReleaseArtifactError("CUDA image reference must pin the independently reviewed digest")
    return reference


def verify(reference: str, key_digest: str, image_digest: str,
           inspection: Path, bundle: Path, output: Path) -> Path:
    """Bind Docker's measured repository/config identities to the reviewed inputs."""
    configured_image(reference, key_digest, image_digest)
    inspection_info, data = custody.stable_read_path(inspection, max_size=1024 * 1024)
    observed = custody.load_json_object(data, "Docker image inspection")
    identities = observed.get("RepoDigests")
    identity = observed.get("Id")
    if (not isinstance(identities, list) or not all(isinstance(value, str) for value in identities)
            or reference not in identities
            or not isinstance(identity, str) or not identity.startswith("sha256:")
            or not SHA256.fullmatch(identity[7:]) or identity[7:] == "0" * 64
            or observed.get("Os") != "linux" or observed.get("Architecture") not in {"amd64", "arm64"}):
        raise custody.ReleaseArtifactError("Docker image inspection does not identify the pinned Linux CUDA image")
    manifest_info, manifest = custody.stable_read_relative(bundle, "provenance.v1", max_size=16 * 1024, return_payload=True)
    public_info, public = custody.stable_read_relative(bundle, "provenance.v1.pub", max_size=32, return_payload=True)
    if len(public) != 32 or public_info.sha256 != key_digest:
        raise custody.ReleaseArtifactError("CUDA source public key differs from independently reviewed fingerprint")
    try:
        lines = manifest.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise custody.ReleaseArtifactError("CUDA source provenance must be ASCII") from error
    if (not manifest.endswith(b"\n") or b"\r" in manifest or len(lines) < 2
            or lines[:2] != ["ivm-cuda-ptx-provenance-v1", f"cuda_image_sha256={image_digest}"]):
        raise custody.ReleaseArtifactError("CUDA source image claim differs from the independently pinned image")
    receipt = {
        "schema": "ivm.cuda-release-image-binding.v1",
        "scope": "Reviewed input and Docker inspection binding only; bundled Rust admission and physical execution remain required.",
        "image_reference": reference,
        "image_config_digest": identity,
        "image_manifest_sha256": image_digest,
        "trusted_key_sha256": key_digest,
        "source_manifest_sha256": manifest_info.sha256,
        "inspection_sha256": inspection_info.sha256,
        "signature_verified": False,
        "hardware_qualified": False,
    }
    custody.exclusive_write_bytes(output, custody.canonical_json_bytes(receipt), mode=0o600)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image-reference", required=True)
    parser.add_argument("--trusted-key-sha256", required=True)
    parser.add_argument("--image-sha256", required=True)
    parser.add_argument("--inspection", type=Path, required=True)
    parser.add_argument("--bundle-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        output = verify(args.image_reference, args.trusted_key_sha256, args.image_sha256,
                        args.inspection, args.bundle_dir, args.output)
    except (custody.ReleaseArtifactError, OSError) as error:
        print(f"CUDA release image binding failed: {error}", file=sys.stderr)
        return 1
    print(f"CUDA image inputs bound; signature and hardware checks remain required: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
