#!/usr/bin/env python3
"""Authenticate and expand reviewed Rust include closures for formal gates."""

from __future__ import annotations

import ast
import hashlib
import json
import re
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


DEFAULT_ROOT = Path(__file__).resolve().parents[2]
REVIEWED_RUST_SOURCE_HELPER_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_reviewed_rust_source.py"
)
REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_proof_ledger_source_seal_contracts.py"
)
REVIEWED_RUST_INCLUDE_MANIFEST_SHA256 = (
    "252bafdaf7fe19aff63cb7c22081f1d597c0210f768abfd4f6534b6102b16cfb"
)


def _regular_file(path: Path, label: str, errors: list[str]) -> bool:
    if not path.is_file() or path.is_symlink():
        errors.append(f"{label} must be a regular non-symlink file: {path}")
        return False
    return True


def _manifest_assignment(
    tree: ast.Module, name: str, path: Path, errors: list[str]
) -> ast.expr | None:
    """Return one exact top-level assignment from the reviewed manifest source."""

    values: list[ast.expr] = []
    for node in tree.body:
        if not isinstance(node, ast.Assign) or len(node.targets) != 1:
            continue
        target = node.targets[0]
        if isinstance(target, ast.Name) and target.id == name:
            values.append(node.value)
    if len(values) != 1:
        errors.append(
            f"{path}: reviewed Rust include manifest must define exactly one "
            f"{name} assignment; found {len(values)}"
        )
        return None
    return values[0]


def _safe_manifest_path(raw: str, *, parent: bool) -> bool:
    path = Path(raw)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and path.suffix == ".rs"
        and path.as_posix() == raw
        and (not parent or len(path.parts) > 1)
    )


def _decode_reviewed_rust_include_manifest(
    path: Path, errors: list[str]
) -> dict[str, tuple[str, ...]]:
    """Safely decode and authenticate the proof-ledger include allowlist."""

    if not _regular_file(path, "reviewed Rust include manifest source", errors):
        return {}
    try:
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
    except (OSError, UnicodeDecodeError, SyntaxError) as error:
        errors.append(f"{path}: cannot parse reviewed Rust include manifest: {error}")
        return {}

    kura_node = _manifest_assignment(
        tree, "_KURA_PRODUCTION_COMPONENT_FILES", path, errors
    )
    manifest_node = _manifest_assignment(
        tree, "_REVIEWED_RUST_INCLUDE_MANIFESTS", path, errors
    )
    if kura_node is None or manifest_node is None:
        return {}
    try:
        kura_components = ast.literal_eval(kura_node)
    except (ValueError, TypeError) as error:
        errors.append(
            f"{path}: Kura production include tuple is not a literal: {error}"
        )
        return {}
    if (
        not isinstance(kura_components, tuple)
        or not kura_components
        or not all(isinstance(component, str) for component in kura_components)
        or len(kura_components) != len(set(kura_components))
    ):
        errors.append(
            f"{path}: Kura production include tuple must contain unique paths"
        )
        return {}
    if not isinstance(manifest_node, ast.Dict):
        errors.append(f"{path}: reviewed Rust include manifest must be a dict literal")
        return {}

    manifest: dict[str, tuple[str, ...]] = {}
    for key_node, value_node in zip(manifest_node.keys, manifest_node.values):
        try:
            parent = ast.literal_eval(key_node)
        except (ValueError, TypeError) as error:
            errors.append(
                f"{path}: reviewed Rust include parent is not a literal: {error}"
            )
            continue
        if not isinstance(parent, str) or not isinstance(value_node, ast.Tuple):
            errors.append(
                f"{path}: reviewed Rust include entries must map strings to tuples"
            )
            continue
        if parent in manifest:
            errors.append(f"{path}: duplicate reviewed Rust include parent {parent!r}")
            continue
        components: list[str] = []
        malformed = False
        for element in value_node.elts:
            if isinstance(element, ast.Starred):
                if (
                    not isinstance(element.value, ast.Name)
                    or element.value.id != "_KURA_PRODUCTION_COMPONENT_FILES"
                ):
                    errors.append(
                        f"{path}: {parent!r} contains an unreviewed starred include"
                    )
                    malformed = True
                    continue
                components.extend(kura_components)
                continue
            try:
                component = ast.literal_eval(element)
            except (ValueError, TypeError) as error:
                errors.append(
                    f"{path}: {parent!r} include is not a literal: {error}"
                )
                malformed = True
                continue
            if not isinstance(component, str):
                errors.append(f"{path}: {parent!r} include path must be a string")
                malformed = True
                continue
            components.append(component)
        if malformed:
            continue
        if (
            not _safe_manifest_path(parent, parent=True)
            or not components
            or len(components) != len(set(components))
            or any(
                not _safe_manifest_path(component, parent=False)
                for component in components
            )
        ):
            errors.append(
                f"{path}: reviewed Rust include entry {parent!r} has an unsafe "
                "or noncanonical path inventory"
            )
            continue
        manifest[parent] = tuple(components)

    payload = json.dumps(
        manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii")
    digest = hashlib.sha256(payload).hexdigest()
    if digest != REVIEWED_RUST_INCLUDE_MANIFEST_SHA256:
        errors.append(
            f"{path}: reviewed Rust include manifest digest must equal "
            f"{REVIEWED_RUST_INCLUDE_MANIFEST_SHA256}; found {digest}"
        )
    return manifest


_CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS: list[str] = []
_REVIEWED_RUST_INCLUDE_MANIFESTS = _decode_reviewed_rust_include_manifest(
    DEFAULT_ROOT / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE,
    _CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS,
)


def _validate_reviewed_rust_include_manifest(
    root: Path, errors: list[str]
) -> None:
    """Require the target tree to retain the checker-pinned include allowlist."""

    errors.extend(_CANONICAL_REVIEWED_RUST_INCLUDE_MANIFEST_ERRORS)
    target_errors: list[str] = []
    observed = _decode_reviewed_rust_include_manifest(
        root / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE, target_errors
    )
    errors.extend(target_errors)
    if not target_errors and observed != _REVIEWED_RUST_INCLUDE_MANIFESTS:
        errors.append(
            f"{root / REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE}: reviewed Rust "
            "include manifest differs from the checker-pinned allowlist"
        )


def _mask_rust_comments(source: str) -> str:
    """Mask nested Rust comments without treating comment markers in literals as code."""

    output = list(source)

    def mask(start: int, end: int) -> None:
        for offset in range(start, end):
            if output[offset] != "\n":
                output[offset] = " "

    index = 0
    length = len(source)
    state = "code"
    raw_hashes = 0
    while index < length:
        char = source[index]
        pair = source[index : index + 2]
        if state == "string":
            if char == "\\":
                index += 2
            else:
                if char == '"':
                    state = "code"
                index += 1
            continue
        if state == "char":
            if char == "\\":
                index += 2
            else:
                if char == "'":
                    state = "code"
                index += 1
            continue
        if state == "raw-string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                index += len(terminator)
                state = "code"
            else:
                index += 1
            continue

        if pair == "//":
            end = source.find("\n", index + 2)
            end = length if end < 0 else end
            mask(index, end)
            index = end
            continue
        if pair == "/*":
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            mask(index, end)
            index = end
            continue
        raw_prefix = None
        for prefix in ("br", "cr", "r"):
            if source.startswith(prefix, index):
                cursor = index + len(prefix)
                while cursor < length and source[cursor] == "#":
                    cursor += 1
                if cursor < length and source[cursor] == '"':
                    raw_prefix = (cursor - index - len(prefix), cursor + 1)
                    break
        if raw_prefix is not None:
            raw_hashes, index = raw_prefix
            state = "raw-string"
            continue
        if source.startswith(('b"', 'c"'), index):
            state = "string"
            index += 2
            continue
        if char == '"':
            state = "string"
            index += 1
            continue
        if source.startswith("b'", index):
            state = "char"
            index += 2
            continue
        if char == "'" and source.find("'", index + 1, min(index + 8, length)) >= 0:
            state = "char"
            index += 1
            continue
        index += 1
    return "".join(output)


_ACTIVE_REVIEWED_RUST_SOURCE_CACHE: (
    dict[tuple[Path, str], tuple[Path, str | None]] | None
) = None


@contextmanager
def _reviewed_rust_source_cache() -> Iterator[None]:
    """Cache immutable reviewed expansions for one complete validation run."""

    global _ACTIVE_REVIEWED_RUST_SOURCE_CACHE
    if _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None:
        yield
        return
    _ACTIVE_REVIEWED_RUST_SOURCE_CACHE = {}
    try:
        yield
    finally:
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE = None


def _read_reviewed_rust_source(
    root: Path,
    relative: str,
    label: str,
    errors: list[str],
) -> tuple[Path, str | None]:
    """Read a Rust source after validating and expanding its reviewed includes."""

    cache_key = (root.resolve(), relative)
    if (
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None
        and cache_key in _ACTIVE_REVIEWED_RUST_SOURCE_CACHE
    ):
        return _ACTIVE_REVIEWED_RUST_SOURCE_CACHE[cache_key]

    path = root / relative
    if not _regular_file(path, label, errors):
        return path, None
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{path}: cannot read {label}: {error}")
        return path, None
    manifest = _REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative)
    if manifest is None:
        return path, source

    masked_source = _mask_rust_comments(source)
    include_invocations = tuple(
        re.finditer(r"(?m)^[ \t]*include\s*!", masked_source)
    )
    include_pattern = re.compile(
        r'(?m)^[ \t]*include\s*!\s*\(\s*"'
        r'(?P<relative>[^"\n]+\.rs)"\s*\)\s*;[ \t]*(?:\n|$)'
    )
    observed = tuple(
        match.group("relative") for match in include_pattern.finditer(masked_source)
    )
    if observed != manifest or len(include_invocations) != len(manifest):
        errors.append(
            f"{path}: reviewed Rust include inventory must equal {manifest!r}; "
            f"found {observed!r} across {len(include_invocations)} include "
            "invocation(s)"
        )

    component_sources: dict[str, str] = {}
    for component_relative in manifest:
        component_path = path.parent / component_relative
        if not _regular_file(
            component_path,
            f"reviewed Rust include component for {path}",
            errors,
        ):
            component_sources[component_relative] = ""
            continue
        try:
            component_sources[component_relative] = component_path.read_text(
                encoding="utf-8"
            )
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{component_path}: cannot read reviewed Rust include component "
                f"for {path}: {error}"
            )
            component_sources[component_relative] = ""

    result = path, include_pattern.sub(
        lambda match: match.group(0)
        + component_sources.get(match.group("relative"), ""),
        source,
    )
    if _ACTIVE_REVIEWED_RUST_SOURCE_CACHE is not None:
        _ACTIVE_REVIEWED_RUST_SOURCE_CACHE[cache_key] = result
    return result


def _expanded_source_manifest_paths(relative_paths: set[Path]) -> set[Path]:
    """Add every authenticated include component consumed by a source binding."""

    expanded = set(relative_paths)
    expanded.add(REVIEWED_RUST_SOURCE_HELPER_RELATIVE)
    expanded.add(REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE)
    for parent in tuple(relative_paths):
        for component in _REVIEWED_RUST_INCLUDE_MANIFESTS.get(
            parent.as_posix(), ()
        ):
            expanded.add(parent.parent / component)
    return expanded
