"""Guards against reintroducing retired SCCP runtime-network surfaces."""

from __future__ import annotations

from html import unescape as html_unescape
import re
from pathlib import Path
from urllib.parse import unquote as url_unquote


REPO_ROOT = Path(__file__).resolve().parents[2]

_SC = "sc"
_ALE = "ale"
_RUNTIME = "runtime"


RETIRED_NETWORK_TOKEN_SEPARATOR = (
    r"(?:[\s._:/|+=~\-\u200b\u200c\u200d\ufeff]|\\)*"
)
DECODED_RETIRED_NETWORK_NOTE_GLUE = r"(?:\s|\\n|\\|[\"'])*"


def _retired_word(*parts: str) -> re.Pattern[str]:
    return re.compile(
        r"\b"
        + RETIRED_NETWORK_TOKEN_SEPARATOR.join(re.escape(part) for part in parts)
        + r"\b",
        re.IGNORECASE,
    )


BANNED_PATTERNS: tuple[re.Pattern[str], ...] = (
    _retired_word("sub", "strate"),
    _retired_word("sub", "strat"),
    _retired_word("pol", "kadot"),
    _retired_word("ku", "sama"),
    _retired_word(_RUNTIME, _SC, _ALE),
    _retired_word("pa", "llet"),
    _retired_word("para", "chain"),
    _retired_word("x", "cm"),
    _retired_word("sr", "25519"),
    _retired_word("sp", _RUNTIME),
    _retired_word("frame", "system"),
    _retired_word("frame", "support"),
    re.compile(chr(0x57FA) + chr(0x677F), re.IGNORECASE),
    re.compile("".join(chr(code) for code in (0x0627, 0x0644, 0x0631, 0x0643, 0x064A, 0x0632, 0x0629))),
)

_DECODED_SUBSTRATE = "".join(("Sub", "strate"))
_DECODED_POLKADOT = "".join(("Pol", "kadot"))
RETIRED_NETWORK_CONFUSABLES = str.maketrans(
    {
        "Α": "A",
        "А": "A",
        "а": "a",
        "Β": "B",
        "В": "B",
        "С": "C",
        "с": "c",
        "Е": "E",
        "е": "e",
        "І": "I",
        "і": "i",
        "Κ": "K",
        "К": "K",
        "κ": "k",
        "к": "k",
        "Μ": "M",
        "М": "M",
        "Ο": "O",
        "О": "O",
        "ο": "o",
        "о": "o",
        "Ρ": "P",
        "Р": "P",
        "ρ": "p",
        "р": "p",
        "Ѕ": "S",
        "ѕ": "s",
        "Τ": "T",
        "Т": "T",
        "τ": "t",
        "т": "t",
        "Χ": "X",
        "Х": "X",
        "χ": "x",
        "х": "x",
        "Υ": "Y",
        "У": "Y",
        "у": "y",
    }
)


def _decoded_note_pattern(*parts: str) -> str:
    return DECODED_RETIRED_NETWORK_NOTE_GLUE.join(re.escape(part) for part in parts)


SCCP_DECODED_RETIRED_NETWORK_ALLOWED_SCOPE_PATTERNS = (
    re.compile(
        _decoded_note_pattern(
            "SCCP",
            "will",
            "not",
            "support",
            _DECODED_SUBSTRATE,
            "/",
            _DECODED_POLKADOT,
            "networks",
            "for",
            "now.",
        ),
        re.IGNORECASE | re.DOTALL,
    ),
    re.compile(
        _decoded_note_pattern(
            "Do",
            "not",
            "track",
            _DECODED_SUBSTRATE,
            "/",
            _DECODED_POLKADOT,
            "relayers,",
            "route",
            "manifests,",
            "proof",
        ),
        re.IGNORECASE | re.DOTALL,
    ),
    re.compile(
        _decoded_note_pattern(
            _DECODED_SUBSTRATE,
            "/",
            _DECODED_POLKADOT,
            "networks",
            "are",
        )
        + (
            rf"(?:explicitly{DECODED_RETIRED_NETWORK_NOTE_GLUE})?"
            rf"(?:out{DECODED_RETIRED_NETWORK_NOTE_GLUE}"
            rf"of{DECODED_RETIRED_NETWORK_NOTE_GLUE}scope|"
            rf"intentionally{DECODED_RETIRED_NETWORK_NOTE_GLUE}unsupported|"
            rf"not{DECODED_RETIRED_NETWORK_NOTE_GLUE}supported)"
        ),
        re.IGNORECASE | re.DOTALL,
    ),
    re.compile(
        _decoded_note_pattern(
            "No",
            "current",
            "source",
            "proof,",
            "manifest,",
            "SDK",
            "helper,",
            "or",
            "Torii",
            "route",
            "should",
            "be",
            "treated",
            "as",
            _DECODED_SUBSTRATE,
            "/",
            f"{_DECODED_POLKADOT}-compatible",
        ),
        re.IGNORECASE | re.DOTALL,
    ),
    re.compile(
        _decoded_note_pattern(
            "imply",
            "hidden",
            _DECODED_SUBSTRATE,
            "/",
            _DECODED_POLKADOT,
            "compatibility",
        ),
        re.IGNORECASE | re.DOTALL,
    ),
    re.compile(
        _decoded_note_pattern(
            _DECODED_SUBSTRATE,
            "/",
            _DECODED_POLKADOT,
            "no-support",
            "sentence",
        ),
        re.IGNORECASE | re.DOTALL,
    ),
)

SCCP_DECODED_RETIRED_NETWORK_ALLOWED_CONTEXT = re.compile(
    "|".join(
        (
            r"will\s+not\s+support",
            r"not\s+support(?:ed)?",
            r"unsupported",
            r"out\s+of\s+(?:SCCP\s+)?scope",
            r"no-support",
            r"do\s+not\s+track",
            r"support\s+boundary",
            r"not-remaining-work",
            r"current-release\s+support\s+boundary",
            r"unsupported-scope",
            re.escape("SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE"),
            re.escape("SCCP_NOT_REMAINING_WORK_SCOPE_NOTE"),
            rf"with\s+no\s+{re.escape(_DECODED_SUBSTRATE)}"
            rf"/{re.escape(_DECODED_POLKADOT)}\s+network\s+support",
            rf"excludes\s+{re.escape(_DECODED_SUBSTRATE)}"
            rf"/{re.escape(_DECODED_POLKADOT)}\s+network\s+support",
        )
    ),
    re.IGNORECASE | re.DOTALL,
)
SCCP_DECODED_RETIRED_NETWORK_SCAN_SKIP_FILES = {
    Path("status.md"),
}

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
    Path("docs/source/bridge_proofs.ja.md"),
    Path("docs/source/bridge_proofs.ru.md"),
    Path("docs/source/bridge_proofs.ur.md"),
    Path("docs/source/engineering_backlog.md"),
    Path("docs/source"),
    Path("docs/source/crypto"),
    Path("roadmap.md"),
    Path("status.md"),
)

SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES = {
    Path("docs/source/bridge_proofs.ja.md"),
    Path("docs/source/bridge_proofs.ru.md"),
    Path("docs/source/bridge_proofs.ur.md"),
}

SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES = {
    Path("docs/source/bridge_proofs.md"),
    Path("docs/source/engineering_backlog.md"),
    Path("roadmap.md"),
    Path("status.md"),
} | SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES

SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE = re.compile(
    r"retired\s+runtime-network families\b.{0,96}\b("
    r"outside|not supported|unsupported"
    r")",
    re.IGNORECASE | re.DOTALL,
)

SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE_FILES = SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES

SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE = (
    "SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now."
)

SCCP_NOT_REMAINING_WORK_NOTE_FILES = SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES

SCCP_NOT_REMAINING_WORK_NOTE = re.compile(
    r"\bdo\s+not\s+track\b.{0,192}\bremaining"
    r"(?:\s+SCCP\s+launch)?\s+work\b.{0,96}\b(?:this|launch)\s+cycle\b",
    re.IGNORECASE | re.DOTALL,
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


def _decoded_retired_network_match_is_allowed(
    text: str, start: int, end: int
) -> bool:
    for pattern in SCCP_DECODED_RETIRED_NETWORK_ALLOWED_SCOPE_PATTERNS:
        for match in pattern.finditer(text):
            if match.start() <= start and end <= match.end():
                return True

    window = text[max(0, start - 180) : min(len(text), end + 180)]
    return SCCP_DECODED_RETIRED_NETWORK_ALLOWED_CONTEXT.search(window) is not None


def _decode_retired_network_surface_text(text: str) -> str:
    decoded = html_unescape(text)
    for _ in range(3):
        next_decoded = url_unquote(decoded)
        if next_decoded == decoded:
            break
        decoded = next_decoded
    return decoded


def _normalize_retired_network_confusables(text: str) -> str:
    return text.translate(RETIRED_NETWORK_CONFUSABLES)


def _decoded_retired_network_surface_violations(
    relative: Path, text: str
) -> list[str]:
    if relative in SCCP_DECODED_RETIRED_NETWORK_SCAN_SKIP_FILES:
        return []

    decoded = _normalize_retired_network_confusables(
        _decode_retired_network_surface_text(text)
    )
    violations: list[str] = []
    for pattern in BANNED_PATTERNS:
        match = pattern.search(decoded)
        while match is not None:
            if not _decoded_retired_network_match_is_allowed(
                decoded, match.start(), match.end()
            ):
                line = decoded.count("\n", 0, match.start()) + 1
                violations.append(f"{line}:decoded:{match.group(0)!r}")
            match = pattern.search(decoded, match.end())
    return violations


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
        (_RUNTIME, _SC, _ALE),
        ("pa", "llet"),
        ("para", "chain"),
        ("x", "cm"),
        ("sr", "25519"),
        ("sp", _RUNTIME),
        ("frame", "system"),
        ("frame", "support"),
        (chr(0x57FA), chr(0x677F)),
        tuple(chr(code) for code in (0x0627, 0x0644, 0x0631, 0x0643, 0x064A, 0x0632, 0x0629)),
    ]

    assert len(BANNED_PATTERNS) == len(examples)
    for pattern, example in zip(BANNED_PATTERNS, examples):
        assert pattern.search("".join(example))


def test_retired_network_patterns_catch_separator_obfuscation_examples() -> None:
    cases = (
        (BANNED_PATTERNS[0], ("sub", "-", "strate")),
        (BANNED_PATTERNS[0], ("sub", "_", "strate")),
        (BANNED_PATTERNS[0], ("sub", ".", "strate")),
        (BANNED_PATTERNS[0], ("sub", " ", "strate")),
        (BANNED_PATTERNS[0], ("sub", chr(0x200B), "strate")),
        (BANNED_PATTERNS[0], ("sub", "/", "strate")),
        (BANNED_PATTERNS[0], ("sub", ":", "strate")),
        (BANNED_PATTERNS[0], ("sub", "|", "strate")),
        (BANNED_PATTERNS[0], ("sub", "\\", "strate")),
        (BANNED_PATTERNS[2], ("pol", "-", "kadot")),
        (BANNED_PATTERNS[2], ("pol", "_", "kadot")),
        (BANNED_PATTERNS[2], ("pol", chr(0xFEFF), "kadot")),
        (BANNED_PATTERNS[2], ("pol", "/", "kadot")),
        (BANNED_PATTERNS[2], ("pol", "+", "kadot")),
        (BANNED_PATTERNS[2], ("pol", "=", "kadot")),
        (BANNED_PATTERNS[2], ("pol", "~", "kadot")),
        (BANNED_PATTERNS[4], (_RUNTIME, ".", _SC, ".", _ALE)),
        (BANNED_PATTERNS[4], (_RUNTIME, "::", _SC, "::", _ALE)),
        (BANNED_PATTERNS[7], ("x", "-", "cm")),
        (BANNED_PATTERNS[7], ("x", "/", "cm")),
        (BANNED_PATTERNS[9], ("sp", "_", _RUNTIME)),
        (BANNED_PATTERNS[9], ("sp", "::", _RUNTIME)),
        (BANNED_PATTERNS[10], ("frame", "-", "system")),
        (BANNED_PATTERNS[10], ("frame", "::", "system")),
        (BANNED_PATTERNS[11], ("frame", ".", "support")),
        (BANNED_PATTERNS[11], ("frame", "/", "support")),
    )

    for pattern, example in cases:
        assert pattern.search("".join(example))


def test_retired_network_patterns_catch_html_entity_obfuscation_examples() -> None:
    encoded_family = "".join(
        ("Sub", "&#115;", "trate", "/", "Pol", "&#107;", "adot")
    )
    encoded_runtime = "".join(("sp", "&#95;", _RUNTIME))
    adversarial_examples = (
        f"operator added {encoded_family} relayer support",
        f"operator added {encoded_family} route manifest",
        f"operator added {encoded_runtime} proof codec",
    )

    for index, text in enumerate(adversarial_examples):
        assert _decoded_retired_network_surface_violations(
            Path(f"adversarial-{index}.md"), text
        )

    approved_examples = (
        SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE,
        f"the exact escaped {encoded_family} no-support sentence remains pinned",
    )
    for index, text in enumerate(approved_examples):
        assert _decoded_retired_network_surface_violations(
            Path(f"approved-{index}.md"), text
        ) == []


def test_retired_network_patterns_catch_url_percent_obfuscation_examples() -> None:
    encoded_family = "".join(
        ("Sub", "%73", "trate", "%2f", "Pol", "%6b", "adot")
    )
    double_encoded_family = "".join(
        ("Sub", "%2573", "trate", "%252f", "Pol", "%256b", "adot")
    )
    encoded_runtime = "".join(("sp", "%5f", _RUNTIME))
    adversarial_examples = (
        f"operator added {encoded_family} relayer support",
        f"operator added {double_encoded_family} route manifest",
        f"operator added {encoded_runtime} proof codec",
    )

    for index, text in enumerate(adversarial_examples):
        assert _decoded_retired_network_surface_violations(
            Path(f"percent-adversarial-{index}.md"), text
        )

    approved_examples = (
        "".join(
            (
                "SCCP will not support ",
                encoded_family,
                " networks for now.",
            )
        ),
    )
    for index, text in enumerate(approved_examples):
        assert _decoded_retired_network_surface_violations(
            Path(f"percent-approved-{index}.md"), text
        ) == []


def test_retired_network_patterns_catch_unicode_confusable_obfuscation_examples() -> None:
    adversarial_examples = (
        "operator added " + "".join(("Ѕ", "ub", "ѕ", "trate")) + " relayer support",
        "operator added " + "".join(("Р", "ol", "κ", "adot")) + " route manifest",
        "operator added " + "".join(("ѕ", "р", "_", _RUNTIME)) + " proof codec",
        "operator added " + "".join(("Χ", "С", "Μ")) + " message bridge",
    )

    for index, text in enumerate(adversarial_examples):
        assert _decoded_retired_network_surface_violations(
            Path(f"confusable-adversarial-{index}.md"), text
        )


def test_retired_network_surface_scan_covers_expected_files() -> None:
    scanned = {path.relative_to(REPO_ROOT) for path in _scanned_files()}

    assert Path("docs/source/bridge_proofs.md") in scanned
    assert SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES <= scanned
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


def test_generic_no_support_note_stays_in_launch_scope_files() -> None:
    for relative in SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES:
        text = (REPO_ROOT / relative).read_text(encoding="utf-8")
        assert SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE.search(text), (
            f"missing generic unsupported scope note in {relative}"
        )


def test_specific_no_support_note_stays_in_launch_scope_files() -> None:
    for relative in SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE_FILES:
        text = (REPO_ROOT / relative).read_text(encoding="utf-8")
        assert SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE in text, (
            f"missing specific unsupported scope note in {relative}"
        )


def test_not_remaining_work_note_stays_in_launch_scope_files() -> None:
    for relative in SCCP_NOT_REMAINING_WORK_NOTE_FILES:
        text = (REPO_ROOT / relative).read_text(encoding="utf-8")
        assert SCCP_NOT_REMAINING_WORK_NOTE.search(text), (
            f"missing not-remaining-work launch-scope note in {relative}"
        )


def test_translated_no_support_scope_notes_stay_complete() -> None:
    assert SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES
    assert (
        SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES
        <= SCCP_GENERIC_UNSUPPORTED_SCOPE_NOTE_FILES
    )
    assert (
        SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES
        <= SCCP_SPECIFIC_UNSUPPORTED_SCOPE_NOTE_FILES
    )
    assert (
        SCCP_TRANSLATED_UNSUPPORTED_SCOPE_NOTE_FILES
        <= SCCP_NOT_REMAINING_WORK_NOTE_FILES
    )


def test_active_tree_excludes_retired_network_surface_tokens() -> None:
    violations: list[str] = []

    for path in _scanned_files():
        relative = path.relative_to(REPO_ROOT)
        text = path.read_text(encoding="utf-8", errors="ignore")
        for pattern in BANNED_PATTERNS:
            match = pattern.search(text)
            while match is not None:
                violations.append(f"{relative}:{match.start()}: {match.group(0)!r}")
                match = pattern.search(text, match.end())
        for violation in _decoded_retired_network_surface_violations(relative, text):
            violations.append(f"{relative}:{violation}")

    assert violations == []
