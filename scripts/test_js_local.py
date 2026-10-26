#!/usr/bin/env python3
"""Build a current Debug N-API artifact and run the complete JavaScript unit profile.

Requires Python 3.12, full Xcode, stock Rust 1.93.1, Node and the repository's
locked dependencies. Select an existing Cargo lane explicitly; this runner
never creates a Cargo target. Native custody, logs and test output remain in a
private target/qualification child. This local-unit workflow grants no package,
release or network qualification. No child is killed, timed out or retried.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

import native_js_local_unit as unit


def selected_tools(root: Path, environment: dict[str, str]) -> dict[str, str]:
    """Resolve genuine host tools using the canonical native producer's policy."""
    unit.require(sys.platform == "darwin" and platform.machine() in unit.HOSTS,
                 "local JavaScript tests require a supported macOS host")
    unit.require(sys.version_info[:2] == (3, 12), "run this command with Python 3.12")
    unit.require("IROHA_JS_NATIVE_DIR" not in environment,
                 "local runner requires unset IROHA_JS_NATIVE_DIR; it builds its own artifact")
    unit.local_policy("local-unit", environment.get("IROHA_JS_NATIVE_BUILD_PROFILE", "debug"))
    selected = environment.get("DEVELOPER_DIR")
    if selected is None:
        result = subprocess.run(["/usr/bin/xcode-select", "-p"], env=environment,
                                text=True, capture_output=True, check=True)
        selected = result.stdout.strip()
    developer = Path(selected)
    unit.require(developer.is_absolute() and developer.resolve(strict=True) == developer,
                 "selected Xcode developer directory must be absolute and canonical")
    apple = developer / "Toolchains/XcodeDefault.xctoolchain/usr/bin"
    unit.require(all((apple / name).is_file() for name in ("clang", "ld"))
                 and (developer / "usr/bin/xcodebuild").is_file(),
                 "full Xcode is required; select its Contents/Developer directory with "
                 "DEVELOPER_DIR or xcode-select (Command Line Tools alone are insufficient)")
    rust = Path.home() / ".rustup/toolchains" / ("1.93.1-" + unit.HOSTS[platform.machine()]) / "bin"
    config = {"python": str(Path(sys.executable).resolve(strict=True)),
              **{name: str(rust / name) for name in ("cargo", "rustc", "rustdoc")},
              "clang": str(apple / "clang"), "ld": str(apple / "ld"),
              "codesign": "/usr/bin/codesign", "developer_dir": str(developer),
              "sdk": str(developer / "Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk")}
    node = shutil.which("node", path=unit.tool_path(config))
    unit.require(node is not None, "Node is absent from the native producer's fixed tool path")
    config["node"] = str(Path(node).resolve(strict=True))
    unit.validate_config(root, config)
    return config


def run_step(command: list[str], *, root: Path, environment: dict[str, str], log: Path):
    """Retain actual child diagnostics and propagate the natural exit status."""
    print(f"{log.stem}: {log}", flush=True)
    with log.open("xb") as output:
        result = subprocess.run(command, cwd=root, env=environment, stdout=output,
                                stderr=subprocess.STDOUT, check=False)
    if result.returncode:
        raise subprocess.CalledProcessError(result.returncode, command)


def prepare_compiler_cache(root: Path, config: dict[str, str], environment: dict[str, str], run: Path):
    """Start or reuse sccache with short private scratch for its startup socket."""
    sccache = shutil.which("sccache", path=unit.tool_path(config))
    if sccache is None:
        return
    # sccache's daemon startup notification socket also uses TMPDIR, regardless
    # of client-side cache mode. Bootstrap before entering long retained paths.
    # This probes the stock compiler only; it builds no native artifact.
    scratch = Path(tempfile.mkdtemp(prefix="iroha-js-", dir="/tmp")).resolve(strict=True)
    # A daemon started here can outlive this suite and retain TMPDIR. Keep its
    # private startup directory available and retain the path with the logs.
    unit.save(run / "compiler-cache-scratch.json", {"path": str(scratch)})
    run_step([str(Path(sccache).resolve(strict=True)), config["rustc"], "-vV"],
             root=root, environment=dict(environment, TMPDIR=str(scratch), SCCACHE_CLIENT_SIDE="0"),
             log=run / "compiler-cache.log")


def run_suite(root: Path, target: Path, config: dict[str, str]) -> Path:
    """Produce, verify, test all unit cases and reverify the original custody pin."""
    unit.custody.original_directory(root)
    unit.require(target.is_absolute() and str(target) == os.path.abspath(target)
                 and target.is_dir() and target.resolve(strict=True) == target
                 and any(target != parent and target.is_relative_to(parent)
                         for parent in (root / "target/cargo-fast", root / "target/qualification")),
                 "Cargo lane must be an existing canonical child of target/cargo-fast or target/qualification")
    qualification = root / "target/qualification"
    qualification.mkdir(mode=0o700, exist_ok=True)
    unit.custody.original_directory(qualification)
    run = Path(tempfile.mkdtemp(prefix="js-", dir=qualification))
    print(f"Local JavaScript artifacts and logs: {run}", flush=True)
    artifact = run / "artifact"
    unit.build_directory(root, target, artifact)
    config_path = run / "native-tools.json"
    unit.save(config_path, config)
    # The producer constructs its own fixed compiler environment. Keep the
    # orchestrator and test process equally independent of inherited selectors,
    # Node preload hooks, alternate Cargo profiles and compiler wrappers.
    environment = {"HOME": str(Path.home()), "PATH": unit.tool_path(config),
                   "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "NODE_OPTIONS": "",
                   "DEVELOPER_DIR": config["developer_dir"], "SDKROOT": config["sdk"],
                   "MACOSX_DEPLOYMENT_TARGET": "13.0", "PYTHONDONTWRITEBYTECODE": "1"}
    prepare_compiler_cache(root, config, environment, run)
    producer = [config["python"], "-I", "-S", "-B", str(root / "scripts/native_js_local_unit.py")]
    run_step([*producer, "produce", "--root", str(root), "--target-dir", str(target),
              "--output", str(artifact), "--config", str(config_path),
              "--acknowledge-local-unit-recipe"], root=root, environment=environment,
             log=run / "native-build.log")
    verify = [*producer, "verify", "--root", str(root), "--output", str(artifact),
              "--producer-sha256", unit.digest(artifact / "producer-record.json")]
    run_step(verify, root=root, environment=environment, log=run / "verify-before.log")
    sdk = root / "javascript/iroha_js"
    test_failure = None
    try:
        run_step([config["node"], str(sdk / "scripts/run-test-profile.mjs"), "unit"],
                 root=sdk, environment=dict(environment, IROHA_JS_NATIVE_DIR=str(artifact)),
                 log=run / "javascript-test.log")
    except (subprocess.CalledProcessError, OSError) as error:
        test_failure = error
    try:
        run_step(verify, root=root, environment=environment, log=run / "verify-after.log")
    except (subprocess.CalledProcessError, OSError) as error:
        if test_failure is not None:
            raise RuntimeError(f"JavaScript tests failed ({test_failure}); post-test custody verification "
                               f"also failed ({error}). See {run / 'javascript-test.log'} and "
                               f"{run / 'verify-after.log'}") from error
        raise
    if test_failure is not None:
        raise test_failure
    print(f"Complete local JavaScript unit profile passed; evidence retained at {run}", flush=True)
    return run


def main(argv=None) -> int:
    """Require the existing Cargo lane without exposing test or custody bypasses."""
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True,
                        help="existing canonical Cargo lane under target/cargo-fast or target/qualification")
    args = parser.parse_args(argv)
    try:
        config = selected_tools(root, os.environ.copy())
        run_suite(root, args.target_dir, config)
        return 0
    except (RuntimeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Local JavaScript run failed: {error}. Retained logs are listed above.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
