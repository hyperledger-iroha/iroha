#!/usr/bin/env python3
"""Build fail-closed AtomicPrivateSettlementV1 TLC release evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Final, Sequence


PROTOCOL: Final = "AtomicPrivateSettlementV1"
REPORT_VERSION: Final = 1
SUCCESS_MARKER: Final = "Model checking completed. No error has been found."
SAFETY_VIOLATION_MARKER: Final = "Error: Invariant Safety is violated."
SANY_VERSION_MARKER: Final = "****** SANY2 Version 2.1 created 24 February 2014"

COUNT_MODEL: Final = "AtomicPrivateSettlementV1.tla"
INDEXED_MODEL: Final = "AtomicPrivateSettlementV1CommitteeFaults.tla"
EVIDENCE_CODE_SOURCE_PATHS: Final[tuple[str, ...]] = (
    "scripts/formal/private_settlement_tlc_report.py",
    "scripts/formal/run_atomic_private_settlement_tlc.sh",
    "scripts/formal/sumeragi_v2_tlc_result_contract.sh",
    "scripts/formal/resolve_java.sh",
)
EVIDENCE_CODE_DOMAIN: Final = b"iroha-aps-formal-evidence-code-v1\0"
MAX_JAVA_VERSION_OUTPUT_BYTES: Final = 64 * 1024

CONFIGURATIONS: Final[tuple[tuple[str, str, str], ...]] = (
    ("AtomicPrivateSettlementV1_3.cfg", "pass", COUNT_MODEL),
    ("AtomicPrivateSettlementV1_255.cfg", "pass", COUNT_MODEL),
    ("AtomicPrivateSettlementV1_expiry.cfg", "pass", COUNT_MODEL),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_2_validator_focused.cfg",
        "pass",
        INDEXED_MODEL,
    ),
    ("AtomicPrivateSettlementV1CommitteeFaults_2.cfg", "pass", INDEXED_MODEL),
    ("AtomicPrivateSettlementV1CommitteeFaults_3.cfg", "pass", INDEXED_MODEL),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_4_clean.cfg",
        "pass",
        INDEXED_MODEL,
    ),
    (
        "AtomicPrivateSettlementV1CommitteeFaults_expiry.cfg",
        "pass",
        INDEXED_MODEL,
    ),
    ("AtomicPrivateSettlementV1_partial_apply_bug.cfg", "safety_violation", COUNT_MODEL),
    (
        "AtomicPrivateSettlementV1_commit_before_prepare_bug.cfg",
        "safety_violation",
        COUNT_MODEL,
    ),
    (
        "AtomicPrivateSettlementV1_drop_stage_on_crash_bug.cfg",
        "safety_violation",
        COUNT_MODEL,
    ),
)

_STATE_SUMMARY = re.compile(
    r"^(?P<generated>[0-9][0-9,]*) states generated, "
    r"(?P<distinct>[0-9][0-9,]*) distinct states found, "
    r"(?P<queued>[0-9][0-9,]*) states left on queue\.$",
    re.MULTILINE,
)
_DEPTH = re.compile(
    r"^The depth of the (?:complete )?state graph search is "
    r"(?P<depth>[0-9][0-9,]*)\.$",
    re.MULTILINE,
)
_TLC_VERSION = re.compile(
    r"^TLC2 Version (?P<version>[0-9]+(?:\.[0-9]+)+) .+$",
    re.MULTILINE,
)
_RUN_HEADER = re.compile(
    r"^Running .+ with fp (?P<fingerprint>[0-9]+) and seed (?P<seed>[0-9]+) "
    r"with (?P<workers>[1-9][0-9]*) workers? on .+$",
    re.MULTILINE,
)
_TERMINAL = re.compile(
    r"^Finished in "
    r"(?:(?:[0-9]+d )?(?:[0-9]+h )?(?:[0-9]+min )?[0-9]+(?:ms|s)"
    r"|(?:[0-9]+d )?(?:[0-9]+h )?[0-9]+min"
    r"|(?:[0-9]+d )?[0-9]+h|[0-9]+d) "
    r"at \([0-9]{4}-[0-9]{2}-[0-9]{2} "
    r"[0-9]{2}:[0-9]{2}:[0-9]{2}\)$",
    re.MULTILINE,
)
_FAILURE_DIAGNOSTIC = re.compile(
    r"^[ \t]*(?:Error:|Deadlock reached(?:\.|$)|Temporal properties were violated\.$)",
    re.MULTILINE,
)
_PRIMARY_FAILURE_DIAGNOSTIC = re.compile(
    r"^[ \t]*(?:Error: (?:Invariant |Action property |Temporal properties were violated\.$"
    r"|Deadlock reached(?:\.|$))|Deadlock reached(?:\.|$)"
    r"|Temporal properties were violated\.$)",
    re.MULTILINE,
)
_UNEXPECTED_NEGATIVE_DIAGNOSTIC = re.compile(
    r"^[ \t]*Error: (?!Invariant Safety is violated\.$"
    r"|The behavior up to this point is:$).+$",
    re.MULTILINE,
)
_SANY_FAILURE_DIAGNOSTIC = re.compile(
    r"(?:error|exception|abort)",
    re.IGNORECASE,
)


class ReportError(RuntimeError):
    """Raised when TLC output cannot support release evidence."""


@dataclass(frozen=True)
class RunSummary:
    """Validated state-space summary for one configuration."""

    name: str
    model: str
    expected_outcome: str
    observed_outcome: str
    generated_states: int
    distinct_states: int
    depth: int

    def as_json(self) -> dict[str, object]:
        """Return the exact release-report projection."""

        return {
            "name": self.name,
            "model": self.model,
            "expected_outcome": self.expected_outcome,
            "observed_outcome": self.observed_outcome,
            "generated_states": self.generated_states,
            "distinct_states": self.distinct_states,
            "depth": self.depth,
        }


def _positive_int(raw: str, *, label: str) -> int:
    value = int(raw.replace(",", ""))
    if value <= 0:
        raise ReportError(f"{label} must be positive")
    return value


def validate_sany(*, model: str, stdout: str, stderr: str, status: int) -> None:
    """Require one clean SANY semantic pass for the named model."""

    semantic_marker = f"Semantic processing of module {Path(model).stem}"
    if (
        status != 0
        or stderr
        or stdout.splitlines().count(SANY_VERSION_MARKER) != 1
        or stdout.splitlines().count(semantic_marker) != 1
        or _SANY_FAILURE_DIAGNOSTIC.search(stdout)
    ):
        raise ReportError(f"{model}: SANY did not produce one clean semantic result")


def parse_run(
    *,
    name: str,
    model: str,
    expected_outcome: str,
    stdout: str,
    stderr: str,
    status: int,
    seed: int,
    fingerprint_index: int,
    workers: str,
    tlc_version: str,
) -> RunSummary:
    """Validate one TLC result and extract its exact state-space counts."""

    if stderr:
        raise ReportError(f"{name}: TLC emitted separate stderr")
    if not model.endswith(".tla") or Path(model).name != model:
        raise ReportError(f"{name}: model identity is not canonical")

    version_matches = list(_TLC_VERSION.finditer(stdout))
    if len(version_matches) != 1 or version_matches[0].group("version") != tlc_version:
        raise ReportError(f"{name}: TLC version header differs from the authenticated tool")
    run_headers = list(_RUN_HEADER.finditer(stdout))
    if len(run_headers) != 1:
        raise ReportError(f"{name}: TLC must emit exactly one deterministic run header")
    run_header = run_headers[0]
    observed_workers = int(run_header.group("workers"))
    if (
        int(run_header.group("seed")) != seed
        or int(run_header.group("fingerprint")) != fingerprint_index
        or (workers != "auto" and observed_workers != int(workers))
    ):
        raise ReportError(f"{name}: TLC run controls differ from the report metadata")

    state_matches = list(_STATE_SUMMARY.finditer(stdout))
    if not state_matches:
        raise ReportError(f"{name}: TLC emitted no state-count summary")
    state = state_matches[-1]
    depth_matches = list(_DEPTH.finditer(stdout))
    if len(depth_matches) != 1:
        raise ReportError(f"{name}: TLC must emit exactly one graph depth")
    terminal_matches = list(_TERMINAL.finditer(stdout))
    if len(terminal_matches) != 1:
        raise ReportError(f"{name}: TLC must emit exactly one terminal marker")
    if stdout.rstrip().splitlines()[-1] != terminal_matches[0].group(0):
        raise ReportError(f"{name}: TLC output continues after its terminal marker")

    success_count = stdout.splitlines().count(SUCCESS_MARKER)
    safety_count = stdout.splitlines().count(SAFETY_VIOLATION_MARKER)
    success_offset = stdout.find(SUCCESS_MARKER)
    safety_offset = stdout.find(SAFETY_VIOLATION_MARKER)
    if not (
        version_matches[0].start()
        < run_header.start()
        < state.start()
        < depth_matches[0].start()
        < terminal_matches[0].start()
    ):
        raise ReportError(f"{name}: TLC result markers are out of order")
    if expected_outcome == "pass":
        if status != 0 or success_count != 1 or safety_count != 0:
            raise ReportError(f"{name}: TLC did not produce one clean passing result")
        diagnostics = _FAILURE_DIAGNOSTIC.findall(stdout)
        if diagnostics:
            raise ReportError(f"{name}: passing TLC output contains failure diagnostics")
        if not depth_matches[0].end() < success_offset < terminal_matches[0].start():
            raise ReportError(f"{name}: passing TLC result markers are out of order")
        observed_outcome = "pass"
    elif expected_outcome == "safety_violation":
        if status != 12 or safety_count != 1 or success_count != 0:
            raise ReportError(
                f"{name}: negative control did not produce the exact Safety violation"
            )
        diagnostics = _PRIMARY_FAILURE_DIAGNOSTIC.findall(stdout)
        if len(diagnostics) != 1:
            raise ReportError(f"{name}: negative control emitted unexpected diagnostics")
        if _UNEXPECTED_NEGATIVE_DIAGNOSTIC.search(stdout):
            raise ReportError(f"{name}: negative control emitted unexpected diagnostics")
        if not run_header.end() < safety_offset < state.start():
            raise ReportError(f"{name}: negative TLC result markers are out of order")
        observed_outcome = "safety_violation"
    else:
        raise ReportError(f"{name}: unsupported expected outcome {expected_outcome!r}")

    generated_states = _positive_int(state.group("generated"), label="generated states")
    distinct_states = _positive_int(state.group("distinct"), label="distinct states")
    queued_states = int(state.group("queued").replace(",", ""))
    if distinct_states > generated_states:
        raise ReportError(f"{name}: distinct state count exceeds generated state count")
    if expected_outcome == "pass" and queued_states != 0:
        raise ReportError(f"{name}: passing TLC run retained queued states")

    return RunSummary(
        name=name,
        model=model,
        expected_outcome=expected_outcome,
        observed_outcome=observed_outcome,
        generated_states=generated_states,
        distinct_states=distinct_states,
        depth=_positive_int(depth_matches[0].group("depth"), label="graph depth"),
    )


def formal_package_sha256(formal_dir: Path) -> str:
    """Hash both models and every ordered configuration with path framing."""

    ordered_paths = [COUNT_MODEL, INDEXED_MODEL]
    ordered_paths.extend(name for name, _, _ in CONFIGURATIONS)
    digest = hashlib.sha256()
    for relative in ordered_paths:
        path = formal_dir / relative
        try:
            payload = path.read_bytes()
        except OSError as error:
            raise ReportError(f"cannot read formal input {path}: {error}") from error
        encoded_path = relative.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def evidence_code_sha256(evidence_dir: Path) -> str:
    """Hash the frozen producer, runner, and helper scripts with source paths."""

    digest = hashlib.sha256(EVIDENCE_CODE_DOMAIN)
    for source_path in EVIDENCE_CODE_SOURCE_PATHS:
        path = evidence_dir / Path(source_path).name
        try:
            payload = path.read_bytes()
        except OSError as error:
            raise ReportError(f"cannot read formal evidence code {path}: {error}") from error
        encoded_path = source_path.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def java_runtime_record(
    *,
    binary_sha256: str,
    binary_bytes: int,
    version_output_path: Path,
) -> dict[str, object]:
    """Validate and bind the Java executable identity reported by the runner."""

    if re.fullmatch(r"[0-9a-f]{64}", binary_sha256) is None:
        raise ReportError("Java binary SHA-256 is not canonical")
    if isinstance(binary_bytes, bool) or binary_bytes <= 0:
        raise ReportError("Java binary byte count must be positive")
    try:
        version_payload = version_output_path.read_bytes()
        version_output = version_payload.decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise ReportError(f"cannot read Java version output: {error}") from error
    if (
        not version_payload
        or len(version_payload) > MAX_JAVA_VERSION_OUTPUT_BYTES
        or "\0" in version_output
        or re.search(r'(?m)^(?:openjdk|java) version "[0-9][^"]*"', version_output)
        is None
    ):
        raise ReportError("Java version output is not one bounded runtime identity")
    return {
        "binary_sha256": binary_sha256,
        "binary_bytes": binary_bytes,
        "version_output": version_output,
        "version_output_sha256": hashlib.sha256(version_payload).hexdigest(),
        "version_output_bytes": len(version_payload),
    }


def _read_run(
    logs_dir: Path,
    name: str,
    model: str,
    expected_outcome: str,
    *,
    seed: int,
    fingerprint_index: int,
    workers: str,
    tlc_version: str,
) -> tuple[RunSummary, bytes]:
    prefix = logs_dir / name
    stdout_path = prefix.with_suffix(prefix.suffix + ".stdout.log")
    stderr_path = prefix.with_suffix(prefix.suffix + ".stderr.log")
    status_path = prefix.with_suffix(prefix.suffix + ".status")
    try:
        stdout_bytes = stdout_path.read_bytes()
        stderr_bytes = stderr_path.read_bytes()
        status_raw = status_path.read_text(encoding="ascii").strip()
        stdout = stdout_bytes.decode("utf-8")
        stderr = stderr_bytes.decode("utf-8")
        status = int(status_raw)
    except (OSError, UnicodeError, ValueError) as error:
        raise ReportError(f"cannot read TLC artifacts for {name}: {error}") from error
    summary = parse_run(
        name=name,
        model=model,
        expected_outcome=expected_outcome,
        stdout=stdout,
        stderr=stderr,
        status=status,
        seed=seed,
        fingerprint_index=fingerprint_index,
        workers=workers,
        tlc_version=tlc_version,
    )
    section = (
        f"===== {name} model {model} stdout (status {status}) =====\n".encode(
            "ascii"
        )
        + stdout_bytes
        + (b"" if stdout_bytes.endswith(b"\n") else b"\n")
        + f"===== {name} model {model} stderr =====\n".encode("ascii")
        + stderr_bytes
        + (b"" if not stderr_bytes or stderr_bytes.endswith(b"\n") else b"\n")
    )
    return summary, section


def _read_sany(sany_dir: Path, model: str) -> bytes:
    prefix = sany_dir / model
    stdout_path = prefix.with_suffix(prefix.suffix + ".stdout.log")
    stderr_path = prefix.with_suffix(prefix.suffix + ".stderr.log")
    status_path = prefix.with_suffix(prefix.suffix + ".status")
    try:
        stdout_bytes = stdout_path.read_bytes()
        stderr_bytes = stderr_path.read_bytes()
        status = int(status_path.read_text(encoding="ascii").strip())
        stdout = stdout_bytes.decode("utf-8")
        stderr_bytes.decode("utf-8")
    except (OSError, UnicodeError, ValueError) as error:
        raise ReportError(f"cannot read SANY artifacts for {model}: {error}") from error
    validate_sany(
        model=model,
        stdout=stdout,
        stderr=stderr_bytes.decode("utf-8"),
        status=status,
    )
    return (
        f"===== SANY {model} stdout (status {status}) =====\n".encode("ascii")
        + stdout_bytes
        + (b"" if stdout_bytes.endswith(b"\n") else b"\n")
        + f"===== SANY {model} stderr =====\n".encode("ascii")
        + stderr_bytes
    )


def build_report(
    *,
    formal_dir: Path,
    logs_dir: Path,
    sany_dir: Path,
    commit: str,
    tool_version: str,
    tool_sha256: str,
    java_binary_sha256: str,
    java_binary_bytes: int,
    java_version_output_path: Path,
    seed: int,
    fingerprint_index: int,
    workers: str,
    transcript_artifact_path: str,
) -> tuple[dict[str, object], bytes]:
    """Validate the complete matrix and construct its report and transcript."""

    if re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit) is None:
        raise ReportError("commit must be a full lowercase Git object id")
    if re.fullmatch(r"[0-9a-f]{64}", tool_sha256) is None:
        raise ReportError("tool SHA-256 must be 64 lowercase hexadecimal characters")
    tool_version = tool_version.strip()
    tool_version_match = re.fullmatch(
        r"TLC (?P<tlc>[0-9]+(?:\.[0-9]+)+) / TLA\+ tools "
        r"(?P<tools>[0-9]+(?:\.[0-9]+)+)",
        tool_version,
    )
    if tool_version_match is None:
        raise ReportError("tool version must identify TLC and TLA+ tools exactly")
    tlc_version = tool_version_match.group("tlc")
    if seed < 0:
        raise ReportError("seed must be an unsigned integer")
    if fingerprint_index < 0 or fingerprint_index > 63:
        raise ReportError("fingerprint index must be between 0 and 63")
    if re.fullmatch(r"[1-9][0-9]*", workers) is None:
        raise ReportError(
            "complete release evidence requires an explicit positive worker count"
        )
    artifact_path = Path(transcript_artifact_path)
    if (
        artifact_path.is_absolute()
        or ".." in artifact_path.parts
        or "\\" in transcript_artifact_path
        or artifact_path.as_posix() in {"", "."}
    ):
        raise ReportError("transcript artifact path must be safe and relative")

    package_digest = formal_package_sha256(formal_dir)
    evidence_code_digest = evidence_code_sha256(formal_dir)
    runtime = java_runtime_record(
        binary_sha256=java_binary_sha256,
        binary_bytes=java_binary_bytes,
        version_output_path=java_version_output_path,
    )
    summaries: list[RunSummary] = []
    metadata = (
        "===== AtomicPrivateSettlementV1 TLC release run =====\n"
        f"commit={commit}\n"
        f"tool_version={tool_version}\n"
        f"tool_sha256={tool_sha256}\n"
        f"model_sha256={package_digest}\n"
        f"evidence_code_sha256={evidence_code_digest}\n"
        f"java_binary_sha256={runtime['binary_sha256']}\n"
        f"java_binary_bytes={runtime['binary_bytes']}\n"
        f"java_version_output_sha256={runtime['version_output_sha256']}\n"
        f"java_version_output_bytes={runtime['version_output_bytes']}\n"
        f"seed={seed}\n"
        f"fingerprint_index={fingerprint_index}\n"
        f"workers={workers}\n"
    ).encode("utf-8")
    sections: list[bytes] = [metadata]
    for model in (COUNT_MODEL, INDEXED_MODEL):
        sections.append(_read_sany(sany_dir, model))
    for name, expected_outcome, model in CONFIGURATIONS:
        summary, section = _read_run(
            logs_dir,
            name,
            model,
            expected_outcome,
            seed=seed,
            fingerprint_index=fingerprint_index,
            workers=workers,
            tlc_version=tlc_version,
        )
        summaries.append(summary)
        sections.append(section)
    transcript = b"".join(sections)
    transcript_digest = hashlib.sha256(transcript).hexdigest()
    report: dict[str, object] = {
        "version": REPORT_VERSION,
        "protocol": PROTOCOL,
        "commit": commit,
        "tool": "TLC",
        "tool_version": tool_version,
        "tool_sha256": tool_sha256,
        "model_sha256": package_digest,
        "evidence_code_sha256": evidence_code_digest,
        "java_runtime": runtime,
        "configurations": [summary.as_json() for summary in summaries],
        "passed": True,
        "transcript": {
            "path": artifact_path.as_posix(),
            "sha256": transcript_digest,
            "bytes": len(transcript),
        },
    }
    return report, transcript


def write_report(
    *,
    report: dict[str, object],
    transcript: bytes,
    report_output: Path,
    transcript_output: Path,
) -> None:
    """Write a validated report and transcript without replacing existing evidence."""

    if report_output == transcript_output:
        raise ReportError("report and transcript outputs must be different files")
    for path in (report_output, transcript_output):
        if path.exists():
            raise ReportError(f"refusing to replace existing evidence file {path}")
        path.parent.mkdir(parents=True, exist_ok=True)
    transcript_output.write_bytes(transcript)
    report_output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--formal-dir", required=True, type=Path)
    parser.add_argument("--logs-dir", required=True, type=Path)
    parser.add_argument("--sany-dir", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument("--tool-sha256", required=True)
    parser.add_argument("--java-binary-sha256", required=True)
    parser.add_argument("--java-binary-bytes", required=True, type=int)
    parser.add_argument("--java-version-output", required=True, type=Path)
    parser.add_argument("--seed", required=True, type=int)
    parser.add_argument("--fingerprint-index", required=True, type=int)
    parser.add_argument("--workers", required=True)
    parser.add_argument("--transcript-artifact-path", required=True)
    parser.add_argument("--report-output", required=True, type=Path)
    parser.add_argument("--transcript-output", required=True, type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the report generator command line."""

    args = _parser().parse_args(argv)
    try:
        report, transcript = build_report(
            formal_dir=args.formal_dir,
            logs_dir=args.logs_dir,
            sany_dir=args.sany_dir,
            commit=args.commit,
            tool_version=args.tool_version,
            tool_sha256=args.tool_sha256,
            java_binary_sha256=args.java_binary_sha256,
            java_binary_bytes=args.java_binary_bytes,
            java_version_output_path=args.java_version_output,
            seed=args.seed,
            fingerprint_index=args.fingerprint_index,
            workers=args.workers,
            transcript_artifact_path=args.transcript_artifact_path,
        )
        write_report(
            report=report,
            transcript=transcript,
            report_output=args.report_output,
            transcript_output=args.transcript_output,
        )
    except ReportError as error:
        raise SystemExit(f"private-settlement TLC report rejected: {error}") from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
