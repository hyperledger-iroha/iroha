"""Join parsed Python wheel payloads to the selected candidate's exact sources.

Only the two current build recipes are supported. The original wheel parser
owns ZIP/RECORD validation; the native manifest separately owns the extension.
This helper authenticates package source and declared data, not reproducible
backend metadata, process execution or release qualification. Call it before
installation and again before publication with the same OriginalInputs owner.
The producer also retains its whole-candidate clean/source-manifest checks.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat
import tomllib

from sorafs_python_producer_inputs import ArtifactError, OriginalInputs, child, verifier

MAX_SOURCE_FILES = 256
MAX_SOURCE_FILE_BYTES = 16 * 1024 * 1024
MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_SOURCE_ENTRIES = 4096
# Full original recipe pins: a changed backend, package finder, dynamic hook or
# data policy requires a reviewed source-selection change, never a fallback.
RECIPE_SHA256 = {
    "iroha_native": "147dfed7432ce18c72e547022c27617e378dbd2519f49e9b5c1b85a9cc22a22a",
    "iroha_python": "2aaa565e9e4ec1dd0d3b2d613c80dc96a81a75cfcbe117caecb6e29359a06a8e",
}
SDK_DATA = frozenset(("py.typed", "examples/connect_app_metadata.json"))
NATIVE_SOURCES = frozenset(("__init__.py", "_loader.py"))
EXCLUDED_SUFFIXES = frozenset((".so", ".dylib", ".pyd", ".pyc", ".pyo"))


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactError(message)


def authenticate_package_source(parsed: verifier.WheelArchive, source_root: Path,
                                originals: OriginalInputs) -> dict[str, bytes]:
    """Return original recipe/source bytes only after an exact member equality join.

    Returned names are repository-relative; retain them under ``package-source/``.
    No generated cache, native build artifact or egg-info is treated as source.
    Known excluded files are ignored only in the source tree, never in a wheel.
    Unknown source data, extra source packages, and missing/extra wheel members
    refuse. The same original owner must survive until final publication checks.
    """
    _require(type(parsed) is verifier.WheelArchive
             and parsed.owner in (verifier.NATIVE_OWNER, verifier.SDK_OWNER),
             "package source requires the sole parser's exact wheel layout")
    _require(type(originals) is OriginalInputs, "package source needs the original input owner")
    _require(isinstance(source_root, Path) and source_root.is_absolute()
             and source_root.resolve(strict=True) == source_root and source_root.is_dir(),
             "package source root must be the canonical selected candidate")
    package = parsed.owner.package
    project = source_root / "python" / package
    recipe_path = project / "pyproject.toml"
    recipe = originals.read(recipe_path, 128 * 1024, hold=True)
    _require(hashlib.sha256(recipe).hexdigest() == RECIPE_SHA256[package],
             "package build recipe differs from its reviewed exact selection")
    metadata = tomllib.loads(recipe.decode("utf-8", "strict"))["project"]
    _require(metadata["name"] == parsed.owner.distribution
             and metadata["version"] == parsed.metadata_version,
             "wheel name/version differs from the original candidate recipe")
    for name in ("setup.py", "setup.cfg", "MANIFEST.in"):
        _require(not os.path.lexists(project / name), "unowned package build selection input")
    retained = {recipe_path.relative_to(source_root).as_posix(): recipe}
    readme = project / metadata["readme"]
    retained[readme.relative_to(source_root).as_posix()] = originals.read(
        readme, MAX_SOURCE_FILE_BYTES, hold=True)
    source = project / "src"
    package_root = source / package
    directories = {}
    pending = [source]
    selected = {}
    count = 0
    total = sum(map(len, retained.values()))
    while pending:
        directory = pending.pop()
        _require(directory.resolve(strict=True) == directory, "package source directory aliases")
        before = directory.lstat()
        _require(stat.S_ISDIR(before.st_mode), "package source directory changed type")
        directories[directory] = child._stat_identity(before)
        with os.scandir(directory) as entries:
            for entry in entries:
                count += 1
                _require(count <= MAX_SOURCE_ENTRIES and not entry.is_symlink(),
                         "package source entry bound or symbolic link")
                path = Path(entry.path)
                if entry.is_dir(follow_symlinks=False):
                    if entry.name == "__pycache__" or path == source / (package + ".egg-info"):
                        continue
                    _require(path == package_root or path.is_relative_to(package_root),
                             "unowned source package outside the exact build selection")
                    pending.append(path)
                    continue
                _require(entry.is_file(follow_symlinks=False), "package source is not a regular file")
                if path.suffix in EXCLUDED_SUFFIXES:
                    continue
                _require(path.is_relative_to(package_root), "unowned file outside the source package")
                relative = path.relative_to(package_root).as_posix()
                child._relative(relative)
                allowed = (relative in NATIVE_SOURCES if parsed.owner.native
                           else path.suffix == ".py" or relative in SDK_DATA)
                _require(allowed, "source file is outside the reviewed package selection")
                _require(len(selected) < MAX_SOURCE_FILES, "package source file count exceeds bound")
                raw = originals.read(path, min(MAX_SOURCE_FILE_BYTES, MAX_SOURCE_BYTES - total), hold=True)
                total += len(raw)
                selected[package + "/" + relative] = (hashlib.sha256(raw).hexdigest(), len(raw))
                retained[path.relative_to(source_root).as_posix()] = raw
    _require(all(child._stat_identity(path.lstat()) == seal for path, seal in directories.items()),
             "package source tree changed during capture")
    _require(package + "/__init__.py" in selected, "canonical package initializer is missing")
    required = NATIVE_SOURCES if parsed.owner.native else SDK_DATA
    _require({package + "/" + name for name in required} <= set(selected),
             "declared package source/data is missing")
    native = parsed.native_member
    _require((native is not None) == parsed.owner.native,
             "native extension must remain owned by the native manifest contract")
    observed = {member.name: (member.sha256, member.size) for member in parsed.package_members
                if member.name != native}
    _require(observed == selected, "wheel package payload differs from exact candidate source members")
    allowed_directories = {str(parent) + "/" for name in (*selected, *((native,) if native else ()))
                           for parent in Path(name).parents if str(parent) != "."}
    _require(set(parsed.package_directories) <= allowed_directories,
             "wheel contains an unowned empty package directory")
    return dict(sorted(retained.items()))
