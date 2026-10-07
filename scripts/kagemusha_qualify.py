#!/usr/bin/env python3
"""Build and qualify the native KAGEMUSHA M3 candidate on a shared macOS host.

Requires Python 3.10+, Cargo, the repository toolchain and macOS vm_stat/sysctl/
pmset. No signing inputs or network access are used. All output must go to an
untracked directory (normally target/qualification). No process is interrupted
or killed. RAYON_NUM_THREADS and M3_SEED are test-only controls of the child.

prepare builds the test executable and binds it to source/compiler provenance.
run executes the predeclared three-block schedule, retaining every raw attempt.
summarize can recompute verdicts without rerunning proofs. Qualification uses
observed values only; calibration never normalizes a failure into a pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import shlex
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CONFIGS = {
    f"{kind}-{workers}": {"kind": kind, "workers": workers, "test": test, "gate": gate}
    for kind, test, gate in (
        ("q_chips", "g3_6_q_leaf_proof", "G3.6/q_leaf_chips"),
        ("q_exact", "g3_6_q_exact_shape_proof", "G3.6/q_exact_shape"),
        ("a_chips", "g3_7_a_load_proof", "G3.7/a_imt_load"),
        ("a_exact", "g3_7_a_exact_shape_proof", "G3.7/a_exact_shape"),
    )
    for workers in (1, 4)
}
PREFIX = "M3_GATE_JSON "
GIB = 1 << 30
PROBES = {
    "pressure": ["sysctl", "-n", "kern.memorystatus_vm_pressure_level"],
    "vm": ["vm_stat"], "power": ["pmset", "-g", "custom"],
    "source": ["pmset", "-g", "batt"], "load": ["sysctl", "-n", "vm.loadavg"],
}


def command(argv: list[str], **kwargs) -> subprocess.CompletedProcess:
    """Run to natural completion; never send timeout signals to build workers."""
    return subprocess.run(argv, cwd=ROOT, text=True, capture_output=True, **kwargs)


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def source_manifest() -> dict[str, str]:
    """Snapshot tracked and non-ignored files, including dirty and deleted files."""
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, capture_output=True, check=True,
    )
    manifest = {}
    for name in sorted(set(result.stdout.split(b"\0")) - {b""}):
        path = ROOT / os.fsdecode(name)
        if path.is_symlink():
            data = "symlink:" + os.readlink(path)
        elif path.is_file():
            data = "file:" + file_hash(path)
        else:
            data = "deleted"
        manifest[os.fsdecode(name)] = data
    return manifest


def selected_sources(manifest: dict, scope: dict | None) -> dict:
    if scope is None:
        return manifest
    if scope.get("kind") != "cargo_component":
        raise ValueError("unknown source scope")
    return {name: value for name, value in manifest.items() if name in scope["files"] or
            any(name == root or name.startswith(root + "/") for root in scope["roots"])}


def manifest_digest(manifest: dict) -> str:
    return hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def source_digest(scope: dict | None = None) -> str:
    """Bind the complete selected source set, detecting additions and deletions."""
    return manifest_digest(selected_sources(source_manifest(), scope))


def component_scope(metadata: dict, artifacts: list[dict]) -> dict:
    """Use Cargo's actual artifacts, including build/dev dependencies, not guesses."""
    packages = {package["id"]: package for package in metadata["packages"]}
    roots = {".cargo"}
    files = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain",
             "scripts/cargo_fast.sh", "scripts/check_cargo_target_owner.py",
             "scripts/kagemusha_qualify.py"}
    compiled = {}
    for artifact in artifacts:
        package = packages[artifact["package_id"]]
        compiled[package["id"]] = {"name": package["name"], "version": package["version"],
                                    "source": package["source"], "manifest_path": package["manifest_path"]}
        if package["source"] is not None:
            continue
        directory = Path(package["manifest_path"]).resolve().parent.relative_to(ROOT)
        roots.add(directory.as_posix())
        # Rust dep-info includes source/fixture includes outside the crate root.
        # Generated target files are bound by the executable; their generators
        # and build-script packages are already members of this artifact set.
        for filename in artifact.get("filenames", []):
            path = Path(filename)
            stem = path.stem.removeprefix("lib") if path.suffix in (".rlib", ".rmeta", ".dylib", ".so") else path.stem
            depinfo = path.with_name(stem + ".d")
            if not depinfo.is_file():
                continue
            for line in depinfo.read_text().splitlines():
                _, separator, dependencies = line.partition(": ")
                if not separator or line.startswith("#"):
                    continue
                for dependency in shlex.split(dependencies):
                    source = Path(dependency)
                    source = (ROOT / source).resolve() if not source.is_absolute() else source.resolve()
                    if source.is_relative_to(ROOT) and not source.is_relative_to(ROOT / "target"):
                        files.add(source.relative_to(ROOT).as_posix())
    if not any(package["name"] == "iroha_plonk_gadgets" for package in compiled.values()):
        raise ValueError("component artifact set omits its measurement driver")
    return {"kind": "cargo_component", "roots": sorted(roots), "files": sorted(files),
            "packages": [compiled[key] for key in sorted(compiled)]}


def ignored_output(path: Path) -> Path:
    """Refuse output inside a tracked or non-ignored part of the checkout."""
    path = path.resolve()
    try:
        relative = path.relative_to(ROOT)
    except ValueError:
        pass
    else:
        result = command(["git", "check-ignore", "--quiet", str(relative / "probe.json")])
        if result.returncode:
            raise ValueError("output must be outside the checkout or in an ignored directory")
    path.mkdir(parents=True, exist_ok=True)
    return path


def prepare(output: Path, *, component: bool = False, target_slot: str = "m3b") -> None:
    output = ignored_output(output)
    candidate_path = output / "candidate.json"
    if candidate_path.exists():
        raise ValueError("candidate.json already exists; use a fresh output directory")
    before_manifest = source_manifest() if component else None
    before = source_digest() if not component else manifest_digest(before_manifest)
    metadata = None
    if component:
        metadata_result = command(["cargo", "metadata", "--locked", "--offline", "--format-version=1"])
        metadata_result.check_returncode()
        metadata = json.loads(metadata_result.stdout)
        write_json(output / "cargo-metadata.json", metadata)
    argv = [
        "scripts/cargo_fast.sh", "--stable-local-metadata", "--jobs", "2", "--target-slot", target_slot, "--",
        "test", "--locked", "--offline", "--release", "-p", "iroha_plonk_gadgets",
        "--test", "m3_gates", "--no-run", "--message-format=json",
    ]
    result = command(argv)
    (output / "build.stdout.log").write_text(result.stdout)
    (output / "build.stderr.log").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"build failed ({result.returncode}); see build.stderr.log")
    executables = set()
    artifacts = []
    for line in result.stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get("reason") == "compiler-artifact":
            artifacts.append(item)
            if item.get("target", {}).get("name") == "m3_gates" and item.get("executable"):
                executables.add(item["executable"])
    if len(executables) != 1:
        raise ValueError(f"expected one m3_gates executable, found {len(executables)}")
    scope = component_scope(metadata, artifacts) if component else None
    if component:
        after_manifest = source_manifest()
        source_files = selected_sources(after_manifest, scope)
        before_selected = selected_sources(before_manifest, scope)
        write_json(output / "source-provenance.json", {
            "scope": scope, "before": before_selected, "after": source_files,
            "whole_checkout_before": before, "whole_checkout_after": manifest_digest(after_manifest),
            "changed_in_scope": sorted(name for name in before_selected.keys() | source_files.keys()
                                       if before_selected.get(name) != source_files.get(name)),
        })
        before = manifest_digest(before_selected)
        after = manifest_digest(source_files)
    else:
        after = source_digest()
    if before != after:
        raise ValueError("source changed during build; candidate is not frozen")
    binary = Path(executables.pop()).resolve()
    binary_sha256 = file_hash(binary)
    compiler = command(["rustc", "-Vv"])
    compiler.check_returncode()
    listing = command([str(binary), "--list"])
    listing.check_returncode()
    for config in CONFIGS.values():
        if f"{config['test']}: test" not in listing.stdout:
            raise ValueError(f"missing benchmark {config['test']}")
    inventory = command([
        str(binary), "--ignored", "--exact", "qualification_candidate_layouts",
        "--nocapture", "--test-threads=1",
    ], env={**os.environ, "RAYON_NUM_THREADS": "1"})
    (output / "layouts.stdout.log").write_text(inventory.stdout)
    (output / "layouts.stderr.log").write_text(inventory.stderr)
    inventory.check_returncode()
    layouts = parse_layouts(inventory.stdout)
    if (source_digest(scope) if component else source_digest()) != after:
        raise ValueError("source changed during descriptor inventory; candidate is not frozen")
    if file_hash(binary) != binary_sha256:
        raise ValueError("executable changed during descriptor inventory; candidate is not frozen")
    write_json(candidate_path, {
        "schema": "kagemusha.m3.candidate.v1", "source_sha256": after,
        "source_scope": scope, "qualification_scope": "component" if component else "whole_checkout",
        "binary": str(binary), "binary_sha256": binary_sha256,
        "compiler": compiler.stdout, "build_command": argv,
        "profile": "release", "features": "default",
        "layouts": layouts,
        "platform": platform.platform(), "machine": platform.machine(),
        "build_flags": {key: os.environ.get(key) for key in (
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_PROFILE_RELEASE_LTO",
        )},
    })
    print(candidate_path)


def parse_vm_stat(text: str) -> dict[str, int]:
    counters = {}
    for label in ("Compressions", "Pageouts", "Swapouts", "Swapins"):
        match = re.search(rf"^{label}:\s+(\d+)\.", text, re.MULTILINE)
        if not match:
            raise ValueError(f"missing vm_stat counter {label}")
        counters[label] = int(match[1])
    return counters


def environment() -> dict:
    """Capture raw evidence; unsupported or failed probes cannot qualify."""
    raw = {}
    if sys.platform == "darwin":
        for key, argv in PROBES.items():
            try:
                result = command(argv)
                raw[key] = {"code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
            except OSError as error:
                raw[key] = {"code": None, "stdout": "", "stderr": str(error)}
    return {"platform": sys.platform, "raw": raw}


def environment_counters(value: dict) -> dict:
    """Derive validity from raw probe results, never cached validity flags."""
    if value.get("platform") != "darwin":
        raise ValueError("this qualification profile requires macOS")
    raw = value["raw"]
    if not isinstance(raw, dict) or set(raw) != set(PROBES):
        raise ValueError("incomplete environment probes")
    for name, probe in raw.items():
        if (not isinstance(probe, dict) or type(probe.get("code")) is not int or
                probe["code"] != 0 or not isinstance(probe.get("stdout"), str) or
                not isinstance(probe.get("stderr"), str) or not probe["stdout"].strip()):
            raise ValueError(f"environment probe failed: {name}")
    if raw["pressure"]["stdout"].strip() != "1":
        raise ValueError("memory pressure is not normal")
    if "AC Power" not in raw["source"]["stdout"]:
        raise ValueError("qualification host is not on AC power")
    if re.search(r"lowpowermode\s+1\b", raw["power"]["stdout"]):
        raise ValueError("low power mode is enabled")
    return parse_vm_stat(raw["vm"]["stdout"])


def environment_reasons(before: dict, after: dict) -> list[str]:
    reasons = []
    counters = []
    for name, value in (("before", before), ("after", after)):
        try:
            counters.append(environment_counters(value))
        except (ValueError, KeyError, TypeError, AttributeError) as error:
            reasons.append(f"{name}: environment unavailable: {error}")
    if reasons:
        return reasons
    for name, count in counters[0].items():
        if counters[1][name] != count:
            reasons.append(f"memory counter changed: {name}")
    if before["raw"]["power"]["stdout"] != after["raw"]["power"]["stdout"]:
        reasons.append("power policy changed")
    return reasons


def parse_report(text: str) -> dict:
    lines = [line.split(PREFIX, 1)[1] for line in text.splitlines() if PREFIX in line]
    if len(lines) != 1:
        raise ValueError(f"expected one measurement record, found {len(lines)}")
    report = json.loads(lines[0])
    if not isinstance(report, dict) or report.get("schema") != "kagemusha.m3.process.v1":
        raise ValueError("unknown measurement schema")
    return report


def parse_layouts(text: str) -> dict:
    """Require one actual-key descriptor for each distinct circuit workload."""
    layouts = {}
    for line in text.splitlines():
        if "M3_LAYOUT_JSON " not in line:
            continue
        item = json.loads(line.split("M3_LAYOUT_JSON ", 1)[1])
        if (not isinstance(item, dict) or not isinstance(item.get("gate"), str) or
                item["gate"] in layouts):
            raise ValueError("malformed or duplicate layout inventory")
        if (item.get("k") != 16 or item.get("transcript_profile") != "pipa-r" or
                not isinstance(item.get("shape"), str) or
                not item["shape"] or item.get("descriptor_hash") != "blake2b256-pipa-v2-circdesc" or
                not isinstance(item.get("descriptor_digest"), str) or
                re.fullmatch(r"[0-9a-f]{64}", item["descriptor_digest"]) is None):
            raise ValueError("invalid layout descriptor")
        layouts[item["gate"]] = item
    if set(layouts) != {config["gate"] for config in CONFIGS.values()}:
        raise ValueError("incomplete or unexpected layout inventory")
    return layouts


def report_reasons(report: dict, config: dict, candidate: dict, seed: int) -> list[str]:
    reasons = []
    expected = {
        "workers": config["workers"], "binary_sha256": candidate["binary_sha256"],
        "witness_api": "owned", "coset_cache": "on_demand", "commitment_tables": False,
        "peak_rss_source": "kernel_lifetime_high_water",
        "quotient_workspace": "caller_owned", "quotient_workspace_budget_bytes": 256 << 20,
        "msm_process_budget_bytes": 64 << 20,
        "msm_process_retained_bytes": 0, "seed": seed, "transcript_profile": "pipa-r",
        **candidate["layouts"][config["gate"]],
    }
    for name, value in expected.items():
        if type(report.get(name)) is not type(value) or report[name] != value:
            reasons.append(f"measurement configuration mismatch: {name}")
    if type(report.get("peak_rss_bytes")) is not int or report["peak_rss_bytes"] <= 0:
        reasons.append("missing kernel peak RSS")
    scratch = report.get("msm_process_peak_bytes")
    if type(scratch) is not int or not 0 < scratch <= 64 << 20:
        reasons.append("invalid shared MSM scratch high-water")
    samples = report.get("samples", [])
    if not isinstance(samples, list) or any(not isinstance(sample, dict) for sample in samples):
        return reasons + ["malformed proof samples"]
    if len(samples) != 2:
        reasons.append("exactly the first two proofs must be retained")
    if [sample.get("index") for sample in samples] != [0, 1]:
        reasons.append("proof sample order or identity differs")
    for sample in samples:
        if sample.get("verified") is not True:
            reasons.append("unverified measured proof")
        for metric in ("cpu_ns", "total_ns"):
            if type(sample.get(metric)) is not int or sample[metric] <= 0:
                reasons.append(f"invalid {metric}")
        for boundary in ("thermal_before", "thermal_after"):
            if sample.get(boundary) != "nominal":
                reasons.append("non-nominal or unavailable thermal state")
    workspace = report.get("quotient_workspace_allocated_bytes")
    if type(workspace) is not int or not 0 < workspace <= 256 << 20:
        reasons.append("invalid caller-owned quotient workspace allocation")
    else:
        for index, sample in enumerate(samples):
            expected_before = 0 if index == 0 else workspace
            if (type(sample.get("quotient_workspace_before_bytes")) is not int or
                    sample["quotient_workspace_before_bytes"] != expected_before or
                    type(sample.get("quotient_workspace_after_bytes")) is not int or
                    sample["quotient_workspace_after_bytes"] != workspace):
                reasons.append("quotient workspace not retained and reused between proofs")
    for boundary in ("thermal_before", "thermal_after"):
        if report.get(boundary) != "nominal":
            reasons.append("non-nominal or unavailable process thermal state")
    return reasons


def slower(report: dict, metric: str) -> int:
    return max(sample[metric] for sample in report["samples"])


def calibration_reasons(before: dict, after: dict) -> list[str]:
    reasons = []
    for metric in ("cpu_ns", "total_ns"):
        values = [slower(before, metric), slower(after, metric)]
        if min(values) <= 0 or max(values) * 100 > min(values) * 105:
            reasons.append(f"calibration drift exceeds 5%: {metric}")
    return reasons


def run_process(candidate: dict, config: dict, seed: int, path: Path) -> dict:
    before = environment()
    env = {**os.environ, "RAYON_NUM_THREADS": str(config["workers"]), "M3_SEED": str(seed)}
    try:
        result = command([
            candidate["binary"], "--ignored", "--exact", config["test"], "--nocapture",
            "--test-threads=1",
        ], env=env)
        process = {"code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    except OSError as error:
        process = {"code": None, "stdout": "", "stderr": str(error)}
    after = environment()
    (path.with_suffix(".stdout.log")).write_text(process["stdout"])
    (path.with_suffix(".stderr.log")).write_text(process["stderr"])
    try:
        report = parse_report(process["stdout"])
    except (ValueError, KeyError, TypeError):
        report = None
    observation = {"before": before, "after": after, "seed": seed,
                   "report": report, "process": process}
    observation["reasons"] = observation_reasons(observation, config, candidate, seed)
    write_json(path.with_suffix(".json"), observation)
    return observation


def observation_reasons(value: dict, config: dict, candidate: dict, seed: int) -> list[str]:
    """Recompute a process result from its exit, raw output and raw probes."""
    reasons = []
    try:
        reasons.extend(environment_reasons(value["before"], value["after"]))
        if type(value["seed"]) is not int or value["seed"] != seed:
            reasons.append("process seed differs")
        process = value["process"]
        if type(process["code"]) is not int or process["code"] != 0:
            reasons.append(f"benchmark exited {process['code']}")
        if not isinstance(process["stdout"], str) or not isinstance(process["stderr"], str):
            raise ValueError("missing raw process output")
        report = parse_report(process["stdout"])
        if report != value["report"]:
            reasons.append("retained report differs from raw process output")
        reasons.extend(report_reasons(report, config, candidate, seed))
    except (ValueError, KeyError, TypeError, AttributeError) as error:
        reasons.append(f"retained process evidence invalid: {error}")
    return reasons


def candidate_boundary(candidate: dict) -> dict:
    scope = candidate.get("source_scope")
    return {"source_sha256": source_digest(scope) if scope else source_digest(),
            "binary_sha256": file_hash(Path(candidate["binary"]))}


def attempt_reasons(item: dict, config: dict, candidate: dict) -> list[str]:
    """Validity is derived from all three processes and both candidate seals."""
    reasons = []
    try:
        seed = item["seed"]
        if type(seed) is not int or not 0 <= seed < 1 << 64:
            raise ValueError("invalid measurement seed")
        measured = item["measured"]
        if item["report"] != measured["report"]:
            reasons.append("attempt report differs from measured process")
        reasons.extend(observation_reasons(measured, config, candidate, seed))
        calibration = CONFIGS[f"{config['kind'][0]}_exact-{config['workers']}"]
        calibration_valid = True
        for label in ("pre", "post"):
            errors = observation_reasons(item[label], calibration, candidate, 0)
            reasons.extend(f"{label}-calibration: {error}" for error in errors)
            calibration_valid &= not errors
        if calibration_valid:
            reasons.extend(calibration_reasons(item["pre"]["report"], item["post"]["report"]))
        expected = {key: candidate[key] for key in ("source_sha256", "binary_sha256")}
        for label in ("candidate_before", "candidate_after"):
            if item[label] != expected:
                reasons.append(f"{label} differs from frozen candidate")
    except (ValueError, KeyError, TypeError, AttributeError) as error:
        reasons.append(f"retained attempt evidence invalid: {error}")
    return reasons


def verdict(config: dict, attempts: list[dict]) -> dict:
    """All valid samples participate; a favorable minimum never decides a gate."""
    valid = [item for item in attempts if not item["reasons"]]
    family = config["kind"][0]
    hard_rss = (0.75 if family == "q" else 0.85) * GIB
    margin_rss = hard_rss * 0.95
    hard_time = 30 if family == "q" and config["workers"] == 1 else (
        36 if config["workers"] == 1 else (10 if family == "q" else None)
    )
    metric = "cpu_ns" if config["workers"] == 1 else "total_ns"
    peaks = [item["report"]["peak_rss_bytes"] for item in valid]
    times = [slower(item["report"], metric) / 1e9 for item in valid]
    medians = []
    rss_medians = []
    complete = True
    for block in range(3):
        block_reports = [item["report"] for item in valid if item["block"] == block]
        values = [slower(report, metric) / 1e9 for report in block_reports]
        complete &= len(values) == 3
        if values:
            medians.append(statistics.median(values))
            rss_medians.append(statistics.median(report["peak_rss_bytes"] for report in block_reports))
    hard_failure = any(value > hard_rss for value in peaks) or (
        hard_time is not None and any(value > hard_time for value in times)
    )
    margin_failure = any(value > margin_rss for value in rss_medians) or (
        hard_time is not None and any(value > hard_time * 0.9 for value in medians)
    )
    status = "fail" if hard_failure else (
        "inconclusive" if not complete else ("borderline" if margin_failure else "pass")
    )
    return {"status": status, "valid_processes": len(valid), "attempts": len(attempts),
            "block_medians_seconds": medians, "all_process_times_seconds": times,
            "block_medians_rss_bytes": rss_medians, "all_process_rss_bytes": peaks,
            "maximum_kernel_rss_bytes": max(peaks, default=None), "metric": metric}


def summarize(path: Path) -> dict:
    ledger = json.loads(path.read_text())
    if not isinstance(ledger["attempts"], dict) or set(ledger["attempts"]) - set(CONFIGS):
        raise ValueError("invalid or unknown qualification configuration")
    ledger_reasons = []
    if ledger.get("schema") != "kagemusha.m3.runs.v1":
        ledger_reasons.append("missing complete qualification ledger schema")
    seed = ledger.get("shuffle_seed")
    if type(seed) is not int or ledger.get("schedule") != schedule(random.Random(seed)):
        ledger_reasons.append("invalid predeclared configuration schedule")
    if not ledger_reasons:
        sequence = []
        for block, names in enumerate(ledger["schedule"]):
            for name in names:
                rows = ledger["attempts"].get(name, [])
                if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
                    raise ValueError("attempts must be an ordered list of records")
                sequence.extend(row.get("sequence") for row in rows if row.get("block") == block)
        if (any(type(number) is not int for number in sequence) or
                sequence != list(range(sum(len(rows) for rows in ledger["attempts"].values())))):
            ledger_reasons.append("attempt execution differs from predeclared schedule")
    results = {}
    for name, config in CONFIGS.items():
        checked = []
        seen_seeds = set()
        rows = ledger["attempts"].get(name, [])
        if not isinstance(rows, list):
            raise ValueError("attempts must be an ordered list")
        config_reasons = ledger_reasons + (["configuration exceeds 18 attempts"] if len(rows) > 18 else [])
        previous_block = -1
        for item in rows:
            reasons = list(config_reasons)
            try:
                retained = item["reasons"]
                if not isinstance(retained, list) or any(not isinstance(reason, str) for reason in retained):
                    raise ValueError("malformed retained reasons")
                reasons.extend(retained)
                if type(item["block"]) is not int or item["block"] not in range(3):
                    raise ValueError("invalid measurement block")
                if item["block"] < previous_block:
                    reasons.append("measurement blocks are out of order")
                previous_block = item["block"]
                seed = item["seed"]
                if type(seed) is not int or not 0 <= seed < 1 << 64:
                    raise ValueError("invalid measurement seed")
                if seed in seen_seeds:
                    reasons.append("duplicate measured process seed")
                seen_seeds.add(seed)
                reasons.extend(attempt_reasons(item, config, ledger["candidate"]))
            except (ValueError, KeyError, TypeError) as error:
                reasons.append(f"retained evidence invalid: {error}")
            checked.append({**item, "reasons": list(dict.fromkeys(reasons))})
        results[name] = verdict(config, checked)
        results[name]["invalid_attempts"] = [
            {"attempt": index + 1, "reasons": item["reasons"]}
            for index, item in enumerate(checked) if item["reasons"]
        ]
    statuses = {value["status"] for value in results.values()}
    overall = next((status for status in ("fail", "inconclusive", "borderline") if status in statuses), "pass")
    return {"status": overall, "configurations": results, "candidate": ledger["candidate"]}


def schedule(rng: random.Random) -> list[list[str]]:
    order = []
    for _ in range(3):
        names = list(CONFIGS)
        rng.shuffle(names)
        order.append(names)
    return order


def run(candidate_path: Path, seed: int, *, stop_on_hard_failure: bool = False) -> dict:
    candidate = json.loads(candidate_path.read_text())
    output = candidate_path.parent
    if (output / "runs.json").exists():
        raise ValueError("runs.json already exists; retained attempts may not be overwritten")
    boundary = candidate_boundary(candidate)
    if any(candidate[key] != boundary[key] for key in boundary):
        raise ValueError("source or executable differs from frozen candidate")
    rng = random.Random(seed)
    order = schedule(rng)
    ledger = {"schema": "kagemusha.m3.runs.v1", "candidate": candidate,
              "shuffle_seed": seed, "schedule": order,
              "stop_on_hard_failure": stop_on_hard_failure,
              "attempts": {name: [] for name in CONFIGS}}
    write_json(output / "runs.json", ledger)
    for block, names in enumerate(order):
        for name in names:
            config = CONFIGS[name]
            attempts = ledger["attempts"][name]
            while sum(not value["reasons"] and value["block"] == block for value in attempts) < 3 and len(attempts) < 18:
                before = candidate_boundary(candidate)
                if any(before[key] != candidate[key] for key in before):
                    raise ValueError("candidate changed during qualification; retained results cannot qualify the new source")
                number = len(attempts)
                prefix = output / f"{name}-b{block + 1}-attempt{number + 1:02}"
                calibration = CONFIGS[f"{config['kind'][0]}_exact-{config['workers']}"]
                # Fixed calibration witness, independent of the recorded measurement seed.
                pre = run_process(candidate, calibration, 0, prefix.with_name(prefix.name + "-pre"))
                measured = run_process(candidate, config, rng.randrange(1 << 64), prefix)
                post = run_process(candidate, calibration, 0, prefix.with_name(prefix.name + "-post"))
                after = candidate_boundary(candidate)
                changed = any(after[key] != candidate[key] for key in after)
                attempt = {"block": block, "seed": measured["seed"], "report": measured["report"],
                           "sequence": sum(len(rows) for rows in ledger["attempts"].values()),
                           "pre": pre, "measured": measured, "post": post,
                           "candidate_before": before, "candidate_after": after}
                reasons = attempt_reasons(attempt, config, candidate)
                attempt["reasons"] = reasons
                attempts.append(attempt)
                write_json(output / "runs.json", ledger)
                write_json(output / "summary.json", summarize(output / "runs.json"))
                print(f"{name} block {block + 1} attempt {number + 1}: " + ("valid" if not reasons else "; ".join(reasons)), flush=True)
                if changed:
                    raise ValueError("candidate changed; qualification stopped with raw evidence retained")
                if stop_on_hard_failure and verdict(config, attempts)["status"] == "fail":
                    print("observed hard limit exceeded; incomplete schedule retained", flush=True)
                    return summarize(output / "runs.json")
            # Configurations between blocks provide their temporal separation.
    result = summarize(output / "runs.json")
    write_json(output / "summary.json", result)
    print(json.dumps(result, indent=2))
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    build = actions.add_parser("prepare", help="build and freeze source/executable provenance")
    build.add_argument("--output", type=Path, required=True)
    build.add_argument("--component", action="store_true",
                       help="bind actual Cargo dependency sources; cannot qualify the whole release")
    build.add_argument("--target-slot", default="m3b")
    execute = actions.add_parser("run", help="collect the fixed fresh-process qualification schedule")
    execute.add_argument("--candidate", type=Path, required=True)
    execute.add_argument("--seed", type=int, default=20261006)
    execute.add_argument("--stop-on-hard-failure", action="store_true",
                         help="retain a failing partial schedule and stop after a valid hard-cap breach")
    report = actions.add_parser("summarize", help="recompute verdicts from retained raw records")
    report.add_argument("runs", type=Path)
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(args.output, component=args.component, target_slot=args.target_slot)
    elif args.action == "run":
        result = run(args.candidate.resolve(), args.seed, stop_on_hard_failure=args.stop_on_hard_failure)
        raise SystemExit(0 if result["status"] == "pass" else 1)
    else:
        result = summarize(args.runs)
        print(json.dumps(result, indent=2))
        raise SystemExit(0 if result["status"] == "pass" else 1)


if __name__ == "__main__":
    main()
