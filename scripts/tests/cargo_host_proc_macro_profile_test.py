"""Keep Cargo's local host proc macros loadable under the actual workspace profiles."""

from __future__ import annotations

import copy
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

import pytest


MANIFEST = Path(__file__).resolve().parents[2] / "Cargo.toml"


def _check_host_profiles(manifest: dict) -> None:
    profiles = manifest["profile"]
    for name in ("dev", "test", "release"):
        assert profiles[name]["build-override"].get("strip") == "none", (
            f"{name} host proc macros must retain loadable Mach-O metadata"
        )
    assert profiles["dev"]["build-override"] == profiles["test"]["build-override"], (
        "dev/test host profiles must share the same compilation settings"
    )
    for name in ("dev", "test"):
        assert profiles[name]["package"]["*"].get("strip") == "none", (
            f"{name} wildcard package override must retain loadable host proc macros"
        )
    assert profiles["dev"]["package"]["*"] == profiles["test"]["package"]["*"], (
        "dev/test wildcard profiles must share the same compilation settings"
    )


def _manifest() -> dict:
    return tomllib.loads(MANIFEST.read_text(encoding="utf-8"))


def test_actual_workspace_keeps_matching_loadable_host_profiles() -> None:
    _check_host_profiles(_manifest())


@pytest.mark.parametrize("name", ("dev", "test", "release"))
@pytest.mark.parametrize("strip", (None, "debuginfo", "symbols"))
def test_each_host_profile_rejects_implicit_or_explicit_stripping(
    name: str, strip: str | None
) -> None:
    manifest = copy.deepcopy(_manifest())
    host = manifest["profile"][name]["build-override"]
    if strip is None:
        del host["strip"]
        # A package/profile setting is insufficient: proc macros are host units.
        manifest["profile"][name]["strip"] = "none"
    else:
        host["strip"] = strip
    with pytest.raises(AssertionError, match=f"{name} host proc macros"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("field", ("debug", "codegen-units"))
def test_host_profile_drift_cannot_split_the_warm_dev_test_graph(field: str) -> None:
    manifest = copy.deepcopy(_manifest())
    host = manifest["profile"]["test"]["build-override"]
    host[field] += 1
    with pytest.raises(AssertionError, match="dev/test host profiles"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("name", ("dev", "test"))
@pytest.mark.parametrize("strip", (None, "debuginfo", "symbols"))
def test_wildcard_precedence_cannot_restore_unsafe_host_stripping(
    name: str, strip: str | None
) -> None:
    manifest = copy.deepcopy(_manifest())
    package = manifest["profile"][name]["package"]["*"]
    if strip is None:
        del package["strip"]
    else:
        package["strip"] = strip
    with pytest.raises(AssertionError, match=f"{name} wildcard package override"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("field", ("debug", "codegen-units"))
def test_wildcard_profile_drift_cannot_split_the_warm_dev_test_graph(field: str) -> None:
    manifest = copy.deepcopy(_manifest())
    package = manifest["profile"]["test"]["package"]["*"]
    package[field] += 1
    with pytest.raises(AssertionError, match="dev/test wildcard profiles"):
        _check_host_profiles(manifest)
