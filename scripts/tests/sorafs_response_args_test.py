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
    MAX_EXPANDED_ARGS,
    MAX_RESPONSE_ARGFILE_BYTES,
    expand_response_args,
    non_negative_int_arg,
    parse_int_arg,
    positive_int_arg,
)


def test_response_file_resolution_uses_shared_identity_helper() -> None:
    import sorafs_response_args

    assert (
        sorafs_response_args.expand_response_args.__globals__[
            "resolve_path_identity"
        ].__module__
        == "sorafs_path_identity"
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


def test_direct_non_string_argument_fails_without_traceback() -> None:
    try:
        expand_response_args(["--dry-run", 7], EvidenceArgumentParser())
    except ValueError as error:
        assert "argument `7` must be a string" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("non-string response argument was accepted")


def test_raw_string_argument_container_fails_without_character_expansion() -> None:
    try:
        expand_response_args("--dry-run", EvidenceArgumentParser())
    except ValueError as error:
        assert "arguments must be a sequence of strings" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("raw string arguments were expanded character-wise")


def test_raw_bytes_argument_container_fails_without_character_expansion() -> None:
    try:
        expand_response_args(b"--dry-run", EvidenceArgumentParser())
    except ValueError as error:
        assert "arguments must be a sequence of strings" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("raw bytes arguments were expanded byte-wise")


def test_raw_bytearray_argument_container_fails_without_byte_expansion() -> None:
    try:
        expand_response_args(bytearray(b"--dry-run"), EvidenceArgumentParser())
    except ValueError as error:
        assert "arguments must be a sequence of strings" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("raw bytearray arguments were expanded byte-wise")


def test_mapping_argument_container_fails_without_key_expansion() -> None:
    try:
        expand_response_args({"--dry-run": "ignored"}, EvidenceArgumentParser())
    except ValueError as error:
        assert "arguments must be a sequence of strings" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("mapping arguments were expanded from keys")


def test_malformed_direct_argument_text_fails_closed() -> None:
    for value in ("", " --dry-run", "--dry-run ", "--dry\nrun"):
        try:
            expand_response_args([value], EvidenceArgumentParser())
        except ValueError as error:
            assert "argument must be a non-empty canonical string" in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError(f"malformed argument {value!r} was accepted")


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


def test_response_file_stat_failure_sanitizes_malformed_error(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")
    original_stat = Path.stat
    bad_message = "argfile stat denied\nsecret"

    def stat(self: Path, *args, **kwargs):
        if self == args_file:
            raise RuntimeError(bad_message)
        return original_stat(self, *args, **kwargs)

    monkeypatch.setattr(Path, "stat", stat)

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert str(error) == (
            f"failed to stat @ARGFILE `{args_file}`: <non-canonical-error>"
        )
        assert bad_message not in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("malformed stat failure escaped response-file handling")


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


def test_response_file_read_failure_sanitizes_malformed_error(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args_file = tmp_path / "reviewed.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")
    original_read_bytes = Path.read_bytes
    bad_message = "argfile read denied\nsecret"

    def read_bytes(self: Path) -> bytes:
        if self == args_file:
            raise RuntimeError(bad_message)
        return original_read_bytes(self)

    monkeypatch.setattr(Path, "read_bytes", read_bytes)

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert str(error) == (
            f"failed to read @ARGFILE `{args_file}`: <non-canonical-error>"
        )
        assert bad_message not in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("malformed read failure escaped response-file handling")


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


def test_response_file_line_parse_error_sanitizes_malformed_error(
    tmp_path: Path,
) -> None:
    class BrokenParser(EvidenceArgumentParser):
        def convert_arg_line_to_args(self, arg_line: str):
            raise ValueError("line parse denied\nsecret")

    args_file = tmp_path / "broken.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], BrokenParser())
    except ValueError as error:
        assert str(error) == (
            f"@ARGFILE `{args_file}` line 1: <non-canonical-error>"
        )
        assert "line parse denied" not in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("malformed response-file line error leaked diagnostics")


def test_response_file_parser_returning_scalar_line_args_fails_with_line(
    tmp_path: Path,
) -> None:
    class BrokenParser(EvidenceArgumentParser):
        def convert_arg_line_to_args(self, arg_line: str):
            return "--dry-run"

    args_file = tmp_path / "broken-parser.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], BrokenParser())
    except ValueError as error:
        assert f"@ARGFILE `{args_file}` line 1" in str(error)
        assert "response-file line arguments must be a sequence of strings" in str(
            error
        )
    else:  # pragma: no cover - defensive
        raise AssertionError("scalar line-argument container was accepted")


def test_response_file_parser_returning_non_string_line_arg_fails_with_line(
    tmp_path: Path,
) -> None:
    class BrokenParser(EvidenceArgumentParser):
        def convert_arg_line_to_args(self, arg_line: str):
            return ["--dry-run", 7]

    args_file = tmp_path / "broken-parser.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], BrokenParser())
    except ValueError as error:
        assert f"@ARGFILE `{args_file}` line 1" in str(error)
        assert "argument `7` must be a string" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("non-string line argument was accepted")


def test_response_file_parser_returning_malformed_line_arg_fails_with_line(
    tmp_path: Path,
) -> None:
    class BrokenParser(EvidenceArgumentParser):
        def convert_arg_line_to_args(self, arg_line: str):
            return ["--dry-run", " bad"]

    args_file = tmp_path / "broken-parser.args"
    args_file.write_text("--dry-run\n", encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], BrokenParser())
    except ValueError as error:
        assert f"@ARGFILE `{args_file}` line 1" in str(error)
        assert "argument must be a non-empty canonical string" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("malformed line argument was accepted")


def test_convert_arg_line_to_args_rejects_non_string_line() -> None:
    try:
        EvidenceArgumentParser().convert_arg_line_to_args(b"--dry-run")
    except ValueError as error:
        assert "response-file line must be a string" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("non-string response-file line was accepted")


def test_oversized_response_file_fails(tmp_path: Path) -> None:
    args_file = tmp_path / "large.args"
    args_file.write_text("x" * (MAX_RESPONSE_ARGFILE_BYTES + 1), encoding="utf-8")

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert "exceeds" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("oversized response file was accepted")


def test_direct_expanded_argument_limit_fails() -> None:
    try:
        expand_response_args(
            ["--flag"] * (MAX_EXPANDED_ARGS + 1), EvidenceArgumentParser()
        )
    except ValueError as error:
        assert f"expanded arguments must be <= {MAX_EXPANDED_ARGS}" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("direct expanded arguments bypassed the shared cap")


def test_response_file_expanded_argument_limit_fails(tmp_path: Path) -> None:
    args_file = tmp_path / "too-many.args"
    args_file.write_text(
        " ".join(["--flag"] * (MAX_EXPANDED_ARGS + 1)), encoding="utf-8"
    )

    try:
        expand_response_args([f"@{args_file}"], EvidenceArgumentParser())
    except ValueError as error:
        assert f"expanded arguments must be <= {MAX_EXPANDED_ARGS}" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("response-file expanded arguments bypassed the shared cap")


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


def test_shared_integer_arg_parsers_reject_non_string_values() -> None:
    for value in [7, True, None, b"7"]:
        try:
            parse_int_arg(value)
        except argparse.ArgumentTypeError as error:
            assert "must be an integer" in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError(f"parse_int_arg accepted {value!r}")


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
