#!/usr/bin/env python3
"""Adversarial tests for the strict syscall documentation generator."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "gen_syscall_doc.py"
SPEC = importlib.util.spec_from_file_location("gen_syscall_doc", SCRIPT)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import machinery guard
    raise RuntimeError(f"could not import {SCRIPT}")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def row(number: str = "0x01") -> str:
    return (
        "[[syscall]]\n"
        f'number = "{number}"\n'
        'args = "r10=\\\"value\\\""\n'
        'ret = "u64=0"\n'
        'gas = "G_test + bytes"\n'
    )


class SyscallSpecParserTests(unittest.TestCase):
    def test_accepts_complete_canonical_rows(self) -> None:
        self.assertEqual(
            MODULE.parse_syscall_spec(row()),
            [
                {
                    "number": 1,
                    "args": 'r10="value"',
                    "ret": "u64=0",
                    "gas": "G_test + bytes",
                }
            ],
        )
        explicit_formula = row("0x02").replace(
            "G_test + bytes", "250,000 per proof + 5 per encoded byte"
        )
        self.assertEqual(
            MODULE.parse_syscall_spec(explicit_formula)[0]["number"],
            2,
        )

    def test_rejects_unknown_missing_duplicate_and_malformed_fields(self) -> None:
        bad_specs = (
            "",
            "[[other]]\n",
            row().replace('gas = "G_test + bytes"', 'unknown = "x"'),
            row("not-a-number"),
            row().replace('number = "0x01"', "number = 1"),
            row().replace(
                'number = "0x01"', 'number = "0x01"\nnumber = "0x02"'
            ),
            row().replace('args = "r10=\\\"value\\\""', r'args = "\u0041"'),
            row().replace('gas = "G_test + bytes"', 'gas = "bytes only"'),
            row().replace('gas = "G_test + bytes"\n', ""),
            row("1") + row("0x01"),
        )
        for spec in bad_specs:
            with self.subTest(spec=spec):
                with self.assertRaises(RuntimeError):
                    MODULE.parse_syscall_spec(spec)


class PublicConstantParserTests(unittest.TestCase):
    def test_rejects_omitted_and_noncanonical_public_constants(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            spec = root / "syscalls.toml"
            source = root / "syscalls.rs"
            spec.write_text(row(), encoding="utf-8")
            source.write_text(
                "pub const SYSCALL_ONE: u32 = 0x01;\n", encoding="utf-8"
            )
            self.assertEqual(
                MODULE.build_rows(source, spec),
                ["| SYSCALL_ONE | 0x1 |  |"],
            )

            source.write_text(
                " pub const SYSCALL_ONE: u32 = 0x01;\n", encoding="utf-8"
            )
            with self.assertRaises(RuntimeError):
                MODULE.build_rows(source, spec)

            source.write_text(
                "pub const SYSCALL_ONE: u32 = 0x0a;\n", encoding="utf-8"
            )
            with self.assertRaises(RuntimeError):
                MODULE.build_rows(source, spec)

            source.write_text(
                "pub const SYSCALL_OTHER: u32 = 0x02;\n", encoding="utf-8"
            )
            with self.assertRaises(RuntimeError):
                MODULE.build_rows(source, spec)


class PublicationTests(unittest.TestCase):
    def test_command_mode_is_exactly_one_known_argument(self) -> None:
        self.assertFalse(MODULE.parse_mode(("--write",)))
        self.assertTrue(MODULE.parse_mode(("--check",)))
        for arguments in ((), ("--write", "--check"), ("--unknown",)):
            with self.subTest(arguments=arguments):
                with self.assertRaises(ValueError):
                    MODULE.parse_mode(arguments)

    def test_canonical_check_is_nonmutating_and_write_is_atomic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "generated.md"
            path.write_text("stale\n", encoding="utf-8")
            before = path.read_bytes()
            rows = ["| SYSCALL_ONE | 0x1 |  |"]

            with self.assertRaises(RuntimeError):
                MODULE.sync_canonical_table(rows, check=True, path=path)
            self.assertEqual(path.read_bytes(), before)

            MODULE.sync_canonical_table(rows, check=False, path=path)
            self.assertEqual(path.read_text(encoding="utf-8"), MODULE.render(rows))
            MODULE.sync_canonical_table(rows, check=True, path=path)
            self.assertEqual(
                [entry.name for entry in root.iterdir()],
                ["generated.md"],
            )

    def test_atomic_publication_rejects_symlink_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "target.md"
            output = root / "generated.md"
            target.write_text("untouched\n", encoding="utf-8")
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError):
                self.skipTest("symlinks are unavailable on this platform")

            with self.assertRaises(RuntimeError):
                MODULE._atomic_write(output, "replacement\n")
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")
            self.assertTrue(output.is_symlink())

    def test_late_destination_validation_failure_is_nonmutating(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "first.md"
            second = root / "second.md"
            target = root / "target.md"
            first.write_text("old first\n", encoding="utf-8")
            second.write_text("old second\n", encoding="utf-8")
            target.write_text("untouched\n", encoding="utf-8")
            first_before = first.read_bytes()
            outputs = [
                MODULE._prepare_output(first, "new first\n"),
                MODULE._prepare_output(second, "new second\n"),
            ]
            second.unlink()
            try:
                second.symlink_to(target)
            except (NotImplementedError, OSError):
                self.skipTest("symlinks are unavailable on this platform")

            with self.assertRaises(RuntimeError):
                MODULE._sync_prepared_outputs(outputs, check=False)
            self.assertEqual(first.read_bytes(), first_before)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")
            self.assertFalse(
                any(entry.name.endswith(".tmp") for entry in root.iterdir())
            )
if __name__ == "__main__":
    unittest.main()
