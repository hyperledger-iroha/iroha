"""Tests for the runner behind `make guards`."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("run_guards", ROOT / "scripts" / "run_guards.py")
assert SPEC is not None and SPEC.loader is not None
run_guards = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = run_guards
SPEC.loader.exec_module(run_guards)


def test_every_guard_runs_even_after_a_failure(capsys):
    outcomes = {"first": (1, "bad one\n"), "second": (0, ""), "third": (2, "bad three\n")}
    seen = []

    def run(command):
        seen.append(command[0])
        return outcomes[command[0]]

    failed = run_guards.run_all((("first",), ("second",), ("third",)), run)
    assert seen == ["first", "second", "third"]
    assert failed == ["first", "third"]
    output = capsys.readouterr().out
    assert "FAIL first" in output and "bad one" in output
    assert "ok   second" in output
    assert "FAIL third" in output and "bad three" in output


def test_long_output_keeps_the_end_and_says_what_was_dropped():
    output = "\n".join(f"line {number}" for number in range(100))
    text = run_guards.tail(output, 3)
    assert "97 earlier lines omitted" in text
    assert "line 99" in text and "line 96" not in text


def test_listed_guards_exist():
    for command in run_guards.GUARDS:
        assert (ROOT / command[1]).is_file(), command


def test_makefile_uses_the_runner():
    makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
    target = re.search(r"^guards:\n((?:\t.*\n)+)", makefile, re.MULTILINE)
    assert target is not None
    assert target.group(1) == "\t@python3 scripts/run_guards.py\n"
