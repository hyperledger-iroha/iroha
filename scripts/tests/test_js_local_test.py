"""Test local JavaScript orchestration using inert children, never native artifacts."""
import json
from pathlib import Path
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_js_local as runner


@pytest.fixture
def recipe(tmp_path):
    root = tmp_path.resolve() / "repo"
    target = root / "target/cargo-fast/existing"
    target.mkdir(parents=True)
    config = {"python": "/tools/python3.12", "cargo": "/rust/bin/cargo",
              "rustc": "/rust/bin/rustc", "rustdoc": "/rust/bin/rustdoc",
              "node": "/tools/node", "clang": "/xcode/clang", "ld": "/xcode/ld",
              "codesign": "/usr/bin/codesign", "developer_dir": "/xcode", "sdk": "/xcode/sdk"}
    return root, target, config


def intercept_steps(monkeypatch, *, failure=None, during_tests=None):
    calls = []
    failures = {failure} if isinstance(failure, str) else set(failure or [])

    def step(command, *, root, environment, log):
        calls.append((log.name, command, environment))
        if log.name == "javascript-test.log":
            assert root.name == "iroha_js" and root.parent.name == "javascript"
        if log.name in failures:
            raise subprocess.CalledProcessError(23, command)
        if log.name == "native-build.log":
            artifact = Path(command[command.index("--output") + 1])
            artifact.mkdir(mode=0o700)
            (artifact / "producer-record.json").write_text("inert mocked orchestration record")
        if log.name == "javascript-test.log" and during_tests:
            during_tests(command, environment)

    monkeypatch.setattr(runner, "run_step", step)
    monkeypatch.setattr(runner, "prepare_compiler_cache", lambda *args: None)
    return calls


def test_complete_unit_profile_binds_fresh_artifact_and_rechecks_same_producer(monkeypatch, recipe):
    root, target, config = recipe
    calls = intercept_steps(monkeypatch)
    run = runner.run_suite(root, target, config)
    assert [name for name, _, _ in calls] == ["native-build.log", "verify-before.log",
                                           "javascript-test.log", "verify-after.log"]
    assert run.is_relative_to(root / "target/qualification")
    assert run.stat().st_mode & 0o777 == 0o700
    assert json.loads((run / "native-tools.json").read_text()) == config
    build, before, tests, after = [command for _, command, _ in calls]
    assert build == [config["python"], "-I", "-S", "-B", str(root / "scripts/native_js_local_unit.py"),
        "produce", "--root", str(root), "--target-dir", str(target), "--output", str(run / "artifact"),
        "--config", str(run / "native-tools.json"), "--acknowledge-local-unit-recipe"]
    assert before == after
    assert before[-2:] == ["--producer-sha256", runner.unit.digest(run / "artifact/producer-record.json")]
    assert tests == [config["node"], str(root / "javascript/iroha_js/scripts/run-test-profile.mjs"), "unit"]
    assert calls[2][2]["IROHA_JS_NATIVE_DIR"] == str(run / "artifact")
    assert all("IROHA_JS_NATIVE_DIR" not in calls[index][2] for index in (0, 1, 3))


@pytest.mark.parametrize("failure,expected", [
    ("native-build.log", ["native-build.log"]),
    ("verify-before.log", ["native-build.log", "verify-before.log"]),
    ("javascript-test.log", ["native-build.log", "verify-before.log", "javascript-test.log", "verify-after.log"]),
    ("verify-after.log", ["native-build.log", "verify-before.log", "javascript-test.log", "verify-after.log"]),
])
def test_failure_propagates_and_never_starts_unadmitted_tests(monkeypatch, recipe, failure, expected):
    calls = intercept_steps(monkeypatch, failure=failure)
    with pytest.raises(subprocess.CalledProcessError) as error:
        runner.run_suite(*recipe)
    assert error.value.returncode == 23
    assert [name for name, _, _ in calls] == expected


def test_test_and_verification_failures_are_both_reported(monkeypatch, recipe):
    calls = intercept_steps(monkeypatch, failure=["javascript-test.log", "verify-after.log"])
    with pytest.raises(RuntimeError, match="JavaScript tests failed.*post-test custody verification also failed") as error:
        runner.run_suite(*recipe)
    assert "javascript-test.log" in str(error.value) and "verify-after.log" in str(error.value)
    assert calls[-1][0] == "verify-after.log"


def test_failed_test_process_launch_still_reverifies_custody(monkeypatch, recipe):
    def launch_failure(_command, _environment):
        raise FileNotFoundError("test child missing")

    calls = intercept_steps(monkeypatch, during_tests=launch_failure)
    with pytest.raises(FileNotFoundError, match="test child missing"):
        runner.run_suite(*recipe)
    assert calls[-1][0] == "verify-after.log"


def test_post_test_check_never_repins_a_modified_producer(monkeypatch, recipe):
    def mutate_record(_command, environment):
        (Path(environment["IROHA_JS_NATIVE_DIR"]) / "producer-record.json").write_text("changed during tests")

    calls = intercept_steps(monkeypatch, during_tests=mutate_record)
    run = runner.run_suite(*recipe)
    assert calls[1][1] == calls[3][1]
    assert calls[3][1][-1] != runner.unit.digest(run / "artifact/producer-record.json")


def test_repeated_runs_preserve_existing_lane_and_evidence(monkeypatch, recipe):
    root, target, _ = recipe
    marker = target / "original-artifact"
    marker.write_bytes(b"existing native build bytes")
    calls = intercept_steps(monkeypatch)
    first, second = runner.run_suite(*recipe), runner.run_suite(*recipe)
    assert first != second and first.exists() and second.exists()
    assert marker.read_bytes() == b"existing native build bytes"
    assert calls[2][2]["IROHA_JS_NATIVE_DIR"] != calls[6][2]["IROHA_JS_NATIVE_DIR"]
    assert sorted(path.name for path in (root / "target/cargo-fast").iterdir()) == ["existing"]


@pytest.mark.parametrize("lane", ["outside", "target", "target/cargo-fast", "target/qualification", "relative", "missing"])
def test_invalid_or_missing_lane_never_creates_a_target(monkeypatch, recipe, lane):
    root, _, config = recipe
    target = Path("relative") if lane == "relative" else root / lane
    if lane == "outside":
        target.mkdir()
    calls = intercept_steps(monkeypatch)
    with pytest.raises(RuntimeError, match="Cargo lane must be an existing canonical child"):
        runner.run_suite(root, target, config)
    assert calls == []
    assert not (root / "target/qualification").exists()
    assert not (root / "missing").exists()


def test_symbolic_link_lane_is_not_admitted(monkeypatch, recipe):
    root, target, config = recipe
    alias = target.parent / "alias"
    alias.symlink_to(target, target_is_directory=True)
    calls = intercept_steps(monkeypatch)
    with pytest.raises(RuntimeError, match="Cargo lane"):
        runner.run_suite(root, alias, config)
    assert calls == []


def test_children_use_fixed_environment_without_inherited_overrides(monkeypatch, recipe):
    for key in ("NODE_OPTIONS", "CARGO_PROFILE_DEV_OPT_LEVEL", "RUSTC_WRAPPER", "GIT_INDEX_FILE", "CARGO_TARGET_DIR"):
        monkeypatch.setenv(key, "inherited override")
    monkeypatch.setenv("MACOSX_DEPLOYMENT_TARGET", "27.0")
    monkeypatch.setenv("SDKROOT", "/stale/sdk")
    calls = intercept_steps(monkeypatch)
    runner.run_suite(*recipe)
    for _, _, environment in calls:
        assert environment["NODE_OPTIONS"] == ""
        assert environment["SDKROOT"] == recipe[2]["sdk"]
        assert environment["MACOSX_DEPLOYMENT_TARGET"] == "13.0"
        assert environment["PATH"] == "/rust/bin:/opt/homebrew/bin:/usr/bin:/bin"
        assert not set(environment) & {"CARGO_PROFILE_DEV_OPT_LEVEL", "RUSTC_WRAPPER", "GIT_INDEX_FILE", "CARGO_TARGET_DIR"}


def test_step_preserves_diagnostics_and_never_times_out_or_signals(monkeypatch, tmp_path):
    calls = []

    def run(command, **kwargs):
        calls.append((command, kwargs))
        kwargs["stdout"].write(b"actual child failure")
        return subprocess.CompletedProcess(command, 19)

    monkeypatch.setattr(runner.subprocess, "run", run)
    log = tmp_path / "failure.log"
    with pytest.raises(subprocess.CalledProcessError) as error:
        runner.run_step(["child"], root=tmp_path, environment={}, log=log)
    assert error.value.returncode == 19
    assert log.read_bytes() == b"actual child failure"
    assert "timeout" not in calls[0][1] and calls[0][1]["check"] is False
    with pytest.raises(FileExistsError):
        runner.run_step(["child"], root=tmp_path, environment={}, log=log)
    assert len(calls) == 1


def test_compiler_cache_bootstrap_has_short_private_scratch_and_never_stops_daemon(monkeypatch, tmp_path, recipe):
    root, _, config = recipe
    sccache = tmp_path / "sccache"
    sccache.write_text("inert tool, never executed")
    monkeypatch.setattr(runner.shutil, "which", lambda name, *, path: str(sccache))
    calls = []

    def step(command, *, root, environment, log):
        scratch = Path(environment["TMPDIR"])
        calls.append((command, scratch, log))
        assert scratch.parent == Path("/tmp").resolve()
        assert scratch.resolve(strict=True) == scratch
        assert scratch.stat().st_mode & 0o777 == 0o700
        assert len(str(scratch).encode()) < 60
        assert environment["SCCACHE_CLIENT_SIDE"] == "0"

    monkeypatch.setattr(runner, "run_step", step)
    runner.prepare_compiler_cache(root, config, {}, tmp_path)
    assert calls[0][0] == [str(sccache.resolve()), config["rustc"], "-vV"]
    assert calls[0][2] == tmp_path / "compiler-cache.log"
    assert calls[0][1].exists()
    assert json.loads((tmp_path / "compiler-cache-scratch.json").read_text()) == {"path": str(calls[0][1])}
    calls[0][1].rmdir()


def test_absent_compiler_cache_needs_no_bootstrap(monkeypatch, tmp_path, recipe):
    monkeypatch.setattr(runner.shutil, "which", lambda name, *, path: None)
    monkeypatch.setattr(runner, "run_step", lambda *args, **kwargs: pytest.fail("no cache to probe"))
    runner.prepare_compiler_cache(recipe[0], recipe[2], {}, tmp_path)


def test_compiler_cache_failure_is_not_retried_or_hidden(monkeypatch, tmp_path, recipe):
    sccache = tmp_path / "sccache"
    sccache.write_text("inert tool, never executed")
    monkeypatch.setattr(runner.shutil, "which", lambda name, *, path: str(sccache))
    calls = []

    def fail(command, **kwargs):
        calls.append(command)
        raise subprocess.CalledProcessError(19, command)

    monkeypatch.setattr(runner, "run_step", fail)
    with pytest.raises(subprocess.CalledProcessError) as error:
        runner.prepare_compiler_cache(recipe[0], recipe[2], {}, tmp_path)
    assert error.value.returncode == 19 and len(calls) == 1


@pytest.mark.parametrize("environment", [{"IROHA_JS_NATIVE_DIR": "explicit"}, {"IROHA_JS_NATIVE_DIR": ""},
                                       {"IROHA_JS_NATIVE_BUILD_PROFILE": "release"}])
def test_explicit_artifact_or_release_selection_never_falls_back(monkeypatch, environment):
    monkeypatch.setattr(runner.sys, "platform", "darwin")
    monkeypatch.setattr(runner.platform, "machine", lambda: "arm64")
    monkeypatch.setattr(runner.sys, "version_info", (3, 12, 0))
    monkeypatch.setattr(runner.subprocess, "run", lambda *args, **kwargs: pytest.fail("must refuse before tool selection"))
    with pytest.raises(RuntimeError):
        runner.selected_tools(Path("/unused"), environment)


@pytest.mark.parametrize("platform_name,version", [("linux", (3, 12, 0)), ("darwin", (3, 11, 0))])
def test_host_and_interpreter_fail_before_tool_selection(monkeypatch, platform_name, version):
    monkeypatch.setattr(runner.sys, "platform", platform_name)
    monkeypatch.setattr(runner.platform, "machine", lambda: "arm64")
    monkeypatch.setattr(runner.sys, "version_info", version)
    monkeypatch.setattr(runner.subprocess, "run", lambda *args, **kwargs: pytest.fail("must refuse before tool selection"))
    with pytest.raises(RuntimeError):
        runner.selected_tools(Path("/unused"), {})


def test_tool_selection_uses_exact_producer_policy_and_canonical_node(monkeypatch, tmp_path):
    root = tmp_path.resolve()
    developer = root / "Xcode.app/Contents/Developer"
    apple = developer / "Toolchains/XcodeDefault.xctoolchain/usr/bin"
    apple.mkdir(parents=True)
    (developer / "usr/bin").mkdir(parents=True)
    for path in (apple / "clang", apple / "ld", developer / "usr/bin/xcodebuild"):
        path.write_text("inert tool, never executed")
    node = root / "node"
    node.write_text("inert node, never executed")
    alias = root / "node-alias"
    alias.symlink_to(node)
    monkeypatch.setattr(runner.sys, "platform", "darwin")
    monkeypatch.setattr(runner.platform, "machine", lambda: "arm64")
    monkeypatch.setattr(runner.sys, "version_info", (3, 12, 0))
    calls, policies = [], []

    def run(command, **kwargs):
        calls.append(command)
        return subprocess.CompletedProcess(command, 0, str(developer) + "\n")

    def which(name, *, path):
        assert name == "node"
        assert path.endswith("/.rustup/toolchains/1.93.1-aarch64-apple-darwin/bin:/opt/homebrew/bin:/usr/bin:/bin")
        return str(alias)

    monkeypatch.setattr(runner.subprocess, "run", run)
    monkeypatch.setattr(runner.shutil, "which", which)
    monkeypatch.setattr(runner.unit, "validate_config", lambda root, config: policies.append((root, config.copy())))
    config = runner.selected_tools(root, {"SDKROOT": "/stale/sdk"})
    assert calls == [["/usr/bin/xcode-select", "-p"]]
    assert set(config) == {"python", "cargo", "rustc", "rustdoc", "node", "clang", "ld", "codesign", "sdk", "developer_dir"}
    assert config["node"] == str(node)
    assert config["sdk"] == str(developer / "Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk")
    assert config["developer_dir"] == str(developer)
    assert policies == [(root, config)]


def test_command_line_tools_is_rejected_before_native_production(monkeypatch, tmp_path):
    developer = tmp_path.resolve() / "CommandLineTools"
    developer.mkdir()
    monkeypatch.setattr(runner.sys, "platform", "darwin")
    monkeypatch.setattr(runner.platform, "machine", lambda: "arm64")
    monkeypatch.setattr(runner.sys, "version_info", (3, 12, 0))
    monkeypatch.setattr(runner.subprocess, "run", lambda *args, **kwargs: pytest.fail("explicit selection needs no discovery"))
    with pytest.raises(RuntimeError, match="full Xcode is required"):
        runner.selected_tools(tmp_path, {"DEVELOPER_DIR": str(developer)})


@pytest.mark.parametrize("arguments", [[], ["--skip-build"], ["--filter", "connect"],
                                      ["--profile", "release"], ["--skip", "native"]])
def test_cli_requires_lane_and_has_no_partial_suite_escape(arguments):
    with pytest.raises(SystemExit) as error:
        runner.main(arguments)
    assert error.value.code == 2


def test_package_entry_and_documentation_expose_local_runner_without_changing_release_path():
    root = Path(__file__).resolve().parents[2]
    package = json.loads((root / "javascript/iroha_js/package.json").read_text())
    assert package["scripts"]["test:local"] == "python3.12 -E -s -B ../../scripts/test_js_local.py"
    assert package["scripts"]["test"] == "npm run build:native && node ./scripts/run-test-profile.mjs unit"
    assert "npm run test:local -- --target-dir" in (root / "javascript/iroha_js/README.md").read_text()
