"""Tests for shared SoraFS evidence JSON loading."""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_json import (  # noqa: E402
    decode_evidence_json,
    EvidenceFileTooLargeError,
    EVIDENCE_JSON_LOAD_DIAGNOSTIC,
    EVIDENCE_JSON_READ_DIAGNOSTIC,
    evidence_read_open_flags,
    load_evidence_json,
    load_evidence_json_with_sha256,
    load_evidence_json_with_sha256_or_record_error,
    read_evidence_bytes,
    validate_evidence_file_for_read,
)


def test_load_evidence_json_returns_object(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    assert load_evidence_json(evidence, 1024) == {"schema": "test"}


def test_load_evidence_json_with_sha256_hashes_same_bytes(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    raw = b'{"schema":"test"}'
    evidence.write_bytes(raw)

    payload, digest = load_evidence_json_with_sha256(evidence, 1024)

    assert payload == {"schema": "test"}
    assert digest == hashlib.sha256(raw).hexdigest()


def test_load_evidence_json_with_sha256_or_record_error_returns_payload(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    raw = b'{"schema":"test"}'
    evidence.write_bytes(raw)
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded == ({"schema": "test"}, hashlib.sha256(raw).hexdigest())
    assert errors == []


def test_read_evidence_bytes_uses_no_follow_open_flags(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence = tmp_path / "evidence.json"
    raw = b'{"schema":"test"}'
    evidence.write_bytes(raw)
    original_open = os.open
    captured: dict[str, int] = {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == evidence:
            captured["flags"] = flags
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(os, "open", open_path)

    assert read_evidence_bytes(evidence, 1024) == raw
    assert captured["flags"] == evidence_read_open_flags()
    if hasattr(os, "O_NOFOLLOW"):
        assert captured["flags"] & os.O_NOFOLLOW


def test_load_evidence_json_with_sha256_or_record_error_records_failure(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("[]", encoding="utf-8")
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: evidence root must be a JSON object"
    ]
    assert str(evidence) not in errors[0]


def test_load_evidence_json_rejects_non_path_without_traceback() -> None:
    try:
        load_evidence_json("evidence.json", 1024)
    except ValueError as error:
        assert "evidence path must be a path" in str(error)
    else:
        raise AssertionError("expected non-path evidence to fail")


def test_load_evidence_json_with_sha256_or_record_error_records_non_path() -> None:
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(
        "evidence.json",
        1024,
        errors,
    )

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: evidence path must be a path"
    ]
    assert "evidence.json:" not in errors[0]


def test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_path_label() -> None:
    for path in (" bad.json", "bad\npath.json", b"bad.json"):
        errors: list[str] = []

        loaded = load_evidence_json_with_sha256_or_record_error(
            path,
            1024,
            errors,
        )

        assert loaded is None
        assert errors == [
            f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: evidence path must be a path"
        ]
        assert "\n" not in errors[0]
        assert "<non-path>" not in errors[0]
        assert "<non-canonical-path>" not in errors[0]


def test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_os_error(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "bad\npath.json"
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: evidence file must exist and be a file"
    ]
    assert str(evidence) not in errors[0]


def test_load_evidence_json_with_sha256_or_record_error_rejects_malformed_errors(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            load_evidence_json_with_sha256_or_record_error(
                evidence,
                1024,
                errors,
            )
        except ValueError as error:
            assert "evidence JSON errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_load_evidence_json_with_sha256_or_record_error_rejects_malformed_existing_error_text(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    for errors in ([""], [" old"], ["old "], ["old\nerror"]):
        try:
            load_evidence_json_with_sha256_or_record_error(
                evidence,
                1024,
                errors,
            )
        except ValueError as error:
            assert (
                "evidence JSON errors must contain non-empty canonical strings"
                in str(error)
            )
        else:
            raise AssertionError(f"accepted malformed error text {errors!r}")


def test_load_evidence_json_rejects_invalid_byte_limit_without_traceback(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    for max_bytes in (0, -1, True, "1024"):
        try:
            load_evidence_json(evidence, max_bytes)
        except ValueError as error:
            assert "evidence byte limit must be positive" in str(error)
        else:
            raise AssertionError(f"expected byte limit {max_bytes!r} to fail")


def test_read_evidence_bytes_rejects_symlink_before_open(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text('{"schema":"test"}', encoding="utf-8")
    symlink.symlink_to(target)

    try:
        read_evidence_bytes(symlink, 1024)
    except ValueError as error:
        assert str(error) == "evidence file must not be a symlink"
        assert str(symlink) not in str(error)
    else:
        raise AssertionError("expected symlink evidence read to fail")


def test_read_evidence_bytes_rejects_directory_before_open(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()

    try:
        read_evidence_bytes(evidence_dir, 1024)
    except ValueError as error:
        assert str(error) == "evidence file must exist and be a file"
        assert str(evidence_dir) not in str(error)
    else:
        raise AssertionError("expected directory evidence read to fail")


def test_load_evidence_json_with_sha256_or_record_error_accepts_parent_symlink(
    tmp_path: Path,
) -> None:
    target_root = tmp_path / "target-root"
    symlink_root = tmp_path / "evidence-root"
    target = target_root / "evidence.json"
    raw = b'{"schema":"test"}'
    target_root.mkdir()
    target.write_bytes(raw)
    symlink_root.symlink_to(target_root, target_is_directory=True)

    evidence = symlink_root / "evidence.json"
    errors: list[str] = []
    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded == ({"schema": "test"}, hashlib.sha256(raw).hexdigest())
    assert errors == []


def test_validate_evidence_file_for_read_records_inspection_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")
    original_is_symlink = Path.is_symlink

    def is_symlink(path: Path) -> bool:
        if path == evidence:
            raise RuntimeError("evidence symlink stat denied")
        return original_is_symlink(path)

    monkeypatch.setattr(Path, "is_symlink", is_symlink)

    try:
        validate_evidence_file_for_read(evidence)
    except RuntimeError as error:
        assert str(error) == "evidence file cannot be inspected"
        assert str(evidence) not in str(error)
        assert "evidence symlink stat denied" not in str(error)
    else:
        raise AssertionError("expected evidence inspection failure")


def test_load_evidence_json_with_sha256_or_record_error_records_runtime_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")
    original_open = os.open

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == evidence:
            raise RuntimeError("evidence read denied")
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(os, "open", open_path)
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: {EVIDENCE_JSON_READ_DIAGNOSTIC}"
    ]
    assert str(evidence) not in errors[0]
    assert "evidence read denied" not in errors[0]


def test_load_evidence_json_rejects_oversized_file(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    try:
        load_evidence_json(evidence, 1)
    except ValueError as error:
        assert "evidence file exceeds 1 bytes" in str(error)
    else:
        raise AssertionError("expected oversized evidence to fail")


def test_read_evidence_bytes_raises_typed_oversize_error(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    try:
        read_evidence_bytes(evidence, 1)
    except EvidenceFileTooLargeError as error:
        assert error.max_bytes == 1
        assert str(error) == "evidence file exceeds 1 bytes"
        assert isinstance(error, ValueError)
    else:
        raise AssertionError("expected typed oversized evidence failure")


def test_load_evidence_json_with_sha256_rejects_oversized_file(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    try:
        load_evidence_json_with_sha256(evidence, 1)
    except ValueError as error:
        assert "evidence file exceeds 1 bytes" in str(error)
    else:
        raise AssertionError("expected oversized evidence to fail")


def test_load_evidence_json_rejects_non_object_root(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("[]", encoding="utf-8")

    try:
        load_evidence_json(evidence, 1024)
    except ValueError as error:
        assert "evidence root must be a JSON object" in str(error)
    else:
        raise AssertionError("expected non-object evidence to fail")


def test_load_evidence_json_rejects_malformed_json(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{not-json", encoding="utf-8")

    try:
        load_evidence_json(evidence, 1024)
    except json.JSONDecodeError:
        pass
    else:
        raise AssertionError("expected malformed JSON to fail")


def test_decode_evidence_json_rejects_non_byte_input_without_traceback() -> None:
    try:
        decode_evidence_json('{"schema":"test"}')
    except ValueError as error:
        assert "evidence JSON bytes must be bytes" in str(error)
    else:
        raise AssertionError("expected non-byte evidence JSON to fail")


def test_load_evidence_json_rejects_duplicate_top_level_keys(tmp_path: Path) -> None:
    evidence = tmp_path / "duplicate.json"
    evidence.write_text('{"schema":"good","schema":"shadow"}', encoding="utf-8")

    try:
        load_evidence_json(evidence, 1024)
    except ValueError as error:
        assert "evidence JSON object contains duplicate key `schema`" in str(error)
    else:
        raise AssertionError("expected duplicate top-level key to fail")


def test_load_evidence_json_rejects_duplicate_nested_keys(tmp_path: Path) -> None:
    evidence = tmp_path / "duplicate-nested.json"
    evidence.write_text(
        '{"schema":"test","payload":{"status":"ready","status":"blocked"}}',
        encoding="utf-8",
    )

    try:
        load_evidence_json(evidence, 1024)
    except ValueError as error:
        assert "evidence JSON object contains duplicate key `status`" in str(error)
    else:
        raise AssertionError("expected duplicate nested key to fail")


def test_decode_evidence_json_sanitizes_malformed_duplicate_key_label() -> None:
    for raw in (
        b'{"bad\\nkey":1,"bad\\nkey":2}',
        b'{" padded":1," padded":2}',
    ):
        try:
            decode_evidence_json(raw)
        except ValueError as error:
            message = str(error)
            assert "evidence JSON object contains duplicate key `<non-canonical>`" in (
                message
            )
            assert "\n" not in message
        else:
            raise AssertionError("expected malformed duplicate key to fail")


def test_load_evidence_json_with_sha256_or_record_error_records_duplicate_key(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "duplicate.json"
    evidence.write_text('{"schema":"good","schema":"shadow"}', encoding="utf-8")
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: "
        "evidence JSON object contains duplicate key `schema`"
    ]
    assert str(evidence) not in errors[0]


def test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_duplicate_key(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "duplicate.json"
    evidence.write_text('{"bad\\nkey":1,"bad\\nkey":2}', encoding="utf-8")
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: "
        "evidence JSON object contains duplicate key `<non-canonical>`"
    ]
    assert str(evidence) not in errors[0]


def test_load_evidence_json_rejects_non_standard_numeric_constants(
    tmp_path: Path,
) -> None:
    for constant in ("NaN", "Infinity", "-Infinity"):
        evidence = tmp_path / f"{constant}.json"
        evidence.write_text(f'{{"latency_ms": {constant}}}', encoding="utf-8")

        try:
            load_evidence_json(evidence, 1024)
        except ValueError as error:
            assert f"non-standard JSON constant `{constant}` is not allowed" in str(
                error
            )
        else:
            raise AssertionError(f"expected {constant} evidence to fail")
