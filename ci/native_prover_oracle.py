#!/usr/bin/env python3
"""Run the complete temporary M1a oracle on its actual native instruction target.

This is a CI correctness receipt, not native SDK admission or a performance
qualification. Compiler JSON selects every required harness; no benchmark or
fixture regeneration is included. Failed builds and tests retain their logs.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin")
TIMING_TEST = "timing::timing_sigma_k11_native_vs_vendored"
HARNESSES = (
    "iroha_plonk_oracle", "vendored_goldens", "native_prover_kats",
    "pasta_parity", "plonk_cs_parity",
)
REQUIRED_TESTS = {
    "iroha_plonk_oracle": {"export::tests::mixed_circuit_proofs_match_vendored"},
    "vendored_goldens": {
        "proof_parity::sigma_eq_k11_native_proofs_are_golden",
        "proof_parity::sigma_ep_k11_native_proofs_are_golden",
        "kagemusha_parity::sigma_k11_kagemusha_proofs_match_vendored",
        "succinct_parity::sigma_eq_succinct_matches_snark_verifier",
        "succinct_parity::sigma_ep_succinct_matches_snark_verifier",
        "succinct_parity::wide_eq_succinct_matches_snark_verifier",
        "succinct_parity::wide_ep_succinct_matches_snark_verifier",
    },
    "native_prover_kats": {"native_prover_release_params_match_fixture"},
    "pasta_parity": {"params::params_bytes_match_vendored_k15", "params::params_bytes_match_vendored_k16"},
    "plonk_cs_parity": {"cs_parity_pallas", "cs_parity_vesta"},
}


def compiler_harnesses(text: str, target: Path) -> dict[str, Path]:
    """Admit exactly the five successful compiler-produced test executables."""
    messages = [json.loads(line) for line in text.splitlines() if line.strip()]
    finished = [m for m in messages if m.get("reason") == "build-finished"]
    if len(finished) != 1 or finished[0].get("success") is not True:
        raise ValueError("missing successful Cargo build-finished record")
    found = {}
    for message in messages:
        if message.get("reason") != "compiler-artifact" or not message.get("executable"):
            continue
        name = message["target"]["name"]
        if name not in HARNESSES or not message["profile"]["test"] or name in found:
            raise ValueError(f"unexpected or duplicate executable: {name}")
        path = Path(message["executable"])
        if not path.is_absolute() or not path.resolve().is_relative_to(target.resolve()):
            raise ValueError(f"executable escaped the selected target directory: {name}")
        found[name] = path
    if set(found) != set(HARNESSES):
        raise ValueError(f"missing oracle harnesses: {sorted(set(HARNESSES) - set(found))}")
    return found


def test_command(binary: Path) -> list[str]:
    """Run ordinary and ignored correctness tests, excluding only timing cases."""
    return [str(binary), "--include-ignored", "--skip", TIMING_TEST, "--test-threads=2"]


def test_inventory(name: str, listing: str) -> set[str]:
    """Reject disabled oracle mode or missing release cases before execution."""
    names = {line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")}
    if not REQUIRED_TESTS[name].issubset(names):
        raise ValueError(f"missing required oracle-mode/release cases in {name}")
    return {test for test in names if TIMING_TEST not in test}


def complete_result(text: str, expected_count: int) -> bool:
    """Require every selected correctness case to pass without an ignored tail."""
    results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", text)
    return results == [(str(expected_count), "0", "0")] and expected_count > 0


def shipping_rejection(text: str, exit_code: int) -> bool:
    """Require the actual shipping consumer's const guard, not an unrelated build failure."""
    messages = [json.loads(line) for line in text.splitlines() if line.strip()]
    finished = [m for m in messages if m.get("reason") == "build-finished"]
    errors = [m for m in messages if m.get("reason") == "compiler-message"
              and m.get("message", {}).get("level") == "error"]
    return (exit_code == 101 and len(finished) == 1
            and finished[0].get("success") is False and len(errors) == 1
            and errors[0].get("target", {}).get("name") == "kaigi_zk"
            and (errors[0]["message"].get("code") or {}).get("code") == "E0080"
            and "iroha_plonk_oracle is test-only" in errors[0]["message"].get("message", "")
            and any(span.get("is_primary") is True
                    and Path(span.get("file_name", "")).parts[-4:] == ("crates", "kaigi_zk", "src", "lib.rs")
                    for span in errors[0]["message"].get("spans", [])))


def source_hashes(root: Path) -> dict[str, str]:
    """Pin tracked checkout bytes; verify them again after every harness ends."""
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).split(b"\0")
    return {os.fsdecode(path): hashlib.sha256((root / os.fsdecode(path)).read_bytes()).hexdigest()
            for path in paths if path and (root / os.fsdecode(path)).is_file()}


def main() -> int:
    """Build and execute native-target parity with retained natural outcomes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = root / "target/qualification/native-prover-oracle" / args.target
    evidence = output / "evidence"
    evidence.mkdir(parents=True, mode=0o700, exist_ok=False)
    temporary = output / "temporary"
    temporary.mkdir(mode=0o700)
    target = output / "cargo"
    environment = os.environ.copy()
    for key in ("IROHA_UPDATE_NATIVE_PROVER_KATS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "RUSTDOCFLAGS"):
        environment.pop(key, None)
    environment.update(RUSTFLAGS="--cfg iroha_plonk_oracle", TMPDIR=str(temporary),
                       CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="2")
    rust = subprocess.check_output(["rustc", "-vV"], cwd=root, env=environment, text=True)
    (evidence / "rustc.txt").write_text(rust)
    if f"host: {args.target}\n" not in rust:
        raise ValueError("runner toolchain host differs from the requested instruction target")
    before = source_hashes(root)
    (evidence / "source-before.json").write_text(json.dumps(before, sort_keys=True, indent=2))
    command = ["cargo", "test", "--locked", "--release", "--target", args.target,
               "-p", "iroha_plonk_oracle", "--lib"]
    for name in HARNESSES[1:]:
        command.extend(["--test", name])
    command.extend(["--no-run", "--message-format=json"])
    record = {"target": args.target, "command": command,
              "scope": "native instruction-target correctness only", "runs": []}
    with (evidence / "compiler.jsonl").open("w") as stdout, (evidence / "compiler.log").open("w") as stderr:
        build = subprocess.run(command, cwd=root, env=environment, stdout=stdout, stderr=stderr)
    record["build_exit"] = build.returncode
    (evidence / "result.json").write_text(json.dumps(record, indent=2))
    if build.returncode:
        return build.returncode
    harnesses = compiler_harnesses((evidence / "compiler.jsonl").read_text(), target)
    # The same oracle-enabled engine must be unusable by a production relation owner.
    # A missing dependency, parse error or successful build cannot satisfy this gate.
    rejection_command = ["cargo", "check", "--locked", "--release", "--target", args.target,
                         "-p", "kaigi_zk", "--lib", "--message-format=json"]
    with (evidence / "shipping-rejection.jsonl").open("w") as stdout, \
         (evidence / "shipping-rejection.log").open("w") as stderr:
        rejection = subprocess.run(rejection_command, cwd=root, env=environment, stdout=stdout, stderr=stderr)
    record["shipping_rejection"] = {
        "command": rejection_command, "natural_exit": rejection.returncode,
        "expected_const_failure": shipping_rejection(
            (evidence / "shipping-rejection.jsonl").read_text(), rejection.returncode),
    }
    (evidence / "result.json").write_text(json.dumps(record, indent=2))
    if not record["shipping_rejection"]["expected_const_failure"]:
        return 1
    for name in HARNESSES:
        source = harnesses[name]
        binary = output / name
        shutil.copy2(source, binary)
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        listing = subprocess.check_output([str(binary), "--list"], cwd=root, env=environment, text=True)
        (evidence / f"{name}-inventory.txt").write_text(listing)
        expected = test_inventory(name, listing)
        command = test_command(binary)
        with (evidence / f"{name}.log").open("w") as log:
            result = subprocess.run(command, cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT)
        record["runs"].append({"name": name, "binary_sha256": digest,
                               "command": command, "natural_exit": result.returncode,
                               "expected_tests": len(expected),
                               "complete": complete_result((evidence / f"{name}.log").read_text(), len(expected))})
        (evidence / "result.json").write_text(json.dumps(record, indent=2))
    after = source_hashes(root)
    (evidence / "source-after.json").write_text(json.dumps(after, sort_keys=True, indent=2))
    record["source_drift"] = sorted(p for p in before.keys() | after.keys() if before.get(p) != after.get(p))
    (evidence / "result.json").write_text(json.dumps(record, indent=2))
    return int(bool(record["source_drift"]) or any(run["natural_exit"] != 0 or not run["complete"] for run in record["runs"]))


if __name__ == "__main__":
    sys.exit(main())
