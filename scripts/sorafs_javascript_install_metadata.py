"""Exact generated npm hidden-lock relation for the fixed offline consumer.

Path strings are historical labels, never opened. The parent authenticates the
selected npm/runtime inputs and retains actual execution. Comparing these bytes
proves a graph/content relation only, not that npm ran or produced the file.
"""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import PurePosixPath
import posixpath
import re

from sorafs_javascript_archive import NpmArchive, _require
from sorafs_javascript_dependencies import DependencyLock, MAX_LOCK_BYTES, _json

CONSUMER_NAME = "iroha-sorafs-javascript-consumer"
CONSUMER_VERSION = "1.0.0"
HIDDEN_LOCK = ".package-lock.json"
SDK_LOCATION = "node_modules/@iroha/iroha-js"


def _label(value: str) -> str:
    _require(type(value) is str and 1 < len(value) <= 4096
             and re.fullmatch(r"/[A-Za-z0-9._@/-]+", value) is not None
             and not value.startswith("//") and str(PurePosixPath(value)) == value
             and ".." not in PurePosixPath(value).parts,
             "npm installation path label is not canonical bounded POSIX ASCII")
    return value


def consumer_package(sdk_archive_label: str) -> bytes:
    """Construct the one private parent-owned package.json without filesystem I/O."""
    path = _label(sdk_archive_label)
    value = {"name": CONSUMER_NAME, "version": CONSUMER_VERSION, "private": True,
             "type": "module", "dependencies": {"@iroha/iroha-js": "file://" + path}}
    return (json.dumps(value, indent=2) + "\n").encode("ascii")


def verify_npm_hidden_lock(raw: bytes, *, archive: NpmArchive, lock: DependencyLock,
                           environment_label: str, sdk_archive_label: str) -> None:
    """Join the generated closed ten-location lock to original package/lock bytes."""
    environment, sdk_path = _label(environment_label), _label(sdk_archive_label)
    package = _json(archive.files()["package.json"], 128 * 1024)
    original = _json(lock.raw, MAX_LOCK_BYTES)
    sdk = {"version": lock.sdk_version,
           "resolved": "file:" + posixpath.relpath(sdk_path, environment),
           "integrity": "sha512-" + base64.b64encode(hashlib.sha512(archive.raw).digest()).decode("ascii"),
           "dependencies": package["dependencies"], "engines": package["engines"]}
    expected = {"name": CONSUMER_NAME, "version": CONSUMER_VERSION,
                "lockfileVersion": 3, "requires": True,
                "packages": {SDK_LOCATION: sdk, **{row.location: original["packages"][row.location]
                                                 for row in lock.dependencies}}}
    observed = _json(raw, MAX_LOCK_BYTES)
    _require(type(observed.get("lockfileVersion")) is int and observed.get("requires") is True
             and observed == expected,
             "generated npm hidden lock differs from exact original graph or normalized archive label")
