#!/usr/bin/env python3
"""Check a standalone physical production App Attest capture."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_app_attest_capture as capture_evidence
import kagemusha_production_ios_evidence as production_evidence


class OutputPublicationUncertain(candidate_evidence.EvidenceError):
    """Raised when a non-transactional output group may be partly published."""


def _prepare_new_outputs(
    requested: tuple[tuple[str | None, dict[str, object]], ...],
) -> list[tuple[Path, dict[str, object]]]:
    """Validate every optional output before publishing the first one."""

    prepared: list[tuple[Path, dict[str, object]]] = []
    seen: set[Path] = set()
    for output_text, value in requested:
        if output_text is None:
            continue
        output = Path(output_text)
        if not output.is_absolute() or output.name in {"", ".", ".."}:
            raise candidate_evidence.EvidenceError(
                "outputs must be new absolute files beneath existing private directories"
            )
        try:
            parent = output.parent.resolve(strict=True)
            candidate_evidence._validate_private_directory(
                parent, "capture validation output parent"
            )
        except OSError as error:
            raise candidate_evidence.EvidenceError(
                "capture validation output metadata could not be read"
            ) from error
        target = parent / output.name
        try:
            target.lstat()
        except FileNotFoundError:
            pass
        except OSError as error:
            raise candidate_evidence.EvidenceError(
                "capture validation output metadata could not be read"
            ) from error
        else:
            raise candidate_evidence.EvidenceError(
                "capture validation output already exists"
            )
        if target in seen:
            raise candidate_evidence.EvidenceError(
                "platform evidence and summary outputs must be distinct files"
            )
        seen.add(target)
        prepared.append((target, value))
    return prepared


def _publish_new_outputs(
    outputs: list[tuple[Path, dict[str, object]]],
) -> None:
    """Publish new outputs without ever deleting a concurrently changed name.

    Separate pathnames cannot be committed or conditionally rolled back as one
    portable filesystem transaction. Once any output is linked, a later error
    is therefore reported as commit-uncertain and every final name is left
    untouched for operator inspection.
    """

    published: list[candidate_evidence.FileSnapshot] = []
    for output, value in outputs:
        try:
            snapshot = candidate_evidence.write_new_private_json(output, value)
        except candidate_evidence.NewPrivateJsonPublicationUncertain as error:
            raise OutputPublicationUncertain(
                "capture output publication is uncertain: an output reached its final "
                "name without a confirmed durable commit; no final names were removed"
            ) from error
        except candidate_evidence.EvidenceError as error:
            if published:
                raise OutputPublicationUncertain(
                    "capture output group is incomplete: an earlier output was published; "
                    "no final names were removed"
                ) from error
            raise
        published.append(snapshot)

    for snapshot in published:
        try:
            current = candidate_evidence._snapshot_private_file(
                snapshot.path,
                "published capture output",
                maximum=candidate_evidence.MAX_JSON_BYTES,
                retain_payload=True,
            )
        except (OSError, candidate_evidence.EvidenceError) as error:
            raise OutputPublicationUncertain(
                "capture output group changed before final verification; "
                "no final names were removed"
            ) from error
        if (
            current.identity != snapshot.identity
            or current.sha256 != snapshot.sha256
            or current.payload != snapshot.payload
        ):
            raise OutputPublicationUncertain(
                "capture output group changed before final verification; "
                "no final names were removed"
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture", required=True)
    parser.add_argument("--request", required=True)
    parser.add_argument("--production-policy", required=True)
    parser.add_argument("--capture-app-code-sign-measurements")
    parser.add_argument("--platform-evidence-output")
    parser.add_argument("--summary-output")
    args = parser.parse_args()

    errors, platform_evidence, summary = capture_evidence.validate_capture(
        Path(args.capture),
        Path(args.request),
        Path(args.production_policy),
        candidate_evidence,
        production_evidence,
        capture_app_code_sign_measurements_path=(
            Path(args.capture_app_code_sign_measurements)
            if args.capture_app_code_sign_measurements is not None
            else None
        ),
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-app-attest-capture] ERROR: {error}", file=sys.stderr)
        return 1
    assert platform_evidence is not None
    assert summary is not None
    try:
        outputs = _prepare_new_outputs(
            (
                (args.platform_evidence_output, platform_evidence),
                (args.summary_output, summary),
            )
        )
        _publish_new_outputs(outputs)
    except OutputPublicationUncertain as error:
        print(f"[kagemusha-app-attest-capture] ERROR: {error}", file=sys.stderr)
        return 75
    except candidate_evidence.EvidenceError as error:
        print(f"[kagemusha-app-attest-capture] ERROR: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-app-attest-capture] production App Attest capture is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
