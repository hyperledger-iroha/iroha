#!/usr/bin/env python3
"""Validate the Sumeragi v2 proof ledger and reject unchecked proof escapes."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
FORMAL_DIR = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
LEDGER_PATH = FORMAL_DIR / "proof_coverage.json"
VERUS_SOURCE_DIR = ROOT_DIR / "crates" / "iroha_sumeragi_core" / "src"

STATUS_VALUES = (
    "tlaps_proved",
    "specified_unproved",
    "trusted_contract",
    "out_of_scope",
)

# These are the deductive release modules.  TLC configurations are deliberately
# absent: a finite counterexample search can never satisfy a proof obligation.
RELEASE_PROOF_MODULES = (
    "SumeragiV2QuorumProofs",
    "SumeragiV2Availability",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Reconfiguration",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2LivenessProofs",
    "SumeragiV2AsyncLivenessProofs",
)

REQUIRED_MODEL_MODULES = (
    "SumeragiV2",
    "SumeragiV2Quorums",
    "SumeragiV2QuorumProofs",
    "SumeragiV2Availability",
    "SumeragiV2Core",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Reconfiguration",
    "SumeragiV2SafetyDefinitions",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2Inductive",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2ChainEpoch",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2LivenessProofs",
    "SumeragiV2AsyncNetwork",
    "SumeragiV2AsyncLivenessProofs",
)

REQUIRED_TLC_CONFIGS = (
    "quorum_count.cfg",
    "quorum_stake.cfg",
    "safety_count.cfg",
    "safety_stake.cfg",
    "chain_epoch.cfg",
    "liveness.cfg",
)

RETIRED_PATHS = (
    ROOT_DIR / "docs" / "formal" / "sumeragi",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_tlc.sh",
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_formal_coverage.py",
    ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh",
)

MODULE_HEADER_RE = re.compile(r"(?m)^---- MODULE ([A-Za-z_][A-Za-z0-9_]*) ----$")
DECLARATION_TEMPLATE = r"(?m)^\s*(?:(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+)?{symbol}\s*(?:\([^\n]*\))?\s*=="
TOP_LEVEL_TRUST_RE = re.compile(
    r"(?mi)^\s*(?:ASSUME(?:S|PTION|PTIONS)?|AXIOM(?:S)?)\b"
)
OMITTED_RE = re.compile(r"(?i)\bOMITTED\b")
VERUS_ESCAPE_RE = re.compile(
    r"(?i)(?:\b(?:assume|admit)\s*!?\s*\(|"
    r"#\s*\[\s*verifier\s*::\s*(?:external_body|external_fn_specification|"
    r"external_type_specification|assume_specification|trusted)\s*\])"
)


class DuplicateKeyError(ValueError):
    """Raised when a JSON object repeats a key."""


@dataclass(frozen=True)
class LedgerValidation:
    """Validation result returned to tests and the command-line entry point."""

    errors: tuple[str, ...]
    machine_checked_completion: bool


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateKeyError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_ledger(path: Path = LEDGER_PATH) -> dict[str, Any]:
    """Load a ledger while rejecting duplicate JSON keys."""

    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_unique_object)


def strip_tla_comments(source: str) -> str:
    """Remove nested TLA+ block comments and line comments, preserving lines."""

    output: list[str] = []
    index = 0
    depth = 0
    in_string = False
    while index < len(source):
        pair = source[index : index + 2]
        char = source[index]
        if depth:
            if pair == "(*":
                depth += 1
                output.extend("  ")
                index += 2
                continue
            if pair == "*)":
                depth -= 1
                output.extend("  ")
                index += 2
                continue
            output.append("\n" if char == "\n" else " ")
            index += 1
            continue
        if in_string:
            if pair == '\"\"':
                output.extend("  ")
                index += 2
                continue
            if char == '\"':
                output.append(char)
                in_string = False
            else:
                output.append("\n" if char == "\n" else " ")
            index += 1
            continue
        if pair == "(*":
            depth = 1
            output.extend("  ")
            index += 2
            continue
        if pair == "\\*":
            newline = source.find("\n", index)
            if newline == -1:
                output.extend(" " * (len(source) - index))
                break
            output.extend(" " * (newline - index))
            output.append("\n")
            index = newline + 1
            continue
        output.append(char)
        if char == '"':
            in_string = True
        index += 1
    return "".join(output)


def tla_shortcut_errors(path: Path, source: str) -> list[str]:
    """Find unchecked top-level assumptions, axioms, and omitted proofs."""

    stripped = strip_tla_comments(source)
    errors: list[str] = []
    for match in TOP_LEVEL_TRUST_RE.finditer(stripped):
        line = stripped.count("\n", 0, match.start()) + 1
        token = match.group(0).strip().split()[0]
        errors.append(f"{path}:{line}: unchecked top-level {token} is prohibited")
    for match in OMITTED_RE.finditer(stripped):
        line = stripped.count("\n", 0, match.start()) + 1
        errors.append(f"{path}:{line}: OMITTED proof is prohibited")
    return errors


def verus_shortcut_errors(path: Path, source: str) -> list[str]:
    """Find Verus assumption/admission and unreviewed trusted-body escapes."""

    errors: list[str] = []
    for match in VERUS_ESCAPE_RE.finditer(source):
        line = source.count("\n", 0, match.start()) + 1
        token = " ".join(match.group(0).split())
        errors.append(f"{path}:{line}: Verus proof escape is prohibited: {token}")
    return errors


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _symbol_names(symbol_field: str) -> tuple[str, ...]:
    return tuple(part.strip() for part in symbol_field.split("/") if part.strip())


def _symbol_exists(module_source: str, symbol: str) -> bool:
    pattern = re.compile(DECLARATION_TEMPLATE.format(symbol=re.escape(symbol)))
    return pattern.search(strip_tla_comments(module_source)) is not None


def _module_sources(formal_dir: Path) -> tuple[dict[str, str], list[str]]:
    sources: dict[str, str] = {}
    errors: list[str] = []
    for module in REQUIRED_MODEL_MODULES:
        path = formal_dir / f"{module}.tla"
        if not path.is_file():
            errors.append(f"missing required TLA+ module: {path}")
            continue
        source = path.read_text(encoding="utf-8")
        header = MODULE_HEADER_RE.search(source)
        if header is None or header.group(1) != module:
            errors.append(f"{path}: module header must declare {module}")
        if not source.rstrip().endswith("===="):
            errors.append(f"{path}: module must end with ====")
        errors.extend(tla_shortcut_errors(path, source))
        sources[module] = source
    return sources, errors


def _release_evidence_errors(ledger: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if ledger.get("machine_checked_completion") is not True:
        errors.append("release gate requires machine_checked_completion=true")

    obligations = ledger.get("obligations")
    if isinstance(obligations, list):
        for obligation in obligations:
            if not isinstance(obligation, dict):
                continue
            status = obligation.get("status")
            if status == "specified_unproved":
                errors.append(
                    f"release gate rejects unproved obligation: {obligation.get('id', '<unknown>')}"
                )

    run = ledger.get("last_tlaps_run")
    if not isinstance(run, dict):
        return errors + ["release gate requires last_tlaps_run evidence"]
    if run.get("backend_verification") is not True:
        errors.append("release gate requires a backend-verified last_tlaps_run")
    modules = run.get("modules")
    if not isinstance(modules, list):
        errors.append("last_tlaps_run.modules must be an array")
        return errors
    observed: set[str] = set()
    for entry in modules:
        if not isinstance(entry, dict):
            errors.append("last_tlaps_run module entries must be objects")
            continue
        module = entry.get("module")
        proved = entry.get("obligations_proved")
        if not _nonempty_string(module):
            errors.append("last_tlaps_run module is missing a name")
            continue
        if module in observed:
            errors.append(f"last_tlaps_run repeats module {module}")
        observed.add(module)
        if not isinstance(proved, int) or isinstance(proved, bool) or proved <= 0:
            errors.append(f"last_tlaps_run module {module} has no positive proved count")
    if observed != set(RELEASE_PROOF_MODULES):
        errors.append(
            "last_tlaps_run must cover exactly the release proof modules; "
            f"expected {sorted(RELEASE_PROOF_MODULES)}, found {sorted(observed)}"
        )
    return errors


def validate_ledger(
    ledger: dict[str, Any],
    *,
    formal_dir: Path = FORMAL_DIR,
    verus_source_dir: Path = VERUS_SOURCE_DIR,
    release: bool = False,
    check_retired_paths: bool = True,
) -> LedgerValidation:
    """Validate schema, source linkage, trust boundaries, and release evidence."""

    errors: list[str] = []
    if ledger.get("schema_version") != 1:
        errors.append("proof ledger schema_version must equal 1")
    if ledger.get("protocol") != "sumeragi-v2":
        errors.append("proof ledger protocol must equal sumeragi-v2")
    if ledger.get("status_values") != list(STATUS_VALUES):
        errors.append(f"proof ledger status_values must equal {list(STATUS_VALUES)}")
    completion = ledger.get("machine_checked_completion")
    if not isinstance(completion, bool):
        errors.append("machine_checked_completion must be a boolean")
        completion = False

    module_sources, module_errors = _module_sources(formal_dir)
    errors.extend(module_errors)
    for cfg_name in REQUIRED_TLC_CONFIGS:
        cfg = formal_dir / cfg_name
        if not cfg.is_file():
            errors.append(f"missing required TLC counterexample configuration: {cfg}")

    obligations = ledger.get("obligations")
    if not isinstance(obligations, list) or not obligations:
        errors.append("proof ledger obligations must be a non-empty array")
        obligations = []
    seen_ids: set[str] = set()
    for index, obligation in enumerate(obligations):
        where = f"obligations[{index}]"
        if not isinstance(obligation, dict):
            errors.append(f"{where} must be an object")
            continue
        obligation_id = obligation.get("id")
        requirement = obligation.get("requirement")
        module = obligation.get("module")
        symbol = obligation.get("symbol")
        status = obligation.get("status")
        for field_name, value in (
            ("id", obligation_id),
            ("requirement", requirement),
            ("module", module),
            ("symbol", symbol),
        ):
            if not _nonempty_string(value):
                errors.append(f"{where}.{field_name} must be a non-empty string")
        if not _nonempty_string(obligation_id):
            continue
        if obligation_id in seen_ids:
            errors.append(f"duplicate proof obligation id: {obligation_id}")
        seen_ids.add(obligation_id)
        if status not in STATUS_VALUES:
            errors.append(f"{where}.status has unknown value: {status!r}")
            continue
        if not _nonempty_string(module) or not _nonempty_string(symbol):
            continue
        if status in {"tlaps_proved", "specified_unproved"}:
            source = module_sources.get(module)
            if source is None:
                module_path = formal_dir / f"{module}.tla"
                if module_path.is_file():
                    source = module_path.read_text(encoding="utf-8")
                    errors.extend(tla_shortcut_errors(module_path, source))
                else:
                    errors.append(f"{where} references missing module {module}")
                    continue
            names = _symbol_names(symbol)
            if not names:
                errors.append(f"{where}.symbol contains no symbols")
            for name in names:
                if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
                    errors.append(f"{where}.symbol is not a TLA+ identifier: {name}")
                elif not _symbol_exists(source, name):
                    errors.append(f"{where} references missing symbol {module}!{name}")
        elif module != "trusted-boundary":
            errors.append(
                f"{where} with status {status} must use module trusted-boundary"
            )

    if verus_source_dir.is_dir():
        for path in sorted(verus_source_dir.rglob("*.rs")):
            errors.extend(verus_shortcut_errors(path, path.read_text(encoding="utf-8")))
    else:
        errors.append(f"missing Verus production proof source directory: {verus_source_dir}")

    if check_retired_paths:
        for path in RETIRED_PATHS:
            if path.exists():
                errors.append(f"retired Sumeragi v1 formal corridor still exists: {path}")

    if release:
        errors.extend(_release_evidence_errors(ledger))

    return LedgerValidation(tuple(errors), bool(completion))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release",
        action="store_true",
        help="fail unless every deductive obligation has backend proof evidence",
    )
    parser.add_argument(
        "--ledger",
        type=Path,
        default=LEDGER_PATH,
        help="proof ledger path (primarily for tests and release tooling)",
    )
    parser.add_argument(
        "--print-proof-modules",
        action="store_true",
        help="print the ordered deductive module list and exit",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.print_proof_modules:
        print("\n".join(RELEASE_PROOF_MODULES))
        return 0
    try:
        ledger = load_ledger(args.ledger)
    except (OSError, json.JSONDecodeError, DuplicateKeyError) as error:
        print(f"proof ledger load failed: {error}", file=sys.stderr)
        return 1
    result = validate_ledger(ledger, release=args.release)
    if result.errors:
        for error in result.errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    status = "release-complete" if result.machine_checked_completion else "release-incomplete"
    print(f"Sumeragi v2 proof ledger is structurally valid ({status})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
