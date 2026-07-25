#!/usr/bin/env python3
"""Fail closed if timeout readiness drifts from the Core action guard."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


DECLARATION_RE = re.compile(
    r"(?m)^(?:(?:LOCAL\s+)?(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+)?"
    r"[A-Za-z_][A-Za-z0-9_]*(?:\([^)=\n]*\))?\s*=="
)


def normalized(text: str) -> str:
    """Return whitespace-normalized TLA+ source."""

    return " ".join(text.split())


def declaration(
    source: str, symbol: str, *, theorem: bool = False
) -> tuple[str, str]:
    """Extract one top-level operator or theorem statement and its proof."""

    if theorem:
        prefix = r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+"
    else:
        prefix = ""
    header = re.compile(
        rf"(?m)^{prefix}{re.escape(symbol)}"
        r"(?:\([^)=\n]*\))?\s*==\s*"
    )
    matches = list(header.finditer(source))
    if len(matches) != 1:
        kind = "theorem" if theorem else "operator"
        raise ValueError(
            f"{symbol}: expected exactly one top-level {kind}; found {len(matches)}"
        )
    match = matches[0]
    following = DECLARATION_RE.search(source, match.end())
    footer = re.search(r"(?m)^={10,}\s*$", source[match.end() :])
    candidate_ends = [len(source)]
    if following is not None:
        candidate_ends.append(following.start())
    if footer is not None:
        candidate_ends.append(match.end() + footer.start())
    end = min(candidate_ends)
    body = source[match.end() : end]
    parts = re.split(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1)
    return normalized(parts[0]), normalized(parts[1] if len(parts) == 2 else "")


def validate(core_path: Path, network_path: Path, proof_path: Path) -> list[str]:
    """Validate the exact shared guard, both consumers, and its proof."""

    errors: list[str] = []
    sources: dict[str, str] = {}
    for name, path in (
        ("Core", core_path),
        ("AsyncNetwork", network_path),
        ("proof", proof_path),
    ):
        try:
            sources[name] = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read {name} source: {error}")
    if errors:
        return errors

    expected_ready = normalized(
        r"""
LET roundView == nodeView[node]
    request == TimeoutRequestFor(node)
IN /\ node \in Honest \cap up \cap CurrentVoters
   /\ NodeIdle(node)
   /\ NoDecisionForNode(node)
   /\ ~NodeTimedOut(node, roundView)
   /\ request \in TimeoutWalSet
"""
    )
    try:
        ready, _ = declaration(sources["Core"], "BeginTimeoutReady")
    except ValueError as error:
        errors.append(f"{core_path}: {error}")
    else:
        if ready != expected_ready:
            errors.append(
                f"{core_path}: BeginTimeoutReady must retain the exact complete "
                f"unprimed Core guard {expected_ready!r}; found {ready!r}"
            )

    try:
        begin, _ = declaration(sources["Core"], "BeginTimeout")
    except ValueError as error:
        errors.append(f"{core_path}: {error}")
    else:
        exact_prefix = (
            "LET request == TimeoutRequestFor(node) "
            "IN /\\ BeginTimeoutReady(node) "
            "/\\ pendingTimeout' = pendingTimeout \\cup {request}"
        )
        if not begin.startswith(exact_prefix):
            errors.append(
                f"{core_path}: BeginTimeout must consume BeginTimeoutReady as "
                "its first and only guard before the durable pending write"
            )
        if begin.count("BeginTimeoutReady(node)") != 1:
            errors.append(
                f"{core_path}: BeginTimeout must invoke BeginTimeoutReady(node) "
                "exactly once"
            )
        duplicated_guards = tuple(
            guard
            for guard in (
                "NodeIdle(node)",
                "NoDecisionForNode(node)",
                "NodeTimedOut(node",
                "TimeoutWalSet",
            )
            if guard in begin
        )
        if duplicated_guards:
            errors.append(
                f"{core_path}: BeginTimeout duplicates shared readiness guards "
                f"outside the pure kernel: {duplicated_guards!r}"
            )

    try:
        scheduler, _ = declaration(
            sources["AsyncNetwork"], "BeginTimeoutEnabled"
        )
    except ValueError as error:
        errors.append(f"{network_path}: {error}")
    else:
        if scheduler != "BeginTimeoutReady(node)":
            errors.append(
                f"{network_path}: BeginTimeoutEnabled must equal only the shared "
                f"BeginTimeoutReady(node) kernel; found {scheduler!r}"
            )

    theorem_contracts = {
        "BeginTimeoutReadyExactlyCharacterizesEnabledAction": (
            "\\A node \\in ValidatorIds: BeginTimeoutReady(node) "
            "<=> ENABLED BeginTimeout(node)",
            "ExpandENABLED, Isa DEF BeginTimeoutReady, BeginTimeout, "
            "TimeoutRequestFor, vars",
        ),
        "SchedulerTimeoutGuardExactlyMatchesCoreReadiness": (
            "\\A node \\in ValidatorIds: BeginTimeoutEnabled(node) "
            "<=> BeginTimeoutReady(node)",
            "DEF BeginTimeoutEnabled",
        ),
    }
    for theorem, (expected_statement, expected_proof) in theorem_contracts.items():
        try:
            statement, proof = declaration(
                sources["proof"], theorem, theorem=True
            )
        except ValueError as error:
            errors.append(f"{proof_path}: {error}")
            continue
        if statement != expected_statement:
            errors.append(
                f"{proof_path}: {theorem} must state only the exact non-vacuous "
                f"guard equivalence {expected_statement!r}; found {statement!r}"
            )
        if proof != expected_proof:
            errors.append(
                f"{proof_path}: {theorem} must retain the exact proof dependency "
                f"{expected_proof!r}; found {proof!r}"
            )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("core", type=Path)
    parser.add_argument("network", type=Path)
    parser.add_argument("proof", type=Path)
    args = parser.parse_args()
    errors = validate(args.core, args.network, args.proof)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("BeginTimeout readiness source contract is exact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
