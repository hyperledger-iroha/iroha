#!/usr/bin/env python3
"""Regenerate the frozen IVM baseline v1 fixtures from the pre-C10 anchor commit.

Purpose
    `crates/ivm/tests/fixtures/baseline_v1` holds what the installed baseline selection depends
    on: the ABI and gas-schedule descriptor bytes, the HostProfile `abi.v1` definition, the
    baseline catalog, a corpus of compiled and hand-assembled artifacts, and an execution oracle
    recorded on the anchor's own interpreter (`specs/runtime/ivm_baseline_and_c10_profiles.md`,
    phase 1). This script reproduces that directory:

      1. exports the anchor commit with `git archive` into a work directory outside the checkout;
      2. applies `anchor_accessors.patch` (two `#[doc(hidden)]` functions returning the descriptor
         bytes the anchor already hashes; no behaviour changes);
      3. copies the capture program (`capture/*.rs`, `kotodama/`) into the export as the `ivm`
         example `baseline_v1_capture`, and generates `anchor_core_admission.rs` from the text of
         the anchor's Core admission functions so the oracle needs no `iroha_core` build;
      4. builds that example with the anchor's own `scripts/cargo_fast.sh` and runs it;
      5. checks the captured hashes against the installed baseline's identities, installs the
         output into the fixture directory and writes `manifest.json` with the SHA-256 of every
         file.

Prerequisites
    Git with the anchor commit, the workspace Rust toolchain, a warm offline Cargo registry
    (`--locked --offline`), and about 15 GiB for the anchor target directory. Cargo runs once,
    under `nice`, with `--jobs` workers; on a shared host wait for free capacity first.

Safe defaults
    Writes only below `--work` and `--out`. In `--out` it replaces the generated directories
    (`artifacts/`, `descriptors/`, `oracle/`, `sources/`) and `manifest.json`, and only after the
    capture succeeded and its hashes matched; other files, such as the README, are kept. An
    existing export is reused unless `--fresh` is given. `--manifest-only` rewrites
    `manifest.json` from the files already in `--out` (after a README edit, for example) without
    building or running anything. The capture runs with `LANG=C` and without `IVM_*` developer
    variables, because compiler diagnostics follow the locale.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
ANCHOR = "77600a4840"
ANCHOR_SUCCESSOR = "c5511ebfc4"
EXAMPLE = "baseline_v1_capture"
GENERATED_DIRS = ("artifacts", "descriptors", "oracle", "sources")

# Identities of the installed baseline (`specs/runtime/ivm_baseline_and_c10_profiles.md`).
EXPECTED = {
    "abi_hash": "7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7",
    "gas_schedule_hash": "af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f",
    "host_profile_id_prefix": "d50c38a8",
    "selection_prefix": "56c23286",
    "header_size": 49,
}

# Pure Core admission functions the oracle applies: (anchor file, declaration prefix).
CORE_ADMISSION_FUNCTIONS = (
    (
        "crates/iroha_core/src/smartcontracts/ivm.rs",
        "pub(crate) fn admission_reason_from_vm_error(",
    ),
    (
        "crates/iroha_core/src/pipeline/overlay.rs",
        "pub(crate) fn validate_header_policy(",
    ),
    (
        "crates/iroha_core/src/smartcontracts/ivm.rs",
        "pub(crate) fn validate_cycle_ceiling(",
    ),
    (
        "crates/iroha_core/src/smartcontracts/ivm.rs",
        "pub(crate) fn validate_cycle_limits(",
    ),
    ("crates/iroha_core/src/smartcontracts/ivm.rs", "pub fn gas_limit_for_cycles("),
)
CONFIG_DEFAULTS = "crates/iroha_config/src/parameters/defaults.rs"
UPPER_BOUND_PATTERN = re.compile(
    r"pub const IVM_MAX_CYCLES_UPPER_BOUND: NonZeroU64 = nonzero!\(([0-9_]+)_u64\);"
)


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    """Run a command, echoing it, and fail on a non-zero exit."""
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def sha256(path: Path) -> str:
    """SHA-256 of a file as lower-case hex."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def export_anchor(tree: Path, commit: str, fresh: bool) -> None:
    """Materialize the anchor commit as plain files."""
    if fresh and tree.exists():
        shutil.rmtree(tree)
    if (tree / "Cargo.toml").exists():
        print(f"reusing the anchor export at {tree}")
        return
    tree.mkdir(parents=True, exist_ok=True)
    archive = subprocess.Popen(
        ["git", "-C", str(REPO_ROOT), "archive", commit], stdout=subprocess.PIPE
    )
    subprocess.run(["tar", "-x", "-C", str(tree)], stdin=archive.stdout, check=True)
    if archive.wait() != 0:
        raise SystemExit("git archive failed")


def apply_accessors(tree: Path) -> None:
    """Apply the descriptor accessor patch once."""
    marker = "baseline_capture_abi_surface_descriptor"
    if marker in (tree / "crates/ivm_abi/src/syscalls.rs").read_text(encoding="utf-8"):
        print("descriptor accessors are already applied")
        return
    run(
        ["patch", "-p1", "--forward", "-i", str(SCRIPT_DIR / "anchor_accessors.patch")],
        cwd=tree,
    )


def extract_function(text: str, prefix: str, origin: str) -> str:
    """Return one top-level function, from its declaration to its closing brace."""
    start = text.find(prefix)
    if start < 0 or text.find(prefix, start + 1) >= 0:
        raise SystemExit(f"{origin}: expected exactly one `{prefix}`")
    depth = 0
    for index in range(text.index("{", start), len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    raise SystemExit(f"{origin}: unbalanced braces after `{prefix}`")


def core_admission_module(tree: Path) -> str:
    """Rust module holding the anchor's Core admission functions, verbatim."""
    defaults = (tree / CONFIG_DEFAULTS).read_text(encoding="utf-8")
    bounds = UPPER_BOUND_PATTERN.findall(defaults)
    if len(bounds) != 1:
        raise SystemExit(f"{CONFIG_DEFAULTS}: IVM_MAX_CYCLES_UPPER_BOUND not found once")
    parts = [
        "//! Core admission of the anchor commit, extracted verbatim by\n"
        "//! `scripts/ivm_baseline_v1/capture.py` (only `pub(crate)` became `pub`). Do not edit.\n"
        "use std::num::NonZeroU64;\n\n"
        "use iroha_crypto::Hash;\n"
        "use iroha_data_model::executor::{\n"
        "    ArtifactAbiHashMismatchInfo, IvmAdmissionError, MaxCyclesExceedsFuelInfo,\n"
        "    MaxCyclesExceedsUpperBoundInfo,\n"
        "};\n\n"
        f"/// `pipeline::IVM_MAX_CYCLES_UPPER_BOUND` in `{CONFIG_DEFAULTS}`.\n"
        f"pub const IVM_MAX_CYCLES_UPPER_BOUND: u64 = {bounds[0]};\n"
    ]
    for origin, prefix in CORE_ADMISSION_FUNCTIONS:
        body = extract_function((tree / origin).read_text(encoding="utf-8"), prefix, origin)
        parts.append(f"\n/// From `{origin}`.\n" + body.replace("pub(crate) fn", "pub fn", 1) + "\n")
    return "".join(parts)


def install_capture(tree: Path) -> Path:
    """Copy the capture program into the export and return its directory."""
    target = tree / "crates/ivm/examples" / EXAMPLE
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(SCRIPT_DIR / "capture", target)
    shutil.copytree(SCRIPT_DIR / "kotodama", target / "kotodama")
    (target / "anchor_core_admission.rs").write_text(
        core_admission_module(tree), encoding="utf-8"
    )
    return target


def build(tree: Path, target_dir: Path, jobs: int) -> Path:
    """Build the capture example with the anchor's own build wrapper."""
    run(
        [
            "nice",
            "-n",
            "5",
            "scripts/cargo_fast.sh",
            "--stable-local-metadata",
            "--no-incremental",
            "--jobs",
            str(jobs),
            "--target-dir",
            str(target_dir),
            "--",
            "build",
            "-p",
            "ivm",
            "--example",
            EXAMPLE,
            "--locked",
            "--offline",
        ],
        cwd=tree,
    )
    return target_dir / "debug/examples" / EXAMPLE


def capture_environment() -> dict[str, str]:
    """Environment of the capture run: compiler diagnostics follow the locale, so pin it."""
    env = {
        key: value
        for key, value in os.environ.items()
        if key not in {"LANGUAGE", "LC_MESSAGES"} and not key.startswith("IVM_")
    }
    env.update({"LANG": "C", "LC_ALL": "C", "RUST_BACKTRACE": "1"})
    return env


def check_frozen(staging: Path) -> dict[str, object]:
    """Compare the captured identities with the installed baseline's."""
    frozen = json.loads((staging / "descriptors/frozen_values.json").read_text(encoding="utf-8"))
    captured = {
        "abi_hash": frozen["abi_v1"]["abi_hash"],
        "gas_schedule_hash": frozen["gas_schedule_v1"]["schedule_hash"],
        "host_profile_id": frozen["host_profile_abi_v1"]["definition_id"],
        "selection": frozen["runtime_baseline_catalog"]["selection"],
        "header_size": frozen["header"]["size"],
    }
    failures = []
    if captured["abi_hash"] != EXPECTED["abi_hash"]:
        failures.append("ABI hash")
    if captured["gas_schedule_hash"] != EXPECTED["gas_schedule_hash"]:
        failures.append("gas schedule hash")
    if not captured["host_profile_id"].startswith(EXPECTED["host_profile_id_prefix"]):
        failures.append("HostProfile definition id")
    if not captured["selection"].startswith(EXPECTED["selection_prefix"]):
        failures.append("baseline selection id")
    if captured["header_size"] != EXPECTED["header_size"]:
        failures.append("header size")
    for name, value in captured.items():
        print(f"captured {name}: {value}")
    if failures:
        raise SystemExit("captured values differ from the installed baseline: " + ", ".join(failures))
    return captured


def write_manifest(out: Path, commit: str, rustc: str, frozen: dict[str, object]) -> None:
    """Write `manifest.json`: provenance and the SHA-256 of every other fixture file."""
    files = []
    for path in sorted(out.rglob("*")):
        if path.is_file() and path.name != "manifest.json":
            files.append(
                {
                    "path": path.relative_to(out).as_posix(),
                    "bytes": path.stat().st_size,
                    "sha256": sha256(path),
                }
            )
    manifest = {
        "format": 1,
        "anchor_commit": commit,
        "anchor_is_parent_of": ANCHOR_SUCCESSOR,
        "generated_by": "scripts/ivm_baseline_v1/capture.py",
        "rustc": rustc,
        "frozen": frozen,
        "files": files,
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    total = sum(entry["bytes"] for entry in files)
    print(f"manifest: {len(files)} files, {total} bytes")


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--anchor", default=ANCHOR, help="anchor commit (default: %(default)s)")
    parser.add_argument(
        "--work",
        type=Path,
        default=Path.home() / "dev/.taira-testnet-build-targets/ivm-anchor",
        help="work directory outside the checkout holding tree/, target/ and out/",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=REPO_ROOT / "crates/ivm/tests/fixtures/baseline_v1",
        help="fixture directory to regenerate",
    )
    parser.add_argument("--jobs", type=int, default=4, help="Cargo workers (default: 4)")
    parser.add_argument("--fresh", action="store_true", help="discard an existing anchor export")
    parser.add_argument(
        "--skip-build", action="store_true", help="reuse the capture binary already built"
    )
    parser.add_argument(
        "--manifest-only",
        action="store_true",
        help="only rewrite manifest.json from the files already in --out",
    )
    args = parser.parse_args()

    commit = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "rev-parse", f"{args.anchor}^{{commit}}"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    out = args.out.resolve()
    if args.manifest_only:
        previous = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
        if previous["anchor_commit"] != commit:
            raise SystemExit("the existing manifest names a different anchor commit")
        write_manifest(out, commit, previous["rustc"], previous["frozen"])
        return 0

    work = args.work.resolve()
    if REPO_ROOT in work.parents or work == REPO_ROOT:
        raise SystemExit("--work must be outside the checkout")
    tree = work / "tree"
    export_anchor(tree, commit, args.fresh)
    apply_accessors(tree)
    capture_dir = install_capture(tree)
    binary = work / "target/debug/examples" / EXAMPLE
    if not args.skip_build:
        binary = build(tree, work / "target", args.jobs)
    rustc = subprocess.run(
        ["rustc", "--version"], cwd=tree, check=True, capture_output=True, text=True
    ).stdout.strip()

    staging = work / "out"
    if staging.exists():
        shutil.rmtree(staging)
    run(
        [
            str(binary),
            "--anchor-root",
            str(tree),
            "--capture-dir",
            str(capture_dir),
            "--anchor-commit",
            commit,
            "--out",
            str(staging),
        ],
        cwd=tree,
        env=capture_environment(),
    )
    shutil.copyfile(
        capture_dir / "anchor_core_admission.rs",
        staging / "descriptors/core_admission_anchor.rs.txt",
    )
    frozen = check_frozen(staging)

    out.mkdir(parents=True, exist_ok=True)
    for name in GENERATED_DIRS:
        if (out / name).exists():
            shutil.rmtree(out / name)
        shutil.copytree(staging / name, out / name)
    write_manifest(out, commit, rustc, frozen)
    return 0


if __name__ == "__main__":
    sys.exit(main())
