#!/usr/bin/env python3
"""Compile every Kotodama source snippet in repository documentation.

Prerequisites: Python 3.9+ and a freshly built canonical Rust ``koto``
driver. The normative grammar and documentation roots are read from
``docs/source/kotodama_v1_docs.json``. Every tracked or newly added Markdown
file below those roots is scanned. Explicit ``kotodama``/``ko`` fences and
``cat > *.ko <<'TAG'`` shell snippets are checked. The checker only creates
temporary ``.ko`` files and never writes generated artifacts into the
repository.

The checker intentionally fails closed: the inventory must be well formed,
every required document must contain source, fence directives and heredocs
must be understood, misspelled or omitted Kotodama fence labels are rejected,
and every extracted source must pass ``koto check``.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


DEFAULT_MANIFEST = Path("docs/source/kotodama_v1_docs.json")
MANIFEST_SCHEMA = 2
SOURCE_LANGUAGES = frozenset({"ko", "kotodama"})
SOURCE_DIRECTIVES = frozenset({"zk"})
SOURCE_LANGUAGE_ALIASES = frozenset({"koto"})
_OPENING_FENCE = re.compile(r"^( {0,3})(?P<fence>`{3,}|~{3,})(?P<info>[^\r\n]*)$")
_SOURCE_UNIT = re.compile(
    r"^\s*(?P<kind>seiyaku|誓約|module)\s+[A-Za-z_][A-Za-z0-9_]*\s*\{",
    re.MULTILINE,
)
_SOURCE_KEYWORD = re.compile(
    r"^\s*(?P<kind>seiyaku|誓約)(?=\s|$)",
    re.MULTILINE,
)
_HEREDOC_SOURCE = re.compile(
    r"^\s*cat\s+.*?\.ko\s*<<(?P<strip_tabs>-)?\s*(?P<quote>['\"]?)"
    r"(?P<tag>[A-Za-z_][A-Za-z0-9_]*)(?P=quote)\s*$"
)


class DocumentationCheckError(RuntimeError):
    """Raised when the inventory, a source fence, or compilation is invalid."""


@dataclass(frozen=True)
class DocumentSet:
    """The grammar, required documents, and scanned documentation roots."""

    grammar: Path
    documents: tuple[Path, ...]
    source_roots: tuple[Path, ...] = ()


@dataclass(frozen=True)
class SourceFence:
    """One extracted Kotodama source and its Markdown location."""

    document: Path
    opening_line: int
    source_line: int
    source: str
    zk: bool

    @property
    def location(self) -> str:
        """Return a stable, human-readable source location."""

        return f"{self.document}:{self.opening_line}"


def repository_root() -> Path:
    """Return the repository root containing this script's parent directory."""

    return Path(__file__).resolve().parents[1]


def _relative_document(root: Path, raw: object, context: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise DocumentationCheckError(f"{context} must be a non-empty path string")
    relative = Path(raw)
    if relative.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise DocumentationCheckError(
            f"{context} must be a normalized repository-relative path: {raw!r}"
        )
    if relative.suffix != ".md":
        raise DocumentationCheckError(f"{context} must name a Markdown file: {raw!r}")

    root = root.resolve()
    resolved = (root / relative).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise DocumentationCheckError(
            f"{context} escapes the repository: {raw!r}"
        ) from error
    if not resolved.is_file():
        raise DocumentationCheckError(f"{context} does not exist: {raw!r}")
    return relative


def _relative_source_root(root: Path, raw: object, context: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise DocumentationCheckError(f"{context} must be a non-empty path string")
    relative = Path(raw)
    if relative.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise DocumentationCheckError(
            f"{context} must be a normalized repository-relative path: {raw!r}"
        )

    root = root.resolve()
    resolved = (root / relative).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise DocumentationCheckError(
            f"{context} escapes the repository: {raw!r}"
        ) from error
    if not resolved.is_dir():
        raise DocumentationCheckError(f"{context} is not a directory: {raw!r}")
    return relative


def load_document_set(manifest: Path, root: Path) -> DocumentSet:
    """Read and strictly validate the canonical documentation inventory."""

    try:
        raw = json.loads(manifest.read_text(encoding="utf-8"))
    except OSError as error:
        raise DocumentationCheckError(f"failed to read {manifest}: {error}") from error
    except (UnicodeError, json.JSONDecodeError) as error:
        raise DocumentationCheckError(
            f"invalid documentation manifest {manifest}: {error}"
        ) from error

    if not isinstance(raw, dict):
        raise DocumentationCheckError(
            f"documentation manifest root must be an object, got {type(raw).__name__}"
        )

    expected_keys = {
        "schema",
        "normative_grammar",
        "source_documents",
        "source_roots",
    }
    unknown = sorted(set(raw) - expected_keys)
    missing = sorted(expected_keys - set(raw))
    if unknown:
        raise DocumentationCheckError(
            f"documentation manifest has unknown keys: {', '.join(unknown)}"
        )
    if missing:
        raise DocumentationCheckError(
            f"documentation manifest is missing keys: {', '.join(missing)}"
        )
    schema = raw["schema"]
    if (
        not isinstance(schema, int)
        or isinstance(schema, bool)
        or schema != MANIFEST_SCHEMA
    ):
        raise DocumentationCheckError(
            "documentation manifest schema must be "
            f"{MANIFEST_SCHEMA}, got {schema!r}"
        )

    grammar = _relative_document(root, raw["normative_grammar"], "normative_grammar")
    document_values = raw["source_documents"]
    if not isinstance(document_values, list) or not document_values:
        raise DocumentationCheckError("source_documents must be a non-empty array")
    documents = tuple(
        _relative_document(root, value, f"source_documents[{index}]")
        for index, value in enumerate(document_values)
    )
    if len(set(documents)) != len(documents):
        raise DocumentationCheckError("source_documents contains duplicate paths")
    if grammar not in documents:
        raise DocumentationCheckError(
            "normative_grammar must also appear in source_documents so its "
            "examples are checked"
        )

    root_values = raw["source_roots"]
    if not isinstance(root_values, list) or not root_values:
        raise DocumentationCheckError("source_roots must be a non-empty array")
    source_roots = tuple(
        _relative_source_root(root, value, f"source_roots[{index}]")
        for index, value in enumerate(root_values)
    )
    if len(set(source_roots)) != len(source_roots):
        raise DocumentationCheckError("source_roots contains duplicate paths")
    uncovered_documents = tuple(
        document
        for document in documents
        if not any(
            document == source_root or source_root in document.parents
            for source_root in source_roots
        )
    )
    if uncovered_documents:
        rendered = ", ".join(path.as_posix() for path in uncovered_documents)
        raise DocumentationCheckError(
            "every source_document must be below a scanned source_root; "
            f"uncovered: {rendered}"
        )
    return DocumentSet(
        grammar=grammar,
        documents=documents,
        source_roots=source_roots,
    )


def _closing_fence(line: str, marker: str, minimum_length: int) -> bool:
    stripped = line.lstrip(" ")
    indentation = len(line) - len(stripped)
    if indentation > 3 or not stripped.startswith(marker * minimum_length):
        return False
    marker_count = len(stripped) - len(stripped.lstrip(marker))
    return marker_count >= minimum_length and not stripped[marker_count:].strip()


def _edit_distance(left: str, right: str) -> int:
    """Return the Levenshtein distance between two short language labels."""

    previous = list(range(len(right) + 1))
    for left_index, left_character in enumerate(left, start=1):
        current = [left_index]
        for right_index, right_character in enumerate(right, start=1):
            current.append(
                min(
                    current[-1] + 1,
                    previous[right_index] + 1,
                    previous[right_index - 1]
                    + (left_character != right_character),
                )
            )
        previous = current
    return previous[-1]


def _normalise_language_claim(token: str) -> str:
    """Normalise common Markdown language-class wrappers for auditing."""

    candidate = token.strip().strip("{}[](),;:")
    candidate = candidate.lstrip(".")
    lowered = candidate.lower()
    for prefix in ("language-", "lang-"):
        if lowered.startswith(prefix):
            lowered = lowered[len(prefix) :]
            break
    return re.sub(r"[^a-z0-9]", "", lowered)


def _looks_like_source_language(token: str) -> bool:
    """Return whether an info-string field claims a Kotodama language."""

    normalised = _normalise_language_claim(token)
    if normalised in SOURCE_LANGUAGES or normalised in SOURCE_LANGUAGE_ALIASES:
        return True
    if "kotodama" in normalised:
        return True
    return len(normalised) >= 6 and _edit_distance(normalised, "kotodama") <= 2


def _source_mode(info: str, document: Path, line: int) -> bool | None:
    fields = info.strip().split()
    if not fields:
        return None
    language = fields[0]
    if language.lower() in SOURCE_LANGUAGES and language not in SOURCE_LANGUAGES:
        raise DocumentationCheckError(
            f"{document}:{line}: Kotodama fence language must be lowercase"
        )
    if language not in SOURCE_LANGUAGES:
        claimed_fields = [
            field for field in fields if _looks_like_source_language(field)
        ]
        if claimed_fields:
            rendered = ", ".join(repr(field) for field in claimed_fields)
            raise DocumentationCheckError(
                f"{document}:{line}: non-canonical Kotodama fence language "
                f"claim(s) {rendered}; use lowercase 'kotodama' or 'ko' as "
                "the first info-string field"
            )
        return None

    directives = fields[1:]
    unknown = sorted(set(directives) - SOURCE_DIRECTIVES)
    if unknown:
        raise DocumentationCheckError(
            f"{document}:{line}: unknown Kotodama fence directive(s): "
            f"{', '.join(unknown)}"
        )
    if len(directives) != len(set(directives)):
        raise DocumentationCheckError(
            f"{document}:{line}: duplicate Kotodama fence directive"
        )
    return "zk" in directives


def _without_ko_heredoc_bodies(text: str) -> str:
    """Mask recognized ``*.ko`` heredoc bodies before fence-intent checks."""

    lines = text.splitlines(keepends=True)
    masked = list(lines)
    index = 0
    while index < len(lines):
        opening = _HEREDOC_SOURCE.fullmatch(lines[index].rstrip("\r\n"))
        if opening is None:
            index += 1
            continue
        tag = opening.group("tag")
        strip_tabs = opening.group("strip_tabs") is not None
        index += 1
        while index < len(lines):
            candidate = lines[index].rstrip("\r\n")
            if strip_tabs:
                candidate = candidate.lstrip("\t")
            if candidate == tag:
                break
            masked[index] = "\n" if lines[index].endswith(("\n", "\r")) else ""
            index += 1
        if index < len(lines):
            masked[index] = "\n" if lines[index].endswith(("\n", "\r")) else ""
            index += 1
    return "".join(masked)


def _clear_source_kind(text: str, *, include_incomplete: bool) -> str | None:
    """Find source that is not already carried by a checked ``*.ko`` heredoc."""

    clear_text = _without_ko_heredoc_bodies(text)
    source_unit = _SOURCE_UNIT.search(clear_text)
    if source_unit is not None:
        return source_unit.group("kind")
    if not include_incomplete:
        return None
    source_keyword = _SOURCE_KEYWORD.search(clear_text)
    if source_keyword is not None:
        return source_keyword.group("kind")
    return None


def extract_source_fences(document: Path, text: str) -> tuple[SourceFence, ...]:
    """Extract explicit source fences and ``*.ko`` shell heredocs."""

    lines = text.splitlines(keepends=True)
    fences: list[SourceFence] = []
    index = 0
    while index < len(lines):
        raw_line = lines[index].rstrip("\r\n")
        opening = _OPENING_FENCE.fullmatch(raw_line)
        if opening is None:
            index += 1
            continue

        mode = _source_mode(opening.group("info"), document, index + 1)
        marker_text = opening.group("fence")
        marker = marker_text[0]
        fence_length = len(marker_text)
        opening_line = index + 1
        index += 1
        body_start = index
        while index < len(lines) and not _closing_fence(
            lines[index].rstrip("\r\n"), marker, fence_length
        ):
            index += 1
        terminated = index < len(lines)
        source = "".join(lines[body_start:index])

        if mode is None:
            language = opening.group("info").strip().split(maxsplit=1)
            source_kind = _clear_source_kind(
                source,
                include_incomplete=not language,
            )
            if source_kind is not None:
                label = repr(language[0]) if language else "no language label"
                termination = "unterminated " if not terminated else ""
                raise DocumentationCheckError(
                    f"{document}:{opening_line}: {termination}fenced code block "
                    f"with {label} contains apparent Kotodama source "
                    f"({source_kind}); use lowercase 'kotodama' "
                    "or 'ko'"
                )
            if not terminated:
                # An unrelated unterminated block is malformed documentation,
                # but it is outside this language-specific acceptance check.
                break
            index += 1
            continue

        if not terminated:
            raise DocumentationCheckError(
                f"{document}:{opening_line}: unterminated fenced code block"
            )
        if not source.strip():
            raise DocumentationCheckError(
                f"{document}:{opening_line}: Kotodama source fence is empty"
            )
        fences.append(
            SourceFence(
                document=document,
                opening_line=opening_line,
                source_line=opening_line + 1,
                source=source,
                zk=mode,
            )
        )
        index += 1

    for index, raw_line in enumerate(lines):
        opening = _HEREDOC_SOURCE.fullmatch(raw_line.rstrip("\r\n"))
        if opening is None:
            continue
        tag = opening.group("tag")
        strip_tabs = opening.group("strip_tabs") is not None
        body_start = index + 1
        body_end = body_start
        while body_end < len(lines):
            candidate = lines[body_end].rstrip("\r\n")
            if strip_tabs:
                candidate = candidate.lstrip("\t")
            if candidate == tag:
                break
            body_end += 1
        if body_end == len(lines):
            raise DocumentationCheckError(
                f"{document}:{index + 1}: unterminated Kotodama heredoc {tag!r}"
            )
        body_lines = lines[body_start:body_end]
        source = "".join(
            line.lstrip("\t") if strip_tabs else line for line in body_lines
        )
        if not source.strip() or not _SOURCE_UNIT.search(source):
            raise DocumentationCheckError(
                f"{document}:{index + 1}: *.ko heredoc does not contain one "
                "Kotodama source unit"
            )
        fences.append(
            SourceFence(
                document=document,
                opening_line=index + 1,
                source_line=body_start + 1,
                source=source,
                zk=False,
            )
        )
    return tuple(sorted(fences, key=lambda fence: fence.opening_line))


def tracked_markdown_documents(
    root: Path, source_roots: Sequence[Path]
) -> tuple[Path, ...]:
    """Return tracked and newly added Markdown below the configured roots."""

    if not source_roots:
        return ()
    command = [
        "git",
        "-C",
        str(root),
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        *(path.as_posix() for path in source_roots),
    ]
    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise DocumentationCheckError(
            f"failed to inventory documentation: {error}"
        ) from error
    if completed.returncode != 0:
        message = completed.stderr.decode("utf-8", errors="replace").strip()
        raise DocumentationCheckError(
            f"failed to inventory documentation: {message or 'git failed'}"
        )

    documents: list[Path] = []
    for raw in completed.stdout.split(b"\0"):
        if not raw:
            continue
        try:
            document = Path(raw.decode("utf-8"))
        except UnicodeDecodeError as error:
            raise DocumentationCheckError(
                "documentation path inventory is not valid UTF-8"
            ) from error
        if document.suffix not in {".md", ".mdx"}:
            continue
        if (root / document).is_file():
            documents.append(document)
    return tuple(sorted(set(documents)))


def collect_source_fences(
    document_set: DocumentSet, root: Path
) -> tuple[SourceFence, ...]:
    """Extract every repository source snippet and require canonical coverage."""

    fences: list[SourceFence] = []
    candidates = set(document_set.documents)
    candidates.update(tracked_markdown_documents(root, document_set.source_roots))
    covered_required: set[Path] = set()
    for document in sorted(candidates):
        path = root / document
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise DocumentationCheckError(
                f"failed to read {document}: {error}"
            ) from error
        document_fences = extract_source_fences(document, text)
        if document_fences and document in document_set.documents:
            covered_required.add(document)
        fences.extend(document_fences)

    missing_required = sorted(set(document_set.documents) - covered_required)
    if missing_required:
        rendered = ", ".join(path.as_posix() for path in missing_required)
        raise DocumentationCheckError(
            "required source_documents contain no Kotodama source: " + rendered
        )
    if not fences:
        raise DocumentationCheckError(
            "no Kotodama documentation sources found below source_roots"
        )
    return tuple(fences)


def resolve_koto(raw: str, root: Path) -> Path:
    """Resolve and validate the canonical compiler-driver executable."""

    candidate = Path(raw).expanduser()
    if candidate.is_absolute() or len(candidate.parts) > 1:
        if not candidate.is_absolute():
            candidate = root / candidate
        resolved = candidate.resolve()
    else:
        found = shutil.which(raw)
        if found is None:
            raise DocumentationCheckError(f"koto executable was not found: {raw}")
        resolved = Path(found).resolve()
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise DocumentationCheckError(f"koto is not an executable file: {resolved}")
    return resolved


def compile_source_fences(
    fences: Sequence[SourceFence], koto: Path, root: Path, timeout_seconds: float
) -> None:
    """Compile each unique source and bind the result to every matching snippet.

    Reusable ``module`` units stop after the canonical frontend check because
    they are not independently deployable. A ``seiyaku``/``誓約`` unit must
    also complete code generation through ``koto build``. This prevents a
    documentation fence from passing CI when its typed HIR is valid but its
    executable lowering, assembler, or manifest generation is not.
    """

    if timeout_seconds <= 0:
        raise DocumentationCheckError("timeout must be positive")
    groups: dict[tuple[str, bool], list[SourceFence]] = {}
    for fence in fences:
        groups.setdefault((fence.source, fence.zk), []).append(fence)

    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="kotodama-doc-fences-") as temporary:
        temporary_root = Path(temporary)
        for index, ((source, zk), occurrences) in enumerate(groups.items(), start=1):
            source_path = temporary_root / f"fence-{index:03d}.ko"
            source_path.write_text(source, encoding="utf-8")
            source_unit = _SOURCE_UNIT.search(source)
            if source_unit is not None and source_unit.group("kind") in {
                "seiyaku",
                "誓約",
            }:
                commands = [[
                    str(koto),
                    "build",
                    "--profile",
                    "docs",
                    "--target-dir",
                    str(temporary_root / "target"),
                ]]
            else:
                commands = [[str(koto), "check"]]
            source_failed = False
            for command in commands:
                if zk:
                    command.append("--zk")
                command.append(str(source_path))
                try:
                    completed = subprocess.run(
                        command,
                        cwd=root,
                        check=False,
                        capture_output=True,
                        text=True,
                        timeout=timeout_seconds,
                    )
                except (OSError, subprocess.SubprocessError) as error:
                    locations = ", ".join(item.location for item in occurrences)
                    failures.append(
                        f"{locations}: failed to execute koto: {error}"
                    )
                    source_failed = True
                    break
                if completed.returncode == 0:
                    continue
                output = "\n".join(
                    part.rstrip()
                    for part in (completed.stdout, completed.stderr)
                    if part.strip()
                )
                if not output:
                    output = "koto produced no diagnostics"
                locations = ", ".join(
                    f"{item.location} (source starts at line {item.source_line})"
                    for item in occurrences
                )
                failures.append(
                    f"{locations} failed `koto {command[1]}`:\n{output}"
                )
                source_failed = True
                break
            if source_failed:
                continue
    if failures:
        raise DocumentationCheckError("\n\n".join(failures))


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""

    parser = argparse.ArgumentParser(
        description="Compile all repository Kotodama V1 documentation sources."
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help=f"document inventory (default: {DEFAULT_MANIFEST})",
    )
    parser.add_argument(
        "--koto",
        default="target/debug/koto",
        help="canonical koto executable (default: target/debug/koto)",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=60.0,
        help="per-fence compiler timeout (default: 60)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the documentation acceptance check."""

    args = parse_args(argv)
    root = repository_root()
    manifest = args.manifest
    if not manifest.is_absolute():
        manifest = root / manifest
    try:
        document_set = load_document_set(manifest, root)
        fences = collect_source_fences(document_set, root)
        koto = resolve_koto(args.koto, root)
        compile_source_fences(fences, koto, root, args.timeout_seconds)
    except DocumentationCheckError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    documents = len({fence.document for fence in fences})
    unique_sources = len({(fence.source, fence.zk) for fence in fences})
    print(
        f"checked {len(fences)} Kotodama source snippet(s) across "
        f"{documents} document(s) ({unique_sources} unique) with {koto}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
