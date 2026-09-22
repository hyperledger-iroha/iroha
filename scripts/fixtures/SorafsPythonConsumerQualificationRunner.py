"""Fixed same-process SoraFS reference child; consumes a parent's captured inputs.

Run with a private Python 3.12 interpreter using -I -B and --input INPUT.json.
Inputs are prebuilt installed native/SDK wheels, their original FileSeals, and a
closed captured source inventory. No builds, network, installation, arbitrary
pytest options or output paths are accepted. Stdout contains bounded captured
logs followed by one report frame; the parent must retain and authenticate both.

The source-owned parent supplies runtime, dependency and child-process custody.
TODO: complete the original-index adapter and independently authenticated
candidate/producer approval. This child is not a sandbox or release authority;
synthetic controls cannot qualify actual wheel/native execution.
"""
from __future__ import annotations

import argparse
import base64
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import importlib.machinery
import importlib.util
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import site
import stat
import sys
import sysconfig
import types
import unicodedata

INPUT_SCHEMA = "sorafs.python.reference_child_input.v1"
REPORT_SCHEMA = "sorafs.python.reference_child_report.v1"
FRAME = b"SORAFS_PYTHON_REPORT_V1="
MAX_INPUT = 4 * 1024 * 1024
MAX_REPORT = 8 * 1024 * 1024
MAX_LOG = 32 * 1024 * 1024
MAX_FILES = 8192
MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_FILE_BYTES = 16 * 1024 * 1024
RUNNER = "scripts/fixtures/SorafsPythonConsumerQualificationRunner.py"
VERIFIER = "ci/verify_privacy_python_wheel.py"
CASES = "scripts/sorafs_python_consumer_cases.py"
TEST = "python/iroha_python/tests/sorafs_reference_validation_test.py"
SOURCE_TREES = ("fixtures/sorafs_manifest", "python/norito_py/src",
                "python/iroha_torii_client")
_DIGEST = re.compile(r"[0-9a-f]{64}\Z")


class QualificationError(ValueError):
    """A captured input, actual owner or execution observation was refused."""


def require(condition: bool, message: str) -> None:
    """Refuse without emitting a success report."""
    if not condition:
        raise QualificationError(message)


def canonical_json(value: object) -> bytes:
    """Encode the one closed report/input representation."""
    return (json.dumps(value, sort_keys=True, separators=(",", ":"),
                       ensure_ascii=True, allow_nan=False) + "\n").encode("utf-8")


def _pairs(rows: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in rows:
        require(key not in result, "duplicate JSON field")
        result[key] = value
    return result


def _object(value: object, fields: set[str]) -> dict:
    require(type(value) is dict and set(value) == fields, "closed input fields differ")
    return value


def _absolute_content(value: object) -> PurePosixPath:
    """Validate one logical POSIX path without opening its historical location."""
    require(type(value) is str and 0 < len(value.encode("utf-8")) <= 4096
            and unicodedata.normalize("NFC", value) == value
            and not any(ord(char) < 32 or ord(char) == 127 for char in value), "invalid absolute path")
    path = PurePosixPath(value)
    require(path.is_absolute() and path.anchor == "/" and str(path) == value
            and ".." not in path.parts, "path is not canonical")
    return path


def _absolute(value: object) -> Path:
    path = Path(_absolute_content(value))
    require(path.resolve(strict=True) == path, "path is not canonical")
    return path


def _relative(value: object) -> str:
    require(type(value) is str and 0 < len(value.encode("utf-8")) <= 1024
            and unicodedata.normalize("NFC", value) == value
            and not any(ord(char) < 32 or ord(char) == 127 for char in value), "invalid source path")
    path = PurePosixPath(value)
    require(not path.is_absolute() and str(path) == value
            and all(part not in ("", ".", "..") for part in value.split("/"))
            and "\\" not in value, "source path is not canonical")
    return value


def _identity_row(value: object, *, relative: bool) -> dict:
    row = _object(value, {"path", "sha256", "size"})
    _relative(row["path"]) if relative else _absolute_content(row["path"])
    require(type(row["sha256"]) is str and _DIGEST.fullmatch(row["sha256"]) is not None,
            "invalid source digest")
    require(type(row["size"]) is int and 0 <= row["size"] <= MAX_FILE_BYTES,
            "source size exceeds bound")
    return row


def _stat_identity(metadata: os.stat_result) -> tuple:
    return (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns,
            metadata.st_ctime_ns, metadata.st_mode, metadata.st_nlink)


def read_stable(path: Path, *, limit: int = MAX_FILE_BYTES) -> tuple[bytes, tuple]:
    """Read bounded singly linked bytes with original metadata retained for recheck."""
    require(path.resolve(strict=True) == path, "source path changed or aliases")
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1,
            "source is not a singly linked regular file")
    require(before.st_size <= limit, "file size exceeds bound")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as stream:
        opened = os.fstat(stream.fileno())
        require(_stat_identity(before) == _stat_identity(opened), "source changed before open")
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    require(len(raw) <= limit and _stat_identity(before) == _stat_identity(after)
            == _stat_identity(path.lstat()), "source changed while read")
    seal = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns,
            before.st_ctime_ns, before.st_mode, hashlib.sha256(raw).hexdigest())
    return raw, seal


def parse_input_content(raw: bytes) -> dict:
    """Validate closed input bytes and logical labels without filesystem access.

    Wheel seals remain bounded strings here. The live child validates them with
    its authenticated snapshot verifier later; an offline adapter must use its
    already authenticated sole FileSeal parser. This grants no live path owner.
    """
    require(type(raw) is bytes and 0 < len(raw) <= MAX_INPUT, "input byte bound")
    value = json.loads(raw, object_pairs_hook=_pairs,
                       parse_constant=lambda _value: require(False, "nonfinite JSON"))
    _object(value, {"schema", "snapshot_root", "environment_root", "native_wheel",
                    "sdk_wheel", "source_files", "python"})
    require(canonical_json(value) == raw and value["schema"] == INPUT_SCHEMA,
            "input is not canonical V1 JSON")
    snapshot = _absolute_content(value["snapshot_root"])
    environment = _absolute_content(value["environment_root"])
    require(snapshot != environment, "source and environment roots must be distinct")
    for name in ("native_wheel", "sdk_wheel"):
        wheel = _object(value[name], {"path", "seal"})
        _absolute_content(wheel["path"])
        require(type(wheel["seal"]) is str and len(wheel["seal"]) <= 512,
                "invalid wheel seal")
    _identity_row(value["python"], relative=False)
    rows = value["source_files"]
    require(type(rows) is list and 4 < len(rows) <= MAX_FILES, "source inventory bound")
    names = []
    total = 0
    for row in rows:
        row = _identity_row(row, relative=True)
        names.append(row["path"])
        total += row["size"]
        require(total <= MAX_SOURCE_BYTES, "aggregate source byte bound")
        require(row["path"] in (RUNNER, VERIFIER, CASES, TEST)
                or any(row["path"].startswith(tree + "/") for tree in SOURCE_TREES),
                "source inventory contains an unowned path")
    require(names == sorted(set(names)) and {RUNNER, VERIFIER, CASES, TEST} <= set(names),
            "source inventory is unordered, duplicate or missing a fixed owner")
    return value


def parse_input(raw: bytes) -> dict:
    """Apply the sole byte algorithm, then require its current live path owners."""
    value = parse_input_content(raw)
    snapshot = _absolute(value["snapshot_root"])
    environment = _absolute(value["environment_root"])
    require(snapshot.is_dir() and environment.is_dir(),
            "source and environment roots must be distinct directories")
    for name in ("native_wheel", "sdk_wheel"):
        _absolute(value[name]["path"])
    _absolute(value["python"]["path"])
    return value


def capture_sources(inputs: dict) -> tuple[dict[str, bytes], dict[str, tuple]]:
    """Seal the whole snapshot, including ancestors pytest could import as packages."""
    root = Path(inputs["snapshot_root"])
    expected = {row["path"] for row in inputs["source_files"]}
    directories = {".", *SOURCE_TREES}
    for name in (*expected, *SOURCE_TREES):
        directories.update(str(parent) for parent in PurePosixPath(name).parents)
    observed, observed_directories, seals = set(), set(), {}
    pending, entries = [root], 0
    while pending:
        directory = pending.pop()
        relative = directory.relative_to(root).as_posix()
        require(relative in directories and directory.resolve(strict=True) == directory,
                "unowned or aliased snapshot directory")
        before = directory.lstat()
        require(stat.S_ISDIR(before.st_mode), "snapshot directory changed type")
        observed_directories.add(relative)
        seals["d:" + relative] = _stat_identity(before)
        with os.scandir(directory) as stream:
            for entry in stream:
                entries += 1
                require(entries <= MAX_FILES, "snapshot entry bound")
                require(not entry.is_symlink(), "snapshot symlink")
                if entry.is_dir(follow_symlinks=False):
                    pending.append(Path(entry.path))
                else:
                    require(entry.is_file(follow_symlinks=False), "snapshot special file")
                    observed.add(Path(entry.path).relative_to(root).as_posix())
        require(_stat_identity(directory.lstat()) == seals["d:" + relative],
                "snapshot directory changed during enumeration")
    require(observed == expected and observed_directories == directories,
            "complete snapshot inventory differs")
    contents = {}
    for row in inputs["source_files"]:
        raw, seal = read_stable(root / row["path"])
        require(len(raw) == row["size"] and seal[-1] == row["sha256"], "source identity mismatch")
        contents[row["path"]], seals[row["path"]] = raw, seal
    require(all(_stat_identity((root / name).lstat()) == seals["d:" + name]
                for name in directories), "snapshot directories changed during source reads")
    return contents, seals


def load_source(name: str, path: Path, raw: bytes) -> types.ModuleType:
    """Execute already captured tool bytes; never reopen an unverified tool for import."""
    require(name not in sys.modules, "tool module was preseeded")
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


class BoundedLog(io.TextIOBase):
    """Retain actual Python-level output; outer pipe comparison rejects FD bypasses."""
    def __init__(self) -> None:
        super().__init__()
        self.payload = bytearray()
        self.exceeded = False

    def write(self, value: str) -> int:
        if len(value) > MAX_LOG - len(self.payload):
            self.exceeded = True
            raise QualificationError("captured log exceeds byte bound")
        for offset in range(0, len(value), 8192):
            encoded = value[offset:offset + 8192].encode("utf-8", "strict")
            if len(encoded) > MAX_LOG - len(self.payload):
                self.exceeded = True
                raise QualificationError("captured log exceeds byte bound")
            self.payload.extend(encoded)
        return len(value)

    def flush(self) -> None:
        pass

    def isatty(self) -> bool:
        return False


class CaseObserver:
    """Observe actual pytest collection and all ordered successful phase reports."""
    def __init__(self, expected: tuple[str, ...]) -> None:
        require(len(expected) == 77 and len(set(expected)) == 77, "fixed case inventory")
        self.expected = expected
        self.collected = False
        self.current = None
        self.rows = []
        self.errors = []

    def refuse(self, condition: bool, message: str) -> None:
        if not condition:
            if len(self.errors) < 16:
                self.errors.append(message)

    def pytest_collection_modifyitems(self, session, config, items) -> None:
        self.refuse(not self.collected, "duplicate collection")
        self.collected = True
        self.refuse(tuple(item.nodeid for item in items) == self.expected, "actual case inventory differs")
        self.refuse(all(not any(item.iter_markers(name=name))
                        for item in items for name in ("skip", "skipif", "xfail")),
                    "skip/xfail marker is forbidden")

    def pytest_collectreport(self, report) -> None:
        self.refuse(report.outcome == "passed", "collection failed or skipped")

    def pytest_deselected(self, items) -> None:
        self.refuse(not items, "cases were deselected")

    def pytest_runtest_logstart(self, nodeid, location) -> None:
        self.refuse(self.current is None and len(self.rows) < 77
                    and nodeid == self.expected[len(self.rows)], "case start order differs")
        self.current = {"nodeid": nodeid, "phases": []}

    def pytest_runtest_logreport(self, report) -> None:
        self.refuse(self.current is not None and report.nodeid == self.current["nodeid"],
                    "phase lost original case owner")
        if self.current is None:
            return
        phases = self.current["phases"]
        self.refuse(len(phases) < 3 and report.when == ("setup", "call", "teardown")[len(phases)]
                    and report.outcome == "passed" and not hasattr(report, "wasxfail"),
                    "phase missing, repeated, failed, skipped or xfailed")
        if len(phases) < 3:
            phases.append({"phase": report.when, "outcome": report.outcome})

    def pytest_runtest_logfinish(self, nodeid, location) -> None:
        self.refuse(self.current is not None and self.current["nodeid"] == nodeid
                    and len(self.current["phases"]) == 3, "case finish lost complete phase owner")
        if self.current is not None and len(self.rows) < 77:
            self.rows.append(self.current)
        self.current = None

    def finish(self, exit_code: int) -> list[dict]:
        require(exit_code == 0 and self.collected and self.current is None
                and len(self.rows) == 77 and not self.errors,
                "complete reference execution refused: " + "; ".join(self.errors))
        return self.rows


def module_state(module: object) -> tuple:
    """Retain mutable spec/loader/search-path fields as values, not aliases."""
    spec = getattr(module, "__spec__", None)
    require(isinstance(spec, importlib.machinery.ModuleSpec), "loaded module lost its spec")
    loader = spec.loader
    return (module, spec, loader, getattr(module, "__file__", None),
            getattr(module, "__package__", None), getattr(module, "__name__", None),
            None if not hasattr(module, "__path__") else tuple(module.__path__),
            spec.name, spec.origin, spec.loader_state, spec.cached, spec.has_location,
            None if spec.submodule_search_locations is None else tuple(spec.submodule_search_locations),
            getattr(loader, "name", None), getattr(loader, "path", None))


def loaded_members(verifier, owners: list[tuple]) -> tuple[list[list[dict]], dict]:
    """Bind every actually loaded SDK/native/dependency descendant to original bytes."""
    groups, retained = [], {}
    for prefix, root, members, native_name in owners:
        rows = []
        names = sorted(name for name in sys.modules if name == prefix or name.startswith(prefix + "."))
        require(prefix in names and len(names) <= MAX_FILES, "loaded package inventory bound")
        for name in names:
            module = sys.modules[name]
            state = module_state(module)
            spec, loader = state[1:3]
            loader_type = (importlib.machinery.ExtensionFileLoader if name == native_name
                           else importlib.machinery.SourceFileLoader)
            require(type(loader) is loader_type, "loaded owner has an unexpected loader")
            path = _absolute(getattr(module, "__file__", None))
            member = path.relative_to(root).as_posix()
            require(member in members, "loaded module is outside its authenticated members")
            if name != native_name:
                expected_member = name.replace(".", "/")
                if prefix in ("norito", "iroha_torii_client"):
                    expected_member = expected_member.removeprefix(prefix).lstrip("/")
                require(member in (expected_member + ".py", (expected_member + "/" if expected_member else "") + "__init__.py"),
                        "loaded module name does not match its member")
            verifier._assert_loaded_module(module=module, spec=spec, expected_name=name, expected_path=path)
            require(loader.name == name and Path(loader.path) == path, "mutable loader origin differs")
            expected_search = (str(path.parent),) if path.name == "__init__.py" else None
            require(state[6] == expected_search and state[12] == expected_search,
                    "loaded package search path differs")
            raw, _seal = read_stable(path, limit=verifier.MAX_MEMBER_BYTES)
            digest = hashlib.sha256(raw).hexdigest()
            require((digest, len(raw)) == members[member], "loaded member bytes differ")
            row = {"name": name, "path": str(path), "sha256": digest,
                   "size": len(raw), "loader": loader_type.__name__}
            if prefix in ("iroha_native", "iroha_python"):
                row["member"] = member
            rows.append(row)
            retained[name] = state
        groups.append(rows)
    return groups, retained


def require_retained_modules(before: dict, after: dict) -> None:
    """Require original object identity and copied immutable descriptor fields."""
    require(all(name in after
                and all(after[name][index] is state[index] for index in range(3))
                and after[name][3:] == state[3:] for name, state in before.items()),
            "loaded module/spec/loader owner changed during tests")


def machinery() -> tuple:
    """Snapshot path and hook owners; legitimate importer-cache additions are not hooks."""
    return tuple(sys.path), tuple(map(id, sys.meta_path)), tuple(map(id, sys.path_hooks))


def execute(inputs: dict, raw_input: bytes, log: BoundedLog) -> dict:
    """Authenticate, execute and reauthenticate in this one original process."""
    require(sys.version_info[:2] == (3, 12) and sys.flags.isolated == 1
            and sys.dont_write_bytecode, "child requires Python 3.12 -I -B")
    require(os.environ.get("PYTEST_DISABLE_PLUGIN_AUTOLOAD") == "1"
            and not any(key.startswith("PYTEST_") and key != "PYTEST_DISABLE_PLUGIN_AUTOLOAD"
                        for key in os.environ), "ambient pytest configuration is forbidden")
    snapshot = Path(inputs["snapshot_root"])
    require(Path(__file__).resolve() == snapshot / RUNNER, "runner is outside captured source")
    contents, source_seals = capture_sources(inputs)
    runtime = inputs["python"]
    python_path = Path(sys.executable).resolve(strict=True)
    require(str(python_path) == runtime["path"], "executing Python differs")
    python_bytes, python_seal = read_stable(python_path)
    require((len(python_bytes), python_seal[-1]) == (runtime["size"], runtime["sha256"]),
            "Python executable identity differs")
    verifier = load_source("_sorafs_owned_wheel_verifier", snapshot / VERIFIER, contents[VERIFIER])
    cases = load_source("_sorafs_owned_python_cases", snapshot / CASES, contents[CASES])
    expected = cases.expected_node_ids(contents[TEST])
    environment = Path(inputs["environment_root"])
    native_input, sdk_input = inputs["native_wheel"], inputs["sdk_wheel"]
    norito_root, torii_root = snapshot / "python/norito_py/src", snapshot / "python/iroha_torii_client"
    verifier.verify_current_environment(environment, Path(native_input["path"]), native_input["seal"],
                                        norito_root, torii_root, Path(sdk_input["path"]), sdk_input["seal"])
    wheels = [verifier.preflight_wheel(Path(row["path"]), row["seal"], owner=owner)
              for row, owner in ((native_input, verifier.NATIVE_OWNER), (sdk_input, verifier.SDK_OWNER))]
    sites = {Path(value) for value in (*site.getsitepackages(), sysconfig.get_paths()["purelib"],
                                      sysconfig.get_paths()["platlib"])}
    layouts = [verifier.derive_installed_layout(environment_root=environment, site_roots=sites, wheel=wheel)
               for wheel in wheels]
    installed = [verifier.verify_installed_files(wheel, layout) for wheel, layout in zip(wheels, layouts, strict=True)]
    dependencies = verifier.authenticate_dependency_roots(environment_root=environment,
                                                         norito_root=norito_root, torii_root=torii_root)
    owners = [(wheel.owner.package, layout.site_root,
               {member.name: (member.sha256, member.size) for member in wheel.package_members},
               "iroha_native._crypto" if wheel.owner == verifier.NATIVE_OWNER else None)
              for wheel, layout in zip(wheels, layouts, strict=True)]
    for dependency in dependencies:
        root = dependency.initializer_path.parent
        members = {str((snapshot / row["path"]).relative_to(root)): (row["sha256"], row["size"])
                   for row in inputs["source_files"] if (snapshot / row["path"]).is_relative_to(root)}
        owners.append((dependency.module_name, root, members, None))
    _before_rows, retained = loaded_members(verifier, owners)
    native = sys.modules["iroha_native._crypto"]
    sorafs = sys.modules["iroha_python.sorafs"]
    require(sorafs._crypto is native, "SoraFS did not retain original native owner")
    baseline = machinery()
    import pytest
    require(pytest.__version__ == cases.PYTEST_VERSION, "pytest version differs")
    pytest_path = _absolute(pytest.__file__)
    require(pytest_path.is_relative_to(environment), "pytest is outside private environment")
    pytest_bytes, pytest_seal = read_stable(pytest_path)
    observer = CaseObserver(expected)
    exit_code = pytest.main(["-s", "--noconftest", "--assert=plain", "--import-mode=importlib",
                             "-p", "no:cacheprovider", "-p", "no:terminal", "-c", os.devnull,
                             "--rootdir", str(snapshot), str(snapshot / TEST)], plugins=[observer])
    observations = observer.finish(int(exit_code))
    require(not log.exceeded, "log bound exceeded during tests")
    require(machinery() == baseline, "test execution changed import machinery")
    require(sys.modules.get("iroha_python.sorafs") is sorafs and sorafs._crypto is native,
            "SoraFS native owner was not restored after teardown")
    loaded_rows, current = loaded_members(verifier, owners)
    require_retained_modules(retained, current)
    wheel_rows = []
    for wheel, layout, before, modules in zip(wheels, layouts, installed, loaded_rows[:2], strict=True):
        after = verifier.verify_installed_files(wheel, layout)
        require(after == before, "installed wheel files changed during tests")
        verifier._assert_unique_distribution_origin(wheel, layout)
        verifier.assert_expected_file_seal(wheel.path, wheel.seal, label="original executed wheel",
                                           max_bytes=verifier.MAX_WHEEL_BYTES)
        wheel_rows.append({"owner": wheel.owner.package, "path": str(wheel.path), "seal": wheel.seal.render(),
                           "version": wheel.metadata_version,
                           "installed_files": [{"path": str(item.path), "seal": item.seal.render()}
                                               for item in sorted(after.files, key=lambda item: str(item.path))],
                           "loaded_modules": modules})
    require(verifier.authenticate_dependency_roots(environment_root=environment, norito_root=norito_root,
                                                   torii_root=torii_root) == dependencies,
            "dependency source owners changed during tests")
    _after_contents, after_seals = capture_sources(inputs)
    require(after_seals == source_seals, "captured source original owners changed")
    require(read_stable(python_path)[1] == python_seal and read_stable(pytest_path)[1] == pytest_seal,
            "runtime tool file changed")
    if log.payload and not log.payload.endswith(b"\n"):
        log.write("\n")
    return {"schema": REPORT_SCHEMA, "input_sha256": hashlib.sha256(raw_input).hexdigest(),
            "source_files": inputs["source_files"],
            "python": {**runtime, "version": ".".join(map(str, sys.version_info[:3]))},
            "pytest": {"version": pytest.__version__, "path": str(pytest_path),
                       "sha256": hashlib.sha256(pytest_bytes).hexdigest(), "size": len(pytest_bytes)},
            "wheels": wheel_rows,
            "dependencies": [{"module": owner.module_name, "root": str(owner.path),
                              "loaded_modules": rows} for owner, rows in zip(dependencies, loaded_rows[2:], strict=True)],
            "cases": observations,
            "captured_output": {"bytes": len(log.payload), "sha256": hashlib.sha256(log.payload).hexdigest()}}


def main() -> int:
    """Emit only actual captured logs and one bounded report after every check passes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True, help="parent-owned canonical input JSON")
    args = parser.parse_args()
    output = sys.stdout.buffer
    log = BoundedLog()
    try:
        raw, input_seal = read_stable(_absolute(str(args.input)), limit=MAX_INPUT)
        inputs = parse_input(raw)
        with redirect_stdout(log), redirect_stderr(log):
            report = execute(inputs, raw, log)
        require(read_stable(args.input, limit=MAX_INPUT)[1] == input_seal, "input owner changed")
        encoded = canonical_json(report)
        require(len(encoded) <= MAX_REPORT, "report byte bound")
        require(FRAME not in log.payload, "captured output contains a report-frame marker")
        output.write(log.payload)
        output.write(FRAME + base64.b64encode(encoded) + b"\n")
        output.flush()
        return 0
    except Exception as error:
        output.write(log.payload)
        output.flush()
        sys.stderr.write((f"qualification refused: {type(error).__name__}: {error}")[:4096] + "\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
