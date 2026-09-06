#!/usr/bin/env python3
"""Compare cold, source-bound compiler RSS reports against measured byte limits.

This reads reports from profile_cargo_build.py; it never builds or profiles.
The caller must schedule both runs on the pinned runner named by the policy.
Report hashes bind the supplied evidence, not its publisher's authenticity;
signed CI provenance and runner scheduling remain release-gate responsibilities.
"""

from __future__ import annotations

import argparse
from collections import Counter
import importlib.util
import json
from pathlib import Path
import re
import sys
from typing import Any


_SPEC = importlib.util.spec_from_file_location(
    "iroha_memory_build_profiler", Path(__file__).with_name("profile_cargo_build.py")
)
assert _SPEC is not None and _SPEC.loader is not None
PROFILER = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = PROFILER
_SPEC.loader.exec_module(PROFILER)

RELEASE_CEILING = 13 * 1024**3
MODEL_PACKAGES = frozenset((
    "iroha_data_model", "iroha_model_base", "iroha_privacy_model", "iroha_service_model",
))
SURFACES = {
    "sdk": ["build", "-p", "iroha", "--lib"],
    "model": ["build", "-p", "iroha_data_model", "--lib"],
    "core-tests": ["test", "-p", "iroha_core", "--features", "iroha-core-tests"],
    "torii-tests": ["test", "-p", "iroha_torii"],
    "daemon-release": ["build", "--release", "-p", "irohad", "--bin", "iroha3d"],
    "native-bridge-release": [
        "build", "--release", "-p", "connect_norito_bridge", "--lib",
        "--features", "privacy-production-enabled",
    ],
    "javascript-native-release": ["build", "--release", "-p", "iroha_js_host", "--lib"],
}


def require(condition: bool, message: str) -> None:
    """Reject incomplete or inconsistent qualification evidence."""
    if not condition:
        raise ValueError(message)


def digest(value: Any) -> str:
    """Use the authoritative profiler's canonical JSON identity."""
    return PROFILER.sha256_bytes(PROFILER.canonical_json_bytes(value))


def sha256(value: Any) -> bool:
    """Recognize one canonical lowercase SHA-256 digest."""
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def positive(value: Any) -> bool:
    """Reject booleans and nonintegral/missing byte measurements."""
    return type(value) is int and value > 0


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Reject duplicate JSON fields instead of letting later values win."""
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def read_json(path: Path, expected_sha256: str | None = None) -> dict[str, Any]:
    """Read one bounded regular file and optionally verify its external pin."""
    path = path.resolve(strict=True)
    identity = PROFILER.RUSTC_PROFILE.stable_file_identity(path, maximum=64 * 1024**2)
    with path.open("rb") as source:
        payload = source.read(64 * 1024**2 + 1)
    require(len(payload) <= 64 * 1024**2, "JSON source exceeds the report byte limit")
    require(PROFILER.sha256_bytes(payload) == identity["sha256"], "JSON source changed while reading")
    if expected_sha256 is not None:
        require(sha256(expected_sha256), "expected report digest is not SHA-256")
        require(identity["sha256"] == expected_sha256, f"report digest mismatch: {path}")
    document = json.loads(payload, object_pairs_hook=unique_object,
                          parse_constant=lambda value: require(False, f"nonfinite JSON value: {value}"))
    require(isinstance(document, dict), "JSON root must be an object")
    return document


def unit_key(unit: dict[str, Any]) -> str:
    """Reuse the profiler's complete package/target/features/profile projection."""
    require(isinstance(unit, dict), "compiler unit must be an object")
    require(set(unit) == {"package_id", "name", "kind", "crate_types", "source_path", "features", "profile"},
            "compiler unit fields differ from the profiler identity")
    source = unit["source_path"]
    require(isinstance(source, str) and source and not Path(source).is_absolute(),
            "compiler source must be package-relative")
    # These paths only adapt the already-normalized record to the authoritative
    # projector. No file is opened and no package name is inferred from a path.
    projected = PROFILER.artifact_unit({
        "reason": "compiler-artifact", "package_id": unit["package_id"],
        "manifest_path": "/memory-unit/Cargo.toml", "profile": unit["profile"],
        "features": unit["features"], "target": {
            "name": unit["name"], "kind": unit["kind"], "crate_types": unit["crate_types"],
            "src_path": "/memory-unit/" + source,
        },
    })
    require(projected == unit, "compiler unit is incomplete or noncanonical")
    return PROFILER.canonical_json_bytes(unit).decode("utf-8")


def package_name(unit: dict[str, Any]) -> str | None:
    """Identify workspace packages without conflating registry dependencies."""
    package = unit["package_id"]
    if package.startswith("workspace#"):
        name, separator, version = package[len("workspace#"):].rpartition("@")
        require(bool(name and separator and version), "workspace package lacks name/version")
        return name
    return None


def normalized_compiler_arguments(arguments: list[str]) -> list[str]:
    """Normalize rustc's codegen aliases before profile/control inspection."""
    require(isinstance(arguments, list) and all(isinstance(a, str) for a in arguments),
            "compiler arguments must be strings")
    normalized = []
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        if argument == "--codegen" or argument.startswith("--codegen="):
            if argument == "--codegen":
                index += 1
                require(index < len(arguments), "missing --codegen argument")
                value = arguments[index]
            else:
                value = argument[len("--codegen="):]
            require(bool(value), "empty --codegen argument")
            normalized.extend(("-C", value))
        elif argument == "-O":
            normalized.extend(("-C", "opt-level=3"))
        elif argument == "-g":
            normalized.extend(("-C", "debuginfo=2"))
        else:
            normalized.append(argument)
        index += 1
    return normalized


def compiler_controls(arguments: list[str]) -> dict[str, Any]:
    """Retain codegen controls omitted by Cargo's public profile identity.

    Cargo's metadata and filename disambiguators change with dependency/source
    identities. They do not select optimization. All other -C options, target,
    edition, cfgs, sysroot and unstable switches remain comparison inputs.
    """
    arguments = normalized_compiler_arguments(arguments)
    codegen: list[str] = []
    unstable: list[str] = []
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        prefix = argument[:2]
        if prefix in ("-C", "-Z"):
            if len(argument) == 2:
                index += 1
                require(index < len(arguments), f"missing {prefix} argument")
                value = arguments[index]
            else:
                value = argument[2:]
            key = value.partition("=")[0]
            require(key != "incremental", "incremental compiler invocation is not cold evidence")
            if prefix == "-Z":
                unstable.append(value)
            elif key not in ("metadata", "extra-filename"):
                codegen.append(value)
        index += 1
    return {
        "codegen": codegen, "unstable": unstable,
        "target": PROFILER.RUSTC_PROFILE.option_values(arguments, "--target"),
        "edition": PROFILER.RUSTC_PROFILE.option_values(arguments, "--edition"),
        "cfg": sorted(set(PROFILER.RUSTC_PROFILE.option_values(arguments, "--cfg"))),
        "sysroot": PROFILER.RUSTC_PROFILE.option_values(arguments, "--sysroot"),
    }


def validate_report(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Require a complete cold report with intact source and measurement seals."""
    require(report.get("schema_version") == 4 and report.get("valid") is True,
            "memory qualification requires a valid profiler schema-4 report")
    inputs, result, validation = report["input"], report["result"], report["input_validation"]
    require(digest(inputs) == report["input_sha256"], "input manifest digest mismatch")
    require(validation["stable"] is True and validation["changed_fields"] == []
            and validation["error"] is None and validation["private_state_cleanup_error"] is None,
            "profile source/toolchain seals are not stable")
    require(validation["post_input"] == inputs
            and validation["post_input_sha256"] == report["input_sha256"], "post-run input identity differs")
    require(inputs["profile_mode"] == "cold" and inputs["target_initial"] == {
        "bytes": 0, "files": 0, "records": 0, "sha256": PROFILER.sha256_bytes(b""),
    }, "memory qualification requires an empty cold target")
    measurement = inputs["compiler_measurement"]
    require(measurement["method"] == "wait4-per-compiler" and measurement["rss_unit"] == "bytes"
            and measurement["record_schema"] == 3 and measurement["compiler_entrypoint"] == "RUSTC-and-PATH",
            "unsupported compiler measurement instrumentation")
    require(type(result["returncode"]) is int and result["returncode"] == 0
            and result["compiler_measurement_error"] is None, "failed compiler build or instrumentation")
    for field in ("unit_inventory", "compiler_measurements", "compiler_records"):
        require(digest(result[field]) == result[field + "_sha256"], f"{field} digest mismatch")
    measurements = result["compiler_measurements"]
    require(measurements["fresh"] == [] and measurements["failed"] == [],
            "cached or failed compiler units cannot qualify a memory budget")
    require(isinstance(measurements["probes"], list), "compiler probe inventory is missing")
    for probe in measurements["probes"]:
        require(positive(probe["peak_rss_bytes"]) and type(probe["returncode"]) is int
                and isinstance(probe["arguments"], list)
                and all(isinstance(argument, str) for argument in probe["arguments"]),
                "invalid compiler probe RSS evidence")
        source = probe["source"]
        require(source is None or (isinstance(source, dict) and source.get("kind") in ("stdin", "file")
                and type(source.get("bytes")) is int and source["bytes"] >= 0 and sha256(source.get("sha256"))),
                "compiler source probe identity is missing")
    compiled = measurements["compiled"]
    require(isinstance(compiled, list) and bool(compiled)
            and type(result["compiled_units"]) is int and result["compiled_units"] == len(compiled)
            and type(result["fresh_units"]) is int and result["fresh_units"] == 0,
            "compiler inventory count mismatch")
    inventory = [unit_key(unit) for unit in result["unit_inventory"]]
    units: dict[str, dict[str, Any]] = {}
    compiled_inventory = []
    for row in compiled:
        key = unit_key(row["unit"])
        compiled_inventory.append(key)
        require(type(row["returncode"]) is int and row["returncode"] == 0
                and positive(row["peak_rss_bytes"]), "invalid per-compiler RSS measurement")
        require(row["compiler_artifacts"] and row["cargo_artifacts"], "compiler artifacts are missing")
        for artifact in row["compiler_artifacts"] + row["cargo_artifacts"]:
            require(isinstance(artifact, dict) and set(artifact) == {"path", "bytes", "sha256"}
                    and isinstance(artifact["path"], str) and artifact["path"].startswith("target/")
                    and ".." not in Path(artifact["path"]).parts
                    and type(artifact["bytes"]) is int and artifact["bytes"] >= 0
                    and sha256(artifact["sha256"]), "invalid sealed compiler artifact")
        emitted = {(a["bytes"], a["sha256"]) for a in row["compiler_artifacts"]}
        require(all((a["bytes"], a["sha256"]) in emitted for a in row["cargo_artifacts"]),
                "Cargo artifact lacks matching compiler output identity")
        controls = compiler_controls(row["arguments"])
        observed_profile = PROFILER.RUSTC_PROFILE.compiler_profile(normalized_compiler_arguments(row["arguments"]))
        require(all(row["unit"]["profile"].get(k) == v for k, v in observed_profile.items()),
                "compiler arguments disagree with the Cargo profile")
        if key not in units:
            units[key] = {"unit": row["unit"], "peak_rss_bytes": row["peak_rss_bytes"],
                          "compiler_controls": [], "invocations": 0}
        group = units[key]
        group["invocations"] += 1
        group["peak_rss_bytes"] = max(group["peak_rss_bytes"], row["peak_rss_bytes"])
        if controls not in group["compiler_controls"]:
            group["compiler_controls"].append(controls)
            group["compiler_controls"].sort(key=PROFILER.canonical_json_bytes)
    require(Counter(compiled_inventory) == Counter(inventory), "compiled units differ from Cargo inventory")
    # Cargo can build one projected identity more than once through distinct
    # dependency graphs. Preserve every invocation in the sealed report and
    # enforce the group's maximum, rather than discarding a duplicate row.
    # Source probes legitimately return negative compiler results. Their failure
    # is not a failed Cargo unit; the profiler's sealed reconciliation owns that
    # distinction and fatal instrumentation errors.
    return units


def validate_policy(policy: dict[str, Any]) -> None:
    """Keep every required surface and the first-release ceilings explicit."""
    require(type(policy["schema_version"]) is int and policy["schema_version"] == 1
            and type(policy["release_ceiling_bytes"]) is int and policy["release_ceiling_bytes"] == RELEASE_CEILING
            and type(policy["model_reduction_percent"]) is int and policy["model_reduction_percent"] == 25,
            "architecture memory policy was weakened")
    require(set(policy["model_packages"]) == MODEL_PACKAGES, "model compilation boundary is incomplete")
    require(isinstance(policy["runner"], str) and bool(policy["runner"].strip()), "pinned runner is missing")
    require(set(policy["surfaces"]) == set(SURFACES), "required memory surfaces differ")
    for name, entry in policy["surfaces"].items():
        require(entry["cargo_args"] == SURFACES[name], f"{name}: production selection differs")
        require(entry["state"] in ("measured", "pending"), f"{name}: invalid qualification state")
        if entry["state"] == "pending":
            require(isinstance(entry["reason"], str) and bool(entry["reason"].strip()), f"{name}: pending reason missing")
            continue
        baseline = entry["baseline"]
        require(all(sha256(baseline[key]) for key in (
            "report_sha256", "input_sha256", "compiler_measurements_sha256",
        )), f"{name}: baseline identity is incomplete")
        require(positive(baseline["model_peak_bytes"])
                and type(entry["model_limit_bytes"]) is int
                and entry["model_limit_bytes"] == baseline["model_peak_bytes"] * 3 // 4,
                f"{name}: model byte limit does not require a 25% reduction")
        unit_key(baseline["model_unit"])
        require(package_name(baseline["model_unit"]) == "iroha_data_model"
                and baseline["model_unit"]["kind"] == ["lib"]
                and not baseline["model_unit"]["profile"]["test"], f"{name}: model baseline unit differs")
        introduced = set()
        for limit in entry["introduced_units"]:
            key = unit_key(limit["unit"])
            require(key not in introduced and positive(limit["max_peak_bytes"])
                    and isinstance(limit["reason"], str) and bool(limit["reason"].strip()),
                    f"{name}: introduced unit needs a unique reviewed byte limit")
            require(isinstance(limit["compiler_controls"], list) and bool(limit["compiler_controls"]),
                    f"{name}: introduced compiler controls missing")
            for controls in limit["compiler_controls"]:
                require(set(controls) == {"codegen", "unstable", "target", "edition", "cfg", "sysroot"}
                        and all(isinstance(v, list) and all(isinstance(s, str) for s in v)
                                for v in controls.values()), f"{name}: introduced compiler controls missing")
            introduced.add(key)


def comparable_inputs(baseline: dict[str, Any], candidate: dict[str, Any]) -> None:
    """Permit the intended source/lock change while keeping measurement controls."""
    for field in ("cargo_args", "jobs", "profile_mode", "path", "selected_env", "cargo_cache",
                  "private_cargo_input", "rustup_tree", "private_rustup_input", "compiler_measurement"):
        require(baseline["input"][field] == candidate["input"][field], f"incomparable input: {field}")
    for tool in ("cargo", "rustc", "git"):
        # Private cache locations may differ. Compare actual executable and
        # launcher bytes and version records, not temporary absolute paths.
        identity = lambda report: {k: v for k, v in report["input"]["toolchain"][tool].items()
                                   if not k.endswith("_path")}
        require(identity(baseline) == identity(candidate), f"incomparable toolchain: {tool}")
    require(baseline["result"]["platform"] == candidate["result"]["platform"], "incomparable host platform")


def compare(policy: dict[str, Any], surface: str, baseline: dict[str, Any],
            candidate: dict[str, Any], runner: str) -> dict[str, Any]:
    """Apply exact retained-unit limits, reviewed new units and model/release caps."""
    validate_policy(policy)
    require(runner == policy["runner"], "run must use the policy's pinned runner")
    entry = policy["surfaces"][surface]
    require(entry["state"] == "measured", f"{surface}: baseline qualification is pending")
    old, new = validate_report(baseline), validate_report(candidate)
    pin = entry["baseline"]
    require(baseline["input_sha256"] == pin["input_sha256"]
            and baseline["result"]["compiler_measurements_sha256"] == pin["compiler_measurements_sha256"],
            "baseline does not match the reviewed measurement contract")
    model_key = unit_key(pin["model_unit"])
    require(model_key in old and old[model_key]["peak_rss_bytes"] == pin["model_peak_bytes"],
            "model baseline peak differs from the measured byte limit")
    expected_args = PROFILER.normalized_cargo_args(entry["cargo_args"], baseline["input"]["jobs"])
    require(baseline["input"]["cargo_args"] == expected_args, "baseline production Cargo selection differs")
    comparable_inputs(baseline, candidate)
    selected_package = entry["cargo_args"][entry["cargo_args"].index("-p") + 1]
    roots = {key for key, row in old.items() if package_name(row["unit"]) == selected_package
             and row["unit"]["kind"] != ["custom-build"]}
    require(bool(roots) and roots <= set(new), "selected package targets/features/profile were removed or changed")
    introduced = {unit_key(limit["unit"]): limit for limit in entry["introduced_units"]}
    require(not (set(introduced) & set(old)), "new-unit limits cannot override retained-unit baselines")
    require(set(introduced) <= set(new), "reviewed introduced unit is absent from candidate")
    failures: list[dict[str, Any]] = []
    limits: list[dict[str, Any]] = []
    for key, row in sorted(new.items()):
        unit, measured = row["unit"], row["peak_rss_bytes"]
        require(key in old or key in introduced, f"new compiler unit lacks reviewed byte limit: {unit['package_id']} {unit['name']}")
        prior = old.get(key)
        maximum = prior["peak_rss_bytes"] if prior else introduced[key]["max_peak_bytes"]
        controls = prior["compiler_controls"] if prior else introduced[key]["compiler_controls"]
        require(row["compiler_controls"] == controls,
                f"compiler codegen controls changed: {unit['package_id']} {unit['name']}")
        reasons = ["retained-unit baseline" if prior else "reviewed introduced unit"]
        if package_name(unit) in MODEL_PACKAGES:
            if unit["kind"] != ["custom-build"]:
                require(unit["profile"]["opt_level"] == pin["model_unit"]["profile"]["opt_level"],
                        "model optimization changed without separate memory/runtime qualification")
            maximum = min(maximum, entry["model_limit_bytes"])
            reasons.append("25% model reduction")
        if "--release" in entry["cargo_args"]:
            maximum = min(maximum, RELEASE_CEILING)
            reasons.append("13-GiB release ceiling")
        result = {"unit": unit, "peak_rss_bytes": measured, "max_peak_bytes": maximum,
                  "constraints": reasons, "invocations": row["invocations"]}
        limits.append(result)
        if measured > maximum:
            failures.append(result)
    require(any(package_name(row["unit"]) in MODEL_PACKAGES for row in new.values()), "model units are missing")
    probe_limits = []
    if "--release" in entry["cargo_args"]:
        for probe in candidate["result"]["compiler_measurements"]["probes"]:
            result = {"probe_source": probe["source"], "arguments_sha256": digest(probe["arguments"]),
                      "returncode": probe["returncode"], "peak_rss_bytes": probe["peak_rss_bytes"],
                      "max_peak_bytes": RELEASE_CEILING, "constraints": ["13-GiB release ceiling"]}
            probe_limits.append(result)
            if probe["peak_rss_bytes"] > RELEASE_CEILING:
                failures.append(result)
    return {
        "schema_version": 1, "surface": surface, "runner": runner,
        "baseline_input_sha256": baseline["input_sha256"], "candidate_input_sha256": candidate["input_sha256"],
        "candidate_source_sha256": candidate["input"]["source"]["sha256"],
        "candidate_execution_source_sha256": candidate["input"]["execution_source"]["sha256"],
        "candidate_lock_sha256": candidate["input"]["cargo_lock_sha256"],
        "passed": not failures, "failures": failures, "limits": limits, "probe_limits": probe_limits,
        "removed_units": [old[key]["unit"] for key in sorted(set(old) - set(new))],
    }


def compare_suite(policy: dict[str, Any], reports: dict[str, tuple[dict, dict]],
                  runner: str, candidate_identity: dict[str, str]) -> dict[str, Any]:
    """Require every surface and one consistent candidate source/lock revision."""
    validate_policy(policy)
    require(set(reports) == set(SURFACES), "complete qualification requires every memory surface")
    require(set(candidate_identity) == {"source_sha256", "execution_source_sha256", "cargo_lock_sha256", "git_revision"}
            and all(sha256(candidate_identity[key]) for key in (
                "source_sha256", "execution_source_sha256", "cargo_lock_sha256"))
            and isinstance(candidate_identity["git_revision"], str)
            and re.fullmatch(r"[0-9a-f]{40}", candidate_identity["git_revision"]) is not None,
            "candidate source/lock/revision identity is incomplete")
    results = {}
    for name in SURFACES:
        baseline, candidate = reports[name]
        inputs = candidate["input"]
        actual = {"source_sha256": inputs["source"]["sha256"],
                  "execution_source_sha256": inputs["execution_source"]["sha256"],
                  "cargo_lock_sha256": inputs["cargo_lock_sha256"], "git_revision": inputs["git_revision"]}
        require(actual == candidate_identity, f"{name}: candidate source/lock/revision differs from the suite")
        results[name] = compare(policy, name, baseline, candidate, runner)
    return {"schema_version": 1, "runner": runner, "candidate": candidate_identity,
            "passed": all(result["passed"] for result in results.values()), "surfaces": results}


def main(argv: list[str] | None = None) -> int:
    """Read trusted report pins and emit a deterministic comparison without Cargo."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=Path(__file__).resolve().parents[1] / "ci/compile_memory_budgets.json")
    scope = parser.add_mutually_exclusive_group(required=True)
    scope.add_argument("--surface", choices=SURFACES)
    scope.add_argument("--suite", type=Path, help="trusted manifest binding every surface to one candidate revision")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--candidate-sha256", help="report digest from the candidate's trusted artifact manifest")
    parser.add_argument("--runner", help="pinned runner identity supplied by the scheduling workflow")
    args = parser.parse_args(argv)
    try:
        policy = read_json(args.policy)
        validate_policy(policy)
        if args.suite:
            require(not any((args.baseline, args.candidate, args.candidate_sha256, args.runner)),
                    "suite inputs must come only from its manifest")
            manifest = read_json(args.suite)
            require(type(manifest["schema_version"]) is int and manifest["schema_version"] == 1
                    and set(manifest["reports"]) == set(SURFACES), "suite manifest must include every memory surface")
            reports = {}
            for name, paths in manifest["reports"].items():
                entry = policy["surfaces"][name]
                require(entry["state"] == "measured", f"{name}: baseline qualification is pending")
                require(sha256(paths["candidate_sha256"]), f"{name}: candidate report digest is missing")
                reports[name] = (
                    read_json(args.suite.parent / paths["baseline"], entry["baseline"]["report_sha256"]),
                    read_json(args.suite.parent / paths["candidate"], paths["candidate_sha256"]),
                )
            result = compare_suite(policy, reports, manifest["runner"], manifest["candidate"])
        else:
            require(all((args.baseline, args.candidate, args.candidate_sha256, args.runner)),
                    "surface comparison requires baseline, candidate, candidate-sha256 and runner")
            entry = policy["surfaces"][args.surface]
            require(entry["state"] == "measured", f"{args.surface}: baseline qualification is pending")
            baseline = read_json(args.baseline, entry["baseline"]["report_sha256"])
            candidate = read_json(args.candidate, args.candidate_sha256)
            result = compare(policy, args.surface, baseline, candidate, args.runner)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"check_compile_memory_budget: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
