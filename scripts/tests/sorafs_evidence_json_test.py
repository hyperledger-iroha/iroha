"""Tests for shared SoraFS evidence JSON loading."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_json import (  # noqa: E402
    decode_evidence_json,
    load_evidence_json,
    load_evidence_json_with_sha256,
    load_evidence_json_with_sha256_or_record_error,
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


def test_load_evidence_json_with_sha256_or_record_error_records_failure(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("[]", encoding="utf-8")
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{evidence}: failed to load evidence JSON: "
        "evidence root must be a JSON object"
    ]


def test_load_evidence_json_rejects_non_path_without_traceback() -> None:
    try:
        load_evidence_json("evidence.json", 1024)
    except ValueError as error:
        assert "evidence path `evidence.json` must be a path" in str(error)
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
        "evidence.json: failed to load evidence JSON: "
        "evidence path `evidence.json` must be a path"
    ]


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


def test_load_evidence_json_with_sha256_or_record_error_records_runtime_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")
    original_open = Path.open

    def open_path(path: Path, *args, **kwargs):
        if path == evidence:
            raise RuntimeError("evidence read denied")
        return original_open(path, *args, **kwargs)

    monkeypatch.setattr(Path, "open", open_path)
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{evidence}: failed to load evidence JSON: evidence read denied"
    ]


def test_load_evidence_json_rejects_oversized_file(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text('{"schema":"test"}', encoding="utf-8")

    try:
        load_evidence_json(evidence, 1)
    except ValueError as error:
        assert "evidence file exceeds 1 bytes" in str(error)
    else:
        raise AssertionError("expected oversized evidence to fail")


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


def test_load_evidence_json_with_sha256_or_record_error_records_duplicate_key(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "duplicate.json"
    evidence.write_text('{"schema":"good","schema":"shadow"}', encoding="utf-8")
    errors: list[str] = []

    loaded = load_evidence_json_with_sha256_or_record_error(evidence, 1024, errors)

    assert loaded is None
    assert errors == [
        f"{evidence}: failed to load evidence JSON: "
        "evidence JSON object contains duplicate key `schema`"
    ]


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
