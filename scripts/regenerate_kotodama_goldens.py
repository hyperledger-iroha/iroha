#!/usr/bin/env python3
"""Build, validate, and atomically publish the canonical Kotodama V1 goldens.

Prerequisites are freshly built ``koto`` and ``iroha`` binaries. The script
never invokes Cargo, accepts no signing material, and writes only below
``target/kotodama`` unless ``--write`` is explicitly selected. The checked-in
``ivm_artifacts.tsv`` file is the authoritative ownership and
source-to-artifact map for every IVM program in the repository.

``--check`` is the safe default: compile everything in a temporary staging
tree and fail if any checked-in artifact or compiler manifest differs.
``--write`` prevalidates the complete staging tree before replacing changed
destinations one file at a time. The signed prediction-market deployment
manifest is intentionally untracked and outside this workflow because private
signing input must remain runtime-only.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


MAX_CYCLES = 1_000_000
HEADER_SIZE = 49
ZK_MODE_BIT = 0x01
VECTOR_MODE_BIT = 0x02
KNOWN_CONTRACT_MODE_BITS = ZK_MODE_BIT | VECTOR_MODE_BIT
MAP_PATH = Path("scripts/ivm_artifacts.tsv")
SIZE_BASELINE_PATH = Path("scripts/kotodama_v1_size_baseline.json")
TEST_SOURCE = Path("crates/ivm/docs/examples/19_contract_flow_test.test.ko")
FILTERED_TEST_FRAGMENT = "actor_helpers"
EXACT_TEST_NAME = "actor_helpers_roundtrip"
SIZE_BASELINE_SCHEMA = "kotodama-v1-size-baseline-v1"
SIZE_BASELINE_CORPUS = "kotodama-v1-audited-padding-heavy-control-flow"
# `ADDI r0, r0, 0` is reserved by the Kotodama assembler as its one-word
# relocation placeholder. A deployable image may contain it only when it was
# deliberately emitted as a semantic no-op; V1 keeps such words below 1%.
RELOCATION_NOP_WORD = 0x2000_0000
MAX_RELOCATION_PADDING_PERCENT = 1
LITERAL_CRC16_POLYNOMIAL = 0x1021
LITERAL_CRC16_INITIAL = 0xFFFF
EXTRA_ZK_SOURCES = frozenset(
    {
        Path("tools/kotodama_linguist/samples/zk_bridge.ko"),
    }
)
COMPILER_MANIFESTS = {
    Path("demo/authority_probe.ko"): Path("demo/authority_probe.manifest.json"),
    Path("crates/kotodama_lang/src/samples/irohaswap.ko"): Path(
        "demo/irohaswap.manifest.json"
    ),
    Path("demo/ivm_smoke.ko"): Path("demo/ivm_smoke.manifest.json"),
    Path("demo/prediction_market.ko"): Path(
        "demo/prediction_market.manifest.json"
    ),
}
RETIRED_OUTPUTS = (
    Path("crates/ivm/docs/examples/01_init.to"),
    Path("crates/ivm/docs/examples/02_entry_fn.to"),
    Path("crates/ivm/docs/examples/03_upgrade.to"),
    Path("crates/ivm/docs/examples/16_dynamic_take.to"),
    Path("crates/ivm/docs/examples/17_dynamic_range.to"),
    Path("crates/ivm/tests/data/dai.to"),
    Path("crates/kotodama_lang/src/samples/kotodama_jp.to"),
    Path("crates/kotodama_lang/src/samples/trigger_cat_and_mouse.to"),
    Path("docs/portal/static/norito-snippets/init-entrypoint.to"),
    Path("integration_tests/fixtures/ivm/trigger_cat_and_mouse.to"),
)


class GoldenError(RuntimeError):
    """Raised when source inventory, compilation, or artifact validation fails."""


@dataclass(frozen=True)
class Golden:
    """One canonical source, execution mode, and checked-in destination."""

    mode: str
    source: Path
    destination: Path


@dataclass(frozen=True)
class ArtifactCodeMetrics:
    """Executable-region measurements used by the V1 size gate."""

    code_offset: int
    code_bytes: int
    instruction_words: int
    relocation_nop_words: int


def repository_root() -> Path:
    """Return the repository root containing this script's parent directory."""

    return Path(__file__).resolve().parents[1]


def _safe_relative(raw: str, suffix: str, context: str) -> Path:
    path = Path(raw)
    if (
        path.is_absolute()
        or not path.parts
        or ".." in path.parts
        or path.suffix != suffix
    ):
        raise GoldenError(f"invalid {context} path {raw!r}")
    return path


def read_map(path: Path) -> list[Golden]:
    """Read and strictly validate the canonical TSV artifact map."""

    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise GoldenError(f"failed to read golden map {path}: {error}") from error
    if not lines:
        raise GoldenError(f"golden map {path} is empty")

    rows: list[Golden] = []
    destinations: set[Path] = set()
    source_modes: dict[Path, str] = {}
    for number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 3:
            raise GoldenError(f"invalid golden map row {number}: {line!r}")
        if fields[0] in {"predecoder", "synthetic", "default"}:
            continue
        if fields[0] not in {"kotodama-standard", "kotodama-zk"}:
            raise GoldenError(f"invalid golden map row {number}: {line!r}")
        mode = fields[0].removeprefix("kotodama-")
        source = _safe_relative(fields[1], ".ko", f"row {number} source")
        destination = _safe_relative(fields[2], ".to", f"row {number} output")
        previous_mode = source_modes.setdefault(source, mode)
        if previous_mode != mode:
            raise GoldenError(f"source {source} has conflicting execution modes")
        if destination in destinations:
            raise GoldenError(f"duplicate golden destination {destination}")
        destinations.add(destination)
        rows.append(Golden(mode, source, destination))
    if not rows:
        raise GoldenError(f"golden map {path} contains no Kotodama artifacts")
    return rows


def tracked_sources(root: Path) -> list[Path]:
    """Return every present versioned or newly added Kotodama source."""

    result = subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.ko",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise GoldenError(f"failed to inventory tracked Kotodama sources: {message}")
    candidates = [
        _safe_relative(raw.decode("utf-8"), ".ko", "source inventory")
        for raw in result.stdout.split(b"\0")
        if raw
    ]
    sources = [source for source in candidates if (root / source).is_file()]
    if not sources:
        raise GoldenError("repository contains no tracked Kotodama sources")
    return sorted(sources)


def tracked_outputs(root: Path) -> list[Path]:
    """Return every present versioned ``.to`` file, including staged additions."""

    result = subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "ls-files",
            "-z",
            "--cached",
            "--",
            "*.to",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise GoldenError(f"failed to inventory tracked .to outputs: {message}")
    candidates = [
        _safe_relative(raw.decode("utf-8"), ".to", "output inventory")
        for raw in result.stdout.split(b"\0")
        if raw
    ]
    return sorted(output for output in candidates if (root / output).is_file())


def compiler_owned_outputs(
    sources: Sequence[Path], outputs: Sequence[Path]
) -> list[Path]:
    """Select tracked ``.to`` files owned by the Kotodama compiler.

    Kotodama's primary outputs and copied aliases preserve the source filename
    stem. Therefore a tracked ``.to`` belongs to this workflow exactly when its
    stem matches a present ``.ko`` source stem. Other uses of the ``.to`` suffix
    (notably canonical Norito payloads and hand-authored IVM data fixtures) have
    no corresponding ``.ko`` stem and are deliberately outside this inventory.
    """

    source_stems = {source.stem for source in sources}
    return sorted(output for output in outputs if output.stem in source_stems)


def validate_output_inventory(
    rows: Sequence[Golden], sources: Sequence[Path], outputs: Sequence[Path]
) -> None:
    """Require every compiler-owned tracked artifact to have an explicit row."""

    mapped = {row.destination: row for row in rows}
    source_set = set(sources)
    owned = compiler_owned_outputs(sources, outputs)
    unmapped = [output for output in owned if output not in mapped]
    if unmapped:
        raise GoldenError(
            "compiler-owned tracked .to artifacts are missing explicit golden "
            "map rows: " + ", ".join(path.as_posix() for path in unmapped)
        )
    invalid = [
        output
        for output in owned
        if mapped[output].source not in source_set
        or mapped[output].source.stem != output.stem
    ]
    if invalid:
        raise GoldenError(
            "compiler-owned tracked .to artifacts have invalid source mappings: "
            + ", ".join(path.as_posix() for path in invalid)
        )


def unique_builds(rows: Sequence[Golden]) -> list[Golden]:
    """Deduplicate alias outputs and reject ambiguous staged file stems."""

    by_source: dict[Path, Golden] = {}
    by_stem: dict[str, Path] = {}
    for row in rows:
        by_source.setdefault(row.source, row)
        previous = by_stem.setdefault(row.source.stem, row.source)
        if previous != row.source:
            raise GoldenError(
                f"staged stem collision: {previous} and {row.source} both use "
                f"{row.source.stem!r}"
            )
    return sorted(by_source.values(), key=lambda row: row.source.as_posix())


def run(command: Sequence[os.PathLike[str] | str], root: Path) -> str:
    """Run one non-secret tool command and return its UTF-8 stdout."""

    rendered = [os.fspath(part) for part in command]
    result = subprocess.run(
        rendered,
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        details = result.stderr.strip() or result.stdout.strip()
        raise GoldenError(f"command failed ({' '.join(rendered)}):\n{details}")
    return result.stdout


def validate_noop_build_output(output: str, expected_fresh: int) -> None:
    """Require one unambiguous `fresh` notice for every requested source."""

    notices = [line.strip() for line in output.splitlines() if line.strip()]
    if len(notices) != expected_fresh or any(
        not notice.startswith("fresh ") or len(notice) == len("fresh ")
        for notice in notices
    ):
        raise GoldenError("no-op Kotodama graph performed compilation")


def literal_checksum(tag: str, body: str) -> str:
    """Return Norito's canonical four-hex-digit literal checksum."""

    checksum = LITERAL_CRC16_INITIAL
    for byte in f"{tag}:{body}".encode("ascii"):
        checksum ^= byte << 8
        for _ in range(8):
            checksum = (
                ((checksum << 1) ^ LITERAL_CRC16_POLYNOMIAL) & 0xFFFF
                if checksum & 0x8000
                else (checksum << 1) & 0xFFFF
            )
    return f"{checksum:04X}"


def manifest_abi_hash(path: Path) -> bytes:
    """Return the exact 32-byte ABI digest declared by a compiler manifest."""

    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GoldenError(f"failed to read compiler manifest {path}: {error}") from error
    value = document.get("abi_hash") if isinstance(document, dict) else None
    if not isinstance(value, str) or not value.startswith("hash:"):
        raise GoldenError(f"{path} does not declare a canonical ABI hash")
    digest, separator, checksum = value[5:].partition("#")
    if (
        not separator
        or len(digest) != 64
        or len(checksum) != 4
        or any(character not in "0123456789ABCDEF" for character in digest + checksum)
    ):
        raise GoldenError(f"{path} contains a malformed ABI hash")
    if checksum != literal_checksum("hash", digest):
        raise GoldenError(f"{path} contains a noncanonical ABI hash checksum")
    return bytes.fromhex(digest)


def validate_artifact(path: Path, mode: str, expected_abi_hash: bytes) -> None:
    """Fail closed on a non-canonical V1 header or embedded debug section."""

    try:
        artifact = path.read_bytes()
    except OSError as error:
        raise GoldenError(f"failed to read staged artifact {path}: {error}") from error
    if len(artifact) < HEADER_SIZE + 8 or artifact[:4] != b"IVM\0":
        raise GoldenError(f"{path} is not an IVM artifact")
    if artifact[4:6] != b"\x01\x01":
        raise GoldenError(f"{path} does not use canonical IVM 1.1")
    if artifact[16] != 1:
        raise GoldenError(f"{path} does not use ABI v1")
    if len(expected_abi_hash) != 32 or artifact[17:HEADER_SIZE] != expected_abi_hash:
        raise GoldenError(f"{path} does not authenticate the compiler ABI hash")
    if struct.unpack_from("<Q", artifact, 8)[0] != MAX_CYCLES:
        raise GoldenError(f"{path} does not embed the {MAX_CYCLES}-cycle ceiling")
    header_mode = artifact[6]
    if header_mode & ~KNOWN_CONTRACT_MODE_BITS:
        raise GoldenError(f"{path} contains unknown execution-mode bits")
    if artifact[7] != 0:
        raise GoldenError(f"{path} contains a noncanonical vector-length override")
    is_zk = bool(header_mode & ZK_MODE_BIT)
    if is_zk != (mode == "zk"):
        raise GoldenError(f"{path} has the wrong ZK execution bit for mode {mode}")
    if artifact[HEADER_SIZE : HEADER_SIZE + 4] != b"CNTR":
        raise GoldenError(f"{path} is missing the required CNTR interface")
    interface_length = struct.unpack_from("<I", artifact, HEADER_SIZE + 4)[0]
    after_interface = HEADER_SIZE + 8 + interface_length
    if after_interface > len(artifact):
        raise GoldenError(f"{path} contains a truncated CNTR interface")
    if artifact[after_interface : after_interface + 4] == b"DBG1":
        raise GoldenError(f"{path} embeds forbidden debug metadata")


def artifact_code_metrics(path: Path) -> ArtifactCodeMetrics:
    """Locate and measure a canonical compiler-generated instruction region."""

    try:
        artifact = path.read_bytes()
    except OSError as error:
        raise GoldenError(f"failed to read staged artifact {path}: {error}") from error
    if len(artifact) < HEADER_SIZE + 8 or artifact[:4] != b"IVM\0":
        raise GoldenError(f"{path} is not an IVM artifact")

    offset = HEADER_SIZE
    if artifact[offset : offset + 4] != b"CNTR":
        raise GoldenError(f"{path} is missing the required CNTR interface")
    interface_length = struct.unpack_from("<I", artifact, offset + 4)[0]
    offset += 8 + interface_length
    if offset > len(artifact):
        raise GoldenError(f"{path} contains a truncated CNTR interface")
    if artifact[offset : offset + 4] == b"DBG1":
        raise GoldenError(f"{path} embeds forbidden debug metadata")

    if artifact[offset : offset + 4] == b"LTLB":
        if offset + 16 > len(artifact):
            raise GoldenError(f"{path} contains a truncated literal-table header")
        literal_count, alignment_padding, data_length = struct.unpack_from(
            "<III", artifact, offset + 4
        )
        if literal_count > 65_536 or alignment_padding > 3:
            raise GoldenError(f"{path} contains invalid literal-table dimensions")
        offset += 16 + literal_count * 8 + data_length + alignment_padding
        if offset > len(artifact):
            raise GoldenError(f"{path} contains a truncated literal table")

    code_bytes = len(artifact) - offset
    if code_bytes == 0 or code_bytes % 4 != 0:
        raise GoldenError(
            f"{path} executable region must be non-empty and word aligned, "
            f"got {code_bytes} bytes"
        )
    words = struct.iter_unpack("<I", artifact[offset:])
    relocation_nops = sum(word == RELOCATION_NOP_WORD for (word,) in words)
    return ArtifactCodeMetrics(
        code_offset=offset,
        code_bytes=code_bytes,
        instruction_words=code_bytes // 4,
        relocation_nop_words=relocation_nops,
    )


def read_size_baseline(path: Path) -> dict[Path, int]:
    """Read immutable pre-reset evidence for the normative V1 size corpus."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GoldenError(f"failed to read Kotodama size baseline {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GoldenError(f"invalid Kotodama size baseline JSON {path}: {error}") from error
    if not isinstance(payload, dict) or payload.get("schema") != SIZE_BASELINE_SCHEMA:
        raise GoldenError(
            f"Kotodama size baseline {path} must declare schema "
            f"{SIZE_BASELINE_SCHEMA!r}"
        )
    if payload.get("unit") != "code_bytes":
        raise GoldenError(f"Kotodama size baseline {path} must use code_bytes")
    if payload.get("corpus") != SIZE_BASELINE_CORPUS:
        raise GoldenError(
            f"Kotodama size baseline {path} must bind the normative corpus "
            f"{SIZE_BASELINE_CORPUS!r}"
        )
    source_revision = payload.get("source_revision")
    if (
        not isinstance(source_revision, str)
        or len(source_revision) != 40
        or any(character not in "0123456789abcdef" for character in source_revision)
    ):
        raise GoldenError(
            f"Kotodama size baseline {path} must bind a lowercase 40-hex source_revision"
        )
    samples = payload.get("samples")
    if not isinstance(samples, dict) or not samples:
        raise GoldenError(f"Kotodama size baseline {path} has no samples")

    result: dict[Path, int] = {}
    for raw_path, raw_size in samples.items():
        if not isinstance(raw_path, str):
            raise GoldenError(f"Kotodama size baseline {path} contains a non-string path")
        artifact_path = _safe_relative(raw_path, ".to", "size baseline")
        if isinstance(raw_size, bool) or not isinstance(raw_size, int):
            raise GoldenError(f"size baseline for {artifact_path} must be an integer")
        if raw_size <= 0 or raw_size % 4 != 0:
            raise GoldenError(
                f"size baseline for {artifact_path} must be positive and word aligned"
            )
        result[artifact_path] = raw_size
    return result


def validate_performance(
    stage: Path,
    builds: Sequence[Golden],
    rows: Sequence[Golden],
    baseline_path: Path,
) -> None:
    """Enforce relocation-padding and representative code-size acceptance gates."""

    metrics_by_source: dict[Path, ArtifactCodeMetrics] = {}
    for build in builds:
        artifact = stage / "release" / f"{build.source.stem}.to"
        metrics = artifact_code_metrics(artifact)
        metrics_by_source[build.source] = metrics
        # Strictly below 1%, expressed with integer arithmetic so boundary
        # behavior is deterministic on every host.
        if (
            metrics.relocation_nop_words * 100
            >= metrics.instruction_words * MAX_RELOCATION_PADDING_PERCENT
        ):
            raise GoldenError(
                f"{build.source} retains {metrics.relocation_nop_words}/"
                f"{metrics.instruction_words} relocation NOP words; V1 requires "
                "strictly less than 1%"
            )

    by_destination = {row.destination: row for row in rows}
    for destination, old_code_bytes in read_size_baseline(baseline_path).items():
        row = by_destination.get(destination)
        if row is None:
            raise GoldenError(
                f"size baseline references unmapped Kotodama artifact {destination}"
            )
        metrics = metrics_by_source.get(row.source)
        if metrics is None:
            raise GoldenError(f"size baseline source was not built: {row.source}")
        if metrics.code_bytes * 2 > old_code_bytes:
            raise GoldenError(
                f"{destination} code region is {metrics.code_bytes} bytes; V1 requires "
                f"at least a 50% reduction from the audited {old_code_bytes}-byte baseline"
            )


def atomic_publish(source: Path, destination: Path) -> bool:
    """Replace one destination atomically only when its bytes changed."""

    payload = source.read_bytes()
    try:
        if destination.read_bytes() == payload:
            return False
    except FileNotFoundError:
        pass
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)
    return True


def compare_file(source: Path, destination: Path) -> None:
    """Require a checked-in destination to exactly match its staged source."""

    try:
        expected = source.read_bytes()
        actual = destination.read_bytes()
    except OSError as error:
        raise GoldenError(f"failed to compare {destination}: {error}") from error
    if actual != expected:
        raise GoldenError(f"stale Kotodama generated output: {destination}")


def build_command(
    koto: Path, stage: Path, mode: str, sources: Iterable[Path]
) -> list[str]:
    """Construct one deterministic batch build command."""

    command = [
        str(koto),
        "build",
        "--profile",
        "release",
        "--target-dir",
        str(stage),
        "--max-cycles",
        str(MAX_CYCLES),
    ]
    if mode == "zk":
        command.append("--zk")
    command.extend(path.as_posix() for path in sources)
    return command


def validate_sources(koto: Path, root: Path, sources: Sequence[Path]) -> None:
    """Format-check and compile-check every tracked source under its V1 mode."""

    if TEST_SOURCE not in sources:
        raise GoldenError(f"missing canonical test module {TEST_SOURCE}")
    ordinary = [source for source in sources if source != TEST_SOURCE]
    zk_sources = sorted(
        {
            *EXTRA_ZK_SOURCES,
            *(
                row.source
                for row in read_map(root / MAP_PATH)
                if row.mode == "zk"
            ),
        }
    )
    missing_zk = sorted(set(zk_sources) - set(ordinary))
    if missing_zk:
        raise GoldenError(
            "configured ZK sources are missing: "
            + ", ".join(path.as_posix() for path in missing_zk)
        )
    standard_sources = sorted(set(ordinary) - set(zk_sources))

    run([koto, "fmt", "--check", *sources], root)
    run([koto, "check", "--format", "human", *standard_sources], root)
    run([koto, "check", "--format", "human", "--zk", *zk_sources], root)


def contract_test_commands(
    koto: Path, stage: Path
) -> tuple[list[os.PathLike[str] | str], ...]:
    """Return the exact fail-closed acceptance command inventory."""

    return (
        [koto, "test", "list", TEST_SOURCE],
        [
            koto,
            "test",
            "run",
            "--filter",
            FILTERED_TEST_FRAGMENT,
            TEST_SOURCE,
        ],
        [
            koto,
            "test",
            "run",
            "--filter",
            EXACT_TEST_NAME,
            "--exact",
            TEST_SOURCE,
        ],
        [
            koto,
            "test",
            "run",
            "--jobs",
            "2",
            "--seed",
            "0",
            "--format",
            "json",
            TEST_SOURCE,
        ],
        [
            koto,
            "test",
            "run",
            "--jobs",
            "2",
            "--seed",
            "0",
            "--junit",
            stage / "contract-flow-tests.xml",
            TEST_SOURCE,
        ],
    )


def run_contract_tests(koto: Path, root: Path, stage: Path) -> None:
    """Exercise list/filter/exact and parallel JSON/JUnit runner paths."""

    commands = contract_test_commands(koto, stage)
    for command in commands[:3]:
        run(command, root)
    json_output = run(commands[3], root)
    (stage / "contract-flow-tests.json").write_text(json_output, encoding="utf-8")
    run(commands[4], root)
    validate_contract_test_reports(
        json_output,
        stage / "contract-flow-tests.xml",
    )


def validate_contract_test_reports(json_output: str, junit_path: Path) -> None:
    """Require equivalent, successful canonical JSON and JUnit reports."""

    try:
        report = json.loads(json_output)
    except json.JSONDecodeError as error:
        raise GoldenError(f"koto test emitted invalid JSON: {error}") from error
    expected_report_keys = {"target", "seed", "passed", "failed", "tests"}
    if not isinstance(report, dict) or set(report) != expected_report_keys:
        raise GoldenError("koto test JSON has a noncanonical report shape")
    tests = report["tests"]
    if (
        not isinstance(report["target"], str)
        or not report["target"]
        or isinstance(report["seed"], bool)
        or report["seed"] != 0
        or isinstance(report["passed"], bool)
        or not isinstance(report["passed"], int)
        or isinstance(report["failed"], bool)
        or not isinstance(report["failed"], int)
        or not isinstance(tests, list)
        or not tests
        or report["passed"] != len(tests)
        or report["failed"] != 0
    ):
        raise GoldenError("koto test JSON does not describe a complete successful run")

    expected_test_keys = {"name", "line", "passed", "duration_ns", "failure"}
    names: list[str] = []
    for test in tests:
        if not isinstance(test, dict) or set(test) != expected_test_keys:
            raise GoldenError("koto test JSON contains a noncanonical test result")
        if (
            not isinstance(test["name"], str)
            or not test["name"]
            or isinstance(test["line"], bool)
            or not isinstance(test["line"], int)
            or test["line"] <= 0
            or test["passed"] is not True
            or isinstance(test["duration_ns"], bool)
            or not isinstance(test["duration_ns"], int)
            or test["duration_ns"] < 0
            or test["failure"] is not None
        ):
            raise GoldenError("koto test JSON contains an invalid successful test result")
        names.append(test["name"])
    if len(names) != len(set(names)):
        raise GoldenError("koto test JSON contains duplicate test names")

    try:
        suite = ElementTree.parse(junit_path).getroot()
    except (OSError, ElementTree.ParseError) as error:
        raise GoldenError(f"koto test emitted invalid JUnit XML: {error}") from error
    try:
        junit_tests = int(suite.attrib["tests"])
        junit_failures = int(suite.attrib["failures"])
        junit_seed = int(suite.attrib["seed"])
        junit_duration = float(suite.attrib["time"])
    except (KeyError, TypeError, ValueError) as error:
        raise GoldenError("koto test JUnit has invalid summary attributes") from error
    cases = list(suite)
    if (
        suite.tag != "testsuite"
        or set(suite.attrib) != {"name", "tests", "failures", "time", "seed"}
        or suite.attrib["name"] != report["target"]
        or junit_tests != len(tests)
        or junit_failures != 0
        or junit_seed != 0
        or not math.isfinite(junit_duration)
        or junit_duration < 0.0
        or len(cases) != len(tests)
    ):
        raise GoldenError("koto test JUnit does not match the successful JSON run")
    junit_names: list[str] = []
    for case in cases:
        if (
            case.tag != "testcase"
            or set(case.attrib) != {"name", "classname", "line", "time"}
            or case.attrib["classname"] != report["target"]
            or list(case)
        ):
            raise GoldenError("koto test JUnit contains a noncanonical test case")
        try:
            line = int(case.attrib["line"])
            duration = float(case.attrib["time"])
        except (KeyError, TypeError, ValueError) as error:
            raise GoldenError("koto test JUnit contains invalid case attributes") from error
        if line <= 0 or not math.isfinite(duration) or duration < 0.0:
            raise GoldenError("koto test JUnit contains invalid case values")
        junit_names.append(case.attrib["name"])
    if junit_names != names:
        raise GoldenError("koto test JSON and JUnit test inventories differ")


def verify_runtime_manifests(
    iroha: Path, root: Path, stage: Path, builds: Sequence[Golden]
) -> None:
    """Cross-check compiler manifests with the independent runtime CLI parser."""

    verified = stage / "verified"
    verified.mkdir()
    for row in builds:
        stem = row.source.stem
        artifact = stage / "release" / f"{stem}.to"
        generated = stage / "release" / f"{stem}.manifest.json"
        runtime_manifest = verified / f"{stem}.manifest.json"
        run(
            [
                iroha,
                "--machine",
                "contract",
                "manifest",
                "build",
                "--code-file",
                artifact,
                "--out",
                runtime_manifest,
            ],
            root,
        )
        compare_file(generated, runtime_manifest)


def build_and_validate(
    koto: Path,
    iroha: Path | None,
    root: Path,
    stage: Path,
    rows: Sequence[Golden],
    run_tests: bool,
) -> list[Golden]:
    """Build each unique source once, authenticate outputs, and prove no-op reuse."""

    builds = unique_builds(rows)
    commands: list[tuple[list[str], int]] = []
    for mode in ("standard", "zk"):
        selected = [row.source for row in builds if row.mode == mode]
        if selected:
            commands.append((build_command(koto, stage, mode, selected), len(selected)))
    for command, _ in commands:
        run(command, root)

    for row in builds:
        artifact = stage / "release" / f"{row.source.stem}.to"
        manifest = stage / "release" / f"{row.source.stem}.manifest.json"
        validate_artifact(artifact, row.mode, manifest_abi_hash(manifest))
    validate_performance(stage, builds, rows, root / SIZE_BASELINE_PATH)
    if iroha is not None:
        verify_runtime_manifests(iroha, root, stage, builds)
    if run_tests:
        run_contract_tests(koto, root, stage)

    before_paths = sorted(path for path in (stage / "release").rglob("*") if path.is_file())
    before = {path: path.stat().st_mtime_ns for path in before_paths}
    for command, expected_fresh in commands:
        output = run(command, root)
        validate_noop_build_output(output, expected_fresh)
    after_paths = sorted(path for path in (stage / "release").rglob("*") if path.is_file())
    if after_paths != before_paths:
        raise GoldenError("no-op Kotodama graph changed the generated output inventory")
    after = {path: path.stat().st_mtime_ns for path in before}
    if before != after:
        raise GoldenError("no-op Kotodama graph rewrote generated output")
    return builds


def publish_or_check(
    root: Path,
    stage: Path,
    rows: Sequence[Golden],
    write: bool,
) -> int:
    """Publish or compare every mapped artifact and selected compiler manifest."""

    changed = 0
    for row in rows:
        staged = stage / "release" / f"{row.source.stem}.to"
        destination = root / row.destination
        if write:
            changed += int(atomic_publish(staged, destination))
        else:
            compare_file(staged, destination)
    for source, relative_destination in COMPILER_MANIFESTS.items():
        staged = stage / "release" / f"{source.stem}.manifest.json"
        destination = root / relative_destination
        if write:
            changed += int(atomic_publish(staged, destination))
        else:
            compare_file(staged, destination)
    for retired in RETIRED_OUTPUTS:
        path = root / retired
        if path.exists():
            if not write:
                raise GoldenError(f"retired Kotodama artifact still exists: {retired}")
            path.unlink()
            changed += 1
    return changed


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse the discoverable command-line interface."""

    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="verify without publishing (default)")
    mode.add_argument("--write", action="store_true", help="publish all prevalidated changes")
    parser.add_argument("--koto", type=Path, default=Path("target/debug/koto"))
    parser.add_argument("--iroha", type=Path, default=Path("target/debug/iroha"))
    parser.add_argument(
        "--skip-runtime-manifest-check",
        action="store_true",
        help="skip independent iroha manifest verification for local iteration",
    )
    parser.add_argument(
        "--skip-contract-tests",
        action="store_true",
        help="skip the canonical koto test JSON/JUnit run for local iteration",
    )
    return parser.parse_args(argv)


def _resolve_tool(root: Path, path: Path, name: str) -> Path:
    tool = path if path.is_absolute() else root / path
    if not tool.is_file() or not os.access(tool, os.X_OK):
        raise GoldenError(f"{name} binary is missing or not executable: {tool}")
    return tool


def main(argv: Sequence[str] | None = None) -> int:
    """Run the complete fail-closed golden pipeline."""

    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = repository_root()
    try:
        rows = read_map(root / MAP_PATH)
        sources = tracked_sources(root)
        validate_output_inventory(rows, sources, tracked_outputs(root))
        koto = _resolve_tool(root, args.koto, "koto")
        iroha = (
            None
            if args.skip_runtime_manifest_check
            else _resolve_tool(root, args.iroha, "iroha")
        )
        validate_sources(koto, root, sources)
        target = root / "target" / "kotodama"
        target.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="v1-goldens.", dir=target) as raw_stage:
            stage = Path(raw_stage)
            build_and_validate(
                koto,
                iroha,
                root,
                stage,
                rows,
                not args.skip_contract_tests,
            )
            changed = publish_or_check(root, stage, rows, args.write)
    except (GoldenError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    action = "published" if args.write else "verified"
    print(f"{action} {len(rows)} Kotodama V1 artifact mappings ({changed} changes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
