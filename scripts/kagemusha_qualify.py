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


def source_digest() -> str:
    """Bind all tracked and non-ignored source, including the dirty checkout."""
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, capture_output=True, check=True,
    )
    digest = hashlib.sha256()
    for name in sorted(set(result.stdout.split(b"\0")) - {b""}):
        path = ROOT / os.fsdecode(name)
        digest.update(len(name).to_bytes(8, "little"))
        digest.update(name)
        if path.is_symlink():
            data = b"symlink:" + os.fsencode(os.readlink(path))
        elif path.is_file():
            data = b"file:" + bytes.fromhex(file_hash(path))
        else:
            data = b"deleted"
        digest.update(len(data).to_bytes(8, "little"))
        digest.update(data)
    return digest.hexdigest()


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


def prepare(output: Path) -> None:
    output = ignored_output(output)
    candidate_path = output / "candidate.json"
    if candidate_path.exists():
        raise ValueError("candidate.json already exists; use a fresh output directory")
    before = source_digest()
    argv = [
        "scripts/cargo_fast.sh", "--incremental", "--target-slot", "m3b", "--",
        "test", "--locked", "--offline", "--release", "-p", "iroha_plonk_gadgets",
        "--test", "m3_gates", "--no-run", "--message-format=json",
    ]
    result = command(argv)
    (output / "build.stdout.log").write_text(result.stdout)
    (output / "build.stderr.log").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"build failed ({result.returncode}); see build.stderr.log")
    executables = set()
    for line in result.stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "m3_gates":
            if item.get("executable"):
                executables.add(item["executable"])
    if len(executables) != 1:
        raise ValueError(f"expected one m3_gates executable, found {len(executables)}")
    after = source_digest()
    if before != after:
        raise ValueError("source changed during build; candidate is not frozen")
    binary = Path(executables.pop()).resolve()
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
    if source_digest() != after:
        raise ValueError("source changed during descriptor inventory; candidate is not frozen")
    write_json(candidate_path, {
        "schema": "kagemusha.m3.candidate.v1", "source_sha256": after,
        "binary": str(binary), "binary_sha256": file_hash(binary),
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
    if sys.platform != "darwin":
        return {"valid": False, "reason": "this qualification profile requires macOS"}
    raw = {}
    for key, argv in {
        "pressure": ["sysctl", "-n", "kern.memorystatus_vm_pressure_level"],
        "vm": ["vm_stat"], "power": ["pmset", "-g", "custom"],
        "source": ["pmset", "-g", "batt"], "load": ["sysctl", "-n", "vm.loadavg"],
    }.items():
        result = command(argv)
        raw[key] = {"code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    try:
        if any(value["code"] != 0 for value in raw.values()):
            raise ValueError("environment probe failed")
        counters = parse_vm_stat(raw["vm"]["stdout"])
        if raw["pressure"]["stdout"].strip() != "1":
            raise ValueError("memory pressure is not normal")
        if "AC Power" not in raw["source"]["stdout"]:
            raise ValueError("qualification host is not on AC power")
        if re.search(r"lowpowermode\s+1\b", raw["power"]["stdout"]):
            raise ValueError("low power mode is enabled")
        return {"valid": True, "raw": raw, "counters": counters}
    except ValueError as error:
        return {"valid": False, "raw": raw, "reason": str(error)}


def environment_reasons(before: dict, after: dict) -> list[str]:
    reasons = []
    for name, value in (("before", before), ("after", after)):
        if not value.get("valid"):
            reasons.append(f"{name}: {value.get('reason', 'environment unavailable')}")
    if reasons:
        return reasons
    for name, count in before["counters"].items():
        if after["counters"].get(name) != count:
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
    result = command([
        candidate["binary"], "--ignored", "--exact", config["test"], "--nocapture",
        "--test-threads=1",
    ], env=env)
    after = environment()
    (path.with_suffix(".stdout.log")).write_text(result.stdout)
    (path.with_suffix(".stderr.log")).write_text(result.stderr)
    reasons = environment_reasons(before, after)
    if result.returncode:
        reasons.append(f"benchmark exited {result.returncode}")
    try:
        report = parse_report(result.stdout)
        reasons.extend(report_reasons(report, config, candidate, seed))
    except (ValueError, KeyError, TypeError) as error:
        report = None
        reasons.append(str(error))
    observation = {"before": before, "after": after, "seed": seed,
                   "report": report, "reasons": reasons}
    write_json(path.with_suffix(".json"), observation)
    return observation


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
    results = {}
    for name, config in CONFIGS.items():
        checked = []
        seen_seeds = set()
        for item in ledger["attempts"].get(name, []):
            reasons = list(item["reasons"])
            try:
                if type(item["block"]) is not int or item["block"] not in range(3):
                    raise ValueError("invalid measurement block")
                seed = item["seed"]
                if type(seed) is not int or not 0 <= seed < 1 << 64:
                    raise ValueError("invalid measurement seed")
                if not reasons:
                    if seed in seen_seeds:
                        raise ValueError("duplicate measured process seed")
                    seen_seeds.add(seed)
                    if not isinstance(item["report"], dict):
                        raise ValueError("missing measured process report")
                    reasons.extend(report_reasons(item["report"], config, ledger["candidate"], seed))
            except (ValueError, KeyError, TypeError) as error:
                reasons.append(f"retained evidence invalid: {error}")
            checked.append({**item, "reasons": reasons})
        results[name] = verdict(config, checked)
    statuses = {value["status"] for value in results.values()}
    overall = next((status for status in ("fail", "inconclusive", "borderline") if status in statuses), "pass")
    return {"status": overall, "configurations": results, "candidate": ledger["candidate"]}


def run(candidate_path: Path, seed: int) -> None:
    candidate = json.loads(candidate_path.read_text())
    output = candidate_path.parent
    if (output / "runs.json").exists():
        raise ValueError("runs.json already exists; retained attempts may not be overwritten")
    if candidate["source_sha256"] != source_digest() or candidate["binary_sha256"] != file_hash(Path(candidate["binary"])):
        raise ValueError("source or executable differs from frozen candidate")
    rng = random.Random(seed)
    order = []
    for block in range(3):
        names = list(CONFIGS)
        rng.shuffle(names)
        order.append(names)
    ledger = {"candidate": candidate, "shuffle_seed": seed, "schedule": order,
              "attempts": {name: [] for name in CONFIGS}}
    write_json(output / "runs.json", ledger)
    for block, names in enumerate(order):
        for name in names:
            config = CONFIGS[name]
            attempts = ledger["attempts"][name]
            while sum(not value["reasons"] and value["block"] == block for value in attempts) < 3 and len(attempts) < 18:
                if source_digest() != candidate["source_sha256"] or file_hash(Path(candidate["binary"])) != candidate["binary_sha256"]:
                    raise ValueError("candidate changed during qualification; retained results cannot qualify the new source")
                number = len(attempts)
                prefix = output / f"{name}-b{block + 1}-attempt{number + 1:02}"
                calibration = CONFIGS[f"{config['kind'][0]}_exact-{config['workers']}"]
                # Fixed calibration witness, independent of the recorded measurement seed.
                pre = run_process(candidate, calibration, 0, prefix.with_name(prefix.name + "-pre"))
                measured = run_process(candidate, config, rng.randrange(1 << 64), prefix)
                post = run_process(candidate, calibration, 0, prefix.with_name(prefix.name + "-post"))
                reasons = measured["reasons"] + ["pre-calibration: " + x for x in pre["reasons"]] + ["post-calibration: " + x for x in post["reasons"]]
                if not pre["reasons"] and not post["reasons"]:
                    reasons += calibration_reasons(pre["report"], post["report"])
                changed = (source_digest() != candidate["source_sha256"] or
                           file_hash(Path(candidate["binary"])) != candidate["binary_sha256"])
                if changed:
                    reasons.append("candidate changed during attempt")
                attempts.append({"block": block, "seed": measured["seed"],
                                 "report": measured["report"], "reasons": reasons})
                write_json(output / "runs.json", ledger)
                write_json(output / "summary.json", summarize(output / "runs.json"))
                print(f"{name} block {block + 1} attempt {number + 1}: " + ("valid" if not reasons else "; ".join(reasons)), flush=True)
                if changed:
                    raise ValueError("candidate changed; qualification stopped with raw evidence retained")
            # Configurations between blocks provide their temporal separation.
    result = summarize(output / "runs.json")
    write_json(output / "summary.json", result)
    print(json.dumps(result, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    build = actions.add_parser("prepare", help="build and freeze source/executable provenance")
    build.add_argument("--output", type=Path, required=True)
    execute = actions.add_parser("run", help="collect the fixed fresh-process qualification schedule")
    execute.add_argument("--candidate", type=Path, required=True)
    execute.add_argument("--seed", type=int, default=20261006)
    report = actions.add_parser("summarize", help="recompute verdicts from retained raw records")
    report.add_argument("runs", type=Path)
    args = parser.parse_args()
    if args.action == "prepare":
        prepare(args.output)
    elif args.action == "run":
        run(args.candidate.resolve(), args.seed)
    else:
        print(json.dumps(summarize(args.runs), indent=2))


if __name__ == "__main__":
    main()
