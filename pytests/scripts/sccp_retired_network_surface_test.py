"""Guards against reintroducing retired SCCP runtime-network surfaces."""

from __future__ import annotations

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

_SC = "sc"
_ALE = "ale"
_RUNTIME = "runtime"


def _literal(*parts: str) -> re.Pattern[str]:
    return re.compile(re.escape("".join(parts)), re.IGNORECASE)


def _word(*parts: str) -> re.Pattern[str]:
    return re.compile(r"\b" + re.escape("".join(parts)) + r"\b", re.IGNORECASE)


BANNED_PATTERNS: tuple[re.Pattern[str], ...] = (
    _literal("sub", "strate"),
    _literal("sub", "strat"),
    _literal("pol", "kadot"),
    _literal("ku", "sama"),
    _literal(_RUNTIME, " ", _SC, _ALE),
    _literal(_RUNTIME, "-", _SC, _ALE),
    _literal(_RUNTIME, "_", _SC, _ALE),
    _word("pa", "llet"),
    _word("para", "chain"),
    _word("x", "cm"),
    _word("sr", "25519"),
    _word("sp", "_", _RUNTIME),
    _word("frame", "_", "support"),
    re.compile(chr(0x57FA) + chr(0x677F), re.IGNORECASE),
    re.compile("".join(chr(code) for code in (0x0627, 0x0644, 0x0631, 0x0643, 0x064A, 0x0632, 0x0629))),
)

TEXT_SUFFIXES = {
    "",
    ".cfg",
    ".css",
    ".gradle",
    ".html",
    ".java",
    ".js",
    ".json",
    ".kt",
    ".lock",
    ".md",
    ".py",
    ".rs",
    ".sh",
    ".swift",
    ".tla",
    ".toml",
    ".ts",
    ".txt",
    ".xml",
    ".yaml",
    ".yml",
}

EXCLUDED_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "node_modules",
    "target",
}

EXCLUDED_PATHS = {
    Path("javascript/iroha_js/package-lock.json"),
}

EXCLUDED_PREFIXES = (
    Path("fixtures"),
    Path("tests/interop"),
)


def _is_scanned_file(path: Path) -> bool:
    if not path.is_file() or path.is_symlink():
        return False
    relative = path.relative_to(REPO_ROOT)
    if any(part in EXCLUDED_DIRS for part in relative.parts):
        return False
    if relative in EXCLUDED_PATHS:
        return False
    if any(relative == prefix or relative.is_relative_to(prefix) for prefix in EXCLUDED_PREFIXES):
        return False
    return path.suffix.lower() in TEXT_SUFFIXES


def _scanned_files() -> list[Path]:
    return sorted(path for path in REPO_ROOT.rglob("*") if _is_scanned_file(path))


def test_retired_network_patterns_catch_adversarial_examples() -> None:
    examples = [
        ("sub", "strate"),
        ("sub", "strat"),
        ("pol", "kadot"),
        ("ku", "sama"),
        (_RUNTIME, " ", _SC, _ALE),
        (_RUNTIME, "-", _SC, _ALE),
        (_RUNTIME, "_", _SC, _ALE),
        ("pa", "llet"),
        ("para", "chain"),
        ("x", "cm"),
        ("sr", "25519"),
        ("sp", "_", _RUNTIME),
        ("frame", "_", "support"),
        (chr(0x57FA), chr(0x677F)),
        tuple(chr(code) for code in (0x0627, 0x0644, 0x0631, 0x0643, 0x064A, 0x0632, 0x0629)),
    ]

    for pattern, example in zip(BANNED_PATTERNS, examples, strict=True):
        assert pattern.search("".join(example))


def test_retired_network_surface_scan_covers_expected_files() -> None:
    scanned = {path.relative_to(REPO_ROOT) for path in _scanned_files()}

    assert Path("docs/source/bridge_proofs.md") in scanned
    assert Path("roadmap.md") in scanned
    assert Path("status.md") in scanned
    assert Path("crates/iroha_sccp/src/lib.rs") in scanned
    assert Path("python/iroha_torii_client/tests/sccp_test.py") in scanned
    assert Path("javascript/iroha_js/test/sccpPackageExports.test.js") in scanned


def test_active_tree_excludes_retired_network_surface_tokens() -> None:
    violations: list[str] = []

    for path in _scanned_files():
        relative = path.relative_to(REPO_ROOT)
        text = path.read_text(encoding="utf-8", errors="ignore")
        for pattern in BANNED_PATTERNS:
            match = pattern.search(text)
            if match is not None:
                violations.append(f"{relative}:{match.start()}: {match.group(0)!r}")

    assert violations == []
