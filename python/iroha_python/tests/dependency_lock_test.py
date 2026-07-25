from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Iterable

import pytest
from packaging.requirements import Requirement
from packaging.version import Version

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    tomllib = None  # type: ignore[assignment]

SDK_ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = SDK_ROOT.parent
REPO_ROOT = PYTHON_ROOT.parent
INPUT_PATH = SDK_ROOT / "requirements-ci.in"
LOCK_PATH = SDK_ROOT / "requirements-ci.lock"
LOCAL_PROJECT_NAMES = {"iroha-norito", "iroha-python", "iroha-torii-client"}
HASH_PATTERN = re.compile(r"--hash=sha256:[0-9a-f]{64}(?:\s|$)")


def _canonical_name(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def _requirement_lines(path: Path) -> Iterable[str]:
    pending: list[str] = []
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        pending.append(line.removesuffix("\\").rstrip())
        if not line.endswith("\\"):
            yield " ".join(pending)
            pending.clear()
    assert not pending, f"unterminated requirement in {path}"


def _requirements_by_name(path: Path) -> dict[str, Requirement]:
    requirements: dict[str, Requirement] = {}
    for line in _requirement_lines(path):
        requirement = Requirement(line.split()[0])
        requirements[_canonical_name(requirement.name)] = requirement
    return requirements


def _pyproject(path: Path) -> dict[str, Any]:
    assert tomllib is not None
    with path.open("rb") as source:
        return tomllib.load(source)


def _expected_direct_requirements() -> list[Requirement]:
    pyprojects = [
        _pyproject(SDK_ROOT / "pyproject.toml"),
        _pyproject(PYTHON_ROOT / "iroha_torii_client" / "pyproject.toml"),
        _pyproject(PYTHON_ROOT / "norito_py" / "pyproject.toml"),
    ]
    metadata = [pyproject["project"] for pyproject in pyprojects]
    requirements = [
        Requirement(specification)
        for project in metadata
        for specification in project.get("dependencies", [])
        if _canonical_name(Requirement(specification).name) not in LOCAL_PROJECT_NAMES
    ]
    sdk_dev = metadata[0]["optional-dependencies"]["dev"]
    requirements.extend(
        Requirement(specification)
        for specification in sdk_dev
        if _canonical_name(Requirement(specification).name) == "pytest"
    )
    requirements.extend(
        Requirement(specification) for specification in pyprojects[0]["build-system"]["requires"]
    )
    return requirements


def _exact_version(requirement: Requirement) -> Version:
    specifiers = list(requirement.specifier)
    assert len(specifiers) == 1
    assert specifiers[0].operator == "=="
    return Version(specifiers[0].version)


@pytest.mark.skipif(tomllib is None, reason="project metadata check requires Python 3.11+")
def test_ci_dependency_roots_match_local_project_metadata() -> None:
    input_requirements = _requirements_by_name(INPUT_PATH)
    expected_requirements = _expected_direct_requirements()
    expected_names = {_canonical_name(requirement.name) for requirement in expected_requirements}

    assert set(input_requirements) == expected_names
    for metadata_requirement in expected_requirements:
        name = _canonical_name(metadata_requirement.name)
        version = _exact_version(input_requirements[name])
        assert version in metadata_requirement.specifier


def test_ci_lock_is_exact_and_hash_pinned() -> None:
    input_requirements = _requirements_by_name(INPUT_PATH)
    lock_requirements = _requirements_by_name(LOCK_PATH)

    assert set(input_requirements) <= set(lock_requirements)
    for name, requirement in lock_requirements.items():
        assert _exact_version(requirement)
        logical_line = next(
            line
            for line in _requirement_lines(LOCK_PATH)
            if _canonical_name(Requirement(line.split()[0]).name) == name
        )
        assert HASH_PATTERN.search(logical_line), f"{name} has no SHA-256 artifact hash"
    for name, requirement in input_requirements.items():
        assert _exact_version(requirement) == _exact_version(lock_requirements[name])


def test_ci_uses_checkout_sources_and_blake3_runtime() -> None:
    import blake3
    import iroha_torii_client
    import norito

    assert (
        Path(iroha_torii_client.__file__)
        .resolve()
        .is_relative_to(PYTHON_ROOT / "iroha_torii_client")
    )
    assert Path(norito.__file__).resolve().is_relative_to(PYTHON_ROOT / "norito_py")
    assert len(blake3.blake3(b"numeric-v1").digest()) == 32


def test_numeric_workflow_enforces_the_ci_lock() -> None:
    workflow = (REPO_ROOT / ".github/workflows/numeric_v1_sdk.yml").read_text(encoding="utf-8")

    assert "cache-dependency-path: python/iroha_python/requirements-ci.lock" in workflow
    assert "--require-hashes" in workflow
    assert "--only-binary=:all:" in workflow
    assert "PYTHONPATH: src:../norito_py/src:.." in workflow
