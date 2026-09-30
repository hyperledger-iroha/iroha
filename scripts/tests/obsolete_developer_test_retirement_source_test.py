#!/usr/bin/env python3
"""Fail closed on retired obsolete developer and shadow-test units.

The stdlib-only guard rejects resurrected retired sources and declarations,
and requires current replacement coverage and native target/module wiring.
Mutations stay in memory.
"""

from __future__ import annotations

import re
import stat
import subprocess
import unittest
import tomllib
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LOCKFILE = "Cargo.lock"
IVM_MANIFEST = "crates/ivm/Cargo.toml"
WORKFLOW = ".github/workflows/pr.yml"
GUARD_PATH = "scripts/tests/obsolete_developer_test_retirement_source_test.py"
GUARD_MODULE = "scripts.tests.obsolete_developer_test_retirement_source_test"


RETIRED_SOURCES = (
    'crates/ivm/tests/streaming_access_contract.rs',
    'crates/norito/benches/adaptive_telemetry.rs',
)


@dataclass(frozen=True)
class RetiredDeclaration:
    path: str
    removal: bytes


RETIRED_DECLARATIONS = (
    RetiredDeclaration('crates/ivm/tests/grouped/group_08.rs', b'#[path = "../streaming_access_contract.rs"]\nmod streaming_access_contract;\n'),
    RetiredDeclaration('crates/norito/Cargo.toml', b'[[bench]]\nname = "adaptive_telemetry"\nharness = false\n\n'),
)

REPLACEMENT_PATHS = (
    'crates/iroha_core/src/streaming.rs',
    'crates/norito/examples/telemetry_dump.rs',
    'crates/norito/tests/adaptive_telemetry.rs',
    'crates/norito/tests/adaptive_more_shapes.rs',
    'crates/norito/tests/adaptive_combo.rs',
    'crates/norito/tests/adaptive_opt_rows.rs',
    'crates/norito/tests/adaptive_enum_rows.rs',
    'crates/norito/tests/grouped/group_01.rs',
    'crates/norito/README.md',
)

REPLACEMENT_MARKERS = {
    "crates/iroha_core/src/streaming.rs": (
        ("fn process_streaming_event_registers_ticket_from_ready_event()", 1),
        ("fn process_streaming_event_revokes_ticket()", 1),
        ("fn ticket_revoked_removes_registered_state()", 1),
        ("fn duplicate_ticket_nullifier_is_rejected()", 1),
        ("fn ticket_envelope_commitment_mismatch_is_rejected()", 1),
    ),
    "crates/norito/examples/telemetry_dump.rs": (
        ("fn main()", 1),
        ("norito::columnar::encode_rows_u64_str_bool_adaptive(&rows)", 1),
        ("norito::telemetry::snapshot_json_string()", 1),
    ),
    "crates/norito/tests/grouped/group_01.rs": (
        ('#[path = "../adaptive_telemetry.rs"]', 1),
        ("mod adaptive_telemetry;", 1),
    ),
    "crates/norito/README.md": (
        ("cargo run -p norito --example telemetry_dump", 2),
    ),
}

CONSUMER_PATTERNS = (
    "streaming_access_contract",
    "--bench adaptive_telemetry",
    "crates/norito/benches/adaptive_telemetry.rs",
)
ALLOWED_CONSUMER_HITS: tuple[tuple[str, str], ...] = ()

TABLE_RE = re.compile(
    r"^\[\[(?P<kind>[^\]]+)\]\]\n(?P<body>.*?)(?=^\[\[|^\[[^[]|\Z)",
    re.MULTILINE | re.DOTALL,
)
FIELD_RE = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"$', re.MULTILINE)
FEATURE_RE = re.compile(r'^required-features\s*=\s*\[([^]]*)\]$', re.MULTILINE)
GROUP_MODULE_RE = re.compile(
    r'^#\[path = "([^"]+)"\]\nmod ([A-Za-z_][A-Za-z0-9_]*);$',
    re.MULTILINE,
)


class GuardError(AssertionError):
    """Raised when the obsolete-unit retirement contract changes."""


@dataclass(frozen=True)
class Snapshot:
    files: dict[str, bytes | None]
    consumer_hits: tuple[tuple[str, str], ...]
    implicit_benches: tuple[str, ...]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)










def _git(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=ROOT,
            check=check,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GuardError(f"git {' '.join(arguments)} failed: {error}") from error




def _regular_bytes(relative: str) -> bytes:
    path = ROOT / relative
    _require(not path.is_symlink(), f"symlink is not allowed: {relative}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise GuardError(f"cannot stat {relative}: {error}") from error
    _require(stat.S_ISREG(mode), f"not a regular file: {relative}")
    try:
        path.resolve(strict=True).relative_to(ROOT.resolve(strict=True))
    except ValueError as error:
        raise GuardError(f"path escapes repository: {relative}") from error
    return path.read_bytes()










def _tables(manifest: str, kind: str) -> tuple[tuple[str, str | None, tuple[str, ...]], ...]:
    rows = []
    for table in TABLE_RE.finditer(manifest):
        if table.group("kind") != kind:
            continue
        body = table.group("body")
        fields = dict(FIELD_RE.findall(body))
        feature_match = FEATURE_RE.search(body)
        features = (
            tuple(re.findall(r'"([^"]+)"', feature_match.group(1))) if feature_match else ()
        )
        rows.append((fields["name"], fields.get("path"), features))
    return tuple(rows)


def _workflow_modules(workflow: str) -> tuple[str, ...]:
    blocks = re.findall(
        r"python3 -m unittest \\\n(?P<body>(?:[ \t]+scripts\.tests\.[A-Za-z0-9_]+(?: \\\n|\n))+)",
        workflow,
    )
    matches = [
        tuple(re.findall(r"scripts\.tests\.[A-Za-z0-9_]+", block))
        for block in blocks
        if GUARD_MODULE in block
    ]
    _require(len(matches) == 1, "retirement guard unittest block changed")
    return matches[0]


def _consumer_inventory() -> tuple[tuple[str, str], ...]:
    arguments = ["grep", "--no-index", "--exclude-standard", "-n", "-I", "-F"]
    for pattern in CONSUMER_PATTERNS:
        arguments.extend(("-e", pattern))
    arguments.extend(("--", "."))
    result = _git(*arguments, check=False)
    _require(result.returncode in (0, 1), "active-consumer scan failed")
    rows = []
    for raw_line in result.stdout.decode("utf-8").splitlines():
        path, _line, text = raw_line.split(":", 2)
        path = path.removeprefix("./")
        if path in {GUARD_PATH, "status.md"}:
            continue
        rows.append((path, text))
    return tuple(sorted(rows))


def _implicit_norito_benches() -> tuple[str, ...]:
    root = ROOT / "crates/norito/benches"
    _require(root.is_dir() and not root.is_symlink(), "Norito bench root changed")
    names = []
    for entry in root.iterdir():
        _require(not entry.is_symlink(), f"Norito bench symlink is not allowed: {entry.name}")
        if entry.is_file() and entry.suffix == ".rs":
            names.append(entry.stem)
        elif entry.is_dir():
            main = entry / "main.rs"
            _require(not main.is_symlink(), f"Norito nested bench symlink: {entry.name}")
            if main.is_file():
                names.append(entry.name)
    return tuple(sorted(names))


def _snapshot() -> Snapshot:
    paths = {
        LOCKFILE,
        IVM_MANIFEST,
        WORKFLOW,
        *(pin.path for pin in RETIRED_DECLARATIONS),
        *REPLACEMENT_PATHS,
    }
    files: dict[str, bytes | None] = {path: _regular_bytes(path) for path in paths}
    for path in RETIRED_SOURCES:
        source_path = ROOT / path
        files[path] = _regular_bytes(path) if source_path.exists() or source_path.is_symlink() else None
    return Snapshot(files, _consumer_inventory(), _implicit_norito_benches())


def _validate(snapshot: Snapshot) -> None:
    for path in RETIRED_SOURCES:
        _require(snapshot.files[path] is None, f"retired source resurrected: {path}")
    for pin in RETIRED_DECLARATIONS:
        data = snapshot.files[pin.path]
        _require(data is not None, f"declaration file missing: {pin.path}")
        _require(pin.removal not in data, f"retired declaration postimage resurrected: {pin.path}")

    norito = snapshot.files["crates/norito/Cargo.toml"]
    ivm = snapshot.files[IVM_MANIFEST]
    group = snapshot.files["crates/ivm/tests/grouped/group_08.rs"]
    assert norito is not None and ivm is not None and group is not None
    _require(norito.decode().count("autotests = false") == 1, "Norito autotests changed")
    benches = _tables(norito.decode(), "bench")
    _require(all(path is None and not features for _name, path, features in benches), "bench shape")
    _require(norito.decode().count("harness = false") == len(benches), "bench harness ledger")
    _require(all(name != "adaptive_telemetry" for name, _, _ in benches), "retired Norito bench declaration")
    _require("adaptive_telemetry" not in snapshot.implicit_benches,
             "Norito implicit bench resurrected")
    _require(ivm.decode().count("autobins = false") == 1, "IVM autobins changed")
    _require(ivm.decode().count("autotests = false") == 1, "IVM autotests changed")
    _require(all(name != "streaming_access_contract" for name, _, _ in _tables(ivm.decode(), "test")),
             "retired streaming test target resurrected")
    modules = tuple(GROUP_MODULE_RE.findall(group.decode()))
    _require(all(name != "streaming_access_contract" and "streaming_access_contract" not in path
                 for path, name in modules), "retired streaming module resurrected")

    for path in ("crates/norito/Cargo.toml", IVM_MANIFEST, LOCKFILE):
        data = snapshot.files[path]
        _require(data is not None, f"current manifest missing: {path}")
        try:
            tomllib.loads(data.decode())
        except (ValueError, UnicodeError) as error:
            raise GuardError(f"current manifest is invalid: {path}") from error
    for path in REPLACEMENT_PATHS:
        _require(snapshot.files[path] is not None, f"replacement missing: {path}")
    for path, markers in REPLACEMENT_MARKERS.items():
        source = snapshot.files[path]
        assert source is not None
        text = source.decode()
        for marker, count in markers:
            _require(text.count(marker) == count, f"replacement marker changed: {path}: {marker}")

    workflow = snapshot.files[WORKFLOW]
    assert workflow is not None
    modules = _workflow_modules(workflow.decode())
    _require(len(modules) == len(set(modules)), "workflow unittest duplicates a guard")
    _require(modules == tuple(sorted(modules)), "workflow unittest inventory is not alphabetized")
    _require(
        snapshot.consumer_hits == ALLOWED_CONSUMER_HITS,
        f"active retired-surface consumer inventory changed: {snapshot.consumer_hits}",
    )


def _mutate(snapshot: Snapshot, path: str, data: bytes | None) -> Snapshot:
    files = dict(snapshot.files)
    files[path] = data
    return Snapshot(files, snapshot.consumer_hits, snapshot.implicit_benches)


class ObsoleteDeveloperTestRetirementSourceTest(unittest.TestCase):
    """Validate current retirement invariants and fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.snapshot = _snapshot()

    def test_retirement_contract(self) -> None:
        _validate(self.snapshot)

    def test_mutation_each_deleted_source_resurrection_fails(self) -> None:
        for path in RETIRED_SOURCES:
            with self.subTest(path=path):
                with self.assertRaisesRegex(GuardError, "retired source resurrected"):
                    _validate(
                        _mutate(self.snapshot, path, b"fn resurrected() {}\n"),
                    )

    def test_mutation_each_declaration_resurrection_fails(self) -> None:
        for pin in RETIRED_DECLARATIONS:
            with self.subTest(path=pin.path):
                data = self.snapshot.files[pin.path]
                assert data is not None
                with self.assertRaisesRegex(GuardError, "declaration postimage"):
                    _validate(_mutate(self.snapshot, pin.path, data + pin.removal))

    def test_mutation_replacement_source_fails(self) -> None:
        path = "crates/iroha_core/src/streaming.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = source.replace(
            b"fn process_streaming_event_registers_ticket_from_ready_event(",
            b"fn weakened_streaming_event_registers_ticket_from_ready_event(",
            1,
        )
        with self.assertRaisesRegex(GuardError, "replacement"):
            _validate(_mutate(self.snapshot, path, mutated))

    def test_legitimate_manifest_evolution_preserves_retirement(self) -> None:
        lock = self.snapshot.files[LOCKFILE]
        assert lock is not None
        _validate(_mutate(self.snapshot, LOCKFILE, lock + b"# current first-release graph\n"))

    def test_current_manifest_corruption_fails(self) -> None:
        lock = self.snapshot.files[LOCKFILE]
        assert lock is not None
        with self.assertRaisesRegex(GuardError, "current manifest is invalid"):
            _validate(_mutate(self.snapshot, LOCKFILE, lock + b"[unterminated\n"))

    def test_mutation_active_consumer_fails(self) -> None:
        mutated = Snapshot(
            dict(self.snapshot.files),
            self.snapshot.consumer_hits + (("README.md", "run streaming_access_contract"),),
            self.snapshot.implicit_benches,
        )
        with self.assertRaisesRegex(GuardError, "active retired-surface consumer"):
            _validate(mutated)

    def test_mutation_nested_bench_resurrection_fails(self) -> None:
        mutated = Snapshot(
            dict(self.snapshot.files),
            self.snapshot.consumer_hits,
            tuple(sorted((*self.snapshot.implicit_benches, "adaptive_telemetry"))),
        )
        with self.assertRaisesRegex(GuardError, "Norito implicit bench"):
            _validate(mutated)

    def test_mutation_workflow_hook_removal_fails(self) -> None:
        workflow = self.snapshot.files[WORKFLOW]
        assert workflow is not None
        line = f"            {GUARD_MODULE} \\\n".encode()
        with self.assertRaisesRegex(GuardError, "retirement guard unittest block"):
            _validate(_mutate(self.snapshot, WORKFLOW, workflow.replace(line, b"", 1)))

    def test_mutation_workflow_order_fails(self) -> None:
        workflow = self.snapshot.files[WORKFLOW]
        assert workflow is not None
        first = f"            {GUARD_MODULE} \\\n".encode()
        second = b"            scripts.tests.shared_proc_macro_emitter_source_test\n"
        swapped = second.rstrip(b"\n") + b" \\\n" + first.rstrip(b" \\\n") + b"\n"
        with self.assertRaisesRegex(GuardError, "workflow unittest"):
            _validate(
                _mutate(self.snapshot, WORKFLOW, workflow.replace(first + second, swapped, 1)),
            )


if __name__ == "__main__":
    unittest.main()
