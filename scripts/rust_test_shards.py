#!/usr/bin/env python3
"""Run complete native Cargo test inventories in bounded, disjoint CI shards.

Called by rust_ci.py with Python 3.11+, Git and the pinned native Rust toolchain.
Cargo owns compilation, target selection, runtime environment and working directory;
this module only supplies exact libtest filters. It reuses the ordinary target and
writes receipts below RUNNER_TEMP/iroha-native-test-receipts in CI, or
target/ci/native-tests locally. GITHUB_RUN_ID/GITHUB_RUN_ATTEMPT, when
present, bind receipts to the current run. No tests or deadlines are changed.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _sha(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _text(root: Path, *command: str) -> str:
    return subprocess.check_output(command, cwd=root, text=True).strip()


def _source(root: Path) -> dict:
    _require(not _text(root, "git", "status", "--porcelain", "--untracked-files=no"),
             "native test receipts require an unchanged tracked source checkout")
    return {
        "commit": _text(root, "git", "rev-parse", "HEAD"),
        "workspace_manifest": _sha(root / "Cargo.toml"),
        "lock": _sha(root / "Cargo.lock"),
        "run_id": os.environ.get("GITHUB_RUN_ID", "local"),
        "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "local"),
    }


def _cargo(root: Path, command: list[str], *, listing: bool = False) -> tuple[list[dict], list[str]]:
    """Keep Cargo JSON separate from actual libtest output, without buffering test logs."""
    print("+ " + " ".join(command), flush=True)
    messages, lines = [], []
    with subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, text=True) as child:
        assert child.stdout is not None
        for line in child.stdout:
            if line.startswith('{"reason":'):
                message = json.loads(line)
                _require(isinstance(message, dict), "invalid Cargo message")
                messages.append(message)
                if message.get("reason") == "compiler-message":
                    print(message.get("message", {}).get("rendered", ""), end="", flush=True)
            elif listing:
                lines.append(line.rstrip("\n"))
            else:
                print(line, end="", flush=True)
        _require(child.wait() == 0, "Cargo/native test command failed")
    finished = [value for value in messages if value.get("reason") == "build-finished"]
    _require(len(finished) == 1 and finished[0].get("success") is True,
             "Cargo did not report one successful build")
    return messages, lines


def _key(target: dict) -> str:
    kind = target.get("kind")
    _require(kind in (["lib"], ["bin"], ["test"], ["example"]),
             f"unsupported native test target kind: {kind}")
    name = target.get("name")
    _require(isinstance(name, str) and name and not name.startswith("-")
             and all(char.isalnum() or char in "_-" for char in name), "invalid Cargo target name")
    return kind[0] + ":" + name


def _artifacts(messages: list[dict], package_id: str, root: Path) -> dict[str, dict]:
    artifacts = {}
    for message in messages:
        if (message.get("reason") != "compiler-artifact"
                or message.get("package_id") != package_id
                or message.get("profile", {}).get("test") is not True):
            continue
        _require(message.get("executable") is not None, "test artifact has no native executable")
        target = message["target"]
        key = _key(target)
        _require(key not in artifacts, "duplicate Cargo test artifact")
        executable = Path(message["executable"])
        _require(executable.is_absolute() and executable.is_file(), "missing Cargo-selected executable")
        source = Path(target["src_path"]).resolve().relative_to(root.resolve()).as_posix()
        artifacts[key] = {
            "target": {"key": key, "source": source, "crate_types": target["crate_types"],
                       "features": sorted(message["features"]), "profile": message["profile"]},
            "executable": str(executable), "sha256": _sha(executable),
        }
    return artifacts


def _expected(package: dict, features: set[str], test_target: str | None = None) -> set[str]:
    """Cargo metadata supplies auto-discovered targets; the manifest supplies harness=false."""
    import tomllib
    with Path(package["manifest_path"]).open("rb") as source:
        manifest = tomllib.load(source)
    expected = set()
    for target in package["targets"]:
        if test_target is not None and (target["kind"] != ["test"] or target["name"] != test_target):
            continue
        if not target["test"] or not set(target.get("required-features", ())) <= features:
            continue
        key = _key(target)
        kind, name = key.split(":", 1)
        entries = [manifest.get("lib", {})] if kind == "lib" else manifest.get(kind, [])
        for entry in entries:
            explicit_path = entry.get("path")
            same_path = (explicit_path is not None and
                         (Path(package["manifest_path"]).parent / explicit_path).resolve()
                         == Path(target["src_path"]).resolve())
            if kind == "lib" or entry.get("name") == name or same_path:
                _require(entry.get("harness", True) is True, "custom test harnesses require explicit coverage")
        _require(key not in expected, "duplicate metadata test target")
        expected.add(key)
    _require(bool(expected), "package has no default-enabled native test targets")
    return expected


def parse_list(lines: list[str]) -> list[str]:
    """Admit only stable libtest terse test records, including an empty target."""
    names = []
    for line in lines:
        _require(line.endswith(": test"), f"unrecognized libtest inventory record: {line!r}")
        name = line[:-6]
        _require(name and not name.startswith("-") and not any(char.isspace() for char in name),
                 "invalid libtest name")
        _require(name not in names, "duplicate libtest name")
        names.append(name)
    return sorted(names)


def partition(target: str, names: list[str], count: int) -> list[list[str]]:
    """Partition all exact names without relying on a maintained source whitelist."""
    _require(count > 0 and names == sorted(set(names)), "invalid native inventory")
    groups = [[] for _ in range(count)]
    for name in names:
        digest = hashlib.sha256((target + "\0" + name).encode()).digest()
        groups[int.from_bytes(digest[:8], "big") % count].append(name)
    flattened = [name for group in groups for name in group]
    _require(sorted(flattened) == names and len(flattened) == len(set(flattened)),
             "native shard partition lost or duplicated a test")
    return groups


def _selector(key: str) -> list[str]:
    kind, name = key.split(":", 1)
    return ["--lib"] if kind == "lib" else ["--" + kind, name]


def _batches(names: list[str]) -> list[list[str]]:
    batches, current, size = [], [], 0
    for name in names:
        amount = len(name.encode()) + 1
        _require(amount < 48 * 1024, "native test name exceeds command-line budget")
        if size + amount >= 48 * 1024:
            batches.append(current)
            current, size = [], 0
        current.append(name)
        size += amount
    if current:
        batches.append(current)
    return batches


def parse_status(path: Path, selected: list[str], ignored: set[str]) -> dict[str, str]:
    statuses = {}
    for line in path.read_text().splitlines():
        if line.startswith("ignored: "):
            # Stable libtest places an explicit ignore reason before the final name.
            matches = [name for name in ignored if line.endswith(" " + name)]
            _require(len(matches) == 1, "ambiguous ignored native test status")
            status, separator, name = "ignored", " ", matches[0]
        else:
            status, separator, name = line.partition(" ")
        _require(separator == " " and status in {"ok", "ignored"} and name in selected,
                 "unexpected or failed native test status")
        _require(name not in statuses, "duplicate native test execution")
        _require((status == "ignored") == (name in ignored), "native ignored status changed")
        statuses[name] = status
    _require(set(statuses) == set(selected), "native execution omitted selected tests")
    return statuses


def run_shard(root: Path, package_name: str, index: int, count: int, *,
              test_target: str | None = None, cargo_test_command: list[str] | None = None,
              dry_run: bool = False) -> None:
    _require(1 <= index <= count, "native shard index is outside its declared range")
    base = [*(cargo_test_command or ["cargo", "test", "--locked", "--no-fail-fast", "-p", package_name]),
            "--message-format=json"]
    if test_target is not None:
        base += ["--test", test_target]
    if dry_run:
        print("+ " + " ".join([*base, "--no-run"]), flush=True)
        print(f"+ enumerate every compiled harness and run exact shard {index}/{count}", flush=True)
        return
    source = _source(root)
    metadata = json.loads(_text(root, "cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"))
    packages = [item for item in metadata["packages"] if item["name"] == package_name
                and item["id"] in metadata["workspace_members"]]
    _require(len(packages) == 1, "native package selection is ambiguous")
    package = packages[0]
    messages, _ = _cargo(root, [*base, "--no-run"])
    artifacts = _artifacts(messages, package["id"], root)
    feature_sets = {tuple(item["target"]["features"]) for item in artifacts.values()}
    _require(len(feature_sets) == 1, "native target feature selections disagree")
    _require(set(artifacts) == _expected(package, set(next(iter(feature_sets))), test_target),
             "Cargo native artifact inventory differs from enabled metadata targets")
    output = (Path(os.environ["RUNNER_TEMP"]) / "iroha-native-test-receipts"
              if "RUNNER_TEMP" in os.environ else root / "target" / "ci" / "native-tests")
    output.mkdir(parents=True, exist_ok=True)
    owner = package_name + (":" + test_target if test_target else "")
    stem = owner.replace(":", "-")
    receipt_path = output / f"{stem}-{index}.json"
    _require(not receipt_path.exists(), "native shard receipt already exists")
    inventory, executions = [], []
    # Admit every target and its complete/ignored inventory before executing any shard.
    for key, artifact in sorted(artifacts.items()):
        lists = []
        for ignored_only in (False, True):
            arguments = ["--list", "--format", "terse", *(["--ignored"] if ignored_only else [])]
            selector = [] if test_target else _selector(key)
            messages, lines = _cargo(root, [*base, *selector, "--", *arguments], listing=True)
            _require(_artifacts(messages, package["id"], root) == {key: artifact},
                     "Cargo changed the selected native listing artifact")
            lists.append(parse_list(lines))
        names, ignored = lists
        _require(set(ignored) <= set(names), "ignored inventory is not part of complete inventory")
        groups = partition(key, names, count)
        inventory.append({**artifact["target"], "tests": names, "ignored": ignored, "shards": groups})
    for target in inventory:
        key = target["key"]
        artifact = artifacts[key]
        selected = target["shards"][index - 1]
        statuses = {}
        # An actually empty target still receives its ordinary zero-test launch once.
        # An empty partition of a nonempty target must never launch without filters.
        batches = [[]] if not target["tests"] and index == 1 else _batches(selected)
        for batch_index, batch in enumerate(batches):
            status_path = output / f"{stem}-{index}-{key.replace(':', '-')}-{batch_index}.log"
            _require(not status_path.exists(), "native test status log already exists")
            selector = [] if test_target else _selector(key)
            serial = ["--test-threads=1"] if test_target else []
            messages, _ = _cargo(root, [*base, *selector, "--", "--exact", *batch, *serial,
                                       "--logfile", str(status_path)])
            _require(_artifacts(messages, package["id"], root) == {key: artifact},
                     "Cargo changed the executed native artifact")
            statuses.update(parse_status(status_path, batch, set(target["ignored"])))
        executions.append({"key": key, "statuses": statuses, "executable_sha256": artifact["sha256"]})
    _require(_source(root) == source, "native source changed during execution")
    receipt = {"version": 1, "source": source, "owner": owner, "package": package_name,
               "test_target": test_target, "index": index,
               "count": count, "toolchain": _text(root, "rustc", "-Vv"),
               "manifest": _sha(Path(package["manifest_path"])),
               "inventory": inventory, "executions": executions}
    receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(f"Native shard {package_name} {index}/{count} completed with audited exact coverage.")


def verify_receipts(root: Path, directory: Path, matrices: list[dict], declarations: dict) -> None:
    """Require every selected shard, common source/inventory and exactly-once outcomes."""
    rows = [row for matrix in matrices for row in matrix["include"]]
    for row in rows:
        if "native_shard" in row:
            owner = row["packages"] + (":" + row["test_target"] if row.get("test_target") else "")
            _require(owner in declarations and type(row["native_shard"]) is int,
                     "unknown native shard matrix owner")
    expected = {}
    for owner, (_, count) in declarations.items():
        package, _, test_target = owner.partition(":")
        package_rows = [row for row in rows if package in row["packages"].split(",")]
        selected = [row for row in package_rows if not test_target or row.get("test_target") == test_target]
        if not selected:
            continue
        owners = [row for row in selected if "native_shard" not in row]
        shards = [row for row in selected if "native_shard" in row]
        if test_target:
            owners = [row for row in package_rows if row.get("test_target") == "library"]
            _require(len(owners) == 1 and owners[0]["packages"] == package
                     and owners[0].get("checks", "clippy,build,test,doc") == "clippy,build,test,doc",
                     "native integration target lost its library/check owner")
        else:
            _require(len(owners) == 1 and owners[0]["packages"] == package
                     and owners[0].get("checks") == "clippy,build,doctest,doc",
                     "native package lost its non-native check owner")
        _require(len(shards) == count and {row["native_shard"] for row in shards} == set(range(1, count + 1))
                 and all(row["packages"] == package and row["checks"] == "test" for row in shards),
                 "native matrix has missing, duplicate or foreign shards")
        expected[owner] = count
    source = _source(root)
    receipts = [json.loads(path.read_text()) for path in sorted(directory.rglob("*.json"))]
    _require(len(receipts) == sum(expected.values()), "native receipt count differs from selected matrix")
    seen = set()
    canonical = {}
    completed = {}
    for receipt in receipts:
        owner, index = receipt.get("owner"), receipt.get("index")
        package, _, test_target = owner.partition(":") if isinstance(owner, str) else ("", "", "")
        _require(receipt.get("version") == 1 and owner in expected and type(index) is int
                 and receipt.get("package") == package and receipt.get("test_target") == (test_target or None)
                 and 1 <= index <= expected[owner] and receipt.get("count") == expected[owner]
                 and receipt.get("source") == source, "foreign or malformed native shard receipt")
        _require((owner, index) not in seen, "duplicate native shard receipt")
        seen.add((owner, index))
        binding = {key: receipt[key] for key in ("toolchain", "manifest", "inventory")}
        _require(owner not in canonical or canonical[owner] == binding,
                 "native shard source/features/profile/inventory disagree")
        canonical[owner] = binding
        targets = {target["key"]: target for target in receipt["inventory"]}
        _require(len(targets) == len(receipt["inventory"]) and bool(targets), "duplicate or empty native target inventory")
        executions = receipt["executions"]
        _require(len(executions) == len(targets) and {item["key"] for item in executions} == set(targets),
                 "native receipt omitted or duplicated a target")
        for execution in executions:
            target = targets[execution["key"]]
            groups = partition(target["key"], target["tests"], expected[owner])
            _require(target["shards"] == groups and set(target["ignored"]) <= set(target["tests"]),
                     "native receipt changed its complete partition")
            statuses = execution["statuses"]
            _require(set(statuses) == set(groups[index - 1]), "native receipt omitted or added executed tests")
            for name, status in statuses.items():
                _require(status == ("ignored" if name in target["ignored"] else "ok"),
                         "native receipt changed a result or ignored status")
                identity = (owner, target["key"], name)
                _require(identity not in completed, "native test executed in multiple shards")
                completed[identity] = status
    wanted = {(package, target["key"], name) for package, binding in canonical.items()
              for target in binding["inventory"] for name in target["tests"]}
    _require(set(completed) == wanted and len(seen) == sum(expected.values()),
             "native shard union is incomplete")
    print(f"Verified {len(seen)} native shards and {len(completed)} exact test outcomes.")
