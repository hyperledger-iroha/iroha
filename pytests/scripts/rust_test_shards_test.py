"""Actual Cargo/libtest controls and adversarial native shard coverage receipts."""

from __future__ import annotations

import copy
import importlib.util
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("native_shards", ROOT / "scripts/rust_test_shards.py")
assert SPEC and SPEC.loader
shards = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(shards)


@pytest.fixture
def native_package(tmp_path, monkeypatch):
    """Use real Cargo selection, runtime variables and stable Rust's real libtest."""
    monkeypatch.delenv("RUNNER_TEMP", raising=False)
    (tmp_path / "src").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "Cargo.toml").write_text('''
[package]
name = "native-shard-control"
version = "0.0.0"
edition = "2024"
[features]
unselected = []
[[test]]
name = "disabled"
required-features = ["unselected"]
''')
    (tmp_path / "src/lib.rs").write_text('''//! Native shard control.
#[test] fn alpha() { assert_eq!(std::env::var("CARGO_MANIFEST_DIR").unwrap(), env!("CARGO_MANIFEST_DIR")); }
#[test] fn alphabet() { assert_eq!(std::env::current_dir().unwrap(), std::path::Path::new(env!("CARGO_MANIFEST_DIR"))); }
#[test] #[ignore = "original ignored status"] fn ignored() { panic!("ignored test must not execute"); }
#[test] fn omega() {}
''')
    (tmp_path / "tests/route.rs").write_text('''//! Additional auto-discovered native target.
#[test] fn same_name() {}
#[test] fn prefix() {}
#[test] fn prefix_more() {}
''')
    (tmp_path / "tests/empty.rs").write_text("//! An empty default native target remains in the census.\n")
    (tmp_path / "tests/disabled.rs").write_text('compile_error!("disabled feature target must not build");\n')
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=tmp_path, check=True, capture_output=True)
    identity = {"commit": "current", "workspace_manifest": shards._sha(tmp_path / "Cargo.toml"),
                "lock": shards._sha(tmp_path / "Cargo.lock"), "run_id": "control", "run_attempt": "1"}
    monkeypatch.setattr(shards, "_source", lambda root: dict(identity))
    return tmp_path


def matrix(package="native-shard-control", count=2, target=None):
    if target:
        owner = {"packages": package, "test_target": "library"}
    else:
        owner = {"packages": package, "checks": "clippy,build,doctest,doc"}
    rows = [owner]
    for index in range(1, count + 1):
        row = {"packages": package, "checks": "test", "native_shard": index}
        if target:
            row["test_target"] = target
        rows.append(row)
    return [{"include": rows}]


def test_actual_cargo_complete_union_exact_filters_runtime_and_ignored(native_package):
    root = native_package
    for index in (1, 2):
        shards.run_shard(root, "native-shard-control", index, 2)
    directory = root / "target/ci/native-tests"
    shards.verify_receipts(root, directory, matrix(), {"native-shard-control": ("control", 2)})
    receipts = [json.loads(path.read_text()) for path in sorted(directory.glob("*.json"))]
    assert receipts[0]["inventory"] == receipts[1]["inventory"]
    targets = {target["key"]: target for target in receipts[0]["inventory"]}
    assert set(targets) == {"lib:native_shard_control", "test:route", "test:empty"}
    assert targets["test:empty"]["tests"] == []
    assert targets["lib:native_shard_control"]["ignored"] == ["ignored"]
    actual = {(execution["key"], name): status for receipt in receipts
              for execution in receipt["executions"] for name, status in execution["statuses"].items()}
    assert len(actual) == 7
    assert actual["lib:native_shard_control", "ignored"] == "ignored"
    assert actual["lib:native_shard_control", "alpha"] == "ok"
    assert actual["lib:native_shard_control", "alphabet"] == "ok"



def test_actual_core_router_retains_feature_gated_harnesses(native_package, monkeypatch):
    """The real router must compile and execute the previously unified Core targets."""
    spec = importlib.util.spec_from_file_location("native_core_router", ROOT / "scripts/rust_ci.py")
    assert spec and spec.loader
    router = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, spec.name, router)
    spec.loader.exec_module(router)
    monkeypatch.setattr(router, "NATIVE_SHARDS", shards)
    manifest = native_package / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace('name = "native-shard-control"', 'name = "iroha_core"')
                        .replace('unselected = []', 'unselected = []\niroha-core-tests = []\nexpensive-telemetry = []\ndev-tools = []\nnode = []\njson = []'))
    original = tomllib.loads((ROOT / "crates/iroha_core/Cargo.toml").read_text())
    names = {test["name"] for test in original["test"]
             if test["name"] == "private_dataspace_runtime" or test["name"].startswith("iroha_core_group_")}
    assert names == {"private_dataspace_runtime", *(f"iroha_core_group_{index:02}" for index in range(1, 6))}
    for name in sorted(names):
        with manifest.open("a") as output:
            output.write(f'\n[[test]]\nname = "{name}"\nrequired-features = ["iroha-core-tests"]\n')
        (native_package / "tests" / (name + ".rs")).write_text(
            '//! Required-feature harness.\n#[test] fn retained() { '
            'assert!(cfg!(feature = "iroha-core-tests")); assert!(cfg!(feature = "expensive-telemetry")); }\n')
    preflight_cases = {}
    for binary in original["bin"]:
        if binary["name"] not in {"ironwood_activation_preflight", "trigger_lifecycle_preflight"}:
            continue
        leaves = re.findall(r"#\[test\]\s*fn\s+(\w+)\s*\(",
                            (ROOT / "crates/iroha_core" / binary["path"]).read_text())
        assert leaves
        preflight_cases["bin:" + binary["name"]] = leaves
        required = binary["required-features"]
        assert "dev-tools" in required
        with manifest.open("a") as output:
            output.write(f'\n[[bin]]\nname = "{binary["name"]}"\npath = "{binary["path"]}"\n'
                         + 'required-features = ' + json.dumps(required) + '\n')
        source = native_package / binary["path"]
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text('//! Required-feature developer executable.\nfn main() {}\n' +
                          ''.join(f'#[test] fn {leaf}() {{ assert!(cfg!(feature = "dev-tools")); }}\n'
                                  for leaf in leaves))
    assert set(preflight_cases) == {"bin:ironwood_activation_preflight", "bin:trigger_lifecycle_preflight"}
    assert sum(map(len, preflight_cases.values())) == 5
    # The production Core defaults already enable node/json; keep that graph here too.
    manifest.write_text(manifest.read_text().replace('[features]\n', '[features]\ndefault = ["node", "json"]\n'))
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=native_package,
                   check=True, capture_output=True)
    for index in range(1, 5):
        router.run_checks(("iroha_core",), ("test",), root=native_package, native_shard=index)
    directory = native_package / "target/ci/native-tests"
    shards.verify_receipts(native_package, directory, matrix(package="iroha_core", count=4),
                           router.NATIVE_TEST_SHARDS)
    receipts = [json.loads(path.read_text()) for path in sorted(directory.glob("*.json"))]
    inventory = {target["key"]: target for target in receipts[0]["inventory"]}
    assert {"test:" + name for name in names} <= set(inventory)
    for target in inventory.values():
        assert {"iroha-core-tests", "expensive-telemetry", "dev-tools"} <= set(target["features"])
    executed = {(item["key"], name) for receipt in receipts for item in receipt["executions"]
                for name, status in item["statuses"].items() if status == "ok"}
    assert {("test:" + name, "retained") for name in names} <= executed
    for key, leaves in preflight_cases.items():
        assert set(inventory[key]["tests"]) == set(leaves)
        assert {(key, leaf) for leaf in leaves} <= executed


def test_actual_cargo_target_only_shards_retain_serial_harness(native_package, monkeypatch):
    calls = []
    original = shards._cargo
    def record(root, command, **kwargs):
        calls.append(command)
        return original(root, command, **kwargs)
    monkeypatch.setattr(shards, "_cargo", record)
    for index in (1, 2):
        shards.run_shard(native_package, "native-shard-control", index, 2, test_target="route")
    shards.verify_receipts(native_package, native_package / "target/ci/native-tests",
                           matrix(target="route"), {"native-shard-control:route": ("control", 2)})
    executed = [call for call in calls if "--exact" in call]
    assert executed and all("--test-threads=1" in call for call in executed)
    assert all(call.count("--test") == 1 and call[call.index("--test") + 1] == "route" for call in calls)


def test_custom_harness_is_rejected_before_any_execution(native_package):
    manifest = native_package / "Cargo.toml"
    manifest.write_text(manifest.read_text() + '\n[[test]]\nname="custom"\nharness=false\n')
    (native_package / "tests/custom.rs").write_text('fn main() { panic!("must never execute"); }\n')
    with pytest.raises(ValueError, match="custom test harness"):
        shards.run_shard(native_package, "native-shard-control", 1, 2)
    assert not (native_package / "target/ci/native-tests").exists()


def test_ci_receipts_use_uncached_runner_directory(native_package, monkeypatch):
    runner = native_package / "runner-private"
    runner.mkdir()
    monkeypatch.setenv("RUNNER_TEMP", str(runner))
    stale = native_package / "target/ci/native-tests/stale.json"
    stale.parent.mkdir(parents=True)
    stale.write_text("stale cached bytes")
    shards.run_shard(native_package, "native-shard-control", 1, 2, test_target="route")
    assert stale.read_text() == "stale cached bytes"
    assert len(list((runner / "iroha-native-test-receipts").glob("*.json"))) == 1
    workflow = (ROOT / ".github/workflows/pr.yml").read_text()
    assert workflow.count("path: ${{ runner.temp }}/iroha-native-test-receipts/*.json") == 2


@pytest.mark.parametrize("mutation", ["missing_artifact", "duplicate_artifact", "listing_features", "listing_text"])
def test_actual_cargo_artifact_and_inventory_substitution_refused(native_package, monkeypatch, mutation):
    original = shards._cargo
    def substitute(root, command, **kwargs):
        messages, lines = original(root, command, **kwargs)
        artifacts = [item for item in messages if item.get("reason") == "compiler-artifact"
                     and item.get("profile", {}).get("test") and item.get("executable")]
        if "--no-run" in command and mutation == "missing_artifact":
            messages.remove(artifacts[0])
        elif "--no-run" in command and mutation == "duplicate_artifact":
            messages.append(copy.deepcopy(artifacts[0]))
        elif kwargs.get("listing") and mutation == "listing_features":
            artifacts[0]["features"].append("substituted-feature")
        elif kwargs.get("listing") and mutation == "listing_text":
            lines.append("unknown custom harness output")
        return messages, lines
    monkeypatch.setattr(shards, "_cargo", substitute)
    with pytest.raises(ValueError):
        shards.run_shard(native_package, "native-shard-control", 1, 2)
    assert not list((native_package / "target/ci/native-tests").glob("*.json"))


def test_actual_native_test_failure_never_publishes_success_receipt(native_package):
    source = native_package / "src/lib.rs"
    source.write_text(source.read_text().replace("#[test] fn omega() {}", '#[test] fn omega() { panic!("native failure remains fatal"); }'))
    index = next(index for index, names in enumerate(shards.partition("lib:native_shard_control", ["omega"], 2), 1) if names)
    with pytest.raises(ValueError, match="Cargo/native test command failed"):
        shards.run_shard(native_package, "native-shard-control", index, 2)
    assert not list((native_package / "target/ci/native-tests").glob("*.json"))


@pytest.mark.parametrize("lines", [["alpha: benchmark"], ["alpha: test", "alpha: test"],
                                   ["--ignored: test"], ["arbitrary output"], ["a b: test"]])
def test_unknown_and_ambiguous_libtest_inventory_refused(lines):
    with pytest.raises(ValueError):
        shards.parse_list(lines)


@pytest.mark.parametrize("records", ["ok alpha\n", "ok alpha\nok alpha\nok beta\n",
                                     "ok alpha\nok beta\nok foreign\n", "ignored alpha\nok beta\n",
                                     "ok alpha\nfailed beta\n"])
def test_statuses_refuse_missing_duplicate_extra_ignored_or_failed(tmp_path, records):
    path = tmp_path / "native.log"
    path.write_text(records)
    with pytest.raises(ValueError):
        shards.parse_status(path, ["alpha", "beta"], set())


def receipts_fixture(tmp_path, monkeypatch):
    source = {"commit": "current"}
    monkeypatch.setattr(shards, "_source", lambda root: source)
    names = ["alpha", "alphabet", "ignored", "omega"]
    groups = shards.partition("lib:control", names, 2)
    target = {"key": "lib:control", "tests": names, "ignored": ["ignored"], "shards": groups,
              "profile": {"test": True, "opt_level": "0"}, "features": ["default"]}
    receipts = []
    for index in (1, 2):
        receipts.append({"version": 1, "source": source, "owner": "native-shard-control",
                         "package": "native-shard-control", "test_target": None, "index": index,
                         "count": 2, "toolchain": "pinned", "manifest": "manifest", "inventory": [copy.deepcopy(target)],
                         "executions": [{"key": "lib:control", "executable_sha256": "bytes",
                                         "statuses": {name: "ignored" if name == "ignored" else "ok" for name in groups[index - 1]}}]})
    return receipts


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "source", "features", "profile", "inventory",
                                      "extra_test", "omitted_test", "ignored", "foreign_package", "target", "partition"])
def test_aggregate_rejects_stale_incomplete_or_inconsistent_receipts(tmp_path, monkeypatch, mutation):
    receipts = receipts_fixture(tmp_path, monkeypatch)
    if mutation == "missing": receipts.pop()
    elif mutation == "duplicate": receipts[1] = copy.deepcopy(receipts[0])
    elif mutation == "source": receipts[0]["source"] = {"commit": "stale"}
    elif mutation == "features": receipts[0]["inventory"][0]["features"] = []
    elif mutation == "profile": receipts[0]["inventory"][0]["profile"]["opt_level"] = "3"
    elif mutation == "inventory": receipts[0]["inventory"][0]["tests"] = []
    elif mutation == "extra_test": receipts[0]["executions"][0]["statuses"]["foreign"] = "ok"
    elif mutation == "omitted_test": receipts[0]["executions"][0]["statuses"] = {}
    elif mutation == "ignored":
        for receipt in receipts:
            if "ignored" in receipt["executions"][0]["statuses"]:
                receipt["executions"][0]["statuses"]["ignored"] = "ok"
    elif mutation == "foreign_package": receipts[0]["package"] = "foreign"
    elif mutation == "target": receipts[0]["executions"].append(copy.deepcopy(receipts[0]["executions"][0]))
    elif mutation == "partition": receipts[0]["inventory"][0]["shards"] = [[], []]
    for index, receipt in enumerate(receipts):
        (tmp_path / f"{index}.json").write_text(json.dumps(receipt))
    with pytest.raises(ValueError):
        shards.verify_receipts(tmp_path, tmp_path, matrix(), {"native-shard-control": ("control", 2)})


@pytest.mark.parametrize("mutation", ["no_owner", "missing_shard", "duplicate_shard", "owner_omits_docs"])
def test_aggregate_requires_original_check_owner_and_every_matrix_shard(tmp_path, monkeypatch, mutation):
    receipts = receipts_fixture(tmp_path, monkeypatch)
    rows = matrix()
    if mutation == "no_owner": rows[0]["include"].pop(0)
    elif mutation == "missing_shard": rows[0]["include"].pop()
    elif mutation == "duplicate_shard": rows[0]["include"][-1]["native_shard"] = 1
    elif mutation == "owner_omits_docs": rows[0]["include"][0]["checks"] = "clippy,build,doc"
    for index, receipt in enumerate(receipts):
        (tmp_path / f"{index}.json").write_text(json.dumps(receipt))
    with pytest.raises(ValueError):
        shards.verify_receipts(tmp_path, tmp_path, rows, {"native-shard-control": ("control", 2)})
