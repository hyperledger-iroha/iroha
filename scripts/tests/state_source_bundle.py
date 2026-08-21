"""Expand repository-local Rust `include!` fragments for source-contract tests."""

from __future__ import annotations

import re
from pathlib import Path


_DIRECT_INCLUDE_RE = re.compile(
    r'(?m)^[ \t]*include!\("(?P<path>[^"\n]+)"\);[ \t]*(?:\n|\Z)'
)


class SourceBundleError(RuntimeError):
    """Raised when a Rust include closure cannot be authenticated."""


def read_rust_source_bundle(path: Path, *, root: Path) -> str:
    """Return `path` with every direct string-literal include expanded."""

    resolved_root = root.resolve()

    def read(current: Path, stack: tuple[Path, ...]) -> str:
        resolved = current.resolve()
        try:
            resolved.relative_to(resolved_root)
        except ValueError as error:
            raise SourceBundleError(f"include escapes repository root: {resolved}") from error
        if resolved in stack:
            cycle = " -> ".join(str(item) for item in (*stack, resolved))
            raise SourceBundleError(f"recursive Rust include cycle: {cycle}")
        if not resolved.is_file():
            raise SourceBundleError(f"missing Rust include: {resolved}")

        source = resolved.read_text(encoding="utf-8")

        def expand(match: re.Match[str]) -> str:
            child = (resolved.parent / match.group("path")).resolve()
            if child.suffix != ".rs":
                raise SourceBundleError(f"direct include is not Rust source: {child}")
            return read(child, (*stack, resolved))

        return _DIRECT_INCLUDE_RE.sub(expand, source)

    return read(path, ())
