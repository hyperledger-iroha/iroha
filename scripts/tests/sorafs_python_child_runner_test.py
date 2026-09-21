"""Synthetic child ownership/phase controls; never native wheel qualification.

The source-owned original wheel harness is extracted unchanged for real archive,
installed-file and source-loader controls; its extension is explicitly inert.
The 77-case pytest control replaces test bodies only in a target-local synthetic
module, retaining real source decorators and exact pytest-generated identities.
Actual canonical native assertions require the future parent-owned execution.
"""
from __future__ import annotations

import ast
import copy
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT / "scripts/fixtures/SorafsPythonConsumerQualificationRunner.py"
CASES_PATH = ROOT / "scripts/sorafs_python_consumer_cases.py"
TEST_PATH = ROOT / "python/iroha_python/tests/sorafs_reference_validation_test.py"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


runner = load("python_child_runner_controls", RUNNER_PATH)
cases = load("python_child_cases_controls", CASES_PATH)


def expected():
    return cases.expected_node_ids(TEST_PATH.read_bytes())


def test_exact_source_inventory_and_long_parameter() -> None:
    nodes = expected()
    assert len(nodes) == len(set(nodes)) == 77
    assert len(cases.FUNCTIONS) == 37
    assert max(map(len, nodes)) > 10_000
    assert any(node.endswith("[1.0_0]") for node in nodes)
    assert any(node.endswith("[1.0_1]") for node in nodes)


@pytest.mark.parametrize("mutation", ["name", "parameter", "profile", "duplicate", "mutable", "empty"])
def test_source_inventory_rejects_changed_owners(mutation: str) -> None:
    source = TEST_PATH.read_bytes()
    if mutation == "name":
        source = source.replace(b"test_max_scaled_xor_quantity_uses_the_155_character_boundary", b"test_substituted")
    elif mutation == "parameter":
        source = source.replace(b'"pricePerGibMicro",', b'"wrong",')
    elif mutation == "profile":
        source = source.replace(b'"bundle_heterogeneous_positive",', b'"wrong",')
    elif mutation == "duplicate":
        source += b"\ndef test_substituted():\n    pass\n"
    elif mutation == "mutable":
        source = bytearray(source)
    else:
        source = b""
    with pytest.raises(ValueError):
        cases.expected_node_ids(source)


def phase_stream(observer, *, mutation=None):
    items = [SimpleNamespace(nodeid=node, iter_markers=lambda name: ()) for node in observer.expected]
    observer.pytest_collection_modifyitems(None, None, items)
    for index, node in enumerate(observer.expected):
        observer.pytest_runtest_logstart(node, None)
        for phase in ("setup", "call", "teardown"):
            row = SimpleNamespace(nodeid=node, when=phase, outcome="passed")
            if index == 1 and phase == "call" and mutation:
                if mutation == "missing":
                    continue
                if mutation == "xfail":
                    row.wasxfail = "synthetic unexpected pass"
                elif mutation == "wrong-node":
                    row.nodeid = observer.expected[0]
                elif mutation == "phase-order":
                    row.when = "teardown"
                elif mutation in ("failed", "skipped"):
                    row.outcome = mutation
                elif mutation == "duplicate":
                    observer.pytest_runtest_logreport(row)
            observer.pytest_runtest_logreport(row)
        observer.pytest_runtest_logfinish(node, None)


def test_phase_owner_accepts_complete_exact_observations() -> None:
    observer = runner.CaseObserver(expected())
    phase_stream(observer)
    result = observer.finish(0)
    assert len(result) == 77
    assert sum(len(row["phases"]) for row in result) == 231


@pytest.mark.parametrize("mutation", ["missing", "xfail", "wrong-node", "phase-order", "failed", "skipped", "duplicate"])
def test_phase_owner_rejects_incomplete_or_substituted_execution(mutation: str) -> None:
    observer = runner.CaseObserver(expected())
    phase_stream(observer, mutation=mutation)
    with pytest.raises(runner.QualificationError, match="execution refused"):
        observer.finish(0)


@pytest.mark.parametrize("mutation", ["inventory", "marker", "deselected", "collection", "exit", "duplicate-collection", "unfinished"])
def test_collection_session_and_exit_are_required(mutation: str) -> None:
    observer = runner.CaseObserver(expected())
    phase_stream(observer)
    if mutation == "inventory":
        observer.pytest_collection_modifyitems(None, None, [])
    elif mutation == "marker":
        observer.pytest_collection_modifyitems(None, None, [SimpleNamespace(nodeid=node, iter_markers=lambda name: (object(),)) for node in expected()])
    elif mutation == "deselected":
        observer.pytest_deselected([object()])
    elif mutation == "collection":
        observer.pytest_collectreport(SimpleNamespace(outcome="skipped"))
    elif mutation == "duplicate-collection":
        observer.pytest_collection_modifyitems(None, None, [SimpleNamespace(nodeid=node, iter_markers=lambda name: ()) for node in expected()])
    elif mutation == "unfinished":
        observer.current = {"nodeid": expected()[0], "phases": []}
    with pytest.raises(runner.QualificationError):
        observer.finish(1 if mutation == "exit" else 0)


def make_input(tmp_path: Path):
    snapshot, env = tmp_path / "snapshot", tmp_path / "env"
    snapshot.mkdir(); env.mkdir()
    paths = [runner.RUNNER, runner.VERIFIER, runner.CASES, runner.TEST,
             "fixtures/sorafs_manifest/control.bin", "python/norito_py/src/norito/__init__.py",
             "python/iroha_torii_client/__init__.py"]
    rows = []
    for name in paths:
        path = snapshot / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"# synthetic source\n")
        rows.append({"path": name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": path.stat().st_size})
    wheel = tmp_path / "native.whl"; wheel.write_bytes(b"synthetic")
    sdk = tmp_path / "sdk.whl"; sdk.write_bytes(b"synthetic")
    python = Path(sys.executable).resolve()
    return {"schema": runner.INPUT_SCHEMA, "snapshot_root": str(snapshot), "environment_root": str(env),
            "native_wheel": {"path": str(wheel), "seal": "synthetic-not-admission"},
            "sdk_wheel": {"path": str(sdk), "seal": "synthetic-not-admission"},
            "source_files": sorted(rows, key=lambda row: row["path"]),
            "python": {"path": str(python), "sha256": hashlib.sha256(python.read_bytes()).hexdigest(), "size": python.stat().st_size}}


def test_closed_input_and_complete_source_owner(tmp_path: Path) -> None:
    inputs = make_input(tmp_path)
    assert runner.parse_input(runner.canonical_json(inputs)) == inputs
    contents, seals = runner.capture_sources(inputs)
    assert len(contents) == 7 and all(name in seals for name in contents)
    assert "d:." in seals
    source = Path(inputs["snapshot_root"]) / runner.TEST
    source.write_bytes(contents[runner.TEST])
    assert runner.capture_sources(inputs)[1] != seals  # identical-byte replacement still changes owner
    (source.parent / "unowned.py").write_bytes(b"unowned source must not execute")
    with pytest.raises(runner.QualificationError, match="inventory differs"):
        runner.capture_sources(inputs)


@pytest.mark.parametrize("name", ["python/__init__.py", "scripts/__init__.py",
                                   "python/iroha_python/__init__.py",
                                   "python/iroha_python/tests/__init__.py",
                                   "python/iroha_python/tests/conftest.py",
                                   "python/norito_py/src/norito/extra.py"])
def test_snapshot_rejects_every_unrecorded_package_ancestor_and_sibling(tmp_path: Path, name: str) -> None:
    inputs = make_input(tmp_path)
    runner.capture_sources(inputs)
    path = Path(inputs["snapshot_root"]) / name
    path.write_bytes(b"raise AssertionError('unrecorded source executed')\n")
    with pytest.raises(runner.QualificationError, match="snapshot inventory differs"):
        runner.capture_sources(inputs)


def test_snapshot_directories_have_retained_identity_and_closed_inventory(tmp_path: Path) -> None:
    inputs = make_input(tmp_path)
    root = Path(inputs["snapshot_root"])
    _contents, before = runner.capture_sources(inputs)
    directory = root / "fixtures/sorafs_manifest"
    os.chmod(directory, 0o700)
    _contents, after = runner.capture_sources(inputs)
    assert after["d:fixtures/sorafs_manifest"] != before["d:fixtures/sorafs_manifest"]
    (root / "foreign_empty_directory").mkdir()
    with pytest.raises(runner.QualificationError, match="unowned"):
        runner.capture_sources(inputs)


@pytest.mark.parametrize("mutation", ["unknown", "bool", "duplicate", "missing", "foreign", "limit", "alias", "noncanonical"])
def test_input_schema_refuses_unowned_fields_and_bounds(tmp_path: Path, mutation: str) -> None:
    value = make_input(tmp_path)
    if mutation == "unknown": value["command"] = "ignored success"
    elif mutation == "bool": value["source_files"][0]["size"] = True
    elif mutation == "duplicate": value["source_files"].append(value["source_files"][0])
    elif mutation == "missing": value["source_files"] = value["source_files"][1:]
    elif mutation == "foreign": value["source_files"][0]["path"] = "arbitrary/command.py"
    elif mutation == "limit": value["source_files"][0]["size"] = runner.MAX_FILE_BYTES + 1
    elif mutation == "alias": value["source_files"][0]["path"] = "ci/../ci/verify_privacy_python_wheel.py"
    raw = runner.canonical_json(value)
    if mutation == "noncanonical": raw += b" "
    with pytest.raises(runner.QualificationError): runner.parse_input(raw)


def test_stable_reads_reject_links_and_exact_size_overrun(tmp_path: Path) -> None:
    path = tmp_path / "source"; path.write_bytes(b"original")
    raw, seal = runner.read_stable(path, limit=8)
    assert raw == b"original" and seal[-1] == hashlib.sha256(raw).hexdigest()
    with pytest.raises(runner.QualificationError, match="size"): runner.read_stable(path, limit=7)
    alias = tmp_path / "alias"; alias.symlink_to(path)
    with pytest.raises(runner.QualificationError): runner.read_stable(alias)
    alias.unlink(); os.link(path, alias)
    with pytest.raises(runner.QualificationError, match="singly linked"): runner.read_stable(path)


@pytest.mark.parametrize("kind", ["fifo", "directory", "symlink"])
def test_open_race_rejects_actual_special_files_without_blocking(tmp_path: Path, monkeypatch, kind: str) -> None:
    path = tmp_path / "source"; path.write_bytes(b"original")
    other = tmp_path / "other"; other.write_bytes(b"substitute")
    original_open = os.open
    def replaced_open(target, flags, *args, **kwargs):
        assert flags & os.O_NONBLOCK and flags & os.O_CLOEXEC and flags & os.O_NOFOLLOW
        path.unlink()
        if kind == "fifo": os.mkfifo(path)
        elif kind == "directory": path.mkdir()
        else: path.symlink_to(other)
        return original_open(target, flags, *args, **kwargs)
    monkeypatch.setattr(runner.os, "open", replaced_open)
    with pytest.raises((runner.QualificationError, OSError)):
        runner.read_stable(path)


def test_bounded_output_refusal_remains_sticky(monkeypatch) -> None:
    monkeypatch.setattr(runner, "MAX_LOG", 4)
    log = runner.BoundedLog()
    assert log.write("éé") == 2 and log.payload == "éé".encode()
    with pytest.raises(runner.QualificationError): log.write("x")
    assert log.exceeded and len(log.payload) == 4
    assert not log.isatty()


def test_retained_module_identity_not_merely_equivalent_spec(tmp_path: Path) -> None:
    path = tmp_path / "owned.py"; path.write_bytes(b"VALUE=1\n")
    module = load("owned_module_control", path)
    before = {module.__name__: runner.module_state(module)}
    runner.require_retained_modules(before, {module.__name__: runner.module_state(module)})
    module.__spec__ = copy.copy(module.__spec__)
    with pytest.raises(runner.QualificationError, match="owner changed"):
        runner.require_retained_modules(before, {module.__name__: runner.module_state(module)})
    module.__spec__ = before[module.__name__][1]
    module.__spec__.loader.path = str(tmp_path / "substituted.py")
    with pytest.raises(runner.QualificationError, match="owner changed"):
        runner.require_retained_modules(before, {module.__name__: runner.module_state(module)})
    sys.modules.pop(module.__name__)


def run_synthetic_pytest(root: Path):
    source = ast.parse(TEST_PATH.read_bytes())
    functions = [node for node in source.body if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")]
    profile_names = cases.PARAMETER_IDS["test_fixture_bundle_matches_release_wide_outcomes_byte_for_byte"]
    synthetic = "from __future__ import annotations\nimport pytest\nfrom types import SimpleNamespace\nMARKER=SimpleNamespace(value='original')\n"
    synthetic += "_REFERENCE_SDK_BUNDLE_PROFILES=" + repr(tuple((name, 0, ()) for name in profile_names)) + "\n"
    for node in functions:
        node.body = ast.parse("assert True").body
        if node.name == "test_validate_governance_log_node_fails_closed_without_native_function":
            node.body = ast.parse("monkeypatch.setattr(MARKER, 'value', 'temporary')\nassert MARKER.value == 'temporary'").body
        if node.name == "test_max_scaled_xor_quantity_uses_the_155_character_boundary":
            node.body = ast.parse("print(\"synthetic observed stdout\")\nassert True").body
        synthetic += ast.unparse(node) + "\n"
    path = root / cases.TEST_PATH
    path.parent.mkdir(parents=True); path.write_text(synthetic)
    observer = runner.CaseObserver(expected())
    machinery_before = runner.machinery()
    log = runner.BoundedLog()
    with redirect_stdout(log), redirect_stderr(log):
        code = pytest.main(["-s", "--noconftest", "--assert=plain", "--import-mode=importlib", "-p", "no:cacheprovider", "-p", "no:terminal", "-c", os.devnull, "--rootdir", str(root), str(path)], plugins=[observer])
    assert b"synthetic observed stdout\n" in log.payload and not log.exceeded
    rows = observer.finish(int(code))
    assert runner.machinery() == machinery_before
    loaded = [module for module in sys.modules.values() if getattr(module, "__file__", None) == str(path)]
    assert len(loaded) == 1 and loaded[0].MARKER.value == "original"
    return {"scope": "synthetic pytest bodies only", "cases": len(rows), "phases": sum(len(row["phases"]) for row in rows), "max_node_bytes": max(len(row["nodeid"].encode()) for row in rows)}


def run_original_harness(root: Path):
    harness = load("owned_original_wheel_harness", ROOT / "scripts/tests/python_wheel_byte_owner_test.py")
    shell = ROOT / "ci/privacy_sdk_cargo_lockfile_test.sh"
    verifier_path = ROOT / "ci/verify_privacy_python_wheel.py"
    original = {"__name__": "__main__", "__file__": str(shell)}
    sys.argv = [str(shell), str(verifier_path), str(root)]
    body = harness.extract_original_harness(shell.read_bytes())
    exec(compile(body, str(shell) + ":wheel-verifier-heredoc", "exec"), original)
    verifier = original["verifier"]
    verifier.load_from_trusted_specs(wheel=original["wheel"], layout=original["layout"],
        package_spec=original["package_spec"], native_spec=original["inert_native_spec"],
        dependencies=original["dependencies"], sdk_wheel=original["sdk_wheel"],
        sdk_layout=original["sdk_layout"], sdk_spec=original["sdk_spec"])
    wheel, layout = original["sdk_wheel"], original["sdk_layout"]
    owners = [(wheel.owner.package, layout.site_root, {member.name: (member.sha256, member.size) for member in wheel.package_members}, None)]
    rows, retained = runner.loaded_members(verifier, owners)
    assert rows[0][0]["name"] == "iroha_python"
    module = sys.modules["iroha_python"]
    saved = list(module.__path__); module.__path__.append(str(root))
    try:
        with pytest.raises(runner.QualificationError, match="search path"):
            runner.loaded_members(verifier, owners)
    finally:
        module.__path__[:] = saved
    runner.require_retained_modules(retained, runner.loaded_members(verifier, owners)[1])
    native_wheel, native_layout = original["wheel"], original["layout"]
    native_owners = [(native_wheel.owner.package, native_layout.site_root,
                      {member.name: (member.sha256, member.size) for member in native_wheel.package_members},
                      "iroha_native._crypto")]
    with pytest.raises(runner.QualificationError, match="unexpected loader"):
        runner.loaded_members(verifier, native_owners)  # inert subclass is never a production native loader
    return {"scope": "original inert wheel harness and source-owner controls only",
            "harness_sha256": hashlib.sha256(body.encode()).hexdigest()}


@pytest.mark.parametrize("mode", ["synthetic", "harness"])
def test_isolated_actual_pytest_and_original_archive_owners(tmp_path: Path, mode: str) -> None:
    environment = {**os.environ, "PYTEST_DISABLE_PLUGIN_AUTOLOAD": "1", "PYTHONDONTWRITEBYTECODE": "1"}
    environment.pop("PYTEST_CURRENT_TEST", None)
    result = subprocess.run([sys.executable, "-I", "-B", str(Path(__file__).resolve()), "--run", mode, str(tmp_path / mode)],
                            env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    (tmp_path / "stdout.log").write_bytes(result.stdout)
    (tmp_path / "stderr.log").write_bytes(result.stderr)
    assert result.returncode == 0, result.stdout.decode() + result.stderr.decode()
    assert b"SYNTHETIC_CHILD_CONTROL=" in result.stdout
    report = json.loads(result.stdout.split(b"SYNTHETIC_CHILD_CONTROL=")[-1])
    if mode == "synthetic":
        assert report["cases"] == 77 and report["phases"] == 231 and report["max_node_bytes"] > 10_000
    else:
        assert "inert" in report["scope"]


if __name__ == "__main__":
    if len(sys.argv) != 4 or sys.argv[1] != "--run":
        raise SystemExit("synthetic controls require --run synthetic|harness TARGET_DIRECTORY")
    operation, root = sys.argv[2], Path(sys.argv[3])
    report = run_synthetic_pytest(root) if operation == "synthetic" else run_original_harness(root)
    print("SYNTHETIC_CHILD_CONTROL=" + json.dumps(report, sort_keys=True))
