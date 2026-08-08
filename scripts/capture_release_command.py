#!/usr/bin/env python3
"""Capture bounded deterministic command output into an exclusive file."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    exclusive_write_bytes,
    stable_hash_path,
    stable_hash_relative,
    stable_read_relative,
)


MAX_CAPTURE_BYTES = 16 * 1024 * 1024
MAX_EXECUTABLE_BYTES = 256 * 1024 * 1024
VALIDATION_OUTCOME_FIELDS_V1 = frozenset(
    {
        "status",
        "code",
        "category",
        "message",
        "action",
        "docs_url",
        "telemetry_tags",
        "context",
        "inputs",
        "version",
        "generated_at",
    }
)
VALIDATION_OUTCOME_DOCS_URL_V1 = "https://docs.iroha.tech/"
VALIDATION_OUTCOME_MAX_ROWS_V1 = 1_024
VALIDATION_OUTCOME_MAX_STRING_BYTES_V1 = 4 * 1_024


def _decode_unique_json_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    decoded: dict[str, object] = {}
    for key, value in pairs:
        if key in decoded:
            raise ReleaseArtifactError(
                "captured validation outcome contains a duplicate JSON field"
            )
        decoded[key] = value
    return decoded


def _reject_nonfinite_json(_value: str) -> object:
    raise ReleaseArtifactError(
        "captured validation outcome contains a non-finite JSON number"
    )


def _bounded_string(value: object, label: str, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value):
        raise ReleaseArtifactError(
            f"captured validation outcome {label} must be a bounded string"
        )
    if len(value.encode("utf-8")) > VALIDATION_OUTCOME_MAX_STRING_BYTES_V1:
        raise ReleaseArtifactError(
            f"captured validation outcome {label} exceeds its byte limit"
        )
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise ReleaseArtifactError(
            f"captured validation outcome {label} contains a control character"
        )
    return value


def _validate_validation_outcome_ok_v1(
    captured: bytes,
    *,
    expected_code: str,
    expected_generated_at: int,
    required_telemetry_tags: tuple[str, ...],
) -> None:
    if captured.startswith(b"\xef\xbb\xbf"):
        raise ReleaseArtifactError(
            "captured validation outcome must not contain a UTF-8 BOM"
        )
    try:
        rendered = captured.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReleaseArtifactError(
            "captured validation outcome must be UTF-8"
        ) from error
    if (
        not rendered.startswith("{")
        or not rendered.endswith("}\n")
        or rendered.endswith("}\n\n")
        or "\r" in rendered
    ):
        raise ReleaseArtifactError(
            "captured validation outcome must be one newline-terminated JSON object"
        )
    try:
        outcome = json.loads(
            rendered,
            object_pairs_hook=_decode_unique_json_object,
            parse_constant=_reject_nonfinite_json,
        )
    except (json.JSONDecodeError, UnicodeError) as error:
        raise ReleaseArtifactError(
            "captured validation outcome is not canonical JSON"
        ) from error
    if not isinstance(outcome, dict) or set(outcome) != VALIDATION_OUTCOME_FIELDS_V1:
        raise ReleaseArtifactError(
            "captured validation outcome V1 field inventory is not exact"
        )
    if outcome["status"] != "Ok":
        raise ReleaseArtifactError("captured validation outcome did not succeed")
    if outcome["code"] != expected_code:
        raise ReleaseArtifactError(
            "captured validation outcome code does not match the release smoke"
        )
    if outcome["category"] != "validation":
        raise ReleaseArtifactError(
            "captured successful validation outcome category is not canonical"
        )
    _bounded_string(outcome["message"], "message")
    if outcome["action"] is not None:
        raise ReleaseArtifactError(
            "captured successful validation outcome must not carry an action"
        )
    if outcome["docs_url"] != VALIDATION_OUTCOME_DOCS_URL_V1:
        raise ReleaseArtifactError(
            "captured validation outcome documentation URL is not canonical"
        )
    if outcome["version"] != 1 or isinstance(outcome["version"], bool):
        raise ReleaseArtifactError(
            "captured validation outcome version is not V1"
        )
    if (
        outcome["generated_at"] != expected_generated_at
        or isinstance(outcome["generated_at"], bool)
    ):
        raise ReleaseArtifactError(
            "captured validation outcome timestamp does not match the release smoke"
        )

    telemetry_tags = outcome["telemetry_tags"]
    if (
        not isinstance(telemetry_tags, list)
        or not telemetry_tags
        or len(telemetry_tags) > VALIDATION_OUTCOME_MAX_ROWS_V1
    ):
        raise ReleaseArtifactError(
            "captured validation outcome telemetry inventory is not bounded"
        )
    normalized_tags = [
        _bounded_string(tag, "telemetry tag") for tag in telemetry_tags
    ]
    if len(set(normalized_tags)) != len(normalized_tags):
        raise ReleaseArtifactError(
            "captured validation outcome telemetry tags are not unique"
        )
    if not set(required_telemetry_tags).issubset(normalized_tags):
        raise ReleaseArtifactError(
            "captured validation outcome is missing a required telemetry tag"
        )

    context = outcome["context"]
    if not isinstance(context, list) or len(context) > VALIDATION_OUTCOME_MAX_ROWS_V1:
        raise ReleaseArtifactError(
            "captured validation outcome context inventory is not bounded"
        )
    context_keys: set[str] = set()
    for row in context:
        if not isinstance(row, dict) or set(row) != {"key", "value"}:
            raise ReleaseArtifactError(
                "captured validation outcome context row shape is not exact"
            )
        key = _bounded_string(row["key"], "context key")
        _bounded_string(row["value"], "context value", allow_empty=True)
        if key in context_keys:
            raise ReleaseArtifactError(
                "captured validation outcome context keys are not unique"
            )
        context_keys.add(key)

    inputs = outcome["inputs"]
    if (
        not isinstance(inputs, list)
        or not inputs
        or len(inputs) > VALIDATION_OUTCOME_MAX_ROWS_V1
    ):
        raise ReleaseArtifactError(
            "captured validation outcome input inventory is not bounded"
        )
    input_rows: set[tuple[str, str]] = set()
    for row in inputs:
        if not isinstance(row, dict) or set(row) != {"kind", "path"}:
            raise ReleaseArtifactError(
                "captured validation outcome input row shape is not exact"
            )
        normalized = (
            _bounded_string(row["kind"], "input kind"),
            _bounded_string(row["path"], "input path"),
        )
        if normalized in input_rows:
            raise ReleaseArtifactError(
                "captured validation outcome input rows are not unique"
            )
        input_rows.add(normalized)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--executable-root", required=True)
    parser.add_argument("--executable-relative", required=True)
    parser.add_argument("--trusted-executable-sha256")
    parser.add_argument("--require-validation-outcome-ok-v1", action="store_true")
    parser.add_argument("--expected-validation-code")
    parser.add_argument("--expected-generated-at")
    parser.add_argument("--required-telemetry-tag", action="append", default=[])
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.arguments or args.arguments[0] != "--":
        parser.error("command arguments must follow --")
    validation_options_present = (
        args.expected_validation_code is not None
        or args.expected_generated_at is not None
        or bool(args.required_telemetry_tag)
    )
    if args.require_validation_outcome_ok_v1:
        if (
            args.expected_validation_code is None
            or re.fullmatch(r"SFS-[A-Z0-9]+(?:-[A-Z0-9]+)*", args.expected_validation_code)
            is None
        ):
            parser.error(
                "--require-validation-outcome-ok-v1 requires one canonical --expected-validation-code"
            )
        if (
            args.expected_generated_at is None
            or re.fullmatch(r"0|[1-9][0-9]*", args.expected_generated_at) is None
            or int(args.expected_generated_at) > (1 << 64) - 1
        ):
            parser.error(
                "--require-validation-outcome-ok-v1 requires one canonical u64 --expected-generated-at"
            )
        try:
            required_tags = tuple(
                _bounded_string(tag, "required telemetry tag")
                for tag in args.required_telemetry_tag
            )
        except ReleaseArtifactError as error:
            parser.error(str(error))
        if not required_tags or len(set(required_tags)) != len(required_tags):
            parser.error(
                "--require-validation-outcome-ok-v1 requires unique --required-telemetry-tag values"
            )
        expected_generated_at = int(args.expected_generated_at)
    else:
        if validation_options_present:
            parser.error(
                "validation-outcome expectations require --require-validation-outcome-ok-v1"
            )
        required_tags = ()
        expected_generated_at = 0
    root = Path(args.executable_root)
    relative = args.executable_relative
    try:
        before, executable_payload = stable_read_relative(
            root,
            relative,
            max_size=MAX_EXECUTABLE_BYTES,
            return_payload=True,
        )
        assert executable_payload is not None
        if args.trusted_executable_sha256 is not None:
            if (
                re.fullmatch(
                    r"[0-9a-f]{64}",
                    args.trusted_executable_sha256,
                )
                is None
            ):
                raise ReleaseArtifactError(
                    "trusted release executable SHA256 must be 64 lowercase hex"
                )
            if before.sha256 != args.trusted_executable_sha256:
                raise ReleaseArtifactError(
                    "release executable SHA256 is not trusted"
                )
        if not before.mode & stat.S_IXUSR:
            raise ReleaseArtifactError(
                "captured release executable must be owner-executable"
            )
        temp_parent = os.path.realpath(tempfile.gettempdir())
        with tempfile.TemporaryDirectory(
            prefix="iroha-release-command.",
            dir=temp_parent,
        ) as private_directory_raw:
            private_executable = (
                Path(private_directory_raw) / "release-executable"
            )
            exclusive_write_bytes(
                private_executable,
                executable_payload,
                mode=0o755,
            )
            private_before = stable_hash_path(private_executable)
            if private_before.sha256 != before.sha256:
                raise ReleaseArtifactError(
                    "private release executable digest mismatch"
                )
            command_environment = os.environ.copy()
            command_environment[
                "IROHA_RELEASE_ORIGINAL_EXECUTABLE_ROOT"
            ] = os.path.abspath(root)
            process = subprocess.Popen(
                [str(private_executable), *args.arguments[1:]],
                stdout=subprocess.PIPE,
                stderr=None,
                env=command_environment,
            )
            assert process.stdout is not None
            captured = process.stdout.read(MAX_CAPTURE_BYTES + 1)
            if len(captured) > MAX_CAPTURE_BYTES:
                process.kill()
                process.wait()
                raise ReleaseArtifactError(
                    f"captured release output exceeds {MAX_CAPTURE_BYTES} bytes"
                )
            returncode = process.wait()
            after = stable_hash_relative(root, relative)
            if before != after:
                raise ReleaseArtifactError(
                    "release executable changed while its output was captured"
                )
            if stable_hash_path(private_executable) != private_before:
                raise ReleaseArtifactError(
                    "private release executable changed while its output was "
                    "captured"
                )
            if returncode != 0:
                return returncode
            if args.require_validation_outcome_ok_v1:
                _validate_validation_outcome_ok_v1(
                    captured,
                    expected_code=args.expected_validation_code,
                    expected_generated_at=expected_generated_at,
                    required_telemetry_tags=required_tags,
                )
            exclusive_write_bytes(Path(args.output), captured)
    except (OSError, ReleaseArtifactError, subprocess.SubprocessError) as exc:
        print(f"release command capture error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
