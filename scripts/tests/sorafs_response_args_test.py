"""Tests for shared SoraFS response-file expansion."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    MAX_RESPONSE_ARGFILE_BYTES,
    expand_response_args,
    non_negative_int_arg,
    parse_int_arg,
    positive_int_arg,
)


def test_response_file_expands_shell_style_args(tmp_path: Path) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_text(
        "\n".join(
            [
                "# comments are ignored",
                "",
                "--evidence-dir evidence",
                "--require-kind 'feed collector'",
            ]
        ),
        encoding="utf-8",
    )

    expanded = expand_response_args(
        [f"@{args_file}", "--dry-run"], EvidenceArgumentParser()
    )

    assert expanded == [
        "--evidence-dir",
        "evidence",
        "--require-kind",
        "feed collector",
        "--dry-run",
    ]


def test_recursive_response_file_fails(tmp_path: Path) -> None:
    args_file = tmp_path / "loop.args"
    args_file.write_text(f"@{args_file}\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert "recursive @ARGFILE" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("recursive response file was accepted")


def test_directory_response_file_fails(tmp_path: Path) -> None:
    try:
        expand_response_args([f"@{tmp_path}"], EvidenceArgumentParser())
    except ValueError as error:
        assert "must exist and be a file" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("directory response file was accepted")


def test_response_file_resolve_failure_is_stable_value_error(
    tmp_path: Path, monkeypatch
) -> None:
    args_file = tmp_path / "loop.args"
    original_resolve = Path.resolve

    def fail_selected_path(self: Path, *args, **kwargs):
        if self == args_file:
            raise RuntimeError("symlink loop")
        return original_resolve(self, *args, **kwargs)

    monkeypatch.setattr(Path, "resolve", fail_selected_path)

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"failed to resolve @ARGFILE `{args_file}`" in str(error)
        assert "symlink loop" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("resolver failure escaped response-file handling")


def test_response_file_stat_failure_is_stable_value_error(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")
    original_stat = Path.stat

    def stat(self: Path, *args, **kwargs):
        if self == args_file:
            raise RuntimeError("argfile stat denied")
        return original_stat(self, *args, **kwargs)

    monkeypatch.setattr(Path, "stat", stat)

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"failed to stat @ARGFILE `{args_file}`" in str(error)
        assert "argfile stat denied" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("stat failure escaped response-file handling")


def test_response_file_read_failure_is_stable_value_error(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")
    original_read_bytes = Path.read_bytes

    def read_bytes(self: Path) -> bytes:
        if self == args_file:
            raise RuntimeError("argfile read denied")
        return original_read_bytes(self)

    monkeypatch.setattr(Path, "read_bytes", read_bytes)

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"failed to read @ARGFILE `{args_file}`" in str(error)
        assert "argfile read denied" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("read failure escaped response-file handling")


def test_response_file_non_utf8_bytes_fail_stably(tmp_path: Path) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_bytes(b"\xff")

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"@ARGFILE `{args_file}` must be UTF-8" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("non-UTF-8 response file was accepted")


def test_response_file_line_parse_error_identifies_file_and_line(tmp_path: Path) -> None:
    args_file = tmp_path / "broken.args"
    args_file.write_text("--require-kind 'unterminated\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"@ARGFILE `{args_file}` line 1" in str(error)
        assert "No closing quotation" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("malformed response-file line was accepted")


def test_oversized_response_file_fails(tmp_path: Path) -> None:
    args_file = tmp_path / "large.args"
    args_file.write_text("x" * (MAX_RESPONSE_ARGFILE_BYTES + 1), encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert "exceeds" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("oversized response file was accepted")


def test_shared_integer_arg_parsers_accept_expected_values() -> None:
    assert parse_int_arg("-5") == -5
    assert parse_int_arg("0") == 0
    assert positive_int_arg("7") == 7
    assert non_negative_int_arg("0") == 0


def test_shared_integer_arg_parsers_reject_invalid_values() -> None:
    for parser, value, expected in [
        (parse_int_arg, "nan", "must be an integer"),
        (positive_int_arg, "0", "must be positive"),
        (positive_int_arg, "-1", "must be positive"),
        (non_negative_int_arg, "-1", "must be non-negative"),
    ]:
        try:
            parser(value)
        except argparse.ArgumentTypeError as error:
            assert expected in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError(f"{parser.__name__} accepted {value!r}")


def test_shared_integer_arg_parsers_reject_non_canonical_values() -> None:
    for value in [
        "+1",
        "-0",
        "01",
        "1_000",
        " 1",
        "1 ",
        "\N{FULLWIDTH DIGIT ONE}",
    ]:
        try:
            parse_int_arg(value)
        except argparse.ArgumentTypeError as error:
            assert "must be an integer" in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError(f"parse_int_arg accepted {value!r}")


def test_shared_response_parser_strips_inline_comments() -> None:
    parser = EvidenceArgumentParser()

    assert parser.convert_arg_line_to_args("  # only a comment") == []
    assert parser.convert_arg_line_to_args("--flag value # reviewed note") == [
        "--flag",
        "value",
    ]
