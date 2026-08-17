"""Regression contract for TLAPM's Dune-projected Isabelle backend."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
FORMAL_SCRIPTS = ROOT_DIR / "scripts" / "formal"
HELPER = FORMAL_SCRIPTS / "sumeragi_v2_tlapm_source_lock.py"
LOCK = FORMAL_SCRIPTS / "sumeragi_v2_tlapm_source_build_lock.json"
LOCKED_WGET = FORMAL_SCRIPTS / "sumeragi_v2_tlapm_locked_wget.sh"
SOURCE_BUILDER = FORMAL_SCRIPTS / "build_sumeragi_v2_tlapm_from_source.sh"
FIXTURE_BYTES = b"bounded Isabelle derivation fixture\n"
MANIFEST_BYTES = b"Isabelle/bin/isabelle Isabelle/lib/runtime\n"
CRITICAL_SPAN_SHA256 = "859e47d5f99a3c467d8d7b7cbbb53d32eb62dc85951ef745f6a03f1883a7d802"


def _materialize(
    root: Path,
    relative: str,
    payload: bytes = FIXTURE_BYTES,
    *,
    mode: int = 0o400,
) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    path.chmod(mode)
    return path


def _replace_file(path: Path, payload: bytes, *, mode: int = 0o400) -> None:
    path.chmod(0o600)
    path.write_bytes(payload)
    path.chmod(mode)


def _fixture(root: Path) -> dict[str, Path]:
    root.mkdir(mode=0o700)
    fixture_sha256 = hashlib.sha256(FIXTURE_BYTES).hexdigest()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    platform_lock = lock["platforms"]["arm64-darwin"]
    for backend in platform_lock["backend_downloads"]:
        if backend["name"] in {"community-modules", "z3"}:
            backend["locked_output_sha256"] = fixture_sha256
    lock_path = root / "lock.json"
    lock_path.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")

    build_tree = root / "build"
    distribution_tree = root / "distribution"
    package_root = distribution_tree / "tlapm"
    isabelle_backend = next(
        backend
        for backend in platform_lock["backend_downloads"]
        if backend["name"] == "isabelle"
    )
    for backend in platform_lock["backend_downloads"]:
        if backend["derivation_kind"] == "tree":
            continue
        executable = backend["name"] in {"ls4", "z3"}
        mode = 0o500 if executable else 0o400
        _materialize(package_root, backend["package_path"], mode=mode)
        _materialize(build_tree, backend["build_path"], mode=mode)
    for build_relative, package_relative in (
        ("_build/default/translate/main.exe", "lib/tlapm/backends/bin/ptl_to_trp"),
        ("_build/default/deps/zenon/zenon", "lib/tlapm/backends/bin/zenon"),
    ):
        _materialize(build_tree, build_relative, mode=0o500)
        _materialize(package_root, package_relative, mode=0o500)

    build_isabelle = build_tree / isabelle_backend["build_path"]
    package_isabelle = package_root / isabelle_backend["package_path"]
    for tree, executable_mode, regular_mode in (
        (build_isabelle, 0o500, 0o400),
        (package_isabelle, 0o700, 0o600),
    ):
        _materialize(tree, "bin/isabelle", mode=executable_mode)
        _materialize(tree, "lib/runtime", mode=executable_mode)
        _materialize(tree, "etc/settings", mode=regular_mode)
        (tree / "lib/settings-link").symlink_to("../etc/settings")
    (build_isabelle / "heaps/polyml-fixture/log").mkdir(parents=True)
    (package_isabelle / "package-only-empty").mkdir()
    build_manifest = build_isabelle.parent / "Isabelle.exec-files"
    package_manifest = package_isabelle.parent / "Isabelle.exec-files"
    _materialize(build_manifest.parent, build_manifest.name, MANIFEST_BYTES)
    _materialize(package_manifest.parent, package_manifest.name, MANIFEST_BYTES)

    archive = root / "archive.tar.gz"
    archive.write_bytes(FIXTURE_BYTES)
    return {
        "archive": archive,
        "build_isabelle": build_isabelle,
        "build_manifest": build_manifest,
        "build_tree": build_tree,
        "distribution_tree": distribution_tree,
        "lock": lock_path,
        "package_isabelle": package_isabelle,
        "package_manifest": package_manifest,
        "package_root": package_root,
        "root": root,
    }


def _invoke(fixture: dict[str, Path], output_name: str = "attestation.json"):
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(HELPER),
            "--lock",
            str(fixture["lock"]),
            "--platform",
            "arm64-darwin",
            "write-attestation",
            "--archive",
            str(fixture["archive"]),
            "--build-tree",
            str(fixture["build_tree"]),
            "--distribution-tree",
            str(fixture["distribution_tree"]),
            "--locked-wget",
            str(LOCKED_WGET),
            "--source-builder",
            str(SOURCE_BUILDER),
            "--output",
            str(fixture["root"] / output_name),
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )


def _verify(fixture: dict[str, Path], attestation: Path):
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(HELPER),
            "--lock",
            str(fixture["lock"]),
            "--platform",
            "arm64-darwin",
            "verify-attestation",
            "--archive",
            str(fixture["archive"]),
            "--distribution-tree",
            str(fixture["distribution_tree"]),
            "--locked-wget",
            str(LOCKED_WGET),
            "--source-builder",
            str(SOURCE_BUILDER),
            "--attestation",
            str(attestation),
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )


def _load_helper_module():
    spec = importlib.util.spec_from_file_location("tlapm_source_lock_tested", HELPER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_isabelle_leaf_projection_accepts_only_dune_directory_and_mode_projection(
    tmp_path: Path,
) -> None:
    fixture = _fixture((tmp_path / "accepted").resolve())
    written = _invoke(fixture)
    assert written.returncode == 0, written.stderr
    attestation_path = fixture["root"] / "attestation.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    derivation = next(
        item for item in attestation["backend_derivations"] if item["name"] == "isabelle"
    )
    assert derivation["projection"] == "leaf-path-content-v1"
    assert derivation["build_output_sha256"] == derivation["packaged_sha256"]
    assert derivation["executable_manifest_sha256"] == hashlib.sha256(
        MANIFEST_BYTES
    ).hexdigest()
    verified = _verify(fixture, attestation_path)
    assert verified.returncode == 0, verified.stderr

    helper = _load_helper_module()
    closure_before = helper._distribution_closure_sha256(fixture["distribution_tree"])
    (fixture["package_isabelle"] / "package-only-empty").rmdir()
    closure_after = helper._distribution_closure_sha256(fixture["distribution_tree"])
    assert closure_before != closure_after


def _mutate_derivation(case: str, fixture: dict[str, Path]) -> None:
    build = fixture["build_isabelle"]
    package = fixture["package_isabelle"]
    if case == "missing-leaf":
        (package / "etc/settings").unlink()
    elif case == "extra-leaf":
        _materialize(package, "etc/extra")
    elif case == "mutated-leaf":
        _replace_file(package / "etc/settings", b"changed\n", mode=0o600)
    elif case == "symlink-target":
        (package / "lib/settings-link").unlink()
        (package / "lib/settings-link").symlink_to("../bin/isabelle")
    elif case == "symlink-to-file":
        (package / "lib/settings-link").unlink()
        _materialize(package, "lib/settings-link")
    elif case == "escaping-symlink":
        (package / "lib/settings-link").unlink()
        (package / "lib/settings-link").symlink_to("../../../escape")
    elif case == "directory-symlink":
        (package / "lib/settings-link").unlink()
        (package / "lib/settings-link").symlink_to("../etc")
    elif case == "hardlink":
        target = package / "etc/settings"
        target.unlink()
        os.link(package / "bin/isabelle", target)
    elif case == "writable-file":
        (package / "etc/settings").chmod(0o620)
    elif case == "special-entry":
        os.mkfifo(package / "special", 0o600)
    elif case == "manifest-omits-executable":
        for path in (fixture["build_manifest"], fixture["package_manifest"]):
            _replace_file(path, b"Isabelle/bin/isabelle\n")
    elif case == "manifest-duplicate":
        duplicate = b"Isabelle/bin/isabelle Isabelle/bin/isabelle Isabelle/lib/runtime\n"
        for path in (fixture["build_manifest"], fixture["package_manifest"]):
            _replace_file(path, duplicate)
    elif case == "manifest-unsafe":
        unsafe = b"Isabelle/bin/isabelle Isabelle/../escape\n"
        for path in (fixture["build_manifest"], fixture["package_manifest"]):
            _replace_file(path, unsafe)
    elif case == "manifest-mismatch":
        _replace_file(
            fixture["package_manifest"],
            b"Isabelle/lib/runtime Isabelle/bin/isabelle\n",
        )
    elif case == "manifest-noncanonical":
        for path in (fixture["build_manifest"], fixture["package_manifest"]):
            _replace_file(path, MANIFEST_BYTES.rstrip(b"\n"))
    elif case == "manifest-writable":
        fixture["package_manifest"].chmod(0o620)
    elif case == "manifest-hardlink":
        fixture["package_manifest"].unlink()
        os.link(fixture["build_manifest"], fixture["package_manifest"])
    elif case == "listed-package-file-not-executable":
        (package / "lib/runtime").chmod(0o600)
    elif case == "package-extra-executable":
        (package / "etc/settings").chmod(0o700)
    elif case == "missing-manifest":
        fixture["package_manifest"].unlink()
    elif case == "build-extra-executable":
        (build / "etc/settings").chmod(0o500)
    else:
        raise AssertionError(f"unknown mutation case: {case}")


@pytest.mark.parametrize(
    "case",
    (
        "missing-leaf",
        "extra-leaf",
        "mutated-leaf",
        "symlink-target",
        "symlink-to-file",
        "escaping-symlink",
        "directory-symlink",
        "hardlink",
        "writable-file",
        "special-entry",
        "manifest-omits-executable",
        "manifest-duplicate",
        "manifest-unsafe",
        "manifest-mismatch",
        "manifest-noncanonical",
        "manifest-writable",
        "manifest-hardlink",
        "listed-package-file-not-executable",
        "package-extra-executable",
        "missing-manifest",
        "build-extra-executable",
    ),
)
def test_isabelle_leaf_projection_rejects_provenance_mutations(
    tmp_path: Path, case: str
) -> None:
    fixture = _fixture((tmp_path / case).resolve())
    _mutate_derivation(case, fixture)
    rejected = _invoke(fixture)
    assert rejected.returncode != 0
    assert "TLAPM source lock error:" in rejected.stderr


def _critical_span(source: str) -> str:
    return source[
        source.index("def _tree_digest(") : source.index("\ndef _shell_assignments(")
    ]


def _assert_static_projection_contract(source: str) -> None:
    critical = _critical_span(source)
    assert hashlib.sha256(critical.encode()).hexdigest() == CRITICAL_SPAN_SHA256
    assert source.count("include_directories=False") == 2
    assert 'return _tree_digest(directory / "tlapm", include_modes=False)' in source
    assert '"projection": _ISABELLE_DERIVATION_PROJECTION' not in source
    assert 'derivation["projection"] = _ISABELLE_DERIVATION_PROJECTION' in critical
    assert "if built_digest != packaged_digest:" in critical
    assert "if built_manifest != packaged_manifest:" in critical
    assert critical.count("require_exact_executable_set=True") == 2
    assert "require_exact_executable_set=False" not in critical
    assert 'tree_root.joinpath(*relative.parts[1:])' in critical


def test_isabelle_derivation_static_contract_is_exact() -> None:
    _assert_static_projection_contract(HELPER.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    ("old", "new"),
    (
        ("include_directories=False", "include_directories=True"),
        (
            "_tree_digest(\n                    build_path,",
            "_tree_digest(\n                    package_path,",
        ),
        ("if built_digest != packaged_digest:", "if False:"),
        ("if built_manifest != packaged_manifest:", "if False:"),
        ("require_exact_executable_set=True", "require_exact_executable_set=False"),
        (
            'return _tree_digest(directory / "tlapm", include_modes=False)',
            'return _tree_digest(directory / "tlapm", include_modes=False, '
            "include_directories=False)",
        ),
        ("or not metadata.st_mode & 0o111", "or False"),
        ("derivations.append(derivation)", "derivations.append({**derivation})"),
    ),
)
def test_isabelle_derivation_static_contract_rejects_mutations(
    old: str, new: str
) -> None:
    source = HELPER.read_text(encoding="utf-8")
    assert old in source
    mutated = source.replace(old, new, 1)
    with pytest.raises((AssertionError, ValueError)):
        _assert_static_projection_contract(mutated)
