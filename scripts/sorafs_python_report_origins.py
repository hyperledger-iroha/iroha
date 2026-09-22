"""Join parsed Python child observations to original captured byte owners.

Paths are logical labels from the original producer, never reopened here. A
successful join authenticates byte relationships only: reported physical seals
remain observations, and no live file or process authority is manufactured.
The caller owns original-index authentication, parsed input/runtime custody,
command/output verification and approved producer/signature authority.
"""
from __future__ import annotations

from dataclasses import asdict
import hashlib
from pathlib import Path, PurePosixPath

from sorafs_python_consumer_artifact import (
    ArtifactError, ChildReport, FileIdentity, MAX_SOURCE_BYTES, MAX_SOURCE_FILES,
    MAX_SOURCE_FILE_BYTES, SCHEMA, _VERIFIER as verifier, _path, _require,
    _source_member,
)
from sorafs_python_consumer_cases import PYTEST_VERSION
from sorafs_python_dependency_install import MAX_ENVIRONMENT_BYTES, MAX_ENVIRONMENT_FILES, SITE
from sorafs_python_runtime_inputs import RuntimeManifest


def _identity(raw: bytes) -> tuple[str, int]:
    return hashlib.sha256(raw).hexdigest(), len(raw)


def _logical(value: Path) -> PurePosixPath:
    _require(type(value) in (type(Path()), PurePosixPath), "report origin path is not a logical Path")
    return PurePosixPath(_path(str(value), absolute=True))


def _inventory(files: dict[str, bytes], *, count: int, member_bytes: int, total_bytes: int) -> None:
    _require(type(files) is dict and 0 < len(files) <= count, "captured report origin inventory bound")
    total = 0
    for name, raw in files.items():
        _path(name, absolute=False)
        _require(type(raw) is bytes and len(raw) <= member_bytes, "captured report origin member bound")
        total += len(raw)
        _require(total <= total_bytes, "captured report origin aggregate byte bound")


def _member(files: dict[str, bytes], name: str) -> bytes:
    raw = files.get(name)
    _require(type(raw) is bytes, "reported origin has no captured original bytes: " + name)
    return raw


def _module_inventory(modules, *, owner: str, required: set[str]) -> None:
    names = tuple(module.name for module in modules)
    _require(type(modules) is tuple and 0 < len(modules) <= 4096
             and names == tuple(sorted(set(names))) and required <= set(names)
             and all(name == owner or name.startswith(owner + ".") for name in names)
             and len({module.path for module in modules}) == len(modules),
             "reported loaded module inventory differs from its owner")


def verify_report_origins(
    report: ChildReport,
    *,
    environment: Path,
    snapshot: Path,
    sources: dict[str, bytes],
    environment_files: dict[str, bytes],
    wheels: tuple[tuple[verifier.WheelArchive, verifier.InstalledWheelContent], ...],
    wheel_paths: tuple[Path, ...],
    wheel_seals: tuple[verifier.FileSeal, ...],
    runtime: RuntimeManifest,
) -> dict[str, bytes]:
    """Join a sole-parser ChildReport to the exact retained source/installed bytes.

    `wheels` is the original parsed native/SDK pair in that order; `wheel_paths`
    and `wheel_seals` are the logical original child-input observations already
    joined by the caller to indexed archive bytes. No path is opened or resolved.
    Installed physical device/inode/time/mode observations cannot be authenticated
    from captured bytes and never become a live owner here. The result borrows
    the four exact RECORD/direct_url byte values for archive metadata comparison.
    """
    _require(type(report) is ChildReport and report.schema == SCHEMA
             and type(runtime) is RuntimeManifest, "report/runtime requires the sole parsed contract")
    environment, snapshot = _logical(environment), _logical(snapshot)
    _require(environment != snapshot, "runtime and source snapshot labels alias")
    _inventory(sources, count=MAX_SOURCE_FILES, member_bytes=MAX_SOURCE_FILE_BYTES,
               total_bytes=MAX_SOURCE_BYTES)
    _inventory(environment_files, count=MAX_ENVIRONMENT_FILES, member_bytes=verifier.MAX_MEMBER_BYTES,
               total_bytes=MAX_ENVIRONMENT_BYTES)
    expected_sources = tuple(FileIdentity(name, *_identity(raw)) for name, raw in sorted(sources.items()))
    _require(report.source_files == expected_sources, "reported source inventory differs from captured source bytes")

    executable = _member(environment_files, "bin/python3.12")
    expected_python = (runtime.executable.sha256, runtime.executable.size)
    _require(report.python.path == str(environment / "bin/python3.12")
             and report.python.version == runtime.version
             and (report.python.sha256, report.python.size) == expected_python == _identity(executable),
             "reported Python does not join original runtime and captured interpreter")
    site = environment / SITE.rstrip("/")
    pytest_member = SITE + "pytest/__init__.py"
    _require(report.pytest.path == str(environment / pytest_member)
             and report.pytest.version == PYTEST_VERSION
             and (report.pytest.sha256, report.pytest.size) == _identity(_member(environment_files, pytest_member)),
             "reported pytest does not join captured environment bytes")

    _require(all(type(value) is tuple and len(value) == 2
                 for value in (wheels, wheel_paths, wheel_seals, report.wheels)),
             "report origin requires the exact ordered native/SDK pair")
    metadata, installed_paths = {}, set()
    for pair, path, seal, observation, owner in zip(
        wheels, wheel_paths, wheel_seals, report.wheels,
        (verifier.NATIVE_OWNER, verifier.SDK_OWNER), strict=True,
    ):
        _require(type(pair) is tuple and len(pair) == 2, "original wheel/content pairing differs")
        wheel, content = pair
        _require(type(wheel) is verifier.WheelArchive and type(content) is verifier.InstalledWheelContent
                 and type(seal) is verifier.FileSeal and wheel.owner == content.owner == owner,
                 "report wheel pair is not the sole parsed original/content owner")
        path = _logical(path)
        _require(observation.owner == owner.package and observation.version == wheel.metadata_version
                 and observation.path == str(path) and asdict(observation.seal) == asdict(seal)
                 and content.source_uri == path.as_uri() and content.wheel_sha256 == seal.sha256,
                 "reported wheel identity differs from original archive observations")
        _require(len(content.files) <= verifier.MAX_ARCHIVE_MEMBERS + len(verifier.PIP_GENERATED_DIST_INFO_FILES)
                 and len({member.name for member in content.files}) == len(content.files),
                 "installed content member inventory differs")
        # Select the complete captured package/distribution family with the
        # original verifier's name predicate. Selecting only declared content
        # files would hide an extra file or competing installed distribution.
        captured = {}
        for name, raw in environment_files.items():
            if name.startswith(SITE):
                member = name[len(SITE):]
                top = member.split("/", 1)[0]
                if top.casefold() == owner.package.casefold() or verifier._is_matching_distribution_entry(top, owner):
                    captured[member] = raw
        try:
            derived = verifier.verify_installed_wheel_bytes(
                wheel, source_uri=path.as_uri(), wheel_sha256=seal.sha256, installed_files=captured)
        except verifier.VerificationError as error:
            raise ArtifactError("captured installed bytes do not join the original wheel: " + str(error)) from error
        _require(derived == content, "installed content belongs to a different original byte relation")
        expected_files = tuple(str(site / member.name) for member in content.files)
        _require(tuple(item.path for item in observation.installed_files) == expected_files,
                 "reported installed inventory differs from sole captured content")
        for member, observed in zip(content.files, observation.installed_files, strict=True):
            _require(observed.path not in installed_paths
                     and (observed.seal.sha256, observed.seal.size) == (member.sha256, member.size),
                     "reported installed file differs from captured original bytes")
            installed_paths.add(observed.path)
        originals = {member.name: member for member in wheel.package_members}
        required = {owner.package, "iroha_native._crypto" if owner.native else "iroha_python.sorafs"}
        _module_inventory(observation.loaded_modules, owner=owner.package, required=required)
        for module in observation.loaded_modules:
            member = originals.get(module.member)
            native_module = module.name == "iroha_native._crypto"
            _require(member is not None and module.path == str(site / member.name)
                     and (module.sha256, module.size) == (member.sha256, member.size)
                     and module.loader == ("ExtensionFileLoader" if native_module else "SourceFileLoader")
                     and (member.name == wheel.native_member if native_module
                          else _source_member(module.name, member.name)),
                     "reported loaded module does not join an original wheel package member")
        for basename in ("RECORD", "direct_url.json"):
            name = wheel.dist_info_root + "/" + basename
            key = "installed-metadata/" + name
            _require(key not in metadata, "wheel metadata owners alias")
            metadata[key] = captured[name]

    _require(type(report.dependencies) is tuple and len(report.dependencies) == 2,
             "report origin requires both source dependencies")
    for dependency, owner, prefix in zip(
        report.dependencies, ("norito", "iroha_torii_client"),
        ("python/norito_py/src/", "python/iroha_torii_client/"), strict=True,
    ):
        root = snapshot / prefix.rstrip("/")
        _require(dependency.module == owner and dependency.root == str(root),
                 "reported source dependency belongs to another snapshot")
        _module_inventory(dependency.loaded_modules, owner=owner, required={owner})
        for module in dependency.loaded_modules:
            path = PurePosixPath(_path(module.path, absolute=True))
            _require(path.is_relative_to(root), "reported source module escaped its logical snapshot")
            member = path.relative_to(root).as_posix()
            named_member = member if owner == "norito" else owner + "/" + member
            _require(module.member is None and module.loader == "SourceFileLoader"
                     and _source_member(module.name, named_member)
                     and (module.sha256, module.size) == _identity(_member(sources, prefix + member)),
                     "reported source module does not join captured dependency bytes")
    return metadata
