"""Shared path-safety helpers for repository scripts."""

from __future__ import annotations

import stat
from pathlib import Path


def _is_allowed_absolute_root_alias(path: Path, component: Path) -> bool:
    return path.is_absolute() and component.parent == Path(path.anchor)


def first_symlinked_existing_path_component(path: Path) -> Path | None:
    """Return the first existing symlink component, excluding root aliases."""

    current = Path(path.anchor) if path.is_absolute() else Path(".")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    for part in parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            return None
        if stat.S_ISLNK(mode) and not _is_allowed_absolute_root_alias(path, current):
            return current
    return None
