#!/usr/bin/env python3
"""Validate and sign one release-bound Kagemusha production iOS envelope."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
import tempfile
from typing import Optional


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_ios_evidence as production_evidence


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _publish_new_validated_envelope(
    output: Path,
    evidence: dict[str, object],
    *,
    artifact_root: Path,
    signer_key_id: str,
    public_key: Path,
    production_policy: Path,
) -> None:
    """Validate a private temporary file, then publish it without replacement."""

    if not output.is_absolute() or output.name in {"", ".", ".."}:
        raise candidate_evidence.EvidenceError(
            "production signed evidence output must be an absolute file path"
        )
    try:
        parent = output.parent.resolve(strict=True)
        artifact_absolute = artifact_root.resolve(strict=True)
    except OSError as error:
        raise candidate_evidence.EvidenceError(
            "production output parent or artifact root could not be resolved"
        ) from error
    candidate_evidence._validate_private_directory(
        parent, "production signed evidence output parent"
    )
    target = parent / output.name
    try:
        target.relative_to(artifact_absolute)
    except ValueError:
        pass
    else:
        raise candidate_evidence.EvidenceError(
            "production signed evidence output must stay outside artifact root"
        )
    try:
        target.lstat()
    except FileNotFoundError:
        pass
    except OSError as error:
        raise candidate_evidence.EvidenceError(
            "production signed evidence output metadata could not be read"
        ) from error
    else:
        raise candidate_evidence.EvidenceError(
            "production signed evidence output already exists"
        )

    payload = candidate_evidence.canonical_json_bytes(evidence)
    descriptor, temporary_text = tempfile.mkstemp(
        prefix=f".{target.name}.", dir=os.fspath(parent)
    )
    temporary = Path(temporary_text)
    linked = False
    try:
        os.fchmod(descriptor, 0o600)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short production envelope write")
            offset += written
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1

        errors = production_evidence.validate_production_signed_evidence(
            temporary,
            artifact_absolute,
            signer_key_id,
            public_key,
            production_policy,
            candidate_evidence,
        )
        if errors != [production_evidence.MISSING_FRESHNESS_RECEIPT]:
            raise candidate_evidence.EvidenceError(
                "production envelope failed its pre-publication validation: "
                + "; ".join(errors)
            )

        os.link(temporary, target, follow_symlinks=False)
        linked = True
        _fsync_directory(parent)
        temporary.unlink()
        _fsync_directory(parent)
        snapshot = candidate_evidence._snapshot_private_file(
            target,
            "published production signed evidence",
            maximum=candidate_evidence.MAX_JSON_BYTES,
            retain_payload=True,
        )
        if snapshot.payload != payload:
            raise candidate_evidence.EvidenceError(
                "published production signed evidence bytes changed"
            )
    except FileExistsError as error:
        raise candidate_evidence.EvidenceError(
            "production signed evidence output already exists"
        ) from error
    except candidate_evidence.EvidenceError:
        raise
    except OSError as error:
        message = (
            "production envelope publication commit state is uncertain"
            if linked
            else "production envelope could not be published"
        )
        raise candidate_evidence.EvidenceError(message) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-root", required=True)
    parser.add_argument("--platform-evidence", required=True)
    parser.add_argument("--production-policy", required=True)
    parser.add_argument("--release-manifest-sha256", required=True)
    parser.add_argument("--private-key", required=True)
    parser.add_argument("--public-key", required=True)
    parser.add_argument("--signer-key-id", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    try:
        evidence = production_evidence.build_production_signed_evidence(
            Path(args.artifact_root),
            Path(args.platform_evidence),
            Path(args.production_policy),
            args.release_manifest_sha256,
            Path(args.private_key),
            Path(args.public_key),
            args.signer_key_id,
            candidate_evidence,
        )
        _publish_new_validated_envelope(
            Path(args.output),
            evidence,
            artifact_root=Path(args.artifact_root),
            signer_key_id=args.signer_key_id,
            public_key=Path(args.public_key),
            production_policy=Path(args.production_policy),
        )
    except candidate_evidence.EvidenceError as error:
        print(f"[kagemusha-production-ios-signer] ERROR: {error}", file=sys.stderr)
        return 1
    print(
        f"[kagemusha-production-ios-signer] signed production envelope: {args.output}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
