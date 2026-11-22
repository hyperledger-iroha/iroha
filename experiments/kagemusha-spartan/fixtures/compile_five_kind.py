#!/usr/bin/env python3
"""Build the five-kind public monetary program bundle on the canonical warm Cargo lanes."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

from export_compiler_originals import export, read_bounded
from norito import SchemaDescriptor, WireLayout, from_bytes_view

EXP = Path(__file__).resolve().parents[1]
REPO = EXP.parents[1]
COMPILER = ("native::engine::adversarial_tests::archive_remove_successor_tests::"
            "pending_state::authenticated::recursion::compiler::")
INPUTS = tuple("KAGEMUSHA_SPARTAN_" + key for key in (
    "DEVICE_ARTIFACTS", "DEVICE_ROOT", "ARCHIVE_RAW_ORIGINALS", "ARCHIVE_PREVIOUS",
))
FOUR_BUNDLE = "KAGEMUSHA_SPARTAN_FOUR_KIND_BUNDLE"
TEST_OUTPUT = "KAGEMUSHA_SPARTAN_TEST_OUTPUT"


def test_case(crate, name, env):
    cmd = ["cargo", "iroha-fast", "--", "test", "--manifest-path", str(crate / "Cargo.toml"),
           "--target-dir", str(crate / "target"), "--release", "--locked", "--offline",
           "--lib", name, "--", "--exact", "--ignored", "--nocapture"]
    print(f"Running {name}", flush=True)
    with subprocess.Popen(cmd, cwd=REPO, env=env, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, text=True) as proc:
        last_result = ""
        for line in proc.stdout:
            print(line, end="", flush=True)
            if line.startswith("test result:"):
                last_result = line
        if proc.wait() != 0 or not last_result.startswith("test result: ok. 1 passed; 0 failed;"):
            raise ValueError(f"expected one passing test for {name}")


def demo_paths(work, env, public_bundle):
    if public_bundle is None:
        old = work / "four-programs"
        test_case(EXP, "full::backend_chain_tests::compile_wallet_program_bundle",
                  {**env, TEST_OUTPUT: str(old), "KAGEMUSHA_SPARTAN_TEST_LAYOUT": "p18-s17"})
        public_bundle = old / "program-bundle.norito"
    if not public_bundle.is_file():
        raise ValueError(f"missing four-kind public program bundle {public_bundle}")
    artifacts, wallet, originals, seeds = (work / path for path in (
        "software-device", "wallet", "originals", "archive-seeds"))
    selected = dict(zip(INPUTS, map(str, (artifacts, wallet, originals, seeds)), strict=True))
    test_case(EXP, COMPILER + "issue_software_device_fixture_from_public_bundle",
              {**env, FOUR_BUNDLE: str(public_bundle), TEST_OUTPUT: str(artifacts)})
    for file in ("program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin"):
        if not (artifacts / file).is_file():
            raise ValueError(f"software fixture issuer did not produce {file}")
    wallet.mkdir(mode=0o700)
    test_case(EXP / "device/runtime", "host_tests::two_wallet_device_flow_on_host",
              {**env, INPUTS[0]: str(artifacts), INPUTS[1]: str(wallet)})
    export(wallet, originals)
    test_case(EXP, COMPILER + "compile_completed_archive_programs_group_four",
              {**env, **selected, TEST_OUTPUT: str(seeds)})
    return selected


def check_final(directory):
    bundle = read_bounded(directory / "program-bundle.norito", 10 * 1024 * 1024)
    schema = SchemaDescriptor.type_name("iroha.research.kagemusha_spartan.native.ProgramBundle.v2")
    from_bytes_view(bundle, schema=schema, layout=WireLayout.CURRENT)
    profile = json.loads(read_bounded(directory / "public-program-profile.json", 16 * 1024))
    if profile.get("schema") != "iroha.kagemusha.spartan.public-program-inspection.v2":
        raise ValueError("unexpected public inspection schema")
    for side in ("primary", "secondary"):
        pins = profile.get(side)
        if (not isinstance(pins, list) or len(pins) != 5 or
                not all(isinstance(p, str) and re.fullmatch(r"[0-9a-f]{64}", p) for p in pins) or
                len(set(pins)) != 5 or "0" * 64 in pins):
            raise ValueError(f"incomplete {side} program pins in the compared compiler profile")


def recheck_fixed_point(final, env):
    # Each native compiler invocation owns this new temporary directory.
    # Read the *last* complete round, including rounds 10–12 in numeric order.
    rounds = [round_no for round_no in range(1, 13)
              if final.with_suffix(f".round-{round_no}.unqualified.norito").is_file()]
    if not rounds or rounds != list(range(1, rounds[-1] + 1)):
        raise ValueError("missing consecutive fixed-point compiler outputs")
    prior = final.with_suffix(f".round-{rounds[-1]}.unqualified.norito")
    bundle = final / "program-bundle.norito"
    if prior.samefile(bundle):
        raise ValueError("the prior compiler round aliases the final output")
    profile = final.parent / "independent-profile.json"
    test_case(EXP, COMPILER + "fixed_point::rederive_four_variable_fixed_point_public_profile",
              {**env, INPUTS[3]: str(prior), "KAGEMUSHA_SPARTAN_ARCHIVE_FINAL": str(bundle),
               TEST_OUTPUT: str(profile)})
    if read_bounded(profile, 16 * 1024) != read_bounded(final / "public-program-profile.json", 16 * 1024):
        raise ValueError("independently derived program profile differs")


def compile_five_kind(output, public_bundle):
    output = output.parent.resolve() / output.name
    if os.path.lexists(output) or not output.parent.is_dir():
        raise ValueError("output must be a new directory in an existing parent")
    supplied = {key: os.environ[key] for key in INPUTS if os.environ.get(key)}
    if supplied and len(supplied) != len(INPUTS):
        missing = ", ".join(k for k in INPUTS if k not in supplied)
        raise ValueError(f"incomplete precomputed software fixture, missing {missing}")
    if any(not Path(value).is_dir() for value in supplied.values()):
        raise ValueError("every supplied software-fixture path must be a directory")
    if not supplied and public_bundle is not None and not public_bundle.is_file():
        raise ValueError(f"missing four-kind public program bundle {public_bundle}")

    # Each experiment stage gets its own explicit output and fixture inputs.
    # Host child-launch or measurement overrides from another run must not
    # change the selected three-launch software fixture.
    env = {k: v for k, v in os.environ.items() if not k.startswith("KAGEMUSHA_SPARTAN_")}
    work = Path(tempfile.mkdtemp(prefix=f".{output.name}-compiler-", dir=output.parent))
    try:
        chosen = ({k: str(Path(v).resolve()) for k, v in supplied.items()} if supplied else
                  demo_paths(work, env, public_bundle.resolve() if public_bundle else None))
        for name in ("archive-primary.program", "archive-secondary.program"):
            if not (Path(chosen[INPUTS[3]]) / name).is_file():
                raise ValueError(f"missing completed ArchiveSent seed {name}")
        final = work / "final"
        test_case(EXP, COMPILER + "fixed_point::compile_five_kind_wallet_program_bundle_group_four",
                  {**env, **chosen, TEST_OUTPUT: str(final)})
        check_final(final)
        recheck_fixed_point(final, env)
        if os.path.lexists(output):
            raise ValueError(f"another process created {output}")
        final.rename(output)
    except BaseException:
        print(f"Compiler inputs and diagnostics are retained at {work}", file=sys.stderr, flush=True)
        raise
    shutil.rmtree(work)
    print(f"Five-kind public bundle and inspection profile are in {output}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new directory for the final public originals")
    parser.add_argument("--four-kind-bundle", type=Path, default=os.environ.get(FOUR_BUNDLE),
                        help=f"public four-kind bundle to seed the software fixture (also {FOUR_BUNDLE})")
    args = parser.parse_args()
    try:
        compile_five_kind(args.output, args.four_kind_bundle)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
