"""Helpers for loading required native extension modules."""

from __future__ import annotations

import importlib
import importlib.machinery
import importlib.util
import re
import subprocess
import sys
from pathlib import Path

__all__ = ["load_crypto_extension"]

_EXTENSION_PACKAGE = "iroha_python_rs"
_EXTENSION_BASENAME = "iroha_python_rs"
_BUILD_ERROR_MESSAGE = (
    "iroha_python._crypto extension module is not built. "
    "Run `maturin develop` or install the wheel before importing `iroha_python`."
)
_PYTHON_FRAMEWORK_VERSION_RE = re.compile(
    r"Python3?\.framework/Versions/([0-9]+\.[0-9]+)/Python3?"
)


def _linked_python_framework_versions(candidate: Path) -> tuple[str, ...]:
    if sys.platform != "darwin":
        return ()
    try:
        output = subprocess.run(
            ["otool", "-L", str(candidate)],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return ()
    if output.returncode != 0:
        return ()
    return tuple(dict.fromkeys(_PYTHON_FRAMEWORK_VERSION_RE.findall(output.stdout)))


def _assert_extension_compatible(candidate: Path) -> None:
    linked_versions = _linked_python_framework_versions(candidate)
    if not linked_versions:
        return
    current_version = f"{sys.version_info.major}.{sys.version_info.minor}"
    if current_version in linked_versions:
        return
    linked = ", ".join(linked_versions)
    raise RuntimeError(
        "iroha_python._crypto extension module at "
        f"{candidate} links Python {linked}, but the current interpreter is "
        f"Python {current_version}. Rebuild it with `maturin develop --release` "
        f"using Python {current_version}."
    )


def _resolve_extension_candidate(original_exc: ModuleNotFoundError | None = None) -> Path:
    pkg_spec = importlib.util.find_spec(_EXTENSION_PACKAGE)
    if (
        pkg_spec is None
        or pkg_spec.submodule_search_locations is None
        or not pkg_spec.submodule_search_locations
    ):
        raise RuntimeError(_BUILD_ERROR_MESSAGE)

    suffixes = tuple(importlib.machinery.EXTENSION_SUFFIXES)
    search_roots = tuple(Path(entry) for entry in pkg_spec.submodule_search_locations)
    for root in search_roots:
        for suffix in suffixes:
            candidate = root / f"{_EXTENSION_BASENAME}{suffix}"
            if candidate.exists():
                return candidate

    raise RuntimeError(_BUILD_ERROR_MESSAGE)


def load_crypto_extension():
    """Return the compiled `iroha_python._crypto` module, loading it from the build dir if needed."""

    existing = sys.modules.get("iroha_python._crypto")
    if existing is not None:
        return existing

    try:
        spec = importlib.util.find_spec("iroha_python._crypto")
        if spec is not None and spec.origin:
            _assert_extension_compatible(Path(spec.origin))
        return importlib.import_module("iroha_python._crypto")
    except ModuleNotFoundError as original_exc:
        candidate = _resolve_extension_candidate(original_exc)
        if not candidate.exists():
            raise RuntimeError(
                _BUILD_ERROR_MESSAGE
            ) from original_exc
        _assert_extension_compatible(candidate)
        loader = importlib.machinery.ExtensionFileLoader(
            "iroha_python._crypto",
            str(candidate),
        )
        spec = importlib.util.spec_from_loader("iroha_python._crypto", loader)
        if spec is None:
            raise RuntimeError(
                _BUILD_ERROR_MESSAGE
            ) from original_exc
        module = importlib.util.module_from_spec(spec)
        sys.modules.setdefault("iroha_python._crypto", module)
        loader.exec_module(module)
        return module
