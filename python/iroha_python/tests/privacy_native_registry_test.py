from __future__ import annotations

import importlib
import re
from pathlib import Path

from iroha_python.privacy_catalog import (
    PRODUCTION_GATE_REQUIREMENTS,
    get_privacy_algorithm_descriptors,
)


_REPO_ROOT = Path(__file__).resolve().parents[3]
_RUST_REGISTRY_PATHS = (
    _REPO_ROOT / "crates/connect_norito_bridge/src/lib.rs",
    _REPO_ROOT / "crates/iroha_js_host/src/lib.rs",
    _REPO_ROOT / "python/iroha_python/iroha_python_rs/src/lib.rs",
)
_PYTHON_COMPONENT_MODULES = {
    "anonymous-pgc-k-out-of-n-v1": "iroha_python.anonymous_pgc",
    "verange-transparent-range-v1": "iroha_python.verange",
    "zkat-policy-private-auth-v1": "iroha_python.zkat",
    "zk-ams-recursive-admission-v0": "iroha_python.zk_ams",
    "vega-existing-credential-zk-v0": "iroha_python.vega",
    "silent-threshold-anoncred-v0": "iroha_python.silent_threshold",
    "zk-x509-onchain-identity-v0": "iroha_python.zk_x509",
    "jindo-lattice-pcs-zk-v0": "iroha_python.jindo",
    "sis-hints-anoncred-pq-v0": "iroha_python.sis_hints",
}

_ENTRY_RE = re.compile(
    r"PrivacyAlgorithmEntry\s*\{\s*"
    r'id:\s*"(?P<id>[^"]+)",\s*'
    r'proof_family:\s*"(?P<proof_family>[^"]+)",\s*'
    r'backend_family:\s*"(?P<backend_family>[^"]+)",\s*'
    r"sdk_entrypoints:\s*&\[(?P<sdk_entrypoints>.*?)\],\s*"
    r"planned_entrypoints:\s*&\[(?P<planned_entrypoints>.*?)\],\s*"
    r"\}",
    re.DOTALL,
)
_STRING_RE = re.compile(r'"([^"]+)"')
_GATE_RE = re.compile(
    r"const\s+PRIVACY_PRODUCTION_GATE_REQUIREMENTS:[^=]+=\s*&\[(?P<body>.*?)\];",
    re.DOTALL,
)
_GATE_ITEM_RE = re.compile(r'\("([^"]+)",\s*"([^"]+)"\)')


def _parse_string_list(body: str) -> tuple[str, ...]:
    return tuple(_STRING_RE.findall(body))


def _parse_native_registry(path: Path) -> dict[str, dict[str, object]]:
    source = path.read_text(encoding="utf-8")
    registry: dict[str, dict[str, object]] = {}
    for match in _ENTRY_RE.finditer(source):
        algorithm_id = match.group("id")
        assert algorithm_id not in registry, f"{path} duplicates {algorithm_id}"
        registry[algorithm_id] = {
            "proof_family": match.group("proof_family"),
            "backend_family": match.group("backend_family"),
            "sdk_entrypoints": _parse_string_list(match.group("sdk_entrypoints")),
            "planned_entrypoints": _parse_string_list(match.group("planned_entrypoints")),
        }
    assert registry, f"{path} must expose native privacy algorithm entries"
    return registry


def _parse_native_gate_requirements(path: Path) -> tuple[tuple[str, str], ...]:
    source = path.read_text(encoding="utf-8")
    match = _GATE_RE.search(source)
    assert match is not None, f"{path} must expose native privacy production gates"
    return tuple(_GATE_ITEM_RE.findall(match.group("body")))


def _snake_entrypoint_name(entrypoint: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", entrypoint).lower()


def _python_entrypoint_name_variants(entrypoint: str) -> set[str]:
    snake = _snake_entrypoint_name(entrypoint)
    return {
        entrypoint,
        snake,
        snake.replace("ve_range", "verange"),
        snake.replace("zk_at", "zkat"),
    }


def test_native_privacy_registries_match_each_other() -> None:
    registries = [_parse_native_registry(path) for path in _RUST_REGISTRY_PATHS]
    first = registries[0]

    for path, registry in zip(_RUST_REGISTRY_PATHS[1:], registries[1:]):
        assert registry == first, f"{path} drifted from {_RUST_REGISTRY_PATHS[0]}"


def test_native_privacy_registries_match_python_catalog() -> None:
    native = _parse_native_registry(_RUST_REGISTRY_PATHS[0])
    catalog = {
        descriptor["id"]: {
            "proof_family": descriptor["proof_family"],
            "backend_family": descriptor["backend_family"],
            "sdk_entrypoints": tuple(descriptor["sdk_entrypoints"]),
            "planned_entrypoints": tuple(descriptor["planned_sdk_entrypoints"]),
        }
        for descriptor in get_privacy_algorithm_descriptors()
    }

    assert set(native) == set(catalog)
    for algorithm_id, native_entry in native.items():
        assert native_entry["proof_family"] == catalog[algorithm_id]["proof_family"]
        assert native_entry["backend_family"] == catalog[algorithm_id]["backend_family"]
        assert native_entry["sdk_entrypoints"] == catalog[algorithm_id]["sdk_entrypoints"]
        assert (
            native_entry["planned_entrypoints"]
            == catalog[algorithm_id]["planned_entrypoints"]
        )


def test_python_component_modules_match_cataloged_sdk_builder_surface() -> None:
    catalog = {descriptor["id"]: descriptor for descriptor in get_privacy_algorithm_descriptors()}

    assert set(_PYTHON_COMPONENT_MODULES).issubset(catalog)
    for algorithm_id, module_name in _PYTHON_COMPONENT_MODULES.items():
        descriptor = catalog[algorithm_id]
        module = importlib.import_module(module_name)
        exports = set(getattr(module, "__all__", ()))
        planned_entrypoints = tuple(descriptor["planned_sdk_entrypoints"])
        planned_names = set().union(
            *(_python_entrypoint_name_variants(name) for name in planned_entrypoints)
        )

        assert descriptor["implementation_stage"] in {"component", "sdk-builder"}
        assert descriptor["sdk_entrypoints"], f"{algorithm_id} must expose dev builder entrypoints"
        assert descriptor["planned_sdk_entrypoints"], (
            f"{algorithm_id} must keep production entrypoints planned until gates pass"
        )
        for entrypoint in descriptor["sdk_entrypoints"]:
            assert entrypoint in exports, f"{module_name}.__all__ dropped {entrypoint}"
            assert callable(getattr(module, entrypoint)), f"{module_name}.{entrypoint} is not callable"

        assert planned_names.isdisjoint(exports), (
            f"{module_name} must not export planned production entrypoints until gates pass"
        )
        for entrypoint in planned_names:
            assert not hasattr(module, entrypoint), (
                f"{module_name}.{entrypoint} is planned production surface and must remain absent"
            )


def test_python_package_root_matches_cataloged_component_sdk_surface() -> None:
    catalog = {descriptor["id"]: descriptor for descriptor in get_privacy_algorithm_descriptors()}
    package = importlib.import_module("iroha_python")
    exports = set(getattr(package, "__all__", ()))

    for algorithm_id in _PYTHON_COMPONENT_MODULES:
        descriptor = catalog[algorithm_id]
        module = importlib.import_module(_PYTHON_COMPONENT_MODULES[algorithm_id])
        module_exports = set(getattr(module, "__all__", ()))
        planned_entrypoints = tuple(descriptor["planned_sdk_entrypoints"])
        planned_names = set().union(
            *(_python_entrypoint_name_variants(name) for name in planned_entrypoints)
        )

        for entrypoint in descriptor["sdk_entrypoints"]:
            module_callable = getattr(module, entrypoint)
            alias_entrypoints = {
                name
                for name in module_exports
                if name != entrypoint
                and callable(getattr(module, name, None))
                and getattr(module, name) is module_callable
            }
            assert entrypoint in exports, f"iroha_python.__all__ dropped {entrypoint}"
            assert alias_entrypoints, (
                f"{module.__name__}.{entrypoint} must expose at least one Python alias"
            )
            assert callable(getattr(package, entrypoint)), (
                f"iroha_python.{entrypoint} is not callable"
            )
            for alias in alias_entrypoints:
                assert alias in exports, f"iroha_python.__all__ dropped alias {alias}"
                assert callable(getattr(package, alias)), f"iroha_python.{alias} is not callable"

        assert planned_names.isdisjoint(exports), (
            "iroha_python must not export planned production entrypoints until gates pass"
        )
        for entrypoint in planned_names:
            assert not hasattr(package, entrypoint), (
                f"iroha_python.{entrypoint} is planned production surface and must remain absent"
            )


def test_native_privacy_gate_requirements_match_python_catalog() -> None:
    expected = tuple(PRODUCTION_GATE_REQUIREMENTS)

    for path in _RUST_REGISTRY_PATHS:
        assert _parse_native_gate_requirements(path) == expected
