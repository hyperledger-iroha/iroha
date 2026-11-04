#!/usr/bin/env python3
"""Derive and attest a typed-query measurement driver without editing either revision.

The driver selects the typed-query dispatch and grants its existing fixture
reader CanReadAllLedgerData before raw-response setup or timing. The exact same
derivation applies to both revisions; every other setup byte, assertion and timed
body is preserved. Cargo still builds the revision-native runtime and launches
Criterion with its normal environment. This is ordinary same-job CI evidence,
not release signing or a sandbox against code already able to modify the runner.
Original source/lock admission remains mandatory.
"""

from __future__ import annotations

import argparse
import functools
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys
import uuid

SOURCE = Path("crates/iroha_core/benches/queries.rs")
SCRIPT = Path(__file__).resolve()
PLAN_ENV = "IROHA_KOTODAMA_QUERY_PLAN"
SCHEMA = "iroha.kotodama.derived-query-driver.v2"
# Bind the entire untimed fixture from the approved baseline, including its
# authority, all five 64-entity inventories and raw QueryResponse encoding.
SETUP_START = "fn build_state_for_typed_core_query_pages()"
SETUP_END = "fn bench_typed_core_query_pages("
SETUP_SHA256 = "99f8129019d5881860dbc19c02fbf4411efd137c6da39b7ceae21187fdc963e8"
WORLD_ARGUMENT = """        World::with_assets(domains, accounts, asset_definitions, assets, nfts),
"""
AUTHORIZED_WORLD_ARGUMENT = """        {
            let mut world =
                World::with_assets(domains, accounts, asset_definitions, assets, nfts);
            world.account_permissions_mut_for_testing().insert(
                authority.clone(),
                [iroha_executor_data_model::permission::query::CanReadAllLedgerData.into()]
                    .into_iter()
                    .collect(),
            );
            world
        },
"""
# An exact dispatch contract: additions/reordering require a reviewed driver update.
DISPATCH = """    bench_find_accounts_small(&mut c);
    bench_find_accounts_large(&mut c);
    bench_find_accounts_filter_id_literal(&mut c);
    bench_find_assets_iter(&mut c);
    bench_find_assets_filter_account(&mut c);
    bench_find_assets_filter_quantity(&mut c);
    bench_find_assets_filter_definition_literal(&mut c);
    bench_find_assets_filter_domain_literal(&mut c);
    bench_find_accounts_with_asset_literal(&mut c);
    bench_find_domains_iter(&mut c);
    bench_find_domains_sort(&mut c);
    bench_find_asset_defs_iter(&mut c);
    bench_find_triggers_iter(&mut c);
    bench_find_active_trigger_ids_iter(&mut c);
    bench_find_accounts_sort_id(&mut c);
    bench_find_accounts_paginate(&mut c);
    bench_typed_core_query_pages(&mut c);
    bench_snapshot_vs_live_find_domains_first_batch(&mut c);
    bench_snapshot_vs_live_find_assets_first_batch(&mut c);
    bench_snapshot_sorted_asset_defs_first_batch(&mut c);
"""
MAIN = """fn main() {
    // Silence IVM banner for benches that may trigger VM init in inner paths.
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
""" + DISPATCH + "    c.final_summary();\n}\n"
CARGO_ARGS = [
    "bench",
    "--locked",
    "--jobs",
    "1",
    "-p",
    "iroha_core",
    "--features",
    "bench,iroha-core-tests",
    "--bench",
    "queries",
]


@functools.lru_cache(maxsize=1)
def policy() -> dict:
    """Load the candidate's existing inventory and source/lock admission rules."""
    return runpy.run_path(str(SCRIPT.with_name("check_kotodama_perf.py")))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha(path: Path) -> str:
    require(path.is_file() and not path.is_symlink(), f"not a regular file: {path}")
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def read_json(path: Path) -> dict:
    sha(path)
    value = json.loads(path.read_text())
    require(isinstance(value, dict), f"expected an object: {path}")
    return value


def write_json(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as destination:
        json.dump(value, destination, indent=2, sort_keys=True)
        destination.write("\n")


def typed_setup(text: str) -> str:
    """Select the unique complete fixture immediately before its benchmark."""
    require(
        text.count(SETUP_START) == text.count(SETUP_END) == 1,
        "typed-query setup boundaries are not unique",
    )
    start, end = text.index(SETUP_START), text.index(SETUP_END)
    require(start < end, "typed-query setup boundaries are reversed")
    return text[start:end]


def derive(original: bytes) -> bytes:
    """Apply only the reviewed dispatch and untimed fixture-reader grant."""
    text = original.decode("utf-8")
    require(
        text.count("fn main()") == 1 and text.endswith(MAIN),
        "queries main differs from the reviewed dispatch contract",
    )
    require(text.count(DISPATCH) == 1, "queries dispatch is not unique")
    setup = typed_setup(text)
    require(
        hashlib.sha256(setup.encode()).hexdigest() == SETUP_SHA256,
        "typed-query setup differs from the reviewed fixture contract",
    )
    require(setup.count(WORLD_ARGUMENT) == 1, "typed-query world is not unique")
    authorized_setup = setup.replace(WORLD_ARGUMENT, AUTHORIZED_WORLD_ARGUMENT, 1)
    return text.replace(setup, authorized_setup, 1).replace(
        DISPATCH, "    bench_typed_core_query_pages(&mut c);\n", 1
    ).encode()


def source_contract(root: Path, derived: Path) -> dict:
    """Bind all 46 original bodies and the exact authorized fixture derivation."""
    p = policy()
    originals = p["_benchmark_sources_at"](root)
    changed = (*originals[:2], derived)
    require(
        derived.read_bytes() == derive((root / SOURCE).read_bytes()),
        "derived driver does not equal exact dispatch-and-permission derivation",
    )
    p["validate_revision_inventories"](
        base_sources=originals, candidate_sources=changed
    )
    before = p["validate_comparable_timed_bodies"](originals, "original")
    after = p["validate_comparable_timed_bodies"](changed, "derived")
    require(before == after, "derived timed bodies changed")
    return {
        "original_sha256": sha(root / SOURCE),
        "derived_sha256": sha(derived),
        "original_setup_sha256": SETUP_SHA256,
        "derived_setup_sha256": hashlib.sha256(
            typed_setup(derived.read_text()).encode()
        ).hexdigest(),
        "fixture_permission": {
            "authority_seed": "typed-query-authority",
            "permission": "CanReadAllLedgerData",
            "grant_count": 1,
            "phase": "before raw-response setup and timed queries",
        },
        "timed_body_sha256": {
            name: hashlib.sha256(body.encode()).hexdigest()
            for name, body in sorted(before.items())
        },
        "derivation": (
            "select typed main dispatch and grant the existing fixture reader "
            "CanReadAllLedgerData; all other setup bytes, assertions and timed "
            "bodies remain identical"
        ),
    }


def option(args: list[str], name: str) -> str | None:
    matches = []
    for i, arg in enumerate(args):
        if arg == name:
            require(i + 1 < len(args), f"missing value for {name}")
            matches.append(args[i + 1])
        elif arg.startswith(name + "="):
            matches.append(arg[len(name) + 1 :])
    require(len(matches) <= 1, f"duplicate compiler option {name}")
    return matches[0] if matches else None


def codegen(args: list[str], key: str) -> str | None:
    values = [args[i + 1] for i, arg in enumerate(args[:-1]) if arg == "-C"]
    values += [arg[2:] for arg in args if arg.startswith("-C") and arg != "-C"]
    matches = [
        value.split("=", 1)[1] for value in values if value.startswith(key + "=")
    ]
    require(len(matches) == 1, f"expected one compiler codegen option {key}")
    return matches[0]


def admitted_compiler(
    plan: dict,
    rustc: str,
    args: list[str],
    cwd: Path,
    manifest_dir: str | None,
    package: str | None,
) -> tuple[int, Path, Path]:
    """Admit only this revision's native, optimized queries bench invocation."""
    root = Path(plan["revision_root"])
    require(Path(rustc).resolve() == Path(plan["rustc"]), "foreign compiler")
    require(
        sha(Path(rustc).resolve()) == plan["rustc_sha256"], "compiler bytes changed"
    )
    require(
        manifest_dir == str(root / "crates/iroha_core") and package == "iroha_core",
        "foreign compiler package",
    )
    require(
        not any(arg.startswith("@") for arg in args),
        "compiler response files are not admitted",
    )
    sources = [(i, arg) for i, arg in enumerate(args) if arg.endswith(".rs")]
    require(len(sources) == 1, "expected exactly one Rust source input")
    index, source = sources[0]
    require((cwd / source).resolve() == root / SOURCE, "foreign compiler source")
    require(
        option(args, "--crate-name") == "queries"
        and option(args, "--crate-type") in (None, "bin"),
        "foreign compiler target",
    )
    require(
        option(args, "--target") is None and "--test" not in args,
        "foreign compiler platform or harness",
    )
    require(
        option(args, "--emit") == "dep-info,link", "queries requires linked dep-info"
    )
    require(codegen(args, "opt-level") == "3", "queries optimization changed")
    require(
        'feature="bench"' in args and 'feature="iroha-core-tests"' in args,
        "queries feature selection changed",
    )
    output = Path(option(args, "--out-dir") or "").resolve()
    require(
        output == Path(plan["target"]) / "release/deps",
        "foreign compiler output directory",
    )
    suffix = codegen(args, "extra-filename")
    require(
        re.fullmatch(r"-[0-9a-f]{16}", suffix or "") is not None,
        "invalid queries executable suffix",
    )
    return index, output / ("queries" + suffix), output / ("queries" + suffix + ".d")


def validate_compilation(
    directory: Path, plan: dict, artifact: dict, *, archived: bool = False
) -> dict:
    """Reject stale/faked receipts, wrong artifacts, and original-source dep-info."""
    receipt = read_json(directory / "compiler.json")
    require(
        receipt["nonce"] == plan["nonce"] and receipt["returncode"] == 0,
        "missing fresh successful compiler invocation",
    )
    require(
        receipt["driver_sha256"] == sha(SCRIPT) == plan["driver_sha256"],
        "compiler driver changed",
    )
    args = receipt["original_args"]
    index, executable, depinfo = admitted_compiler(
        plan,
        receipt["rustc"],
        args,
        Path(receipt["cwd"]),
        receipt["manifest_dir"],
        receipt["package"],
    )
    changed = args.copy()
    changed[index] = str(directory / "queries.rs")
    require(
        receipt["derived_args"] == changed,
        "compiler arguments changed beyond source substitution",
    )
    require(
        receipt["contract"]
        == source_contract(Path(plan["revision_root"]), directory / "queries.rs"),
        "compiler source contract changed",
    )
    require(
        artifact.get("fresh") is False
        and artifact.get("executable") == str(executable),
        "missing freshly compiled selected artifact",
    )
    binary_bytes = directory / "measured-executable" if archived else executable
    depinfo_bytes = directory / "measured-dep-info" if archived else depinfo
    require(
        receipt["executable"] == str(executable)
        and receipt["executable_sha256"] == sha(binary_bytes),
        "compiled executable hash mismatch",
    )
    require(
        receipt["depinfo"] == str(depinfo)
        and receipt["depinfo_sha256"] == sha(depinfo_bytes),
        "compiler dep-info hash mismatch",
    )
    # rustc emits an empty dependency rule for every input. Require the exact
    # per-invocation path, including its fresh nonce, not a substring match.
    escaped = str(directory / "queries.rs").replace("\\", "\\\\").replace(" ", "\\ ")
    require(
        escaped + ":" in depinfo_bytes.read_text().splitlines(),
        "dep-info does not bind the fresh derived source",
    )
    return receipt


def wrapper(rustc: str, args: list[str]) -> int:
    """Delegate unchanged except for the one strictly admitted bench source."""
    plan_path = Path(os.environ[PLAN_ENV])
    plan = read_json(plan_path)
    root = Path(plan["revision_root"])
    selected = option(args, "--crate-name") == "queries"
    has_source = any(
        arg.endswith(".rs") and (Path.cwd() / arg).resolve() == root / SOURCE
        for arg in args
    )
    if not selected and not has_source:
        return subprocess.call([rustc, *args])
    directory = plan_path.parent
    index, executable, depinfo = admitted_compiler(
        plan,
        rustc,
        args,
        Path.cwd(),
        os.environ.get("CARGO_MANIFEST_DIR"),
        os.environ.get("CARGO_PKG_NAME"),
    )
    require(
        not (directory / "compiler.json").exists(),
        "unexpected repeated queries compilation",
    )
    contract = source_contract(root, directory / "queries.rs")
    changed = args.copy()
    changed[index] = str(directory / "queries.rs")
    result = subprocess.call([rustc, *changed])
    if result:
        return result
    write_json(
        directory / "compiler.json",
        {
            "nonce": plan["nonce"],
            "driver_sha256": sha(SCRIPT),
            "rustc": rustc,
            "cwd": str(Path.cwd()),
            "manifest_dir": os.environ.get("CARGO_MANIFEST_DIR"),
            "package": os.environ.get("CARGO_PKG_NAME"),
            "original_args": args,
            "derived_args": changed,
            "returncode": result,
            "contract": contract,
            "executable": str(executable),
            "executable_sha256": sha(executable),
            "depinfo": str(depinfo),
            "depinfo_sha256": sha(depinfo),
        },
    )
    return 0


def selected_artifact(messages: list[dict], root: Path) -> dict:
    selected = [
        item
        for item in messages
        if item.get("reason") == "compiler-artifact"
        and item.get("target", {}).get("name") == "queries"
    ]
    require(len(selected) == 1, "expected one queries Cargo artifact")
    artifact = selected[0]
    require(
        artifact["target"].get("kind") == ["bench"]
        and artifact["target"].get("crate_types") == ["bin"]
        and artifact["target"].get("src_path") == str(root / SOURCE)
        and artifact.get("manifest_path") == str(root / "crates/iroha_core/Cargo.toml"),
        "foreign Cargo benchmark artifact",
    )
    require(
        artifact.get("profile", {}).get("opt_level") == "3",
        "foreign Cargo benchmark profile",
    )
    return artifact


def cargo(args: list[str], root: Path, env: dict) -> dict:
    """Keep Cargo's benchmark launcher, runtime environment and failure status."""
    messages = []
    with subprocess.Popen(
        ["cargo", *args], cwd=root, env=env, stdout=subprocess.PIPE, text=True
    ) as child:
        assert child.stdout is not None
        for line in child.stdout:
            print(line, end="", flush=True)
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                messages.append(value)
        status = child.wait()
    if status:
        raise subprocess.CalledProcessError(status, ["cargo", *args])
    return selected_artifact(messages, root)


def admit_sources(
    root: Path, candidate: Path, expected_candidate: str, label: str
) -> None:
    p = policy()
    p["validate_candidate_provenance"](candidate, expected_candidate)
    if label == "base":
        p["validate_baseline_provenance"](root, p["_benchmark_sources_at"](candidate))
    else:
        require(root == candidate, "candidate measurement root mismatch")


def measure(
    root: Path, candidate: Path, expected_candidate: str, label: str, output: Path
) -> None:
    root, candidate = root.resolve(strict=True), candidate.resolve(strict=True)
    target = Path(os.environ["CARGO_TARGET_DIR"]).resolve(strict=True)
    output = output.resolve()
    require(
        output == target / "kotodama-query-drivers" / label,
        "foreign driver evidence directory",
    )
    require(not output.exists(), "driver evidence already exists; fresh run required")
    require(
        not target.is_relative_to(root) and not target.is_relative_to(candidate),
        "derived driver must live outside both revision checkouts",
    )
    require(
        os.environ.get("CARGO_BUILD_JOBS") == "1"
        and os.environ.get("CARGO_INCREMENTAL") == "0"
        and os.environ.get("RUSTUP_TOOLCHAIN") == "1.99.1",
        "benchmark build environment changed",
    )
    require(
        not os.environ.get("RUSTC_WRAPPER")
        and not os.environ.get("RUSTC_WORKSPACE_WRAPPER"),
        "unreviewed compiler wrapper",
    )
    admit_sources(root, candidate, expected_candidate, label)
    rustc = Path(
        subprocess.check_output(["rustup", "which", "rustc"], text=True).strip()
    ).resolve(strict=True)
    nonce = uuid.uuid4().hex
    directory = output / nonce
    directory.mkdir(parents=True)
    (directory / "queries.rs").write_bytes(derive((root / SOURCE).read_bytes()))
    build_environment = {
        key: value
        for key, value in os.environ.items()
        if key
        in {
            "CARGO_BUILD_JOBS",
            "CARGO_INCREMENTAL",
            "RUSTUP_TOOLCHAIN",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
        }
        or key.startswith("CARGO_PROFILE_")
    }
    plan = {
        "schema": SCHEMA,
        "build_environment": build_environment,
        "nonce": nonce,
        "label": label,
        "revision_root": str(root),
        "revision": policy()["_git_head"](root),
        "lock_sha256": sha(root / "Cargo.lock"),
        "target": str(target),
        "driver_sha256": sha(SCRIPT),
        "rustc": str(rustc),
        "rustc_sha256": sha(rustc),
        "contract": source_contract(root, directory / "queries.rs"),
    }
    write_json(directory / "plan.json", plan)
    environment = {
        **os.environ,
        "RUSTC": str(rustc),
        "RUSTC_WRAPPER": str(SCRIPT),
        PLAN_ENV: str(directory / "plan.json"),
    }
    prepare = [*CARGO_ARGS, "--no-run", "--message-format=json"]
    artifact = cargo(prepare, root, environment)
    if artifact.get("fresh") is True and not (directory / "compiler.json").exists():
        # Only this exact admitted Cargo output is disposable. Keep the warm
        # dependency graph and all other binaries. A missing executable forces
        # Cargo to rebuild this bench even if an unwrapped version was cached.
        executable = Path(artifact["executable"])
        require(
            executable.parent == target / "release/deps"
            and re.fullmatch(r"queries-[0-9a-f]{16}", executable.name) is not None,
            "foreign cached queries executable",
        )
        sha(executable)
        executable.unlink()
        artifact = cargo(prepare, root, environment)
    validate_compilation(directory, plan, artifact)
    command = [
        *CARGO_ARGS,
        "--message-format=json",
        "--",
        "typed_core_query_",
        "--save-baseline",
        label,
    ]
    measured = cargo(command, root, environment)
    require(
        measured.get("fresh") is True
        and measured.get("executable") == artifact["executable"],
        "measurement did not launch the admitted compiled artifact",
    )
    validate_compilation(directory, plan, artifact)
    # Preserve the admitted bytes: the next revision may reuse the same Cargo
    # output filename. These are explicitly derived benchmark artifacts.
    compiled = read_json(directory / "compiler.json")
    shutil.copyfile(compiled["executable"], directory / "measured-executable")
    shutil.copyfile(compiled["depinfo"], directory / "measured-dep-info")
    validate_compilation(directory, plan, artifact, archived=True)
    admit_sources(root, candidate, expected_candidate, label)
    write_json(
        output / "measurement.json",
        {
            "schema": SCHEMA,
            "nonce": nonce,
            "preparation_artifact": artifact,
            "measurement_artifact": measured,
            "cargo_argv": command,
            "plan_sha256": sha(directory / "plan.json"),
            "compiler_receipt_sha256": sha(directory / "compiler.json"),
        },
    )


def validate_pair(
    output: Path, baseline: Path, candidate: Path, expected_candidate: str
) -> dict:
    """Re-admit both independently derived drivers before any gate can qualify."""
    output = output.resolve(strict=True)
    require(output.name == "kotodama-query-drivers", "foreign paired driver directory")
    summaries = {}
    compile_environment = None
    for label, root in (
        ("base", baseline.resolve(strict=True)),
        ("candidate", candidate.resolve(strict=True)),
    ):
        measured = read_json(output / label / "measurement.json")
        nonce = measured["nonce"]
        require(
            re.fullmatch(r"[0-9a-f]{32}", nonce) is not None, "invalid driver nonce"
        )
        directory = output / label / nonce
        plan = read_json(directory / "plan.json")
        require(
            plan["schema"] == measured["schema"] == SCHEMA,
            "unknown driver evidence schema",
        )
        environment = plan["build_environment"]
        require(
            environment.get("CARGO_BUILD_JOBS") == "1"
            and environment.get("CARGO_INCREMENTAL") == "0"
            and environment.get("RUSTUP_TOOLCHAIN") == "1.99.1",
            "driver build environment changed",
        )
        identity = (environment, plan["rustc"], plan["rustc_sha256"])
        if compile_environment is not None:
            require(
                identity == compile_environment, "paired compiler environment differs"
            )
        compile_environment = identity
        require(
            plan["nonce"] == nonce and plan["label"] == label, "wrong driver invocation"
        )
        require(plan["revision_root"] == str(root), "foreign revision root")
        require(
            plan["target"] == str(output.parent) and not output.is_relative_to(root),
            "foreign driver target or generated source inside revision",
        )
        require(
            plan["revision"] == policy()["_git_head"](root)
            and plan["lock_sha256"] == sha(root / "Cargo.lock"),
            "driver revision or lock drift",
        )
        require(
            plan["contract"] == source_contract(root, directory / "queries.rs"),
            "driver source drift",
        )
        require(
            measured["plan_sha256"] == sha(directory / "plan.json")
            and measured["compiler_receipt_sha256"] == sha(directory / "compiler.json"),
            "driver receipt hash mismatch",
        )
        require(
            measured["cargo_argv"]
            == [
                *CARGO_ARGS,
                "--message-format=json",
                "--",
                "typed_core_query_",
                "--save-baseline",
                label,
            ],
            "measurement command changed",
        )
        artifact = selected_artifact([measured["preparation_artifact"]], root)
        current = selected_artifact([measured["measurement_artifact"]], root)
        require(
            current["fresh"] is True
            and current["executable"] == artifact["executable"],
            "measurement artifact mismatch",
        )
        receipt = validate_compilation(directory, plan, artifact, archived=True)
        admit_sources(root, candidate, expected_candidate, label)
        summaries[label] = {
            "schema": SCHEMA,
            "revision": plan["revision"],
            "lock_sha256": plan["lock_sha256"],
            "contract": plan["contract"],
            "driver_sha256": plan["driver_sha256"],
            "measurement_sha256": sha(output / label / "measurement.json"),
            "compiler_receipt_sha256": measured["compiler_receipt_sha256"],
            "executable_sha256": receipt["executable_sha256"],
            "depinfo_sha256": receipt["depinfo_sha256"],
        }
    return summaries


def main() -> int:
    try:
        if PLAN_ENV in os.environ:
            return wrapper(sys.argv[1], sys.argv[2:])
        parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
        parser.add_argument("--revision-root", required=True, type=Path)
        parser.add_argument("--candidate-root", required=True, type=Path)
        parser.add_argument("--expected-candidate-commit", required=True)
        parser.add_argument("--label", required=True, choices=("base", "candidate"))
        parser.add_argument("--output", required=True, type=Path)
        args = parser.parse_args()
        measure(
            args.revision_root,
            args.candidate_root,
            args.expected_candidate_commit,
            args.label,
            args.output,
        )
        return 0
    except subprocess.CalledProcessError as error:
        return error.returncode
    except (ValueError, OSError, KeyError, RuntimeError) as error:
        print(f"error: derived query driver: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
