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
    _word("frame", "_", "system"),
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

SCAN_ROOTS = (
    Path("crates/iroha_sccp/src"),
    Path("crates/iroha_torii/src"),
    Path("crates/iroha_schema_derive/src"),
    Path("crates/iroha_schema_gen/src"),
    Path("python/iroha_torii_client"),
    Path("javascript/iroha_js/src"),
    Path("javascript/iroha_js/dist"),
    Path("javascript/iroha_js/test"),
    Path("IrohaSwift/Sources"),
    Path("IrohaSwift/Tests"),
    Path("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sccp"),
    Path("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sccp"),
    Path("java/iroha_android/src/main/java/org/hyperledger/iroha/android/sccp"),
    Path("java/iroha_android/src/test/java/org/hyperledger/iroha/android/sccp"),
    Path("scripts"),
    Path("pytests/scripts"),
    Path("docs/source/bridge_proofs.md"),
    Path("docs/source/engineering_backlog.md"),
    Path("docs/source"),
    Path("docs/source/crypto"),
    Path("roadmap.md"),
    Path("status.md"),
)

SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES = {
    Path("docs/source/bridge_proofs.md"),
    Path("docs/source/engineering_backlog.md"),
    Path("roadmap.md"),
    Path("status.md"),
}

SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE = re.compile(
    r"retired\s+runtime-network families\b.{0,96}\b("
    r"outside|not supported|unsupported"
    r")",
    re.IGNORECASE | re.DOTALL,
)

SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE = re.compile(
    re.escape("".join(("Sub", "strate")))
    + r"\s*/\s*"
    + re.escape("".join(("Pol", "kadot")))
    + r"(?:-family)?\s+networks\b.{0,160}\b("
    + r"outside|not supported|unsupported"
    + r")",
    re.IGNORECASE | re.DOTALL,
)


def _is_specific_no_support_scope_note_match(
    relative: Path, text: str, match: re.Match[str]
) -> bool:
    if relative not in SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES:
        return False

    return any(
        note_match.start() <= match.start() and match.end() <= note_match.end()
        for note_match in SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE.finditer(text)
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
    files: set[Path] = set()
    for root in SCAN_ROOTS:
        path = REPO_ROOT / root
        if path.is_file():
            if _is_scanned_file(path):
                files.add(path)
            continue
        if root == Path("scripts") or root == Path("pytests/scripts"):
            files.update(candidate for candidate in path.glob("sccp*.py") if _is_scanned_file(candidate))
            continue
        if root == Path("docs/source"):
            files.update(candidate for candidate in path.glob("new_pipeline*.md") if _is_scanned_file(candidate))
            continue
        if root == Path("docs/source/crypto"):
            files.update(
                candidate
                for candidate in path.glob("sm_audit_*.md")
                if _is_scanned_file(candidate)
            )
            continue
        files.update(candidate for candidate in path.rglob("*") if _is_scanned_file(candidate))
    return sorted(files)


def test_retired_network_surface_scan_roots_exist_and_are_nonempty() -> None:
    for root in SCAN_ROOTS:
        path = REPO_ROOT / root
        assert path.exists(), f"retired-network scan root is missing: {root}"

    scanned = {path.relative_to(REPO_ROOT) for path in _scanned_files()}
    for root in SCAN_ROOTS:
        path = REPO_ROOT / root
        if path.is_file():
            assert root in scanned
        else:
            assert any(
                scanned_path == root or scanned_path.is_relative_to(root)
                for scanned_path in scanned
            ), f"retired-network scan root has no scanned files: {root}"


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
        ("frame", "_", "system"),
        ("frame", "_", "support"),
        (chr(0x57FA), chr(0x677F)),
        tuple(chr(code) for code in (0x0627, 0x0644, 0x0631, 0x0643, 0x064A, 0x0632, 0x0629)),
    ]

    assert len(BANNED_PATTERNS) == len(examples)
    for pattern, example in zip(BANNED_PATTERNS, examples):
        assert pattern.search("".join(example))


def test_retired_network_surface_scan_covers_expected_files() -> None:
    scanned = {path.relative_to(REPO_ROOT) for path in _scanned_files()}

    assert Path("docs/source/bridge_proofs.md") in scanned
    assert Path("docs/source/engineering_backlog.md") in scanned
    assert Path("docs/source/new_pipeline.md") in scanned
    assert Path("roadmap.md") in scanned
    assert Path("status.md") in scanned
    assert Path("crates/iroha_sccp/src/lib.rs") in scanned
    assert Path("python/iroha_torii_client/tests/sccp_test.py") in scanned
    assert Path("javascript/iroha_js/test/sccpPackageExports.test.js") in scanned


def test_retired_network_surface_scan_covers_pipeline_translations() -> None:
    pipeline_docs = {
        path.relative_to(REPO_ROOT)
        for path in (REPO_ROOT / "docs/source").glob("new_pipeline*.md")
        if _is_scanned_file(path)
    }
    scanned = {path.relative_to(REPO_ROOT) for path in _scanned_files()}

    assert pipeline_docs
    assert pipeline_docs <= scanned


def test_retired_network_surface_scan_rejects_family_specific_notes() -> None:
    text = "Current scope. " + "".join(
        (
            "Sub",
            "strate",
            "/Pol",
            "kadot",
            " networks are explicitly outside SCCP launch support for now.",
        )
    )
    for pattern in BANNED_PATTERNS[:3]:
        match = pattern.search(text)
        assert match is not None
        assert _is_specific_no_support_scope_note_match(
            Path("docs/source/bridge_proofs.md"), text, match
        )
        assert not _is_specific_no_support_scope_note_match(
            Path("scripts/sccp_release_bundle.py"), text, match
        )

    unsupported_text = "".join(("Sub", "strate", " lane support is active"))
    match = BANNED_PATTERNS[0].search(unsupported_text)
    assert match is not None
    assert not _is_specific_no_support_scope_note_match(
        Path("docs/source/bridge_proofs.md"), unsupported_text, match
    )


def test_generic_no_support_note_stays_in_launch_scope_files() -> None:
    for relative in SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES:
        text = (REPO_ROOT / relative).read_text(encoding="utf-8")
        assert SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE.search(text), (
            f"missing generic unsupported scope note in {relative}"
        )
        assert SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE.search(text), (
            f"missing specific unsupported scope note in {relative}"
        )

    unsupported_text = "".join(("Sub", "strate", " lane support is active"))
    match = BANNED_PATTERNS[0].search(unsupported_text)
    assert match is not None


def test_active_tree_excludes_retired_network_surface_tokens() -> None:
    violations: list[str] = []

    for path in _scanned_files():
        relative = path.relative_to(REPO_ROOT)
        text = path.read_text(encoding="utf-8", errors="ignore")
        for pattern in BANNED_PATTERNS:
            match = pattern.search(text)
            while match is not None:
                if not _is_specific_no_support_scope_note_match(relative, text, match):
                    violations.append(f"{relative}:{match.start()}: {match.group(0)!r}")
                match = pattern.search(text, match.end())

    assert violations == []
