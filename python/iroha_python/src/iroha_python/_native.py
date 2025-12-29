"""Helpers for loading optional native extension modules."""

from __future__ import annotations

import importlib
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

__all__ = ["load_crypto_extension"]

_EXTENSION_PACKAGE = "iroha_python_rs"
_EXTENSION_BASENAME = "iroha_python_rs"
_BUILD_ERROR_MESSAGE = (
    "iroha_python._crypto extension module is not built. "
    "Run `maturin develop` or install the wheel before importing `iroha_python`."
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
        # As a fallback, look for any file that ends with a supported suffix.
        for candidate in root.glob(f"{_EXTENSION_BASENAME}*"):
            name = candidate.name
            if any(name.endswith(suffix) for suffix in suffixes):
                return candidate

    raise RuntimeError(_BUILD_ERROR_MESSAGE)


def load_crypto_extension():
    """Return the compiled `iroha_python._crypto` module, loading it from the build dir if needed."""

    try:
        return importlib.import_module("iroha_python._crypto")
    except ModuleNotFoundError as original_exc:
        candidate = _resolve_extension_candidate(original_exc)
        if not candidate.exists():
            raise RuntimeError(
                _BUILD_ERROR_MESSAGE
            ) from original_exc
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
