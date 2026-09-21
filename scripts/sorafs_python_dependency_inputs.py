"""Exact independently pinned offline wheel inputs for the fixed Python child.

Library only; no download, archive parser, installation or process execution.
Fifteen ordinary dependency artifacts serve POSIX CPython 3.12. Existing privacy
CI's pytest 8.4.2 lock is not silently repurposed for the fixed pytest 9.0.3 child.
Pinned pip owns future installation; the parent/adapter must still establish
complete installed-file and loaded-module joins from these actual artifacts.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Mapping

from sorafs_python_consumer_artifact import _require
from sorafs_python_runtime_inputs import closed, file_reference, pinned_json
from sorafs_sdk_artifact_index import FileReference, OpenedIndexFiles

SCHEMA = "sorafs.python.offline_dependencies.v1"
MAX_MANIFEST_BYTES = 64 * 1024
MAX_WHEEL_BYTES = 256 * 1024 * 1024
MAX_WHEELS_BYTES = 1024 * 1024 * 1024
MODULES = ("blake3", "certifi", "cffi", "charset-normalizer", "cryptography", "idna",
           "iniconfig", "packaging", "pip", "pluggy", "pycparser", "pygments", "pytest",
           "requests", "urllib3")
DIRECT_VERSIONS = {"blake3": "1.0.9", "cffi": "2.1.1", "cryptography": "50.0.1",
                   "pycparser": "3.0", "pytest": "9.0.3", "requests": "2.33.0"}


@dataclass(frozen=True)
class DependencyWheel:
    """One exact original artifact identity, not a parsed or installed wheel."""
    module: str
    version: str
    file: FileReference


@dataclass(frozen=True)
class DependencyManifest:
    """Closed original-input selection with an independently supplied content pin."""
    raw: bytes
    sha256: str
    wheels: tuple[DependencyWheel, ...]


def parse_dependency_manifest(raw: bytes, *, expected_sha256: str) -> DependencyManifest:
    """Validate exact POSIX 3.12 dependency identities without accepting online resolution."""
    row = closed(pinned_json(raw, expected_sha256, MAX_MANIFEST_BYTES), {"schema", "wheels"}, "dependency manifest")
    _require(row["schema"] == SCHEMA and type(row["wheels"]) is list
             and len(row["wheels"]) == len(MODULES), "dependency profile/inventory differs")
    result = []
    for value, module in zip(row["wheels"], MODULES, strict=True):
        value = closed(value, {"module", "version", "file"}, "dependency wheel")
        _require(value["module"] == module, "dependency modules must match the exact sorted closure")
        version = value["version"]
        _require(type(version) is str and len(version) <= 64
                 and re.fullmatch(r"(?:0|[1-9][0-9]*)(?:\.(?:0|[1-9][0-9]*)){1,3}", version) is not None,
                 "dependency version must be an exact stable release")
        _require(module not in DIRECT_VERSIONS or version == DIRECT_VERSIONS[module], "dependency differs from current source pin")
        reference = file_reference(value["file"], absolute=True)
        _require(0 < reference.size <= MAX_WHEEL_BYTES and reference.path.endswith(".whl"), "dependency must be a bounded nonempty wheel artifact")
        result.append(DependencyWheel(module, version, reference))
    _require(len({wheel.file.path for wheel in result}) == len(MODULES), "dependency original paths alias")
    _require(sum(wheel.file.size for wheel in result) <= MAX_WHEELS_BYTES, "dependency aggregate byte bound")
    return DependencyManifest(raw, expected_sha256, tuple(result))


def authenticate_dependency_inputs(owner: OpenedIndexFiles, manifest: DependencyManifest,
                                   *, paths_by_module: Mapping[str, str]) -> tuple[DependencyWheel, ...]:
    """Read each exact dependency through its original held index owner, never live paths.

    The mapping joins retained indexed paths to producer-original absolute labels.
    Actual wheel/installed metadata interpretation is deliberately not performed.
    No second or weaker native/SDK wheel parser is introduced.
    """
    _require(type(owner) is OpenedIndexFiles and type(manifest) is DependencyManifest,
             "dependency bytes require their original index and parsed manifest owners")
    _require(parse_dependency_manifest(manifest.raw, expected_sha256=manifest.sha256) == manifest,
             "dependency inventory projection differs from its pinned bytes")
    _require(set(paths_by_module) == set(MODULES)
             and len(set(paths_by_module.values())) == len(MODULES), "dependency input mapping is not exact")
    consumer = owner.index.consumer("python")
    for wheel in manifest.wheels:
        path = paths_by_module[wheel.module]
        _require(type(path) is str and path in consumer.inputs, "dependency is not an original Python input")
        reference = owner.index.file(path)
        _require((reference.sha256, reference.size) == (wheel.file.sha256, wheel.file.size),
                 "dependency index identity differs from independent pin")
        owner.read(path, MAX_WHEEL_BYTES)
    owner.recheck()
    return manifest.wheels
