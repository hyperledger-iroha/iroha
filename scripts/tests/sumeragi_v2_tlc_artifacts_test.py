"""Exercise retained TLC artifacts with controlled subprocesses, never real TLC."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts" / "formal"
PINNED_JAR_SHA256 = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
RUNNERS = ("multilane_mutations", "inflight_first_release")


FAKE_JAVA = r'''
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import time

if sys.argv[1:] == ["-version"]:
    print("controlled fake Java, not a TLC qualification run", file=sys.stderr)
    sys.exit(0)
config = Path(sys.argv[sys.argv.index("-config") + 1])
module = Path(sys.argv[-1])
mode = os.environ.get("FAKE_TLC_MODE", "valid")
fixed = config.name.endswith("_fixed.cfg")
invariant = re.findall(r"^INVARIANT (\w+)", config.read_text(), re.M)[-1]
Path("observed-inputs.json").write_text(json.dumps({
    "argv": sys.argv,
    "cwd": str(Path.cwd()),
    "module_sha256": hashlib.sha256(module.read_bytes()).hexdigest(),
    "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
}))
lines = ["TLC2 Version 2.19 of 08 August 2024"]
if fixed:
    lines.append("Model checking completed. No error has been found.")
else:
    if mode == "wrong-invariant":
        invariant = "ForeignInvariant"
    lines.append("Error: Invariant " + invariant + " is violated.")
    if mode != "missing-trace":
        lines.extend(["Error: The behavior up to this point is:",
                      "State 1: <Initial predicate>", "/\\ owned = FALSE",
                      "State 2: <Advance>", "/\\ owned = TRUE"])
        Path("counterexample.trace").write_bytes(b"exact fake counterexample\x00\xff\n")
    if mode == "duplicate-invariant":
        lines.append("Error: Invariant " + invariant + " is violated.")
    if mode == "extra-primary":
        print("Error: Action property ForeignProperty is violated.", file=sys.stderr, flush=True)
    if mode == "input-drift":
        module.write_text(module.read_text() + "\n\\* changed during tool execution\n")
    if mode == "tool-drift":
        jar = Path(sys.argv[sys.argv.index("-cp") + 1])
        jar.write_bytes(jar.read_bytes() + b"changed")
    if mode == "tamper-prior-result":
        previous = Path.cwd().parents[1] / "fixed/result.json"
        if previous.exists():
            record = json.loads(previous.read_text())
            record["later_edit"] = "changed after acceptance"
            previous.write_text(json.dumps(record))
lines.append("0 states generated, 0 distinct states found, 0 states left on queue."
             if not fixed and mode == "zero-states" else
             "2 states generated, 2 distinct states found, 0 states left on queue.")
if fixed or mode != "missing-terminal":
    lines.append("Finished in 1s at (2026-09-06 12:00:00)")
if not fixed and mode == "trailing-output":
    lines.append("unexpected output after completion")
print("\n".join(lines), flush=True)
if not fixed and mode == "late-stderr":
    time.sleep(0.02)
    print("late stderr after successful-looking stdout footer", file=sys.stderr, flush=True)
sys.exit(0 if fixed or mode == "wrong-status" else 12)
'''


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


@pytest.fixture
def fixture(tmp_path: Path) -> tuple[Path, dict]:
    """Change only the tool pin in copied runners; production has no fake mode."""
    repo = tmp_path / "fixture"
    scripts = repo / "scripts" / "formal"
    formal = repo / "formal" / "sumeragi_v2"
    scripts.mkdir(parents=True)
    formal.mkdir(parents=True)
    jar = tmp_path / "fake-tla2tools.jar"
    jar.write_bytes(b"controlled test jar, never production TLC")
    java = tmp_path / "fake-java"
    java.write_text(f"#!{sys.executable}\n" + FAKE_JAVA)
    java.chmod(0o755)
    for name in ("sumeragi_v2_tlc_artifacts.py", "sumeragi_v2_tlc_result_contract.sh",
                 "resolve_java.sh"):
        shutil.copy2(SCRIPTS / name, scripts / name)
    # The production source checker has its own real-source controls. This
    # fixture isolates process retention and uses the exact current models/cfgs.
    (scripts / "check_sumeragi_v2_multilane_models.py").write_text(
        'print("controlled structural preflight fixture")\n')
    for runner in RUNNERS:
        name = f"run_sumeragi_v2_{runner}.sh"
        source = (SCRIPTS / name).read_text()
        assert source.count(PINNED_JAR_SHA256) == 1
        (scripts / name).write_text(source.replace(PINNED_JAR_SHA256, sha256(jar)))
        for filename in set(re.findall(r"\b\w+\.(?:tla|cfg)\b", source)):
            shutil.copy2(ROOT / "formal" / "sumeragi_v2" / filename, formal / filename)
    evidence = tmp_path / "evidence"
    evidence.mkdir(mode=0o700)
    environment = os.environ.copy()
    environment.update({"JAVA_BIN": str(java), "TLA2TOOLS_JAR": str(jar),
                        "SUMERAGI_V2_FORMAL_EVIDENCE_DIR": str(evidence),
                        "TMPDIR": str(tmp_path)})
    environment.pop("FAKE_TLC_MODE", None)
    return repo, environment


def invoke(fixture: tuple[Path, dict], runner: str, mode: str = "valid"):
    repo, environment = fixture
    environment = {**environment, "FAKE_TLC_MODE": mode}
    result = subprocess.run(["bash", str(repo / "scripts" / "formal" /
                                       f"run_sumeragi_v2_{runner}.sh")],
                            env=environment, capture_output=True, text=True, check=False)
    paths = re.findall(r"^\[tlc\] retained artifacts: (.+)$", result.stderr, re.M)
    assert paths, result.stderr
    assert len(set(paths)) == 1
    root = Path(paths[0])
    assert root.is_dir()
    assert root.stat().st_mode & 0o777 == 0o700
    return result, root


@pytest.mark.parametrize("runner,count,mutations", [
    ("multilane_mutations", 106, 106), ("inflight_first_release", 23, 22)])
def test_complete_corpus_retains_exact_executed_inputs_and_raw_results(
    fixture, runner: str, count: int, mutations: int
) -> None:
    result, root = invoke(fixture, runner)
    assert result.returncode == 0, result.stderr
    terminal = read_json(root / "finished.json")
    assert terminal["all_expected_cases_accepted"] is True
    assert len(terminal["accepted_cases"]) == count
    assert len(list((root / "cases").iterdir())) == count
    assert read_json(root / "tools.json")["jar"]["sha256"] == sha256(root / "tools/tla2tools.jar")
    assert "controlled fake Java" in (root / "tools/java-version.stderr.log").read_text()
    assert (root / "support/sumeragi_v2_tlc_artifacts.py").read_bytes() == (
        SCRIPTS / "sumeragi_v2_tlc_artifacts.py").read_bytes()
    negatives = 0
    for case in (root / "cases").iterdir():
        started = read_json(case / "started.json")
        observed = read_json(case / "inputs/observed-inputs.json")
        actual = read_json(case / "result.json")
        accepted = read_json(case / "accepted.json")
        assert observed["argv"] == started["command"]
        assert observed["cwd"] == started["cwd"] == str(case / "inputs")
        assert accepted["result_sha256"] == sha256(case / "result.json")
        for filename, snapshot in started["inputs"].items():
            kind = "module" if filename.endswith(".tla") else "config"
            assert observed[f"{kind}_sha256"] == snapshot["sha256"] == sha256(case / "inputs" / filename)
            assert (case / "inputs" / filename).read_bytes() == Path(snapshot["source"]).read_bytes()
        for name, record in actual["raw_artifacts"].items():
            assert record == {"bytes": (case / name).stat().st_size, "sha256": sha256(case / name)}
        assert (case / "stderr.log").read_bytes() == b""
        assert (case / "combined.log").read_bytes() == (case / "stdout.log").read_bytes()
        if started["expectation"] != "fixed-success":
            negatives += 1
            assert actual["returncode"] == 12
            assert "State 2: <Advance>" in (case / "combined.log").read_text()
            assert (case / "inputs/counterexample.trace").read_bytes() == b"exact fake counterexample\x00\xff\n"
        else:
            assert actual["returncode"] == 0
    assert negatives == mutations


@pytest.mark.parametrize("runner", RUNNERS)
@pytest.mark.parametrize("mode", ["wrong-status", "wrong-invariant", "missing-trace",
                                  "duplicate-invariant", "extra-primary", "zero-states",
                                  "missing-terminal", "trailing-output", "input-drift", "tool-drift",
                                  "late-stderr"])
def test_failed_control_retains_raw_evidence_without_acceptance(fixture, runner, mode) -> None:
    result, root = invoke(fixture, runner, mode)
    assert result.returncode != 0
    terminal = read_json(root / "finished.json")
    assert terminal["all_expected_cases_accepted"] is False
    assert terminal["runner_exit_status"] != 0
    cases = list((root / "cases").iterdir())
    failed = next(case for case in cases if not (case / "accepted.json").exists())
    assert (failed / "started.json").is_file()
    assert (failed / "result.json").is_file()
    assert (failed / "stdout.log").stat().st_size > 0
    if mode in ("extra-primary", "late-stderr"):
        assert (failed / "stderr.log").stat().st_size > 0
        assert read_json(failed / "rejected.json")["reason"] == "TLC stderr must be empty"
        assert "TLC stderr must be empty" in result.stderr
    assert len(terminal["accepted_cases"]) == (1 if runner == "inflight_first_release" else 0)


def test_standalone_setup_failure_reports_fresh_retained_paths(fixture) -> None:
    repo, environment = fixture
    environment.pop("SUMERAGI_V2_FORMAL_EVIDENCE_DIR")
    environment["TLA2TOOLS_JAR"] = str(repo / "missing.jar")
    first_result, first = invoke((repo, environment), "inflight_first_release")
    second_result, second = invoke((repo, environment), "inflight_first_release")
    assert first_result.returncode != 0 and second_result.returncode != 0
    assert first != second
    assert first.parent == Path(environment["TMPDIR"]).resolve()
    for root in (first, second):
        assert read_json(root / "finished.json")["all_expected_cases_accepted"] is False
        assert read_json(root / "invocation.json")["expected_cases"] == 23


def test_recorded_raw_output_cannot_be_changed_before_acceptance(fixture) -> None:
    _, root = invoke(fixture, "multilane_mutations", "wrong-invariant")
    case = next((root / "cases").iterdir())
    with (case / "stdout.log").open("ab") as stream:
        stream.write(b"replacement evidence\n")
    result = subprocess.run([sys.executable, "-I", "-S",
                             str(root / "support/sumeragi_v2_tlc_artifacts.py"),
                             "accept", "--run-dir", str(root), "--name", case.name],
                            capture_output=True, text=True, check=False)
    assert result.returncode != 0
    assert "raw artifact changed" in result.stderr
    assert not (case / "accepted.json").exists()


def test_executed_input_cannot_be_changed_between_capture_and_acceptance(fixture) -> None:
    _, root = invoke(fixture, "multilane_mutations", "wrong-invariant")
    case = next((root / "cases").iterdir())
    module = next((case / "inputs").glob("*.tla"))
    module.write_text(module.read_text() + "\n\\* edited after capture\n")
    result = subprocess.run([sys.executable, "-I", "-S",
                             str(root / "support/sumeragi_v2_tlc_artifacts.py"),
                             "accept", "--run-dir", str(root), "--name", case.name],
                            capture_output=True, text=True, check=False)
    assert result.returncode != 0
    assert "retained executed input changed" in result.stderr
    assert not (case / "accepted.json").exists()


def test_result_cannot_be_changed_between_acceptance_and_completion(fixture) -> None:
    result, root = invoke(fixture, "inflight_first_release", "tamper-prior-result")
    assert result.returncode != 0
    terminal = read_json(root / "finished.json")
    assert terminal["runner_body_exit_status"] == 0
    assert terminal["runner_exit_status"] == result.returncode == 1
    assert len(terminal["accepted_cases"]) == 23
    assert terminal["all_expected_cases_accepted"] is False
    assert any("accepted result changed: fixed" in error for error in terminal["consistency_errors"])
