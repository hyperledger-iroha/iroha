"""Explicit inputs supplied by the isolated, test-only preflight driver."""
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class PreflightContext:
    """Parent-selected test inputs; no environment or ambient package lookup."""
    repository_root: Path
    dependency_root: Path
    work_root: Path


_context: PreflightContext | None = None


def install_context(value: PreflightContext) -> None:
    """Install one exact context before importing a phase's original cases."""
    global _context
    if type(value) is not PreflightContext or _context is not None:
        raise RuntimeError('preflight context must be installed exactly once')
    _context = value


def current_context() -> PreflightContext:
    """Reject importing isolated cases through ordinary pytest discovery."""
    if _context is None:
        raise RuntimeError('use the isolated fixed scaling preflight driver')
    return _context
