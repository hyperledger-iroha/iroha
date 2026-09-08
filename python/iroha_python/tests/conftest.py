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
_add_path(_ROOT / "iroha_python" / "tests")

_INSTALLED_PACKAGE_MODE = os.environ.get("IROHA_PYTHON_TEST_INSTALLED_PACKAGE")
if _INSTALLED_PACKAGE_MODE not in {None, "1"}:
    raise RuntimeError("IROHA_PYTHON_TEST_INSTALLED_PACKAGE must be unset or 1")

if _INSTALLED_PACKAGE_MODE == "1":
    for module_name in ("iroha_python", "iroha_native", "iroha_native._crypto"):
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

    def _package_spec(name: str):
        spec = importlib.machinery.PathFinder.find_spec(name)
        if spec is None or spec.loader_state is not None or spec.submodule_search_locations is None:
            raise RuntimeError(f"{name} must resolve as one installed regular package")
        origin = _trusted_origin(spec, name, importlib.machinery.SourceFileLoader)
        roots = {Path(path).resolve(strict=True) for path in spec.submodule_search_locations}
        if roots != {origin.parent}:
            raise RuntimeError(f"{name} package search path must match its trusted origin")
        if spec.loader.name != name or Path(spec.loader.path) != origin:
            raise RuntimeError(f"{name} source loader must match its trusted origin")
        return spec, origin

    package_spec, package_origin = _package_spec("iroha_python")
    owner_spec, owner_origin = _package_spec("iroha_native")
    native_spec = importlib.machinery.PathFinder.find_spec(
        "iroha_native._crypto", [str(owner_origin.parent)]
    )
    if native_spec is None or native_spec.loader_state is not None:
        raise RuntimeError("iroha_native._crypto must have an unmodified extension spec")
    native_origin = _trusted_origin(native_spec, "iroha_native._crypto", importlib.machinery.ExtensionFileLoader)
    if not any(native_origin.name == f"_crypto{suffix}" for suffix in importlib.machinery.EXTENSION_SUFFIXES):
        raise RuntimeError("iroha_native._crypto origin has the wrong platform suffix")
    if native_origin.parent != owner_origin.parent or native_spec.submodule_search_locations is not None:
        raise RuntimeError("native extension must belong to the authenticated iroha_native package")
    if native_spec.loader.name != "iroha_native._crypto" or Path(native_spec.loader.path) != native_origin:
        raise RuntimeError("iroha_native._crypto loader must match its trusted origin")

    owner = importlib.util.module_from_spec(owner_spec)
    sys.modules["iroha_native"] = owner
    owner_spec.loader.exec_module(owner)
    native = owner.load_crypto_extension()
    package = importlib.util.module_from_spec(package_spec)
    sys.modules["iroha_python"] = package
    package_spec.loader.exec_module(package)

    def _assert_loaded(module, name: str, origin: Path, loader_type: type) -> None:
        spec = module.__spec__
        if (
            sys.modules.get(name) is not module
            or spec is None
            or spec.loader_state is not None
            or module.__loader__ is not spec.loader
            or not isinstance(getattr(module, "__file__", None), str)
            or Path(module.__file__) != origin
            or _trusted_origin(spec, f"loaded {name}", loader_type) != origin
            or spec.loader.name != name
            or Path(spec.loader.path) != origin
        ):
            raise RuntimeError(f"loaded {name} spec changed from its trusted origin")

    _assert_loaded(owner, "iroha_native", owner_origin, importlib.machinery.SourceFileLoader)
    _assert_loaded(package, "iroha_python", package_origin, importlib.machinery.SourceFileLoader)
    _assert_loaded(native, "iroha_native._crypto", native_origin, importlib.machinery.ExtensionFileLoader)
else:
    _add_path(_ROOT / "iroha_python" / "src")
    _add_path(_ROOT / "iroha_native" / "src")
