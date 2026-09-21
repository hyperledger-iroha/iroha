"""Closed immutable Python child observations; no execution qualification authority.

The parent owns the actual child, exit status, separate stderr, original runtime
and dependency inputs. The adapter must join these observations to original
indexed wheel bytes using ci.verify_privacy_python_wheel.parse_wheel_bytes.
Parsing a self-consistent report never authenticates its producer or interpreter.
"""
from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass
import hashlib
import importlib.util
import json
from pathlib import Path, PurePosixPath
import re
import sys
import unicodedata

from sorafs_python_consumer_cases import PYTEST_VERSION, TEST_PATH, expected_node_ids

SCHEMA = "sorafs.python.reference_child_report.v1"
REPORT_PREFIX = b"SORAFS_PYTHON_REPORT_V1="
MAX_REPORT_BYTES = 8 * 1024 * 1024
MAX_LOG_BYTES = 32 * 1024 * 1024
MAX_OUTPUT_BYTES = MAX_LOG_BYTES + 4 * ((MAX_REPORT_BYTES + 2) // 3) + len(REPORT_PREFIX) + 1
MAX_SOURCE_FILES = 8192
MAX_SOURCE_FILE_BYTES = 16 * 1024 * 1024
MAX_SOURCE_BYTES = 64 * 1024 * 1024
FIXED_SOURCES = frozenset(("scripts/fixtures/SorafsPythonConsumerQualificationRunner.py",
                           "ci/verify_privacy_python_wheel.py", "scripts/sorafs_python_consumer_cases.py", TEST_PATH))
SOURCE_PREFIXES = ("fixtures/sorafs_manifest/", "python/norito_py/src/", "python/iroha_torii_client/")
MAX_NODE_BYTES = 16384
MAX_PATH_BYTES = 4096
MAX_FILES = 16384
MAX_FILE_BYTES = 512 * 1024 * 1024
_DIGEST = re.compile(r"[0-9a-f]{64}\Z")
_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*\Z")
_VERSION = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\Z")


class ArtifactError(ValueError):
    """An observation violates the exact child contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactError(message)


def _load_verifier():
    """Load the sole seal/archive owner from this tool's trusted repository path."""
    name = "_sorafs_python_wheel_verifier"
    spec = importlib.util.spec_from_file_location(
        name, Path(__file__).resolve().parents[1] / "ci/verify_privacy_python_wheel.py")
    _require(spec is not None and spec.loader is not None, "wheel verifier owner is absent")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        if previous is None:
            del sys.modules[name]
        else:
            sys.modules[name] = previous
    return module


_VERIFIER = _load_verifier()


def _text_size(value: str) -> int:
    try:
        return len(value.encode("utf-8", "strict"))
    except UnicodeError as error:
        raise ArtifactError("text is not valid UTF-8") from error


def canonical_json(value: object) -> bytes:
    """Return the sole UTF-8 JSON representation of a child report."""
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False) + "\n").encode("utf-8")


def _object(value: object, fields: set[str], label: str) -> dict:
    _require(type(value) is dict and set(value) == fields, f"{label} fields differ")
    return value


def _integer(value: object, label: str, maximum: int, minimum: int = 0) -> int:
    _require(type(value) is int and minimum <= value <= maximum, f"{label} integer is outside its bound")
    return value


def _digest(value: object) -> str:
    _require(type(value) is str and _DIGEST.fullmatch(value) is not None and value != "0" * 64,
             "digest must be canonical nonzero SHA-256")
    return value


def _path(value: object, *, absolute: bool) -> str:
    _require(type(value) is str and value and _text_size(value) <= MAX_PATH_BYTES,
             "path exceeds its bound")
    _require(unicodedata.normalize("NFC", value) == value and "\\" not in value
             and not any(ord(c) < 32 or ord(c) == 127 for c in value), "path is not canonical text")
    path = PurePosixPath(value)
    _require(path.is_absolute() == absolute and path.as_posix() == value
             and 0 < len(path.parts) <= 64 and value not in ("/", ".") and ".." not in path.parts
             and all(_text_size(part) <= 255 for part in path.parts), "path is not canonical POSIX")
    return value


def _rows(value: object, label: str, maximum: int = MAX_FILES) -> list:
    _require(type(value) is list and 0 < len(value) <= maximum, f"{label} inventory is empty or excessive")
    return value


def _ordered(values: tuple[str, ...], label: str) -> None:
    _require(values == tuple(sorted(set(values))), f"{label} is not sorted and unique")


@dataclass(frozen=True)
class FileIdentity:
    path: str
    sha256: str
    size: int


@dataclass(frozen=True)
class ToolIdentity:
    path: str
    sha256: str
    size: int
    version: str


@dataclass(frozen=True)
class FileSeal:
    sha256: str
    device: int
    inode: int
    size: int
    mtime_ns: int
    ctime_ns: int
    mode: int


@dataclass(frozen=True)
class SealedFile:
    path: str
    seal: FileSeal


@dataclass(frozen=True)
class LoadedModule:
    name: str
    path: str
    sha256: str
    size: int
    loader: str
    member: str | None = None


@dataclass(frozen=True)
class WheelObservation:
    owner: str
    path: str
    seal: FileSeal
    version: str
    installed_files: tuple[SealedFile, ...]
    loaded_modules: tuple[LoadedModule, ...]


@dataclass(frozen=True)
class DependencyObservation:
    module: str
    root: str
    loaded_modules: tuple[LoadedModule, ...]


@dataclass(frozen=True)
class Phase:
    phase: str
    outcome: str


@dataclass(frozen=True)
class Case:
    nodeid: str
    phases: tuple[Phase, ...]


@dataclass(frozen=True)
class CapturedOutput:
    bytes: int
    sha256: str


@dataclass(frozen=True)
class ChildReport:
    schema: str
    input_sha256: str
    source_files: tuple[FileIdentity, ...]
    python: ToolIdentity
    pytest: ToolIdentity
    wheels: tuple[WheelObservation, ...]
    dependencies: tuple[DependencyObservation, ...]
    cases: tuple[Case, ...]
    captured_output: CapturedOutput


@dataclass(frozen=True)
class RuntimeOutput:
    output: bytes
    report_bytes: bytes
    observations: ChildReport


def _file(value: object, *, absolute: bool) -> FileIdentity:
    row = _object(value, {"path", "sha256", "size"}, "file identity")
    return FileIdentity(_path(row["path"], absolute=absolute), _digest(row["sha256"]),
                        _integer(row["size"], "file size", MAX_FILE_BYTES))


def _tool(value: object, *, python: bool) -> ToolIdentity:
    row = _object(value, {"path", "sha256", "size", "version"}, "tool identity")
    version = row["version"]
    _require(type(version) is str and len(version) <= 64 and (re.fullmatch(r"3\.12\.(?:0|[1-9][0-9]*)", version) is not None
                                     if python else version == PYTEST_VERSION), "tool version differs")
    return ToolIdentity(_path(row["path"], absolute=True), _digest(row["sha256"]),
                        _integer(row["size"], "tool size", MAX_FILE_BYTES, 1), version)


def _seal(value: object) -> FileSeal:
    _require(type(value) is str and len(value) <= 512, "file seal is malformed")
    try:
        seal = _VERIFIER.FileSeal.parse(value)
    except (ValueError, _VERIFIER.VerificationError) as error:
        raise ArtifactError("file seal is invalid or noncanonical") from error
    # Observation bounds supplement the sole canonical syntax owner above.
    _digest(seal.sha256)
    _require(seal.inode > 0 and seal.size <= MAX_FILE_BYTES, "file seal identity is invalid")
    return FileSeal(seal.sha256, seal.device, seal.inode, seal.size,
                    seal.mtime_ns, seal.ctime_ns, seal.mode)


def _source_member(name: str, member: str) -> bool:
    stem = name.replace(".", "/")
    return member in (stem + ".py", stem + "/__init__.py")


def _modules(value: object, owner: str, *, wheel: bool) -> tuple[LoadedModule, ...]:
    modules = []
    fields = {"name", "path", "sha256", "size", "loader"} | ({"member"} if wheel else set())
    for value in _rows(value, "loaded modules", 4096):
        row = _object(value, fields, "loaded module")
        name = row["name"]
        _require(type(name) is str and len(name) <= 512 and _NAME.fullmatch(name) is not None
                 and (name == owner or name.startswith(owner + ".")), "loaded module name has a different owner")
        loader = row["loader"]
        expected = "ExtensionFileLoader" if name == "iroha_native._crypto" else "SourceFileLoader"
        _require(loader == expected, "loaded module loader differs")
        member = _path(row["member"], absolute=False) if wheel else None
        if member is not None:
            if name == "iroha_native._crypto":
                _require(member.startswith("iroha_native/_crypto.")
                         and "/" not in member.removeprefix("iroha_native/")
                         and member.endswith((".so", ".pyd")), "native loaded member differs")
            else:
                _require(_source_member(name, member), "module name differs from its wheel member")
        modules.append(LoadedModule(name, _path(row["path"], absolute=True), _digest(row["sha256"]),
                                   _integer(row["size"], "module size", MAX_FILE_BYTES), loader, member))
    result = tuple(modules)
    _ordered(tuple(m.name for m in result), "loaded module names")
    _require(len({m.path for m in result}) == len(result), "loaded module paths are duplicated")
    _require(owner in {m.name for m in result}, "loaded package initializer is absent")
    return result


def _wheels(value: object) -> tuple[WheelObservation, ...]:
    rows = _rows(value, "wheels", 2)
    _require(len(rows) == 2, "both fixed wheel owners are required")
    result, seen_paths, seen_physical = [], set(), set()
    def admit(path: str, seal: FileSeal) -> None:
        _require(path not in seen_paths, "wheel or installed paths are duplicated")
        physical = seal.device, seal.inode
        _require(physical not in seen_physical, "different wheel files share a physical identity")
        seen_paths.add(path); seen_physical.add(physical)
    for value, owner in zip(rows, ("iroha_native", "iroha_python")):
        row = _object(value, {"owner", "path", "seal", "version", "installed_files", "loaded_modules"}, "wheel")
        _require(row["owner"] == owner, "wheel owner order differs")
        path = _path(row["path"], absolute=True); seal = _seal(row["seal"])
        _require(path.endswith(".whl") and seal.size > 0, "wheel identity is not a positive wheel file")
        admit(path, seal)
        version = row["version"]
        _require(type(version) is str and len(version) <= 64 and _VERSION.fullmatch(version) is not None, "wheel version is not canonical")
        files = []
        for entry in _rows(row["installed_files"], "installed files", 4096):
            entry = _object(entry, {"path", "seal"}, "installed file")
            installed = SealedFile(_path(entry["path"], absolute=True), _seal(entry["seal"]))
            admit(installed.path, installed.seal); files.append(installed)
        _ordered(tuple(f.path for f in files), "installed files")
        modules = _modules(row["loaded_modules"], owner, wheel=True)
        by_path = {f.path: f.seal for f in files}
        site_roots = set()
        for module in modules:
            installed = by_path.get(module.path)
            _require(installed is not None and installed.sha256 == module.sha256 and installed.size == module.size,
                     "loaded module does not join an installed sealed file")
            _require(module.path.endswith("/" + module.member), "loaded module path differs from its wheel member")
            site_roots.add(module.path[:-len(module.member)])
        _require(len(site_roots) == 1, "loaded wheel members have different installation roots")
        required = {owner, "iroha_native._crypto" if owner == "iroha_native" else "iroha_python.sorafs"}
        _require(required <= {m.name for m in modules}, "required native/SoraFS loaded owner is absent")
        result.append(WheelObservation(owner, path, seal, version, tuple(files), modules))
    _require(result[0].version == result[1].version, "native and SDK wheel versions differ")
    return tuple(result)


def _dependencies(value: object, sources: tuple[FileIdentity, ...]) -> tuple[DependencyObservation, ...]:
    rows = _rows(value, "dependencies", 2)
    _require(len(rows) == 2, "both source dependency owners are required")
    result = []; source_map = {f.path: f for f in sources}; snapshot_roots = set()
    for value, owner, prefix in zip(rows, ("norito", "iroha_torii_client"),
                                   ("python/norito_py/src/", "python/iroha_torii_client/")):
        row = _object(value, {"module", "root", "loaded_modules"}, "dependency")
        _require(row["module"] == owner, "source dependency owner order differs")
        root = _path(row["root"], absolute=True)
        suffix = "/" + prefix.removesuffix("/")
        _require(root.endswith(suffix), "source dependency root differs from copied layout")
        snapshot_roots.add(root[:-len(suffix)])
        modules = _modules(row["loaded_modules"], owner, wheel=False)
        for module in modules:
            _require(module.path.startswith(root + "/"), "dependency module escaped its source root")
            member = module.path[len(root) + 1:]
            named_member = member if owner == "norito" else owner + "/" + member
            _require(_source_member(module.name, named_member), "dependency module name differs from source member")
            source = source_map.get(prefix + member)
            _require(source is not None and source.sha256 == module.sha256 and source.size == module.size,
                     "dependency module does not join a retained source file")
        result.append(DependencyObservation(owner, root, modules))
    _require(len(snapshot_roots) == 1 and "" not in snapshot_roots, "source dependency roots use different snapshots")
    return tuple(result)


def parse_report(raw: bytes, *, expected_input_sha256: str, test_source: bytes) -> ChildReport:
    """Parse exact child observations, bound to independently supplied input/test bytes."""
    expected_input_sha256 = _digest(expected_input_sha256)
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_REPORT_BYTES, "report bytes exceed their bound")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            _require(key not in result, "duplicate JSON field")
            result[key] = value
        return result
    try:
        value = json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(ArtifactError("nonfinite JSON number")))
        _require(canonical_json(value) == raw, "report JSON is not canonical")
    except (UnicodeError, ValueError, RecursionError, OverflowError) as error:
        raise ArtifactError("report JSON is invalid or noncanonical") from error
    row = _object(value, {"schema", "input_sha256", "source_files", "python", "pytest", "wheels", "dependencies", "cases", "captured_output"}, "report")
    _require(row["schema"] == SCHEMA and _digest(row["input_sha256"]) == expected_input_sha256,
             "report belongs to a different schema or child input")
    sources = tuple(_file(f, absolute=False) for f in _rows(row["source_files"], "source files", MAX_SOURCE_FILES))
    _ordered(tuple(f.path for f in sources), "source files")
    _require(FIXED_SOURCES <= {f.path for f in sources}
             and all(f.path in FIXED_SOURCES or f.path.startswith(SOURCE_PREFIXES) for f in sources),
             "source inventory differs from fixed child owners")
    _require(all(_text_size(f.path) <= 1024 and f.size <= MAX_SOURCE_FILE_BYTES for f in sources)
             and sum(f.size for f in sources) <= MAX_SOURCE_BYTES, "source inventory exceeds child bounds")
    try:
        expected = expected_node_ids(test_source)
    except (ValueError, SyntaxError, UnicodeError, RecursionError) as error:
        raise ArtifactError("reviewed test source cannot own this case inventory") from error
    _require(len(expected) == len(set(expected)) == 77, "reviewed source case inventory differs")
    test = next((f for f in sources if f.path == TEST_PATH), None)
    _require(test is not None and test.sha256 == hashlib.sha256(test_source).hexdigest() and test.size == len(test_source),
             "test-source identity differs from reviewed bytes")
    cases = _rows(row["cases"], "cases", 77)
    _require(len(cases) == 77, "report must contain exactly 77 cases")
    parsed = []
    for value, nodeid in zip(cases, expected):
        case = _object(value, {"nodeid", "phases"}, "case")
        _require(type(case["nodeid"]) is str and _text_size(case["nodeid"]) <= MAX_NODE_BYTES
                 and case["nodeid"] == nodeid, "case identity or exact order differs")
        phases = _rows(case["phases"], "case phases", 3)
        _require(len(phases) == 3, "case must retain every setup/call/teardown phase")
        for phase, name in zip(phases, ("setup", "call", "teardown")):
            _require(_object(phase, {"phase", "outcome"}, "phase") == {"phase": name, "outcome": "passed"},
                     "case phase is missing, repeated, reordered or not passed")
        parsed.append(Case(nodeid, tuple(Phase(p["phase"], p["outcome"]) for p in phases)))
    wheels = _wheels(row["wheels"])
    dependencies = _dependencies(row["dependencies"], sources)
    loaded_paths = [m.path for group in (*wheels, *dependencies) for m in group.loaded_modules]
    _require(len(loaded_paths) == len(set(loaded_paths)), "loaded paths alias across owners")
    python = _tool(row["python"], python=True); pytest = _tool(row["pytest"], python=False)
    distinct_paths = [python.path, pytest.path, *(wheel.path for wheel in wheels),
                      *(f.path for wheel in wheels for f in wheel.installed_files),
                      *(m.path for dep in dependencies for m in dep.loaded_modules)]
    _require(len(distinct_paths) == len(set(distinct_paths)), "different file owners share a path")
    output = _object(row["captured_output"], {"bytes", "sha256"}, "captured output")
    return ChildReport(SCHEMA, expected_input_sha256, sources, python, pytest, wheels, dependencies, tuple(parsed),
                       CapturedOutput(_integer(output["bytes"], "captured output size", MAX_LOG_BYTES), _digest(output["sha256"])))


def consume_runtime_output(raw: bytes, *, expected_input_sha256: str, test_source: bytes) -> RuntimeOutput:
    """Bind every byte before one final canonical frame to its child observation."""
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_OUTPUT_BYTES and raw.endswith(b"\n"),
             "runtime stream is empty, excessive or incomplete")
    _require(raw.count(REPORT_PREFIX) == 1, "runtime stream must contain exactly one report frame")
    offset = raw.index(REPORT_PREFIX)
    _require(offset == 0 or raw[offset-1:offset] == b"\n", "report frame must begin a line")
    _require(offset <= MAX_LOG_BYTES, "captured output exceeds its byte bound")
    output, encoded = raw[:offset], raw[offset+len(REPORT_PREFIX):-1]
    _require(encoded and len(encoded) <= ((MAX_REPORT_BYTES + 2) // 3) * 4
             and b"\n" not in encoded and b"\r" not in encoded, "report frame is malformed or excessive")
    try:
        report = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as error:
        raise ArtifactError("report frame is not canonical Base64") from error
    _require(base64.b64encode(report) == encoded, "report frame Base64 is not canonical")
    observations = parse_report(report, expected_input_sha256=expected_input_sha256, test_source=test_source)
    _require(observations.captured_output == CapturedOutput(len(output), hashlib.sha256(output).hexdigest()),
             "actual preceding process bytes differ from child captured_output")
    # TODO: The parent/adapter must independently bind full CPython and dependency
    # inputs, original indexed wheel bytes and native manifest. These observations
    # alone carry no original-input, installed-wheel, or process authority.
    return RuntimeOutput(output, report, observations)
