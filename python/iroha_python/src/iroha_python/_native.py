"""Helpers for loading required native extension modules."""

from __future__ import annotations

import importlib
import importlib.machinery
import importlib.util
import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

__all__ = ["load_crypto_extension"]

_EXTENSION_PACKAGE = "iroha_python_rs"
_EXTENSION_BASENAME = "iroha_python_rs"
_BUILD_ERROR_MESSAGE = (
    "iroha_python._crypto extension module is not built. "
    "Run `maturin develop --release --locked` or install the wheel before "
    "importing `iroha_python`."
)
_PYTHON_FRAMEWORK_DEPENDENCY_RE = re.compile(
    r"(?:^|/)Python3?\.framework/Versions/(?P<version>[0-9]+\.[0-9]+)/Python3?$"
)
_LIBPYTHON_DEPENDENCY_RE = re.compile(
    r"(?:^|/)libpython(?P<version>[0-9]+\.[0-9]+)[A-Za-z]*"
    r"(?:\.dylib|\.so(?:\.[0-9]+)*)$"
)
_PYTHON_RUNTIME_DEPENDENCY_MARKER_RE = re.compile(
    r"(?:^|/)(?:Python3?\.framework/|libpython)"
)


class _LinkedPythonRuntimeDependency(NamedTuple):
    kind: str
    path: str
    version: str | None


def _parse_otool_python_dependencies(output: str) -> tuple[_LinkedPythonRuntimeDependency, ...]:
    dependencies: list[_LinkedPythonRuntimeDependency] = []
    for line in output.splitlines():
        value = line.strip()
        if not value or value.endswith(":"):
            continue
        dependency_path = value.split(" (", maxsplit=1)[0]
        framework = _PYTHON_FRAMEWORK_DEPENDENCY_RE.search(dependency_path)
        if framework is not None:
            dependencies.append(
                _LinkedPythonRuntimeDependency(
                    "framework",
                    dependency_path,
                    framework.group("version"),
                )
            )
            continue
        libpython = _LIBPYTHON_DEPENDENCY_RE.search(dependency_path)
        if libpython is not None:
            dependencies.append(
                _LinkedPythonRuntimeDependency(
                    "libpython",
                    dependency_path,
                    libpython.group("version"),
                )
            )
            continue
        if _PYTHON_RUNTIME_DEPENDENCY_MARKER_RE.search(dependency_path):
            dependencies.append(
                _LinkedPythonRuntimeDependency("malformed", dependency_path, None)
            )
    return tuple(dependencies)


def _linked_python_runtime_dependencies(
    candidate: Path,
) -> tuple[_LinkedPythonRuntimeDependency, ...]:
    if sys.platform != "darwin":
        return ()
    try:
        output = subprocess.run(
            ["/usr/bin/otool", "-L", str(candidate)],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError(
            f"could not inspect Python linkage for extension module at {candidate}"
        ) from error
    if output.returncode != 0:
        raise RuntimeError(
            f"could not inspect Python linkage for extension module at {candidate}: "
            f"otool exited with status {output.returncode}"
        )
    return _parse_otool_python_dependencies(output.stdout)


def _assert_extension_compatible(candidate: Path) -> None:
    dependencies = _linked_python_runtime_dependencies(candidate)
    if not dependencies:
        return
    if len(dependencies) != 1:
        linked = ", ".join(dependency.path for dependency in dependencies)
        raise RuntimeError(
            "iroha_python._crypto extension module at "
            f"{candidate} links multiple Python runtimes ({linked}); rebuild it "
            "with `maturin develop --release --locked`."
        )
    dependency = dependencies[0]
    if dependency.kind == "malformed":
        raise RuntimeError(
            "iroha_python._crypto extension module at "
            f"{candidate} has an unrecognized Python runtime dependency "
            f"({dependency.path}); rebuild it with `maturin develop --release --locked`."
        )
    if dependency.kind == "libpython":
        raise RuntimeError(
            "iroha_python._crypto extension module at "
            f"{candidate} links directly to an alternate Python runtime "
            f"({dependency.path}); rebuild it with `maturin develop --release --locked` "
            "using extension-module dynamic lookup."
        )
    current_version = f"{sys.version_info.major}.{sys.version_info.minor}"
    if dependency.version == current_version:
        return
    raise RuntimeError(
        "iroha_python._crypto extension module at "
        f"{candidate} links Python {dependency.version}, but the current interpreter is "
        f"Python {current_version}. Rebuild it with `maturin develop --release --locked` "
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
