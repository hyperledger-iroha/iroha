#!/usr/bin/env python3
"""Explicit external-corpus qualification; ordinary discovery skips cannot qualify.

Prerequisites: Python 3.11+ (standard library; validated with Python 3.12),
/usr/bin/git, a clean canonical checkout on optimizations, and independently
reviewed matching native artifacts/source/build receipts. Swift requires macOS
with Xcode/SwiftPM; Kotlin requires a compatible macOS/Linux JNI host, the
repository Gradle wrapper and its configured JDK 21 toolchain.

All four environment inputs are mandatory for this dedicated runner:
  BOI_ACTIVATION_FIXTURE_DIR: canonical external generated-original directory.
  BOI_ACTIVATION_MANIFEST_SHA256: independently pinned generated manifest digest.
  BOI_ACTIVATION_PRODUCER_RECEIPT: canonical external producer receipt file.
  BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256: independently pinned receipt digest.
CLI arguments supply exact source commit/tree, admitted artifact directory and
receipt pin, the admission-driver hash, and a fresh external output directory.

The runner builds/runs one selected platform test after public input admission;
only exactly one pass and zero skips/failures/errors can qualify. After creating
its fresh output directory it preserves failure evidence; early preflight refusal
creates no receipt. It never overwrites outputs, generates fixtures or signs.
Signed-source and artifact admission remain independent review
requirements. Use --help and README.md for selectors and ordinary-suite behavior.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET

import native_c_driver as admission

INPUTS = ("BOI_ACTIVATION_FIXTURE_DIR", "BOI_ACTIVATION_MANIFEST_SHA256",
          "BOI_ACTIVATION_PRODUCER_RECEIPT", "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256")
CASES = {
    "swift": ("NativeTimeIronwoodActivationV1Tests", "testPinnedExternalActivationCallsRealNativeAdvanceAndRefusesSubstitutions"),
    "kotlin": ("org.hyperledger.iroha.sdk.client.NativeTimeIronwoodActivationFixtureTest",
               "pinnedExternalActivationCallsRealJniAndRefusesSubstitutions"),
}


def require_selection(environment):
    admission.require(all(key in environment and environment[key] for key in INPUTS),
                      "dedicated qualification requires all four nonempty fixture inputs; absence never qualifies")
    admission.hex_pin(environment[INPUTS[1]])
    admission.hex_pin(environment[INPUTS[3]])


def validation_environment(environment):
    result = dict(environment)
    # Codex may set this presentation-only option. Discard it for every child;
    # the fixed Git invocation also disables paging, so no inherited pager runs.
    result.pop("GIT_PAGER", None)
    blocked = sorted(key for key in result if key.startswith("GIT_"))
    # Reject empty overrides too. Report names only; values may contain private data.
    admission.require(not blocked, "Git environment overrides are not permitted: " + ", ".join(blocked))
    return result


def verify_report(raw, platform):
    admission.require(0 < len(raw) <= 4 * admission.MIB and b"<!DOCTYPE" not in raw.upper()
                      and b"<!ENTITY" not in raw.upper(), "bounded plain xUnit XML required")
    root = ET.fromstring(raw)
    admission.require(root.tag in ("testsuite", "testsuites"), "xUnit root required")
    tests = list(root.iter("testcase"))
    admission.require(len(tests) == 1, "exactly one selected activation test required")
    expected_class, expected_name = CASES[platform]
    valid_classes = {expected_class}
    if platform == "swift":
        valid_classes.add("IrohaSwiftTests." + expected_class)
    case = tests[0]
    admission.require(case.get("classname") in valid_classes
                      and case.get("name") in (expected_name, expected_name + "()"), "exact selected test identity required")
    admission.require(not any(node.tag in ("skipped", "failure", "error", "disabled") for node in root.iter()),
                      "zero skipped, failed or errored tests required")
    admission.require(case.get("status", "run") in ("run", "passed", "success"), "test did not run")
    for node in root.iter():
        if node.tag in ("testsuite", "testsuites"):
            for key, expected in (("tests", "1"), ("failures", "0"), ("errors", "0"), ("skipped", "0"), ("disabled", "0")):
                if key in node.attrib:
                    admission.require(node.attrib[key] == expected, "xUnit counters do not prove one pass/zero skips")
    return {"tests": 1, "passed": 1, "failed": 0, "skipped": 0,
            "classname": case.get("classname"), "name": case.get("name")}


def clean_source(repo, commit, tree, environment):
    environment = validation_environment(environment)
    admission.require(repo.is_absolute() and repo == repo.resolve(strict=True), "canonical source path required")
    prefix = ["/usr/bin/git", "--no-pager", "--no-replace-objects", "--no-optional-locks", "-C", str(repo),
              "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false", "-c", "core.ignoreStat=false"]
    def git(*args):
        return subprocess.run([*prefix, *args], capture_output=True, env=environment,
                              text=True, check=True, timeout=30).stdout.rstrip("\n")
    admission.require(git("rev-parse", "--show-toplevel") == str(repo), "Git top-level differs from canonical source path")
    admission.require(git("branch", "--show-current") == "optimizations", "canonical optimizations branch required")
    git_dir = (repo / ".git").resolve(strict=True)
    admission.require((repo / ".git").is_dir() and git_dir == repo / ".git"
                      and git("rev-parse", "--absolute-git-dir") == str(git_dir), "canonical repository Git directory required")
    common = Path(git("rev-parse", "--git-common-dir"))
    admission.require((common if common.is_absolute() else repo / common).resolve(strict=True) == git_dir,
                      "alternate Git common directory refused")
    entries = git("ls-files", "-v", "-z").split("\0")
    admission.require(all(not entry or (len(entry) >= 2 and entry[1] == " "
                      and not entry[0].islower() and entry[0] != "S") for entry in entries),
                      "assume-unchanged or skip-worktree index entries refused")
    admission.require(git("rev-parse", "HEAD") == commit and git("rev-parse", "HEAD^{tree}") == tree,
                      "reviewed source commit/tree differ")
    admission.require(not git("status", "--porcelain=v1", "--untracked-files=all", "--ignore-submodules=none"),
                      "canonical source must be clean")


def kotlin_init(report_dir):
    encoded = base64.b64encode(str(report_dir).encode()).decode("ascii")
    # The only interpolated value is base64, never unescaped user Groovy code.
    return """gradle.projectsEvaluated {
    def reportDirectory = new File(new String(java.util.Base64.decoder.decode('%s'), java.nio.charset.StandardCharsets.UTF_8))
    gradle.rootProject.project(':core-jvm').tasks.named('test', org.gradle.api.tasks.testing.Test).configure {
        reports.junitXml.required.set(true)
        reports.junitXml.outputLocation.set(reportDirectory)
    }
}
""" % encoded


def run_command(argv, cwd, environment, log, timeout):
    environment = validation_environment(environment)
    with log.open("xb") as output:
        result = subprocess.run(argv, cwd=cwd, env=environment, stdout=output,
                                stderr=subprocess.STDOUT, timeout=timeout)
    admission.require(result.returncode == 0, "qualification command failed; inspect " + str(log))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=tuple(CASES))
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-tree", required=True)
    parser.add_argument("--artifact-dir", required=True, type=Path)
    parser.add_argument("--artifact-receipt", required=True, type=Path)
    parser.add_argument("--artifact-receipt-sha256", required=True)
    parser.add_argument("--admission-driver-sha256", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--timeout-seconds", type=int, default=14400)
    args = parser.parse_args()
    environment = validation_environment(os.environ)
    # This happens before any command (including git), so absence cannot become a skipped pass.
    require_selection(environment)
    admission.require(args.timeout_seconds > 0, "positive command timeout required")
    repo = args.source_root.resolve(strict=True)
    admission.require(args.source_root.is_absolute() and args.source_root == repo, "absolute canonical source root required")
    commit, tree = admission.hex_pin(args.source_commit, 40), admission.hex_pin(args.source_tree, 40)
    artifact = admission.external_path(args.artifact_dir, repo)
    admission.require(artifact.is_dir(), "explicit admitted artifact directory required")
    receipt = admission.external_path(args.artifact_receipt, repo)
    admission.require(admission.hash_regular(receipt, 4 * admission.MIB) == admission.hex_pin(args.artifact_receipt_sha256),
                      "independently reviewed native artifact receipt pin differs")
    driver = Path(admission.__file__).resolve(strict=True)
    admission.require(admission.hash_regular(driver, admission.MIB) == admission.hex_pin(args.admission_driver_sha256),
                      "admission driver pin differs")
    output = args.output_dir
    admission.require(output.is_absolute() and output.parent == output.parent.resolve(strict=True)
                      and not output.exists() and not output.is_symlink(), "fresh canonical absolute output directory required")
    for excluded in (repo, artifact, Path(environment[INPUTS[0]]).resolve(strict=True)):
        admission.require(not output.is_relative_to(excluded), "results must be outside source/artifact/fixture roots")
    clean_source(repo, commit, tree, environment)
    output.mkdir(mode=0o700)
    try:
        admission_argv = [sys.executable, str(driver), "--admit-only", "--source-root", str(repo),
            "--source-commit", commit, "--source-tree", tree, "--output", str(output / "input-admission.json")]
        run_command(admission_argv, repo, environment, output / "input-admission.log", 120)
        admitted = admission.json_original(admission.read_regular(output / "input-admission.json", 65536))
        admission.require(admitted["status"] == "INPUTS_ADMITTED_ONLY" and admitted["nativeChecks"] == 0
                          and admitted["nativeLoaded"] is False
                          and admitted["fixtureManifestSha256"] == environment[INPUTS[1]]
                          and admitted["producerReceiptSha256"] == environment[INPUTS[3]], "explicit input admission required")
        report_dir = output / "test-results"
        report_dir.mkdir()
        class_name, _ = CASES[args.platform]
        if args.platform == "swift":
            for key in ("MOBILE_SDK_APPLE_ARTIFACT_DIR", "MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR", "MOBILE_SDK_APPLE_DEVICE_DEMO_ARTIFACT_DIR",
                        "MOBILE_SDK_APPLE_DEVICE_DEMO_ALLOW_SOURCE_HEAD_DRIFT", "MOBILE_SDK_APPLE_DEVICE_DEMO_CONSUMPTION_RECEIPT"):
                environment.pop(key, None)
            environment["MOBILE_SDK_APPLE_ARTIFACT_DIR"] = str(artifact)
            environment["MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"] = "1"
            argv = ["swift", "test", "--package-path", str(repo / "IrohaSwift"), "--disable-automatic-resolution",
                    "--scratch-path", str(output / "swift-build"), "--filter", class_name,
                    "--enable-xctest", "--disable-swift-testing", "--xunit-output", str(report_dir / "activation.xml")]
            cwd = repo
        else:
            environment["IROHA_NATIVE_LIBRARY_PATH"] = str(artifact)
            init = output / "qualification.init.gradle"
            init.write_text(kotlin_init(report_dir))
            argv = [str(repo / "kotlin/gradlew"), ":core-jvm:test", "--tests", class_name,
                    "--console=plain", "--rerun-tasks", "--no-build-cache", "--no-daemon", "--init-script", str(init)]
            cwd = repo / "kotlin"
        run_command(argv, cwd, environment, output / "platform.log", args.timeout_seconds)
        reports = sorted(report_dir.glob("*.xml"))
        admission.require(len(reports) == 1, "exactly one fresh xUnit report required")
        raw = admission.read_regular(reports[0], 4 * admission.MIB)
        outcome = verify_report(raw, args.platform)
        clean_source(repo, commit, tree, environment)
        admission.require(admission.hash_regular(receipt, 4 * admission.MIB) == args.artifact_receipt_sha256,
                          "artifact admission receipt changed during run")
        result = {"schema": "boi.time-ironwood-platform-qualification.v1", "status": "PASSED",
                  "platform": args.platform, "sourceCommit": commit, "sourceTree": tree, "sourceBranch": "optimizations",
                  "artifactDir": str(artifact), "artifactReceiptSha256": args.artifact_receipt_sha256,
                  "admissionDriverSha256": args.admission_driver_sha256,
                  "fixtureManifestSha256": environment[INPUTS[1]], "producerReceiptSha256": environment[INPUTS[3]],
                  "argv": argv, "cwd": str(cwd), "testResult": outcome,
                  "xunitSha256": admission.digest(raw), "platformLogSha256": admission.hash_regular(output / "platform.log", 256 * admission.MIB),
                  "worldPlatformSessionQualified": False,
                  "limits": "Actual platform finality caller only. Signed-source and native artifact source/build admission require independent receipt review; this runner does not verify commit signatures. World session and physical/live qualification remain separate."}
        with (output / "result.json").open("x") as file:
            json.dump(result, file, indent=2)
            file.write("\n")
        print(json.dumps({"status": "PASSED", "passed": 1, "skipped": 0, "result": str(output / "result.json")}))
    except Exception as error:
        with (output / "failure.json").open("x") as file:
            json.dump({"status": "FAILED", "reason": str(error)}, file, indent=2)
            file.write("\n")
        raise


if __name__ == "__main__":
    main()
