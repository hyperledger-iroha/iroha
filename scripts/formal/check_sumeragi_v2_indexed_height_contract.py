#!/usr/bin/env python3
"""Fail closed on weakened indexed-height TLA+ proof boundaries."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


DECLARATION_RE = re.compile(
    r"(?m)^(?:THEOREM\s+)?[A-Za-z_][A-Za-z0-9_]*"
    r"(?:\([^)=\n]*\))?\s*=="
)


def normalized(text: str) -> str:
    """Return whitespace-normalized TLA+ source."""

    return " ".join(text.split())


def declaration(source: str, symbol: str, *, theorem: bool) -> tuple[str, str]:
    """Extract one top-level declaration and split statement from proof."""

    prefix = "THEOREM " if theorem else ""
    header = re.compile(
        rf"(?m)^{re.escape(prefix + symbol)}"
        r"(?:\([^)=\n]*\))?\s*==\s*"
    )
    matches = list(header.finditer(source))
    if len(matches) != 1:
        raise ValueError(
            f"{symbol}: expected exactly one {'theorem' if theorem else 'operator'}; "
            f"found {len(matches)}"
        )
    match = matches[0]
    following = DECLARATION_RE.search(source, match.end())
    end = following.start() if following is not None else len(source)
    body = source[match.end() : end]
    parts = re.split(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1)
    statement = parts[0]
    proof = parts[1] if len(parts) == 2 else ""
    return normalized(statement), proof


def validate(module_path: Path) -> list[str]:
    """Validate the exact non-vacuous child proof contract."""

    source = module_path.read_text(encoding="utf-8")
    errors: list[str] = []

    operator_contracts = {
        "IndexedExactHeightEntrySource": (
            "IF initialContext.height = 0 "
            "THEN initialContext = GenesisContext "
            "ELSE /\\ initialContext = "
            "CanonicalIndexedContext(initialContext.height) "
            "/\\ IndexedSuccessorActivationPending( "
            "CanonicalIndexedContext(initialContext.height - 1), node)"
        ),
        "IndexedExactContextCompleted": (
            "IF initialContext.height = MaxHeight "
            "THEN IndexedAllResponsiveExactApplicationsAt(initialContext) "
            "ELSE LET nextContext == "
            "CanonicalIndexedContext(initialContext.height + 1) "
            "IN \\A node \\in Responsive: "
            "node \\in joinedByContext[nextContext]"
        ),
        "IndexedExactHeightLivenessProperty": (
            "(/\\ VerificationContext \\in AdmissibleContextRecords "
            "/\\ VerificationContext \\in JoinedContexts "
            "/\\ IndexedCore(VerificationContext, 7)) "
            "~> IndexedExactContextCompleted(VerificationContext)"
        ),
    }
    for symbol, expected in operator_contracts.items():
        try:
            statement, _ = declaration(source, symbol, theorem=False)
        except ValueError as error:
            errors.append(str(error))
            continue
        if statement != expected:
            errors.append(
                f"{symbol} must equal only {expected!r}; found {statement!r}"
            )

    theorem = "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress"
    expected_theorem = (
        "/\\ IndexedChainSpec "
        "/\\ IndexedExactHistoricalRecoveryProgress "
        "/\\ IndexedSuccessorActivationProgress "
        "/\\ VerificationOneHeightCompletion "
        "=> IndexedExactHeightLivenessProperty"
    )
    try:
        statement, proof = declaration(source, theorem, theorem=True)
    except ValueError as error:
        errors.append(str(error))
    else:
        if statement != expected_theorem:
            errors.append(
                f"{theorem} must state only {expected_theorem!r}; "
                f"found {statement!r}"
            )
        required_proof_tokens = (
            "HeightLivenessFromOneHeightAndExactRecoveryProgress",
            "IndexedProjectedCompletionReachesExactCompletion",
            "IndexedTargetJoinedIsStable",
            "IndexedExactHeightLivenessProperty",
        )
        missing = tuple(token for token in required_proof_tokens if token not in proof)
        if missing:
            errors.append(f"{theorem} proof omits exact dependencies {missing!r}")
        if re.search(
            r"(?:\bOBVIOUS\b|\bASSUME\s+FALSE\b|\bBY\s+TRUE\b|"
            r"\bPROVE\s+TRUE\b)",
            proof,
        ):
            errors.append(f"{theorem} proof contains a vacuous assertion")

    wrapper = "IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs"
    try:
        _, wrapper_proof = declaration(source, wrapper, theorem=True)
    except ValueError as error:
        errors.append(str(error))
    else:
        if theorem not in wrapper_proof:
            errors.append(
                f"{wrapper} must consume {theorem} rather than bypass exact membership"
            )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("module", type=Path)
    args = parser.parse_args()
    errors = validate(args.module)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("indexed-height source contract is exact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
