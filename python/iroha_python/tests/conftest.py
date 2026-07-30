from __future__ import annotations

import importlib.machinery
import importlib.util
import os
import site
import sys
import sysconfig
from pathlib import Path


def _add_path(path: Path) -> None:
    location = str(path)
    if location not in sys.path:
        sys.path.insert(0, location)


_ROOT = Path(__file__).resolve().parents[2]
_add_path(_ROOT)
_add_path(_ROOT / "norito_py" / "src")
_add_path(_ROOT / "iroha_torii_client")

_INSTALLED_PACKAGE_MODE = os.environ.get("IROHA_PYTHON_TEST_INSTALLED_PACKAGE")
if _INSTALLED_PACKAGE_MODE not in {None, "1"}:
    raise RuntimeError("IROHA_PYTHON_TEST_INSTALLED_PACKAGE must be unset or 1")

if _INSTALLED_PACKAGE_MODE == "1":
    for module_name in ("iroha_python", "iroha_python._crypto"):
        if module_name in sys.modules:
            raise RuntimeError(
                f"installed-package tests reject pre-seeded module {module_name}"
            )

    environment_root = Path(sys.prefix).resolve(strict=True)
    site_package_roots = {
        Path(path).resolve(strict=True)
        for path in (
            *site.getsitepackages(),
            sysconfig.get_paths()["purelib"],
            sysconfig.get_paths()["platlib"],
        )
    }
    site_package_roots = {
        path
        for path in site_package_roots
        if path.is_relative_to(environment_root)
    }
    if not site_package_roots:
        raise RuntimeError("installed-package tests require private venv site-packages")

    def _trusted_origin(spec: object, label: str, loader_type: type) -> Path:
        origin = getattr(spec, "origin", None)
        loader = getattr(spec, "loader", None)
        if not isinstance(origin, str) or type(loader) is not loader_type:
            raise RuntimeError(f"{label} must have a trusted filesystem import spec")
        path = Path(origin)
        if not path.is_absolute() or path.is_symlink():
            raise RuntimeError(f"{label} import origin must be absolute and non-symlinked")
        canonical = path.resolve(strict=True)
        if canonical != path or not any(
            canonical.is_relative_to(site_root)
            for site_root in site_package_roots
        ):
            raise RuntimeError(
                f"{label} must resolve from private venv site-packages, got {canonical}"
            )
        return canonical

    package_spec = importlib.machinery.PathFinder.find_spec("iroha_python")
    if (
        package_spec is None
        or package_spec.loader_state is not None
        or package_spec.submodule_search_locations is None
    ):
        raise RuntimeError("iroha_python must resolve as one installed regular package")
    package_origin = _trusted_origin(
        package_spec, "iroha_python", importlib.machinery.SourceFileLoader
    )
    package_roots = {
        Path(path).resolve(strict=True)
        for path in package_spec.submodule_search_locations
    }
    if package_roots != {package_origin.parent}:
        raise RuntimeError("iroha_python package search path must match its trusted origin")
    if (
        package_spec.loader.name != "iroha_python"
        or Path(package_spec.loader.path) != package_origin
    ):
        raise RuntimeError("iroha_python source loader must match its trusted origin")

    native_spec = importlib.machinery.PathFinder.find_spec(
        "iroha_python._crypto", [str(package_origin.parent)]
    )
    if native_spec is None or native_spec.loader_state is not None:
        raise RuntimeError("iroha_python._crypto must have an unmodified extension spec")
    native_origin = _trusted_origin(
        native_spec,
        "iroha_python._crypto",
        importlib.machinery.ExtensionFileLoader,
    )
    if not any(
        native_origin.name == f"_crypto{suffix}"
        for suffix in importlib.machinery.EXTENSION_SUFFIXES
    ):
        raise RuntimeError("iroha_python._crypto origin has the wrong platform suffix")
    if (
        native_spec.loader.name != "iroha_python._crypto"
        or Path(native_spec.loader.path) != native_origin
    ):
        raise RuntimeError("iroha_python._crypto loader must match its trusted origin")

    package = importlib.util.module_from_spec(package_spec)
    sys.modules["iroha_python"] = package
    native = importlib.util.module_from_spec(native_spec)
    sys.modules["iroha_python._crypto"] = native
    native_spec.loader.exec_module(native)
    package_spec.loader.exec_module(package)

    loaded_package_spec = package.__spec__
    if (
        sys.modules.get("iroha_python") is not package
        or loaded_package_spec is None
        or loaded_package_spec.loader_state is not None
        or package.__loader__ is not loaded_package_spec.loader
        or not isinstance(getattr(package, "__file__", None), str)
        or Path(package.__file__) != package_origin
        or _trusted_origin(
            loaded_package_spec,
            "loaded iroha_python",
            importlib.machinery.SourceFileLoader,
        )
        != package_origin
    ):
        raise RuntimeError("loaded iroha_python spec changed from its trusted origin")

    loaded_native_spec = native.__spec__
    if (
        sys.modules.get("iroha_python._crypto") is not native
        or loaded_native_spec is None
        or loaded_native_spec.loader_state is not None
        or native.__loader__ is not loaded_native_spec.loader
        or not isinstance(getattr(native, "__file__", None), str)
        or Path(native.__file__) != native_origin
        or _trusted_origin(
            loaded_native_spec,
            "loaded iroha_python._crypto",
            importlib.machinery.ExtensionFileLoader,
        )
        != native_origin
    ):
        raise RuntimeError(
            "loaded iroha_python._crypto spec changed from its trusted extension origin"
        )
else:
    _add_path(_ROOT / "iroha_python" / "src")
